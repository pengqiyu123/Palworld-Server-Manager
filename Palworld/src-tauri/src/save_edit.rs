//! F5 · 存档迁移 / 改写主模块。
//!
//! 本模块是 F5 全部能力的入口，统一编排三件安全底座：
//! 1. **服务器停止断言**：任何改写操作前，若专用服进程仍在运行则直接拒绝（避免存档损坏）。
//! 2. **先备份后改写**：改写前对世界做快照（F5 独立 `backups/` 目录），失败自动回滚。
//! 3. **R11（已修订）**：Oodle(PLM / Kraken, save_type=49) 现已支持——解码经 vendored
//!    `oozextract`(MIT, 纯 Rust) 实现，与参考 `PalworldSaveTools` 的 `palooz` 同宗同构；
//!    写回统一降级为 PLZ(zlib)（社区实证零数据损失）。**铁律保留**：任何**无法解码**
//!    的格式（未知 magic / 解码失败 / 长度不符）仍须明确报错、绝不静默损坏。
//!
//! 不修改 F4（`save_transfer.rs` 等）任何代码；路径/备份等安全逻辑在 F5 内独立副本实现。

pub mod fix_host;
pub mod models;
pub mod oodle;
pub mod path_util;
pub mod sav_io;
pub mod tech_edit;
pub mod transfer;
pub mod world_copy;

use std::path::{Path, PathBuf};

use tauri::State;

use crate::save_edit::models::*;
use crate::server::ServerState;

/// F5 备份根目录：`SaveGames` 同级 `backups/<world>/<backup_id>`。
fn backups_root() -> Result<PathBuf, String> {
    let (save_root, _auto) = path_util::resolve_save_games_root()?;
    Ok(save_root
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| save_root.join("backups")))
}

/// 生成唯一备份 id（纳秒时间戳 + 进程 id）。
fn now_backup_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("f5_{}_{}", nanos, std::process::id())
}

/// 备份世界数据目录，返回 backup_id。
fn backup_world_dir(world: &str) -> Result<String, String> {
    let data_dir = path_util::world_data_dir(world)?;
    let root = backups_root()?;
    let backup_id = now_backup_id();
    let dest = root.join(world).join(&backup_id);
    std::fs::create_dir_all(&dest)
        .map_err(|e| format!("创建备份目录 {} 失败: {}", dest.display(), e))?;
    let mut n = 0usize;
    path_util::copy_dir_recursive(&data_dir, &dest, &mut n)?;
    Ok(backup_id)
}

/// 从备份回滚世界数据目录。
fn restore_world_dir(world: &str, backup_id: &str) -> Result<(), String> {
    let data_dir = path_util::world_data_dir(world)?;
    let root = backups_root()?;
    let src = root.join(world).join(backup_id);
    if !src.is_dir() {
        return Err(format!("备份不存在: {}", src.display()));
    }
    path_util::remove_dir_recursive(&data_dir)?;
    let mut n = 0usize;
    path_util::copy_dir_recursive(&src, &data_dir, &mut n)?;
    Ok(())
}

/// 服务器停止断言：运行时拒绝改写。
fn ensure_server_stopped(state: &ServerState) -> Result<(), String> {
    if crate::server::is_server_process_running(&*state) {
        return Err(
            "服务器正在运行，请先停止服务器再执行存档改写操作（避免存档损坏）".to_string(),
        );
    }
    Ok(())
}

/// T4 安全闸门：调用方必须显式声明目标世界对应的服务器是否已停止。
///
/// 运行中替换会被自动存档覆盖（R3），导致修复结果丢失或部分损坏。
///
/// 本枚举作为**显式契约**：选 [`StopServerAssertion::Undeclared`] 时
/// [`run_fix_host_with_guard`] 直接拒绝执行（不触碰磁盘）；选
/// [`StopServerAssertion::DeclaredStopped`] 表示调用方已（通过实机停服或
/// [`ensure_server_stopped`] 的运行态检测）确认服务器停止。
///
/// ⚠️ 风险自担：类型契约**不二次探测运行态**。若声明不实（服务器实际仍在运行），
/// 运行中自动存档可能覆盖本操作的写入，导致修复结果丢失或部分损坏。因此实机
/// Tauri 命令在调用本包装前仍应走 [`ensure_server_stopped`] 的真实进程检测作为
/// 第一道闸门；本契约是防御性「调用方必须显式声明」的第二道闸门。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopServerAssertion {
    /// 调用方声明：已确认目标服务器停止（推荐；实机操作前务必停服）。
    DeclaredStopped,
    /// 调用方未声明停服：拒绝执行，避免运行中自动存档覆盖导致损坏。
    Undeclared,
}

/// T4 编排包装：在 `data_dir` 上执行 fix_host 双向交换，带
/// **停服守卫** + **自动整包备份** + **失败自动回滚** 三重安全闸门。
///
/// 流程（严格顺序）：
/// 1. **(a) 停服守卫**：`assertion == Undeclared` 直接拒绝（返回明确错误，**不触碰磁盘**）。
/// 2. **(b) 自动备份**：把 `data_dir` 整包快照到 `backup_root` 下带时间戳子目录
///    （复用 `path_util::copy_dir_recursive`，与现有 `backup_world_dir` 同源），
///    拿到备份路径 `backup_path`。
/// 3. **(c) 执行修复**：调用 `fix_host::fix_host_save_in_dir`（T3 双向交换 + 回读校验）。
/// 4. **(d) 失败回滚**：若 c 步抛错，用 b 步备份把 `data_dir` 还原到修改前
///    （`remove_dir_recursive` + `copy_dir_recursive`），返回带「已回滚」语义的错误。
///    成功则**保留备份**（供审计 / 手动还原），并在返回的 `PathBuf` 中告知备份路径。
///
/// 返回 `(changed_file_count, backup_path)`；失败返回 `Err`（含备份路径与「已回滚」说明）。
///
/// 注意：本包装**不做**运行态进程检测——无法把任意 `data_dir` 可靠映射到某个运行实例。
/// 运行态检测由上层 Tauri 命令（`fix_host_save` → `ensure_server_stopped`）负责；本包装
/// 仅以显式 [`StopServerAssertion`] 契约强制「调用方须声明已停服」。因此本包装可脱离
/// settings / 运行态，直接在任意临时 `data_dir` 上被单元测试驱动（见模块内 `tests`）。
pub fn run_fix_host_with_guard(
    data_dir: &Path,
    old_host_guid: &str,
    new_char_guid: &str,
    assertion: StopServerAssertion,
    backup_root: &Path,
) -> Result<(usize, PathBuf), String> {
    // (a) 停服守卫：未声明则拒绝，且不触碰磁盘（不生成备份）。
    if assertion == StopServerAssertion::Undeclared {
        return Err(
            "请先停止该世界对应的服务器，再执行 Fix Host（运行中替换会被自动存档覆盖，\
             导致修复结果丢失或部分损坏）"
                .to_string(),
        );
    }

    // 解析两个 GUID 字节（与 fix_host_save_impl 一致）。
    let old_bytes = sav_io::guid_bytes(old_host_guid)?;
    let new_bytes = sav_io::guid_bytes(new_char_guid)?;

    // (b) 自动备份：整包快照到 backup_root 下带时间戳目录（复用 copy_dir_recursive）。
    let dir_name = data_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "world".to_string());
    let backup_path = backup_root.join(format!("{}.f5_bak_{}", dir_name, now_backup_id()));
    std::fs::create_dir_all(backup_root)
        .map_err(|e| format!("创建备份根 {} 失败: {}", backup_root.display(), e))?;
    let mut n = 0usize;
    path_util::copy_dir_recursive(data_dir, &backup_path, &mut n)
        .map_err(|e| format!("备份世界 {} 失败: {}", data_dir.display(), e))?;

    // (c) 执行修复（T3 双向交换 + 回读校验）。
    match fix_host::fix_host_save_in_dir(data_dir, &old_bytes, &new_bytes) {
        Ok(changed) => Ok((changed, backup_path)),
        Err(e) => {
            // (d) 失败回滚：清除 data_dir 并从备份恢复，返回「已回滚」语义错误。
            let _ = path_util::remove_dir_recursive(data_dir);
            let mut m = 0usize;
            let _ = path_util::copy_dir_recursive(&backup_path, data_dir, &mut m);
            // 备份保留（审计 / 手动还原），不删除。
            Err(format!(
                "Fix Host 执行失败，已回滚至备份 {}: {}",
                backup_path.display(),
                e
            ))
        }
    }
}

// ===========================================================================
// Tauri 命令（与前端 api/tauri.ts 的 invoke 名称严格对应）
// ===========================================================================

/// L1/L2：解析世界玩家与公会摘要（按世界名，服务器 SaveGames 根下）。
#[tauri::command]
pub async fn f5_world_summary(world_name: String) -> Result<WorldSummary, String> {
    world_copy::f5_world_summary_impl(&world_name)
}

/// L1/L2：按真实世界目录路径解析玩家与公会摘要（本地单机 / 服务器通用）。
#[tauri::command]
pub async fn f5_world_summary_by_path(path: String) -> Result<WorldSummary, String> {
    world_copy::f5_world_summary_by_path_impl(&path)
}

/// 科技列表（vendored world_data.json）。
#[tauri::command]
pub async fn f5_tech_list() -> Result<Vec<TechInfo>, String> {
    tech_edit::f5_tech_list_impl()
}

/// U01 Fix Host Save：旧主机角色 ↔ 新角色 UID 互换。
///
/// 第一道闸门：运行态真实检测（`ensure_server_stopped`，复用 server 模块能力）。
/// 第二道闸门：显式停服契约（`StopServerAssertion::DeclaredStopped`）。
/// 两者之下，`run_fix_host_with_guard` 再提供自动备份 + 失败回滚。
#[tauri::command]
pub async fn fix_host_save(
    req: FixHostRequest,
    state: State<'_, ServerState>,
) -> Result<EditResult, String> {
    // 第一道闸门：运行中拒绝（真实进程检测）。
    ensure_server_stopped(&*state)?;
    let data_dir = path_util::world_data_dir(&req.world)?;
    let seg = path_util::safe_name_segment(&req.world)
        .ok_or_else(|| "世界名非法（仅允许字母/数字/下划线/点/连字符）".to_string())?;
    let backup_root = backups_root()?.join(&seg);
    // 第二道闸门（显式声明已停服）+ 自动备份 + 失败回滚，统一在编排包装内完成。
    match run_fix_host_with_guard(
        &data_dir,
        &req.old_host_guid,
        &req.new_char_guid,
        StopServerAssertion::DeclaredStopped,
        &backup_root,
    ) {
        Ok((changed, backup_path)) => {
            let backup_id = backup_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(EditResult {
                ok: true,
                backup_id: backup_id.clone(),
                roundtrip_ok: true,
                warnings: vec![format!(
                    "已改写 {} 个 .sav 文件（已自动备份 {}）",
                    changed, backup_id
                )],
            })
        }
        Err(e) => Err(e),
    }
}

/// T03 整包世界迁移。
#[tauri::command]
pub async fn migrate_world_to_server(
    req: MigrateRequest,
    state: State<'_, ServerState>,
) -> Result<EditResult, String> {
    ensure_server_stopped(&*state)?;
    let mut backup_id = String::new();
    if path_util::world_dir(&req.target_world)
        .map(|p| p.exists())
        .unwrap_or(false)
    {
        backup_id = backup_world_dir(&req.target_world)?;
    }
    match world_copy::migrate_world_impl(&req) {
        Ok(copied) => Ok(EditResult {
            ok: true,
            backup_id,
            roundtrip_ok: true,
            warnings: vec![format!(
                "已拷贝 {} 个文件（{} → {}）",
                copied, req.source_world, req.target_world
            )],
        }),
        Err(e) => {
            if !backup_id.is_empty() {
                let _ = restore_world_dir(&req.target_world, &backup_id);
            }
            Err(format!("世界迁移失败: {}", e))
        }
    }
}

/// T04 跨服角色转移。
#[tauri::command]
pub async fn transfer_character(
    req: TransferRequest,
    state: State<'_, ServerState>,
) -> Result<EditResult, String> {
    ensure_server_stopped(&*state)?;
    let backup_id = backup_world_dir(&req.target_world)?;
    match transfer::transfer_character_impl(&req) {
        Ok(mut r) => {
            r.backup_id = backup_id;
            Ok(r)
        }
        Err(e) => {
            let _ = restore_world_dir(&req.target_world, &backup_id);
            Err(format!("角色转移失败，已回滚备份 {}: {}", backup_id, e))
        }
    }
}

/// T05 科技点编辑（解锁 / 移除）。
#[tauri::command]
pub async fn edit_tech(
    req: TechEditRequest,
    state: State<'_, ServerState>,
) -> Result<EditResult, String> {
    ensure_server_stopped(&*state)?;
    let backup_id = backup_world_dir(&req.world)?;
    match tech_edit::edit_tech_impl(&req) {
        Ok(mut r) => {
            r.backup_id = backup_id;
            Ok(r)
        }
        Err(e) => {
            let _ = restore_world_dir(&req.world, &backup_id);
            Err(format!("科技点编辑失败，已回滚备份 {}: {}", backup_id, e))
        }
    }
}

/// T05 玩家基础属性编辑（改名 / 等级 / Max All）。
#[tauri::command]
pub async fn edit_player_attr(
    req: PlayerAttrRequest,
    state: State<'_, ServerState>,
) -> Result<EditResult, String> {
    ensure_server_stopped(&*state)?;
    let backup_id = backup_world_dir(&req.world)?;
    match tech_edit::edit_player_attr_impl(&req) {
        Ok(mut r) => {
            r.backup_id = backup_id;
            Ok(r)
        }
        Err(e) => {
            let _ = restore_world_dir(&req.world, &backup_id);
            Err(format!("玩家属性编辑失败，已回滚备份 {}: {}", backup_id, e))
        }
    }
}

// ===========================================================================
// T4 单元测试（QA · 严过关）
// 覆盖「停服守卫 + 自动备份 + 失败回滚」编排闸门在**临时样本副本**上的行为
// （不改动原始样本、不依赖 settings / 运行态，可独立 `cargo test` 驱动）：
//   1. guard_undeclared_rejected_and_no_backup：未声明停服 → 拒绝且不触碰磁盘（无备份）。
//   2. guard_declared_creates_backup_with_world_files：声明后执行且自动备份含世界文件。
//   3. midwrite_failure_rolls_back_to_prechange：写入中途失败（文件只读）→ 自动回滚到修改前。
//   4. guard_declared_normal_swap_ok：正常路径 fix_host 仍成功，交换结果正确（复用 T3 断言）。
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    /// 老板真实样本目录（Windows 绝对路径，Rust std 接受正斜杠）。该目录直接含
    /// `Level.sav` 与 `Players/`，即为 `fix_host_save_in_dir` 所需的 `data_dir`。
    const SAMPLE_DIR: &str = "F:/1/0/20260723-235259/1A91A61548C7B6FD7B58B2B70710F7EE";
    /// 样本内两个真实玩家 GUID（磁盘序 32 位小写十六进制）。
    const OLD_GUID: &str = "3F5D130B000000000000000000000000";
    const NEW_GUID: &str = "4E239D4F000000000000000000000000";

    fn sample_dir() -> Option<PathBuf> {
        let p = PathBuf::from(SAMPLE_DIR);
        if p.is_dir() {
            Some(p)
        } else {
            eprintln!("[skip] 真实样本目录不存在，跳过 T4 测试: {}", SAMPLE_DIR);
            None
        }
    }

    /// 把真实样本整包拷到临时随机子目录（避免改动原始样本）。
    fn copy_sample_to_temp(tag: &str) -> Option<PathBuf> {
        let src = sample_dir()?;
        let dst = std::env::temp_dir().join(format!(
            "palworld_t4_{}_{}_{}",
            tag,
            std::process::id(),
            now_backup_id()
        ));
        let _ = std::fs::remove_dir_all(&dst);
        let mut n = 0usize;
        crate::save_edit::path_util::copy_dir_recursive(&src, &dst, &mut n).ok()?;
        Some(dst)
    }

    /// 临时备份根目录（与生产 `backups/` 隔离，便于断言与清理）。
    fn tmp_backup_root(tag: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "palworld_t4_bak_{}_{}_{}",
            tag,
            std::process::id(),
            now_backup_id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// 单向 16 字节 GUID 替换（本地对拍，逻辑同 `SavFile::replace_guid_bytes`）。
    fn replace_guid_bytes_local(raw: &mut Vec<u8>, old: &[u8; 16], new: &[u8; 16]) {
        if old == new {
            return;
        }
        let mut i = 0;
        let len = raw.len();
        while i + 16 <= len {
            if raw[i..i + 16] == *old {
                raw[i..i + 16].copy_from_slice(new);
                i += 16;
            } else {
                i += 1;
            }
        }
    }

    fn load_raw(p: &Path) -> Vec<u8> {
        crate::save_edit::sav_io::SavFile::load(p)
            .unwrap_or_else(|e| panic!("{} 应可解压: {}", p.display(), e))
            .raw
    }

    /// 递归收集目录下所有文件（相对路径 → 绝对路径）。
    fn collect_files(root: &Path) -> HashMap<String, PathBuf> {
        let mut map: HashMap<String, PathBuf> = HashMap::new();
        fn walk(dir: &Path, base: &Path, map: &mut HashMap<String, PathBuf>) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, base, map);
                    } else {
                        let rel = p
                            .strip_prefix(base)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/");
                        map.insert(rel, p);
                    }
                }
            }
        }
        walk(root, root, &mut map);
        map
    }

    /// 递归逐字节比较两个目录内容（忽略文件属性，只读内容）。
    fn dirs_byte_equal(a: &Path, b: &Path) -> bool {
        let fa = collect_files(a);
        let fb = collect_files(b);
        if fa.len() != fb.len() {
            return false;
        }
        for (rel, ap) in &fa {
            match fb.get(rel) {
                Some(bp) => {
                    if std::fs::read(ap).ok() != std::fs::read(bp).ok() {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }

    /// 找到 backup_root 下唯一的备份子目录（T4 应恰好生成一个）。
    fn only_backup_subdir(bak: &Path) -> PathBuf {
        let mut subs: Vec<PathBuf> = std::fs::read_dir(bak)
            .unwrap_or_else(|e| panic!("读备份根 {} 失败: {}", bak.display(), e))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        assert_eq!(subs.len(), 1, "应恰好生成一个备份子目录，实际 {}", subs.len());
        subs.remove(0)
    }

    // ---- 测试 1：未声明停服 → 拒绝，且不触碰磁盘（无备份生成） ----
    #[test]
    fn guard_undeclared_rejected_and_no_backup() {
        let Some(work) = copy_sample_to_temp("undeclared") else {
            return;
        };
        let bak = tmp_backup_root("undeclared");
        let level_pre = load_raw(&work.join("Level.sav"));

        let res = run_fix_host_with_guard(
            &work,
            OLD_GUID,
            NEW_GUID,
            StopServerAssertion::Undeclared,
            &bak,
        );
        assert!(res.is_err(), "未声明停服必须被拒绝");
        let msg = res.unwrap_err();
        assert!(
            msg.contains("停止") || msg.contains("停服") || msg.contains("服务器"),
            "错误应说明需停服: {msg}"
        );

        // 未触碰磁盘：Level.sav 字节与修改前一致；Players 文件数不变；无备份生成。
        assert_eq!(
            load_raw(&work.join("Level.sav")),
            level_pre,
            "未声明时应完全不修改磁盘"
        );
        let players = work.join("Players");
        let player_count = std::fs::read_dir(&players)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().map_or(false, |x| x == "sav"))
            .count();
        assert!(player_count >= 2, "Players 文件不应被改动，实际 {player_count}");
        assert!(
            std::fs::read_dir(&bak).unwrap().flatten().next().is_none(),
            "拒绝时不应生成任何备份"
        );

        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bak);
    }

    // ---- 测试 2：声明停服 → 执行，且自动备份目录已生成并含世界文件 ----
    #[test]
    fn guard_declared_creates_backup_with_world_files() {
        let Some(work) = copy_sample_to_temp("declared") else {
            return;
        };
        let bak = tmp_backup_root("declared");

        let res = run_fix_host_with_guard(
            &work,
            OLD_GUID,
            NEW_GUID,
            StopServerAssertion::DeclaredStopped,
            &bak,
        );
        assert!(
            res.is_ok(),
            "声明停服应成功执行；err={:?}",
            res.err()
        );
        let (_changed, backup_path) = res.unwrap();
        assert!(backup_path.exists(), "备份目录应存在");
        assert!(
            backup_path.join("Level.sav").is_file(),
            "备份应含 Level.sav"
        );
        assert!(
            backup_path.join("Players").is_dir(),
            "备份应含 Players/"
        );
        assert!(
            std::fs::read_dir(&backup_path).unwrap().flatten().count() > 0,
            "备份应含世界文件"
        );

        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bak);
    }

    // ---- 测试 3：修复中途失败（step(d) 重命名目标已存在为目录）→ 自动回滚到修改前 ----
    //
    // 注入方式（跨平台、无需文件权限操作）：预先在 Players/ 下创建
    // `<old>.sav.tmp_swap` 目录。`fix_host` 的 (d) 步 `rename(old, tmp)` 会尝试把
    // 旧玩家文件重命名到该目录名上——目标已存在且为目录，重命名必然失败。
    // 该失败发生在 (a) Level.sav 交换写回、(b) 两个 player 文件单向交换之后，
    // 即「修复中途」而非早期拒绝，从而真正验证回滚能恢复已被部分改写的世界。
    #[test]
    fn midwrite_failure_rolls_back_to_prechange() {
        let Some(work) = copy_sample_to_temp("midfail") else {
            return;
        };
        let bak = tmp_backup_root("midfail");
        let players = work.join("Players");
        // 预创建 step(d) 的 tmp_swap 目标为目录（与 fix_host.rs 中 tmp_path 命名一致）。
        let tmp_swap = players.join(format!("{}.sav.tmp_swap", OLD_GUID.to_uppercase()));
        std::fs::create_dir(&tmp_swap)
            .unwrap_or_else(|e| panic!("创建 tmp_swap 目录失败: {}", e));

        let res = run_fix_host_with_guard(
            &work,
            OLD_GUID,
            NEW_GUID,
            StopServerAssertion::DeclaredStopped,
            &bak,
        );
        assert!(res.is_err(), "重命名失败应报错并回滚");
        let msg = res.unwrap_err();
        assert!(msg.contains("回滚"), "错误应说明已回滚: {msg}");

        // 回滚后 data_dir 应与备份（修改前）逐字节一致。
        let backup_path = only_backup_subdir(&bak);
        assert!(
            dirs_byte_equal(&work, &backup_path),
            "回滚后世界应与备份（修改前）逐字节一致"
        );

        // 回滚后所有 .sav 仍可解压（无损坏）。
        for entry in std::fs::read_dir(&players).unwrap().flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |x| x == "sav") {
                crate::save_edit::sav_io::SavFile::load(&p)
                    .unwrap_or_else(|e| panic!("回滚后 {} 应可解压: {}", p.display(), e));
            }
        }
        crate::save_edit::sav_io::SavFile::load(&work.join("Level.sav"))
            .expect("回滚后 Level.sav 应可解压");

        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bak);
    }

    // ---- 测试 4：正常路径 → fix_host 仍成功且交换正确（复用 T3 交换断言） ----
    #[test]
    fn guard_declared_normal_swap_ok() {
        let Some(work) = copy_sample_to_temp("normal") else {
            return;
        };
        let bak = tmp_backup_root("normal");
        let players = work.join("Players");
        let old_path = players.join(format!("{}.sav", OLD_GUID.to_uppercase()));
        let new_path = players.join(format!("{}.sav", NEW_GUID.to_uppercase()));
        let level_path = work.join("Level.sav");

        let old_bytes = crate::save_edit::sav_io::guid_bytes(OLD_GUID).expect("解析 old GUID");
        let new_bytes = crate::save_edit::sav_io::guid_bytes(NEW_GUID).expect("解析 new GUID");

        let old_pre = load_raw(&old_path);
        let new_pre = load_raw(&new_path);
        let level_pre = load_raw(&level_path);

        let res = run_fix_host_with_guard(
            &work,
            OLD_GUID,
            NEW_GUID,
            StopServerAssertion::DeclaredStopped,
            &bak,
        );
        assert!(
            res.is_ok(),
            "正常路径应成功；err={:?}",
            res.err()
        );

        // OLD.sav 应等于（原 new 内容，new→old 替换）。
        let old_post = load_raw(&old_path);
        let mut expect_old = new_pre.clone();
        replace_guid_bytes_local(&mut expect_old, &new_bytes, &old_bytes);
        assert_eq!(
            old_post, expect_old,
            "OLD.sav 应等于（原 new 内容，new→old 替换）"
        );
        // NEW.sav 应等于（原 old 内容，old→new 替换）。
        let new_post = load_raw(&new_path);
        let mut expect_new = old_pre.clone();
        replace_guid_bytes_local(&mut expect_new, &old_bytes, &new_bytes);
        assert_eq!(
            new_post, expect_new,
            "NEW.sav 应等于（原 old 内容，old→new 替换）"
        );

        // Level 3-pass：old/new 出现次数互换，且与 swap_guids 结果逐字节一致。
        let level_post = load_raw(&level_path);
        let c_old_pre = level_pre.windows(16).filter(|w| *w == &old_bytes).count();
        let c_new_pre = level_pre.windows(16).filter(|w| *w == &new_bytes).count();
        let c_old_post = level_post.windows(16).filter(|w| *w == &old_bytes).count();
        let c_new_post = level_post.windows(16).filter(|w| *w == &new_bytes).count();
        assert_eq!(
            c_old_post, c_new_pre,
            "Level 中 old 出现次数应 = 交换前 new 的次数"
        );
        assert_eq!(
            c_new_post, c_old_pre,
            "Level 中 new 出现次数应 = 交换前 old 的次数"
        );
        let mut expect_level = level_pre.clone();
        crate::save_edit::sav_io::swap_guids(&mut expect_level, &old_bytes, &new_bytes)
            .expect("swap_guids");
        assert_eq!(
            level_post, expect_level,
            "Level.sav 应与 3-pass swap_guids 结果逐字节一致"
        );

        // 备份仍保留（成功路径保留备份供审计）。
        let _backup_path = only_backup_subdir(&bak);

        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bak);
    }
}
