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

pub mod atomic_write;
pub mod fix_host;
pub mod models;
pub mod modifier;
pub mod oodle;
pub mod path_util;
pub mod sav_io;
pub mod tech_edit;
pub mod transfer;
pub mod v4_character_operation;
pub mod v4_full_character_transfer;
pub mod v4_guild_recovery;
pub mod v4_migration;
pub mod v4_workflow;
pub mod world_copy;

use std::path::{Path, PathBuf};

use tauri::{Emitter, State};

use crate::save_edit::models::*;
use crate::server::ServerState;

/// F5 备份根目录：与世界备份/恢复共用 `SaveGames/_backups/<world>/<backup_id>`。
fn backups_root() -> Result<PathBuf, String> {
    let (save_root, _auto) = path_util::resolve_save_games_root()?;
    Ok(save_root.join("_backups"))
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
    let settings = crate::settings::load_settings()?;
    let root = if settings.backup_root.trim().is_empty() {
        backups_root()?
    } else {
        PathBuf::from(settings.backup_root.trim())
    };
    let world_root = root.join(world);
    if let Some(existing) = find_matching_backup(&data_dir, &world_root) {
        return Ok(existing);
    }
    let backup_id = now_backup_id();
    let dest = world_root.join(&backup_id);
    std::fs::create_dir_all(&dest)
        .map_err(|e| format!("创建备份目录 {} 失败: {}", dest.display(), e))?;
    let mut n = 0usize;
    path_util::copy_dir_recursive(&data_dir, &dest, &mut n)?;
    Ok(backup_id)
}

/// 返回与当前世界逐文件一致的最近备份，避免迁移前重复创建相同快照。
fn find_matching_backup(data_dir: &Path, world_root: &Path) -> Option<String> {
    let mut candidates: Vec<(String, std::time::SystemTime)> = std::fs::read_dir(world_root)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            let modified = e.metadata().and_then(|m| m.modified()).ok()?;
            Some((id, modified))
        })
        .collect();
    candidates.sort_by_key(|(_, time)| std::cmp::Reverse(*time));
    candidates.into_iter().find_map(|(id, _)| {
        let backup = world_root.join(&id);
        dirs_equal(data_dir, &backup).then_some(id)
    })
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

/// 从备份回滚世界数据目录。
fn restore_world_dir(world: &str, backup_id: &str) -> Result<(), String> {
    let data_dir = path_util::world_data_dir(world)?;
    let settings = crate::settings::load_settings()?;
    let root = if settings.backup_root.trim().is_empty() {
        backups_root()?
    } else {
        PathBuf::from(settings.backup_root.trim())
    };
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
pub(crate) fn ensure_save_writes_allowed(
    server_running: bool,
    game_running: bool,
) -> Result<(), String> {
    match (server_running, game_running) {
        (true, true) => Err(
            "服务器和游戏客户端正在运行，请全部关闭后再修改存档（避免退出时覆盖修改）".to_string(),
        ),
        (true, false) => {
            Err("服务器正在运行，请先停止服务器再执行存档改写操作（避免存档损坏）".to_string())
        }
        (false, true) => Err(
            "游戏客户端正在运行，请先退出 Palworld 后再修改存档（避免退出时覆盖修改）".to_string(),
        ),
        (false, false) => Ok(()),
    }
}

pub(crate) fn ensure_server_stopped(state: &ServerState) -> Result<(), String> {
    ensure_save_writes_allowed(
        crate::server::is_server_process_running(&*state),
        crate::server::is_palworld_game_running(),
    )
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
    match fix_host::fix_host_save_in_dir(
        data_dir,
        &old_bytes,
        &new_bytes,
        old_host_guid,
        new_char_guid,
    ) {
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
// v2 分步迁移（A 世界，或 B/C 角色与公会绑定）+ 整份快照 / 回滚
// ===========================================================================

/// v2 分步迁移核心（不含停服检测，由命令层负责）。
///
/// 世界迁移和角色身份交换不能在同一次请求中执行。世界迁移完成后，玩家必须先进入
/// 专用服生成目标角色，再单独执行 B/C 身份交换，否则阶段 A 会覆盖新角色。
///
/// 返回的结果含各阶段统计与整份快照 id（供回滚）。
fn migrate_singleplayer_to_server_v2_impl(
    req: &ThreePhaseMigrationRequest,
    save_root: &Path,
) -> Result<MigrationResult, String> {
    if req.run_phase_a && (req.run_phase_b || req.run_phase_c) {
        return Err(
            "世界迁移与角色/公会绑定不能同时执行；请先迁移世界，进入服务器创建角色后再执行绑定"
                .to_string(),
        );
    }
    if req.run_phase_b != req.run_phase_c {
        return Err("角色迁移与公会绑定必须一起执行，不能只运行其中一个阶段".to_string());
    }

    let mut result = MigrationResult::default();

    if req.run_phase_a {
        // 阶段 A：整包世界拷贝（复用既有迁移逻辑，覆盖式顶替目标世界数据层）。
        let migrate_req = MigrateRequest {
            source_world: req.source_world.clone(),
            target_world: req.target_world.clone(),
            source_type: req.source_type.clone(),
            delete_world_option: req.delete_world_option,
        };
        result.phase_a_copied = world_copy::migrate_world_with_root(&migrate_req, save_root)?;
    }

    // B/C 是同一次身份交换：角色文件、Level.sav 中的角色记录和公会引用同步交换。
    if req.run_phase_b && req.run_phase_c {
        let data_dir = path_util::world_data_dir_with_root(&req.target_world, save_root)?;
        let changed = fix_host::fix_host_save_multi(&data_dir, &req.mappings)?;
        result.phase_b_changed = changed;
        result.phase_c_changed = req.mappings.len();
    }

    result.ok = true;
    Ok(result)
}

/// v2 迁移编排包装：整份快照 + 停服守卫 + 失败自动回滚。
///
/// 流程：
/// 1. 停服守卫：未声明直接拒绝（不触碰磁盘）。
/// 2. 整份快照：把 `SaveGames/0/` 完整拷贝到 `_migration_backups/<backup_id>/0/`
///    （迁移前一次性备份，出问题可整份还原）。
/// 3. 三阶段迁移（A 拷贝 → B 角色 → C 公会）。
/// 4. 失败回滚：若某阶段抛错，用快照整份还原 `SaveGames/0/`，返回「已回滚」错误。
pub fn run_migration_v2_with_guard(
    req: &ThreePhaseMigrationRequest,
    assertion: StopServerAssertion,
    migration_backup_root: &Path,
) -> Result<(MigrationResult, PathBuf), String> {
    let (save_root, _auto) = path_util::resolve_save_games_root()?;
    run_migration_v2_with_guard_at_root(req, assertion, &save_root, migration_backup_root)
}

fn run_migration_v2_with_guard_at_root(
    req: &ThreePhaseMigrationRequest,
    assertion: StopServerAssertion,
    save_root: &Path,
    migration_backup_root: &Path,
) -> Result<(MigrationResult, PathBuf), String> {
    if assertion == StopServerAssertion::Undeclared {
        return Err(
            "请先停止该世界对应的服务器，再执行三阶段迁移（运行中替换会被自动存档覆盖，\
             导致迁移结果丢失或部分损坏）"
                .to_string(),
        );
    }

    // 每次操作都快照当前状态，避免后续失败回滚到陈旧的首次快照。
    let backup_id = now_backup_id();
    let backup_path = migration_backup_root.join(&backup_id);
    let zero_dir = save_root.join("0");
    if !zero_dir.is_dir() {
        return Err(format!("SaveGames/0 目录不存在: {}", zero_dir.display()));
    }
    std::fs::create_dir_all(migration_backup_root).map_err(|e| {
        format!(
            "创建迁移备份根 {} 失败: {}",
            migration_backup_root.display(),
            e
        )
    })?;
    let mut n = 0usize;
    path_util::copy_dir_recursive(&zero_dir, &backup_path.join("0"), &mut n)
        .map_err(|e| format!("整份快照 SaveGames/0/ 失败: {}", e))?;

    // (3) 三阶段迁移。
    match migrate_singleplayer_to_server_v2_impl(req, save_root) {
        Ok(mut result) => {
            result.backup_id = backup_id.clone();
            Ok((result, backup_path))
        }
        Err(e) => {
            // (4) 失败回滚：整份还原 SaveGames/0/。
            match rollback_migration_v2_impl(&backup_path, save_root) {
                Ok(()) => Err(format!(
                    "三阶段迁移失败，已整份回滚至快照 {}: {}",
                    backup_path.display(),
                    e
                )),
                Err(rollback_error) => Err(format!(
                    "三阶段迁移失败，且自动回滚失败（快照 {}）: {}; 回滚错误: {}",
                    backup_path.display(),
                    e,
                    rollback_error
                )),
            }
        }
    }
}

/// 用整份快照还原 `SaveGames/0/`（v2 回滚核心）。
fn rollback_migration_v2_impl(backup_path: &Path, save_root: &Path) -> Result<(), String> {
    let backup_zero = backup_path.join("0");
    if !backup_zero.is_dir() {
        return Err(format!(
            "迁移备份不完整（不含 0/ 目录）: {}",
            backup_path.display()
        ));
    }
    let zero_dir = save_root.join("0");
    if zero_dir.is_dir() {
        path_util::remove_dir_recursive(&zero_dir)?;
    }
    let mut n = 0usize;
    path_util::copy_dir_recursive(&backup_zero, &zero_dir, &mut n)
        .map_err(|e| format!("整份回滚 SaveGames/0/ 失败: {}", e))?;
    Ok(())
}

// ===========================================================================
// Tauri 命令（与前端 api/tauri.ts 的 invoke 名称严格对应）
// ===========================================================================

/// L1/L2：解析世界玩家与公会摘要（按世界名，服务器 SaveGames 根下）。
#[tauri::command]
pub async fn f5_world_summary(world_name: String) -> Result<WorldSummary, String> {
    // 存档解析失败边界记录项目日志：解析失败通常意味着存档结构异常或路径错误，
    // 用户无法从 UI 错误信息定位根因，必须落盘以便反馈。
    world_copy::f5_world_summary_impl(&world_name).map_err(|error| {
        crate::app_log::record(
            "ERROR",
            "save.world_summary",
            &error,
            &[("world_name", &world_name)],
        );
        error
    })
}

/// L1/L2：按真实世界目录路径解析玩家与公会摘要（本地单机 / 服务器通用）。
#[tauri::command]
pub async fn f5_world_summary_by_path(path: String) -> Result<WorldSummary, String> {
    world_copy::f5_world_summary_by_path_impl(&path).map_err(|error| {
        crate::app_log::record(
            "ERROR",
            "save.world_summary_by_path",
            &error,
            &[("path", &path)],
        );
        error
    })
}

#[tauri::command]
pub async fn get_modifier_world(
    path: String,
    state: State<'_, ServerState>,
) -> Result<modifier::ModifierWorldState, String> {
    let mut world = modifier::get_modifier_world_impl(&path)?;
    world.server_running = crate::server::is_server_process_running(&*state);
    world.game_running = crate::server::is_palworld_game_running();
    Ok(world)
}

#[tauri::command]
pub async fn discover_modifier_worlds() -> Result<Vec<modifier::ModifierWorldEntry>, String> {
    let (save_root, _) = crate::save_transfer::resolve_save_games_root()?;
    modifier::discover_modifier_worlds_in(&save_root)
}

#[tauri::command]
pub async fn preview_modifier_action(
    request: modifier::ModifierActionRequest,
) -> Result<modifier::ModifierActionPreview, String> {
    modifier::preview_modifier_action_impl(&request)
}

#[tauri::command]
pub async fn apply_modifier_action(
    request: modifier::ModifierActionRequest,
    state: State<'_, ServerState>,
    app: tauri::AppHandle,
) -> Result<modifier::ModifierActionResult, String> {
    let emit = |phase: modifier::ModifierProgressPhase| {
        let _ = app.emit(
            "modifier-operation-progress",
            modifier::ModifierOperationProgress::from(phase),
        );
    };
    emit(modifier::ModifierProgressPhase::CheckingProcesses);
    ensure_server_stopped(&*state)?;
    let settings = crate::settings::load_settings()?;
    let backup_root = crate::backup_service::initialize_backup_root(&settings)?;
    modifier::apply_modifier_action_in_dir_with_progress(&request, &backup_root, emit)
}

/// 修改器：读取指定角色的普通科技点和古代科技点（只读）。
#[tauri::command]
pub async fn get_player_technology_points(
    req: PlayerTechnologyPointsRequest,
) -> Result<PlayerTechnologyPoints, String> {
    tech_edit::player_technology_points_impl(&req)
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

/// 修改器：原子更新角色的普通科技点和古代科技点。
#[tauri::command]
pub async fn update_player_technology_points(
    req: UpdatePlayerTechnologyPointsRequest,
    state: State<'_, ServerState>,
) -> Result<EditResult, String> {
    ensure_server_stopped(&*state)?;
    tech_edit::update_player_technology_points_impl(&req)
}

/// v2 三阶段迁移（A 世界 → B 角色 → C 公会），附整份快照 + 失败回滚。
#[tauri::command]
pub async fn migrate_world_v2(
    req: ThreePhaseMigrationRequest,
    state: State<'_, ServerState>,
) -> Result<MigrationResult, String> {
    ensure_server_stopped(&*state)?;
    let (save_root, _auto) = path_util::resolve_save_games_root()?;
    let migration_backup_root = save_root.join("_migration_backups");
    match run_migration_v2_with_guard(
        &req,
        StopServerAssertion::DeclaredStopped,
        &migration_backup_root,
    ) {
        Ok((result, _backup_path)) => Ok(result),
        Err(e) => Err(e),
    }
}

/// v2 整份回滚：用迁移前快照还原 `SaveGames/0/`。
#[tauri::command]
pub async fn rollback_migration_v2(
    req: RollbackRequest,
    state: State<'_, ServerState>,
) -> Result<EditResult, String> {
    ensure_server_stopped(&*state)?;
    let (save_root, _auto) = path_util::resolve_save_games_root()?;
    let migration_backup_root = save_root.join("_migration_backups");
    let backup_path = migration_backup_root.join(&req.backup_id);
    match rollback_migration_v2_impl(&backup_path, &save_root) {
        Ok(()) => Ok(EditResult {
            ok: true,
            backup_id: req.backup_id,
            roundtrip_ok: true,
            warnings: vec![],
        }),
        Err(e) => Err(e),
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
    use gvas::engine_version::FEngineVersion;
    use gvas::game_version::DeserializedGameVersion;
    use gvas::properties::int_property::IntProperty;
    use gvas::properties::map_property::MapProperty;
    use gvas::properties::struct_property::{StructProperty, StructPropertyValue};
    use gvas::properties::Property;
    use gvas::types::map::HashableIndexMap;

    #[test]
    fn save_write_guard_rejects_server_or_game_processes() {
        assert!(ensure_save_writes_allowed(false, false).is_ok());
        assert!(ensure_save_writes_allowed(true, false)
            .unwrap_err()
            .contains("服务器正在运行"));
        assert!(ensure_save_writes_allowed(false, true)
            .unwrap_err()
            .contains("游戏客户端正在运行"));
    }
    use gvas::types::Guid;
    use gvas::{GvasFile, GvasHeader};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    /// 编排测试使用的两位玩家 UID。
    const OLD_GUID: &str = "3F5D130B000000000000000000000000";
    const NEW_GUID: &str = "4E239D4F000000000000000000000000";

    /// 在临时目录生成有效 GVAS 世界，避免测试依赖会过期的外部备份路径。
    fn copy_sample_to_temp(tag: &str) -> Option<PathBuf> {
        let dst = std::env::temp_dir().join(format!(
            "palworld_t4_{}_{}_{}",
            tag,
            std::process::id(),
            now_backup_id()
        ));
        let _ = std::fs::remove_dir_all(&dst);
        let players = dst.join("Players");
        std::fs::create_dir_all(&players).ok()?;
        let old_bytes = sav_io::guid_bytes(OLD_GUID).ok()?;
        let new_bytes = sav_io::guid_bytes(NEW_GUID).ok()?;
        let old_uid = Guid::from_u8(old_bytes);
        let new_uid = Guid::from_u8(new_bytes);
        write_test_level(&dst.join("Level.sav"), old_uid, new_uid);
        write_test_player(
            &players.join(format!("{}.sav", world_copy::guid_std(&old_bytes))),
            old_uid,
            Guid::from_u8([0xA1; 16]),
            1,
        );
        write_test_player(
            &players.join(format!("{}.sav", world_copy::guid_std(&new_bytes))),
            new_uid,
            Guid::from_u8([0xB2; 16]),
            2,
        );
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

    fn load_raw(p: &Path) -> Vec<u8> {
        crate::save_edit::sav_io::SavFile::load(p)
            .unwrap_or_else(|e| panic!("{} 应可解压: {}", p.display(), e))
            .raw
    }

    fn test_gvas(properties: HashableIndexMap<String, Property>) -> GvasFile {
        GvasFile {
            deserialized_game_version: DeserializedGameVersion::Default,
            header: GvasHeader::Version2 {
                package_file_version: 0x20B,
                engine_version: FEngineVersion::new(5, 0, 0, 0, String::new()),
                custom_version_format: 3,
                custom_versions: HashableIndexMap::new(),
                save_game_class_name: "TestSave".to_string(),
            },
            properties,
        }
    }

    fn guid_property(value: Guid) -> Property {
        Property::StructProperty(StructProperty::new(
            Guid::from_u8([0; 16]),
            "Guid".to_string(),
            StructPropertyValue::Guid(value),
        ))
    }

    fn write_test_player(path: &Path, uid: Guid, instance_id: Guid, marker: i32) {
        let mut individual_id = HashableIndexMap::new();
        individual_id.insert("InstanceId".to_string(), vec![guid_property(instance_id)]);
        let mut save_data = HashableIndexMap::new();
        save_data.insert(
            "IndividualId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                Guid::from_u8([0; 16]),
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(individual_id),
            ))],
        );
        let mut properties = HashableIndexMap::new();
        properties.insert(
            "SaveData".to_string(),
            Property::StructProperty(StructProperty::new(
                Guid::from_u8([0; 16]),
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(save_data),
            )),
        );
        properties.insert("PlayerUId".to_string(), guid_property(uid));
        properties.insert("TestMarker".to_string(), IntProperty::new(marker).into());
        sav_io::SavFile::from_gvas(&test_gvas(properties), sav_io::SavCompression::Plz)
            .expect("构造玩家 GVAS")
            .save(path)
            .expect("写玩家 GVAS");
    }

    fn cspm_key(uid: Guid, instance: Guid) -> Property {
        let mut fields = HashableIndexMap::new();
        fields.insert("PlayerUId".to_string(), vec![guid_property(uid)]);
        fields.insert("InstanceId".to_string(), vec![guid_property(instance)]);
        Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields))
    }

    fn write_test_level(path: &Path, old_uid: Guid, new_uid: Guid) {
        let mut entries = HashableIndexMap::new();
        entries.insert(
            cspm_key(old_uid, Guid::from_u8([0xA1; 16])),
            Property::StructPropertyValue(StructPropertyValue::CustomStruct(
                HashableIndexMap::new(),
            )),
        );
        entries.insert(
            cspm_key(new_uid, Guid::from_u8([0xB2; 16])),
            Property::StructPropertyValue(StructPropertyValue::CustomStruct(
                HashableIndexMap::new(),
            )),
        );
        let cspm = Property::MapProperty(MapProperty::new(
            "StructProperty".to_string(),
            "StructProperty".to_string(),
            0,
            entries,
        ));
        let mut world_fields = HashableIndexMap::new();
        world_fields.insert("CharacterSaveParameterMap".to_string(), vec![cspm]);
        let mut properties = HashableIndexMap::new();
        properties.insert(
            "worldSaveData".to_string(),
            Property::StructProperty(StructProperty::new(
                Guid::from_u8([0; 16]),
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(world_fields),
            )),
        );
        sav_io::SavFile::from_gvas(&test_gvas(properties), sav_io::SavCompression::Plz)
            .expect("构造 Level GVAS")
            .save(path)
            .expect("写 Level GVAS");
    }

    fn test_marker(path: &Path) -> i32 {
        let gvas = sav_io::SavFile::load(path)
            .expect("读取玩家 GVAS")
            .parse()
            .expect("解析玩家 GVAS");
        sav_io::top_field(&gvas, "TestMarker")
            .and_then(sav_io::as_int)
            .expect("TestMarker")
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
        assert_eq!(
            subs.len(),
            1,
            "应恰好生成一个备份子目录，实际 {}",
            subs.len()
        );
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
        assert!(
            player_count >= 2,
            "Players 文件不应被改动，实际 {player_count}"
        );
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
        assert!(res.is_ok(), "声明停服应成功执行；err={:?}", res.err());
        let (_changed, backup_path) = res.unwrap();
        assert!(backup_path.exists(), "备份目录应存在");
        assert!(
            backup_path.join("Level.sav").is_file(),
            "备份应含 Level.sav"
        );
        assert!(backup_path.join("Players").is_dir(), "备份应含 Players/");
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
        std::fs::create_dir(&tmp_swap).unwrap_or_else(|e| panic!("创建 tmp_swap 目录失败: {}", e));

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

        let res = run_fix_host_with_guard(
            &work,
            OLD_GUID,
            NEW_GUID,
            StopServerAssertion::DeclaredStopped,
            &bak,
        );
        assert!(res.is_ok(), "正常路径应成功；err={:?}", res.err());

        assert_eq!(test_marker(&new_path), 1, "目标 UID 文件应得到源角色数据");
        assert_eq!(test_marker(&old_path), 2, "旧 UID 文件应保留目标角色数据");

        let level = sav_io::SavFile::load(&level_path)
            .expect("读取交换后 Level")
            .parse()
            .expect("解析交换后 Level");
        assert_eq!(sav_io::count_guid_in_gvas(&level, &old_bytes), 1);
        assert_eq!(sav_io::count_guid_in_gvas(&level, &new_bytes), 1);

        // 备份仍保留（成功路径保留备份供审计）。
        let _backup_path = only_backup_subdir(&bak);

        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bak);
    }

    // ---- 测试 5：v2 整份回滚核心（rollback_migration_v2_impl，temp 隔离） ----
    #[test]
    fn v2_rollback_restores_whole_savegames() {
        let root =
            std::env::temp_dir().join(format!("palworld_v2_rollback_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let save_root = root.join("SaveGames");
        // 当前世界（将被损坏）
        let live = save_root.join("0").join("TargetWorld").join("TGTGUID");
        std::fs::create_dir_all(live.join("Players")).unwrap();
        std::fs::write(live.join("Level.sav"), b"LIVE-LEVEL-V1").unwrap();
        std::fs::write(live.join("Players").join("OLD.sav"), b"live-player").unwrap();

        // 备份（模拟迁移前快照）
        let backup = root.join("_migration_backups").join("bak1");
        let backup_world = backup.join("0").join("TargetWorld").join("TGTGUID");
        std::fs::create_dir_all(backup_world.join("Players")).unwrap();
        std::fs::write(backup_world.join("Level.sav"), b"BACKUP-LEVEL").unwrap();
        std::fs::write(
            backup_world.join("Players").join("OLD.sav"),
            b"backup-player",
        )
        .unwrap();

        // 先「损坏」当前世界，再回滚
        std::fs::write(live.join("Level.sav"), b"CORRUPT").unwrap();
        let res = rollback_migration_v2_impl(&backup, &save_root);
        assert!(res.is_ok(), "回滚应成功: {:?}", res.err());

        assert_eq!(
            std::fs::read(live.join("Level.sav")).unwrap(),
            b"BACKUP-LEVEL",
            "回滚后 Level.sav 应等于备份"
        );
        assert_eq!(
            std::fs::read(live.join("Players").join("OLD.sav")).unwrap(),
            b"backup-player",
            "回滚后玩家存档应等于备份"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn v2_guard_creates_fresh_snapshot_for_each_operation() {
        let root = std::env::temp_dir().join(format!(
            "palworld_v2_fresh_backup_{}_{}",
            std::process::id(),
            now_backup_id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let save_root = root.join("SaveGames");
        let zero = save_root.join("0");
        let backups = root.join("backups");
        std::fs::create_dir_all(&zero).unwrap();
        std::fs::write(zero.join("state.txt"), b"first").unwrap();
        let req = crate::save_edit::models::ThreePhaseMigrationRequest {
            source_world: String::new(),
            target_world: String::new(),
            source_type: "server".to_string(),
            delete_world_option: false,
            mappings: vec![],
            run_phase_a: false,
            run_phase_b: false,
            run_phase_c: false,
        };

        let (_, first) = run_migration_v2_with_guard_at_root(
            &req,
            StopServerAssertion::DeclaredStopped,
            &save_root,
            &backups,
        )
        .expect("第一次快照");
        std::fs::write(zero.join("state.txt"), b"second").unwrap();
        let (_, second) = run_migration_v2_with_guard_at_root(
            &req,
            StopServerAssertion::DeclaredStopped,
            &save_root,
            &backups,
        )
        .expect("第二次快照");

        assert_ne!(first, second, "每次迁移必须有独立快照");
        assert_eq!(std::fs::read(first.join("0/state.txt")).unwrap(), b"first");
        assert_eq!(
            std::fs::read(second.join("0/state.txt")).unwrap(),
            b"second"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // ---- 测试 6：世界迁移后单独执行 B/C 身份交换（temp 隔离） ----
    #[test]
    fn v2_phase_bc_swap_integrates_without_rerunning_world_copy() {
        let root = std::env::temp_dir().join(format!("palworld_v2_mig_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let save_root = root.join("SaveGames");
        let old_guid = "3F5D130B000000000000000000000000";
        let old_bytes = crate::save_edit::sav_io::guid_bytes(old_guid).unwrap();
        let old_value = Guid::from_u8(old_bytes);
        let old_stem = world_copy::guid_std(&old_bytes);
        let new_guid = "AAAAAAAA000000000000000000000000";
        let new_bytes = crate::save_edit::sav_io::guid_bytes(new_guid).unwrap();
        let new_value = Guid::from_u8(new_bytes);
        let new_stem = world_copy::guid_std(&new_bytes);

        // 目标世界已经完成 Phase A，且两位玩家均已存在。
        let tgt = save_root.join("TargetWorld").join("TGTGUID");
        std::fs::create_dir_all(tgt.join("Players")).unwrap();
        write_test_level(&tgt.join("Level.sav"), old_value, new_value);
        write_test_player(
            &tgt.join("Players").join(format!("{}.sav", old_stem)),
            old_value,
            Guid::from_u8([0xA1; 16]),
            1,
        );
        write_test_player(
            &tgt.join("Players").join(format!("{}.sav", new_stem)),
            new_value,
            Guid::from_u8([0xB2; 16]),
            2,
        );

        let req = crate::save_edit::models::ThreePhaseMigrationRequest {
            source_world: String::new(),
            target_world: "TargetWorld".to_string(),
            source_type: "server".to_string(),
            delete_world_option: false,
            mappings: vec![crate::save_edit::models::UidMapping {
                old_uid: old_guid.to_string(),
                new_uid: new_guid.to_string(),
            }],
            run_phase_a: false,
            run_phase_b: true,
            run_phase_c: true,
        };

        let res = migrate_singleplayer_to_server_v2_impl(&req, &save_root);
        assert!(res.is_ok(), "B/C 身份交换应成功: {:?}", res.err());
        let r = res.unwrap();
        assert!(r.ok, "result.ok 应为 true");
        assert_eq!(r.phase_a_copied, 0, "不得再次运行世界迁移");
        assert!(r.phase_b_changed > 0, "阶段B应改写文件");
        assert_eq!(
            r.phase_c_changed, 1,
            "阶段C应随同一身份交换完成公会引用重绑"
        );

        // 双向交换：两个 CSPM 身份均保留并互换。
        let tgt_level = sav_io::SavFile::load(&tgt.join("Level.sav"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(sav_io::count_guid_in_gvas(&tgt_level, &old_bytes), 1);
        assert_eq!(sav_io::count_guid_in_gvas(&tgt_level, &new_bytes), 1);

        // 两个文件都保留；new UID 得到源角色，old UID 保存服务器空角色。
        let old_path = tgt.join("Players").join(format!("{}.sav", old_stem));
        let new_path = tgt.join("Players").join(format!("{}.sav", new_stem));
        assert!(old_path.is_file() && new_path.is_file());
        assert_eq!(test_marker(&new_path), 1, "目标 UID 文件应得到源角色数据");
        assert_eq!(
            test_marker(&old_path),
            2,
            "旧 UID 文件应保留服务器新角色数据"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn v2_phase_bc_only_preserves_existing_target_without_source_world() {
        let root =
            std::env::temp_dir().join(format!("palworld_v2_phase_bc_only_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let save_root = root.join("SaveGames");
        let target = save_root.join("TargetWorld").join("TGTGUID");
        std::fs::create_dir_all(&target).unwrap();
        let level_path = target.join("Level.sav");
        std::fs::write(&level_path, b"TARGET-WORLD-MUST-NOT-BE-COPIED").unwrap();

        // `run_phase_a=false` is the post-world-migration path. The source no
        // longer needs to exist, and target data must remain in place before
        // Phase B/C take ownership of it.
        let req: ThreePhaseMigrationRequest = serde_json::from_value(serde_json::json!({
            "source_world": "MissingSourceWorld",
            "target_world": "TargetWorld",
            "source_type": "server",
            "delete_world_option": false,
            "mappings": [],
            "run_phase_a": false,
            "run_phase_b": false,
            "run_phase_c": false
        }))
        .expect("请求应可反序列化");

        let result = migrate_singleplayer_to_server_v2_impl(&req, &save_root)
            .expect("B/C-only 不应尝试读取不存在的源世界");
        assert!(result.ok);
        assert_eq!(result.phase_a_copied, 0);
        assert_eq!(
            std::fs::read(&level_path).unwrap(),
            b"TARGET-WORLD-MUST-NOT-BE-COPIED",
            "B/C-only 编排不得覆盖已经迁移并生成新角色的目标世界"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn v2_rejects_world_copy_combined_with_identity_swap_before_writing() {
        let root = std::env::temp_dir().join(format!(
            "palworld_v2_reject_combined_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let save_root = root.join("SaveGames");
        let target = save_root.join("TargetWorld").join("TGTGUID");
        std::fs::create_dir_all(&target).unwrap();
        let level_path = target.join("Level.sav");
        std::fs::write(&level_path, b"UNCHANGED").unwrap();

        let req = ThreePhaseMigrationRequest {
            source_world: "MissingSourceWorld".to_string(),
            target_world: "TargetWorld".to_string(),
            source_type: "server".to_string(),
            delete_world_option: false,
            mappings: vec![UidMapping {
                old_uid: "00000000000000000000000000000001".to_string(),
                new_uid: "4E239D4F000000000000000000000000".to_string(),
            }],
            run_phase_a: true,
            run_phase_b: true,
            run_phase_c: true,
        };

        let error = migrate_singleplayer_to_server_v2_impl(&req, &save_root)
            .expect_err("世界迁移与身份交换不得在同一次请求中执行");
        assert!(
            error.contains("不能同时"),
            "错误信息应说明操作互斥: {error}"
        );
        assert_eq!(std::fs::read(&level_path).unwrap(), b"UNCHANGED");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn v2_rejects_partial_identity_swap_before_writing() {
        let save_root =
            std::env::temp_dir().join(format!("palworld_v2_reject_partial_{}", std::process::id()));
        let base = ThreePhaseMigrationRequest {
            source_world: String::new(),
            target_world: "MissingTarget".to_string(),
            source_type: "server".to_string(),
            delete_world_option: false,
            mappings: vec![],
            run_phase_a: false,
            run_phase_b: true,
            run_phase_c: false,
        };

        let phase_b_error = migrate_singleplayer_to_server_v2_impl(&base, &save_root)
            .expect_err("只运行角色阶段必须被拒绝");
        assert!(phase_b_error.contains("必须一起执行"));

        let phase_c_only = ThreePhaseMigrationRequest {
            run_phase_b: false,
            run_phase_c: true,
            ..base
        };
        let phase_c_error = migrate_singleplayer_to_server_v2_impl(&phase_c_only, &save_root)
            .expect_err("只运行公会阶段必须被拒绝");
        assert!(phase_c_error.contains("必须一起执行"));
    }
}
