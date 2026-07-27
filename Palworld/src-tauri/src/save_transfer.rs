use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::command;

// ==================== 存档/角色转移（F4 · MVP） ====================
//
// 范围（参考 docs/save-transfer-research.md）：
//   P0 整包世界备份/恢复：整目录拷贝 <SaveGames>/<World>/（其下可能含 <GUID>/ 子层，
//      内含 Level.sav + LevelMeta.sav + Players/）。整目录递归拷贝，GUID 层一并保留。
//   P1 角色跨服导出/导入：拷出/拷入 <World>/<GUID>/Players/<steam_id>.sav，保持 steam_id 不变，
//      不处理公会。世界数据层兼容扁平(<World>/)与 GUID 嵌套(<World>/<GUID>/)两种布局。
//   "预置存档"：仅前端预留入口/占位，本模块不实现完整 presets 体系（注意与 presets.rs 配置预设区分）。
//
// 约定（与主理人铁律一致）：
//   - 纯 std::fs，不解析/改写 .sav 内容（安全、版本无关）。
//   - 错误以 Result<String, String> 返回，中文人话。
//   - 所有用户输入（world_name / steam_id / backup_id）做文件名白名单校验，杜绝路径穿越。

/// 世界基本信息（discover_worlds / discover_local_worlds 返回）。
#[derive(Serialize, Clone, Debug)]
pub struct WorldInfo {
    /// 世界名（= SaveGames 下的子目录名）
    pub name: String,
    /// 世界目录绝对路径
    pub path: String,
    /// 是否含 Level.sav（世界主体）
    pub has_level_sav: bool,
    /// Players/ 下角色存档文件(.sav)数量
    pub player_count: usize,
    /// 整目录字节数（递归估算）
    pub size_bytes: u64,
    /// 来源：专用服 = "server"；本机单机 = "appdata"（AppData Local）或 "steam"（Steam 库）
    pub source: String,
    /// 世界 GUID（GUID 嵌套布局下的子层目录名；扁平布局为 None）
    pub guid: Option<String>,
    /// 世界目录最后修改时间（格式化字符串，cheap stat 避免点击展开二次命令）
    pub modified_at: Option<String>,
}

/// discover_worlds 的返回：含实际发现的存档根，便于前端提示"自动发现"。
#[derive(Serialize, Clone, Debug)]
pub struct DiscoverResult {
    /// 实际使用的 SaveGames 根目录
    pub save_root: String,
    /// 是否非 server_path 直拼（向上扫描/默认位置发现）→ 前端应提示用户核对
    pub auto_discovered: bool,
    /// 世界列表
    pub worlds: Vec<WorldInfo>,
}

/// 单个世界备份条目（list_world_backups 返回）。
#[derive(Serialize, Clone, Debug)]
pub struct WorldBackupInfo {
    /// 备份 id（= _backups/<world>/ 下的时间戳子目录名）
    pub backup_id: String,
    /// 备份目录绝对路径
    pub path: String,
    /// 备份创建时间（格式化字符串）
    pub created_at: String,
    /// 备份目录字节数
    pub size_bytes: u64,
}

// ==================== 路径发现 ====================

/// 候选默认 Steam 库根（老板默认 E:\SteamLibrary；外加常见盘符兜底）。
/// ⚠️ 仅作为「最后兜底」：当动态探测 `detect_steam_library_roots()` 完全失败时启用，
/// 不应作为常规主探测路径（R1 写死隐患）。
const STEAM_LIBRARY_ROOTS: &[&str] = &[
    "E:\\SteamLibrary",
    "E:\\Steam",
    "D:\\SteamLibrary",
    "D:\\Steam",
    "C:\\Program Files (x86)\\Steam",
    "C:\\Program Files\\Steam",
];

/// 把 server_path 解析为 SaveGames 根目录。
///
/// 策略（探不到逐级降级，并如实回报路径）：
///   1. 直接用 server_path 拼 `Pal/Saved/SaveGames`（最常见）。
///   2. server_path 指向更深/更浅一层时，从 server_path 逐级向上扫描，
///      对每个祖先尝试 `Pal/Saved/SaveGames`。
///   3. 兜底：扫描常见 Steam 库根下的 `steamapps/common/Palworld/Pal/Saved/SaveGames`。
///
/// 返回 (save_root, auto_discovered)。
pub(crate) fn resolve_save_games_root() -> Result<(PathBuf, bool), String> {
    let settings = crate::settings::load_settings()?;
    let server_path = settings.server_path.trim().to_string();

    if !server_path.is_empty() {
        // 1. 直拼
        let direct = Path::new(&server_path)
            .join("Pal")
            .join("Saved")
            .join("SaveGames");
        if direct.is_dir() {
            return Ok((direct, false));
        }

        // 2. 向上扫描：从 server_path 逐级向上，对每个祖先尝试 Pal/Saved/SaveGames
        let mut cur = Path::new(&server_path);
        loop {
            // 当前层作为根：cur/Pal/Saved/SaveGames
            let cand = cur.join("Pal").join("Saved").join("SaveGames");
            if cand.is_dir() {
                return Ok((cand, true));
            }
            // 也尝试 cur 的父一层（兼容 server_path 已含 Pal 的情况）
            if let Some(parent) = cur.parent() {
                let cand2 = parent.join("Pal").join("Saved").join("SaveGames");
                if cand2.is_dir() {
                    return Ok((cand2, true));
                }
                if cur == parent {
                    break;
                }
                cur = parent;
            } else {
                break;
            }
        }
    }

    // 3. 兜底：动态探测 Steam 库（替代写死 STEAM_LIBRARY_ROOTS 主路径）
    let dynamic = crate::steam_detect::detect_steam_library_roots();
    // 仅当动态探测完全失败时才回退到写死兜底（不在主探测路径）
    let fallback: Vec<std::path::PathBuf> = if dynamic.is_empty() {
        STEAM_LIBRARY_ROOTS
            .iter()
            .map(std::path::PathBuf::from)
            .collect()
    } else {
        dynamic
    };
    for root in &fallback {
        let cand = root
            .join("steamapps")
            .join("common")
            .join("Palworld")
            .join("Pal")
            .join("Saved")
            .join("SaveGames");
        if cand.is_dir() {
            return Ok((cand, true));
        }
    }

    Err(
        "未找到存档目录（SaveGames）。请确认 settings 中的 server_path 指向 PalServer 安装目录，\
         或手动在『设置』中重新指定，以便定位 Pal/Saved/SaveGames。"
            .to_string(),
    )
}

// ==================== 文件名安全校验 ====================

/// 校验输入仅含安全文件名字符（字母/数字/下划线/点/连字符），且不含路径分隔符。
/// 防止 world_name / steam_id / backup_id 被用于路径穿越。
fn safe_name_segment(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    // 去掉任何可能的父目录成分（如 "a/b" → file_name "b"），并拒绝 Windows 驱动器等
    let base = Path::new(s).file_name()?.to_string_lossy().to_string();
    let allowed = base
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');
    if allowed {
        Some(base)
    } else {
        None
    }
}

/// 规范化 steam_id：去掉末尾的 ".sav"（若有），返回纯文件名基底。
fn normalize_steam_id(steam_id: &str) -> Option<String> {
    let trimmed = steam_id.trim();
    let base = if let Some(stripped) = trimmed.strip_suffix(".sav") {
        stripped
    } else {
        trimmed
    };
    safe_name_segment(base)
}

// ==================== 文件/目录工具 ====================

/// 递归拷贝目录（非破坏：目标不存在则创建；目标已有则覆盖写入）。
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("源目录不存在: {}", src.display()));
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("创建目标目录失败: {}", e))?;

    let entries = std::fs::read_dir(src).map_err(|e| format!("读取源目录失败: {}", e))?;
    for entry in entries.flatten() {
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取目录项失败: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("复制文件失败 {}: {}", src_path.display(), e))?;
        }
        // 符号链接等其他类型跳过（专用服存档无此类）
    }
    Ok(())
}

/// 递归估算目录字节数。
fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                let p = entry.path();
                if ft.is_dir() {
                    total += dir_size(&p);
                } else if ft.is_file() {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
    }
    total
}

fn configured_world_backup_root(world_dir: &Path, world: &str) -> PathBuf {
    if let Ok(settings) = crate::settings::load_settings() {
        if !settings.backup_root.trim().is_empty() {
            return PathBuf::from(settings.backup_root.trim()).join(world);
        }
    }
    world_dir
        .parent()
        .map(|p| p.join("_backups").join(world))
        .unwrap_or_else(|| world_dir.join("_backups").join(world))
}

fn find_matching_backup(world_dir: &Path, backup_root: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(backup_root)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| Some((e.path(), e.metadata().and_then(|m| m.modified()).ok()?)))
        .collect();
    candidates.sort_by_key(|(_, time)| std::cmp::Reverse(*time));
    candidates
        .into_iter()
        .find_map(|(path, _)| dirs_equal(world_dir, &path).then_some(path))
}

fn dirs_equal(left: &Path, right: &Path) -> bool {
    let mut left_files = Vec::new();
    let mut right_files = Vec::new();
    collect_files(left, left, &mut left_files);
    collect_files(right, right, &mut right_files);
    left_files.sort_by(|a, b| a.0.cmp(&b.0));
    right_files.sort_by(|a, b| a.0.cmp(&b.0));
    left_files == right_files
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if let Ok(bytes) = std::fs::read(&path) {
                out.push((rel, bytes));
            }
        }
    }
}

/// 统计 Players/ 下 .sav 数量。
fn count_player_saves(world_dir: &Path) -> usize {
    let players_dir = world_dir.join("Players");
    if !players_dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(&players_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("sav"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// 格式化为本地时间时间戳字符串（YYYYMMDD-HHmmss，Asia/Shanghai +8）。
/// 与 config.rs 的 format_timestamp 同口径，但此处独立实现以避免跨模块耦合。
fn format_timestamp(time: SystemTime) -> String {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = secs + 8 * 3600; // +8 小时（上海时区）
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, d) = days_to_ymd(days as i64);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y, mo, d, h, m, s)
}

fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    let mut y = 1970i64;
    let mut d = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let yd = if leap { 366 } else { 365 };
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0u32;
    let mut rem = d as u32;
    while (mo as usize) < mdays.len() && rem >= mdays[mo as usize] {
        rem -= mdays[mo as usize];
        mo += 1;
    }
    (y, mo + 1, rem + 1)
}

/// 定位世界数据目录（含 Level.sav 的那一层）。
///
/// Palworld 实际磁盘结构为 `SaveGames/<World>/<GUID>/Level.sav`（GUID 子层），
/// 但部分版本/外部导出可能是扁平的 `SaveGames/<World>/Level.sav`，
/// 也可能在更深的嵌套下（如本机单机导出的 `<SteamID>/<timestamp>/<GUID>/Level.sav`）。
///
/// 采用**有界 DFS（最大深度 ≤ 4）**：从 `world_dir` 出发逐层下探，
/// 命中首个含 `Level.sav` 的目录即返回。越界、无 Level.sav 或读目录失败均返回 `None`，
/// **绝不 panic**。（与 F5 `path_util::find_world_data_dir` 保持同一逻辑。）
fn find_world_data_dir(world_dir: &Path) -> Option<PathBuf> {
    const MAX_DEPTH: usize = 4;

    /// 递归下探：depth_left 为剩余可下探层数预算（不含当前层）。
    fn dfs(dir: &Path, depth_left: usize) -> Option<PathBuf> {
        // 1) 直接命中（扁平布局 / 已是最深层数据目录），优先返回，绝不继续下探子目录。
        if dir.join("Level.sav").is_file() {
            return Some(dir.to_path_buf());
        }
        // 2) 预算耗尽：不再下探。
        if depth_left == 0 {
            return None;
        }
        // 3) 读目录失败（权限/不存在/非目录）直接放弃该分支，整体返回 None 而非 panic。
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return None,
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(found) = dfs(&p, depth_left - 1) {
                    return Some(found);
                }
            }
        }
        None
    }

    dfs(world_dir, MAX_DEPTH)
}

/// 把 world 目录解析为世界信息（含 Players 数量与体积）。
///
/// 注意：世界主体(Level.sav)与 Players/ 可能位于 world_dir 直接层，
/// 也可能位于 world_dir/<GUID>/ 子层；通过 find_world_data_dir 兼容两者。
fn world_info_from_dir(world_dir: &Path) -> WorldInfo {
    let name = world_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    // 定位世界数据层（扁平或 GUID 嵌套），据此判断是否有 Level.sav 与统计玩家数
    let data_dir = find_world_data_dir(world_dir);
    let has_level_sav = data_dir.is_some();
    let player_count = match &data_dir {
        Some(d) => count_player_saves(d),
        None => 0,
    };
    // 体积按整个 world_dir 递归估算（含 GUID 子层、Players 等）
    let size_bytes = dir_size(world_dir);
    // GUID：仅当数据层嵌套在 GUID 子层（data_dir != world_dir）时取该层目录名
    let guid = match &data_dir {
        Some(d) if d.as_path() != world_dir => d
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string()),
        _ => None,
    };
    // 修改时间：世界目录的最后修改时间（cheap stat，避免点击展开时二次命令）
    let modified_at = std::fs::metadata(world_dir)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| format_timestamp(t));
    WorldInfo {
        name,
        path: world_dir.to_string_lossy().to_string(),
        has_level_sav,
        player_count,
        size_bytes,
        source: String::new(),
        guid,
        modified_at,
    }
}

/// 有界深度优先扫描：收集 `root` 下所有「自身直接含 `Level.sav`」的目录（即真正的世界数据层）。
///
/// 用于手动选目录兜底（`discover_local_worlds` 的 extra_root 分支），兼容本机单机导出的
/// `<SteamID>/<timestamp>/<GUID>/Level.sav` 等多层包裹结构。
/// `max_depth` 为从 `root` 出发允许下探的层数预算（不含 root 自身）。
///
/// - 命中 `root` 自身含 `Level.sav` 即收为数据层（扁平布局 / 已是最深目录），并不再下探其内部子目录。
/// - 收集的是「数据层目录」而非仅作包裹层的祖先目录（如 SteamID / timestamp），避免重复与误判。
/// - 遇不可读目录（`read_dir` 失败）静默跳过，整体**绝不 panic**；找不到返回空。
fn collect_world_data_dirs(root: &Path, max_depth: usize, out: &mut Vec<PathBuf>) {
    // 1) root 自身即为世界数据层（扁平布局 / 已是最深数据目录）→ 直接收下
    if root.join("Level.sav").is_file() {
        out.push(root.to_path_buf());
        return;
    }
    // 2) 预算耗尽：不再下探
    if max_depth == 0 {
        return;
    }
    // 3) 读目录失败（权限/不存在/非目录）直接放弃该分支，整体不 panic
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_world_data_dirs(&p, max_depth - 1, out);
        }
    }
}

// ==================== Tauri 命令 ====================

/// F4-P0：扫描 SaveGames 下含 Level.sav 的目录作为世界列表。
/// 返回实际发现的存档根，便于前端确认路径是否正确。
#[command]
pub async fn discover_worlds(extra_root: Option<String>) -> Result<DiscoverResult, String> {
    let (save_root, auto_discovered) = resolve_save_games_root()?;

    let mut worlds: Vec<WorldInfo> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let entries = std::fs::read_dir(&save_root).map_err(|e| format!("读取存档目录失败: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let mut info = world_info_from_dir(&path);
            // 仅列出含 Level.sav 的世界（健壮：忽略 _backups 等杂目录）
            if info.has_level_sav && seen.insert(info.path.clone()) {
                info.source = "server".to_string();
                worlds.push(info);
            }
        }
    }

    // 手动选目录兜底（与单机 discover_local_worlds 一致）：将用户指定目录作为额外扫描根，
    // 支持直接选 SaveGames，或选含 SaveGames 的父目录。合并去重，防止发现失败。
    if let Some(root) = extra_root {
        let root_path = Path::new(&root);
        let scan = if root_path.join("SaveGames").is_dir() {
            root_path.join("SaveGames")
        } else {
            root_path.to_path_buf()
        };
        if scan.is_dir() {
            if let Ok(es) = std::fs::read_dir(&scan) {
                for entry in es.flatten() {
                    let p = entry.path();
                    if !p.is_dir() {
                        continue;
                    }
                    let mut info = world_info_from_dir(&p);
                    if info.has_level_sav && seen.insert(info.path.clone()) {
                        info.source = "server".to_string();
                        worlds.push(info);
                    }
                }
            }
        }
    }

    // 按世界名排序，稳定展示
    worlds.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(DiscoverResult {
        save_root: save_root.to_string_lossy().to_string(),
        auto_discovered,
        worlds,
    })
}

/// 扫描给定 Steam 库根列表下的本机单机（Steam）存档。
///
/// 仅扫 `<root>/steamapps/common/Palworld/Pal/Saved/SaveGames`，按 `has_level_sav` 过滤、
/// 跨库去重（HashSet）、按名排序；未安装帕鲁的库静默跳过，路径访问异常也仅跳过该库（不 panic）。
/// 提取为独立函数以便单测注入临时根目录（不依赖真实 Steam 安装）。
fn discover_local_worlds_in(roots: &[&str]) -> Vec<WorldInfo> {
    let mut worlds: Vec<WorldInfo> = Vec::new();
    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for root in roots {
        let save_games = Path::new(root)
            .join("steamapps")
            .join("common")
            .join("Palworld")
            .join("Pal")
            .join("Saved")
            .join("SaveGames");
        // 该 Steam 库未装帕鲁单机，跳过
        if !save_games.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(&save_games) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let info = world_info_from_dir(&path);
            // 仅收集含 Level.sav 的世界，去重（跨库同路径不重复）
            if info.has_level_sav && seen_paths.insert(info.path.clone()) {
                worlds.push(info);
            }
        }
    }
    // 按世界名排序，稳定展示
    worlds.sort_by(|a, b| a.name.cmp(&b.name));
    worlds
}

/// R5：同时扫 ① Steam 库（动态探测根，替代写死 `STEAM_LIBRARY_ROOTS` 主路径）
/// 与 ② AppData Local Pal 单机档（`dirs::data_local_dir()/Pal/Saved/SaveGames`），
/// 并支持可选的 `extra_root`（手动选目录兜底）作为额外扫描根，合并去重后
/// 为每个 world 标 `source: "steam" | "appdata"`。
///
/// 未发现任何单机档时返回 `Ok(vec![])`（不报错，UI 优雅空态），
/// 因为单机档缺失属于正常现象；仅当路径访问异常时才 Err。
#[command]
pub async fn discover_local_worlds(extra_root: Option<String>) -> Result<Vec<WorldInfo>, String> {
    let mut worlds: Vec<WorldInfo> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1) Steam 库（动态探测，替代写死 STEAM_LIBRARY_ROOTS 主路径）
    let dyn_roots = crate::steam_detect::detect_steam_library_roots();
    // ⚠️ 不能对 `to_string_lossy()` 返回的临时 Cow 取借用（E0515），
    //    先物化到拥有所有权的 String 缓冲，再取 &str 切片。
    let owned: Vec<String> = dyn_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let root_strs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
    for mut w in discover_local_worlds_in(&root_strs) {
        w.source = "steam".to_string();
        if seen.insert(w.path.clone()) {
            worlds.push(w);
        }
    }

    // 2) AppData Local Pal 单机档（可移植，不依赖机器专属路径）
    if let Some(local) = dirs::data_local_dir() {
        let appdata_sg = local.join("Pal").join("Saved").join("SaveGames");
        if appdata_sg.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&appdata_sg) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if !p.is_dir() {
                        continue;
                    }
                    let mut w = world_info_from_dir(&p);
                    w.source = "appdata".to_string();
                    if w.has_level_sav && seen.insert(w.path.clone()) {
                        worlds.push(w);
                    }
                }
            }
        }
    }

    // 3) 手动选目录兜底：将用户指定的目录作为额外扫描根。
    //    支持直接选 SaveGames 目录、选含 SaveGames 的父目录，或选更深的本机单机存档
    //    （如 <SteamID>/<timestamp>/<GUID>/Level.sav 三层包裹结构）。
    //    采用有界 DFS（最大深度 3）穿透多层包裹目录，定位任何直接含 Level.sav 的世界数据层。
    if let Some(root) = extra_root {
        let root_path = Path::new(&root);
        let scan = if root_path.join("SaveGames").is_dir() {
            root_path.join("SaveGames")
        } else {
            root_path.to_path_buf()
        };
        if scan.is_dir() {
            // 有界深度优先扫描（最大深度 3）：收集所有直接含 Level.sav 的世界数据目录。
            // 不收集仅作包裹层的祖先目录，避免重复；再经 world_info_from_dir 解析完整信息。
            let mut world_dirs: Vec<PathBuf> = Vec::new();
            collect_world_data_dirs(&scan, 3, &mut world_dirs);
            for d in world_dirs {
                let mut w = world_info_from_dir(&d);
                w.source = "appdata".to_string();
                if w.has_level_sav && seen.insert(w.path.clone()) {
                    worlds.push(w);
                }
            }
        }
    }

    worlds.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(worlds)
}

/// F4-P0：整目录备份世界到备份夹。
/// 入参 `world_path` 为世界目录真实路径（本地/服务器通用，不再依赖设置中的 server_path）。
/// 默认目标：<世界同级>/_backups/<world>/<timestamp>/（非破坏，新建时间戳目录）；
/// 自定义 `dest` 视为「存放目录（父目录）」，实际落 dest/<world>/<timestamp>/，
/// 即使 dest 已存在（如 F:\1）也不冲突。
#[command]
pub async fn backup_world(world_path: String, dest: Option<String>) -> Result<String, String> {
    let world_dir = PathBuf::from(world_path.trim());
    if !world_dir.is_dir() {
        return Err(format!("世界目录不存在: {}", world_dir.display()));
    }
    // 世界名取末级目录名，并做白名单校验（防路径穿越）
    let raw = world_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "世界路径非法".to_string())?;
    let world = safe_name_segment(&raw).ok_or_else(|| "世界名含非法字符".to_string())?;

    let ts = format_timestamp(SystemTime::now());
    // 确定目标目录：
    //  - 自定义 dest = 「存放目录」，实际落 dest/<world>/<timestamp>/（即使 dest 已存在也不冲突）
    //  - 默认 = 世界目录同级的 _backups/<world>/<timestamp>/
    let backup_parent = match dest {
        Some(d) if !d.trim().is_empty() => {
            let p = PathBuf::from(d.trim());
            if !p.is_absolute() {
                return Err("自定义存放目录需为绝对路径（请用系统文件夹选择器）".to_string());
            }
            if p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err("备份目标路径不得包含 '..'".to_string());
            }
            p.join(&world)
        }
        _ => {
            let parent = world_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| world_dir.clone());
            parent.join("_backups").join(&world)
        }
    };

    if let Some(existing) = find_matching_backup(&world_dir, &backup_parent) {
        return Ok(format!("已存在相同世界备份：{}", existing.display()));
    }
    let backup_dir = backup_parent.join(&ts);

    if backup_dir.exists() {
        return Err(format!("备份目标已存在: {}", backup_dir.display()));
    }
    copy_dir_recursive(&world_dir, &backup_dir)?;
    let size = dir_size(&backup_dir);
    Ok(format!(
        "已备份世界「{}」到 {}（{:.2} MB）",
        world,
        backup_dir.display(),
        size as f64 / (1024.0 * 1024.0)
    ))
}

/// F4-P0：列出某世界的已有备份（restore 前供前端选择）。
#[command]
pub async fn list_world_backups(world_path: String) -> Result<Vec<WorldBackupInfo>, String> {
    let world_dir = PathBuf::from(world_path.trim());
    let raw = world_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "世界路径非法".to_string())?;
    let world = safe_name_segment(&raw).ok_or_else(|| "世界名非法".to_string())?;
    let backups_root = configured_world_backup_root(&world_dir, &world);
    if !backups_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut result: Vec<WorldBackupInfo> = Vec::new();
    let entries =
        std::fs::read_dir(&backups_root).map_err(|e| format!("读取备份目录失败: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let backup_id = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            // 仅接受安全段名，过滤异常项
            if safe_name_segment(&backup_id).is_none() {
                continue;
            }
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);
            let created_at = format_timestamp(mtime);
            let size_bytes = dir_size(&path);
            result.push(WorldBackupInfo {
                backup_id,
                path: path.to_string_lossy().to_string(),
                created_at,
                size_bytes,
            });
        }
    }
    // 按时间倒序（最新在前）
    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(result)
}

/// F4-P0：把备份整体覆盖回当前世界目录。
/// ⚠️ 由 UI 侧做二次确认 + 提醒先停服（运行中替换会被自动保存覆盖，见风险 R3）。
#[command]
pub async fn restore_world(world_path: String, backup_id: String) -> Result<String, String> {
    let world_dir = PathBuf::from(world_path.trim());
    let raw = world_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "世界路径非法".to_string())?;
    let world = safe_name_segment(&raw).ok_or_else(|| "世界名非法".to_string())?;
    let bid = safe_name_segment(&backup_id).ok_or_else(|| "备份 id 非法".to_string())?;
    let backup_dir = configured_world_backup_root(&world_dir, &world).join(&bid);
    if !backup_dir.is_dir() {
        return Err(format!("备份不存在: {}", backup_dir.display()));
    }
    if !world_dir.is_dir() {
        return Err(format!("目标世界目录不存在: {}", world_dir.display()));
    }

    // 覆盖拷贝（先清空再拷，保证备份完全还原）
    std::fs::remove_dir_all(&world_dir).map_err(|e| format!("清理当前世界失败: {}", e))?;
    copy_dir_recursive(&backup_dir, &world_dir)?;

    Ok(format!(
        "已用备份「{}」恢复世界「{}」（含 Level.sav / Players/ 等）",
        bid, world
    ))
}

/// F4-P0：从用户指定的自定义备份目录整体覆盖回当前世界目录。
/// 用于「指定文件夹存放」的备份还原；仅拒绝路径穿越('..')。
#[command]
pub async fn restore_world_from(world_path: String, src: String) -> Result<String, String> {
    let world_dir = PathBuf::from(world_path.trim());
    let raw = world_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "世界路径非法".to_string())?;
    let world = safe_name_segment(&raw).ok_or_else(|| "世界名非法".to_string())?;
    if !world_dir.is_dir() {
        return Err(format!("目标世界目录不存在: {}", world_dir.display()));
    }
    let src_path = PathBuf::from(src.trim());
    // 仅拒绝含父目录回退('..')的路径穿越
    if src_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("源路径不得包含 '..'".to_string());
    }
    if !src_path.is_dir() {
        return Err(format!("源备份目录不存在: {}", src_path.display()));
    }

    // 覆盖拷贝（先清空再拷，保证完全还原）
    std::fs::remove_dir_all(&world_dir).map_err(|e| format!("清理当前世界失败: {}", e))?;
    copy_dir_recursive(&src_path, &world_dir)?;

    Ok(format!(
        "已从 {} 恢复世界「{}」（含 Level.sav / Players/ 等）",
        src_path.display(),
        world
    ))
}

/// F4-P1：导出单个角色存档到指定路径（保持文件名 = <steam_id>.sav）。
#[command]
pub async fn export_character(
    world_name: String,
    steam_id: String,
    dest_path: String,
) -> Result<String, String> {
    let (save_root, _auto) = resolve_save_games_root()?;
    let world = safe_name_segment(&world_name).ok_or_else(|| "世界名非法".to_string())?;
    let sid = normalize_steam_id(&steam_id)
        .ok_or_else(|| "SteamID 非法（仅允许字母/数字/下划线/点/连字符）".to_string())?;

    let world_dir = save_root.join(&world);
    if !world_dir.is_dir() {
        return Err(format!("世界目录不存在: {}", world_dir.display()));
    }
    // 角色存档位于世界数据层（扁平为 world_dir/Players，GUID 嵌套为 world_dir/<GUID>/Players）
    let data_dir = find_world_data_dir(&world_dir)
        .ok_or_else(|| format!("未找到世界数据(Level.sav)：{}", world_dir.display()))?;
    let src = data_dir.join("Players").join(format!("{}.sav", sid));
    if !src.is_file() {
        return Err(format!("角色存档不存在: {}", src.display()));
    }

    let dest = PathBuf::from(dest_path.trim());
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败: {}", e))?;
        }
    }
    std::fs::copy(&src, &dest).map_err(|e| format!("导出角色失败: {}", e))?;

    Ok(format!("已导出角色 {} → {}", sid, dest.display()))
}

/// F4-P1：导入角色存档（保持 steam_id 不变，覆盖同名文件）。
/// ⚠️ 首版不处理公会（GroupSaveDataMap）；UI 应提示"公会归属可能丢失"。
#[command]
pub async fn import_character(
    world_name: String,
    steam_id: String,
    src_path: String,
) -> Result<String, String> {
    let (save_root, _auto) = resolve_save_games_root()?;
    let world = safe_name_segment(&world_name).ok_or_else(|| "世界名非法".to_string())?;
    let sid = normalize_steam_id(&steam_id)
        .ok_or_else(|| "SteamID 非法（仅允许字母/数字/下划线/点/连字符）".to_string())?;

    let src = PathBuf::from(src_path.trim());
    if !src.is_file() {
        return Err(format!("源文件不存在: {}", src.display()));
    }
    // 简单校验：源应为 .sav
    if src
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| !e.eq_ignore_ascii_case("sav"))
        .unwrap_or(true)
    {
        return Err("源文件不是 .sav 角色存档".to_string());
    }

    let world_dir = save_root.join(&world);
    if !world_dir.is_dir() {
        return Err(format!("世界目录不存在: {}", world_dir.display()));
    }
    // 角色存档落点：世界数据层下的 Players/（兼容扁平与 GUID 嵌套）
    let data_dir = find_world_data_dir(&world_dir)
        .ok_or_else(|| format!("未找到世界数据(Level.sav)：{}", world_dir.display()))?;
    let players_dir = data_dir.join("Players");
    std::fs::create_dir_all(&players_dir).map_err(|e| format!("创建 Players 目录失败: {}", e))?;
    let dst = players_dir.join(format!("{}.sav", sid));
    std::fs::copy(&src, &dst).map_err(|e| format!("导入角色失败: {}", e))?;

    Ok(format!(
        "已导入角色 {} → {}（SteamID 保持不变；公会归属未迁移，属预期）",
        sid,
        dst.display()
    ))
}

// ==================== 安全归并工具 ====================
// 备注：`normalize_within`（旧 backup_world 的相对 dest 归并辅助）在本轮重构后已无调用方，已移除：
// 新版 dest 直接作为「存放目录（父目录）」，由 backup_world 自行处理绝对路径与 '..' 校验。

// ==================== 单元测试（GUID 嵌套结构冒烟校验） ====================

#[cfg(test)]
mod tests {
    use super::*;

    /// 针对本机真实 Palworld 专用服存档结构（SaveGames/<World>/<GUID>/Level.sav）做一次冒烟校验。
    /// 仅当该路径存在时执行，避免在无此路径的 CI 环境中失败。
    #[test]
    fn discover_real_palserver_world() {
        let root =
            Path::new("E:\\SteamLibrary\\steamapps\\common\\PalServer\\Pal\\Saved\\SaveGames");
        if !root.is_dir() {
            eprintln!("skip: 本机无 PalServer 存档目录，跳过冒烟校验");
            return;
        }
        // 真实世界目录：SaveGames/0/<GUID>/Level.sav
        let world_dir = root.join("0");
        let data_dir = find_world_data_dir(&world_dir);
        assert!(
            data_dir.is_some(),
            "world '0' 应定位到含 Level.sav 的数据层（扁平或 GUID 嵌套）"
        );
        let dd = data_dir.unwrap();
        assert!(dd.join("Level.sav").is_file(), "数据层应含 Level.sav");
        let info = world_info_from_dir(&world_dir);
        assert!(info.has_level_sav, "WorldInfo.has_level_sav 应为 true");
        // 若 Players 目录存在，玩家计数应与 count_player_saves 一致
        let players = dd.join("Players");
        if players.is_dir() {
            assert_eq!(info.player_count, count_player_saves(&dd));
        }
        eprintln!(
            "ok: 发现世界 '{}' 数据层={} players={}",
            info.name,
            dd.display(),
            info.player_count
        );
    }

    /// R3：discover_local_worlds 命令本身不依赖真实 Steam 库，仅断言始终返回 Ok
    /// （无档=空数组，有档=发现列表，均合法，UI 优雅空态）。
    #[tokio::test]
    async fn discover_local_worlds_returns_ok() {
        let res = discover_local_worlds(None).await;
        assert!(res.is_ok(), "discover_local_worlds 应始终返回 Ok（不报错）");
        // 返回结构合法即可（内容取决于本机是否装有 Steam 单机帕鲁）
        let _ = res.unwrap();
    }

    /// R3：用临时目录构造 SaveGames/<World>/<GUID>/Level.sav 结构，
    /// 验证 discover_local_worlds 依赖的辅助逻辑（find_world_data_dir / world_info_from_dir）。
    #[test]
    fn world_info_from_fake_steam_structure() {
        let base = std::env::temp_dir()
            .join("palworld_ui2_test")
            .join("SaveGames");
        let _ = std::fs::remove_dir_all(&base);
        let world = base.join("TestWorld");
        let guid = world.join("ABC12345");
        std::fs::create_dir_all(&guid).unwrap();
        std::fs::write(guid.join("Level.sav"), b"fake-level").unwrap();
        // Players/ 位于 GUID 数据层内（与真实 Palworld 结构一致）
        std::fs::create_dir_all(guid.join("Players")).unwrap();
        std::fs::write(guid.join("Players").join("76561190000000001.sav"), b"x").unwrap();

        let dd = find_world_data_dir(&world);
        assert!(
            dd.is_some(),
            "应定位到含 Level.sav 的数据层（扁平或 GUID 嵌套）"
        );
        let info = world_info_from_dir(&world);
        assert!(info.has_level_sav, "WorldInfo.has_level_sav 应为 true");
        assert_eq!(info.player_count, 1, "应统计到 1 个角色存档");
        assert!(info.path.ends_with("TestWorld"), "path 应指向世界目录");

        let _ = std::fs::remove_dir_all(&base);
    }

    /// R3：用临时目录构造 SaveGames/<SteamID>/<GUID>/Level.sav 结构，
    /// 通过可注入根列表的内部函数验证 discover_local_worlds 的扫描/过滤/排序逻辑：
    /// 应返回 1 个世界且 name 等于 SteamID，player_count 正确。
    #[test]
    fn discover_local_worlds_finds_one_world_from_temp() {
        let root = std::env::temp_dir().join("palworld_local_worlds_ui2");
        let _ = std::fs::remove_dir_all(&root);
        // .../steamapps/common/Palworld/Pal/Saved/SaveGames/<SteamID>/<GUID>/Level.sav
        let save_games = root
            .join("steamapps")
            .join("common")
            .join("Palworld")
            .join("Pal")
            .join("Saved")
            .join("SaveGames");
        let world_dir = save_games.join("76561190000000001");
        let guid_dir = world_dir.join("ABC12345-DEAD-BEEF-0001");
        std::fs::create_dir_all(&guid_dir).unwrap();
        std::fs::write(guid_dir.join("Level.sav"), b"fake-level").unwrap();
        std::fs::create_dir_all(guid_dir.join("Players")).unwrap();
        std::fs::write(guid_dir.join("Players").join("76561190000000001.sav"), b"x").unwrap();

        let root_str = root.to_str().expect("temp path should be UTF-8");
        let worlds = discover_local_worlds_in(&[root_str]);
        assert_eq!(worlds.len(), 1, "临时 Steam 库应发现 1 个世界");
        assert_eq!(
            worlds[0].name, "76561190000000001",
            "世界名应为 SaveGames 子目录名（SteamID）"
        );
        assert!(worlds[0].has_level_sav, "应检测到 Level.sav");
        assert_eq!(worlds[0].player_count, 1, "应统计到 1 个角色存档");
        assert!(
            worlds[0].path.ends_with("76561190000000001"),
            "path 应指向世界目录"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// R5（修复）：手动选目录（`extra_root`）支持 <SteamID>/<timestamp>/<GUID>/Level.sav
    /// 三层包裹结构。构造临时 root，选 root 作为 extra_root，验证有界 DFS（最大深度 3）
    /// 能下钻命中最深层数据目录，且返回世界的 path 含 {GUID}、has_level_sav==true。
    #[tokio::test]
    async fn discover_local_worlds_extra_root_finds_wrapped_world() {
        let root = std::env::temp_dir().join("palworld_local_extra_root_wrap");
        let _ = std::fs::remove_dir_all(&root);
        // root/<SteamID>/<timestamp>/<GUID>/Level.sav (+ Players/player.sav)
        let steam_id_dir = root.join("76561199381352956");
        let ts_dir = steam_id_dir.join("20260724-183232");
        let guid_dir = ts_dir.join("A1B2C3D4-0000-1111-2222-333344445555");
        std::fs::create_dir_all(&guid_dir).unwrap();
        std::fs::write(guid_dir.join("Level.sav"), b"fake-level").unwrap();
        std::fs::create_dir_all(guid_dir.join("Players")).unwrap();
        std::fs::write(guid_dir.join("Players").join("76561199381352956.sav"), b"x").unwrap();

        let guid_name = guid_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap()
            .to_string();
        let root_str = root
            .to_str()
            .expect("temp path should be UTF-8")
            .to_string();
        let worlds = discover_local_worlds(Some(root_str)).await.unwrap();

        let found = worlds
            .iter()
            .find(|w| w.path.contains(&guid_name) && w.has_level_sav)
            .expect("extra_root 有界 DFS 应命中三层包裹下的世界数据目录");
        assert!(
            found.path.ends_with(&guid_name),
            "path 应指向含 Level.sav 的数据层目录（{{GUID}}），实际: {}",
            found.path
        );
        assert_eq!(found.player_count, 1, "应统计到 1 个角色存档");
        assert_eq!(found.source, "appdata", "手动选目录来源标记为 appdata");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// 真实冒烟：用户把「存放目录」选为已存在的 F:\1 风格目录，
    /// 修复后 dest 作为存放目录、实际落 dest/<world>/<timestamp>/，不应再报「目标已存在」。
    /// 注：backup_world 为 async 命令，故此处用 #[tokio::test] + .await 驱动（与既有 discover_local_worlds_returns_ok 一致）。
    #[tokio::test]
    async fn backup_world_to_existing_custom_dest() {
        let base = std::env::temp_dir()
            .join("palworld_backup_f1_test")
            .join("SaveGames");
        let _ = std::fs::remove_dir_all(base.parent().unwrap());
        let world = base.join("0");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("Level.sav"), b"lv").unwrap();
        std::fs::create_dir_all(world.join("Players")).unwrap();
        std::fs::write(world.join("Players").join("123.sav"), b"p").unwrap();

        // 自定义存放目录「1」已存在（等价 F:\1）
        let dest = base.parent().unwrap().join("1");
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();

        let res = backup_world(
            world.to_string_lossy().to_string(),
            Some(dest.to_string_lossy().to_string()),
        )
        .await;
        assert!(
            res.is_ok(),
            "备份到已存在的自定义目录应成功，实际错误: {:?}",
            res.err()
        );

        // 验证落点为 dest/0/<timestamp>/ 且含 Level.sav
        let world_back = dest.join("0");
        assert!(world_back.is_dir(), "应在 dest/0 下生成备份");
        let mut found = false;
        if let Ok(es) = std::fs::read_dir(&world_back) {
            for e in es.flatten() {
                let p = e.path();
                if p.is_dir() && p.join("Level.sav").is_file() {
                    found = true;
                }
            }
        }
        assert!(found, "dest/0/<timestamp>/ 下应含 Level.sav");
        let second = backup_world(
            world.to_string_lossy().to_string(),
            Some(dest.to_string_lossy().to_string()),
        )
        .await
        .expect("相同世界再次备份应复用已有快照");
        assert!(second.contains("已存在相同世界备份"));
        assert_eq!(
            std::fs::read_dir(&world_back).unwrap().flatten().count(),
            1,
            "同名世界内容未变化时不得重复创建备份目录"
        );
        let _ = std::fs::remove_dir_all(base.parent().unwrap());
    }
}
