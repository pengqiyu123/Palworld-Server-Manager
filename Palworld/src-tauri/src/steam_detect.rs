//! 自动探测 PalServer.exe 所在目录
//!
//! 算法说明（已在目标机实测验证）：
//! 1. 从注册表读取 Steam 安装根目录（优先 HKLM，回退 HKCU）。
//! 2. 候选库根 = Steam 安装根本身 + 解析 libraryfolders.vdf（steamapps 与 config 两处）。
//! 3. 对每个库根检查 `<root>\steamapps\common\PalServer\PalServer.exe`，
//!    命中则把该 exe 的父目录（即 `.../PalServer` 目录，正是前端要填的 server_path）加入结果。
//! 4. 全程安全降级：任何注册表读取 / VDF 解析 / 路径检查异常都返回空或跳过该根，绝不 panic。

use std::path::PathBuf;
use tauri::command;

#[cfg(windows)]
use std::path::Path;

// ==================== VDF 解析纯函数（跨平台，便于单测） ====================

/// 解析 Steam `libraryfolders.vdf` 文本内容，提取全部 Steam 库根目录。
///
/// 兼容两种 VDF 格式：
/// - 新格式：`"path" "X:\foo"`（key 为 `path`，value 为库路径）
/// - 老格式：`"1" "X:\foo"`（数字键，value 为库路径）
///
/// 同时兼容转义反斜杠 `\\`，统一规整为 `\`。
///
/// # 参数
/// * `vdf_text` - libraryfolders.vdf 的完整文本内容
///
/// # 返回
/// 解析出的库根目录字符串列表（保留输入顺序，未做去重）。
pub fn parse_library_roots(vdf_text: &str) -> Vec<String> {
    let mut roots: Vec<String> = Vec::new();

    for raw_line in vdf_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || !line.starts_with('"') {
            continue;
        }

        // 按引号切分，过滤掉空片段（含纯空白分隔符），得到 [key, value, ...]
        let parts: Vec<&str> = line.split('"').filter(|s| !s.trim().is_empty()).collect();
        if parts.len() < 2 {
            continue;
        }

        let key = parts[0];
        let value = parts[1];

        // 新格式 key 为 "path"；老格式 key 为数字（如 "1"）
        let is_path_entry = key == "path" || key.parse::<u32>().is_ok();
        if !is_path_entry {
            continue;
        }

        // 规整转义反斜杠 "\\" -> "\"
        let normalized = value.replace("\\\\", "\\").trim().to_string();
        if normalized.is_empty() {
            continue;
        }

        roots.push(normalized);
    }

    roots
}

// ==================== Windows 平台真实实现 ====================

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
#[cfg(windows)]
use winreg::RegKey;

/// 从注册表读取 Steam 安装根目录。
///
/// 优先读取 `HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath`，
/// 失败则回退 `HKCU\SOFTWARE\Valve\Steam\SteamPath`。
///
/// 任何读取失败均返回 `None`（安全降级，不向上抛错）。
#[cfg(windows)]
fn read_steam_install_path() -> Option<String> {
    // 优先：HKLM 的 32 位视图（Steam 为 32 位程序，注册在 WOW6432Node）
    if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags("SOFTWARE\\WOW6432Node\\Valve\\Steam", KEY_READ)
    {
        if let Ok(path) = hklm.get_value::<String, _>("InstallPath") {
            let path = path.replace("\\\\", "\\").trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    // 回退：HKCU（部分便携 / 自定义安装会写在这里）
    if let Ok(hkcu) =
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("SOFTWARE\\Valve\\Steam", KEY_READ)
    {
        if let Ok(path) = hkcu.get_value::<String, _>("SteamPath") {
            let path = path.replace("\\\\", "\\").trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

/// 动态探测全部 Steam 库根目录（注册表 Steam 安装路径 + 两处 libraryfolders.vdf）。
///
/// 抽出 `detect_palserver_path` 内的库根推导逻辑，供 `save_transfer` / `path_util`
/// 的存档扫描复用，替代写死的 `STEAM_LIBRARY_ROOTS`。
///
/// 返回可能为空的列表；调用方应自行决定兜底策略
/// （保留 `STEAM_LIBRARY_ROOTS` 作为最后兜底，仅动态探测全失败时使用）。
#[cfg(windows)]
pub fn detect_steam_library_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let steam_root = match read_steam_install_path() {
        Some(p) => p,
        None => return roots,
    };
    roots.push(PathBuf::from(&steam_root));
    for vdf_rel in [
        "steamapps\\libraryfolders.vdf",
        "config\\libraryfolders.vdf",
    ] {
        let vdf_path = Path::new(&steam_root).join(vdf_rel);
        if let Ok(text) = std::fs::read_to_string(&vdf_path) {
            for r in parse_library_roots(&text) {
                roots.push(PathBuf::from(r));
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

/// 非 Windows 平台下返回空列表（无 Steam 库可探测），保证跨平台可编译。
#[cfg(not(windows))]
pub fn detect_steam_library_roots() -> Vec<PathBuf> {
    Vec::new()
}

/// 自动探测 PalServer.exe 所在目录（Windows 真实实现）。
///
/// 返回命中的 server_path 列表（即 `.../PalServer` 目录，可能为空）。
#[cfg(windows)]
#[command]
pub async fn detect_palserver_path() -> Vec<String> {
    // 1. 读取 Steam 安装根
    let steam_root = match read_steam_install_path() {
        Some(p) => p,
        None => return Vec::new(),
    };

    // 2. 候选库根 = Steam 安装根本身 + 两处 libraryfolders.vdf 解析结果
    let mut candidate_roots: Vec<String> = Vec::new();
    candidate_roots.push(steam_root.clone());

    // 两处 VDF 都查：steamapps 下与 config 下
    for vdf_rel in [
        "steamapps\\libraryfolders.vdf",
        "config\\libraryfolders.vdf",
    ] {
        let vdf_path = Path::new(&steam_root).join(vdf_rel);
        if let Ok(text) = std::fs::read_to_string(&vdf_path) {
            for root in parse_library_roots(&text) {
                candidate_roots.push(root);
            }
        }
    }

    // 去重，避免同一库根被重复检查
    candidate_roots.sort();
    candidate_roots.dedup();

    // 3. 逐个库根检查 PalServer.exe 是否存在
    let mut results: Vec<String> = Vec::new();
    for root in candidate_roots {
        let exe_path = Path::new(&root)
            .join("steamapps")
            .join("common")
            .join("PalServer")
            .join("PalServer.exe");
        if exe_path.is_file() {
            // 命中：返回该 exe 的父目录（即 PalServer 目录，正是前端要填的 server_path）
            if let Some(parent) = exe_path.parent() {
                results.push(parent.to_string_lossy().to_string());
            }
        }
    }

    results
}

// ==================== 非 Windows 平台桩实现 ====================

/// 非 Windows 平台下返回空列表，保证跨平台可编译（Windows 分支在 QA 环境会被编到）。
#[cfg(not(windows))]
#[command]
pub async fn detect_palserver_path() -> Vec<String> {
    Vec::new()
}

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_new_format_with_nested_blocks() {
        // 现代格式：库条目为带 "path" 键的嵌套块
        let vdf = r#"
"libraryfolders"
{
	"contentstatsid"		"1234567890"
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
	}
	"1"
	{
		"path"		"D:\\SteamGames"
	}
}
"#;
        let roots = parse_library_roots(vdf);
        assert!(roots.contains(&"C:\\Program Files (x86)\\Steam".to_string()));
        assert!(roots.contains(&"D:\\SteamGames".to_string()));
        // 数字键 "0"/"1" 本身不应被误判为路径
        assert!(!roots.contains(&"0".to_string()));
        assert!(!roots.contains(&"1".to_string()));
    }

    #[test]
    fn test_parse_old_format_flat() {
        // 老格式：数字键直接映射到路径字符串
        let vdf = r#"
"libraryfolders"
{
	"1"		"E:\\SteamLibrary"
	"2"		"F:\\Games\\Steam"
}
"#;
        let roots = parse_library_roots(vdf);
        assert!(roots.contains(&"E:\\SteamLibrary".to_string()));
        assert!(roots.contains(&"F:\\Games\\Steam".to_string()));
    }

    #[test]
    fn test_normalize_escaped_backslash() {
        // 转义反斜杠 "\\" 应被规整为 "\"
        let vdf = "\"path\"\t\t\"X:\\\\foo\\\\bar\"";
        let roots = parse_library_roots(vdf);
        assert_eq!(roots, vec!["X:\\foo\\bar".to_string()]);
    }

    #[test]
    fn test_empty_and_malformed() {
        assert!(parse_library_roots("").is_empty());
        assert!(parse_library_roots("not a vdf at all").is_empty());
        // 只有 key 没有 value 的行应被忽略
        assert!(parse_library_roots("\"path\"").is_empty());
    }
}
