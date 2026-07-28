use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use tauri::{command, State};

const BACKUP_DIRECTORIES: [&str; 5] = [
    "local",
    "server",
    "snapshots",
    "_system",
    "_system/workflows",
];
const MANIFEST_FILE: &str = "manifest.json";
const PAYLOAD_DIRECTORY: &str = "world";
const COPY_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorldClass {
    Local,
    Server,
}

impl WorldClass {
    fn directory(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Server => "server",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    Full,
    Snapshot,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupState {
    Applying,
    Committed,
    RecoveryRequired,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FileFingerprint {
    pub relative_path: String,
    pub size: u64,
    pub sha256: Option<String>,
    #[serde(default)]
    pub absent: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub id: String,
    pub world_id: String,
    pub world_name: String,
    #[serde(default)]
    pub world_path: String,
    pub world_class: WorldClass,
    pub kind: BackupKind,
    pub state: BackupState,
    pub source: String,
    pub created_at_ms: u64,
    pub total_size: u64,
    #[serde(default)]
    pub player_count: Option<usize>,
    #[serde(default)]
    pub save_version: Option<String>,
    pub files: Vec<FileFingerprint>,
    pub workflow_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BackupIndex {
    pub schema_version: u32,
    pub rebuilt_at_ms: u64,
    pub backups: Vec<BackupManifest>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Prepared,
    Applying,
    AwaitingVerification,
    Committed,
    RecoveryRequired,
    RolledBack,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStage {
    #[default]
    Created,
    BackupCreated,
    WorldMigrated,
    AwaitingServerCharacter,
    CharacterTransferred,
    GuildRestored,
    AwaitingGameVerification,
    Completed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WorkflowCharacterIdentity {
    pub source_player_file: String,
    pub target_player_file: String,
    pub source_player_uid: String,
    pub source_instance_id: String,
    pub source_group_id: String,
    pub source_was_guild_admin: bool,
    pub target_player_uid: String,
    pub target_instance_id: String,
    pub target_group_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MigrationWorkflow {
    pub schema_version: u32,
    pub id: String,
    pub world_id: String,
    pub source_world_path: String,
    pub target_world_path: String,
    pub full_backup_id: Option<String>,
    #[serde(default)]
    pub snapshot_ids: Vec<String>,
    #[serde(default)]
    pub identity: Option<WorkflowCharacterIdentity>,
    pub status: WorkflowStatus,
    #[serde(default)]
    pub stage: WorkflowStage,
    pub current_step: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub error: Option<String>,
}

pub fn resolve_backup_root(settings: &AppSettings) -> Result<PathBuf, String> {
    if !settings.backup_root.trim().is_empty() {
        return Ok(PathBuf::from(settings.backup_root.trim()));
    }
    // 默认目录统一由 app_paths 解析：
    // - 安装模式 debug：CARGO_MANIFEST_DIR/.. /backups（逐字保留 HEAD，守护既有回归测试）
    // - 安装模式 release：EXE 同级 /backups（HEAD 既有）
    // - 便携模式：EXE 同级 /backups
    Ok(crate::app_paths::current()?
        .world_backups_dir()
        .to_path_buf())
}

pub fn initialize_backup_root(settings: &AppSettings) -> Result<PathBuf, String> {
    let root = resolve_backup_root(settings)?;
    for relative in BACKUP_DIRECTORIES {
        std::fs::create_dir_all(root.join(relative))
            .map_err(|error| format!("创建备份目录失败: {error}"))?;
    }
    Ok(root)
}

pub fn world_directory_id(world_name: &str, world_path: &Path) -> String {
    let mut slug = String::new();
    let mut previous_separator = false;
    for character in world_name.chars() {
        let normalized = character.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            previous_separator = false;
        } else if !previous_separator && !slug.is_empty() {
            slug.push('-');
            previous_separator = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("world");
    }
    slug.truncate(48);

    let canonical = fs::canonicalize(world_path).unwrap_or_else(|_| world_path.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().to_ascii_lowercase().as_bytes());
    let short = format!("{:x}", hasher.finalize());
    format!("{slug}--{}", &short[..8])
}

pub fn create_full_backup(
    backup_root: &Path,
    source_world: &Path,
    world_id: &str,
    world_name: &str,
    world_class: WorldClass,
    source: &str,
) -> Result<BackupManifest, String> {
    validate_segment(world_id, "世界 ID")?;
    if !source_world.is_dir() {
        return Err("源世界目录不存在或不是目录".to_string());
    }

    let id = new_record_id("full");
    let parent = backup_root.join(world_class.directory()).join(world_id);
    fs::create_dir_all(&parent).map_err(|error| format!("创建世界备份目录失败: {error}"))?;
    let staging = parent.join(format!(".{id}.tmp"));
    let final_path = parent.join(&id);
    let payload = staging.join(PAYLOAD_DIRECTORY);

    let result = (|| {
        fs::create_dir_all(&payload).map_err(|error| format!("创建备份暂存目录失败: {error}"))?;
        let files = copy_tree_with_fingerprints(source_world, &payload)?;
        let manifest = BackupManifest {
            schema_version: 1,
            id: id.clone(),
            world_id: world_id.to_string(),
            world_name: world_name.to_string(),
            world_path: source_world.to_string_lossy().into_owned(),
            world_class,
            kind: BackupKind::Full,
            state: BackupState::Committed,
            source: source.to_string(),
            created_at_ms: unix_time_ms(),
            total_size: files.iter().map(|file| file.size).sum(),
            player_count: Some(count_player_files(source_world)?),
            save_version: None,
            files,
            workflow_id: None,
        };
        write_json(&staging.join(MANIFEST_FILE), &manifest, "写入备份清单失败")?;
        fs::rename(&staging, &final_path).map_err(|error| format!("提交完整备份失败: {error}"))?;
        Ok(manifest)
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

pub fn list_full_backups(backup_root: &Path) -> Result<Vec<BackupManifest>, String> {
    let mut manifests = Vec::new();
    for class in [WorldClass::Local, WorldClass::Server] {
        collect_manifests(
            &backup_root.join(class.directory()),
            BackupKind::Full,
            &mut manifests,
        )?;
    }
    manifests.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(manifests)
}

pub fn delete_full_backup(backup_root: &Path, backup_id: &str) -> Result<(), String> {
    validate_segment(backup_id, "备份 ID")?;
    let manifests = list_full_backups(backup_root)?;
    let manifest = manifests
        .into_iter()
        .find(|manifest| manifest.id == backup_id)
        .ok_or_else(|| "找不到指定的完整备份".to_string())?;
    let backup_path = backup_root
        .join(manifest.world_class.directory())
        .join(&manifest.world_id)
        .join(&manifest.id);
    fs::remove_dir_all(backup_path).map_err(|error| format!("删除完整备份失败: {error}"))
}

pub fn restore_full_backup(
    backup_root: &Path,
    backup_id: &str,
    target_world: &Path,
) -> Result<(), String> {
    validate_segment(backup_id, "备份 ID")?;
    let manifest = list_full_backups(backup_root)?
        .into_iter()
        .find(|manifest| manifest.id == backup_id)
        .ok_or_else(|| "找不到指定的完整备份".to_string())?;
    let backup_path = backup_root
        .join(manifest.world_class.directory())
        .join(&manifest.world_id)
        .join(&manifest.id);
    let payload = backup_path.join(PAYLOAD_DIRECTORY);
    verify_payload(&payload, &manifest)?;
    replace_directory_transactional(&payload, target_world)
}

pub fn restore_full_backup_to_recorded_world(
    backup_root: &Path,
    backup_id: &str,
) -> Result<(), String> {
    let manifest = list_full_backups(backup_root)?
        .into_iter()
        .find(|manifest| manifest.id == backup_id)
        .ok_or_else(|| "找不到指定的完整备份".to_string())?;
    if manifest.world_path.trim().is_empty() {
        return Err("旧版备份未记录原世界位置，无法自动恢复".to_string());
    }
    restore_full_backup(backup_root, backup_id, Path::new(&manifest.world_path))
}

pub fn create_snapshot(
    backup_root: &Path,
    world_root: &Path,
    world_id: &str,
    world_name: &str,
    world_class: WorldClass,
    workflow_id: &str,
    source: &str,
    affected_files: &[PathBuf],
    state: BackupState,
) -> Result<BackupManifest, String> {
    validate_segment(world_id, "世界 ID")?;
    validate_segment(workflow_id, "工作流 ID")?;
    if !world_root.is_dir() {
        return Err("世界目录不存在或不是目录".to_string());
    }
    if affected_files.is_empty() {
        return Err("受影响文件列表不能为空".to_string());
    }
    for relative in affected_files {
        validate_relative_file(relative)?;
    }

    let id = new_record_id("snapshot");
    let parent = backup_root.join("snapshots").join(world_id);
    fs::create_dir_all(&parent).map_err(|error| format!("创建快照目录失败: {error}"))?;
    let staging = parent.join(format!(".{id}.tmp"));
    let final_path = parent.join(&id);

    let result = (|| {
        let files_root = staging.join("files");
        fs::create_dir_all(&files_root)
            .map_err(|error| format!("创建快照暂存目录失败: {error}"))?;
        let mut fingerprints = Vec::with_capacity(affected_files.len());
        for relative in affected_files {
            let portable_path = portable_relative_path(relative)?;
            let source_path = world_root.join(relative);
            if source_path.is_file() {
                let destination = files_root.join(relative);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("创建快照文件目录失败: {error}"))?;
                }
                fs::copy(&source_path, &destination)
                    .map_err(|error| format!("复制快照文件失败: {error}"))?;
                fingerprints.push(fingerprint_file(&source_path, relative)?);
            } else if source_path.exists() {
                return Err(format!("受影响路径不是文件: {portable_path}"));
            } else {
                fingerprints.push(FileFingerprint {
                    relative_path: portable_path,
                    size: 0,
                    sha256: None,
                    absent: true,
                });
            }
        }
        fingerprints.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        fingerprints.dedup_by(|left, right| left.relative_path == right.relative_path);
        let manifest = BackupManifest {
            schema_version: 1,
            id: id.clone(),
            world_id: world_id.to_string(),
            world_name: world_name.to_string(),
            world_path: world_root.to_string_lossy().into_owned(),
            world_class,
            kind: BackupKind::Snapshot,
            state,
            source: source.to_string(),
            created_at_ms: unix_time_ms(),
            total_size: fingerprints.iter().map(|file| file.size).sum(),
            player_count: Some(count_player_files(world_root)?),
            save_version: None,
            files: fingerprints,
            workflow_id: Some(workflow_id.to_string()),
        };
        write_json(&staging.join(MANIFEST_FILE), &manifest, "写入快照清单失败")?;
        fs::rename(&staging, &final_path).map_err(|error| format!("提交轻量快照失败: {error}"))?;
        Ok(manifest)
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    let manifest = result?;
    if manifest.state == BackupState::Committed {
        prune_committed_snapshots(backup_root, world_id)?;
    }
    Ok(manifest)
}

pub fn list_snapshots(
    backup_root: &Path,
    world_id: Option<&str>,
) -> Result<Vec<BackupManifest>, String> {
    let root = match world_id {
        Some(id) => {
            validate_segment(id, "世界 ID")?;
            backup_root.join("snapshots").join(id)
        }
        None => backup_root.join("snapshots"),
    };
    let mut manifests = Vec::new();
    collect_manifests(&root, BackupKind::Snapshot, &mut manifests)?;
    manifests.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(manifests)
}

pub fn update_snapshot_state(
    backup_root: &Path,
    snapshot_id: &str,
    state: BackupState,
) -> Result<BackupManifest, String> {
    validate_segment(snapshot_id, "快照 ID")?;
    let mut manifest = list_snapshots(backup_root, None)?
        .into_iter()
        .find(|manifest| manifest.id == snapshot_id)
        .ok_or_else(|| "找不到指定的轻量快照".to_string())?;
    manifest.state = state;
    let manifest_path = backup_root
        .join("snapshots")
        .join(&manifest.world_id)
        .join(&manifest.id)
        .join(MANIFEST_FILE);
    write_json_atomic(&manifest_path, &manifest, "更新快照状态失败")?;
    if state == BackupState::Committed {
        prune_committed_snapshots(backup_root, &manifest.world_id)?;
    }
    Ok(manifest)
}

pub fn restore_snapshot(
    backup_root: &Path,
    snapshot_id: &str,
    world_root: &Path,
) -> Result<(), String> {
    validate_segment(snapshot_id, "快照 ID")?;
    if !world_root.is_dir() {
        return Err("世界目录不存在或不是目录".to_string());
    }
    let manifest = list_snapshots(backup_root, None)?
        .into_iter()
        .find(|manifest| manifest.id == snapshot_id)
        .ok_or_else(|| "找不到指定的轻量快照".to_string())?;
    let snapshot_path = backup_root
        .join("snapshots")
        .join(&manifest.world_id)
        .join(&manifest.id);
    let payload = snapshot_path.join("files");
    verify_payload(&payload, &manifest)?;

    for file in manifest.files.iter().filter(|file| !file.absent) {
        let relative = manifest_relative_path(&file.relative_path)?;
        let source = payload.join(&relative);
        let destination = world_root.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建快照恢复目录失败: {error}"))?;
        }
        let temporary = destination.with_extension(format!("restore-{}", new_record_id("tmp")));
        fs::copy(&source, &temporary).map_err(|error| format!("暂存快照恢复文件失败: {error}"))?;
        if destination.exists() {
            fs::remove_file(&destination)
                .map_err(|error| format!("替换快照恢复文件失败: {error}"))?;
        }
        fs::rename(&temporary, &destination)
            .map_err(|error| format!("提交快照恢复文件失败: {error}"))?;
    }
    for file in manifest.files.iter().filter(|file| file.absent) {
        let destination = world_root.join(manifest_relative_path(&file.relative_path)?);
        if destination.is_file() {
            fs::remove_file(destination)
                .map_err(|error| format!("删除快照后新增文件失败: {error}"))?;
        } else if destination.exists() {
            return Err(format!(
                "快照缺失标记对应路径不是文件: {}",
                file.relative_path
            ));
        }
    }
    Ok(())
}

pub fn restore_snapshot_to_recorded_world(
    backup_root: &Path,
    snapshot_id: &str,
) -> Result<(), String> {
    let manifest = list_snapshots(backup_root, None)?
        .into_iter()
        .find(|manifest| manifest.id == snapshot_id)
        .ok_or_else(|| "找不到指定的操作回滚点".to_string())?;
    if manifest.world_path.trim().is_empty() {
        return Err("旧版操作回滚点未记录原世界位置，无法自动回滚".to_string());
    }
    restore_snapshot(backup_root, snapshot_id, Path::new(&manifest.world_path))
}

pub fn delete_snapshot(backup_root: &Path, snapshot_id: &str) -> Result<(), String> {
    validate_segment(snapshot_id, "回滚点 ID")?;
    let manifest = list_snapshots(backup_root, None)?
        .into_iter()
        .find(|manifest| manifest.id == snapshot_id)
        .ok_or_else(|| "找不到指定的操作回滚点".to_string())?;
    if manifest.state != BackupState::Committed {
        return Err("正在写入或需要人工处理的操作回滚点不能删除".to_string());
    }
    let path = backup_root
        .join("snapshots")
        .join(&manifest.world_id)
        .join(&manifest.id);
    fs::remove_dir_all(path).map_err(|error| format!("删除操作回滚点失败: {error}"))
}

pub fn rebuild_index(backup_root: &Path) -> Result<BackupIndex, String> {
    let mut backups = list_full_backups(backup_root)?;
    backups.extend(list_snapshots(backup_root, None)?);
    backups.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    let index = BackupIndex {
        schema_version: 1,
        rebuilt_at_ms: unix_time_ms(),
        backups,
    };
    let system_dir = backup_root.join("_system");
    fs::create_dir_all(&system_dir).map_err(|error| format!("创建备份系统目录失败: {error}"))?;
    write_json_atomic(&system_dir.join("index.json"), &index, "写入备份索引失败")?;
    Ok(index)
}

pub fn save_workflow(backup_root: &Path, workflow: &MigrationWorkflow) -> Result<(), String> {
    validate_segment(&workflow.id, "工作流 ID")?;
    validate_segment(&workflow.world_id, "世界 ID")?;
    if workflow.schema_version == 0 {
        return Err("工作流版本无效".to_string());
    }
    let workflows_dir = backup_root.join("_system/workflows");
    fs::create_dir_all(&workflows_dir).map_err(|error| format!("创建工作流目录失败: {error}"))?;
    write_json_atomic(
        &workflows_dir.join(format!("{}.json", workflow.id)),
        workflow,
        "保存工作流失败",
    )
}

pub fn load_workflow(backup_root: &Path, workflow_id: &str) -> Result<MigrationWorkflow, String> {
    validate_segment(workflow_id, "工作流 ID")?;
    let path = backup_root
        .join("_system/workflows")
        .join(format!("{workflow_id}.json"));
    if !path.is_file() {
        return Err("找不到指定的工作流".to_string());
    }
    read_json(&path, "读取工作流失败")
}

pub fn list_workflows(backup_root: &Path) -> Result<Vec<MigrationWorkflow>, String> {
    let workflows_dir = backup_root.join("_system/workflows");
    if !workflows_dir.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(&workflows_dir)
        .map_err(|error| format!("扫描工作流目录失败: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("扫描工作流目录项失败: {error}"))?;
    let mut workflows = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json") {
            workflows.push(read_json(&path, "读取工作流失败")?);
        }
    }
    workflows.sort_by(|left: &MigrationWorkflow, right: &MigrationWorkflow| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(workflows)
}

fn command_backup_root() -> Result<PathBuf, String> {
    let settings = crate::settings::load_settings()?;
    initialize_backup_root(&settings)
}

pub fn configured_backup_roots(settings: &AppSettings) -> Result<Vec<PathBuf>, String> {
    let current = initialize_backup_root(&settings)?;
    let mut roots = vec![current];
    for root in &settings.backup_roots {
        let path = PathBuf::from(root.trim());
        if path.is_dir()
            && !roots.iter().any(|known| {
                known
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&path.to_string_lossy())
            })
        {
            roots.push(path);
        }
    }
    Ok(roots)
}

pub fn find_workflow_backup_root(
    settings: &AppSettings,
    workflow_id: &str,
) -> Result<PathBuf, String> {
    for root in configured_backup_roots(settings)? {
        if load_workflow(&root, workflow_id).is_ok() {
            return Ok(root);
        }
    }
    Err("找不到指定的迁移工作流".to_string())
}

fn command_backup_roots() -> Result<Vec<PathBuf>, String> {
    let settings = crate::settings::load_settings()?;
    configured_backup_roots(&settings)
}

fn command_full_backups() -> Result<Vec<BackupManifest>, String> {
    let mut manifests = Vec::new();
    for root in command_backup_roots()? {
        manifests.extend(list_full_backups(&root)?);
    }
    manifests.sort_by(|left, right| right.created_at_ms.cmp(&left.created_at_ms));
    manifests.dedup_by(|left, right| left.id == right.id);
    Ok(manifests)
}

fn command_snapshots(world_id: Option<&str>) -> Result<Vec<BackupManifest>, String> {
    let mut manifests = Vec::new();
    for root in command_backup_roots()? {
        manifests.extend(list_snapshots(&root, world_id)?);
    }
    manifests.sort_by(|left, right| right.created_at_ms.cmp(&left.created_at_ms));
    manifests.dedup_by(|left, right| left.id == right.id);
    Ok(manifests)
}

fn find_full_backup_root(backup_id: &str) -> Result<PathBuf, String> {
    for root in command_backup_roots()? {
        if list_full_backups(&root)?
            .iter()
            .any(|backup| backup.id == backup_id)
        {
            return Ok(root);
        }
    }
    Err("找不到指定的完整备份".to_string())
}

fn find_snapshot_root(snapshot_id: &str) -> Result<PathBuf, String> {
    for root in command_backup_roots()? {
        if list_snapshots(&root, None)?
            .iter()
            .any(|snapshot| snapshot.id == snapshot_id)
        {
            return Ok(root);
        }
    }
    Err("找不到指定的操作回滚点".to_string())
}

#[command]
pub async fn backup_create_full(
    source_world: String,
    world_id: String,
    world_name: String,
    world_class: WorldClass,
    source: String,
    state: tauri::State<'_, crate::server::ServerState>,
) -> Result<BackupManifest, String> {
    crate::save_edit::ensure_server_stopped(&state)?;
    create_full_backup(
        &command_backup_root()?,
        Path::new(&source_world),
        &world_id,
        &world_name,
        world_class,
        &source,
    )
}

#[command]
pub async fn backup_get_root() -> Result<String, String> {
    Ok(command_backup_root()?.to_string_lossy().into_owned())
}

#[command]
pub async fn backup_list_full() -> Result<Vec<BackupManifest>, String> {
    command_full_backups()
}

#[command]
pub async fn backup_delete_full(backup_id: String) -> Result<(), String> {
    delete_full_backup(&find_full_backup_root(&backup_id)?, &backup_id)
}

#[command]
pub async fn backup_restore_full(
    backup_id: String,
    state: State<'_, crate::server::ServerState>,
) -> Result<(), String> {
    crate::save_edit::ensure_server_stopped(&state)?;
    restore_full_backup_to_recorded_world(&find_full_backup_root(&backup_id)?, &backup_id)
}

#[command]
pub async fn backup_list_snapshots(
    world_id: Option<String>,
) -> Result<Vec<BackupManifest>, String> {
    command_snapshots(world_id.as_deref())
}

#[command]
pub async fn backup_restore_snapshot(
    snapshot_id: String,
    state: State<'_, crate::server::ServerState>,
) -> Result<(), String> {
    crate::save_edit::ensure_server_stopped(&state)?;
    restore_snapshot_to_recorded_world(&find_snapshot_root(&snapshot_id)?, &snapshot_id)
}

#[command]
pub async fn backup_delete_snapshot(snapshot_id: String) -> Result<(), String> {
    delete_snapshot(&find_snapshot_root(&snapshot_id)?, &snapshot_id)
}

#[command]
pub async fn backup_rebuild_index() -> Result<BackupIndex, String> {
    rebuild_index(&command_backup_root()?)
}

#[command]
pub async fn backup_load_workflow(workflow_id: String) -> Result<MigrationWorkflow, String> {
    let settings = crate::settings::load_settings()?;
    let root = find_workflow_backup_root(&settings, &workflow_id)?;
    load_workflow(&root, &workflow_id)
}

#[command]
pub async fn backup_list_workflows() -> Result<Vec<MigrationWorkflow>, String> {
    let mut workflows = Vec::new();
    for root in command_backup_roots()? {
        workflows.extend(list_workflows(&root)?);
    }
    workflows.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
    workflows.dedup_by(|left, right| left.id == right.id);
    Ok(workflows)
}

fn prune_committed_snapshots(backup_root: &Path, world_id: &str) -> Result<(), String> {
    const RETAINED_COMMITTED_SNAPSHOTS: usize = 3;
    let committed = list_snapshots(backup_root, Some(world_id))?
        .into_iter()
        .filter(|manifest| manifest.state == BackupState::Committed)
        .collect::<Vec<_>>();
    for manifest in committed.into_iter().skip(RETAINED_COMMITTED_SNAPSHOTS) {
        let path = backup_root
            .join("snapshots")
            .join(world_id)
            .join(manifest.id);
        fs::remove_dir_all(path).map_err(|error| format!("清理旧快照失败: {error}"))?;
    }
    Ok(())
}

fn verify_payload(payload_root: &Path, manifest: &BackupManifest) -> Result<(), String> {
    for expected in manifest.files.iter().filter(|file| !file.absent) {
        let relative = manifest_relative_path(&expected.relative_path)?;
        let path = payload_root.join(relative);
        if !path.is_file() {
            return Err(format!("备份载荷缺少文件: {}", expected.relative_path));
        }
        let actual = fingerprint_file(&path, Path::new(&expected.relative_path))?;
        if actual.size != expected.size || actual.sha256 != expected.sha256 {
            return Err(format!("备份文件校验失败: {}", expected.relative_path));
        }
    }
    Ok(())
}

fn replace_directory_transactional(source: &Path, target: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err("备份载荷目录不存在".to_string());
    }
    let parent = target
        .parent()
        .ok_or_else(|| "目标世界目录缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("创建恢复目标父目录失败: {error}"))?;
    let target_name = target
        .file_name()
        .ok_or_else(|| "目标世界目录名称无效".to_string())?
        .to_string_lossy();
    let operation_id = new_record_id("restore");
    let staging = parent.join(format!(".{target_name}.{operation_id}.tmp"));
    let recovery = parent.join(format!(".{target_name}.{operation_id}.previous"));
    fs::create_dir_all(&staging).map_err(|error| format!("创建恢复暂存目录失败: {error}"))?;
    if let Err(error) = copy_tree_with_fingerprints(source, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    let had_target = target.exists();
    if had_target {
        fs::rename(target, &recovery).map_err(|error| format!("暂存当前世界失败: {error}"))?;
    }
    if let Err(error) = fs::rename(&staging, target) {
        if had_target {
            let _ = fs::rename(&recovery, target);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("提交完整备份恢复失败: {error}"));
    }
    if had_target {
        fs::remove_dir_all(&recovery).map_err(|error| format!("清理恢复前世界失败: {error}"))?;
    }
    Ok(())
}

fn copy_tree_with_fingerprints(
    source: &Path,
    destination: &Path,
) -> Result<Vec<FileFingerprint>, String> {
    let mut files = Vec::new();
    copy_tree_level(source, source, destination, &mut files)?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn count_player_files(world_root: &Path) -> Result<usize, String> {
    let players = world_root.join("Players");
    if !players.is_dir() {
        return Ok(0);
    }
    let entries = fs::read_dir(&players).map_err(|error| format!("读取角色目录失败: {error}"))?;
    let mut count = 0usize;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取角色条目失败: {error}"))?;
        let path = entry.path();
        let is_player = path.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("sav")
            && !path
                .file_stem()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.ends_with("_dps"));
        if is_player {
            count += 1;
        }
    }
    Ok(count)
}

fn copy_tree_level(
    root: &Path,
    current: &Path,
    destination: &Path,
    files: &mut Vec<FileFingerprint>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("读取源世界目录失败: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取源世界目录项失败: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取源文件类型失败: {error}"))?;
        if file_type.is_symlink() {
            return Err(format!("备份不支持符号链接: {}", entry.path().display()));
        }
        let source_path = entry.path();
        let relative = source_path
            .strip_prefix(root)
            .map_err(|_| "计算备份相对路径失败".to_string())?;
        let destination_path = destination.join(relative);
        if file_type.is_dir() {
            fs::create_dir_all(&destination_path)
                .map_err(|error| format!("创建备份子目录失败: {error}"))?;
            copy_tree_level(root, &source_path, destination, files)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("创建备份文件目录失败: {error}"))?;
            }
            fs::copy(&source_path, &destination_path)
                .map_err(|error| format!("复制备份文件失败: {error}"))?;
            files.push(fingerprint_file(&source_path, relative)?);
        }
    }
    Ok(())
}

fn fingerprint_file(path: &Path, relative: &Path) -> Result<FileFingerprint, String> {
    let file = File::open(path).map_err(|error| format!("打开待校验文件失败: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("读取文件大小失败: {error}"))?
        .len();
    let mut reader = BufReader::with_capacity(COPY_BUFFER_SIZE, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_SIZE];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("计算文件指纹失败: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(FileFingerprint {
        relative_path: portable_relative_path(relative)?,
        size,
        sha256: Some(format!("{:x}", hasher.finalize())),
        absent: false,
    })
}

fn collect_manifests(
    root: &Path,
    expected_kind: BackupKind,
    manifests: &mut Vec<BackupManifest>,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(root)
        .map_err(|error| format!("扫描备份目录失败: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("扫描备份目录项失败: {error}"))?;
    for entry in entries {
        let path = entry.path();
        if !path.is_dir() || entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let manifest_path = path.join(MANIFEST_FILE);
        if manifest_path.is_file() {
            let manifest: BackupManifest = read_json(&manifest_path, "读取备份清单失败")?;
            if manifest.kind == expected_kind {
                manifests.push(manifest);
            }
        } else {
            collect_manifests(&path, expected_kind, manifests)?;
        }
    }
    Ok(())
}

fn validate_segment(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(format!("{label} 无效"));
    }
    Ok(())
}

fn validate_relative_file(path: &Path) -> Result<(), String> {
    use std::path::Component;
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("受影响文件路径必须是世界目录内的相对路径".to_string());
    }
    Ok(())
}

fn manifest_relative_path(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value.replace('/', std::path::MAIN_SEPARATOR_STR));
    validate_relative_file(&path)?;
    Ok(path)
}

fn portable_relative_path(path: &Path) -> Result<String, String> {
    let parts = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part == "." || part == "..") {
        return Err("备份文件相对路径无效".to_string());
    }
    Ok(parts.join("/"))
}

fn write_json<T: Serialize>(path: &Path, value: &T, context: &str) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("{context}: {error}"))?;
    serde_json::to_writer_pretty(file, value).map_err(|error| format!("{context}: {error}"))
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T, context: &str) -> Result<(), String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("{context}: 文件名无效"))?
        .to_string_lossy();
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", new_record_id("write")));
    write_json(&temporary, value, context)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("{context}: {error}"))?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("{context}: {error}"));
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, context: &str) -> Result<T, String> {
    let file = File::open(path).map_err(|error| format!("{context}: {error}"))?;
    serde_json::from_reader(file).map_err(|error| format!("{context}: {error}"))
}

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn new_record_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    format!(
        "{prefix}-{:x}-{:x}-{:x}",
        unix_time_ms(),
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_backup_command_checks_server_and_game_processes_before_copying() {
        let source = include_str!("backup_service.rs");
        let command = source
            .split("pub async fn backup_create_full")
            .nth(1)
            .expect("full backup command must exist")
            .split("#[command]")
            .next()
            .expect("full backup command must end before the next command");

        assert!(command.contains("ensure_server_stopped"));
    }
    use crate::settings::AppSettings;
    use std::path::{Path, PathBuf};

    fn settings_with_backup_root(path: &Path) -> AppSettings {
        AppSettings {
            backup_root: path.to_string_lossy().into_owned(),
            ..AppSettings::default()
        }
    }

    #[test]
    fn debug_default_backup_root_is_repository_palworld_backups() {
        let root = resolve_backup_root(&AppSettings::default()).expect("应能解析默认备份目录");

        if cfg!(debug_assertions) {
            assert_eq!(
                root,
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("backups")
            );
        }
    }

    #[test]
    fn world_directory_id_is_ascii_safe_and_distinguishes_same_names() {
        let first = world_directory_id("我的世界:主档", Path::new("D:/saves/world-a"));
        let second = world_directory_id("我的世界:主档", Path::new("E:/saves/world-a"));

        assert!(first.starts_with("world--"));
        assert!(first
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'));
        assert_ne!(first, second, "相同显示名的不同世界必须有不同短标识");
    }

    #[test]
    fn workflow_stage_serializes_the_user_visible_resume_point() {
        let encoded = serde_json::to_string(&WorkflowStage::AwaitingServerCharacter).unwrap();
        assert_eq!(encoded, "\"awaiting_server_character\"");
        assert_eq!(
            serde_json::from_str::<WorkflowStage>(&encoded).unwrap(),
            WorkflowStage::AwaitingServerCharacter
        );
    }

    #[test]
    fn workflow_identity_is_backend_owned_and_old_records_default_to_none() {
        let legacy = r#"{
            "schema_version":1,"id":"legacy","world_id":"world",
            "source_world_path":"C:/source","target_world_path":"D:/target",
            "full_backup_id":null,"snapshot_ids":[],"status":"prepared",
            "stage":"created","current_step":"created",
            "created_at_ms":1,"updated_at_ms":1,"error":null
        }"#;
        let decoded: MigrationWorkflow = serde_json::from_str(legacy).unwrap();
        assert!(decoded.identity.is_none());

        let identity = WorkflowCharacterIdentity {
            source_player_file: "source".to_string(),
            target_player_file: "target".to_string(),
            source_player_uid: "source-uid".to_string(),
            source_instance_id: "source-instance".to_string(),
            source_group_id: "source-group".to_string(),
            source_was_guild_admin: true,
            target_player_uid: "target-uid".to_string(),
            target_instance_id: "target-instance".to_string(),
            target_group_id: "target-group".to_string(),
        };
        assert_eq!(identity.source_player_file, "source");
        assert!(identity.source_was_guild_admin);
    }

    #[test]
    fn explicit_backup_root_overrides_default_and_creates_all_directory_classes() {
        let temp = TestDir::new("explicit-root");
        let root = initialize_backup_root(&settings_with_backup_root(temp.path()))
            .expect("应能初始化显式备份目录");

        assert_eq!(root, temp.path());
        for relative in [
            "local",
            "server",
            "snapshots",
            "_system",
            "_system/workflows",
        ] {
            assert!(root.join(relative).is_dir(), "缺少目录 {relative}");
        }
    }

    #[test]
    fn full_backup_copies_world_writes_sha256_manifest_and_can_be_listed() {
        let temp = TestDir::new("full-backup");
        let source = temp.path().join("source-world");
        std::fs::create_dir_all(source.join("Players")).unwrap();
        std::fs::write(source.join("Level.sav"), b"abc").unwrap();
        std::fs::write(source.join("Players/001.sav"), b"player").unwrap();

        let manifest = create_full_backup(
            temp.path(),
            &source,
            "world-a",
            "测试世界",
            WorldClass::Local,
            "manual",
        )
        .expect("应能创建完整备份");

        assert_eq!(manifest.world_path, source.to_string_lossy());
        assert_eq!(manifest.player_count, Some(1));
        assert_eq!(manifest.save_version, None);
        let backup_dir = temp.path().join("local/world-a").join(&manifest.id);
        assert_eq!(
            std::fs::read(backup_dir.join("world/Level.sav")).unwrap(),
            b"abc"
        );
        assert_eq!(
            manifest
                .files
                .iter()
                .find(|file| file.relative_path == "Level.sav")
                .unwrap()
                .sha256
                .as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );

        let saved: BackupManifest =
            serde_json::from_slice(&std::fs::read(backup_dir.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(saved, manifest);
        assert_eq!(list_full_backups(temp.path()).unwrap(), vec![manifest]);
    }

    #[test]
    fn deleting_full_backup_removes_manifest_and_world_payload() {
        let temp = TestDir::new("delete-full");
        let source = temp.path().join("source-world");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("Level.sav"), b"level").unwrap();
        let manifest = create_full_backup(
            temp.path(),
            &source,
            "world-b",
            "世界 B",
            WorldClass::Server,
            "migration",
        )
        .unwrap();

        delete_full_backup(temp.path(), &manifest.id).expect("应能删除完整备份");

        assert!(list_full_backups(temp.path()).unwrap().is_empty());
        assert!(!temp
            .path()
            .join("server/world-b")
            .join(manifest.id)
            .exists());
    }

    #[test]
    fn full_backup_restores_to_the_world_path_recorded_in_its_manifest() {
        let temp = TestDir::new("restore-recorded-full");
        let world = temp.path().join("world");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("Level.sav"), b"before").unwrap();
        let manifest = create_full_backup(
            temp.path(),
            &world,
            "world-recorded",
            "记录世界",
            WorldClass::Server,
            "manual",
        )
        .unwrap();
        std::fs::write(world.join("Level.sav"), b"after").unwrap();

        restore_full_backup_to_recorded_world(temp.path(), &manifest.id).unwrap();

        assert_eq!(std::fs::read(world.join("Level.sav")).unwrap(), b"before");
    }

    #[test]
    fn snapshot_copies_only_explicit_files_and_marks_files_that_were_absent() {
        let temp = TestDir::new("snapshot-files");
        let world = temp.path().join("world");
        std::fs::create_dir_all(world.join("Players")).unwrap();
        std::fs::write(world.join("Level.sav"), b"level").unwrap();
        std::fs::write(world.join("Players/keep.sav"), b"keep").unwrap();

        let manifest = create_snapshot(
            temp.path(),
            &world,
            "world-s",
            "快照世界",
            WorldClass::Server,
            "workflow-1",
            "character_transfer",
            &[
                PathBuf::from("Level.sav"),
                PathBuf::from("Players/missing.sav"),
            ],
            BackupState::Applying,
        )
        .expect("应能创建轻量快照");

        let snapshot_dir = temp.path().join("snapshots/world-s").join(&manifest.id);
        assert_eq!(
            std::fs::read(snapshot_dir.join("files/Level.sav")).unwrap(),
            b"level"
        );
        assert!(!snapshot_dir.join("files/Players/keep.sav").exists());
        assert!(manifest.files.iter().any(|file| {
            file.relative_path == "Players/missing.sav"
                && file.absent
                && file.sha256.is_none()
                && file.size == 0
        }));
    }

    #[test]
    fn snapshot_deletion_rejects_protected_states_and_allows_committed_points() {
        let temp = TestDir::new("delete-snapshot");
        let world = temp.path().join("world");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("Level.sav"), b"level").unwrap();
        let snapshot = create_snapshot(
            temp.path(),
            &world,
            "world-delete",
            "删除测试",
            WorldClass::Server,
            "workflow-delete",
            "character_transfer",
            &[PathBuf::from("Level.sav")],
            BackupState::Applying,
        )
        .unwrap();

        let error =
            delete_snapshot(temp.path(), &snapshot.id).expect_err("写入中的回滚点必须受保护");
        assert!(error.contains("不能删除"));
        update_snapshot_state(temp.path(), &snapshot.id, BackupState::Committed).unwrap();
        delete_snapshot(temp.path(), &snapshot.id).unwrap();
        assert!(list_snapshots(temp.path(), None).unwrap().is_empty());
    }

    #[test]
    fn snapshot_retention_keeps_three_committed_and_never_prunes_protected_states() {
        let temp = TestDir::new("snapshot-retention");
        let world = temp.path().join("world");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("Level.sav"), b"level").unwrap();
        let affected = [PathBuf::from("Level.sav")];

        for _ in 0..5 {
            create_snapshot(
                temp.path(),
                &world,
                "world-r",
                "保留世界",
                WorldClass::Local,
                "workflow-r",
                "test_retention",
                &affected,
                BackupState::Committed,
            )
            .unwrap();
        }
        create_snapshot(
            temp.path(),
            &world,
            "world-r",
            "保留世界",
            WorldClass::Local,
            "workflow-r",
            "test_retention",
            &affected,
            BackupState::Applying,
        )
        .unwrap();
        create_snapshot(
            temp.path(),
            &world,
            "world-r",
            "保留世界",
            WorldClass::Local,
            "workflow-r",
            "test_retention",
            &affected,
            BackupState::RecoveryRequired,
        )
        .unwrap();

        let snapshots = list_snapshots(temp.path(), Some("world-r")).unwrap();
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.state == BackupState::Committed)
                .count(),
            3
        );
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.state == BackupState::Applying)
                .count(),
            1
        );
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.state == BackupState::RecoveryRequired)
                .count(),
            1
        );
    }

    #[test]
    fn restoring_full_backup_replaces_the_target_world_payload() {
        let temp = TestDir::new("restore-full");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        std::fs::create_dir_all(source.join("Players")).unwrap();
        std::fs::write(source.join("Level.sav"), b"backup-level").unwrap();
        std::fs::write(source.join("Players/one.sav"), b"backup-player").unwrap();
        let manifest = create_full_backup(
            temp.path(),
            &source,
            "world-restore",
            "恢复世界",
            WorldClass::Server,
            "manual",
        )
        .unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("Level.sav"), b"changed").unwrap();
        std::fs::write(target.join("stale.sav"), b"stale").unwrap();

        restore_full_backup(temp.path(), &manifest.id, &target).expect("应能恢复完整备份");

        assert_eq!(
            std::fs::read(target.join("Level.sav")).unwrap(),
            b"backup-level"
        );
        assert_eq!(
            std::fs::read(target.join("Players/one.sav")).unwrap(),
            b"backup-player"
        );
        assert!(!target.join("stale.sav").exists());
    }

    #[test]
    fn rolling_back_snapshot_restores_files_and_removes_previously_absent_files() {
        let temp = TestDir::new("rollback-snapshot");
        let world = temp.path().join("world");
        std::fs::create_dir_all(world.join("Players")).unwrap();
        std::fs::write(world.join("Level.sav"), b"before").unwrap();
        let manifest = create_snapshot(
            temp.path(),
            &world,
            "world-rollback",
            "回滚世界",
            WorldClass::Local,
            "workflow-rollback",
            "test_rollback",
            &[PathBuf::from("Level.sav"), PathBuf::from("Players/new.sav")],
            BackupState::Applying,
        )
        .unwrap();
        std::fs::write(world.join("Level.sav"), b"after").unwrap();
        std::fs::write(world.join("Players/new.sav"), b"created-after-snapshot").unwrap();

        restore_snapshot(temp.path(), &manifest.id, &world).expect("应能回滚轻量快照");

        assert_eq!(std::fs::read(world.join("Level.sav")).unwrap(), b"before");
        assert!(!world.join("Players/new.sav").exists());
    }

    #[test]
    fn rebuilding_index_scans_full_and_snapshot_manifests() {
        let temp = TestDir::new("rebuild-index");
        let world = temp.path().join("world");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("Level.sav"), b"level").unwrap();
        let full = create_full_backup(
            temp.path(),
            &world,
            "world-index",
            "索引世界",
            WorldClass::Local,
            "manual",
        )
        .unwrap();
        let snapshot = create_snapshot(
            temp.path(),
            &world,
            "world-index",
            "索引世界",
            WorldClass::Local,
            "workflow-index",
            "test_index",
            &[PathBuf::from("Level.sav")],
            BackupState::Committed,
        )
        .unwrap();

        let index = rebuild_index(temp.path()).expect("应能从清单重建索引");

        assert_eq!(index.backups.len(), 2);
        assert!(index.backups.iter().any(|entry| entry.id == full.id));
        assert!(index.backups.iter().any(|entry| entry.id == snapshot.id));
        let saved: BackupIndex =
            serde_json::from_slice(&std::fs::read(temp.path().join("_system/index.json")).unwrap())
                .unwrap();
        assert_eq!(saved, index);
    }

    #[test]
    fn workflow_is_serialized_updated_and_listed_from_system_directory() {
        let temp = TestDir::new("workflow-persistence");
        let mut workflow = MigrationWorkflow {
            schema_version: 1,
            id: "workflow-persist".to_string(),
            world_id: "world-persist".to_string(),
            source_world_path: "C:/source/world".to_string(),
            target_world_path: "D:/target/world".to_string(),
            full_backup_id: Some("full-1".to_string()),
            snapshot_ids: vec!["snapshot-1".to_string()],
            identity: None,
            status: WorkflowStatus::Applying,
            stage: WorkflowStage::CharacterTransferred,
            current_step: "transfer_character".to_string(),
            created_at_ms: 10,
            updated_at_ms: 20,
            error: None,
        };
        save_workflow(temp.path(), &workflow).expect("应能持久化工作流");
        workflow.status = WorkflowStatus::RecoveryRequired;
        workflow.updated_at_ms = 30;
        workflow.error = Some("写入失败".to_string());
        save_workflow(temp.path(), &workflow).expect("应能覆盖工作流状态");

        assert_eq!(load_workflow(temp.path(), &workflow.id).unwrap(), workflow);
        assert_eq!(list_workflows(temp.path()).unwrap(), vec![workflow]);
    }

    #[test]
    fn committing_an_applying_snapshot_updates_manifest_and_runs_retention() {
        let temp = TestDir::new("snapshot-state");
        let world = temp.path().join("world");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("Level.sav"), b"level").unwrap();
        let affected = [PathBuf::from("Level.sav")];
        for _ in 0..3 {
            create_snapshot(
                temp.path(),
                &world,
                "world-state",
                "状态世界",
                WorldClass::Server,
                "workflow-state",
                "test_state",
                &affected,
                BackupState::Committed,
            )
            .unwrap();
        }
        let applying = create_snapshot(
            temp.path(),
            &world,
            "world-state",
            "状态世界",
            WorldClass::Server,
            "workflow-state",
            "test_state",
            &affected,
            BackupState::Applying,
        )
        .unwrap();

        let updated = update_snapshot_state(temp.path(), &applying.id, BackupState::Committed)
            .expect("应能提交快照状态");

        assert_eq!(updated.state, BackupState::Committed);
        let snapshots = list_snapshots(temp.path(), Some("world-state")).unwrap();
        assert_eq!(snapshots.len(), 3);
        assert!(snapshots.iter().any(|snapshot| snapshot.id == applying.id));
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "palworld-backup-service-{label}-{}-{}",
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
