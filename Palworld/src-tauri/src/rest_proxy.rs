//! REST API 代理模块。
//!
//! 所有帕鲁 REST API（端口 8212）调用经 Rust 代理，AdminPassword 在 Rust 侧
//! 从 PalWorldSettings.ini 读取、不传前端。前端只传 `server_path`。
//!
//! 认证：HTTP Basic Auth，username=`admin`，password=AdminPassword。
//! Base URL：`http://127.0.0.1:{RESTAPIPort}`（默认 8212，从配置读取）。

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::command;

use crate::config;

// ==================== 全局复用 HTTP Client（Q2 遗留清理） ====================
// 避免每次 REST 调用都重建连接池 / 重复 TCP 握手。30s 超时 + 默认连接池。
use once_cell::sync::Lazy;

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("构建全局 HTTP 客户端失败")
});

// ==================== REST 响应结构体 ====================

/// GET /v1/api/info 响应：服名/版本/世界GUID
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInfo {
    pub version: String,
    pub servername: String,
    pub description: String,
    pub worldguid: String,
}

/// GET /v1/api/metrics 响应：FPS/人数/天数/运行时长
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerMetrics {
    pub currentplayernum: u32,
    pub serverfps: f64,
    pub serverfpsaverage: f64,
    pub serverframetime: f64,
    pub days: u32,
    pub maxplayernum: u32,
    pub basecampnum: u32,
    pub uptime: u64,
}

/// GET /v1/api/players 响应中的单个玩家。
/// 注意：REST API 返回的字段名有大小写混合（iP / userId / playerId / location_x），
/// serde 直接按原字段名反序列化，透传给前端。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerInfo {
    pub name: String,
    #[serde(rename = "playerId")]
    pub player_id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "iP")]
    pub ip: String,
    pub ping: f64,
    pub location_x: f64,
    pub location_y: f64,
    pub level: u32,
}

/// /v1/api/players 的外层 JSON 包装：{"players": [...]}
#[derive(Deserialize)]
struct PlayersResponse {
    players: Vec<PlayerInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ManagementConnectionInfo {
    pub message: String,
    pub host: String,
    pub port: u16,
    pub servername: String,
    pub version: String,
}

#[derive(Debug, PartialEq, Eq)]
enum ManagementCommand {
    Info,
    ShowPlayers,
    Save,
    Broadcast(String),
    Shutdown { waittime: u32, message: String },
}

// ==================== 内部辅助函数 ====================

/// 从 server_path 拼接配置文件路径并读取 AdminPassword + RESTAPIPort。
///
/// 路径：`{server_path}/Pal/Saved/Config/WindowsServer/PalWorldSettings.ini`
///
/// 返回 `(admin_password, rest_port)`，密码为空字符串时仍返回（调用方会收到 401 错误）。
fn read_rest_config(server_path: &str) -> Result<(String, u16), String> {
    let config_path = Path::new(server_path)
        .join("Pal")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
        .join("PalWorldSettings.ini");

    let config_map =
        config::read_config_from_file(config_path.to_str().ok_or("服务器路径包含非法字符")?)?;

    let admin_password = config::extract_admin_password(&config_map);
    let rest_port = config::extract_rest_port(&config_map);

    Ok((admin_password, rest_port))
}

fn split_command(input: &str) -> (&str, &str) {
    let trimmed = input.trim();
    match trimmed.find(char::is_whitespace) {
        Some(index) => (&trimmed[..index], trimmed[index..].trim()),
        None => (trimmed, ""),
    }
}

fn parse_management_command(input: &str) -> Result<ManagementCommand, String> {
    let (name, arguments) = split_command(input);
    if name.is_empty() {
        return Err("请输入管理命令".to_string());
    }

    match name.to_ascii_lowercase().as_str() {
        "info" if arguments.is_empty() => Ok(ManagementCommand::Info),
        "showplayers" if arguments.is_empty() => Ok(ManagementCommand::ShowPlayers),
        "save" | "saveworld" if arguments.is_empty() => Ok(ManagementCommand::Save),
        "broadcast" | "announce" => {
            if arguments.is_empty() {
                Err("公告内容不能为空，例如：Broadcast 服务器即将维护".to_string())
            } else {
                Ok(ManagementCommand::Broadcast(arguments.to_string()))
            }
        }
        "shutdown" => {
            let (seconds, message) = split_command(arguments);
            let waittime = seconds
                .parse::<u32>()
                .map_err(|_| "关服命令格式：Shutdown 秒数 公告内容".to_string())?;
            if !(1..=3600).contains(&waittime) {
                return Err("关服倒计时必须在 1 到 3600 秒之间".to_string());
            }
            if message.is_empty() {
                return Err("关服公告不能为空，例如：Shutdown 60 服务器即将关闭".to_string());
            }
            Ok(ManagementCommand::Shutdown {
                waittime,
                message: message.to_string(),
            })
        }
        _ => Err(
            "当前版本不支持任意 RCON 命令。可用命令：Info、ShowPlayers、Save、Broadcast、Shutdown"
                .to_string(),
        ),
    }
}

/// 统一处理 reqwest 错误 → 人话中文错误信息。
fn map_reqwest_error(err: reqwest::Error) -> String {
    if err.is_connect() {
        "REST API 不可达：请确认服务器已启动且 RESTAPIEnabled=True".to_string()
    } else if err.is_timeout() {
        "REST 请求超时：服务器响应过慢或网络异常".to_string()
    } else {
        format!("REST 请求失败: {}", err)
    }
}

fn http_status_error_message(status: reqwest::StatusCode, body: &str) -> String {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        if body.contains("AdminPassword is empty") {
            return "REST 认证失败：服务器实际管理密码为空。迁移存档中的 WorldOption.sav 可能覆盖了 PalWorldSettings.ini，请停服后禁用该文件再启动。".to_string();
        }
        return "REST 认证失败：请检查 AdminPassword 配置".to_string();
    }
    format!("REST 请求失败: {} {}", status, body)
}

/// 统一处理 HTTP 响应状态码 → 人话中文错误信息。
async fn check_http_status(resp: reqwest::Response) -> Result<reqwest::Response, String> {
    if resp.status().is_success() {
        return Ok(resp);
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    Err(http_status_error_message(status, &body))
}

// ==================== GET 类命令（读取状态） ====================

/// GET /v1/api/info — 服名/版本/世界GUID
#[command]
pub async fn rest_get_info(server_path: String) -> Result<ServerInfo, String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/info", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .get(&url)
        .basic_auth("admin", Some(&password))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    let resp = check_http_status(resp).await?;
    let info = resp
        .json::<ServerInfo>()
        .await
        .map_err(|e| format!("解析服务器信息失败: {}", e))?;

    Ok(info)
}

/// GET /v1/api/metrics — FPS/人数/天数/运行时长
#[command]
pub async fn rest_get_metrics(server_path: String) -> Result<ServerMetrics, String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/metrics", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .get(&url)
        .basic_auth("admin", Some(&password))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    let resp = check_http_status(resp).await?;
    let metrics = resp
        .json::<ServerMetrics>()
        .await
        .map_err(|e| format!("解析服务器指标失败: {}", e))?;

    Ok(metrics)
}

/// GET /v1/api/players — 在线玩家列表
#[command]
pub async fn rest_get_players(server_path: String) -> Result<Vec<PlayerInfo>, String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/players", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .get(&url)
        .basic_auth("admin", Some(&password))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    let resp = check_http_status(resp).await?;
    let players_resp = resp
        .json::<PlayersResponse>()
        .await
        .map_err(|e| format!("解析玩家列表失败: {}", e))?;

    Ok(players_resp.players)
}

#[command]
pub async fn rest_management_connect(
    server_path: String,
) -> Result<ManagementConnectionInfo, String> {
    let (_, port) = read_rest_config(&server_path)?;
    let info = rest_get_info(server_path).await?;
    Ok(ManagementConnectionInfo {
        message: "服务器管理接口已连接".to_string(),
        host: "127.0.0.1".to_string(),
        port,
        servername: info.servername,
        version: info.version,
    })
}

// ==================== POST 类命令（管理动作） ====================

/// POST /v1/api/kick — 踢人（body: {"userid": "steam_xxx"}）
#[command]
pub async fn rest_kick_player(server_path: String, userid: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/kick", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "userid": userid }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/ban — 封人（body: {"userid": "steam_xxx"}）
#[command]
pub async fn rest_ban_player(server_path: String, userid: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/ban", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "userid": userid }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/unban — 解封（body: {"userid": "steam_xxx"}）
#[command]
pub async fn rest_unban_player(server_path: String, userid: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/unban", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "userid": userid }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/announce — 全服广播（body: {"message": "..."}）
#[command]
pub async fn rest_announce(server_path: String, message: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/announce", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "message": message }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/save - 请求服务器立即保存世界。
#[command]
pub async fn rest_save_world(server_path: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/save", port);
    let resp = HTTP_CLIENT
        .post(&url)
        .basic_auth("admin", Some(&password))
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/shutdown — 优雅关服（body: {"waittime": 30, "message": "..."}）
///
/// REST /shutdown 是异步的：服务器在 waittime 秒后自行退出，
/// 前端不等返回就显示「正在关闭」状态，进程退出由 server-status-change 事件检测。
#[command]
pub async fn rest_shutdown(
    server_path: String,
    waittime: u32,
    message: String,
) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/shutdown", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "waittime": waittime, "message": message }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

#[command]
pub async fn rest_execute_management_command(
    server_path: String,
    command: String,
) -> Result<String, String> {
    match parse_management_command(&command)? {
        ManagementCommand::Info => {
            let info = rest_get_info(server_path).await?;
            Ok(format!(
                "服务器：{}\n版本：{}\n世界：{}",
                info.servername, info.version, info.worldguid
            ))
        }
        ManagementCommand::ShowPlayers => {
            let players = rest_get_players(server_path).await?;
            if players.is_empty() {
                return Ok("当前没有在线玩家".to_string());
            }
            let details = players
                .iter()
                .map(|player| format!("{}（等级 {}）", player.name, player.level))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(format!("在线玩家 {} 人\n{}", players.len(), details))
        }
        ManagementCommand::Save => {
            rest_save_world(server_path).await?;
            Ok("世界保存请求已完成".to_string())
        }
        ManagementCommand::Broadcast(message) => {
            rest_announce(server_path, message).await?;
            Ok("公告已发送".to_string())
        }
        ManagementCommand::Shutdown { waittime, message } => {
            rest_shutdown(server_path, waittime, message).await?;
            Ok(format!("服务器将在 {} 秒后关闭", waittime))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    #[test]
    fn management_console_only_accepts_rest_backed_commands() {
        assert!(matches!(
            parse_management_command("Info"),
            Ok(ManagementCommand::Info)
        ));
        assert!(matches!(
            parse_management_command("ShowPlayers"),
            Ok(ManagementCommand::ShowPlayers)
        ));
        assert!(matches!(
            parse_management_command("Save"),
            Ok(ManagementCommand::Save)
        ));
        assert!(matches!(
            parse_management_command("Broadcast 维护测试"),
            Ok(ManagementCommand::Broadcast(message)) if message == "维护测试"
        ));
        assert!(matches!(
            parse_management_command("Shutdown 60 服务器即将关闭"),
            Ok(ManagementCommand::Shutdown { waittime: 60, message }) if message == "服务器即将关闭"
        ));

        let error = parse_management_command("TeleportToPlayer 123")
            .expect_err("任意 RCON 命令必须被明确拒绝");
        assert!(error.contains("当前版本不支持"));
        assert!(error.contains("Info"));
    }

    #[test]
    fn empty_runtime_admin_password_points_to_world_option_override() {
        let message = http_status_error_message(
            reqwest::StatusCode::UNAUTHORIZED,
            "Unauthorized (AdminPassword is empty)",
        );

        assert!(message.contains("WorldOption.sav"));
        assert!(message.contains("覆盖"));
        assert!(!message.contains("请检查 AdminPassword 配置"));
    }

    #[tokio::test]
    async fn save_world_sends_explicit_zero_content_length() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("应创建本地测试监听器");
        let port = listener.local_addr().unwrap().port();
        let (request_tx, request_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("应收到保存请求");
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("应读取请求头");
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            request_tx
                .send(String::from_utf8_lossy(&bytes).into_owned())
                .unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .unwrap();
        });

        let root = std::env::temp_dir().join(format!(
            "palworld-rest-save-header-{}-{}",
            std::process::id(),
            port
        ));
        let config_dir = root.join("Pal/Saved/Config/WindowsServer");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("PalWorldSettings.ini"),
            format!(
                "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(AdminPassword=\"test\",RESTAPIPort={port})\n"
            ),
        )
        .unwrap();

        rest_save_world(root.to_string_lossy().into_owned())
            .await
            .expect("测试服务器返回 200");
        server.join().unwrap();
        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        let _ = std::fs::remove_dir_all(root);

        assert!(
            request.contains("content-length: 0\r\n"),
            "空 POST 必须显式发送 Content-Length: 0，实际请求头:\n{request}"
        );
    }
}
