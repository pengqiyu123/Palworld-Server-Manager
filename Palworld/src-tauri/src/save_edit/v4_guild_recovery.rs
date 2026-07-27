#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuildPatchResult {
    pub changed: usize,
    pub source_was_admin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OriginalGuildIdentity {
    pub group_id: Guid,
    pub was_admin: bool,
}

#[derive(Debug, Clone)]
pub struct GuildRecoveryIdentity {
    pub source_player_uid: Guid,
    pub source_instance_id: Guid,
    pub source_group_id: Guid,
    pub source_was_admin: bool,
    pub target_player_uid: Guid,
    pub target_instance_id: Guid,
}

#[derive(Debug, Clone)]
pub struct GuildRecoveryCandidate {
    pub target_level: GvasFile,
    pub target_player: GvasFile,
    pub changed_references: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreOriginalGuildRequest {
    pub request_id: String,
    pub workflow_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreOriginalGuildOutcome {
    pub workflow: MigrationWorkflow,
    pub snapshot: BackupManifest,
    pub changed_references: usize,
}

struct GuildCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> GuildCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn take(&mut self, len: usize) -> Result<usize, String> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| "公会数据位置溢出".to_string())?;
        if end > self.bytes.len() {
            return Err("公会数据截断".to_string());
        }
        let start = self.pos;
        self.pos = end;
        Ok(start)
    }

    fn i32(&mut self) -> Result<i32, String> {
        let start = self.take(4)?;
        Ok(i32::from_le_bytes(
            self.bytes[start..start + 4].try_into().unwrap(),
        ))
    }

    fn count(&mut self, label: &str) -> Result<usize, String> {
        let count = self.i32()?;
        if count < 0 {
            return Err(format!("公会 {label} 数量无效"));
        }
        let count = count as usize;
        if count > self.bytes.len().saturating_sub(self.pos) {
            return Err(format!("公会 {label} 数量异常"));
        }
        Ok(count)
    }

    fn fstring(&mut self) -> Result<(), String> {
        let length = self.i32()?;
        let bytes = if length >= 0 {
            length as usize
        } else {
            (length as i64)
                .checked_neg()
                .and_then(|value| value.checked_mul(2))
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| "公会字符串长度溢出".to_string())?
        };
        self.take(bytes)?;
        Ok(())
    }

    fn guid(&mut self) -> Result<usize, String> {
        self.take(16)
    }
    fn guid_at(&self, offset: usize) -> [u8; 16] {
        self.bytes[offset..offset + 16].try_into().unwrap()
    }
    fn guid_array(&mut self, label: &str) -> Result<(), String> {
        let count = self.count(label)?;
        self.take(
            count
                .checked_mul(16)
                .ok_or_else(|| format!("{label} 长度溢出"))?,
        )?;
        Ok(())
    }
}

pub fn patch_original_guild_rawdata(
    bytes: &mut Vec<u8>,
    source_uid: &[u8; 16],
    target_uid: &[u8; 16],
    source_instance: &[u8; 16],
    target_instance: &[u8; 16],
    restore_admin: bool,
) -> Result<GuildPatchResult, String> {
    if source_uid == target_uid || source_instance == target_instance {
        return Err("原角色与目标角色身份相同，无需恢复公会".to_string());
    }
    let mut cursor = GuildCursor::new(bytes);
    let mut writes = Vec::<(usize, [u8; 16])>::new();
    cursor.guid()?;
    cursor.fstring()?;
    let handles = cursor.count("角色句柄")?;
    let mut found_source_handle = false;
    let mut found_target_handle = false;
    for _ in 0..handles {
        let uid_offset = cursor.guid()?;
        let instance_offset = cursor.guid()?;
        let uid = cursor.guid_at(uid_offset);
        let instance = cursor.guid_at(instance_offset);
        if uid == *source_uid || instance == *source_instance {
            found_source_handle = true;
            writes.push((uid_offset, *target_uid));
            writes.push((instance_offset, *target_instance));
        } else if uid == *target_uid || instance == *target_instance {
            found_target_handle = true;
        }
    }
    if !found_source_handle {
        return Err("原公会中找不到单机主角色句柄".to_string());
    }
    if found_target_handle {
        return Err("目标角色已存在于原公会句柄中，拒绝生成重复成员".to_string());
    }
    cursor.take(1)?;
    cursor.take(4)?;
    cursor.guid_array("据点")?;
    cursor.take(8)?;
    cursor.guid_array("据点对象")?;
    cursor.fstring()?;
    let last_modifier = cursor.guid()?;
    replace_uid_at(&cursor, last_modifier, source_uid, target_uid, &mut writes);
    let markers = cursor.count("公会标记")?;
    for _ in 0..markers {
        cursor.guid()?;
        cursor.take(24)?;
        cursor.take(4)?;
        let owner = cursor.guid()?;
        replace_uid_at(&cursor, owner, source_uid, target_uid, &mut writes);
    }
    let tail_start = cursor.pos;
    let (tail_writes, source_was_admin) = parse_tail_directional(
        bytes,
        tail_start,
        source_uid,
        target_uid,
        restore_admin,
        true,
    )
    .or_else(|_| {
        parse_tail_directional(
            bytes,
            tail_start,
            source_uid,
            target_uid,
            restore_admin,
            false,
        )
    })?;
    writes.extend(tail_writes);
    for (offset, value) in &writes {
        bytes[*offset..*offset + 16].copy_from_slice(value);
    }
    Ok(GuildPatchResult {
        changed: writes.len(),
        source_was_admin,
    })
}

pub fn validate_target_group(
    target_group: &[u8; 16],
    source_group: &[u8; 16],
) -> Result<(), String> {
    if *target_group != [0; 16] && target_group != source_group {
        return Err("目标角色已属于其他公会，无法恢复原公会".to_string());
    }
    Ok(())
}

pub fn inspect_original_guild_admin(
    level: &GvasFile,
    group_id: Guid,
    source_uid: Guid,
    source_instance: Guid,
) -> Result<bool, String> {
    if group_id.to_u8() == [0; 16] {
        return Ok(false);
    }
    let mut candidate = level.clone();
    let raw = guild_rawdata_mut(&mut candidate, group_id)?;
    let result = patch_original_guild_rawdata(
        raw,
        &source_uid.to_u8(),
        &[0xFE; 16],
        &source_instance.to_u8(),
        &[0xFD; 16],
        false,
    )?;
    Ok(result.source_was_admin)
}

pub fn inspect_original_guild_identity(
    level: &GvasFile,
    declared_group_id: Guid,
    source_uid: Guid,
    source_instance: Guid,
) -> Result<OriginalGuildIdentity, String> {
    if declared_group_id.to_u8() != [0; 16] {
        return Ok(OriginalGuildIdentity {
            group_id: declared_group_id,
            was_admin: inspect_original_guild_admin(
                level,
                declared_group_id,
                source_uid,
                source_instance,
            )?,
        });
    }

    let mut candidate = level.clone();
    let world = sav_io::top_field_mut(&mut candidate, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .ok_or_else(|| "Level.sav 缺少 worldSaveData".to_string())?;
    let groups = match world
        .get_mut("GroupSaveDataMap")
        .and_then(|values| values.first_mut())
    {
        Some(Property::MapProperty(MapProperty::Properties { value, .. })) => value,
        _ => return Err("Level.sav 缺少 GroupSaveDataMap".to_string()),
    };
    let mut matches = Vec::new();
    for (key, group) in groups.iter_mut() {
        let Some(group_id) = property_guid(key) else {
            continue;
        };
        let Some(fields) = property_fields_mut(group) else {
            continue;
        };
        let Some(Property::ArrayProperty(ArrayProperty::Bytes { bytes })) = fields
            .get_mut("RawData")
            .and_then(|values| values.first_mut())
        else {
            continue;
        };
        if guild_contains_handle(bytes, source_uid, source_instance).unwrap_or(false) {
            matches.push((group_id, bytes.clone()));
        }
    }
    if matches.len() != 1 {
        return Err(format!(
            "无法从 Level.sav 唯一识别原单机主角色所属公会，匹配到 {} 个",
            matches.len()
        ));
    }
    let (group_id, mut raw) = matches.pop().unwrap();
    let result = patch_original_guild_rawdata(
        &mut raw,
        &source_uid.to_u8(),
        &[0xFE; 16],
        &source_instance.to_u8(),
        &[0xFD; 16],
        false,
    )?;
    Ok(OriginalGuildIdentity {
        group_id,
        was_admin: result.source_was_admin,
    })
}

fn guild_contains_handle(
    bytes: &[u8],
    source_uid: Guid,
    source_instance: Guid,
) -> Result<bool, String> {
    let mut cursor = GuildCursor::new(bytes);
    cursor.guid()?;
    cursor.fstring()?;
    let handles = cursor.count("角色句柄")?;
    for _ in 0..handles {
        let uid_offset = cursor.guid()?;
        let instance_offset = cursor.guid()?;
        if cursor.guid_at(uid_offset) == source_uid.to_u8()
            && cursor.guid_at(instance_offset) == source_instance.to_u8()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn build_original_guild_recovery(
    target_level: &GvasFile,
    target_player: &GvasFile,
    identity: &GuildRecoveryIdentity,
) -> Result<GuildRecoveryCandidate, String> {
    let target_identity = describe_player_identity(target_player, "目标角色")?;
    if target_identity.player_uid != identity.target_player_uid
        || target_identity.instance_id != identity.target_instance_id
    {
        return Err("目标玩家文件与迁移工作流记录的角色身份不一致".to_string());
    }
    validate_target_group(
        &target_identity.group_id.to_u8(),
        &identity.source_group_id.to_u8(),
    )?;
    if identity.source_group_id.to_u8() == [0; 16] {
        return Err("原单机主角色没有可恢复的公会".to_string());
    }

    let mut level = target_level.clone();
    let raw = guild_rawdata_mut(&mut level, identity.source_group_id)?;
    let patched = patch_original_guild_rawdata(
        raw,
        &identity.source_player_uid.to_u8(),
        &identity.target_player_uid.to_u8(),
        &identity.source_instance_id.to_u8(),
        &identity.target_instance_id.to_u8(),
        identity.source_was_admin,
    )?;
    if patched.source_was_admin != identity.source_was_admin {
        return Err("原公会管理员状态与迁移工作流记录不一致".to_string());
    }
    let mut player = target_player.clone();
    set_player_group_id(&mut player, identity.source_group_id)?;
    Ok(GuildRecoveryCandidate {
        target_level: level,
        target_player: player,
        changed_references: patched.changed + 1,
    })
}

pub fn restore_original_guild_on_disk(
    backup_root: &Path,
    workflow_id: &str,
) -> Result<RestoreOriginalGuildOutcome, String> {
    let mut workflow = backup_service::load_workflow(backup_root, workflow_id)?;
    if workflow.stage != WorkflowStage::CharacterTransferred {
        return Err("只能在完整角色转移成功后恢复原公会".to_string());
    }
    let recorded = workflow
        .identity
        .clone()
        .ok_or_else(|| "工作流缺少后端记录的角色身份，无法恢复原公会".to_string())?;
    let target_world = PathBuf::from(&workflow.target_world_path);
    let target_player_relative =
        PathBuf::from("Players").join(format!("{}.sav", recorded.target_player_file));
    let level_sav = SavFile::load(&target_world.join("Level.sav"))?;
    let player_sav = SavFile::load(&target_world.join(&target_player_relative))?;
    let level = level_sav.parse()?;
    let player = player_sav.parse()?;
    let identity = GuildRecoveryIdentity {
        source_player_uid: sav_io::parse_guid(&recorded.source_player_uid)?,
        source_instance_id: sav_io::parse_guid(&recorded.source_instance_id)?,
        source_group_id: sav_io::parse_guid(&recorded.source_group_id)?,
        source_was_admin: recorded.source_was_guild_admin,
        target_player_uid: sav_io::parse_guid(&recorded.target_player_uid)?,
        target_instance_id: sav_io::parse_guid(&recorded.target_instance_id)?,
    };
    let candidate = build_original_guild_recovery(&level, &player, &identity)?;
    let level_bytes =
        SavFile::from_gvas(&candidate.target_level, level_sav.compression)?.to_bytes()?;
    let player_bytes =
        SavFile::from_gvas(&candidate.target_player, player_sav.compression)?.to_bytes()?;
    crate::save_edit::v4_character_operation::validate_candidate_file(
        &target_world,
        Path::new("Level.sav"),
        &level_bytes,
    )?;
    crate::save_edit::v4_character_operation::validate_candidate_file(
        &target_world,
        &target_player_relative,
        &player_bytes,
    )?;

    let affected = vec![PathBuf::from("Level.sav"), target_player_relative.clone()];
    let world_name = target_world
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("服务器世界");
    let snapshot = backup_service::create_snapshot(
        backup_root,
        &target_world,
        &workflow.world_id,
        world_name,
        WorldClass::Server,
        workflow_id,
        "guild_recovery",
        &affected,
        BackupState::Applying,
    )?;
    workflow.snapshot_ids.push(snapshot.id.clone());
    workflow.status = WorkflowStatus::Applying;
    workflow.current_step = "guild_recovery".to_string();
    workflow.updated_at_ms = unix_time_ms();
    backup_service::save_workflow(backup_root, &workflow)?;
    let mutations = [
        FileMutation {
            relative_path: PathBuf::from("Level.sav"),
            content: Some(level_bytes),
        },
        FileMutation {
            relative_path: target_player_relative,
            content: Some(player_bytes),
        },
    ];
    if let Err(write_error) = atomic_write::commit_file_set(&target_world, &mutations) {
        let restore = backup_service::restore_snapshot(backup_root, &snapshot.id, &target_world);
        workflow.updated_at_ms = unix_time_ms();
        workflow.error = Some(write_error.clone());
        match restore {
            Ok(()) => {
                workflow.status = WorkflowStatus::RolledBack;
                workflow.current_step = "guild_recovery_rolled_back".to_string();
                let _ = backup_service::save_workflow(backup_root, &workflow);
                return Err(format!("公会恢复失败，已恢复到操作前状态: {write_error}"));
            }
            Err(restore_error) => {
                workflow.status = WorkflowStatus::RecoveryRequired;
                workflow.current_step = "guild_recovery_recovery_required".to_string();
                workflow.error = Some(format!("{write_error}; {restore_error}"));
                let _ = backup_service::update_snapshot_state(
                    backup_root,
                    &snapshot.id,
                    BackupState::RecoveryRequired,
                );
                let _ = backup_service::save_workflow(backup_root, &workflow);
                return Err(format!(
                    "公会恢复失败且自动恢复失败，请勿启动服务器: {write_error}; {restore_error}"
                ));
            }
        }
    }
    let snapshot =
        backup_service::update_snapshot_state(backup_root, &snapshot.id, BackupState::Committed)?;
    workflow.status = WorkflowStatus::AwaitingVerification;
    workflow.stage = WorkflowStage::GuildRestored;
    workflow.current_step = "guild_restored".to_string();
    workflow.updated_at_ms = unix_time_ms();
    workflow.error = None;
    backup_service::save_workflow(backup_root, &workflow)?;
    let _ = backup_service::rebuild_index(backup_root);
    Ok(RestoreOriginalGuildOutcome {
        workflow,
        snapshot,
        changed_references: candidate.changed_references,
    })
}

#[tauri::command]
pub async fn restore_original_guild_v4(
    req: RestoreOriginalGuildRequest,
    state: State<'_, ServerState>,
    app: tauri::AppHandle,
) -> Result<RestoreOriginalGuildOutcome, String> {
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
    emit_progress(&app, &req.request_id, "writing_save", "正在恢复原公会")?;
    let outcome = restore_original_guild_on_disk(&backup_root, &req.workflow_id)?;
    emit_progress(&app, &req.request_id, "checking_result", "正在检查结果")?;
    emit_progress(&app, &req.request_id, "completed", "原公会恢复完成")?;
    Ok(outcome)
}

fn guild_rawdata_mut(level: &mut GvasFile, group_id: Guid) -> Result<&mut Vec<u8>, String> {
    let world = sav_io::top_field_mut(level, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .ok_or_else(|| "Level.sav 缺少 worldSaveData".to_string())?;
    let map = match world
        .get_mut("GroupSaveDataMap")
        .and_then(|values| values.first_mut())
    {
        Some(Property::MapProperty(MapProperty::Properties { value, .. })) => value,
        _ => return Err("Level.sav 缺少 GroupSaveDataMap".to_string()),
    };
    let (_, group) = map
        .iter_mut()
        .find(|(key, _)| property_guid(key) == Some(group_id))
        .ok_or_else(|| "找不到原单机主角色所属公会".to_string())?;
    let fields = property_fields_mut(group).ok_or_else(|| "公会记录结构无效".to_string())?;
    match fields
        .get_mut("RawData")
        .and_then(|values| values.first_mut())
    {
        Some(Property::ArrayProperty(ArrayProperty::Bytes { bytes })) => Ok(bytes),
        _ => Err("公会记录缺少 RawData".to_string()),
    }
}

fn set_player_group_id(player: &mut GvasFile, group_id: Guid) -> Result<(), String> {
    let fields = player
        .properties
        .get_mut("SaveData")
        .and_then(property_fields_mut)
        .ok_or_else(|| "目标玩家文件缺少 SaveData".to_string())?;
    if let Some(property) = fields
        .get_mut("GroupId")
        .and_then(|values| values.first_mut())
    {
        return set_guid(property, group_id);
    }
    fields.insert(
        "GroupId".to_string(),
        vec![StructProperty::new(
            Guid::from_u8([0; 16]),
            "Guid".to_string(),
            StructPropertyValue::Guid(group_id),
        )
        .into()],
    );
    Ok(())
}

fn property_fields_mut(
    property: &mut Property,
) -> Option<&mut gvas::types::map::HashableIndexMap<String, Vec<Property>>> {
    match property {
        Property::StructProperty(value) => match &mut value.value {
            StructPropertyValue::CustomStruct(fields) => Some(fields),
            _ => None,
        },
        Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields)) => Some(fields),
        _ => None,
    }
}

fn property_guid(property: &Property) -> Option<Guid> {
    match property {
        Property::StructProperty(value) => match value.value {
            StructPropertyValue::Guid(guid) => Some(guid),
            _ => None,
        },
        Property::StructPropertyValue(StructPropertyValue::Guid(guid)) => Some(*guid),
        _ => None,
    }
}

fn set_guid(property: &mut Property, group_id: Guid) -> Result<(), String> {
    match property {
        Property::StructProperty(value) => match &mut value.value {
            StructPropertyValue::Guid(guid) => {
                *guid = group_id;
                Ok(())
            }
            _ => Err("GroupId 不是 GUID".to_string()),
        },
        Property::StructPropertyValue(StructPropertyValue::Guid(guid)) => {
            *guid = group_id;
            Ok(())
        }
        _ => Err("GroupId 不是 GUID".to_string()),
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

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn parse_tail_directional(
    bytes: &[u8],
    start: usize,
    source_uid: &[u8; 16],
    target_uid: &[u8; 16],
    restore_admin: bool,
    version_two: bool,
) -> Result<(Vec<(usize, [u8; 16])>, bool), String> {
    let mut cursor = GuildCursor { bytes, pos: start };
    let mut writes = Vec::new();
    if version_two {
        let chest_roles = cursor.count("仓库角色")?;
        cursor.take(chest_roles)?;
        cursor.take(4)?;
    }
    let admin_offset = cursor.guid()?;
    let source_was_admin = cursor.guid_at(admin_offset) == *source_uid;
    if restore_admin && source_was_admin {
        writes.push((admin_offset, *target_uid));
    }
    let members = cursor.count("成员")?;
    let mut found_source = false;
    let mut found_target = false;
    for _ in 0..members {
        let uid_offset = cursor.guid()?;
        let uid = cursor.guid_at(uid_offset);
        if uid == *source_uid {
            found_source = true;
            writes.push((uid_offset, *target_uid));
        } else if uid == *target_uid {
            found_target = true;
        }
        cursor.take(8)?;
        cursor.fstring()?;
        if version_two {
            cursor.take(1)?;
        }
    }
    if !found_source {
        return Err("原公会成员列表中找不到单机主角色".to_string());
    }
    if found_target {
        return Err("目标角色已在原公会成员列表中，拒绝生成重复成员".to_string());
    }
    if version_two {
        let role_permissions = cursor.count("角色权限")?;
        for _ in 0..role_permissions {
            cursor.take(1)?;
            let permissions = cursor.count("权限")?;
            cursor.take(permissions)?;
        }
    }
    cursor.take(4)?;
    if cursor.pos != bytes.len() {
        return Err("公会数据尾部未完全解析".to_string());
    }
    Ok((writes, source_was_admin))
}

fn replace_uid_at(
    cursor: &GuildCursor<'_>,
    offset: usize,
    source_uid: &[u8; 16],
    target_uid: &[u8; 16],
    writes: &mut Vec<(usize, [u8; 16])>,
) {
    if cursor.guid_at(offset) == *source_uid {
        writes.push((offset, *target_uid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directional_guild_patch_rebinds_only_original_host_identity() {
        let source = [0x11; 16];
        let target = [0x22; 16];
        let other = [0x55; 16];
        let source_instance = [0xA1; 16];
        let target_instance = [0xB2; 16];
        let other_instance = [0xC3; 16];
        let mut raw = Vec::new();

        push_guid(&mut raw, &[0x33; 16]);
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 2);
        let source_handle_uid = push_guid(&mut raw, &source);
        let source_handle_instance = push_guid(&mut raw, &source_instance);
        let other_handle_uid = push_guid(&mut raw, &other);
        let other_handle_instance_offset = push_guid(&mut raw, &other_instance);
        raw.push(0);
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 1);
        let base_id = push_guid(&mut raw, &[0x66; 16]);
        raw.extend_from_slice(&[0x77; 8]);
        push_i32(&mut raw, 1);
        let base_object_id = push_guid(&mut raw, &[0x88; 16]);
        push_i32(&mut raw, 0);
        let last_modifier = push_guid(&mut raw, &source);
        push_i32(&mut raw, 1);
        push_guid(&mut raw, &[0x44; 16]);
        raw.extend_from_slice(&[0x99; 24]);
        push_i32(&mut raw, 0);
        let marker_owner = push_guid(&mut raw, &source);
        let administrator = push_guid(&mut raw, &source);
        push_i32(&mut raw, 2);
        let source_member = push_guid(&mut raw, &source);
        raw.extend_from_slice(&[0; 8]);
        push_i32(&mut raw, 0);
        let other_member = push_guid(&mut raw, &other);
        raw.extend_from_slice(&[0; 8]);
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 0);
        let before = raw.clone();

        let result = patch_original_guild_rawdata(
            &mut raw,
            &source,
            &target,
            &source_instance,
            &target_instance,
            true,
        )
        .expect("应能按单向规则恢复原公会");

        assert!(result.source_was_admin);
        assert_eq!(guid_at(&raw, source_handle_uid), target);
        assert_eq!(guid_at(&raw, source_handle_instance), target_instance);
        assert_eq!(guid_at(&raw, last_modifier), target);
        assert_eq!(guid_at(&raw, marker_owner), target);
        assert_eq!(guid_at(&raw, administrator), target);
        assert_eq!(guid_at(&raw, source_member), target);
        for (offset, length) in [
            (other_handle_uid, 16),
            (other_handle_instance_offset, 16),
            (base_id, 16),
            (base_object_id, 16),
            (other_member, 16),
        ] {
            assert_eq!(
                &raw[offset..offset + length],
                &before[offset..offset + length]
            );
        }
    }

    #[test]
    fn admin_is_not_granted_when_original_host_was_not_admin() {
        let source = [0x11; 16];
        let target = [0x22; 16];
        let admin = [0x55; 16];
        let mut raw = minimal_guild_raw(source, [0xA1; 16], admin);

        let result = patch_original_guild_rawdata(
            &mut raw,
            &source,
            &target,
            &[0xA1; 16],
            &[0xB2; 16],
            false,
        )
        .expect("非管理员主角色仍应能恢复成员和句柄");

        assert!(!result.source_was_admin);
        assert!(raw.windows(16).any(|window| window == admin));
    }

    #[test]
    fn target_character_in_another_guild_is_rejected() {
        let source_group = [0x33; 16];
        assert!(validate_target_group(&[0; 16], &source_group).is_ok());
        assert!(validate_target_group(&source_group, &source_group).is_ok());
        let error = validate_target_group(&[0x44; 16], &source_group)
            .expect_err("目标角色属于其他公会时必须拒绝恢复");
        assert!(error.contains("其他公会"));
    }

    fn minimal_guild_raw(member: [u8; 16], instance: [u8; 16], admin: [u8; 16]) -> Vec<u8> {
        let mut raw = Vec::new();
        push_guid(&mut raw, &[0x33; 16]);
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 1);
        push_guid(&mut raw, &member);
        push_guid(&mut raw, &instance);
        raw.push(0);
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 0);
        raw.extend_from_slice(&[0; 8]);
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 0);
        push_guid(&mut raw, &member);
        push_i32(&mut raw, 0);
        push_guid(&mut raw, &admin);
        push_i32(&mut raw, 1);
        push_guid(&mut raw, &member);
        raw.extend_from_slice(&[0; 8]);
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 0);
        raw
    }

    fn push_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn push_guid(bytes: &mut Vec<u8>, value: &[u8; 16]) -> usize {
        let offset = bytes.len();
        bytes.extend_from_slice(value);
        offset
    }
    fn guid_at(bytes: &[u8], offset: usize) -> [u8; 16] {
        bytes[offset..offset + 16].try_into().unwrap()
    }
}
use std::path::{Path, PathBuf};

use gvas::properties::array_property::ArrayProperty;
use gvas::properties::map_property::MapProperty;
use gvas::properties::struct_property::{StructProperty, StructPropertyValue};
use gvas::properties::Property;
use gvas::types::Guid;
use gvas::GvasFile;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::backup_service::{
    self, BackupManifest, BackupState, MigrationWorkflow, WorkflowStage, WorkflowStatus, WorldClass,
};
use crate::save_edit::atomic_write::{self, FileMutation};
use crate::save_edit::sav_io::{self, SavFile};
use crate::save_edit::v4_full_character_transfer::describe_player_identity;
use crate::server::ServerState;
