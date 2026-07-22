use std::path::Path;
use std::process::Command;
use tauri::command;

// ==================== 启动类命令（收官 F2 / F3） ====================
//
// 这类命令只负责"拉起外部进程"，不接管后续交互：
//   - launch_radmin_vpn：仅启动 Radmin VPN 应用，加入网络由用户在 Radmin UI 里操作。
//   - launch_game：优先用 Steam 协议拉起正版（AppID 1623730），兜底直接拉起 Palworld.exe。
//
// 约定（与主理人铁律一致）：
//   - 统一 std::process::Command::spawn() 非阻塞拉起（不 wait）。
//   - 错误以 Result<String, String> 返回，错误信息为中文人话。

/// Radmin VPN 候选安装路径（按常见度排序探测，找到第一个存在的即返回）。
const RADMIN_CANDIDATE_PATHS: &[&str] = &[
    r"C:\Program Files\Radmin VPN\RadminVPN.exe",
    r"C:\Program Files (x86)\Radmin VPN\RadminVPN.exe",
    r"C:\Program Files\Famatech\Radmin VPN\RadminVPN.exe",
    r"D:\Program Files\Radmin VPN\RadminVPN.exe",
    r"D:\Program Files (x86)\Radmin VPN\RadminVPN.exe",
];

/// 探测 Radmin VPN 可执行文件，返回首个存在的完整路径。
fn find_radmin_exe() -> Option<String> {
    for path in RADMIN_CANDIDATE_PATHS {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

/// F2 · 启动 Radmin VPN（仅拉起应用，不接管"加入网络"）。
#[command]
pub async fn launch_radmin_vpn() -> Result<String, String> {
    let exe = find_radmin_exe().ok_or_else(|| "未找到 Radmin VPN，请确认已安装".to_string())?;
    Command::new(&exe)
        .spawn()
        .map_err(|e| format!("启动 Radmin VPN 失败: {}", e))?;
    Ok("已启动 Radmin VPN".to_string())
}

/// Palworld 候选可执行文件路径（按常见度排序探测）。
/// 老板默认盘 E:\SteamLibrary，外加通用 Steam 库相对路径兜底。
fn find_palworld_exe() -> Option<String> {
    const CANDIDATES: [&str; 4] = [
        r"E:\SteamLibrary\steamapps\common\Palworld\Pal\Binaries\Win64\Palworld-Win64-Shipping.exe",
        r"E:\Steam\steamapps\common\Palworld\Pal\Binaries\Win64\Palworld-Win64-Shipping.exe",
        r"D:\SteamLibrary\steamapps\common\Palworld\Pal\Binaries\Win64\Palworld-Win64-Shipping.exe",
        r"C:\Program Files (x86)\Steam\steamapps\common\Palworld\Pal\Binaries\Win64\Palworld-Win64-Shipping.exe",
    ];
    for path in CANDIDATES {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

/// F3 · 启动游戏本体。
/// 优先：`cmd /c start "" steam://rungame/1623730`（老板拥有正版，AppID 1623730）。
/// 兜底：从 Steam 库探测 Palworld.exe 直接拉起。
/// 再兜底：若两者均失败，返回明确中文错误。
#[command]
pub async fn launch_game() -> Result<String, String> {
    // 优先：通过 Steam 协议拉起。空标题 "" 避免 start 把含 ":" 的 URL 误判为窗口标题。
    let steam_ok = Command::new("cmd")
        .args(["/c", "start", "", "steam://rungame/1623730"])
        .spawn()
        .is_ok();

    if steam_ok {
        return Ok("已通过 Steam 启动游戏（steam://rungame/1623730）".to_string());
    }

    // 兜底：从 Steam 库探测 Palworld.exe 直接拉起。
    if let Some(exe) = find_palworld_exe() {
        Command::new(&exe)
            .spawn()
            .map_err(|e| format!("启动游戏失败: {}", e))?;
        return Ok("已直接启动游戏（Palworld.exe）".to_string());
    }

    // 双重兜底：steam 协议与 exe 探测均失败。
    Err(
        "未找到 Palworld 可执行文件，且 Steam 协议启动失败，请确认已安装 Steam 版幻兽帕鲁"
            .to_string(),
    )
}
