use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::backup_service::{
    self, BackupManifest, MigrationWorkflow, WorkflowStage, WorkflowStatus, WorldClass,
};
use crate::save_edit::world_copy;
use crate::server::ServerState;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrateWorldV4Request {
    pub request_id: String,
    pub source_path: String,
    pub source_name: String,
    pub target_world: String,
    #[serde(default = "default_true")]
    pub preserve_server_config: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationProgress {
    pub request_id: String,
    pub phase: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct WorldMigrationPaths<'a> {
    pub source_world: &'a Path,
    pub target_world: &'a Path,
    pub backup_root: &'a Path,
    pub target_name: &'a str,
    pub workflow_id: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMigrationOutcome {
    pub workflow: MigrationWorkflow,
    pub backup: BackupManifest,
    pub copied_files: usize,
}

#[cfg(test)]
pub fn migrate_world_transactional_with_backup(
    paths: WorldMigrationPaths<'_>,
) -> Result<WorldMigrationOutcome, String> {
    migrate_world_transactional_with_progress(paths, |_| {})
}

fn migrate_world_transactional_with_progress<F>(
    paths: WorldMigrationPaths<'_>,
    mut progress: F,
) -> Result<WorldMigrationOutcome, String>
where
    F: FnMut((&'static str, &'static str)),
{
    if !paths.source_world.join("Level.sav").is_file() {
        return Err("源世界缺少 Level.sav".to_string());
    }
    if !paths.target_world.join("Level.sav").is_file() {
        return Err("目标服务器世界缺少 Level.sav".to_string());
    }
    if paths.source_world == paths.target_world {
        return Err("源世界与目标世界相同，无需迁移".to_string());
    }
    std::fs::create_dir_all(paths.backup_root)
        .map_err(|error| format!("创建备份目录失败: {error}"))?;

    let now = unix_time_ms();
    let world_id = backup_service::world_directory_id(paths.target_name, paths.target_world);
    let mut workflow = MigrationWorkflow {
        schema_version: 1,
        id: paths.workflow_id.to_string(),
        world_id: world_id.clone(),
        source_world_path: paths.source_world.to_string_lossy().into_owned(),
        target_world_path: paths.target_world.to_string_lossy().into_owned(),
        full_backup_id: None,
        snapshot_ids: Vec::new(),
        identity: None,
        status: WorkflowStatus::Prepared,
        stage: WorkflowStage::Created,
        current_step: "created".to_string(),
        created_at_ms: now,
        updated_at_ms: now,
        error: None,
    };
    backup_service::save_workflow(paths.backup_root, &workflow)?;

    progress(("creating_backup", "正在创建完整备份"));
    let backup = backup_service::create_full_backup(
        paths.backup_root,
        paths.target_world,
        &world_id,
        paths.target_name,
        WorldClass::Server,
        "world_migration",
    )?;
    workflow.full_backup_id = Some(backup.id.clone());
    workflow.stage = WorkflowStage::BackupCreated;
    workflow.current_step = "backup_created".to_string();
    workflow.updated_at_ms = unix_time_ms();
    backup_service::save_workflow(paths.backup_root, &workflow)?;

    workflow.status = WorkflowStatus::Applying;
    workflow.current_step = "world_migration".to_string();
    workflow.updated_at_ms = unix_time_ms();
    backup_service::save_workflow(paths.backup_root, &workflow)?;

    progress(("writing_save", "正在写入世界存档"));
    let copied_files =
        match world_copy::replace_world_transactional(paths.source_world, paths.target_world) {
            Ok(copied) => copied,
            Err(error) => {
                let restore = backup_service::restore_full_backup(
                    paths.backup_root,
                    &backup.id,
                    paths.target_world,
                );
                workflow.updated_at_ms = unix_time_ms();
                workflow.error = Some(error.clone());
                match restore {
                    Ok(()) => {
                        workflow.status = WorkflowStatus::RolledBack;
                        workflow.current_step = "rolled_back".to_string();
                        let _ = backup_service::save_workflow(paths.backup_root, &workflow);
                        return Err(format!("世界迁移失败，已恢复到操作前状态: {error}"));
                    }
                    Err(restore_error) => {
                        workflow.status = WorkflowStatus::RecoveryRequired;
                        workflow.current_step = "recovery_required".to_string();
                        workflow.error = Some(format!("{error}; {restore_error}"));
                        let _ = backup_service::save_workflow(paths.backup_root, &workflow);
                        return Err(format!(
                            "世界迁移失败且自动恢复失败，请勿启动服务器: {error}; {restore_error}"
                        ));
                    }
                }
            }
        };

    progress(("checking_result", "正在检查迁移结果"));
    workflow.status = WorkflowStatus::AwaitingVerification;
    workflow.stage = WorkflowStage::AwaitingServerCharacter;
    workflow.current_step = "awaiting_server_character".to_string();
    workflow.updated_at_ms = unix_time_ms();
    workflow.error = None;
    backup_service::save_workflow(paths.backup_root, &workflow)?;
    let _ = backup_service::rebuild_index(paths.backup_root);

    Ok(WorldMigrationOutcome {
        workflow,
        backup,
        copied_files,
    })
}

#[tauri::command]
pub async fn migrate_world_v4(
    req: MigrateWorldV4Request,
    state: State<'_, ServerState>,
    app: tauri::AppHandle,
) -> Result<WorldMigrationOutcome, String> {
    // 迁移是高风险破坏性操作，任一步骤失败（服务器未停、源档缺失、备份初始化失败、
    // 事务回滚失败等）都必须落盘项目日志，便于用户反馈与事后追溯。
    // 业务体包进 IIFE，在边界统一记录错误，不污染每个 ? 的错误信息。
    let result = (|| -> Result<WorldMigrationOutcome, String> {
        if req.source_name.trim().is_empty() {
            return Err("源世界名称不能为空".to_string());
        }
        if !req.preserve_server_config {
            return Err("世界迁移必须保留服务器配置".to_string());
        }
        emit_progress(&app, &req.request_id, "checking_server", "正在检查服务器")?;
        crate::save_edit::ensure_server_stopped(&state)?;

        let source = crate::save_edit::path_util::find_world_data_dir(Path::new(&req.source_path))
            .ok_or_else(|| format!("未找到本地世界存档: {}", req.source_path))?;
        let target = crate::save_edit::path_util::world_data_dir(&req.target_world)?;
        let settings = crate::settings::load_settings()?;
        let backup_root = backup_service::initialize_backup_root(&settings)?;
        let workflow_id = new_workflow_id();
        let request_id = req.request_id.clone();
        let app_for_progress = app.clone();
        let outcome = migrate_world_transactional_with_progress(
            WorldMigrationPaths {
                source_world: &source,
                target_world: &target,
                backup_root: &backup_root,
                target_name: &req.target_world,
                workflow_id: &workflow_id,
            },
            move |(phase, label)| {
                let _ = emit_progress(&app_for_progress, &request_id, phase, label);
            },
        )?;
        emit_progress(&app, &req.request_id, "completed", "世界迁移完成")?;
        Ok(outcome)
    })();
    if let Err(error) = &result {
        crate::app_log::record(
            "ERROR",
            "save.migrate_world_v4",
            error,
            &[
                ("request_id", &req.request_id),
                ("source_name", &req.source_name),
            ],
        );
    }
    result
}

fn emit_progress(
    app: &tauri::AppHandle,
    request_id: &str,
    phase: &str,
    label: &str,
) -> Result<(), String> {
    app.emit(
        "save-operation-progress",
        OperationProgress {
            request_id: request_id.to_string(),
            phase: phase.to_string(),
            label: label.to_string(),
        },
    )
    .map_err(|error| format!("发送操作进度失败: {error}"))
}

fn new_workflow_id() -> String {
    format!("workflow-{}-{}", unix_time_ms(), std::process::id())
}

fn default_true() -> bool {
    true
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
    use std::path::PathBuf;

    #[test]
    fn world_migration_creates_server_backup_before_transactional_replace() {
        let temp = TestDir::new("success");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let backups = temp.path().join("backups");
        std::fs::create_dir_all(source.join("Players")).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(source.join("Level.sav"), b"source-level").unwrap();
        std::fs::write(source.join("Players/source.sav"), b"source-player").unwrap();
        std::fs::write(source.join("WorldOption.sav"), b"source-option").unwrap();
        std::fs::write(target.join("Level.sav"), b"target-level").unwrap();
        std::fs::write(target.join("WorldOption.sav"), b"server-option").unwrap();

        let outcome = migrate_world_transactional_with_backup(WorldMigrationPaths {
            source_world: &source,
            target_world: &target,
            backup_root: &backups,
            target_name: "服务器世界",
            workflow_id: "workflow-world-success",
        })
        .expect("迁移应成功");

        assert_eq!(
            std::fs::read(target.join("Level.sav")).unwrap(),
            b"source-level"
        );
        assert!(
            !target.join("WorldOption.sav").exists(),
            "专用服迁移后必须由 PalWorldSettings.ini 管理配置"
        );
        assert_eq!(
            outcome.workflow.stage,
            WorkflowStage::AwaitingServerCharacter
        );
        assert_eq!(
            outcome.workflow.full_backup_id.as_deref(),
            Some(outcome.backup.id.as_str())
        );
        assert!(backups
            .join("server")
            .join(&outcome.workflow.world_id)
            .join(&outcome.backup.id)
            .join("world/Level.sav")
            .is_file());
        assert_eq!(
            std::fs::read(
                backups
                    .join("server")
                    .join(&outcome.workflow.world_id)
                    .join(&outcome.backup.id)
                    .join("world/WorldOption.sav")
            )
            .unwrap(),
            b"server-option",
            "目标原有配置必须保留在迁移前完整备份中"
        );
        assert_eq!(
            backup_service::load_workflow(&backups, "workflow-world-success")
                .unwrap()
                .stage,
            WorkflowStage::AwaitingServerCharacter
        );
    }

    #[test]
    fn unwritable_backup_root_prevents_all_world_changes() {
        let temp = TestDir::new("backup-blocked");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let backup_file = temp.path().join("not-a-directory");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(source.join("Level.sav"), b"source-level").unwrap();
        std::fs::write(target.join("Level.sav"), b"target-level").unwrap();
        std::fs::write(&backup_file, b"blocked").unwrap();

        let error = migrate_world_transactional_with_backup(WorldMigrationPaths {
            source_world: &source,
            target_world: &target,
            backup_root: &backup_file,
            target_name: "target",
            workflow_id: "workflow-world-blocked",
        })
        .expect_err("不可写备份根必须阻止迁移");

        assert!(error.contains("备份") || error.contains("目录"));
        assert_eq!(
            std::fs::read(target.join("Level.sav")).unwrap(),
            b"target-level"
        );
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "palworld-v4-migration-{label}-{}-{}",
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
