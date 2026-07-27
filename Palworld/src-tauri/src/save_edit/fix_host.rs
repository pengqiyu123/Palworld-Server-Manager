//! F5 · T03 Fix Host Save（U01 灵魂步骤）—— 双向交换重写。
//!
//! 当单机存档迁移到专用服务器后，原单机主机的角色 UID 需要与原专用服中
//! 新角色的 UID 做**对称交换**，专用服才能正确识别两玩家的公会 / 帕鲁 / 建筑等引用。
//!
//! 本实现对齐参考工具 Fix Host 的结果语义：
//!   1. 结构化解析 `Level.sav`，交换 GVAS Guid、角色 RawData 和公会 RawData 中的身份引用。
//!   2. 两个 `Players/<guid>.sav` 分别结构化改写内部 UID。
//!   3. `_dps.sav`（每个玩家一个）：设置 `OwnerPlayerUId`，并**显式赋值**
//!      `SlotId.ContainerId.ID = 对方玩家的 PalStorageContainerId`（容器 ID 不是 UID，
//!      不能互换，须按参考 `copy_dps_file` 设值）。解析失败时中止，不做裸字节降级。
//!   4. **最后交换文件名** `<old>.sav ↔ <new>.sav`（`<old>_dps.sav ↔ <new>_dps.sav` 由
//!      第 3 步直接写入对端文件名完成），保证「文件名 = 身份」一致。
//!   5. 回写经现有 `sav_io::SavFile::save()`（PlM 自动降级 PLZ），并回读校验。

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use gvas::properties::array_property::ArrayProperty;
use gvas::properties::map_property::MapProperty;
use gvas::properties::struct_property::StructPropertyValue;
use gvas::properties::Property;
use gvas::types::map::HashableIndexMap;
use gvas::types::Guid;
use gvas::GvasFile;

use crate::save_edit::models::{FixHostRequest, UidMapping};
use crate::save_edit::path_util;
use crate::save_edit::sav_io::{self, SavFile};
#[cfg(test)]
use crate::save_edit::world_copy;

/// 为 `_dps` 临时目录生成唯一序号（避免并发测试互相覆盖）。
static DPS_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// 判断原始字节是否包含某 GUID 的 16 字节。
#[cfg(test)]
fn contains_guid(raw: &[u8], g: &[u8; 16]) -> bool {
    let mut i = 0;
    let n = raw.len();
    while i + 16 <= n {
        if raw[i..i + 16] == *g {
            return true;
        }
        i += 1;
    }
    false
}

/// 归一化 UID 字符串为磁盘文件名 stem：去连字符/空格、转小写。
///
/// Palworld 磁盘文件名使用 registry 32-hex；调用方传入的 UID 已是该格式，直接规范化即可。
fn normalize_uid(s: &str) -> String {
    s.replace(['-', ' '], "").to_lowercase()
}

/// 把 `Property`（须为 `type_name == "Guid"` 的 StructProperty）的值设为 `value`。
/// 返回是否命中并改写。
fn set_guid_property(prop: &mut Property, value: &[u8; 16]) -> bool {
    if let Property::StructProperty(s) = prop {
        if s.type_name == "Guid" {
            if let StructPropertyValue::Guid(g) = &mut s.value {
                *g = Guid::from_u8(*value);
                return true;
            }
        }
    }
    false
}

/// 从玩家存档的结构化 GVAS 中提取 `PalStorageContainerId.ID` 与 `IndividualId.InstanceId`
/// （均为 16 字节 GUID）。解析失败或字段缺失时对应返回 `None`。
///
/// - `PalStorageContainerId.ID`：用于 `_dps` 容器 ID 显式赋值（对方玩家的容器）。
/// - `IndividualId.InstanceId`：用于 R-INST-1 防御校验（两角色 InstanceId 不应相同）。
fn extract_player_uids(gvas: &GvasFile) -> (Option<[u8; 16]>, Option<[u8; 16]>) {
    let mut container = None;
    let mut instance = None;
    if let Some(save_data) = sav_io::top_field(gvas, "SaveData") {
        if let Some(csv) = sav_io::struct_value(save_data) {
            if let Some(map) = sav_io::custom_fields(csv) {
                // PalStorageContainerId.ID
                if let Some(psc) = sav_io::field(map, "PalStorageContainerId") {
                    if let Some(psc_csv) = sav_io::struct_value(psc) {
                        if let Some(psc_map) = sav_io::custom_fields(psc_csv) {
                            if let Some(id_prop) = sav_io::field(psc_map, "ID") {
                                if let Some(g) = sav_io::as_guid(id_prop) {
                                    container = Some(g.to_u8());
                                }
                            }
                        }
                    }
                }
                // IndividualId.InstanceId
                if let Some(iid) = sav_io::field(map, "IndividualId") {
                    if let Some(iid_csv) = sav_io::struct_value(iid) {
                        if let Some(iid_map) = sav_io::custom_fields(iid_csv) {
                            if let Some(inst_prop) = sav_io::field(iid_map, "InstanceId") {
                                if let Some(g) = sav_io::as_guid(inst_prop) {
                                    instance = Some(g.to_u8());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (container, instance)
}

/// 在结构化解析后的 `_dps` GVAS 中给每个 Pal 条目：
/// 1. `OwnerPlayerUId`（Guid）→ `target_uid`；
/// 2. `SlotId.ContainerId.ID`（Guid）→ `target_container`（若提供）。
///
/// 返回实际更新的字段数（用于断言 / 调试）。
fn patch_dps(
    gvas: &mut GvasFile,
    target_uid: &[u8; 16],
    target_container: Option<&[u8; 16]>,
) -> usize {
    let mut updated = 0usize;
    if let Some(prop) = sav_io::top_field_mut(gvas, "SaveParameterArray") {
        if let Some(structs) = sav_io::as_struct_array_mut(prop) {
            for spv in structs.iter_mut() {
                // 每个元素应为 CustomStruct（Pal 条目）
                if let Some(map) = sav_io::custom_fields_mut(spv) {
                    // SaveParameter -> CustomStruct
                    if let Some(save_param) = sav_io::field_mut(map, "SaveParameter") {
                        if let Some(inner) = sav_io::struct_value_mut(save_param) {
                            if let Some(inner_map) = sav_io::custom_fields_mut(inner) {
                                // OwnerPlayerUId (Guid) -> target_uid
                                if let Some(opu) = sav_io::field_mut(inner_map, "OwnerPlayerUId") {
                                    if set_guid_property(opu, target_uid) {
                                        updated += 1;
                                    }
                                }
                                // SlotId -> ContainerId -> ID (Guid) -> target_container
                                if let Some(slot) = sav_io::field_mut(inner_map, "SlotId") {
                                    if let Some(slot_csv) = sav_io::struct_value_mut(slot) {
                                        if let Some(slot_map) = sav_io::custom_fields_mut(slot_csv)
                                        {
                                            if let Some(cid) =
                                                sav_io::field_mut(slot_map, "ContainerId")
                                            {
                                                if let Some(cid_csv) = sav_io::struct_value_mut(cid)
                                                {
                                                    if let Some(cid_map) =
                                                        sav_io::custom_fields_mut(cid_csv)
                                                    {
                                                        if let Some(id_prop) =
                                                            sav_io::field_mut(cid_map, "ID")
                                                        {
                                                            if let Some(c) = target_container {
                                                                if set_guid_property(id_prop, c) {
                                                                    updated += 1;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    updated
}

/// 处理单个 `_dps.sav`：交换 `OwnerPlayerUId` 并（结构化解析可行时）显式赋值
/// `SlotId.ContainerId.ID = 对方 PalStorageContainerId`。
///
/// - `src`：原始 `_dps` 文件路径（已拷到临时目录，避免被覆盖）。
/// - `dst`：目标路径（交换后的文件名）。
/// - `target_uid`：写入 `OwnerPlayerUId` 的目标 16 字节 GUID。
/// - `target_container`：对方玩家的 `PalStorageContainerId`（None 时降级，不设置）。
///
/// 返回 `true` 表示缺少容器 ID；GVAS 解析失败会直接返回错误。
fn patch_dps_file(
    src: &Path,
    dst: &Path,
    target_uid: &[u8; 16],
    target_container: Option<&[u8; 16]>,
) -> Result<bool, String> {
    let file = SavFile::load(src)?;
    match file.parse() {
        Ok(mut gvas) => {
            let _updated = patch_dps(&mut gvas, target_uid, target_container);
            let patched = SavFile::from_gvas(&gvas, file.compression)
                .map_err(|e| format!("{} _dps 重新序列化失败: {}", src.display(), e))?;
            patched.save(dst)?;
            SavFile::load(dst)
                .map_err(|e| format!("{} _dps 写回校验失败: {}", dst.display(), e))?;
            // 若无法得知对方容器 ID，即便结构化成功也标记降级（ContainerId 未动）。
            if target_container.is_none() {
                eprintln!(
                    "[warn] _dps {}: 缺少对方 PalStorageContainerId，未设置 ContainerId.ID（R-DPS-1 降级）。",
                    dst.display()
                );
                return Ok(true);
            }
            Ok(false)
        }
        Err(e) => Err(format!(
            "{} _dps 解析失败，已取消身份交换以避免损坏存档: {}",
            src.display(),
            e
        )),
    }
}

fn custom_fields_mut(
    property: &mut Property,
) -> Option<&mut HashableIndexMap<String, Vec<Property>>> {
    match property {
        Property::StructProperty(value) => match &mut value.value {
            StructPropertyValue::CustomStruct(fields) => Some(fields),
            _ => None,
        },
        Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields)) => Some(fields),
        _ => None,
    }
}

fn rawdata_bytes_mut(property: &mut Property) -> Option<&mut Vec<u8>> {
    let fields = custom_fields_mut(property)?;
    match fields.get_mut("RawData")?.first_mut()? {
        Property::ArrayProperty(ArrayProperty::Bytes { bytes }) => Some(bytes),
        _ => None,
    }
}

fn is_guild_group(property: &Property) -> bool {
    let fields = match property {
        Property::StructProperty(value) => match &value.value {
            StructPropertyValue::CustomStruct(fields) => fields,
            _ => return false,
        },
        Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields)) => fields,
        _ => return false,
    };
    matches!(
        fields.get("GroupType").and_then(|values| values.first()),
        Some(Property::EnumProperty(value)) if value.value == "EPalGroupType::Guild"
    )
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
            .ok_or_else(|| "公会 RawData 位置溢出".to_string())?;
        if end > self.bytes.len() {
            return Err(format!(
                "公会 RawData 截断: 需要 {} 字节，剩余 {} 字节",
                len,
                self.bytes.len().saturating_sub(self.pos)
            ));
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
            return Err(format!("公会 RawData {label} 数量为负数: {count}"));
        }
        let count = count as usize;
        if count > self.bytes.len().saturating_sub(self.pos) {
            return Err(format!("公会 RawData {label} 数量异常: {count}"));
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
                .ok_or_else(|| "公会 RawData FString 长度溢出".to_string())?
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

fn add_guid_swap(
    cursor: &GuildCursor<'_>,
    offset: usize,
    old: &[u8; 16],
    new: &[u8; 16],
    writes: &mut Vec<(usize, [u8; 16])>,
) {
    let value = cursor.guid_at(offset);
    if value == *old {
        writes.push((offset, *new));
    } else if value == *new {
        writes.push((offset, *old));
    }
}

fn parse_guild_player(
    cursor: &mut GuildCursor<'_>,
    old: &[u8; 16],
    new: &[u8; 16],
    has_role: bool,
    writes: &mut Vec<(usize, [u8; 16])>,
) -> Result<(), String> {
    let uid = cursor.guid()?;
    add_guid_swap(cursor, uid, old, new, writes);
    cursor.take(8)?;
    cursor.fstring()?;
    if has_role {
        cursor.take(1)?;
    }
    Ok(())
}

fn parse_guild_tail(
    bytes: &[u8],
    start: usize,
    old: &[u8; 16],
    new: &[u8; 16],
    version_two: bool,
) -> Result<Vec<(usize, [u8; 16])>, String> {
    let mut cursor = GuildCursor { bytes, pos: start };
    let mut writes = Vec::new();
    if version_two {
        let chest_roles = cursor.count("仓库角色")?;
        cursor.take(chest_roles)?;
        cursor.take(4)?;
    }
    let admin = cursor.guid()?;
    add_guid_swap(&cursor, admin, old, new, &mut writes);
    let players = cursor.count("成员")?;
    for _ in 0..players {
        parse_guild_player(&mut cursor, old, new, version_two, &mut writes)?;
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
        return Err(format!(
            "公会 RawData 尾部未完全消费: {} / {}",
            cursor.pos,
            bytes.len()
        ));
    }
    Ok(writes)
}

fn patch_guild_rawdata(
    bytes: &mut Vec<u8>,
    old: &[u8; 16],
    new: &[u8; 16],
    old_instance: Option<&[u8; 16]>,
    new_instance: Option<&[u8; 16]>,
) -> Result<usize, String> {
    let mut cursor = GuildCursor::new(bytes);
    let mut writes = Vec::<(usize, [u8; 16])>::new();
    cursor.guid()?;
    cursor.fstring()?;
    let handles = cursor.count("角色句柄")?;
    for _ in 0..handles {
        let uid = cursor.guid()?;
        let instance = cursor.guid()?;
        let instance_value = cursor.guid_at(instance);
        if old_instance == Some(&instance_value) {
            writes.push((uid, *new));
        } else if new_instance == Some(&instance_value) {
            writes.push((uid, *old));
        }
    }
    cursor.take(1)?;
    cursor.take(4)?;
    cursor.guid_array("据点")?;
    cursor.take(8)?;
    cursor.guid_array("据点对象")?;
    cursor.fstring()?;
    let last_modifier = cursor.guid()?;
    add_guid_swap(&cursor, last_modifier, old, new, &mut writes);
    let markers = cursor.count("公会标记")?;
    for _ in 0..markers {
        cursor.guid()?;
        cursor.take(24)?;
        cursor.take(4)?;
        let owner = cursor.guid()?;
        add_guid_swap(&cursor, owner, old, new, &mut writes);
    }
    let tail_start = cursor.pos;
    let tail_writes = parse_guild_tail(bytes, tail_start, old, new, true)
        .or_else(|_| parse_guild_tail(bytes, tail_start, old, new, false))?;
    writes.extend(tail_writes);

    for (offset, value) in &writes {
        bytes[*offset..*offset + 16].copy_from_slice(value);
    }
    Ok(writes.len())
}

fn patch_cspm_player_keys(
    entries: &mut HashableIndexMap<Property, Property>,
    old: &[u8; 16],
    new: &[u8; 16],
    old_instance: Option<&[u8; 16]>,
    new_instance: Option<&[u8; 16]>,
) -> usize {
    let original = std::mem::take(&mut entries.0);
    let mut changed = 0usize;
    for (mut key, value) in original {
        if let Some(fields) = custom_fields_mut(&mut key) {
            let instance = fields
                .get("InstanceId")
                .and_then(|values| values.first())
                .and_then(sav_io::as_guid)
                .map(|value| value.to_u8());
            let replacement = if instance.as_ref() == old_instance {
                Some(new)
            } else if instance.as_ref() == new_instance {
                Some(old)
            } else {
                None
            };
            if let Some(replacement) = replacement {
                if let Some(player_uid) = fields
                    .get_mut("PlayerUId")
                    .and_then(|values| values.first_mut())
                {
                    changed += usize::from(set_guid_property(player_uid, replacement));
                }
            }
        }
        entries.0.insert(key, value);
    }
    changed
}

fn patch_level_identity(
    level: &SavFile,
    old: &[u8; 16],
    new: &[u8; 16],
    old_instance: Option<&[u8; 16]>,
    new_instance: Option<&[u8; 16]>,
) -> Result<(SavFile, usize), String> {
    let mut gvas = level.parse()?;
    let custom_versions = gvas.header.get_custom_versions().clone();
    let mut changed = sav_io::swap_owner_guids_in_gvas(&mut gvas, old, new)?;
    let world_fields = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .ok_or_else(|| "Level.sav 缺少 worldSaveData".to_string())?;

    if let Some(Property::MapProperty(MapProperty::Properties { value, .. })) = world_fields
        .get_mut("CharacterSaveParameterMap")
        .and_then(|values| values.first_mut())
    {
        changed += patch_cspm_player_keys(value, old, new, old_instance, new_instance);
        for (_, entry) in value.0.iter_mut() {
            if let Some(bytes) = rawdata_bytes_mut(entry) {
                let (patched, count) = sav_io::swap_owner_guids_in_character_property_stream(
                    bytes,
                    &custom_versions,
                    old,
                    new,
                )?;
                if count > 0 {
                    *bytes = patched;
                    changed += count;
                }
            }
        }
    }

    if let Some(Property::MapProperty(MapProperty::Properties { value, .. })) = world_fields
        .get_mut("GroupSaveDataMap")
        .and_then(|values| values.first_mut())
    {
        for (_, group) in value.0.iter_mut() {
            if is_guild_group(group) {
                let bytes =
                    rawdata_bytes_mut(group).ok_or_else(|| "公会记录缺少 RawData".to_string())?;
                changed += patch_guild_rawdata(bytes, old, new, old_instance, new_instance)?;
            }
        }
    }

    let patched = SavFile::from_gvas(&gvas, level.compression)?;
    Ok((patched, changed))
}

/// 核心实现：在给定世界数据目录 `data_dir` 内执行双向交换。
///
/// `old_bytes` / `new_bytes` 为目标交换的两个 16 字节 GUID（磁盘序）。
/// 返回被成功改写并写回的文件数（Level + 两个 player + 可能存在的 _dps）。
///
/// ⚠️ T4 可见性调整：`pub(crate)`（仅放开可见性，T3 双向交换逻辑未改动），
/// 供 `save_edit.rs` 的 `run_fix_host_with_guard` 在任意临时 `data_dir` 上复用，
/// 以支持独立于 settings / 运行态的单元测试。
pub(crate) fn fix_host_save_in_dir(
    data_dir: &Path,
    old_bytes: &[u8; 16],
    new_bytes: &[u8; 16],
    old_uid: &str,
    new_uid: &str,
) -> Result<usize, String> {
    let players_dir = data_dir.join("Players");
    if !players_dir.is_dir() {
        return Err(format!("Players 目录不存在: {}", players_dir.display()));
    }
    if old_bytes == new_bytes {
        return Err("旧主机 GUID 与新角色 GUID 相同，无需替换".to_string());
    }

    let old_stem = normalize_uid(old_uid);
    let new_stem = normalize_uid(new_uid);
    let old_path = players_dir.join(format!("{}.sav", old_stem));
    let new_path = players_dir.join(format!("{}.sav", new_stem));
    if !old_path.is_file() {
        return Err(format!("旧主机角色存档不存在: {}", old_path.display()));
    }
    if !new_path.is_file() {
        return Err(format!("新角色存档不存在: {}", new_path.display()));
    }
    let level_path = data_dir.join("Level.sav");
    if !level_path.is_file() {
        return Err(format!("Level.sav 不存在: {}", level_path.display()));
    }

    let mut changed = 0usize;
    let mut degraded_dps = false;

    // 先完整解析并生成所有核心文件的修改结果；任一解析失败都在写盘前中止。
    let old_sav = SavFile::load(&old_path)?;
    let new_sav = SavFile::load(&new_path)?;
    let old_gvas = old_sav
        .parse()
        .map_err(|e| format!("旧主机角色存档解析失败: {e}"))?;
    let new_gvas = new_sav
        .parse()
        .map_err(|e| format!("服务器新角色存档解析失败: {e}"))?;
    let (old_container, old_inst) = extract_player_uids(&old_gvas);
    let (new_container, new_inst) = extract_player_uids(&new_gvas);
    // R-INST-1 防御：两角色 InstanceId 不应相同（否则 3-pass 交换会退化）。
    if let (Some(a), Some(b)) = (old_inst, new_inst) {
        if a == b {
            return Err("两个角色的 InstanceId 相同，无法安全交换（R-INST-1）".to_string());
        }
    }

    let level = SavFile::load(&level_path)?;
    let (patched_level, level_guid_changes) = patch_level_identity(
        &level,
        old_bytes,
        new_bytes,
        old_inst.as_ref(),
        new_inst.as_ref(),
    )?;
    if level_guid_changes == 0 {
        return Err("Level.sav 中未找到待交换的角色引用，已取消操作".to_string());
    }
    let mut patched_old = SavFile {
        raw: old_sav.raw.clone(),
        compression: old_sav.compression,
    };
    let old_changes = patched_old.replace_guid_structured(old_bytes, new_bytes)?;
    if old_changes == 0 {
        return Err("旧主机角色存档中未找到原角色 UID，已取消操作".to_string());
    }
    let mut patched_new = SavFile {
        raw: new_sav.raw.clone(),
        compression: new_sav.compression,
    };
    let new_changes = patched_new.replace_guid_structured(new_bytes, old_bytes)?;
    if new_changes == 0 {
        return Err("服务器新角色存档中未找到新角色 UID，已取消操作".to_string());
    }

    patched_level.save(&level_path)?;
    SavFile::load(&level_path).map_err(|e| format!("Level.sav 写回校验失败: {e}"))?;
    changed += 1;
    patched_old.save(&old_path)?;
    SavFile::load(&old_path).map_err(|e| format!("旧主机存档写回校验失败: {e}"))?;
    changed += 1;
    patched_new.save(&new_path)?;
    SavFile::load(&new_path).map_err(|e| format!("新角色存档写回校验失败: {e}"))?;
    changed += 1;

    // (c) _dps.sav：交换 OwnerPlayerUId + 显式设 ContainerId.ID = 对方 PalStorageContainerId。
    //     先把两个原始 _dps 拷到临时目录，避免「写 new_dps 时覆盖未读的 old_dps」。
    let old_dps_path = players_dir.join(format!("{}_dps.sav", old_stem));
    let new_dps_path = players_dir.join(format!("{}_dps.sav", new_stem));
    let tmp_dir = std::env::temp_dir().join(format!(
        "palworld_fixhost_dps_{}_{}",
        std::process::id(),
        DPS_TMP_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let old_dps_tmp = tmp_dir.join(format!("{}_dps.sav", old_stem));
    let new_dps_tmp = tmp_dir.join(format!("{}_dps.sav", new_stem));
    if old_dps_path.is_file() {
        std::fs::copy(&old_dps_path, &old_dps_tmp)
            .map_err(|e| format!("拷贝 {} 失败: {}", old_dps_path.display(), e))?;
    }
    if new_dps_path.is_file() {
        std::fs::copy(&new_dps_path, &new_dps_tmp)
            .map_err(|e| format!("拷贝 {} 失败: {}", new_dps_path.display(), e))?;
    }
    // old_dps 内容 → 写入 new_dps_path（OwnerPlayerUId=old→new，ContainerId=new_container）
    if old_dps_tmp.is_file() {
        let deg = patch_dps_file(
            &old_dps_tmp,
            &new_dps_path,
            new_bytes,
            new_container.as_ref(),
        )?;
        if deg {
            degraded_dps = true;
        }
        changed += 1;
    }
    // new_dps 内容 → 写入 old_dps_path（OwnerPlayerUId=new→old，ContainerId=old_container）
    if new_dps_tmp.is_file() {
        let deg = patch_dps_file(
            &new_dps_tmp,
            &old_dps_path,
            old_bytes,
            old_container.as_ref(),
        )?;
        if deg {
            degraded_dps = true;
        }
        changed += 1;
    }
    let _ = std::fs::remove_dir_all(&tmp_dir);

    // (d) 最后交换文件名：OLD.sav ↔ NEW.sav（与参考 fix_host_save.py 对齐）。
    //     （_dps 文件名交换已在第 (c) 步直接写入对端文件名完成。）
    let tmp_path = players_dir.join(format!("{}.sav.tmp_swap", old_stem));
    std::fs::rename(&old_path, &tmp_path)
        .map_err(|e| format!("重命名 {} 失败: {}", old_path.display(), e))?;
    if new_path.is_file() {
        std::fs::rename(&new_path, &old_path)
            .map_err(|e| format!("重命名 {} 失败: {}", new_path.display(), e))?;
    }
    std::fs::rename(&tmp_path, &new_path)
        .map_err(|e| format!("重命名 {} 失败: {}", tmp_path.display(), e))?;

    if degraded_dps {
        eprintln!(
            "[warn] fix_host: 至少一个 _dps.sav 缺少 PalStorageContainerId，\
             OwnerPlayerUId 已更新，但未设置 ContainerId.ID；\
             该玩家的帕鲁箱归属可能需手动整理（R-DPS-1）。"
        );
    }

    Ok(changed)
}

/// 执行 Fix Host Save（公开入口，供 `save_edit.rs` 调用）。
///
/// `req.world` 经 `path_util::world_data_dir` 解析为世界数据目录；`old_host_guid` /
/// `new_char_guid` 为两个角色存档文件名（去 `.sav`，可为带连字符的标准 GUID 形式）。
///
/// 返回被成功改写并写回的 `.sav` 文件数。
#[allow(dead_code)] // Legacy direct entrypoint retained for save-format diagnostics.
pub fn fix_host_save_impl(req: &FixHostRequest) -> Result<usize, String> {
    let data_dir = path_util::world_data_dir(&req.world)?;
    let old_bytes = sav_io::guid_bytes(&req.old_host_guid)?;
    let new_bytes = sav_io::guid_bytes(&req.new_char_guid)?;
    fix_host_save_in_dir(
        &data_dir,
        &old_bytes,
        &new_bytes,
        &req.old_host_guid,
        &req.new_char_guid,
    )
}

/// 供 `transfer.rs` 复用：对单个 `.sav` 文件做 GUID 单向替换并写回（带回读校验）。
#[allow(dead_code)]
pub(crate) fn swap_guid_in_file(path: &Path, old: &[u8; 16], new: &[u8; 16]) -> Result<(), String> {
    let mut file = SavFile::load(path)?;
    file.replace_guid_bytes(old, new);
    file.save(path)?;
    SavFile::load(path).map_err(|e| format!("写回后校验失败 {}: {}", path.display(), e))?;
    Ok(())
}

// === 阶段 B / C 实现（v2 三阶段迁移：角色替换 + 公会绑定） ===

/// 阶段 B/C：按参考工具的 Fix Host 语义交换同一世界中的两位玩家身份。
///
/// Phase A 把单机世界搬到专用服后，玩家首次进入服务器会生成 `new_uid`。此时不能
/// 删除该角色并做 `old -> new` 单向替换：CSPM、`Players/` 和公会 RawData 会失去
/// 成对身份。正确做法是对每对 `(old_uid, new_uid)` 执行对称交换，保留两个 CSPM
/// 条目和两个玩家文件，再把原单机角色绑定到服务器 UID。
///
/// 所有映射必须互不重叠。映射链或重复目标会使两次交换相互覆盖，直接拒绝而不写盘。
pub fn fix_host_save_multi(data_dir: &Path, mappings: &[UidMapping]) -> Result<usize, String> {
    if mappings.is_empty() {
        return Err("请选择一对要交换身份的角色".to_string());
    }

    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let old_stem = normalize_uid(&mapping.old_uid);
        let new_stem = normalize_uid(&mapping.new_uid);
        let old_bytes = sav_io::guid_bytes(&mapping.old_uid)?;
        let new_bytes = sav_io::guid_bytes(&mapping.new_uid)?;
        if old_bytes == new_bytes {
            return Err(format!("角色 UID 相同，无需交换: {}", mapping.old_uid));
        }
        if !seen.insert(old_stem.clone()) || !seen.insert(new_stem.clone()) {
            return Err("角色 UID 映射存在重复或链式覆盖，无法安全交换".to_string());
        }
        resolved.push((
            old_bytes,
            new_bytes,
            mapping.old_uid.as_str(),
            mapping.new_uid.as_str(),
        ));
    }

    let mut changed = 0usize;
    for (old_bytes, new_bytes, old_uid, new_uid) in resolved {
        changed += fix_host_save_in_dir(data_dir, &old_bytes, &new_bytes, old_uid, new_uid)?;
    }
    Ok(changed)
}

// ===========================================================================
// F5 单元测试（QA · 严过关）
// 覆盖：
//   - contains_guid 在原始字节中检测 / 计数 GUID（fix_host 的命中判定）。
//   - patch_dps 在结构化 GVAS 上正确设置 OwnerPlayerUId 与 SlotId.ContainerId.ID
//     （_dps 解析助手的字段级导航）。
//   - 真实样本双向交换：文件名交换 / Level 3-pass 对拍 / 回读无损坏 / 合成损坏输入拒绝。
// UID 全局替换逻辑由 sav_io 单测覆盖；3-pass 交换由 sav_io::swap_guids 单测覆盖。
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use gvas::engine_version::FEngineVersion;
    use gvas::game_version::DeserializedGameVersion;
    use gvas::properties::array_property::ArrayProperty;
    use gvas::properties::int_property::IntProperty;
    use gvas::properties::map_property::MapProperty;
    use gvas::properties::struct_property::{StructProperty, StructPropertyValue};
    use gvas::properties::Property;
    use gvas::types::map::HashableIndexMap;
    use gvas::types::Guid;
    use gvas::GvasFile;
    use gvas::GvasHeader;

    /// 真实 Level.sav 必须能经项目 hint 表解析并逐字节无损重序列化。
    #[test]
    fn gvas_roundtrip_lossless_probe_real() {
        let src = PathBuf::from(
            "E:/SteamLibrary/steamapps/common/PalServer/Pal/Saved/SaveGames/\
             _migration_backups/f5_1785081805544967700_2204/0/\
             1A91A61548C7B6FD7B58B2B70710F7EE",
        );
        if !src.is_dir() {
            eprintln!("[skip] 真实 E: 服务器世界不存在，跳过: {}", src.display());
            return;
        }
        let level_path = src.join("Level.sav");
        let original = SavFile::load(&level_path).expect("load Level");
        let parsed = original.parse().expect("parse Level with Palworld hints");
        let serialized =
            SavFile::from_gvas(&parsed, original.compression).expect("serialize Level");
        assert_eq!(serialized.raw, original.raw, "GVAS 往返必须逐字节无损");
        serialized.parse().expect("re-parse 应成功");
        println!("[ok] gvas 往返无损: {} 字节", original.raw.len());
    }

    /// 对存档数据中的角色做实战分析：用项目自带的 `Level.sav` 权威解析
    /// （`read_players_from_level` → `f5_world_summary_by_path_impl`）列出源/目标存档里的
    /// 全部角色与公会，等价于视频攻略里「角色转移」工具打开两个 Level.sav 后展示的列表。
    #[test]
    fn analyze_characters_source_and_target() {
        let source = "C:/Users/pengq/AppData/Local/Pal/Saved/SaveGames/76561199381352956/1D5D1F304D3AA1FE2818BA98D5223DFE";
        let target = "E:/SteamLibrary/steamapps/common/PalServer/Pal/Saved/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE";

        eprintln!(
            "\n================ 源存档（本地单机）{} ================",
            source
        );
        match world_copy::f5_world_summary_by_path_impl(source) {
            Ok(s) => {
                eprintln!("  角色数: {}", s.players.len());
                for p in &s.players {
                    eprintln!(
                        "  - 昵称={} | 等级={} | PlayerUId={} | InstanceId={} | GUID={}",
                        p.nickname, p.level, p.player_uid, p.instance_id, p.guid
                    );
                }
                eprintln!("  公会数: {}", s.guilds.len());
                for g in &s.guilds {
                    eprintln!("  - 公会 id={} | name={:?}", g.guild_id, g.name);
                }
                assert!(!s.players.is_empty(), "源存档应解析出至少 1 个角色");
            }
            Err(e) => panic!("源存档解析失败: {}", e),
        }

        eprintln!(
            "\n================ 目标存档（专用服务器）{} ================",
            target
        );
        match world_copy::f5_world_summary_by_path_impl(target) {
            Ok(s) => {
                eprintln!("  角色数: {}", s.players.len());
                for p in &s.players {
                    eprintln!(
                        "  - 昵称={} | 等级={} | PlayerUId={} | InstanceId={} | GUID={}",
                        p.nickname, p.level, p.player_uid, p.instance_id, p.guid
                    );
                }
                eprintln!("  公会数: {}", s.guilds.len());
                for g in &s.guilds {
                    eprintln!("  - 公会 id={} | name={:?}", g.guild_id, g.name);
                }
                assert!(!s.players.is_empty(), "目标存档应解析出至少 1 个角色");
            }
            Err(e) => panic!("目标存档解析失败: {}", e),
        }
        eprintln!("\n================ 分析结束 ================\n");
    }

    #[test]
    fn contains_guid_detects_and_counts() {
        let old: [u8; 16] = [0xABu8; 16];
        let mut raw = Vec::new();
        raw.extend_from_slice(&[0u8; 3]);
        raw.extend_from_slice(&old); // 出现 1
        raw.extend_from_slice(&[0u8; 2]);
        raw.extend_from_slice(&old); // 出现 2

        assert!(contains_guid(&raw, &old), "应检测到 old GUID");
        let other: [u8; 16] = [0xCDu8; 16];
        assert!(!contains_guid(&raw, &other), "不应误报 other GUID");
    }

    /// `_dps` 解析助手 `patch_dps` 的字段级导航：构造最小嵌套 GVAS，断言
    /// `OwnerPlayerUId` 与 `SlotId.ContainerId.ID` 被正确设置。
    #[test]
    fn patch_dps_sets_owner_and_container() {
        let old_uid: [u8; 16] = [0xAAu8; 16];
        let new_uid: [u8; 16] = [0xBBu8; 16];
        let container: [u8; 16] = [0xCCu8; 16];

        // 构造：SaveParameterArray -> [ { SaveParameter: { OwnerPlayerUId: Guid(old),
        //   SlotId: { ContainerId: { ID: Guid(0) } } } } ]
        let zero = Guid::from_u8([0u8; 16]);

        // SlotId.ContainerId.ID
        let mut cid_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        cid_map.insert(
            "ID".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "Guid".to_string(),
                StructPropertyValue::Guid(Guid::from_u8([0u8; 16])),
            ))],
        );
        let mut slot_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        slot_map.insert(
            "ContainerId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(cid_map),
            ))],
        );

        // SaveParameter 内部
        let mut inner_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        inner_map.insert(
            "OwnerPlayerUId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "Guid".to_string(),
                StructPropertyValue::Guid(Guid::from_u8(old_uid)),
            ))],
        );
        inner_map.insert(
            "SlotId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(slot_map),
            ))],
        );

        let save_param = Property::StructProperty(StructProperty::new(
            zero,
            "StructProperty".to_string(),
            StructPropertyValue::CustomStruct(inner_map),
        ));

        let mut sp_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        sp_map.insert("SaveParameter".to_string(), vec![save_param]);

        let structs = vec![StructPropertyValue::CustomStruct(sp_map)];
        let arr = Property::ArrayProperty(ArrayProperty::Structs {
            field_name: "SaveParameterArray".to_string(),
            type_name: "StructProperty".to_string(),
            guid: zero,
            structs,
        });

        let mut props: HashableIndexMap<String, Property> = HashableIndexMap::new();
        props.insert("SaveParameterArray".to_string(), arr);

        let mut gvas = GvasFile {
            deserialized_game_version: DeserializedGameVersion::Default,
            header: GvasHeader::Version2 {
                package_file_version: 0,
                engine_version: FEngineVersion::new(5, 0, 0, 0, String::new()),
                custom_version_format: 3,
                custom_versions: HashableIndexMap::new(),
                save_game_class_name: "TestSave".to_string(),
            },
            properties: props,
        };

        let updated = patch_dps(&mut gvas, &new_uid, Some(&container));
        assert!(
            updated >= 2,
            "应更新 OwnerPlayerUId 与 ContainerId.ID，实际 {updated}"
        );

        // 回读校验
        let arr_prop = gvas.properties.get("SaveParameterArray").unwrap();
        if let Property::ArrayProperty(ArrayProperty::Structs { structs, .. }) = arr_prop {
            let spv = &structs[0];
            if let StructPropertyValue::CustomStruct(map) = spv {
                let sp = map.get("SaveParameter").unwrap().first().unwrap();
                if let Property::StructProperty(s) = sp {
                    if let StructPropertyValue::CustomStruct(inner) = &s.value {
                        let opu = inner.get("OwnerPlayerUId").unwrap().first().unwrap();
                        if let Property::StructProperty(os) = opu {
                            if let StructPropertyValue::Guid(g) = &os.value {
                                assert_eq!(g.to_u8(), new_uid, "OwnerPlayerUId 应被设为 new_uid");
                            }
                        }
                        let slot = inner.get("SlotId").unwrap().first().unwrap();
                        if let Property::StructProperty(ss) = slot {
                            if let StructPropertyValue::CustomStruct(slot_map) = &ss.value {
                                let cid = slot_map.get("ContainerId").unwrap().first().unwrap();
                                if let Property::StructProperty(cs) = cid {
                                    if let StructPropertyValue::CustomStruct(cid_map) = &cs.value {
                                        let idp = cid_map.get("ID").unwrap().first().unwrap();
                                        if let Property::StructProperty(ids) = idp {
                                            if let StructPropertyValue::Guid(g) = &ids.value {
                                                assert_eq!(
                                                    g.to_u8(),
                                                    container,
                                                    "ContainerId.ID 应被设为 container"
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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

    #[test]
    fn phase_c_swaps_guild_admin_members_handles_and_marker_owners() {
        let old = [0x11; 16];
        let new = [0x22; 16];
        let old_instance = [0xA1; 16];
        let new_instance = [0xB2; 16];
        let guild_id = [0x33; 16];
        let marker_id = [0x44; 16];
        let mut raw = Vec::new();

        push_guid(&mut raw, &guild_id);
        push_i32(&mut raw, 0); // guild name
        push_i32(&mut raw, 2); // character handles
        let old_handle_uid = push_guid(&mut raw, &old);
        push_guid(&mut raw, &old_instance);
        let new_handle_uid = push_guid(&mut raw, &new);
        push_guid(&mut raw, &new_instance);
        raw.push(0); // base camp worker map flag
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 0); // base ids
        raw.extend_from_slice(&[0; 8]);
        push_i32(&mut raw, 0); // base object ids
        push_i32(&mut raw, 0); // guild name 2
        let last_modifier = push_guid(&mut raw, &old);
        push_i32(&mut raw, 1); // map markers
        push_guid(&mut raw, &marker_id);
        raw.extend_from_slice(&[0; 24]);
        push_i32(&mut raw, 0);
        let marker_owner = push_guid(&mut raw, &new);
        let administrator = push_guid(&mut raw, &old);
        push_i32(&mut raw, 2); // members
        let old_member = push_guid(&mut raw, &old);
        raw.extend_from_slice(&[0; 8]);
        push_i32(&mut raw, 0);
        let new_member = push_guid(&mut raw, &new);
        raw.extend_from_slice(&[0; 8]);
        push_i32(&mut raw, 0);
        push_i32(&mut raw, 0); // trailing unknown

        let changed = patch_guild_rawdata(
            &mut raw,
            &old,
            &new,
            Some(&old_instance),
            Some(&new_instance),
        )
        .expect("公会 RawData 应可解析并交换身份");

        assert_eq!(changed, 7);
        assert_eq!(guid_at(&raw, old_handle_uid), new);
        assert_eq!(guid_at(&raw, new_handle_uid), old);
        assert_eq!(guid_at(&raw, last_modifier), new);
        assert_eq!(guid_at(&raw, marker_owner), old);
        assert_eq!(guid_at(&raw, administrator), new);
        assert_eq!(guid_at(&raw, old_member), new);
        assert_eq!(guid_at(&raw, new_member), old);
    }

    // ---- 阶段 B / C 纯函数与真实样本测试 ----

    /// 实践检验（真实存档数据）：把真实 E: 服务器世界 1A91A615 拷到临时副本，
    /// 跑完整 Phase B+C，断言 煜 继承到 4E239D4F、公会重绑。仅作用于副本，真实 E: 存档不动。
    #[test]
    fn practice_migration_on_live_copy() {
        let src = PathBuf::from(
            "E:/SteamLibrary/steamapps/common/PalServer/Pal/Saved/SaveGames/\
             _migration_backups/f5_1785081805544967700_2204/0/\
             1A91A61548C7B6FD7B58B2B70710F7EE",
        );
        if !src.is_dir() {
            eprintln!("[skip] 真实 E: 服务器世界不存在，跳过: {}", src.display());
            return;
        }
        let work = std::env::temp_dir().join(format!(
            "palworld_live_practice_{}_{}",
            std::process::id(),
            DPS_TMP_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&work);
        let mut n = 0usize;
        crate::save_edit::path_util::copy_dir_recursive(&src, &work, &mut n)
            .expect("拷贝真实世界到临时副本");

        let players = work.join("Players");
        let old_disk = "00000000000000000000000000000001";
        let new_disk = "4E239D4F000000000000000000000000";
        let old_path = players.join(format!("{}.sav", old_disk));
        let new_path = players.join(format!("{}.sav", new_disk));
        assert!(old_path.is_file(), "真实存档应有 old 角色文件");
        assert!(new_path.is_file(), "真实存档应有 new(煜2)角色文件");

        let level_path = work.join("Level.sav");
        let old_bytes = sav_io::guid_bytes(old_disk).expect("old guid");

        // 迁移前：Level.sav 原始字节含 old UID 引用；f5 摘要含 煜。
        let level_raw_pre = SavFile::load(&level_path).expect("读 Level").raw;
        assert!(!level_raw_pre.is_empty(), "迁移前 Level.sav 不应为空");
        let pre =
            world_copy::f5_world_summary_by_path_impl(work.to_str().unwrap()).expect("迁移前摘要");
        assert!(
            pre.players.iter().any(|p| p.nickname == "煜"),
            "迁移前 煜 应存在"
        );

        // 执行同一世界内的角色与公会身份交换。
        let mappings = vec![UidMapping {
            old_uid: old_disk.to_string(),
            new_uid: new_disk.to_string(),
        }];
        let changed = fix_host_save_multi(&work, &mappings).expect("Phase B/C 应成功");

        // 两个玩家文件和两条角色记录都保留，只交换身份。
        assert!(old_path.is_file(), "交换后 old 玩家文件仍应存在");
        assert!(new_path.is_file(), "交换后 new 玩家文件仍应存在");
        let post =
            world_copy::f5_world_summary_by_path_impl(work.to_str().unwrap()).expect("迁移后摘要");
        let yu = post
            .players
            .iter()
            .find(|p| p.nickname == "煜")
            .expect("迁移后 煜 应仍在");
        let yu2 = post
            .players
            .iter()
            .find(|p| p.nickname == "煜2")
            .expect("迁移后 煜2 应仍在");
        assert_eq!(
            world_copy::guid_std(&sav_io::guid_bytes(&yu.player_uid).unwrap()),
            new_disk
        );
        assert_eq!(
            world_copy::guid_std(&sav_io::guid_bytes(&yu2.player_uid).unwrap()),
            old_disk
        );

        // 原单机角色文件现在位于服务器 UID 文件名下，内部不再保留旧 UID。
        let new_player_gvas = SavFile::load(&new_path)
            .expect("读新玩家文件")
            .parse()
            .expect("解析新玩家文件");
        assert_eq!(
            sav_io::count_guid_in_gvas(&new_player_gvas, &old_bytes),
            0,
            "迁移后玩家文件不应再含 old UID 的 Guid 属性"
        );

        SavFile::load(&level_path)
            .expect("读 Level 后")
            .parse()
            .expect("交换后的 Level.sav 必须可完整解析");
        println!("[ok] 真实存档副本 B/C 身份交换通过：煜→4E239D4F，煜2→0001，改写文件数={changed}");
        let _ = std::fs::remove_dir_all(&work);
    }

    /// 构造真实 CSPM MapProperty 的一个键值对。
    ///
    /// Palworld 的 `CharacterSaveParameterMap` 是
    /// `MapProperty<StructProperty, StructProperty>`，不是 ArrayProperty；这个夹具必须
    /// 保持与真实档同形，才能验证 Phase B 的去重逻辑。
    fn make_cspm_map_entry(puid: Guid, iid: Guid) -> (Property, Property) {
        let zero = Guid::from_u8([0u8; 16]);
        let mut key_fields: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        key_fields.insert(
            "PlayerUId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "Guid".to_string(),
                StructPropertyValue::Guid(puid),
            ))],
        );
        key_fields.insert(
            "InstanceId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "Guid".to_string(),
                StructPropertyValue::Guid(iid),
            ))],
        );
        (
            Property::StructPropertyValue(StructPropertyValue::CustomStruct(key_fields)),
            Property::StructPropertyValue(StructPropertyValue::CustomStruct(
                HashableIndexMap::new(),
            )),
        )
    }

    /// 构造最小真实形态的 CSPM MapProperty Level.sav。
    fn make_synthetic_level_with_cspm_map(
        entries: HashableIndexMap<Property, Property>,
    ) -> GvasFile {
        let zero = Guid::from_u8([0u8; 16]);
        let cspm = Property::MapProperty(MapProperty::new(
            "StructProperty".to_string(),
            "StructProperty".to_string(),
            0,
            entries,
        ));
        let mut wsd_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        wsd_map.insert("CharacterSaveParameterMap".to_string(), vec![cspm]);
        let wsd = Property::StructProperty(StructProperty::new(
            zero,
            "StructProperty".to_string(),
            StructPropertyValue::CustomStruct(wsd_map),
        ));
        let mut props: HashableIndexMap<String, Property> = HashableIndexMap::new();
        props.insert("worldSaveData".to_string(), wsd);
        GvasFile {
            deserialized_game_version: DeserializedGameVersion::Default,
            header: GvasHeader::Version2 {
                package_file_version: 0x20B,
                engine_version: FEngineVersion::new(5, 0, 0, 0, String::new()),
                custom_version_format: 3,
                custom_versions: HashableIndexMap::new(),
                save_game_class_name: "TestSave".to_string(),
            },
            properties: props,
        }
    }

    fn map_key_player_instance(key: &Property) -> Option<(Guid, Guid)> {
        let Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields)) = key else {
            return None;
        };
        let player = fields.get("PlayerUId")?.first().and_then(sav_io::as_guid)?;
        let instance = fields
            .get("InstanceId")?
            .first()
            .and_then(sav_io::as_guid)?;
        Some((*player, *instance))
    }

    fn test_guid_property(value: Guid) -> Property {
        Property::StructProperty(StructProperty::new(
            Guid::from_u8([0; 16]),
            "Guid".to_string(),
            StructPropertyValue::Guid(value),
        ))
    }

    fn map_entry_rawdata_by_instance(
        map: &HashableIndexMap<Property, Property>,
        wanted_instance: Guid,
    ) -> Option<Vec<u8>> {
        map.iter().find_map(|(key, value)| {
            let (_, instance) = map_key_player_instance(key)?;
            if instance != wanted_instance {
                return None;
            }
            let mut value = value.clone();
            rawdata_bytes_mut(&mut value).cloned()
        })
    }

    /// 构造最小可解析的玩家 .sav，包含可供结构化身份交换命中的 PlayerUId。
    fn make_minimal_player_sav(path: &Path, player_uid: Guid, instance_id: Guid) {
        let zero = Guid::from_u8([0u8; 16]);
        let mut individual_id = HashableIndexMap::new();
        individual_id.insert(
            "InstanceId".to_string(),
            vec![test_guid_property(instance_id)],
        );
        let mut save_data = HashableIndexMap::new();
        save_data.insert(
            "IndividualId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(individual_id),
            ))],
        );
        let mut props: HashableIndexMap<String, Property> = HashableIndexMap::new();
        props.insert(
            "SaveData".to_string(),
            Property::StructProperty(StructProperty::new(
                zero,
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(save_data),
            )),
        );
        props.insert(
            "PlayerUId".to_string(),
            Property::StructProperty(StructProperty::new(
                zero,
                "Guid".to_string(),
                StructPropertyValue::Guid(player_uid),
            )),
        );
        props.insert("TestInt".to_string(), IntProperty::new(7).into());
        let gvas = GvasFile {
            deserialized_game_version: DeserializedGameVersion::Default,
            header: GvasHeader::Version2 {
                package_file_version: 0x20B,
                engine_version: FEngineVersion::new(5, 0, 0, 0, String::new()),
                custom_version_format: 3,
                custom_versions: HashableIndexMap::new(),
                save_game_class_name: "TestSave".to_string(),
            },
            properties: props,
        };
        SavFile::from_gvas(&gvas, sav_io::SavCompression::Plz)
            .expect("构造玩家占位 .sav 应成功")
            .save(path)
            .expect("写玩家占位 .sav 应成功");
    }

    #[test]
    fn guid_mapping_roundtrips_palworld_disk_uid_word_order() {
        let uid = "4E239D4F000000000000000000000000";
        let raw = sav_io::guid_bytes(uid).expect("磁盘 UID 应可解析");

        assert_eq!(
            &raw[..4],
            &[0x4F, 0x9D, 0x23, 0x4E],
            "Palworld FGuid 的每个 u32 以小端存储"
        );
        assert_eq!(
            world_copy::guid_std(&raw),
            uid,
            "摘要/UI UID 必须能无损往返为 Players 文件名 UID"
        );

        let host_uid = "00000000000000000000000000000001";
        let host_raw = sav_io::guid_bytes(host_uid).expect("单机主机 UID 应可解析");
        assert_eq!(
            &host_raw[12..],
            &[1, 0, 0, 0],
            "单机主机 UID=FGuid 的第 4 个小端 u32"
        );
        assert_eq!(world_copy::guid_std(&host_raw), host_uid);
    }

    #[test]
    fn phase_bc_swaps_real_cspm_map_property() {
        let work = std::env::temp_dir().join(format!(
            "fixhost_cspm_map_{}_{}",
            std::process::id(),
            DPS_TMP_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&work);
        let players = work.join("Players");
        std::fs::create_dir_all(&players).expect("创建 Players 目录");

        let old_uid = "01000000000000000000000000000000";
        let new_uid = "02000000000000000000000000000000";
        let old_guid = Guid::from_u8(sav_io::guid_bytes(old_uid).expect("old UID"));
        let new_guid = Guid::from_u8(sav_io::guid_bytes(new_uid).expect("new UID"));
        let source_instance = Guid::from_u8([0xA1; 16]);
        let target_instance = Guid::from_u8([0xB2; 16]);
        let mut entries = HashableIndexMap::new();
        let (source_key, source_value) = make_cspm_map_entry(old_guid, source_instance);
        let (target_key, target_value) = make_cspm_map_entry(new_guid, target_instance);
        entries.insert(source_key, source_value);
        entries.insert(target_key, target_value);

        let level_path = work.join("Level.sav");
        SavFile::from_gvas(
            &make_synthetic_level_with_cspm_map(entries),
            sav_io::SavCompression::Plz,
        )
        .expect("构造 CSPM Map Level.sav")
        .save(&level_path)
        .expect("写 Level.sav");
        make_minimal_player_sav(
            &players.join(format!("{}.sav", normalize_uid(old_uid))),
            old_guid,
            source_instance,
        );
        make_minimal_player_sav(
            &players.join(format!("{}.sav", normalize_uid(new_uid))),
            new_guid,
            target_instance,
        );

        fix_host_save_multi(
            &work,
            &[UidMapping {
                old_uid: old_uid.to_string(),
                new_uid: new_uid.to_string(),
            }],
        )
        .expect("Phase B/C 应支持真实 CSPM MapProperty");

        let gvas = SavFile::load(&level_path)
            .expect("读 Level")
            .parse()
            .expect("解析 Level");
        let fields = sav_io::top_field(&gvas, "worldSaveData")
            .and_then(sav_io::struct_value)
            .and_then(sav_io::custom_fields)
            .expect("worldSaveData");
        let cspm = sav_io::field(fields, "CharacterSaveParameterMap").expect("CSPM");
        let map = world_copy::as_props_map(cspm).expect("CSPM 必须保持 MapProperty");
        let pairs: Vec<(Guid, Guid)> = map.keys().filter_map(map_key_player_instance).collect();
        assert!(
            pairs.contains(&(new_guid, source_instance)),
            "源角色的 PlayerUId 应交换为 new UID，且保留原 InstanceId"
        );
        assert!(
            pairs.contains(&(old_guid, target_instance)),
            "目标空角色应保留并交换为 old UID，不能被单向删除"
        );
        assert!(
            players
                .join(format!("{}.sav", normalize_uid(old_uid)))
                .is_file()
                && players
                    .join(format!("{}.sav", normalize_uid(new_uid)))
                    .is_file(),
            "Fix Host 交换后两个玩家文件都必须存在"
        );
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn phase_bc_preserves_pal_cspm_key_identity() {
        let root = PathBuf::from(
            "E:/SteamLibrary/steamapps/common/PalServer/Pal/Saved/SaveGames/\
             _migration_backups/f5_1785081805544967700_2204/0/\
             1A91A61548C7B6FD7B58B2B70710F7EE",
        );
        if !root.join("Level.sav").is_file() {
            eprintln!("[skip] 迁移前快照不存在: {}", root.display());
            return;
        }
        let old_uid = "00000000000000000000000000000001";
        let new_uid = "4E239D4F000000000000000000000000";
        let old = sav_io::guid_bytes(old_uid).unwrap();
        let new = sav_io::guid_bytes(new_uid).unwrap();
        let old_player = SavFile::load(&root.join("Players").join(format!("{old_uid}.sav")))
            .unwrap()
            .parse()
            .unwrap();
        let new_player = SavFile::load(&root.join("Players").join(format!("{new_uid}.sav")))
            .unwrap()
            .parse()
            .unwrap();
        let (_, old_instance) = extract_player_uids(&old_player);
        let (_, new_instance) = extract_player_uids(&new_player);
        let old_instance = old_instance.expect("source player InstanceId");

        let level = SavFile::load(&root.join("Level.sav")).unwrap();
        let before = level.parse().unwrap();
        let before_world = sav_io::top_field(&before, "worldSaveData")
            .and_then(sav_io::struct_value)
            .and_then(sav_io::custom_fields)
            .unwrap();
        let before_map = world_copy::as_props_map(
            sav_io::field(before_world, "CharacterSaveParameterMap").unwrap(),
        )
        .unwrap();
        let pal_instance = before_map
            .keys()
            .filter_map(map_key_player_instance)
            .find(|(player, instance)| player.to_u8() == old && instance.to_u8() != old_instance)
            .map(|(_, instance)| instance)
            .expect("snapshot should contain a Pal owned by the local host");
        assert_eq!(before_map.len(), 389, "test snapshot CSPM baseline changed");
        let before_pal_raw = map_entry_rawdata_by_instance(before_map, pal_instance)
            .expect("the selected Pal must contain RawData");
        let custom_versions = before.header.get_custom_versions().clone();
        let absent = [0xD7; 16];
        let (_, before_old_owner_count) = sav_io::swap_owner_guids_in_character_property_stream(
            &before_pal_raw,
            &custom_versions,
            &old,
            &absent,
        )
        .expect("read the Pal owner before migration");
        let (_, before_new_owner_count) = sav_io::swap_owner_guids_in_character_property_stream(
            &before_pal_raw,
            &custom_versions,
            &new,
            &absent,
        )
        .expect("check the target owner before migration");
        assert!(
            before_old_owner_count > 0,
            "selected Pal must belong to the old host"
        );
        assert_eq!(
            before_new_owner_count, 0,
            "selected Pal must not already belong to target UID"
        );

        let (patched, _) = patch_level_identity(
            &level,
            &old,
            &new,
            Some(&old_instance),
            new_instance.as_ref(),
        )
        .expect("patch Level identity");
        let after = patched.parse().unwrap();
        let after_world = sav_io::top_field(&after, "worldSaveData")
            .and_then(sav_io::struct_value)
            .and_then(sav_io::custom_fields)
            .unwrap();
        let after_map = world_copy::as_props_map(
            sav_io::field(after_world, "CharacterSaveParameterMap").unwrap(),
        )
        .unwrap();
        assert_eq!(
            after_map.len(),
            before_map.len(),
            "identity migration must preserve all CSPM records"
        );
        assert!(
            after_map
                .keys()
                .filter_map(map_key_player_instance)
                .any(|(player, instance)| player.to_u8() == new && instance.to_u8() == old_instance),
            "source player CSPM key must move to the server UID"
        );
        let new_instance = new_instance.expect("target player InstanceId");
        assert!(
            after_map
                .keys()
                .filter_map(map_key_player_instance)
                .any(|(player, instance)| player.to_u8() == old && instance.to_u8() == new_instance),
            "target placeholder player CSPM key must swap to the old UID"
        );
        let pal_player_uid = after_map
            .keys()
            .filter_map(map_key_player_instance)
            .find(|(_, instance)| *instance == pal_instance)
            .map(|(player, _)| player.to_u8())
            .expect("the Pal CSPM entry must still exist");
        assert_eq!(
            pal_player_uid, old,
            "Pal CSPM key is a stable CharacterContainer reference and must not follow the player UID swap"
        );
        let after_pal_raw = map_entry_rawdata_by_instance(after_map, pal_instance)
            .expect("the selected Pal RawData must remain after migration");
        let (_, after_old_owner_count) = sav_io::swap_owner_guids_in_character_property_stream(
            &after_pal_raw,
            &custom_versions,
            &old,
            &absent,
        )
        .expect("check the old owner after migration");
        let (_, after_new_owner_count) = sav_io::swap_owner_guids_in_character_property_stream(
            &after_pal_raw,
            &custom_versions,
            &new,
            &absent,
        )
        .expect("read the Pal owner after migration");
        assert_eq!(
            after_old_owner_count, 0,
            "Pal RawData must no longer name the old host as owner"
        );
        assert_eq!(
            after_new_owner_count, before_old_owner_count,
            "Pal RawData OwnerPlayerUId must move to the server UID"
        );
    }

    /// 缺少服务器新角色时必须在改写 Level.sav 和旧玩家文件前失败。
    #[test]
    fn phase_bc_missing_target_player_is_atomic() {
        let work = std::env::temp_dir().join(format!(
            "fixhost_missing_target_{}_{}",
            std::process::id(),
            DPS_TMP_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&work);
        let players = work.join("Players");
        std::fs::create_dir_all(&players).expect("创建 Players 目录");

        let old_uid = "01000000000000000000000000000000";
        let new_uid = "02000000000000000000000000000000";
        let old_bytes = sav_io::guid_bytes(old_uid).expect("解析 old GUID");
        let new_bytes = sav_io::guid_bytes(new_uid).expect("解析 new GUID");

        let mut level_raw = b"LEVEL-BEFORE".to_vec();
        level_raw.extend_from_slice(&old_bytes);
        level_raw.extend_from_slice(&new_bytes);
        let level_path = work.join("Level.sav");
        SavFile {
            raw: level_raw,
            compression: sav_io::SavCompression::Plz,
        }
        .save(&level_path)
        .expect("写 Level.sav 应成功");

        let old_player = players.join(format!("{}.sav", normalize_uid(old_uid)));
        let mut old_player_raw = b"PLAYER-BEFORE".to_vec();
        old_player_raw.extend_from_slice(&old_bytes);
        SavFile {
            raw: old_player_raw,
            compression: sav_io::SavCompression::Plz,
        }
        .save(&old_player)
        .expect("写旧玩家文件");
        let level_before = std::fs::read(&level_path).expect("读取 Level 原始文件");
        let player_before = std::fs::read(&old_player).expect("读取玩家原始文件");

        let error = fix_host_save_multi(
            &work,
            &[UidMapping {
                old_uid: old_uid.to_string(),
                new_uid: new_uid.to_string(),
            }],
        )
        .expect_err("缺少服务器新角色时必须拒绝");
        assert!(error.contains("新角色存档不存在"));
        assert_eq!(std::fs::read(&level_path).unwrap(), level_before);
        assert_eq!(std::fs::read(&old_player).unwrap(), player_before);

        let _ = std::fs::remove_dir_all(&work);
    }

    /// 合成损坏输入必须被拒绝（绝不静默产生损坏存档）。
    #[test]
    fn corrupt_sav_rejected() {
        let tmp = std::env::temp_dir().join(format!(
            "palworld_fixhost_corrupt_{}.sav",
            std::process::id()
        ));
        // 截断 / 非法：仅 7 字节，远不足 12 字节头。
        std::fs::write(&tmp, b"GVAS\x00\x00").unwrap();
        let r = SavFile::load(&tmp);
        assert!(r.is_err(), "截断 / 非法 .sav 必须被拒绝");
        let _ = std::fs::remove_file(&tmp);
    }
}
