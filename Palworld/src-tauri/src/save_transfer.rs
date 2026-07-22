use serde::{Deserialize, Serialize};
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

/// 世界基本信息（discover_worlds 返回）。
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
/// 用于 server_path 完全无法定位存档时的兜底扫描。
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
fn resolve_save_games_root() -> Result<(PathBuf, bool), String> {
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

    // 3. 兜底：常见 Steam 库
    for root in STEAM_LIBRARY_ROOTS {
        let cand = Path::new(root)
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

    let entries =
        std::fs::read_dir(src).map_err(|e| format!("读取源目录失败: {}", e))?;
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
    let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
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
/// 但部分版本/外部导出可能是扁平的 `SaveGames/<World>/Level.sav`。
/// 两种布局都兼容：优先直接层，其次首个含 Level.sav 的子目录（GUID 层）。
fn find_world_data_dir(world_dir: &Path) -> Option<PathBuf> {
    // 1) 扁平结构：world_dir/Level.sav
    if world_dir.join("Level.sav").is_file() {
        return Some(world_dir.to_path_buf());
    }
    // 2) GUID 嵌套结构：world_dir/<GUID>/Level.sav（取第一个匹配的子目录）
    if let Ok(entries) = std::fs::read_dir(world_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("Level.sav").is_file() {
                return Some(p);
            }
        }
    }
    None
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
    WorldInfo {
        name,
        path: world_dir.to_string_lossy().to_string(),
        has_level_sav,
        player_count,
        size_bytes,
    }
}

// ==================== Tauri 命令 ====================

/// F4-P0：扫描 SaveGames 下含 Level.sav 的目录作为世界列表。
/// 返回实际发现的存档根，便于前端确认路径是否正确。
#[command]
pub async fn discover_worlds() -> Result<DiscoverResult, String> {
    let (save_root, auto_discovered) = resolve_save_games_root()?;

    let mut worlds: Vec<WorldInfo> = Vec::new();
    let entries = std::fs::read_dir(&save_root)
        .map_err(|e| format!("读取存档目录失败: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let info = world_info_from_dir(&path);
            // 仅列出含 Level.sav 的世界（健壮：忽略 _backups 等杂目录）
            if info.has_level_sav {
                worlds.push(info);
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

/// F4-P0：整目录备份世界到备份夹。
/// 默认目标：<SaveGames>/_backups/<world>/<timestamp>/，非破坏（新建时间戳目录）。
#[command]
pub async fn backup_world(world_name: String, dest: Option<String>) -> Result<String, String> {
    let (save_root, _auto) = resolve_save_games_root()?;
    let world = safe_name_segment(&world_name)
        .ok_or_else(|| "世界名非法（仅允许字母/数字/下划线/点/连字符）".to_string())?;

    let world_dir = save_root.join(&world);
    if !world_dir.is_dir() {
        return Err(format!("世界目录不存在: {}", world_dir.display()));
    }

    // 确定目标目录
    let backup_dir = match dest {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d.trim()),
        _ => {
            // 默认：<SaveGames>/_backups/<world>/<timestamp>/
            let ts = format_timestamp(SystemTime::now());
            save_root
                .join("_backups")
                .join(&world)
                .join(ts)
        }
    };
    // 校验目标落点在 SaveGames 内（防止 dest 被篡改为任意路径）
    let backup_dir = normalize_within(&save_root, &backup_dir)?;

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
pub async fn list_world_backups(world_name: String) -> Result<Vec<WorldBackupInfo>, String> {
    let (save_root, _auto) = resolve_save_games_root()?;
    let world = safe_name_segment(&world_name)
        .ok_or_else(|| "世界名非法".to_string())?;

    let backups_root = save_root.join("_backups").join(&world);
    if !backups_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut result: Vec<WorldBackupInfo> = Vec::new();
    let entries = std::fs::read_dir(&backups_root)
        .map_err(|e| format!("读取备份目录失败: {}", e))?;
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
pub async fn restore_world(world_name: String, backup_id: String) -> Result<String, String> {
    let (save_root, _auto) = resolve_save_games_root()?;
    let world = safe_name_segment(&world_name)
        .ok_or_else(|| "世界名非法".to_string())?;
    let bid = safe_name_segment(&backup_id)
        .ok_or_else(|| "备份 id 非法".to_string())?;

    let backup_dir = save_root.join("_backups").join(&world).join(&bid);
    if !backup_dir.is_dir() {
        return Err(format!("备份不存在: {}", backup_dir.display()));
    }
    let world_dir = save_root.join(&world);
    if !world_dir.is_dir() {
        return Err(format!("目标世界目录不存在: {}", world_dir.display()));
    }

    // 覆盖拷贝（先清空再拷，保证备份完全还原）
    std::fs::remove_dir_all(&world_dir)
        .map_err(|e| format!("清理当前世界失败: {}", e))?;
    copy_dir_recursive(&backup_dir, &world_dir)?;

    Ok(format!(
        "已用备份「{}」恢复世界「{}」（含 Level.sav / Players/ 等）",
        bid, world
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
    let world = safe_name_segment(&world_name)
        .ok_or_else(|| "世界名非法".to_string())?;
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
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目标目录失败: {}", e))?;
        }
    }
    std::fs::copy(&src, &dest)
        .map_err(|e| format!("导出角色失败: {}", e))?;

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
    let world = safe_name_segment(&world_name)
        .ok_or_else(|| "世界名非法".to_string())?;
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
    std::fs::create_dir_all(&players_dir)
        .map_err(|e| format!("创建 Players 目录失败: {}", e))?;
    let dst = players_dir.join(format!("{}.sav", sid));
    std::fs::copy(&src, &dst)
        .map_err(|e| format!("导入角色失败: {}", e))?;

    Ok(format!(
        "已导入角色 {} → {}（SteamID 保持不变；公会归属未迁移，属预期）",
        sid,
        dst.display()
    ))
}

// ==================== 安全归并工具 ====================

/// 校验 `candidate` 落在 `root` 之内（防 dest 路径穿越）。
/// 若 candidate 已是 root 子路径则直接用；否则回退到 root 下的归并路径（仅取其文件名层）。
fn normalize_within(root: &Path, candidate: &Path) -> Result<PathBuf, String> {
    // 已落在 root 内 → 直接采用
    if candidate.starts_with(root) && candidate != root {
        return Ok(candidate.to_path_buf());
    }
    // 否则取 candidate 的末级文件名，归并回 root，避免任意写出
    let leaf = candidate
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "备份目标路径非法".to_string())?
        .to_string();
    if safe_name_segment(&leaf).is_none() {
        return Err("备份目标路径含非法字符".to_string());
    }
    Ok(root.join(leaf))
}

// ==================== 单元测试（GUID 嵌套结构冒烟校验） ====================

#[cfg(test)]
mod tests {
    use super::*;

    /// 针对本机真实 Palworld 专用服存档结构（SaveGames/<World>/<GUID>/Level.sav）做一次冒烟校验。
    /// 仅当该路径存在时执行，避免在无此路径的 CI 环境中失败。
    #[test]
    fn discover_real_palserver_world() {
        let root = Path::new(
            "E:\\SteamLibrary\\steamapps\\common\\PalServer\\Pal\\Saved\\SaveGames",
        );
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
}
