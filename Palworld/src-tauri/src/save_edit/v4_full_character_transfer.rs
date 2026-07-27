//! V4 full-character-transfer candidate builder.
//!
//! This module deliberately has no filesystem or Tauri dependencies. It builds a complete set of
//! target documents from immutable parsed inputs. Callers may commit the returned documents only
//! after their transaction backup is prepared, so an error from this module can never leave a
//! partially transferred character.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};

use gvas::cursor_ext::{ReadExt, WriteExt};
use gvas::properties::array_property::ArrayProperty;
use gvas::properties::map_property::MapProperty;
use gvas::properties::struct_property::StructPropertyValue;
use gvas::properties::{Property, PropertyOptions, PropertyTrait};
use gvas::types::map::HashableIndexMap;
use gvas::types::Guid;
use gvas::GvasFile;
use sha2::{Digest, Sha256};

const REQUIRED_IDENTITY_FIELDS: [&str; 5] = [
    "PlayerUId",
    "IndividualId",
    "PalStorageContainerId",
    "OtomoCharacterContainerId",
    "InventoryInfo",
];

const INVENTORY_FIELDS: [&str; 6] = [
    "CommonContainerId",
    "EssentialContainerId",
    "WeaponLoadOutContainerId",
    "PlayerEquipArmorContainerId",
    "FoodEquipContainerId",
    "DropSlotContainerId",
];

#[derive(Debug, Clone, Copy)]
pub struct TransferSelection {
    pub source_player_uid: Guid,
    pub target_player_uid: Guid,
}

pub struct FullCharacterTransferInput<'a> {
    pub source_level: &'a GvasFile,
    pub source_player: &'a GvasFile,
    pub source_dps: Option<&'a GvasFile>,
    pub target_level: &'a GvasFile,
    pub target_player: &'a GvasFile,
    pub selection: TransferSelection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FullCharacterTransferStats {
    pub inventory_containers: usize,
    pub character_containers: usize,
    pub pals: usize,
    pub dynamic_items: usize,
}

#[derive(Debug, Clone)]
pub struct FullCharacterTransferCandidate {
    pub target_level: GvasFile,
    pub target_player: GvasFile,
    /// `None` means the transaction layer must remove an existing target DPS file.
    pub target_dps: Option<GvasFile>,
    pub stats: FullCharacterTransferStats,
}

#[derive(Debug, Clone)]
struct PlayerIdentity {
    uid: Guid,
    instance_id: Guid,
    group_id: Guid,
    pal_storage_id: Guid,
    otomo_container_id: Guid,
    inventory: Vec<(String, Guid)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerIdentitySummary {
    pub player_uid: Guid,
    pub instance_id: Guid,
    pub group_id: Guid,
    pub pal_storage_id: Guid,
    pub otomo_container_id: Guid,
    pub inventory: Vec<(String, Guid)>,
}

#[derive(Debug, Clone, Copy)]
struct CharacterInfo {
    is_player: bool,
    owner_uid: Option<Guid>,
}

type GuidMapping = (Guid, Guid);

pub fn describe_player_identity(
    player: &GvasFile,
    label: &str,
) -> Result<PlayerIdentitySummary, String> {
    let identity = player_identity(player, label)?;
    Ok(PlayerIdentitySummary {
        player_uid: identity.uid,
        instance_id: identity.instance_id,
        group_id: identity.group_id,
        pal_storage_id: identity.pal_storage_id,
        otomo_container_id: identity.otomo_container_id,
        inventory: identity.inventory,
    })
}

pub fn build_full_character_transfer(
    input: FullCharacterTransferInput<'_>,
) -> Result<FullCharacterTransferCandidate, String> {
    if input.selection.source_player_uid == input.selection.target_player_uid {
        return Err("源角色与目标角色 UID 相同，无需转移".to_string());
    }

    let source_identity = player_identity(input.source_player, "源角色")?;
    let target_identity = player_identity(input.target_player, "目标角色")?;
    if source_identity.uid != input.selection.source_player_uid {
        return Err("源玩家文件 PlayerUId 与所选源角色不一致".to_string());
    }
    if target_identity.uid != input.selection.target_player_uid {
        return Err("目标玩家文件 PlayerUId 与所选目标角色不一致".to_string());
    }

    let source_versions = input.source_level.header.get_custom_versions();
    let target_versions = input.target_level.header.get_custom_versions();
    let source_cspm = world_map(input.source_level, "CharacterSaveParameterMap")?;
    let source_player_entry =
        find_source_player_entry(source_cspm, &source_identity, source_versions)?;
    let source_pals =
        collect_owned_pals(source_cspm, source_identity.uid, source_versions, "源世界")?;

    let group_before = world_field(input.target_level, "GroupSaveDataMap").cloned();
    let mut target_level = input.target_level.clone();
    let mut mappings = vec![
        (source_identity.uid, target_identity.uid),
        (source_identity.instance_id, target_identity.instance_id),
        (
            source_identity.pal_storage_id,
            target_identity.pal_storage_id,
        ),
        (
            source_identity.otomo_container_id,
            target_identity.otomo_container_id,
        ),
    ];
    if source_identity.inventory.len() != target_identity.inventory.len() {
        return Err("源和目标 InventoryInfo 容器集合不一致".to_string());
    }
    for ((source_name, source_id), (target_name, target_id)) in source_identity
        .inventory
        .iter()
        .zip(target_identity.inventory.iter())
    {
        if source_name != target_name {
            return Err(format!(
                "源和目标 InventoryInfo 字段顺序不一致: {source_name}/{target_name}"
            ));
        }
        mappings.push((*source_id, *target_id));
    }

    let removed_target_entries = remove_character_state(
        world_map_mut(&mut target_level, "CharacterSaveParameterMap")?,
        &target_identity,
        target_versions,
        true,
    )?;
    let _removed_source_clone_entries = remove_character_state(
        world_map_mut(&mut target_level, "CharacterSaveParameterMap")?,
        &source_identity,
        target_versions,
        false,
    )?;
    let mut used_instances =
        collect_cspm_instances(world_map(&target_level, "CharacterSaveParameterMap")?);
    used_instances.insert(target_identity.instance_id);

    let mut pal_mappings = Vec::new();
    for (source_key, _) in &source_pals {
        let (_, source_instance) = cspm_key_identity(source_key)
            .ok_or_else(|| "源帕鲁 CSPM 键缺少 InstanceId".to_string())?;
        let target_instance = if used_instances.contains(&source_instance) {
            derive_unused_guid("pal", source_instance, target_identity.uid, &used_instances)?
        } else {
            source_instance
        };
        used_instances.insert(target_instance);
        if source_instance != target_instance {
            pal_mappings.push((source_instance, target_instance));
        }
    }
    mappings.extend(pal_mappings);

    let source_item_values = collect_source_container_values(
        input.source_level,
        "ItemContainerSaveData",
        &source_identity.inventory,
    )?;
    let target_item_values = collect_source_container_values(
        input.target_level,
        "ItemContainerSaveData",
        &target_identity.inventory,
    )?;
    let dynamic_plan = build_dynamic_plan(
        input.source_level,
        input.target_level,
        &source_item_values,
        &target_item_values,
        target_identity.uid,
    )?;
    let dynamic_transfer_count = dynamic_plan.transfer_count();
    mappings.extend(dynamic_plan.mappings().iter().copied());

    let mut stats = FullCharacterTransferStats::default();
    for ((field, source_id), (_, target_id)) in source_identity
        .inventory
        .iter()
        .zip(target_identity.inventory.iter())
    {
        transfer_container(
            input.source_level,
            &mut target_level,
            "ItemContainerSaveData",
            field,
            *source_id,
            *target_id,
            &mappings,
        )?;
        stats.inventory_containers += 1;
    }
    remove_source_container_clones(
        &mut target_level,
        "ItemContainerSaveData",
        source_identity
            .inventory
            .iter()
            .zip(target_identity.inventory.iter())
            .map(|((_, source), (_, target))| (*source, *target)),
    )?;
    for (field, source_id, target_id) in [
        (
            "PalStorageContainerId",
            source_identity.pal_storage_id,
            target_identity.pal_storage_id,
        ),
        (
            "OtomoCharacterContainerId",
            source_identity.otomo_container_id,
            target_identity.otomo_container_id,
        ),
    ] {
        transfer_container(
            input.source_level,
            &mut target_level,
            "CharacterContainerSaveData",
            field,
            source_id,
            target_id,
            &mappings,
        )?;
        stats.character_containers += 1;
    }
    remove_source_container_clones(
        &mut target_level,
        "CharacterContainerSaveData",
        [
            (
                source_identity.pal_storage_id,
                target_identity.pal_storage_id,
            ),
            (
                source_identity.otomo_container_id,
                target_identity.otomo_container_id,
            ),
        ]
        .into_iter(),
    )?;

    apply_dynamic_plan(&mut target_level, dynamic_plan, &mappings)?;
    stats.dynamic_items = dynamic_transfer_count;

    let remapped_player_key = remap_property(source_player_entry.0, &mappings)?;
    let remapped_player_value =
        remap_character_value(source_player_entry.1, &mappings, source_versions)?;
    {
        let target_cspm = world_map_mut(&mut target_level, "CharacterSaveParameterMap")?;
        target_cspm.insert(remapped_player_key, remapped_player_value);
        for (source_key, source_value) in source_pals {
            let remapped_key = remap_property(&source_key, &mappings)?;
            let remapped_value = remap_character_value(&source_value, &mappings, source_versions)?;
            if target_cspm.insert(remapped_key, remapped_value).is_some() {
                return Err("帕鲁 CSPM 重映射后发生键冲突，已取消转移".to_string());
            }
            stats.pals += 1;
        }
    }
    validate_single_target_player(
        world_map(&target_level, "CharacterSaveParameterMap")?,
        &target_identity,
        target_versions,
    )?;

    if world_field(&target_level, "GroupSaveDataMap").cloned() != group_before {
        return Err("普通角色转移不得修改 GroupSaveDataMap".to_string());
    }

    let mut target_player = input.source_player.clone();
    preserve_target_identity(&mut target_player, input.target_player)?;
    let candidate_identity = player_identity(&target_player, "候选目标角色")?;
    if candidate_identity.uid != target_identity.uid
        || candidate_identity.instance_id != target_identity.instance_id
        || candidate_identity.group_id != target_identity.group_id
        || candidate_identity.pal_storage_id != target_identity.pal_storage_id
        || candidate_identity.otomo_container_id != target_identity.otomo_container_id
        || candidate_identity.inventory != target_identity.inventory
    {
        return Err("候选玩家文件未完整保留目标身份".to_string());
    }

    let target_dps = input
        .source_dps
        .map(|source_dps| remap_gvas(source_dps, &mappings))
        .transpose()?;
    let _removed_target_entries = removed_target_entries;

    Ok(FullCharacterTransferCandidate {
        target_level,
        target_player,
        target_dps,
        stats,
    })
}

fn player_identity(player: &GvasFile, label: &str) -> Result<PlayerIdentity, String> {
    let fields =
        save_data_fields(player).ok_or_else(|| format!("{label}缺少 SaveData CustomStruct"))?;
    let uid = guid_field(fields, "PlayerUId").ok_or_else(|| format!("{label}缺少 PlayerUId"))?;
    let individual =
        custom_field(fields, "IndividualId").ok_or_else(|| format!("{label}缺少 IndividualId"))?;
    let individual_uid = guid_field(individual, "PlayerUId")
        .ok_or_else(|| format!("{label} IndividualId 缺少 PlayerUId"))?;
    if individual_uid != uid {
        return Err(format!(
            "{label} PlayerUId 与 IndividualId.PlayerUId 不一致"
        ));
    }
    let instance_id = guid_field(individual, "InstanceId")
        .ok_or_else(|| format!("{label} IndividualId 缺少 InstanceId"))?;
    // Local host saves may omit GroupId entirely. The workflow resolves the original guild from
    // Level.sav membership; a missing player-file field therefore means "not declared" here.
    let group_id = guid_field(fields, "GroupId").unwrap_or_else(|| Guid::from_u8([0; 16]));
    let pal_storage_id = container_id_field(fields, "PalStorageContainerId")
        .ok_or_else(|| format!("{label}缺少 PalStorageContainerId.ID"))?;
    let otomo_container_id = container_id_field(fields, "OtomoCharacterContainerId")
        .ok_or_else(|| format!("{label}缺少 OtomoCharacterContainerId.ID"))?;
    let inventory_info = custom_field(fields, "InventoryInfo")
        .ok_or_else(|| format!("{label}缺少 InventoryInfo"))?;
    let mut inventory = Vec::new();
    for field in INVENTORY_FIELDS {
        match container_id_field(inventory_info, field) {
            Some(id) => inventory.push((field.to_string(), id)),
            None if field == "DropSlotContainerId" => {}
            None => return Err(format!("{label} InventoryInfo 缺少 {field}.ID")),
        }
    }
    Ok(PlayerIdentity {
        uid,
        instance_id,
        group_id,
        pal_storage_id,
        otomo_container_id,
        inventory,
    })
}

fn preserve_target_identity(candidate: &mut GvasFile, target: &GvasFile) -> Result<(), String> {
    let target_fields =
        save_data_fields(target).ok_or_else(|| "目标角色缺少 SaveData CustomStruct".to_string())?;
    let preserved = REQUIRED_IDENTITY_FIELDS
        .iter()
        .map(|field| {
            target_fields
                .get(*field)
                .cloned()
                .ok_or_else(|| format!("目标角色缺少身份字段 {field}"))
                .map(|value| ((*field).to_string(), value))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let candidate_fields = save_data_fields_mut(candidate)
        .ok_or_else(|| "源角色缺少 SaveData CustomStruct".to_string())?;
    for (field, value) in preserved {
        candidate_fields.insert(field, value);
    }
    match target_fields.get("GroupId").cloned() {
        Some(value) => {
            candidate_fields.insert("GroupId".to_string(), value);
        }
        None => {
            candidate_fields.shift_remove("GroupId");
        }
    }
    Ok(())
}

fn find_source_player_entry<'a>(
    cspm: &'a HashableIndexMap<Property, Property>,
    identity: &PlayerIdentity,
    custom_versions: &HashableIndexMap<Guid, u32>,
) -> Result<(&'a Property, &'a Property), String> {
    let mut matches = Vec::new();
    for (key, value) in cspm.iter() {
        if cspm_key_identity(key) == Some((identity.uid, identity.instance_id)) {
            let info = character_info(value, custom_versions)?
                .ok_or_else(|| "源角色 CSPM 记录无法识别 SaveParameter".to_string())?;
            if info.is_player {
                matches.push((key, value));
            }
        }
    }
    if matches.len() != 1 {
        return Err(format!(
            "源角色 CSPM 记录数量应为 1，实际为 {}",
            matches.len()
        ));
    }
    Ok(matches[0])
}

fn collect_owned_pals(
    cspm: &HashableIndexMap<Property, Property>,
    owner: Guid,
    custom_versions: &HashableIndexMap<Guid, u32>,
    label: &str,
) -> Result<Vec<(Property, Property)>, String> {
    let mut pals = Vec::new();
    for (key, value) in cspm.iter() {
        match character_info(value, custom_versions) {
            Ok(Some(info)) if !info.is_player && info.owner_uid == Some(owner) => {
                pals.push((key.clone(), value.clone()));
            }
            Ok(_) => {}
            Err(error) => return Err(format!("{label}存在无法解析的 CSPM 记录: {error}")),
        }
    }
    Ok(pals)
}

fn remove_character_state(
    cspm: &mut HashableIndexMap<Property, Property>,
    target: &PlayerIdentity,
    custom_versions: &HashableIndexMap<Guid, u32>,
    require_player: bool,
) -> Result<usize, String> {
    let mut remove_keys = Vec::new();
    for (key, value) in cspm.iter() {
        let key_uid = cspm_key_identity(key).map(|identity| identity.0);
        match character_info(value, custom_versions) {
            Ok(Some(info))
                if (info.is_player && key_uid == Some(target.uid))
                    || (!info.is_player && info.owner_uid == Some(target.uid)) =>
            {
                remove_keys.push(key.clone());
            }
            Ok(_) => {}
            Err(error) => return Err(format!("目标世界存在无法解析的 CSPM 记录: {error}")),
        }
    }
    let found_player = remove_keys.iter().any(|key| {
        cspm_key_identity(key).map(|identity| identity.0) == Some(target.uid)
            && cspm
                .get(key)
                .and_then(|value| character_info(value, custom_versions).ok().flatten())
                .map(|info| info.is_player)
                .unwrap_or(false)
    });
    if require_player && !found_player {
        return Err("目标世界不存在所选目标玩家 CSPM 记录".to_string());
    }
    let count = remove_keys.len();
    for key in remove_keys {
        cspm.shift_remove(&key);
    }
    Ok(count)
}

fn remove_source_container_clones<I>(
    target_level: &mut GvasFile,
    map_name: &str,
    identities: I,
) -> Result<(), String>
where
    I: Iterator<Item = (Guid, Guid)>,
{
    let map = world_map_mut(target_level, map_name)?;
    for (source_id, target_id) in identities {
        if source_id == target_id {
            continue;
        }
        if let Some((key, _)) = find_map_entry(map, source_id) {
            let key = key.clone();
            map.shift_remove(&key);
        }
    }
    Ok(())
}

fn validate_single_target_player(
    cspm: &HashableIndexMap<Property, Property>,
    target: &PlayerIdentity,
    custom_versions: &HashableIndexMap<Guid, u32>,
) -> Result<(), String> {
    let mut count = 0;
    for (key, value) in cspm.iter() {
        if cspm_key_identity(key).map(|identity| identity.0) == Some(target.uid) {
            let info = character_info(value, custom_versions)?
                .ok_or_else(|| "目标 UID 的 CSPM 记录无法识别".to_string())?;
            if info.is_player {
                let (_, instance) = cspm_key_identity(key).unwrap();
                if instance != target.instance_id {
                    return Err("目标玩家 CSPM InstanceId 未保留".to_string());
                }
                count += 1;
            }
        }
    }
    if count != 1 {
        return Err(format!("目标 UID 玩家 CSPM 记录应为 1，实际为 {count}"));
    }
    Ok(())
}

fn transfer_container(
    source_level: &GvasFile,
    target_level: &mut GvasFile,
    map_name: &str,
    identity_field: &str,
    source_id: Guid,
    target_id: Guid,
    mappings: &[GuidMapping],
) -> Result<(), String> {
    let source_value = find_map_entry(world_map(source_level, map_name)?, source_id)
        .map(|(_, value)| value.clone())
        .ok_or_else(|| format!("源 {identity_field} 容器不存在"))?;
    let target_key = find_map_entry(world_map(target_level, map_name)?, target_id)
        .map(|(key, _)| key.clone())
        .ok_or_else(|| format!("目标 {identity_field} 容器不存在"))?;
    let remapped_value = remap_property(&source_value, mappings)?;
    let target_map = world_map_mut(target_level, map_name)?;
    target_map.shift_remove(&target_key);
    target_map.insert(target_key, remapped_value);
    Ok(())
}

fn collect_source_container_values(
    level: &GvasFile,
    map_name: &str,
    identities: &[(String, Guid)],
) -> Result<Vec<Property>, String> {
    let map = world_map(level, map_name)?;
    identities
        .iter()
        .map(|(field, id)| {
            find_map_entry(map, *id)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| format!("{field} 引用的 {map_name} 容器不存在"))
        })
        .collect()
}

enum DynamicPlan {
    None,
    Map {
        source_entries: Vec<(Property, Property)>,
        target_remove_keys: Vec<Property>,
        mappings: Vec<GuidMapping>,
    },
    Array {
        source_entries: Vec<StructPropertyValue>,
        target_remove_indices: Vec<usize>,
        mappings: Vec<GuidMapping>,
    },
}

impl DynamicPlan {
    fn mappings(&self) -> &[GuidMapping] {
        match self {
            Self::None => &[],
            Self::Map { mappings, .. } | Self::Array { mappings, .. } => mappings,
        }
    }

    fn transfer_count(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Map { source_entries, .. } => source_entries.len(),
            Self::Array { source_entries, .. } => source_entries.len(),
        }
    }
}

fn build_dynamic_plan(
    source_level: &GvasFile,
    target_level: &GvasFile,
    source_item_values: &[Property],
    target_item_values: &[Property],
    target_uid: Guid,
) -> Result<DynamicPlan, String> {
    let Some(source_property) = world_field(source_level, "DynamicItemSaveData") else {
        return Ok(DynamicPlan::None);
    };
    let target_property = world_field(target_level, "DynamicItemSaveData")
        .ok_or_else(|| "源世界包含动态物品，但目标缺少 DynamicItemSaveData".to_string())?;
    let source_references = collect_dynamic_references(source_item_values);
    let target_references = collect_dynamic_references(target_item_values);
    match (source_property, target_property) {
        (
            Property::MapProperty(MapProperty::Properties {
                value: source_map, ..
            }),
            Property::MapProperty(MapProperty::Properties {
                value: target_map, ..
            }),
        ) => {
            let source_entries: Vec<_> = source_map
                .iter()
                .filter(|(key, _)| {
                    map_key_guid(key)
                        .map(|id| source_references.contains(&id))
                        .unwrap_or(false)
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            let target_remove_keys: Vec<_> = target_map
                .keys()
                .filter(|key| {
                    map_key_guid(key)
                        .map(|id| {
                            target_references.contains(&id) || source_references.contains(&id)
                        })
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            let removed: HashSet<Guid> =
                target_remove_keys.iter().filter_map(map_key_guid).collect();
            let mut used: HashSet<Guid> = target_map.keys().filter_map(map_key_guid).collect();
            used.retain(|id| !removed.contains(id));
            let mappings = plan_dynamic_mappings(
                source_entries
                    .iter()
                    .filter_map(|(key, _)| map_key_guid(key)),
                used,
                target_uid,
            )?;
            Ok(DynamicPlan::Map {
                source_entries,
                target_remove_keys,
                mappings,
            })
        }
        (
            Property::ArrayProperty(ArrayProperty::Structs {
                structs: source, ..
            }),
            Property::ArrayProperty(ArrayProperty::Structs {
                structs: target, ..
            }),
        ) => {
            let source_entries: Vec<_> = source
                .iter()
                .filter(|entry| {
                    dynamic_array_entry_id(entry)
                        .map(|id| source_references.contains(&id))
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            let target_remove_indices: Vec<_> = target
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    dynamic_array_entry_id(entry).and_then(|id| {
                        (target_references.contains(&id) || source_references.contains(&id))
                            .then_some(index)
                    })
                })
                .collect();
            let removed: HashSet<Guid> = target_remove_indices
                .iter()
                .filter_map(|index| dynamic_array_entry_id(&target[*index]))
                .collect();
            let mut used: HashSet<Guid> =
                target.iter().filter_map(dynamic_array_entry_id).collect();
            used.retain(|id| !removed.contains(id));
            let mappings = plan_dynamic_mappings(
                source_entries.iter().filter_map(dynamic_array_entry_id),
                used,
                target_uid,
            )?;
            Ok(DynamicPlan::Array {
                source_entries,
                target_remove_indices,
                mappings,
            })
        }
        _ => Err("源和目标 DynamicItemSaveData 容器类型不一致".to_string()),
    }
}

fn apply_dynamic_plan(
    target_level: &mut GvasFile,
    plan: DynamicPlan,
    mappings: &[GuidMapping],
) -> Result<(), String> {
    match plan {
        DynamicPlan::None => Ok(()),
        DynamicPlan::Map {
            source_entries,
            target_remove_keys,
            ..
        } => {
            let target_map = world_map_mut(target_level, "DynamicItemSaveData")?;
            for key in target_remove_keys {
                target_map.shift_remove(&key);
            }
            for (source_key, source_value) in source_entries {
                let key = remap_property(&source_key, mappings)?;
                let value = remap_property(&source_value, mappings)?;
                if target_map.insert(key, value).is_some() {
                    return Err("动态物品 GUID 重映射后冲突".to_string());
                }
            }
            Ok(())
        }
        DynamicPlan::Array {
            source_entries,
            mut target_remove_indices,
            ..
        } => {
            let property = world_field_mut(target_level, "DynamicItemSaveData")
                .ok_or_else(|| "目标缺少 DynamicItemSaveData".to_string())?;
            let target = match property {
                Property::ArrayProperty(ArrayProperty::Structs { structs, .. }) => structs,
                _ => return Err("目标 DynamicItemSaveData 不是结构体数组".to_string()),
            };
            target_remove_indices.sort_unstable_by(|left, right| right.cmp(left));
            for index in target_remove_indices {
                target.remove(index);
            }
            for mut entry in source_entries {
                remap_struct_value(&mut entry, mappings)?;
                remap_dynamic_array_entry_id(&mut entry, mappings)?;
                target.push(entry);
            }
            Ok(())
        }
    }
}

fn plan_dynamic_mappings<I>(
    source_ids: I,
    mut used: HashSet<Guid>,
    target_uid: Guid,
) -> Result<Vec<GuidMapping>, String>
where
    I: Iterator<Item = Guid>,
{
    let mut mappings = Vec::new();
    for source_id in source_ids {
        let target_id = if used.contains(&source_id) {
            derive_unused_guid("dynamic", source_id, target_uid, &used)?
        } else {
            source_id
        };
        used.insert(target_id);
        if source_id != target_id {
            mappings.push((source_id, target_id));
        }
    }
    Ok(mappings)
}

fn collect_dynamic_references(properties: &[Property]) -> HashSet<Guid> {
    let mut references = collect_guids_many(properties);
    for property in properties {
        collect_item_slot_dynamic_ids(property, &mut references);
    }
    references
}

fn collect_item_slot_dynamic_ids(property: &Property, ids: &mut HashSet<Guid>) {
    match property {
        Property::StructProperty(value) => collect_item_slot_ids_from_struct(&value.value, ids),
        Property::StructPropertyValue(value) => collect_item_slot_ids_from_struct(value, ids),
        Property::ArrayProperty(ArrayProperty::Structs { structs, .. }) => {
            for value in structs {
                collect_item_slot_ids_from_struct(value, ids);
            }
        }
        Property::ArrayProperty(ArrayProperty::Properties { properties, .. }) => {
            for property in properties {
                collect_item_slot_dynamic_ids(property, ids);
            }
        }
        Property::MapProperty(MapProperty::Properties { value, .. }) => {
            for (key, value) in value.iter() {
                collect_item_slot_dynamic_ids(key, ids);
                collect_item_slot_dynamic_ids(value, ids);
            }
        }
        _ => {}
    }
}

fn collect_item_slot_ids_from_struct(value: &StructPropertyValue, ids: &mut HashSet<Guid>) {
    if let StructPropertyValue::CustomStruct(fields) = value {
        for (name, properties) in fields {
            for property in properties {
                if name == "RawData" {
                    if let Property::ArrayProperty(ArrayProperty::Bytes { bytes }) = property {
                        if let Some((_, id)) = item_slot_dynamic_id(bytes) {
                            ids.insert(id);
                        }
                    }
                }
                collect_item_slot_dynamic_ids(property, ids);
            }
        }
    }
}

fn item_slot_dynamic_id(bytes: &[u8]) -> Option<(usize, Guid)> {
    if bytes.len() < 12 {
        return None;
    }
    let string_length = i32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let string_bytes = if string_length >= 0 {
        string_length as usize
    } else {
        (string_length as i64).checked_neg()?.checked_mul(2)? as usize
    };
    let created_world_offset = 12usize.checked_add(string_bytes)?;
    let local_id_offset = created_world_offset.checked_add(16)?;
    let end = local_id_offset.checked_add(16)?;
    if end > bytes.len() {
        return None;
    }
    Some((
        local_id_offset,
        Guid::from_u8(bytes[local_id_offset..end].try_into().ok()?),
    ))
}

fn remap_item_slot_bytes(bytes: &mut [u8], mappings: &[GuidMapping]) {
    let Some((offset, id)) = item_slot_dynamic_id(bytes) else {
        return;
    };
    if let Some((_, target)) = mappings.iter().find(|(source, _)| *source == id) {
        bytes[offset..offset + 16].copy_from_slice(&target.to_u8());
    }
}

fn dynamic_array_entry_id(value: &StructPropertyValue) -> Option<Guid> {
    let StructPropertyValue::CustomStruct(fields) = value else {
        return None;
    };
    let bytes = match fields.get("RawData")?.first()? {
        Property::ArrayProperty(ArrayProperty::Bytes { bytes }) => bytes,
        _ => return None,
    };
    if bytes.len() < 32 {
        return None;
    }
    Some(Guid::from_u8(bytes[16..32].try_into().ok()?))
}

fn remap_dynamic_array_entry_id(
    value: &mut StructPropertyValue,
    mappings: &[GuidMapping],
) -> Result<(), String> {
    let source_id = dynamic_array_entry_id(value)
        .ok_or_else(|| "DynamicItemSaveData 数组项缺少本地动态 ID".to_string())?;
    let Some((_, target_id)) = mappings.iter().find(|(source, _)| *source == source_id) else {
        return Ok(());
    };
    let StructPropertyValue::CustomStruct(fields) = value else {
        unreachable!()
    };
    let bytes = match fields
        .get_mut("RawData")
        .and_then(|values| values.first_mut())
    {
        Some(Property::ArrayProperty(ArrayProperty::Bytes { bytes })) => bytes,
        _ => return Err("DynamicItemSaveData 数组项缺少 RawData".to_string()),
    };
    bytes[16..32].copy_from_slice(&target_id.to_u8());
    Ok(())
}

fn character_info(
    value: &Property,
    custom_versions: &HashableIndexMap<Guid, u32>,
) -> Result<Option<CharacterInfo>, String> {
    if let Some(fields) = property_fields(value) {
        if let Some(is_player) = bool_field(fields, "IsPlayer") {
            return Ok(Some(CharacterInfo {
                is_player,
                owner_uid: guid_field(fields, "OwnerPlayerUId"),
            }));
        }
        if let Some(bytes) = raw_data_bytes(value) {
            let stream = parse_property_stream(bytes, custom_versions)?;
            let save_parameter = stream
                .properties
                .iter()
                .find(|(name, _)| name == "SaveParameter")
                .and_then(|(_, property)| property_fields(property));
            if let Some(save_parameter) = save_parameter {
                return Ok(
                    bool_field(save_parameter, "IsPlayer").map(|is_player| CharacterInfo {
                        is_player,
                        owner_uid: guid_field(save_parameter, "OwnerPlayerUId"),
                    }),
                );
            }
        }
    }
    Ok(None)
}

fn remap_character_value(
    source: &Property,
    mappings: &[GuidMapping],
    custom_versions: &HashableIndexMap<Guid, u32>,
) -> Result<Property, String> {
    let mut value = remap_property(source, mappings)?;
    if let Some(bytes) = raw_data_bytes_mut(&mut value) {
        *bytes = remap_property_stream(bytes, mappings, custom_versions)?;
    }
    Ok(value)
}

struct PropertyStream {
    properties: Vec<(String, Property)>,
    tail: Vec<u8>,
}

fn parse_property_stream(
    bytes: &[u8],
    custom_versions: &HashableIndexMap<Guid, u32>,
) -> Result<PropertyStream, String> {
    let mut reader = Cursor::new(bytes);
    let hints = HashMap::new();
    let mut stack = Vec::new();
    let mut options = PropertyOptions {
        hints: &hints,
        properties_stack: &mut stack,
        custom_versions,
    };
    let mut properties = Vec::new();
    loop {
        let name = reader
            .read_string()
            .map_err(|error| format!("读取 CSPM RawData 属性名失败: {error}"))?;
        if name == "None" {
            break;
        }
        let property_type = reader
            .read_string()
            .map_err(|error| format!("读取 CSPM RawData 属性类型失败 ({name}): {error}"))?;
        let property = Property::new(&mut reader, &property_type, true, &mut options, None)
            .map_err(|error| {
                format!("解析 CSPM RawData 属性失败 ({name}/{property_type}): {error}")
            })?;
        properties.push((name, property));
    }
    let tail_start = reader.position() as usize;
    Ok(PropertyStream {
        properties,
        tail: bytes[tail_start..].to_vec(),
    })
}

fn remap_property_stream(
    bytes: &[u8],
    mappings: &[GuidMapping],
    custom_versions: &HashableIndexMap<Guid, u32>,
) -> Result<Vec<u8>, String> {
    let mut stream = parse_property_stream(bytes, custom_versions)?;
    for (_, property) in stream.properties.iter_mut() {
        remap_property_in_place(property, mappings)?;
    }
    let mut writer = Cursor::new(Vec::with_capacity(bytes.len()));
    let hints = HashMap::new();
    let mut stack = Vec::new();
    let mut options = PropertyOptions {
        hints: &hints,
        properties_stack: &mut stack,
        custom_versions,
    };
    for (name, property) in &stream.properties {
        writer
            .write_string(name)
            .map_err(|error| format!("写入 CSPM RawData 属性名失败 ({name}): {error}"))?;
        property
            .write(&mut writer, true, &mut options)
            .map_err(|error| format!("写入 CSPM RawData 属性失败 ({name}): {error}"))?;
    }
    writer
        .write_string("None")
        .map_err(|error| format!("写入 CSPM RawData 终止符失败: {error}"))?;
    writer
        .write_all(&stream.tail)
        .map_err(|error| format!("写入 CSPM RawData 尾部失败: {error}"))?;
    Ok(writer.into_inner())
}

fn remap_gvas(source: &GvasFile, mappings: &[GuidMapping]) -> Result<GvasFile, String> {
    let mut candidate = source.clone();
    for property in candidate.properties.values_mut() {
        remap_property_in_place(property, mappings)?;
    }
    Ok(candidate)
}

fn remap_property(source: &Property, mappings: &[GuidMapping]) -> Result<Property, String> {
    let mut candidate = source.clone();
    remap_property_in_place(&mut candidate, mappings)?;
    Ok(candidate)
}

fn remap_property_in_place(
    property: &mut Property,
    mappings: &[GuidMapping],
) -> Result<(), String> {
    match property {
        Property::StructProperty(value) => remap_struct_value(&mut value.value, mappings)?,
        Property::StructPropertyValue(value) => remap_struct_value(value, mappings)?,
        Property::ArrayProperty(ArrayProperty::Structs { structs, .. }) => {
            for value in structs {
                remap_struct_value(value, mappings)?;
            }
        }
        Property::ArrayProperty(ArrayProperty::Properties { properties, .. }) => {
            for value in properties {
                remap_property_in_place(value, mappings)?;
            }
        }
        Property::ArrayProperty(ArrayProperty::Bytes { bytes }) => {
            remap_item_slot_bytes(bytes, mappings);
        }
        Property::MapProperty(MapProperty::Properties { value, .. }) => {
            let old_entries = std::mem::take(&mut value.0);
            for (mut key, mut entry_value) in old_entries {
                remap_property_in_place(&mut key, mappings)?;
                remap_property_in_place(&mut entry_value, mappings)?;
                if value.insert(key, entry_value).is_some() {
                    return Err("GUID 重映射导致 MapProperty 键冲突".to_string());
                }
            }
        }
        Property::SetProperty(value) => {
            for property in value.properties.iter_mut() {
                remap_property_in_place(property, mappings)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn remap_struct_value(
    value: &mut StructPropertyValue,
    mappings: &[GuidMapping],
) -> Result<(), String> {
    match value {
        StructPropertyValue::Guid(guid) => {
            if let Some((_, target)) = mappings.iter().find(|(source, _)| source == guid) {
                *guid = *target;
            }
        }
        StructPropertyValue::CustomStruct(fields) => {
            for properties in fields.values_mut() {
                for property in properties {
                    remap_property_in_place(property, mappings)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_guids_many(properties: &[Property]) -> HashSet<Guid> {
    let mut guids = HashSet::new();
    for property in properties {
        collect_guids(property, &mut guids);
    }
    guids
}

fn collect_guids(property: &Property, guids: &mut HashSet<Guid>) {
    match property {
        Property::StructProperty(value) => collect_guids_from_struct(&value.value, guids),
        Property::StructPropertyValue(value) => collect_guids_from_struct(value, guids),
        Property::ArrayProperty(ArrayProperty::Structs { structs, .. }) => {
            for value in structs {
                collect_guids_from_struct(value, guids);
            }
        }
        Property::ArrayProperty(ArrayProperty::Properties { properties, .. }) => {
            for property in properties {
                collect_guids(property, guids);
            }
        }
        Property::MapProperty(MapProperty::Properties { value, .. }) => {
            for (key, value) in value.iter() {
                collect_guids(key, guids);
                collect_guids(value, guids);
            }
        }
        Property::SetProperty(value) => {
            for property in &value.properties {
                collect_guids(property, guids);
            }
        }
        _ => {}
    }
}

fn collect_guids_from_struct(value: &StructPropertyValue, guids: &mut HashSet<Guid>) {
    match value {
        StructPropertyValue::Guid(guid) => {
            guids.insert(*guid);
        }
        StructPropertyValue::CustomStruct(fields) => {
            for properties in fields.values() {
                for property in properties {
                    collect_guids(property, guids);
                }
            }
        }
        _ => {}
    }
}

fn derive_unused_guid(
    domain: &str,
    source: Guid,
    target_uid: Guid,
    used: &HashSet<Guid>,
) -> Result<Guid, String> {
    for counter in 0u32..=u16::MAX as u32 {
        let mut hasher = Sha256::new();
        hasher.update(b"PalworldServerManager/V4/");
        hasher.update(domain.as_bytes());
        hasher.update(source.to_u8());
        hasher.update(target_uid.to_u8());
        hasher.update(counter.to_le_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        let candidate = Guid::from_u8(bytes);
        if !candidate.is_zero() && !used.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Err(format!("无法为 {domain} 生成无冲突 GUID"))
}

fn save_data_fields(player: &GvasFile) -> Option<&HashableIndexMap<String, Vec<Property>>> {
    player.properties.get("SaveData").and_then(property_fields)
}

fn save_data_fields_mut(
    player: &mut GvasFile,
) -> Option<&mut HashableIndexMap<String, Vec<Property>>> {
    player
        .properties
        .get_mut("SaveData")
        .and_then(property_fields_mut)
}

fn world_fields(level: &GvasFile) -> Result<&HashableIndexMap<String, Vec<Property>>, String> {
    level
        .properties
        .get("worldSaveData")
        .and_then(property_fields)
        .ok_or_else(|| "Level.sav 缺少 worldSaveData CustomStruct".to_string())
}

fn world_fields_mut(
    level: &mut GvasFile,
) -> Result<&mut HashableIndexMap<String, Vec<Property>>, String> {
    level
        .properties
        .get_mut("worldSaveData")
        .and_then(property_fields_mut)
        .ok_or_else(|| "Level.sav 缺少 worldSaveData CustomStruct".to_string())
}

fn world_field<'a>(level: &'a GvasFile, name: &str) -> Option<&'a Property> {
    world_fields(level).ok()?.get(name)?.first()
}

fn world_field_mut<'a>(level: &'a mut GvasFile, name: &str) -> Option<&'a mut Property> {
    world_fields_mut(level).ok()?.get_mut(name)?.first_mut()
}

fn world_map<'a>(
    level: &'a GvasFile,
    name: &str,
) -> Result<&'a HashableIndexMap<Property, Property>, String> {
    optional_world_map(level, name)?.ok_or_else(|| format!("Level.sav 缺少 {name}"))
}

fn optional_world_map<'a>(
    level: &'a GvasFile,
    name: &str,
) -> Result<Option<&'a HashableIndexMap<Property, Property>>, String> {
    let Some(property) = world_fields(level)?
        .get(name)
        .and_then(|values| values.first())
    else {
        return Ok(None);
    };
    match property {
        Property::MapProperty(MapProperty::Properties { value, .. }) => Ok(Some(value)),
        _ => Err(format!("{name} 必须是 MapProperty::Properties")),
    }
}

fn world_map_mut<'a>(
    level: &'a mut GvasFile,
    name: &str,
) -> Result<&'a mut HashableIndexMap<Property, Property>, String> {
    let property = world_fields_mut(level)?
        .get_mut(name)
        .and_then(|values| values.first_mut())
        .ok_or_else(|| format!("Level.sav 缺少 {name}"))?;
    match property {
        Property::MapProperty(MapProperty::Properties { value, .. }) => Ok(value),
        _ => Err(format!("{name} 必须是 MapProperty::Properties")),
    }
}

fn property_fields(property: &Property) -> Option<&HashableIndexMap<String, Vec<Property>>> {
    match property {
        Property::StructProperty(value) => match &value.value {
            StructPropertyValue::CustomStruct(fields) => Some(fields),
            _ => None,
        },
        Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields)) => Some(fields),
        _ => None,
    }
}

fn property_fields_mut(
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

fn property_guid(property: &Property) -> Option<Guid> {
    match property {
        Property::StructProperty(value) => match &value.value {
            StructPropertyValue::Guid(guid) => Some(*guid),
            _ => None,
        },
        Property::StructPropertyValue(StructPropertyValue::Guid(guid)) => Some(*guid),
        _ => None,
    }
}

fn guid_field(fields: &HashableIndexMap<String, Vec<Property>>, name: &str) -> Option<Guid> {
    fields.get(name)?.first().and_then(property_guid)
}

fn bool_field(fields: &HashableIndexMap<String, Vec<Property>>, name: &str) -> Option<bool> {
    fields
        .get(name)?
        .first()
        .and_then(|property| match property {
            Property::BoolProperty(value) => Some(value.value),
            _ => None,
        })
}

fn custom_field<'a>(
    fields: &'a HashableIndexMap<String, Vec<Property>>,
    name: &str,
) -> Option<&'a HashableIndexMap<String, Vec<Property>>> {
    fields.get(name)?.first().and_then(property_fields)
}

fn container_id_field(
    fields: &HashableIndexMap<String, Vec<Property>>,
    name: &str,
) -> Option<Guid> {
    custom_field(fields, name).and_then(|container| guid_field(container, "ID"))
}

fn cspm_key_identity(key: &Property) -> Option<(Guid, Guid)> {
    let fields = property_fields(key)?;
    Some((
        guid_field(fields, "PlayerUId")?,
        guid_field(fields, "InstanceId")?,
    ))
}

fn map_key_guid(key: &Property) -> Option<Guid> {
    property_guid(key).or_else(|| {
        let fields = property_fields(key)?;
        guid_field(fields, "ID")
            .or_else(|| guid_field(fields, "InstanceId"))
            .or_else(|| guid_field(fields, "PlayerUId"))
    })
}

fn find_map_entry(
    map: &HashableIndexMap<Property, Property>,
    id: Guid,
) -> Option<(&Property, &Property)> {
    map.iter().find(|(key, _)| map_key_guid(key) == Some(id))
}

fn collect_cspm_instances(map: &HashableIndexMap<Property, Property>) -> HashSet<Guid> {
    map.keys()
        .filter_map(|key| cspm_key_identity(key).map(|identity| identity.1))
        .collect()
}

fn raw_data_bytes(value: &Property) -> Option<&[u8]> {
    let fields = property_fields(value)?;
    match fields.get("RawData")?.first()? {
        Property::ArrayProperty(ArrayProperty::Bytes { bytes }) => Some(bytes),
        _ => None,
    }
}

fn raw_data_bytes_mut(value: &mut Property) -> Option<&mut Vec<u8>> {
    let fields = property_fields_mut(value)?;
    match fields.get_mut("RawData")?.first_mut()? {
        Property::ArrayProperty(ArrayProperty::Bytes { bytes }) => Some(bytes),
        _ => None,
    }
}
