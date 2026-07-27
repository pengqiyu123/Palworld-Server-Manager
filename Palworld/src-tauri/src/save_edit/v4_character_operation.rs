use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::backup_service::{
    self, BackupManifest, BackupState, MigrationWorkflow, WorkflowCharacterIdentity, WorkflowStage,
    WorkflowStatus, WorldClass,
};
use crate::save_edit::atomic_write::{self, FileMutation};
use crate::save_edit::sav_io::SavFile;
use crate::save_edit::v4_full_character_transfer::{
    build_full_character_transfer, describe_player_identity, FullCharacterTransferInput,
    TransferSelection,
};
use crate::server::ServerState;

pub struct CharacterCommitPaths<'a> {
    pub target_world: &'a Path,
    pub backup_root: &'a Path,
    pub workflow_id: &'a str,
    pub target_player_file: &'a str,
    pub world_name: &'a str,
    pub snapshot_source: &'a str,
    pub identity: Option<WorkflowCharacterIdentity>,
}

pub struct PreparedCharacterFiles {
    pub level: Vec<u8>,
    pub player: Vec<u8>,
    pub dps: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterCommitOutcome {
    pub workflow: MigrationWorkflow,
    pub snapshot: BackupManifest,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferFullCharacterRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub source_player_file: String,
    pub target_player_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferFullCharacterOutcome {
    pub workflow: MigrationWorkflow,
    pub snapshot: BackupManifest,
    pub inventory_containers: usize,
    pub character_containers: usize,
    pub pals: usize,
    pub dynamic_items: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportFriendCharacterRequest {
    pub request_id: String,
    pub source_world_path: String,
    pub target_world_path: String,
    pub source_player_file: String,
    pub target_player_file: String,
}

pub fn commit_prepared_character_files(
    paths: CharacterCommitPaths<'_>,
    files: PreparedCharacterFiles,
) -> Result<CharacterCommitOutcome, String> {
    validate_player_file_stem(paths.target_player_file)?;
    let mut workflow = backup_service::load_workflow(paths.backup_root, paths.workflow_id)?;
    if workflow.stage != WorkflowStage::AwaitingServerCharacter {
        return Err("当前工作流阶段不允许转移角色，请先完成世界迁移并创建服务器角色".to_string());
    }
    ensure_same_world_path(paths.target_world, Path::new(&workflow.target_world_path))?;

    let player_relative =
        PathBuf::from("Players").join(format!("{}.sav", paths.target_player_file));
    let dps_relative =
        PathBuf::from("Players").join(format!("{}_dps.sav", paths.target_player_file));
    let affected = vec![
        PathBuf::from("Level.sav"),
        player_relative.clone(),
        dps_relative.clone(),
    ];
    let snapshot = backup_service::create_snapshot(
        paths.backup_root,
        paths.target_world,
        &workflow.world_id,
        paths.world_name,
        WorldClass::Server,
        paths.workflow_id,
        paths.snapshot_source,
        &affected,
        BackupState::Applying,
    )?;
    workflow.snapshot_ids.push(snapshot.id.clone());
    if let Some(identity) = paths.identity {
        workflow.identity = Some(identity);
    }
    workflow.status = WorkflowStatus::Applying;
    workflow.current_step = "character_transfer".to_string();
    workflow.updated_at_ms = unix_time_ms();
    backup_service::save_workflow(paths.backup_root, &workflow)?;

    let mutations = vec![
        FileMutation {
            relative_path: PathBuf::from("Level.sav"),
            content: Some(files.level),
        },
        FileMutation {
            relative_path: player_relative,
            content: Some(files.player),
        },
        FileMutation {
            relative_path: dps_relative,
            content: files.dps,
        },
    ];
    if let Err(write_error) = atomic_write::commit_file_set(paths.target_world, &mutations) {
        let restore =
            backup_service::restore_snapshot(paths.backup_root, &snapshot.id, paths.target_world);
        workflow.updated_at_ms = unix_time_ms();
        workflow.error = Some(write_error.clone());
        match restore {
            Ok(()) => {
                workflow.status = WorkflowStatus::RolledBack;
                workflow.current_step = "character_transfer_rolled_back".to_string();
                let _ = backup_service::save_workflow(paths.backup_root, &workflow);
                return Err(format!("角色转移失败，已恢复到操作前状态: {write_error}"));
            }
            Err(restore_error) => {
                workflow.status = WorkflowStatus::RecoveryRequired;
                workflow.current_step = "character_transfer_recovery_required".to_string();
                workflow.error = Some(format!("{write_error}; {restore_error}"));
                let _ = backup_service::update_snapshot_state(
                    paths.backup_root,
                    &snapshot.id,
                    BackupState::RecoveryRequired,
                );
                let _ = backup_service::save_workflow(paths.backup_root, &workflow);
                return Err(format!(
                    "角色转移失败且自动恢复失败，请勿启动服务器: {write_error}; {restore_error}"
                ));
            }
        }
    }

    let snapshot = backup_service::update_snapshot_state(
        paths.backup_root,
        &snapshot.id,
        BackupState::Committed,
    )?;
    workflow.status = WorkflowStatus::AwaitingVerification;
    workflow.stage = WorkflowStage::CharacterTransferred;
    workflow.current_step = "character_transferred".to_string();
    workflow.updated_at_ms = unix_time_ms();
    workflow.error = None;
    backup_service::save_workflow(paths.backup_root, &workflow)?;
    let _ = backup_service::rebuild_index(paths.backup_root);

    Ok(CharacterCommitOutcome { workflow, snapshot })
}

pub fn transfer_full_character_on_disk(
    backup_root: &Path,
    workflow_id: &str,
    source_player_file: &str,
    target_player_file: &str,
) -> Result<TransferFullCharacterOutcome, String> {
    transfer_full_character_on_disk_mode(
        backup_root,
        workflow_id,
        source_player_file,
        target_player_file,
        true,
    )
}

fn transfer_full_character_on_disk_mode(
    backup_root: &Path,
    workflow_id: &str,
    source_player_file: &str,
    target_player_file: &str,
    capture_guild_identity: bool,
) -> Result<TransferFullCharacterOutcome, String> {
    validate_player_file_stem(source_player_file)?;
    validate_player_file_stem(target_player_file)?;
    if source_player_file.eq_ignore_ascii_case(target_player_file) {
        return Err("源角色与目标服务器角色不能是同一个文件".to_string());
    }

    let workflow = backup_service::load_workflow(backup_root, workflow_id)?;
    if workflow.stage != WorkflowStage::AwaitingServerCharacter {
        return Err("当前工作流阶段不允许转移角色，请先完成世界迁移并创建服务器角色".to_string());
    }
    let source_world = PathBuf::from(&workflow.source_world_path);
    let target_world = PathBuf::from(&workflow.target_world_path);
    let source_player_path = source_world
        .join("Players")
        .join(format!("{source_player_file}.sav"));
    let target_player_path = target_world
        .join("Players")
        .join(format!("{target_player_file}.sav"));
    if !source_player_path.is_file() {
        return Err("所选源角色存档不存在".to_string());
    }
    if !target_player_path.is_file() {
        return Err("目标服务器角色不存在，请先登录服务器创建角色".to_string());
    }

    let source_level_file = SavFile::load(&source_world.join("Level.sav"))?;
    let target_level_file = SavFile::load(&target_world.join("Level.sav"))?;
    let source_player_sav = SavFile::load(&source_player_path)?;
    let target_player_sav = SavFile::load(&target_player_path)?;
    let source_level = source_level_file.parse()?;
    let target_level = target_level_file.parse()?;
    let source_player = source_player_sav.parse()?;
    let target_player = target_player_sav.parse()?;
    let source_identity = describe_player_identity(&source_player, "源角色")?;
    let target_identity = describe_player_identity(&target_player, "目标角色")?;
    let source_guild = if capture_guild_identity {
        Some(
            crate::save_edit::v4_guild_recovery::inspect_original_guild_identity(
                &source_level,
                source_identity.group_id,
                source_identity.player_uid,
                source_identity.instance_id,
            )?,
        )
    } else {
        None
    };

    let source_dps_path = source_world
        .join("Players")
        .join(format!("{source_player_file}_dps.sav"));
    let source_dps_file = if source_dps_path.is_file() {
        Some(SavFile::load(&source_dps_path)?)
    } else {
        None
    };
    let source_dps = source_dps_file.as_ref().map(SavFile::parse).transpose()?;
    let candidate = build_full_character_transfer(FullCharacterTransferInput {
        source_level: &source_level,
        source_player: &source_player,
        source_dps: source_dps.as_ref(),
        target_level: &target_level,
        target_player: &target_player,
        selection: TransferSelection {
            source_player_uid: source_identity.player_uid,
            target_player_uid: target_identity.player_uid,
        },
    })?;

    let level_bytes =
        SavFile::from_gvas(&candidate.target_level, target_level_file.compression)?.to_bytes()?;
    let player_bytes =
        SavFile::from_gvas(&candidate.target_player, target_player_sav.compression)?.to_bytes()?;
    let dps_bytes = candidate
        .target_dps
        .as_ref()
        .map(|gvas| {
            let compression = source_dps_file
                .as_ref()
                .map(|sav| sav.compression)
                .unwrap_or(target_player_sav.compression);
            SavFile::from_gvas(gvas, compression)?.to_bytes()
        })
        .transpose()?;

    validate_candidate_file(&target_world, Path::new("Level.sav"), &level_bytes)?;
    validate_candidate_file(
        &target_world,
        &PathBuf::from("Players").join(format!("{target_player_file}.sav")),
        &player_bytes,
    )?;
    if let Some(bytes) = &dps_bytes {
        validate_candidate_file(
            &target_world,
            &PathBuf::from("Players").join(format!("{target_player_file}_dps.sav")),
            bytes,
        )?;
    }

    let identity = source_guild.map(|source_guild| WorkflowCharacterIdentity {
        source_player_file: source_player_file.to_string(),
        target_player_file: target_player_file.to_string(),
        source_player_uid: crate::save_edit::sav_io::format_guid(source_identity.player_uid),
        source_instance_id: crate::save_edit::sav_io::format_guid(source_identity.instance_id),
        source_group_id: crate::save_edit::sav_io::format_guid(source_guild.group_id),
        source_was_guild_admin: source_guild.was_admin,
        target_player_uid: crate::save_edit::sav_io::format_guid(target_identity.player_uid),
        target_instance_id: crate::save_edit::sav_io::format_guid(target_identity.instance_id),
        target_group_id: crate::save_edit::sav_io::format_guid(target_identity.group_id),
    });
    let world_name = target_world
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("服务器世界");
    let committed = commit_prepared_character_files(
        CharacterCommitPaths {
            target_world: &target_world,
            backup_root,
            workflow_id,
            target_player_file,
            world_name,
            snapshot_source: if capture_guild_identity {
                "character_transfer"
            } else {
                "character_import"
            },
            identity,
        },
        PreparedCharacterFiles {
            level: level_bytes,
            player: player_bytes,
            dps: dps_bytes,
        },
    )?;

    Ok(TransferFullCharacterOutcome {
        workflow: committed.workflow,
        snapshot: committed.snapshot,
        inventory_containers: candidate.stats.inventory_containers,
        character_containers: candidate.stats.character_containers,
        pals: candidate.stats.pals,
        dynamic_items: candidate.stats.dynamic_items,
    })
}

pub fn import_friend_character_on_disk(
    backup_root: &Path,
    source_world: &Path,
    target_world: &Path,
    source_player_file: &str,
    target_player_file: &str,
) -> Result<TransferFullCharacterOutcome, String> {
    if !source_world.join("Level.sav").is_file() {
        return Err("源世界缺少 Level.sav".to_string());
    }
    if !target_world.join("Level.sav").is_file() {
        return Err("目标服务器世界缺少 Level.sav".to_string());
    }
    let workflow_id = format!(
        "character-import-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let world_name = target_world
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("服务器世界");
    let now = unix_time_ms();
    backup_service::save_workflow(
        backup_root,
        &MigrationWorkflow {
            schema_version: 1,
            id: workflow_id.clone(),
            world_id: backup_service::world_directory_id(world_name, target_world),
            source_world_path: source_world.to_string_lossy().into_owned(),
            target_world_path: target_world.to_string_lossy().into_owned(),
            full_backup_id: None,
            snapshot_ids: Vec::new(),
            identity: None,
            status: WorkflowStatus::Prepared,
            stage: WorkflowStage::AwaitingServerCharacter,
            current_step: "character_import_created".to_string(),
            created_at_ms: now,
            updated_at_ms: now,
            error: None,
        },
    )?;
    transfer_full_character_on_disk_mode(
        backup_root,
        &workflow_id,
        source_player_file,
        target_player_file,
        false,
    )
}

#[tauri::command]
pub async fn transfer_full_character_v4(
    req: TransferFullCharacterRequest,
    state: State<'_, ServerState>,
    app: tauri::AppHandle,
) -> Result<TransferFullCharacterOutcome, String> {
    emit_progress(&app, &req.request_id, "checking_server", "正在检查服务器")?;
    crate::save_edit::ensure_server_stopped(&state)?;
    let settings = crate::settings::load_settings()?;
    let backup_root = backup_service::find_workflow_backup_root(&settings, &req.workflow_id)?;
    emit_progress(
        &app,
        &req.request_id,
        "creating_backup",
        "正在创建操作回滚点",
    )?;
    emit_progress(&app, &req.request_id, "writing_save", "正在写入角色存档")?;
    let outcome = transfer_full_character_on_disk(
        &backup_root,
        &req.workflow_id,
        &req.source_player_file,
        &req.target_player_file,
    )?;
    emit_progress(&app, &req.request_id, "checking_result", "正在检查结果")?;
    emit_progress(&app, &req.request_id, "completed", "完整角色转移完成")?;
    Ok(outcome)
}

#[tauri::command]
pub async fn import_friend_character_v4(
    req: ImportFriendCharacterRequest,
    state: State<'_, ServerState>,
    app: tauri::AppHandle,
) -> Result<TransferFullCharacterOutcome, String> {
    emit_progress(&app, &req.request_id, "checking_server", "正在检查服务器")?;
    crate::save_edit::ensure_server_stopped(&state)?;
    let settings = crate::settings::load_settings()?;
    let backup_root = backup_service::initialize_backup_root(&settings)?;
    emit_progress(
        &app,
        &req.request_id,
        "creating_backup",
        "正在创建操作回滚点",
    )?;
    emit_progress(&app, &req.request_id, "writing_save", "正在导入角色")?;
    let outcome = import_friend_character_on_disk(
        &backup_root,
        Path::new(&req.source_world_path),
        Path::new(&req.target_world_path),
        &req.source_player_file,
        &req.target_player_file,
    )?;
    emit_progress(&app, &req.request_id, "checking_result", "正在检查结果")?;
    emit_progress(&app, &req.request_id, "completed", "角色导入完成")?;
    Ok(outcome)
}

pub(crate) fn validate_candidate_file(
    world_root: &Path,
    relative: &Path,
    bytes: &[u8],
) -> Result<(), String> {
    let destination = world_root.join(relative);
    let parent = destination
        .parent()
        .ok_or_else(|| "候选存档路径缺少父目录".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| format!("创建候选校验目录失败: {error}"))?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "候选存档文件名无效".to_string())?;
    let temporary = parent.join(format!(".{file_name}.candidate-{}.tmp", unix_time_ms()));
    let result = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| format!("创建候选校验文件失败: {error}"))?;
        file.write_all(bytes)
            .map_err(|error| format!("写入候选校验文件失败: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("同步候选校验文件失败: {error}"))?;
        SavFile::load(&temporary)?.parse()?;
        Ok::<(), String>(())
    })();
    let cleanup = std::fs::remove_file(&temporary);
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(format!("清理候选校验文件失败: {error}")),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn emit_progress(
    app: &tauri::AppHandle,
    request_id: &str,
    phase: &str,
    label: &str,
) -> Result<(), String> {
    app.emit(
        "save-operation-progress",
        crate::save_edit::v4_migration::OperationProgress {
            request_id: request_id.to_string(),
            phase: phase.to_string(),
            label: label.to_string(),
        },
    )
    .map_err(|error| format!("发送操作进度失败: {error}"))
}

fn validate_player_file_stem(value: &str) -> Result<(), String> {
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("角色文件标识必须是 32 位十六进制字符串".to_string());
    }
    Ok(())
}

fn ensure_same_world_path(left: &Path, right: &Path) -> Result<(), String> {
    let left = std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    if left != right {
        return Err("目标世界与迁移工作流记录不一致".to_string());
    }
    Ok(())
}

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup_service::{self, MigrationWorkflow, WorkflowStage, WorkflowStatus};
    use std::path::{Path, PathBuf};

    #[test]
    fn prepared_character_files_commit_with_a_lightweight_snapshot_and_advance_workflow() {
        let temp = TestDir::new("commit");
        let world = temp.path().join("world");
        let backups = temp.path().join("backups");
        std::fs::create_dir_all(world.join("Players")).unwrap();
        std::fs::write(world.join("Level.sav"), b"old-level").unwrap();
        std::fs::write(
            world.join("Players/11111111111111111111111111111111.sav"),
            b"old-player",
        )
        .unwrap();
        backup_service::save_workflow(
            &backups,
            &workflow(&world, WorkflowStage::AwaitingServerCharacter),
        )
        .unwrap();

        let outcome = commit_prepared_character_files(
            CharacterCommitPaths {
                target_world: &world,
                backup_root: &backups,
                workflow_id: "workflow-character",
                target_player_file: "11111111111111111111111111111111",
                world_name: "服务器世界",
                snapshot_source: "character_transfer",
                identity: None,
            },
            PreparedCharacterFiles {
                level: b"new-level".to_vec(),
                player: b"new-player".to_vec(),
                dps: Some(b"new-dps".to_vec()),
            },
        )
        .expect("角色候选文件应作为一个事务提交");

        assert_eq!(
            std::fs::read(world.join("Level.sav")).unwrap(),
            b"new-level"
        );
        assert_eq!(
            std::fs::read(world.join("Players/11111111111111111111111111111111.sav")).unwrap(),
            b"new-player"
        );
        assert_eq!(
            std::fs::read(world.join("Players/11111111111111111111111111111111_dps.sav")).unwrap(),
            b"new-dps"
        );
        assert_eq!(outcome.workflow.stage, WorkflowStage::CharacterTransferred);
        assert_eq!(outcome.snapshot.files.len(), 3);
        assert_eq!(
            outcome.snapshot.state,
            crate::backup_service::BackupState::Committed
        );
        assert!(outcome.snapshot.files.iter().any(|file| file.relative_path
            == "Players/11111111111111111111111111111111_dps.sav"
            && file.absent));
        assert_eq!(
            std::fs::read(
                backups
                    .join("snapshots")
                    .join("server--test")
                    .join(&outcome.snapshot.id)
                    .join("files/Level.sav")
            )
            .unwrap(),
            b"old-level"
        );
    }

    #[test]
    fn character_commit_rejects_wrong_workflow_stage_without_touching_world() {
        let temp = TestDir::new("wrong-stage");
        let world = temp.path().join("world");
        let backups = temp.path().join("backups");
        std::fs::create_dir_all(world.join("Players")).unwrap();
        std::fs::write(world.join("Level.sav"), b"old-level").unwrap();
        std::fs::write(
            world.join("Players/11111111111111111111111111111111.sav"),
            b"old-player",
        )
        .unwrap();
        backup_service::save_workflow(&backups, &workflow(&world, WorkflowStage::Created)).unwrap();

        let error = commit_prepared_character_files(
            CharacterCommitPaths {
                target_world: &world,
                backup_root: &backups,
                workflow_id: "workflow-character",
                target_player_file: "11111111111111111111111111111111",
                world_name: "服务器世界",
                snapshot_source: "character_transfer",
                identity: None,
            },
            PreparedCharacterFiles {
                level: b"new-level".to_vec(),
                player: b"new-player".to_vec(),
                dps: None,
            },
        )
        .expect_err("未完成世界迁移的工作流必须拒绝角色写入");

        assert!(error.contains("世界迁移") || error.contains("阶段"));
        assert_eq!(
            std::fs::read(world.join("Level.sav")).unwrap(),
            b"old-level"
        );
        assert_eq!(
            std::fs::read(world.join("Players/11111111111111111111111111111111.sav")).unwrap(),
            b"old-player"
        );
        assert!(backup_service::list_snapshots(&backups, None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn real_f1_copy_transfers_character_without_changing_guild_data() {
        let source_original = PathBuf::from(
            "F:/1/local/SaveGames/76561199381352956/1D5D1F304D3AA1FE2818BA98D5223DFE",
        );
        let target_original =
            PathBuf::from("F:/1/server/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE");
        if !source_original.join("Level.sav").is_file()
            || !target_original.join("Level.sav").is_file()
        {
            eprintln!("[skip] F:/1 真实迁移样本不存在");
            return;
        }
        let temp = TestDir::new("real-f1-copy");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let backups = temp.path().join("backups");
        let mut copied = 0;
        crate::save_edit::path_util::copy_dir_recursive(&source_original, &source, &mut copied)
            .unwrap();
        crate::save_edit::path_util::copy_dir_recursive(&target_original, &target, &mut copied)
            .unwrap();
        let pre_migration_level = std::fs::read(target.join("Level.sav")).unwrap();
        let pre_migration_backup = backup_service::create_full_backup(
            &backups,
            &target,
            "real-f1--copy",
            "真实副本服务器世界",
            crate::backup_service::WorldClass::Server,
            "world_migration",
        )
        .unwrap();
        crate::save_edit::path_util::copy_dir_recursive(
            &source.join("Players"),
            &target.join("Players"),
            &mut copied,
        )
        .unwrap();
        let source_level_sav = SavFile::load(&source.join("Level.sav")).unwrap();
        let target_level_sav = SavFile::load(&target.join("Level.sav")).unwrap();
        let mut migrated_level = source_level_sav.parse().unwrap();
        let login_level = target_level_sav.parse().unwrap();
        for field in [
            "CharacterSaveParameterMap",
            "ItemContainerSaveData",
            "CharacterContainerSaveData",
        ] {
            merge_world_map(&mut migrated_level, &login_level, field);
        }
        let migrated_level_bytes =
            SavFile::from_gvas(&migrated_level, target_level_sav.compression)
                .unwrap()
                .to_bytes()
                .unwrap();
        std::fs::write(target.join("Level.sav"), migrated_level_bytes).unwrap();
        backup_service::save_workflow(
            &backups,
            &MigrationWorkflow {
                schema_version: 1,
                id: "workflow-real-f1".to_string(),
                world_id: "real-f1--copy".to_string(),
                source_world_path: source.to_string_lossy().into_owned(),
                target_world_path: target.to_string_lossy().into_owned(),
                full_backup_id: Some(pre_migration_backup.id),
                snapshot_ids: Vec::new(),
                identity: None,
                status: WorkflowStatus::AwaitingVerification,
                stage: WorkflowStage::AwaitingServerCharacter,
                current_step: "awaiting_server_character".to_string(),
                created_at_ms: 1,
                updated_at_ms: 1,
                error: None,
            },
        )
        .unwrap();
        let before = SavFile::load(&target.join("Level.sav"))
            .unwrap()
            .parse()
            .unwrap();
        let guild_before = world_property(&before, "GroupSaveDataMap").cloned();

        let outcome = transfer_full_character_on_disk(
            &backups,
            "workflow-real-f1",
            "00000000000000000000000000000001",
            "4E239D4F000000000000000000000000",
        )
        .expect("F:/1 副本应能完成 V4 角色转移");

        let after = SavFile::load(&target.join("Level.sav"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            world_property(&after, "GroupSaveDataMap").cloned(),
            guild_before
        );
        assert_eq!(outcome.workflow.stage, WorkflowStage::CharacterTransferred);
        let identity = outcome
            .workflow
            .identity
            .as_ref()
            .expect("应持久化迁移身份");
        assert_ne!(
            crate::save_edit::sav_io::parse_guid(&identity.source_group_id)
                .expect("工作流公会 ID 应可解析")
                .to_u8(),
            [0; 16],
            "玩家文件未声明 GroupId 时也必须从 Level.sav 识别原公会"
        );
        assert!(outcome.snapshot.files.iter().all(|file| {
            matches!(
                file.relative_path.as_str(),
                "Level.sav"
                    | "Players/4E239D4F000000000000000000000000.sav"
                    | "Players/4E239D4F000000000000000000000000_dps.sav"
            )
        }));

        let guild_outcome = crate::save_edit::v4_guild_recovery::restore_original_guild_on_disk(
            &backups,
            "workflow-real-f1",
        )
        .expect("F:/1 副本应能在角色转移后恢复原公会");
        assert_eq!(guild_outcome.workflow.stage, WorkflowStage::GuildRestored);

        let rolled_back =
            crate::save_edit::v4_workflow::rollback_entire_workflow(&backups, "workflow-real-f1")
                .expect("发现问题时应能恢复迁移前完整备份");
        assert_eq!(rolled_back.status, WorkflowStatus::RolledBack);
        assert_eq!(
            std::fs::read(target.join("Level.sav")).unwrap(),
            pre_migration_level,
            "整流程回滚必须逐字节恢复迁移前世界"
        );
    }

    #[test]
    fn real_f1_friend_import_has_no_guild_recovery_capability_and_can_roll_back() {
        let source_original = PathBuf::from(
            "F:/1/local/SaveGames/76561199381352956/1D5D1F304D3AA1FE2818BA98D5223DFE",
        );
        let target_original =
            PathBuf::from("F:/1/server/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE");
        if !source_original.join("Level.sav").is_file()
            || !target_original.join("Level.sav").is_file()
        {
            eprintln!("[skip] F:/1 真实迁移样本不存在");
            return;
        }
        let temp = TestDir::new("real-f1-friend-import");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let backups = temp.path().join("backups");
        let mut copied = 0;
        crate::save_edit::path_util::copy_dir_recursive(&source_original, &source, &mut copied)
            .unwrap();
        crate::save_edit::path_util::copy_dir_recursive(&target_original, &target, &mut copied)
            .unwrap();
        let level_before = std::fs::read(target.join("Level.sav")).unwrap();

        let outcome = import_friend_character_on_disk(
            &backups,
            &source,
            &target,
            "00000000000000000000000000000001",
            "4E239D4F000000000000000000000000",
        )
        .expect("朋友角色应能独立导入");

        assert!(outcome.workflow.identity.is_none());
        assert_eq!(outcome.snapshot.source, "character_import");
        let guild_error = crate::save_edit::v4_guild_recovery::restore_original_guild_on_disk(
            &backups,
            &outcome.workflow.id,
        )
        .expect_err("独立角色导入不得具备公会恢复能力");
        assert!(guild_error.contains("缺少后端记录的角色身份"));

        let rolled_back =
            crate::save_edit::v4_workflow::rollback_entire_workflow(&backups, &outcome.workflow.id)
                .expect("独立导入应使用本次操作回滚点恢复");
        assert_eq!(rolled_back.status, WorkflowStatus::RolledBack);
        assert_eq!(
            std::fs::read(target.join("Level.sav")).unwrap(),
            level_before
        );
    }

    fn world_property<'a>(
        gvas: &'a gvas::GvasFile,
        name: &str,
    ) -> Option<&'a gvas::properties::Property> {
        crate::save_edit::sav_io::top_field(gvas, "worldSaveData")
            .and_then(crate::save_edit::sav_io::struct_value)
            .and_then(crate::save_edit::sav_io::custom_fields)
            .and_then(|fields| fields.get(name))
            .and_then(|values| values.first())
    }

    fn merge_world_map(destination: &mut gvas::GvasFile, source: &gvas::GvasFile, name: &str) {
        let source_entries = match world_property(source, name).expect("源世界应包含待合并 Map")
        {
            gvas::properties::Property::MapProperty(
                gvas::properties::map_property::MapProperty::Properties { value, .. },
            ) => value.clone(),
            _ => panic!("{name} 应为 MapProperty"),
        };
        let destination_world =
            crate::save_edit::sav_io::top_field_mut(destination, "worldSaveData")
                .and_then(crate::save_edit::sav_io::struct_value_mut)
                .and_then(crate::save_edit::sav_io::custom_fields_mut)
                .expect("目标世界应包含 worldSaveData");
        let destination_entries = match destination_world
            .get_mut(name)
            .and_then(|values| values.first_mut())
            .expect("目标世界应包含待合并 Map")
        {
            gvas::properties::Property::MapProperty(
                gvas::properties::map_property::MapProperty::Properties { value, .. },
            ) => value,
            _ => panic!("{name} 应为 MapProperty"),
        };
        for (key, value) in source_entries.0 {
            destination_entries.insert(key, value);
        }
    }

    fn workflow(world: &Path, stage: WorkflowStage) -> MigrationWorkflow {
        MigrationWorkflow {
            schema_version: 1,
            id: "workflow-character".to_string(),
            world_id: "server--test".to_string(),
            source_world_path: world.to_string_lossy().into_owned(),
            target_world_path: world.to_string_lossy().into_owned(),
            full_backup_id: Some("full".to_string()),
            snapshot_ids: Vec::new(),
            identity: None,
            status: WorkflowStatus::AwaitingVerification,
            stage,
            current_step: "awaiting_server_character".to_string(),
            created_at_ms: 1,
            updated_at_ms: 1,
            error: None,
        }
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "palworld-v4-character-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
