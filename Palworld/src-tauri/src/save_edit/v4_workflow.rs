use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{Emitter, State};

use crate::backup_service::{self, MigrationWorkflow, WorkflowStage, WorkflowStatus};
use crate::server::ServerState;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowActionRequest {
    pub request_id: String,
    pub workflow_id: String,
}

pub fn complete_workflow(
    backup_root: &Path,
    workflow_id: &str,
) -> Result<MigrationWorkflow, String> {
    let mut workflow = backup_service::load_workflow(backup_root, workflow_id)?;
    if !matches!(
        workflow.stage,
        WorkflowStage::CharacterTransferred
            | WorkflowStage::GuildRestored
            | WorkflowStage::AwaitingGameVerification
    ) || workflow.status != WorkflowStatus::AwaitingVerification
    {
        return Err("当前工作流尚未进入游戏验证阶段".to_string());
    }
    workflow.stage = WorkflowStage::Completed;
    workflow.status = WorkflowStatus::Committed;
    workflow.current_step = "completed".to_string();
    workflow.updated_at_ms = unix_time_ms();
    workflow.error = None;
    backup_service::save_workflow(backup_root, &workflow)?;
    Ok(workflow)
}

pub fn rollback_entire_workflow(
    backup_root: &Path,
    workflow_id: &str,
) -> Result<MigrationWorkflow, String> {
    let mut workflow = backup_service::load_workflow(backup_root, workflow_id)?;
    if matches!(
        workflow.status,
        WorkflowStatus::Committed | WorkflowStatus::RolledBack
    ) {
        return Err("已完成或已回滚的工作流不能再次整流程回滚".to_string());
    }
    let target = PathBuf::from(&workflow.target_world_path);
    if let Some(backup_id) = workflow.full_backup_id.as_deref() {
        backup_service::restore_full_backup(backup_root, backup_id, &target)?;
    } else if let Some(snapshot_id) = workflow.snapshot_ids.last() {
        backup_service::restore_snapshot(backup_root, snapshot_id, &target)?;
    } else {
        return Err("工作流没有可用的完整备份或操作回滚点".to_string());
    }
    workflow.status = WorkflowStatus::RolledBack;
    workflow.current_step = "entire_workflow_rolled_back".to_string();
    workflow.updated_at_ms = unix_time_ms();
    workflow.error = None;
    backup_service::save_workflow(backup_root, &workflow)?;
    Ok(workflow)
}

pub fn recover_interrupted_workflows(backup_root: &Path) -> Result<usize, String> {
    let workflows = backup_service::list_workflows(backup_root)?;
    let mut recovered = 0usize;
    let mut failures = Vec::new();
    for mut workflow in workflows.into_iter().filter(|workflow| {
        matches!(
            workflow.status,
            WorkflowStatus::Applying | WorkflowStatus::RecoveryRequired
        )
    }) {
        let target = PathBuf::from(&workflow.target_world_path);
        let restore = if let Some(snapshot_id) = workflow.snapshot_ids.last() {
            let result = backup_service::restore_snapshot(backup_root, snapshot_id, &target);
            if result.is_ok() {
                let _ = backup_service::update_snapshot_state(
                    backup_root,
                    snapshot_id,
                    crate::backup_service::BackupState::Committed,
                );
            }
            result
        } else if let Some(backup_id) = workflow.full_backup_id.as_deref() {
            backup_service::restore_full_backup(backup_root, backup_id, &target)
        } else {
            Err("中断工作流没有可用的回滚点".to_string())
        };
        workflow.updated_at_ms = unix_time_ms();
        match restore {
            Ok(()) => {
                workflow.status = WorkflowStatus::RolledBack;
                workflow.current_step = "interrupted_operation_recovered".to_string();
                workflow.error = None;
                if let Err(error) = backup_service::save_workflow(backup_root, &workflow) {
                    failures.push(format!("{}: {error}", workflow.id));
                } else {
                    recovered += 1;
                }
            }
            Err(error) => {
                workflow.status = WorkflowStatus::RecoveryRequired;
                workflow.current_step = "recovery_required".to_string();
                workflow.error = Some(error.clone());
                let _ = backup_service::save_workflow(backup_root, &workflow);
                failures.push(format!("{}: {error}", workflow.id));
            }
        }
    }
    if failures.is_empty() {
        Ok(recovered)
    } else {
        Err(format!("部分中断操作恢复失败: {}", failures.join("; ")))
    }
}

#[tauri::command]
pub async fn complete_migration_workflow_v4(
    req: WorkflowActionRequest,
) -> Result<MigrationWorkflow, String> {
    let settings = crate::settings::load_settings()?;
    let backup_root = backup_service::find_workflow_backup_root(&settings, &req.workflow_id)?;
    complete_workflow(&backup_root, &req.workflow_id)
}

#[tauri::command]
pub async fn rollback_migration_workflow_v4(
    req: WorkflowActionRequest,
    state: State<'_, ServerState>,
    app: tauri::AppHandle,
) -> Result<MigrationWorkflow, String> {
    emit_progress(&app, &req.request_id, "checking_server", "正在检查服务器")?;
    crate::save_edit::ensure_server_stopped(&state)?;
    let settings = crate::settings::load_settings()?;
    let backup_root = backup_service::find_workflow_backup_root(&settings, &req.workflow_id)?;
    emit_progress(&app, &req.request_id, "writing_save", "正在回滚迁移")?;
    let workflow = rollback_entire_workflow(&backup_root, &req.workflow_id)?;
    emit_progress(&app, &req.request_id, "checking_result", "正在检查结果")?;
    emit_progress(&app, &req.request_id, "completed", "迁移已回滚")?;
    Ok(workflow)
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

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup_service::{
        self, BackupState, MigrationWorkflow, WorkflowStage, WorkflowStatus, WorldClass,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn user_verification_marks_workflow_completed_without_touching_saves() {
        let temp = TestDir::new("complete");
        let world = temp.path().join("world");
        let backups = temp.path().join("backups");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("Level.sav"), b"unchanged").unwrap();
        backup_service::save_workflow(
            &backups,
            &workflow(&world, WorkflowStage::CharacterTransferred),
        )
        .unwrap();

        let completed = complete_workflow(&backups, "workflow-v4").expect("用户确认后应完成工作流");

        assert_eq!(completed.stage, WorkflowStage::Completed);
        assert_eq!(completed.status, WorkflowStatus::Committed);
        assert_eq!(
            std::fs::read(world.join("Level.sav")).unwrap(),
            b"unchanged"
        );
    }

    #[test]
    fn entire_workflow_rollback_restores_the_pre_migration_full_backup() {
        let temp = TestDir::new("full-rollback");
        let original = temp.path().join("original");
        let world = temp.path().join("world");
        let backups = temp.path().join("backups");
        std::fs::create_dir_all(&original).unwrap();
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(original.join("Level.sav"), b"before-migration").unwrap();
        std::fs::write(world.join("Level.sav"), b"after-migration").unwrap();
        let backup = backup_service::create_full_backup(
            &backups,
            &original,
            "world-v4",
            "服务器世界",
            WorldClass::Server,
            "world_migration",
        )
        .unwrap();
        let mut record = workflow(&world, WorkflowStage::GuildRestored);
        record.full_backup_id = Some(backup.id);
        backup_service::save_workflow(&backups, &record).unwrap();

        let rolled_back =
            rollback_entire_workflow(&backups, "workflow-v4").expect("应恢复迁移前完整备份");

        assert_eq!(
            std::fs::read(world.join("Level.sav")).unwrap(),
            b"before-migration"
        );
        assert_eq!(rolled_back.status, WorkflowStatus::RolledBack);
    }

    #[test]
    fn startup_recovery_restores_an_applying_snapshot() {
        let temp = TestDir::new("startup-recovery");
        let world = temp.path().join("world");
        let backups = temp.path().join("backups");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("Level.sav"), b"before-operation").unwrap();
        let snapshot = backup_service::create_snapshot(
            &backups,
            &world,
            "world-v4",
            "服务器世界",
            WorldClass::Server,
            "workflow-v4",
            "startup_recovery",
            &[PathBuf::from("Level.sav")],
            BackupState::Applying,
        )
        .unwrap();
        std::fs::write(world.join("Level.sav"), b"interrupted-write").unwrap();
        let mut record = workflow(&world, WorkflowStage::CharacterTransferred);
        record.snapshot_ids.push(snapshot.id);
        record.status = WorkflowStatus::Applying;
        backup_service::save_workflow(&backups, &record).unwrap();

        let recovered = recover_interrupted_workflows(&backups).expect("启动时应恢复中断操作");

        assert_eq!(recovered, 1);
        assert_eq!(
            std::fs::read(world.join("Level.sav")).unwrap(),
            b"before-operation"
        );
        assert_eq!(
            backup_service::load_workflow(&backups, "workflow-v4")
                .unwrap()
                .status,
            WorkflowStatus::RolledBack
        );
    }

    fn workflow(world: &Path, stage: WorkflowStage) -> MigrationWorkflow {
        MigrationWorkflow {
            schema_version: 1,
            id: "workflow-v4".to_string(),
            world_id: "world-v4".to_string(),
            source_world_path: world.to_string_lossy().into_owned(),
            target_world_path: world.to_string_lossy().into_owned(),
            full_backup_id: None,
            snapshot_ids: Vec::new(),
            identity: None,
            status: WorkflowStatus::AwaitingVerification,
            stage,
            current_step: "waiting".to_string(),
            created_at_ms: 1,
            updated_at_ms: 1,
            error: None,
        }
    }

    struct TestDir(PathBuf);
    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "palworld-v4-workflow-{label}-{}-{}",
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
