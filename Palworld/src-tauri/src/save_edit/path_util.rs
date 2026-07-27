//! F5 路径安全与世界定位工具。
//!
//! 本模块**复制** F4 (`save_transfer.rs`) 的 `safe_name_segment` / `normalize_within`
//! 白名单算法与 `resolve_save_games_root` / `find_world_data_dir` 逻辑，作为 F5 独立副本。
//! 依据架构文档 §6.1 / §7：**不修改 F4 任何文件**，仅在 F5 内复用等效实现。
//!
//! 所有 world / player / guid 输入在用于拼路径前必须经 `safe_name_segment` 校验，
//! 杜绝路径穿越（R2 / C1 安全底座）。

use std::path::{Path, PathBuf};

use crate::settings;

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
///   2. server_path 指向更深/更浅一层时，从 server_path 逐级向上扫描。
///   3. 兜底：扫描常见 Steam 库根下的 `steamapps/common/Palworld/Pal/Saved/SaveGames`。
///
/// 返回 (save_root, auto_discovered)。
pub fn resolve_save_games_root() -> Result<(PathBuf, bool), String> {
    let settings = settings::load_settings()?;
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

        // 2. 向上扫描
        let mut cur = Path::new(&server_path);
        loop {
            let cand = cur.join("Pal").join("Saved").join("SaveGames");
            if cand.is_dir() {
                return Ok((cand, true));
            }
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

/// 校验输入仅含安全文件名字符（字母/数字/下划线/点/连字符），且不含路径分隔符。
/// 防止 world_name / guid / backup_id 被用于路径穿越。
pub fn safe_name_segment(s: &str) -> Option<String> {
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

/// 规范化 player guid：去掉末尾的 ".sav"（若有），返回纯文件名基底。
pub fn normalize_player_guid(guid: &str) -> Option<String> {
    let trimmed = guid.trim();
    // GUID 是文件名而非路径；即使 safe_name_segment 会保留末段，也不能接受路径穿越输入。
    if trimmed.contains(['/', '\\']) {
        return None;
    }
    let base = if let Some(stripped) = trimmed.strip_suffix(".sav") {
        stripped
    } else {
        trimmed
    };
    safe_name_segment(base)
}

/// 校验 `candidate` 落在 `root` 之内（防 dest 路径穿越）。
/// 若 candidate 已是 root 子路径则直接用；否则回退到 root 下的归并路径（仅取其文件名层）。
#[allow(dead_code)]
pub fn normalize_within(root: &Path, candidate: &Path) -> Result<PathBuf, String> {
    if candidate.starts_with(root) && candidate != root {
        return Ok(candidate.to_path_buf());
    }
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

/// 定位世界数据目录（含 Level.sav 的那一层）。
///
/// Palworld 实际磁盘结构为 `SaveGames/<World>/<GUID>/Level.sav`（GUID 子层），
/// 但部分版本/外部导出可能是扁平的 `SaveGames/<World>/Level.sav`，
/// 也可能在更深的嵌套下（如本机单机导出的
/// `<SteamID>/<timestamp>/<GUID>/Level.sav`，世界根下还散落 `steam_autocloud.vdf` 等无关文件）。
///
/// 采用**有界 DFS（最大深度 ≤ 4）**：从 `world_dir` 出发逐层下探，
/// 命中首个含 `Level.sav` 的目录即返回。越界、无 Level.sav 或读目录失败均返回 `None`，
/// **绝不 panic**（防御式，供整包迁移的 local 源穿透定位）。
pub fn find_world_data_dir(world_dir: &Path) -> Option<PathBuf> {
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

/// 解析世界目录绝对路径（world 名经白名单校验，save_root 可注入便于测试）。
pub fn world_dir_with_root(world: &str, save_root: &Path) -> Result<PathBuf, String> {
    let w = safe_name_segment(world)
        .ok_or_else(|| "世界名非法（仅允许字母/数字/下划线/点/连字符）".to_string())?;
    let dir = save_root.join(&w);
    if !dir.is_dir() {
        return Err(format!("世界目录不存在: {}", dir.display()));
    }
    Ok(dir)
}

/// 解析世界数据层目录（含 Level.sav，save_root 可注入便于测试）。
pub fn world_data_dir_with_root(world: &str, save_root: &Path) -> Result<PathBuf, String> {
    let dir = world_dir_with_root(world, save_root)?;
    find_world_data_dir(&dir).ok_or_else(|| format!("未找到世界数据(Level.sav)：{}", dir.display()))
}

/// 解析世界目录绝对路径（world 名经白名单校验）。
pub fn world_dir(world: &str) -> Result<PathBuf, String> {
    let (save_root, _auto) = resolve_save_games_root()?;
    world_dir_with_root(world, &save_root)
}

/// 解析世界数据层目录（含 Level.sav）。
pub fn world_data_dir(world: &str) -> Result<PathBuf, String> {
    let (save_root, _auto) = resolve_save_games_root()?;
    world_data_dir_with_root(world, &save_root)
}

/// 解析玩家角色存档绝对路径（Players/<guid>.sav），兼容扁平/GUID 嵌套。
#[allow(dead_code)] // Retained for the legacy single-player command contract.
pub fn player_sav_path(world: &str, guid: &str) -> Result<PathBuf, String> {
    let g = normalize_player_guid(guid)
        .ok_or_else(|| "角色 GUID 非法（仅允许字母/数字/下划线/点/连字符）".to_string())?;
    let data_dir = world_data_dir(world)?;
    let path = data_dir.join("Players").join(format!("{}.sav", g));
    if !path.is_file() {
        return Err(format!("角色存档不存在: {}", path.display()));
    }
    Ok(path)
}

/// 解析玩家角色存档所在目录（Players/）。
pub fn players_dir(world: &str) -> Result<PathBuf, String> {
    let data_dir = world_data_dir(world)?;
    Ok(data_dir.join("Players"))
}

/// 递归拷贝目录（F5 独立副本，不依赖 F4）。
/// `counter` 累计已拷贝文件数（用于迁移/备份统计）。
pub fn copy_dir_recursive(src: &Path, dst: &Path, counter: &mut usize) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("源目录不存在: {}", src.display()));
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("创建 {} 失败: {}", dst.display(), e))?;
    let entries =
        std::fs::read_dir(src).map_err(|e| format!("读取 {} 失败: {}", src.display(), e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path, counter)?;
        } else if path.is_file() {
            std::fs::copy(&path, &dst_path)
                .map_err(|e| format!("拷贝 {} 失败: {}", path.display(), e))?;
            *counter += 1;
        }
    }
    Ok(())
}

/// 递归删除目录（用于备份回滚前清理目标）。
pub fn remove_dir_recursive(path: &Path) -> Result<(), String> {
    if path.exists() {
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("删除 {} 失败: {}", path.display(), e))?;
    }
    Ok(())
}

// ===========================================================================
// F5 单元测试（QA · 严过关）
// 覆盖：safe_name_segment 白名单/剥离父目录、normalize_player_guid、
// normalize_within 防路径穿越、copy/remove 递归、整包备份/回滚一致性模拟。
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn tmp_dir(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("f5_path_{}_{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn safe_name_segment_whitelist_and_strip() {
        // 白名单放行
        assert_eq!(safe_name_segment("World_01"), Some("World_01".to_string()));
        assert_eq!(safe_name_segment("a.b-c"), Some("a.b-c".to_string()));
        // 剥离父目录成分（取 file_name）
        assert_eq!(safe_name_segment("foo/bar"), Some("bar".to_string()));
        assert_eq!(safe_name_segment("a/b/c.sav"), Some("c.sav".to_string()));
        assert_eq!(safe_name_segment("../etc"), Some("etc".to_string()));
        assert_eq!(safe_name_segment("a/../../b"), Some("b".to_string()));
        // 拒绝：空 / 非法字符（非 字母数字 _ - .）
        assert_eq!(safe_name_segment(""), None);
        assert_eq!(safe_name_segment("a*b"), None);
        assert_eq!(safe_name_segment("a b"), None);
    }

    #[test]
    fn normalize_player_guid_strips_sav() {
        assert_eq!(normalize_player_guid("ABC.sav"), Some("ABC".to_string()));
        assert_eq!(normalize_player_guid("ABC"), Some("ABC".to_string()));
        assert_eq!(normalize_player_guid("../../x"), None);
    }

    #[test]
    fn normalize_within_prevents_traversal() {
        let root = Path::new("/save/World");
        // 已在 root 内：原样通过
        assert!(normalize_within(root, Path::new("/save/World/Players/x")).is_ok());
        // 穿越尝试：回退为 root 内的叶名，绝不落到 root 之外
        let r = normalize_within(root, Path::new("/etc/passwd"));
        let p = r.expect("normalize_within 必须返回受约束路径");
        assert!(
            p.starts_with(root),
            "normalize_within 必须把目标约束在 root 内: {p:?}"
        );
        assert_eq!(p.file_name().unwrap(), std::ffi::OsStr::new("passwd"));
    }

    #[test]
    fn copy_and_remove_recursive() {
        let src = tmp_dir("copy_src");
        std::fs::write(src.join("a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("sub").join("b.txt"), b"world").unwrap();

        let dst = tmp_dir("copy_dst");
        let mut n = 0;
        copy_dir_recursive(&src, &dst, &mut n).unwrap();
        assert_eq!(n, 2, "应拷贝 2 个文件");
        assert!(dst.join("a.txt").is_file());
        assert!(dst.join("sub").join("b.txt").is_file());

        remove_dir_recursive(&dst).unwrap();
        assert!(!dst.exists());
    }

    /// 模拟 world_copy 的「整包备份 → 损坏 → 回滚」一致性（使用同一组基础原语）。
    #[test]
    fn backup_rollback_simulation() {
        let live = tmp_dir("live");
        std::fs::write(live.join("Level.sav"), b"data1").unwrap();
        let players = live.join("Players");
        std::fs::create_dir_all(&players).unwrap();
        std::fs::write(players.join("0001.sav"), b"player1").unwrap();

        // 1) 改写前整包备份（快照）
        let snap = tmp_dir("snap");
        let mut n = 0;
        copy_dir_recursive(&live, &snap, &mut n).unwrap();

        // 2) 模拟迁移中损坏
        std::fs::write(live.join("Level.sav"), b"CORRUPT").unwrap();
        std::fs::remove_file(players.join("0001.sav")).unwrap();

        // 3) 回滚：清除目标 + 从快照恢复
        remove_dir_recursive(&live).unwrap();
        let mut m = 0;
        copy_dir_recursive(&snap, &live, &mut m).unwrap();

        // 4) 恢复一致
        assert_eq!(
            std::fs::read(live.join("Level.sav")).unwrap(),
            b"data1",
            "回滚后 Level.sav 应与原备份一致"
        );
        assert_eq!(
            std::fs::read(players.join("0001.sav")).unwrap(),
            b"player1",
            "回滚后玩家存档应与原备份一致"
        );
    }

    // ===================== T04：整包本地迁移（穿透 + 字节保真 + 防御） =====================

    /// 构造「本机单机导出」样本：世界根下散落 steam_autocloud.vdf（无关文件，
    /// 位于数据目录之外），数据层嵌套 2 层：<timestamp>/<GUID>/Level.sav。
    /// 返回 (local_root, data_dir)。
    fn make_local_sample(root: &Path) -> (PathBuf, PathBuf) {
        let local_root = root.join("76561199381352956");
        let ts = local_root.join("20260724-183232");
        let guid = ts.join("A1B2C3D4-0000-1111-2222-333344445555");
        std::fs::create_dir_all(&guid).unwrap();
        // 数据目录之外的无关文件（应被自然排除）
        std::fs::write(local_root.join("steam_autocloud.vdf"), b"ignore-me").unwrap();
        // 数据层内容
        std::fs::write(guid.join("Level.sav"), b"LEVEL-BYTES-1234567890").unwrap();
        std::fs::write(guid.join("WorldOption.sav"), b"option").unwrap();
        let players = guid.join("Players");
        std::fs::create_dir_all(&players).unwrap();
        std::fs::write(players.join("76561190000000001.sav"), b"player-data-xyz").unwrap();
        (local_root, guid)
    }

    /// T04-1：有界 DFS 穿透 2 层嵌套，定位到含 Level.sav 的 {GUID} 数据层（真实样本结构）。
    #[test]
    fn find_world_data_dir_penetrates_two_levels() {
        let root = tmp_dir("t04_penetrate");
        let (local_root, data_dir) = make_local_sample(&root);
        let found = find_world_data_dir(&local_root);
        assert!(found.is_some(), "应能穿透 2 层定位到含 Level.sav 的数据层");
        let d = found.unwrap();
        assert_eq!(d, data_dir, "定位结果应等于 GUID 数据层");
        assert!(d.join("Level.sav").is_file());
    }

    /// T04-2：整包迁移字节保真（local 源）+ steam_autocloud.vdf 自然排除。
    /// 镜像 migrate_world_impl 的「源解析(find_world_data_dir) + 深度拷贝」序列，
    /// 目标指向临时专用服布局（等价 world_data_dir 对 "0" 世界的解析结果），
    /// 避免触碰真实存档根（resolve_save_games_root 读 settings）。
    #[test]
    fn migrate_local_world_byte_fidelity() {
        let root = tmp_dir("t04_fidelity");
        let (local_root, data_dir) = make_local_sample(&root);

        // 源：与 migrate_world_impl source_type=="local" 分支完全一致
        let src = find_world_data_dir(Path::new(&local_root))
            .ok_or_else(|| "未找到本地世界数据(Level.sav)".to_string())
            .unwrap();
        assert_eq!(src, data_dir);

        // 目标：模拟专用服 0/<GUID> 数据层（world_data_dir 的解析结果，不依赖真实根）
        let tgt = root.join("0").join("A1B2C3D4-0000-1111-2222-333344445555");
        let mut copied = 0usize;
        copy_dir_recursive(&src, &tgt, &mut copied).unwrap();

        // 断言逐字节一致
        assert_eq!(
            std::fs::read(tgt.join("Level.sav")).unwrap(),
            std::fs::read(data_dir.join("Level.sav")).unwrap(),
            "Level.sav 应逐字节一致"
        );
        assert_eq!(
            std::fs::read(tgt.join("Players").join("76561190000000001.sav")).unwrap(),
            std::fs::read(data_dir.join("Players").join("76561190000000001.sav")).unwrap(),
            "Players/*.sav 应逐字节一致"
        );
        assert_eq!(
            copied, 3,
            "应拷贝 Level.sav + WorldOption.sav + Players/*.sav 共 3 个文件"
        );

        // 断言数据目录之外的 steam_autocloud.vdf 未被拷入目标（任何层级都不应出现）
        assert!(
            !tgt.join("steam_autocloud.vdf").exists(),
            "目标不应包含数据目录之外的 steam_autocloud.vdf"
        );
        let mut stack = vec![tgt.clone()];
        let mut found_vdf = false;
        while let Some(d) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(&d) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p
                        .file_name()
                        .map(|n| n == "steam_autocloud.vdf")
                        .unwrap_or(false)
                    {
                        found_vdf = true;
                        break;
                    }
                }
            }
            if found_vdf {
                break;
            }
        }
        assert!(!found_vdf, "目标目录树内不得出现 steam_autocloud.vdf");
    }

    /// T04-3：防御式——源缺 Level.sav / 路径非法 / 超深结构时返回 None，绝不 unwrap-panic。
    #[test]
    fn find_world_data_dir_defensive_no_panic() {
        // 空目录（无 Level.sav）
        let empty = tmp_dir("t04_empty");
        assert_eq!(
            find_world_data_dir(&empty),
            None,
            "无 Level.sav 应返回 None"
        );

        // 不存在的路径（read_dir 失败）
        let missing = tmp_dir("t04_missing_x").join("does_not_exist");
        assert_eq!(
            find_world_data_dir(&missing),
            None,
            "路径非法/不存在应返回 None，不 panic"
        );

        // 越界保护：超过 MAX_DEPTH(4) 的深层结构应安全返回 None（不栈溢出 / 不 panic）
        let deep = tmp_dir("t04_deep");
        let mut cur = deep.clone();
        for i in 0..8 {
            cur = cur.join(format!("L{}", i));
            std::fs::create_dir_all(&cur).unwrap();
        }
        assert_eq!(
            find_world_data_dir(&deep),
            None,
            "超出最大深度应返回 None，不 panic/不栈溢出"
        );
    }
}
