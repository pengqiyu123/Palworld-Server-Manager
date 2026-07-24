use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::io::{Error as IoError, ErrorKind};
use std::net::UdpSocket;
use std::process::Command;
use tauri::{command, State};

// ==================== Radmin 5 档分级检测（M1 · 收官） ====================
//
// 老板确认：Radmin 虚拟网段为 26.x.x.x（26.0.0.0/8），不是设计初稿里的 25.x.x.x。
// 判定链（一次命令入口 check_radmin_readiness 拿全）：
//   L0 未装        → 网卡不存在
//   L1 已装未启动  → 网卡状态 ∈ {Disabled, Disconnected}
//   L2 已启动未入网 → 网卡 Up 但拿不到 26.x.x.x 段虚拟 IP
//   L3 已入网未就绪 → 拿到虚拟 IP，但服务器未运行 / 8211 未放行 / UDP bind 试探非 AddrInUse
//   L4 联机就绪    → L3 条件全满足 + UDP bind(26.x.x.x:8211) 返回 AddrInUse(10048)

/// Radmin 虚拟网段前缀（老板确认：26.x.x.x）。
pub const RADMIN_IP_PREFIX: &str = "26.";
/// Palworld 游戏 UDP 端口。
pub const PAL_UDP_PORT: u16 = 8211;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReadinessLevel {
    L0, // 未装
    L1, // 已装未启动
    L2, // 已启动未入网
    L3, // 已入网未就绪
    L4, // 联机就绪
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NextAction {
    /// "open_url" | "launch_app" | "show_guide" | "auto_recheck" | "copy_card"
    pub action_type: String,
    pub label: String,
    pub payload: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RadminReadiness {
    pub level: ReadinessLevel,
    pub virtual_ip: String,
    pub adapter_status: String,
    pub reason: Option<String>,
    pub next_action: Option<NextAction>,
}

impl RadminReadiness {
    pub fn l0() -> Self {
        RadminReadiness {
            level: ReadinessLevel::L0,
            virtual_ip: String::new(),
            adapter_status: String::new(),
            reason: Some("未检测到 Radmin VPN 网卡，请先安装".to_string()),
            next_action: Some(next_action(
                "open_url",
                "打开 Radmin 官网下载",
                Some("https://www.radmin-vpn.com/"),
            )),
        }
    }

    pub fn l1(adapter_status: String) -> Self {
        RadminReadiness {
            level: ReadinessLevel::L1,
            virtual_ip: String::new(),
            adapter_status,
            reason: Some("Radmin 网卡存在但未启动，请打开 Radmin VPN 客户端".to_string()),
            next_action: Some(next_action("launch_app", "打开 Radmin 客户端", Some("Radmin VPN"))),
        }
    }

    pub fn l2(virtual_ip: String, adapter_status: String) -> Self {
        RadminReadiness {
            level: ReadinessLevel::L2,
            virtual_ip,
            adapter_status,
            reason: Some(
                "Radmin 已启动但未加入虚拟网络，请在客户端里创建或加入一个网络".to_string(),
            ),
            next_action: Some(next_action(
                "show_guide",
                "在 Radmin 中创建/加入虚拟网络",
                None,
            )),
        }
    }

    pub fn l3(virtual_ip: String, adapter_status: String, reason: String) -> Self {
        RadminReadiness {
            level: ReadinessLevel::L3,
            virtual_ip,
            adapter_status,
            reason: Some(reason),
            next_action: Some(next_action(
                "auto_recheck",
                "稍后复查（先启动服务器并放行 8211）",
                None,
            )),
        }
    }

    pub fn l4(virtual_ip: String) -> Self {
        RadminReadiness {
            level: ReadinessLevel::L4,
            virtual_ip,
            adapter_status: "Up".to_string(),
            reason: Some("联机就绪：虚拟 IP 已拿到、服务器在监听、8211 已放行".to_string()),
            next_action: Some(next_action(
                "copy_card",
                "复制连法给朋友",
                None,
            )),
        }
    }
}

fn next_action(action_type: &str, label: &str, payload: Option<&str>) -> NextAction {
    NextAction {
        action_type: action_type.to_string(),
        label: label.to_string(),
        payload: payload.map(|s| s.to_string()),
    }
}

// ==================== PowerShell 辅助 ====================

fn run_powershell(script: &str) -> Result<String, String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| format!("执行 PowerShell 失败: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[derive(Deserialize)]
struct AdapterInfo {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "ifIndex")]
    if_index: u32,
}

/// 检测 Radmin 网卡：按适配器描述或名称含 "Radmin" 识别（兼容改名版本）。
/// 返回 (name, status, ifIndex)，找不到返回 None（L0）。
fn detect_adapter() -> Result<Option<(String, String, u32)>, String> {
    let script = "Get-NetAdapter | Where-Object { $_.InterfaceDescription -like '*Radmin*' -or $_.Name -like '*Radmin*' } | Select-Object -First 1 | ForEach-Object { $_ | Select-Object Name, Status, ifIndex | ConvertTo-Json -Compress }";
    let out = run_powershell(script)?;
    if out.is_empty() {
        return Ok(None);
    }
    let info: AdapterInfo =
        from_str(&out).map_err(|e| format!("解析网卡信息失败: {}", e))?;
    if info.name.is_empty() {
        return Ok(None);
    }
    Ok(Some((info.name, info.status, info.if_index)))
}

/// 根据 ifIndex 取 IPv4 虚拟 IP（空字符串表示未拿到）。
fn detect_virtual_ip(if_index: u32) -> String {
    let script = format!(
        "Get-NetIPAddress -InterfaceIndex {} -AddressFamily IPv4 -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty IPAddress",
        if_index
    );
    run_powershell(&script).unwrap_or_default()
}

/// UDP bind 试探结果（★D2）：
/// - Listening：AddrInUse(10048) → PalServer 正在 0.0.0.0:8211 监听 = L4 通过信号
/// - Free：绑定成功 → 8211 未监听 = L3
/// - Error：其他错误（如虚拟 IP 未就绪）= L3
enum BindResult {
    Listening,
    Free,
    Error(String),
}

fn probe_udp_bind_8211(ip: &str) -> BindResult {
    // bind 本身瞬时完成，无需超时；socket 在本函数返回时自动 drop，不占用 8211。
    match UdpSocket::bind((ip, PAL_UDP_PORT)) {
        Ok(_) => BindResult::Free,
        Err(e) if e.kind() == ErrorKind::AddrInUse => BindResult::Listening,
        Err(e) => BindResult::Error(e.to_string()),
    }
}

// ==================== Tauri 命令：5 档检测 ====================

#[command]
pub async fn check_radmin_readiness(
    state: State<'_, crate::server::ServerState>,
    server_path: String,
) -> Result<RadminReadiness, String> {
    // server_path 预留给前端统一传参；本命令通过网卡/进程/防火墙实际状态判定。
    let _ = &server_path;

    // Step1 + Step2：网卡存在？状态 Up？
    let adapter = detect_adapter()?;
    let (name, status, if_index) = match adapter {
        Some(a) => a,
        None => return Ok(RadminReadiness::l0()),
    };
    let _ = name;
    if status == "Disabled" || status == "Disconnected" {
        return Ok(RadminReadiness::l1(status));
    }

    // Step3：拿到 26.x.x.x 段虚拟 IP？
    let virtual_ip = detect_virtual_ip(if_index);
    if virtual_ip.is_empty() || !virtual_ip.starts_with(RADMIN_IP_PREFIX) {
        return Ok(RadminReadiness::l2(virtual_ip, status));
    }

    // Step4：服务器进程运行 + 防火墙 8211 已放行？
    let server_running = crate::server::is_server_process_running(state.inner());
    let fw = crate::firewall::check_firewall_rules().await?;
    if !server_running {
        return Ok(RadminReadiness::l3(
            virtual_ip,
            status,
            "服务器未运行，请先启动服务器".to_string(),
        ));
    }
    if !fw.port_8211_open {
        return Ok(RadminReadiness::l3(
            virtual_ip,
            status,
            "UDP 8211 未放行，请在防火墙放行后再复查".to_string(),
        ));
    }

    // Step5：UDP bind 试探（★D2）
    match probe_udp_bind_8211(&virtual_ip) {
        BindResult::Listening => Ok(RadminReadiness::l4(virtual_ip)),
        BindResult::Free => Ok(RadminReadiness::l3(
            virtual_ip,
            status,
            "8211 未真正监听（PalServer 可能未启动成功）".to_string(),
        )),
        BindResult::Error(e) => Ok(RadminReadiness::l3(
            virtual_ip,
            status,
            format!("8211 探测失败: {}", e),
        )),
    }
}

// ==================== 旧版（R2 兼容期，R3 起删除） ====================

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RadminStatus {
    pub installed: bool,
    pub virtual_ip: String,
    pub adapter_status: String,
}

pub fn check_radmin_lan() -> Result<RadminStatus, String> {
    // 检测Radmin VPN适配器
    let adapter_output = Command::new("powershell")
        .args([
            "-Command",
            "Get-NetAdapter | Where-Object { $_.InterfaceDescription -like '*Famatech Radmin VPN*' } | Select-Object -First 1 -ExpandProperty Status",
        ])
        .output()
        .map_err(|e| format!("检测Radmin适配器失败: {}", e))?;

    let adapter_status = String::from_utf8_lossy(&adapter_output.stdout).trim().to_string();

    if adapter_status.is_empty() {
        return Ok(RadminStatus {
            installed: false,
            virtual_ip: String::new(),
            adapter_status: String::new(),
        });
    }

    // 获取虚拟IP（注意：此处沿用 R2 旧的 25.x 段兜底，仅用于兼容老前端；新前端请改用 check_radmin_readiness）
    let ip_output = Command::new("powershell")
        .args([
            "-Command",
            "Get-NetIPAddress -InterfaceAlias 'Radmin VPN' -AddressFamily IPv4 | Select-Object -ExpandProperty IPAddress",
        ])
        .output()
        .map_err(|e| format!("获取Radmin IP失败: {}", e))?;

    let virtual_ip = String::from_utf8_lossy(&ip_output.stdout).trim().to_string();

    Ok(RadminStatus {
        installed: true,
        virtual_ip,
        adapter_status,
    })
}

pub fn get_local_ip() -> Result<String, String> {
    let output = Command::new("powershell")
        .args([
            "-Command",
            "Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*' -and $_.InterfaceAlias -notlike '*Radmin*' -and $_.InterfaceAlias -notlike '*Loopback*' -and $_.InterfaceAlias -notlike '*WSL*' -and $_.InterfaceAlias -notlike '*Hyper*' } | Select-Object -First 1 -ExpandProperty IPAddress",
        ])
        .output()
        .map_err(|e| format!("获取本地IP失败: {}", e))?;

    let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if ip.is_empty() {
        Err("无法获取本地IP".to_string())
    } else {
        Ok(ip)
    }
}

// ==================== Tauri 命令 ====================

#[derive(Serialize, Deserialize, Clone)]
pub struct RadminLanStatus {
    pub installed: bool,
    pub virtual_ip: String,
    pub adapter_status: String,
}

#[command]
pub async fn check_port_usage(port: u16) -> Result<Option<String>, String> {
    let port_str = shell_escape::escape(port.to_string().into());
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("netstat -ano -p UDP | findstr :{}", port_str),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let result = String::from_utf8_lossy(&output.stdout);
    if result.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(result.trim().to_string()))
    }
}

/// R2 旧命令：仅返回 installed / virtual_ip / adapter_status。
/// ⚠️ Deprecated since R3：新前端应使用 `check_radmin_readiness`（5 档分级）。
/// 保留期为 R2→R3 过渡，避免老前端调用方直接报错。
#[deprecated(
    since = "R3",
    note = "Use check_radmin_readiness (5-level) instead; kept for R2 frontend compatibility."
)]
#[command]
pub async fn check_radmin_lan_status() -> Result<RadminLanStatus, String> {
    let status = check_radmin_lan().map_err(|e| e)?;
    let local_ip = get_local_ip().unwrap_or_default();

    Ok(RadminLanStatus {
        installed: status.installed,
        virtual_ip: if status.installed {
            status.virtual_ip
        } else {
            local_ip
        },
        adapter_status: status.adapter_status,
    })
}

// ==================== 单元测试（QA 收官 · 严过关） ====================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::net::UdpSocket;

    #[test]
    fn radmin_ip_prefix_is_26() {
        // 老板确认：Radmin 虚拟网段为 26.x.x.x（设计初稿 25.x 已更正）
        assert_eq!(RADMIN_IP_PREFIX, "26.");
    }

    #[tokio::test]
    async fn port_usage_ignores_a_tcp_listener_when_checking_a_game_udp_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        assert_eq!(check_port_usage(port).await.unwrap(), None);

        drop(listener);
    }

    #[test]
    fn pal_udp_port_is_8211() {
        assert_eq!(PAL_UDP_PORT, 8211);
    }

    #[test]
    fn readiness_constructors_produce_correct_level_and_fields() {
        let l0 = RadminReadiness::l0();
        assert_eq!(l0.level, ReadinessLevel::L0);
        assert!(l0.virtual_ip.is_empty());
        assert!(l0.reason.is_some());
        assert!(l0.next_action.is_some());
        assert_eq!(l0.next_action.unwrap().action_type, "open_url");

        let l1 = RadminReadiness::l1("Disabled".to_string());
        assert_eq!(l1.level, ReadinessLevel::L1);
        assert_eq!(l1.adapter_status, "Disabled");
        assert_eq!(l1.next_action.unwrap().action_type, "launch_app");

        let l2 = RadminReadiness::l2("".to_string(), "Up".to_string());
        assert_eq!(l2.level, ReadinessLevel::L2);

        let l3 = RadminReadiness::l3(
            "26.1.2.3".to_string(),
            "Up".to_string(),
            "reason xyz".to_string(),
        );
        assert_eq!(l3.level, ReadinessLevel::L3);
        assert_eq!(l3.reason.unwrap(), "reason xyz");
        assert_eq!(l3.next_action.unwrap().action_type, "auto_recheck");

        let l4 = RadminReadiness::l4("26.1.2.3".to_string());
        assert_eq!(l4.level, ReadinessLevel::L4);
        assert_eq!(l4.virtual_ip, "26.1.2.3");
        assert_eq!(l4.adapter_status, "Up");
        assert_eq!(l4.next_action.unwrap().action_type, "copy_card");
    }

    #[test]
    fn readiness_serializes_level_uppercase_for_frontend_contract() {
        // 前端 ReadinessLevel 类型为 'L0'..'L4'，依赖 serde rename_all = "UPPERCASE"
        let r = RadminReadiness::l4("26.9.9.9".to_string());
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["level"], json!("L4"));
        assert_eq!(v["virtual_ip"], json!("26.9.9.9"));
        assert!(v["next_action"].is_object());
        // 降档 reason 透传（前端 fail 态展示）
        let l3 = RadminReadiness::l3(
            "26.1.1.1".to_string(),
            "Up".to_string(),
            "8211 未放行".to_string(),
        );
        let v3 = serde_json::to_value(&l3).unwrap();
        assert_eq!(v3["level"], json!("L3"));
        assert_eq!(v3["reason"], json!("8211 未放行"));
    }

    #[test]
    fn probe_udp_bind_classifies_listening_and_free() {
        // ★D2：占用 127.0.0.1:8211 时 probe 应判定为 Listening（AddrInUse = L4 通过信号）
        match UdpSocket::bind("127.0.0.1:8211") {
            Ok(held) => {
                assert!(matches!(
                    probe_udp_bind_8211("127.0.0.1"),
                    BindResult::Listening
                ));
                drop(held);
                // 释放后端口空闲 → Free
                assert!(matches!(probe_udp_bind_8211("127.0.0.1"), BindResult::Free));
            }
            Err(_) => {
                // 端口已被外部进程占用（如 PalServer）→ 本身就是 Listening 信号
                assert!(matches!(
                    probe_udp_bind_8211("127.0.0.1"),
                    BindResult::Listening
                ));
            }
        }
    }

    #[test]
    fn probe_udp_bind_invalid_ip_is_error() {
        let r = probe_udp_bind_8211("not-a-valid-ip");
        assert!(
            matches!(r, BindResult::Error(_)),
            "expected Error variant for invalid ip"
        );
    }
}
