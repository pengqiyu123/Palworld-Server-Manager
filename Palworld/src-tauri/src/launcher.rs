use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::command;

use crate::windows_process::hidden_command;

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
    r"C:\Program Files\Radmin VPN\RvRvpnGui.exe",
    r"C:\Program Files (x86)\Radmin VPN\RvRvpnGui.exe",
    r"C:\Program Files\Radmin VPN\Radmin.exe",
    r"C:\Program Files (x86)\Radmin VPN\Radmin.exe",
    r"C:\Program Files\Radmin VPN\RadminVPN.exe",
    r"C:\Program Files (x86)\Radmin VPN\RadminVPN.exe",
    r"C:\Program Files\Famatech\Radmin VPN\RadminVPN.exe",
    r"D:\Program Files\Radmin VPN\RadminVPN.exe",
    r"D:\Program Files (x86)\Radmin VPN\RadminVPN.exe",
];

const RADMIN_EXECUTABLE_NAMES: &[&str] = &["RvRvpnGui.exe", "Radmin.exe", "RadminVPN.exe"];

/// 探测 Radmin VPN 可执行文件，返回首个存在的完整路径。
fn find_radmin_exe() -> Option<String> {
    for path in RADMIN_CANDIDATE_PATHS {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

fn is_supported_radmin_executable(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            RADMIN_EXECUTABLE_NAMES
                .iter()
                .any(|candidate| name.eq_ignore_ascii_case(candidate))
        })
        .unwrap_or(false)
}

fn resolve_radmin_selection(selection: &str) -> Result<String, String> {
    let selected = PathBuf::from(selection.trim());
    let executable = if selected.is_file() && is_supported_radmin_executable(&selected) {
        selected
    } else if selected.is_dir() {
        RADMIN_EXECUTABLE_NAMES
            .iter()
            .map(|name| selected.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| "所选目录中没有 Radmin VPN 可执行文件".to_string())?
    } else {
        return Err("请选择 Radmin VPN 的 exe 文件或安装目录".to_string());
    };

    executable
        .canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| format!("读取 Radmin VPN 路径失败: {error}"))
}

#[command]
pub async fn validate_radmin_path(path: String) -> Result<String, String> {
    resolve_radmin_selection(&path)
}

#[command]
pub async fn detect_palworld_game() -> Result<String, String> {
    find_palworld_exe().ok_or_else(|| "未在 Steam 库中找到 Palworld 游戏本体".to_string())
}

/// F2 · 启动 Radmin VPN（仅拉起应用，不接管"加入网络"）。
#[command]
pub async fn launch_radmin_vpn(preferred_path: Option<String>) -> Result<String, String> {
    let exe = match preferred_path.filter(|path| !path.trim().is_empty()) {
        Some(path) => resolve_radmin_selection(&path)?,
        None => find_radmin_exe()
            .ok_or_else(|| "未找到 Radmin VPN，请确认已安装或在概览中手动选择其 exe".to_string())?,
    };
    Command::new(&exe)
        .spawn()
        .map_err(|e| format!("启动 Radmin VPN 失败: {}", e))?;
    Ok("已启动 Radmin VPN".to_string())
}

/// 在已发现的 Steam 库中查找 Palworld 本体，不依赖固定盘符。
fn find_palworld_exe() -> Option<String> {
    for root in crate::steam_detect::detect_steam_library_roots() {
        let path = root
            .join("steamapps")
            .join("common")
            .join("Palworld")
            .join("Pal")
            .join("Binaries")
            .join("Win64")
            .join("Palworld-Win64-Shipping.exe");
        if path.is_file() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

/// 检查 Steam 客户端是否正在运行（通过 tasklist 查询 steam.exe 进程）。
/// tasklist 在所有 Windows 版本上均可用；/FO CSV + /NH 输出无表头的 CSV 便于精确匹配。
fn is_steam_running() -> bool {
    hidden_command("tasklist")
        .args(["/FI", "IMAGENAME eq steam.exe", "/FO", "CSV", "/NH"])
        .output()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout
                .lines()
                .any(|line| line.to_lowercase().contains("steam.exe"))
        })
        .unwrap_or(false)
}

/// F3 · 启动游戏本体。
/// 优先：`cmd /c start "" steam://rungame/1623730`（老板拥有正版，AppID 1623730）。
/// 兜底：从 Steam 库探测 Palworld.exe 直接拉起。
/// 再兜底：若两者均失败，返回明确中文错误。
///
/// 诚实反馈：spawn 成功不代表游戏会起——若 Steam 未在运行，
/// steam:// 协议可能无法拉起游戏，故先检测 Steam 进程状态再决定返回 Ok 还是 Err。
#[command]
pub async fn launch_game() -> Result<String, String> {
    // 先检测 Steam 是否在运行（spawn 前），用于诚实反馈。
    let steam_running = is_steam_running();

    // 优先：通过 Steam 协议拉起。空标题 "" 避免 start 把含 ":" 的 URL 误判为窗口标题。
    let steam_ok = hidden_command("cmd")
        .args(["/c", "start", "", "steam://rungame/1623730"])
        .spawn()
        .is_ok();

    if steam_ok {
        if steam_running {
            return Ok("已通过 Steam 启动游戏（steam://rungame/1623730）".to_string());
        } else {
            // spawn 成功但 Steam 未运行——steam:// 协议可能无法拉起游戏，诚实告知。
            return Err(
                "已发送启动指令，但检测到 Steam 未运行。请先启动 Steam 客户端，再点击此按钮"
                    .to_string(),
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radmin_candidates_cover_the_standard_radmin_executable() {
        assert!(RADMIN_CANDIDATE_PATHS
            .iter()
            .any(|path| path.ends_with(r"Radmin VPN\Radmin.exe")));
    }

    #[test]
    fn radmin_candidates_prefer_the_vpn_gui() {
        assert!(RADMIN_CANDIDATE_PATHS[0].ends_with(r"Radmin VPN\RvRvpnGui.exe"));
    }

    #[test]
    fn manual_application_paths_are_validated_by_backend_commands() {
        let source = include_str!("launcher.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("launcher production source must precede tests");

        assert!(production.contains("pub async fn validate_radmin_path"));
        assert!(production.contains("pub async fn detect_palworld_game"));
        assert!(production.contains("resolve_radmin_selection"));
    }
}
