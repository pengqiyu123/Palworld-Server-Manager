#[path = "../src/save_edit/v4_full_character_transfer.rs"]
mod v4_full_character_transfer;

use std::collections::HashMap;
use std::io::Cursor;

use gvas::cursor_ext::WriteExt;
use gvas::engine_version::FEngineVersion;
use gvas::game_version::DeserializedGameVersion;
use gvas::properties::array_property::ArrayProperty;
use gvas::properties::int_property::{BoolProperty, IntProperty};
use gvas::properties::map_property::MapProperty;
use gvas::properties::struct_property::{StructProperty, StructPropertyValue};
use gvas::properties::{Property, PropertyOptions, PropertyTrait};
use gvas::types::map::HashableIndexMap;
use gvas::types::Guid;
use gvas::{GvasFile, GvasHeader};
use v4_full_character_transfer::{
    build_full_character_transfer, describe_player_identity, FullCharacterTransferInput,
    TransferSelection,
};

const INVENTORY_FIELDS: [&str; 5] = [
    "CommonContainerId",
    "EssentialContainerId",
    "WeaponLoadOutContainerId",
    "PlayerEquipArmorContainerId",
    "FoodEquipContainerId",
];

#[test]
fn transfers_full_character_while_preserving_target_identity_and_guild() {
    let fixture = Fixture::new();
    let target_before = fixture.target_level.clone();
    let target_player_before = fixture.target_player.clone();

    let result = build_full_character_transfer(FullCharacterTransferInput {
        source_level: &fixture.source_level,
        source_player: &fixture.source_player,
        source_dps: Some(&fixture.source_dps),
        target_level: &fixture.target_level,
        target_player: &fixture.target_player,
        selection: fixture.selection,
    })
    .expect("完整角色转移候选应构建成功");

    for field in [
        "PlayerUId",
        "IndividualId",
        "GroupId",
        "PalStorageContainerId",
        "OtomoCharacterContainerId",
        "InventoryInfo",
    ] {
        assert_eq!(
            player_field(&result.target_player, field),
            player_field(&target_player_before, field),
            "目标身份字段 {field} 必须保留"
        );
    }
    assert_eq!(player_int(&result.target_player, "CharacterMarker"), 99);
    assert_eq!(player_int(&result.target_player, "TechnologyMarker"), 77);

    assert_eq!(
        world_field(&result.target_level, "GroupSaveDataMap"),
        world_field(&target_before, "GroupSaveDataMap"),
        "普通角色转移绝不能改公会"
    );
    assert_eq!(result.stats.inventory_containers, INVENTORY_FIELDS.len());
    assert_eq!(result.stats.character_containers, 2);
    assert_eq!(result.stats.pals, 1);
    assert_eq!(result.stats.dynamic_items, 1);
    assert!(result.target_dps.is_some(), "源 DPS 存在时必须生成目标 DPS");
    let target_dps = result.target_dps.as_ref().unwrap();
    assert_eq!(
        nested_int(target_dps, "SaveParameterArray", "DpsMarker"),
        801
    );
    assert_eq!(
        nested_guid(target_dps, "SaveParameterArray", "OwnerPlayerUId"),
        fixture.target_uid,
        "DPS owner 必须重映射到目标 UID"
    );

    let transferred_pals: Vec<_> = cspm_entries(&result.target_level)
        .values()
        .filter(|value| !value_is_player(value) && value_owner(value) == Some(fixture.target_uid))
        .collect();
    assert_eq!(transferred_pals.len(), 1);
    assert_eq!(value_marker(transferred_pals[0]), 401);

    for (source_id, target_id) in fixture.inventory_pairs() {
        assert_eq!(
            map_value_marker(&result.target_level, "ItemContainerSaveData", target_id),
            map_value_marker(&fixture.source_level, "ItemContainerSaveData", source_id),
            "背包和装备容器内容必须转移到目标身份容器"
        );
    }
    assert_eq!(
        map_value_marker(
            &result.target_level,
            "CharacterContainerSaveData",
            fixture.target_pal_container,
        ),
        501
    );
    assert_eq!(
        map_value_marker(
            &result.target_level,
            "CharacterContainerSaveData",
            fixture.target_otomo_container,
        ),
        502
    );
    assert_eq!(
        map_value_marker(
            &result.target_level,
            "DynamicItemSaveData",
            fixture.dynamic_item,
        ),
        601
    );
}

#[test]
fn exposes_identity_only_from_the_parsed_player_document() {
    let fixture = Fixture::new();

    let identity = describe_player_identity(&fixture.target_player, "目标角色")
        .expect("应能从玩家文件提取持久化工作流所需身份");

    assert_eq!(identity.player_uid, fixture.target_uid);
    assert_eq!(identity.instance_id, fixture.target_instance);
    assert_eq!(identity.group_id, guid(0x42));
    assert_eq!(identity.pal_storage_id, fixture.target_pal_container);
    assert_eq!(identity.otomo_container_id, fixture.target_otomo_container);
    assert_eq!(identity.inventory.len(), 5);
}

#[test]
fn transfers_when_player_files_omit_optional_group_id() {
    let mut fixture = Fixture::new();
    remove_player_field(&mut fixture.source_player, "GroupId");
    remove_player_field(&mut fixture.target_player, "GroupId");

    let result = build_full_character_transfer(fixture.input())
        .expect("当前版本玩家文件省略 GroupId 时仍应完成角色转移");

    assert!(
        !player_fields(&result.target_player).contains_key("GroupId"),
        "目标未声明 GroupId 时，候选文件也应保持省略"
    );
    assert_eq!(
        world_field(&result.target_level, "GroupSaveDataMap"),
        world_field(&fixture.target_level, "GroupSaveDataMap"),
        "角色转移不得借机修改公会数据"
    );
}

#[test]
fn removes_duplicate_target_player_cspm_records_and_inserts_exactly_one() {
    let mut fixture = Fixture::new();
    let stale_instance = guid(0xE1);
    cspm_entries_mut(&mut fixture.target_level).insert(
        cspm_key(fixture.target_uid, stale_instance),
        cspm_value(true, fixture.target_uid, 2),
    );

    let result =
        build_full_character_transfer(fixture.input()).expect("重复目标玩家记录应被确定性去重");

    let matching: Vec<_> = cspm_entries(&result.target_level)
        .iter()
        .filter(|(key, value)| {
            key_player_uid(key) == Some(fixture.target_uid) && value_is_player(value)
        })
        .collect();
    assert_eq!(matching.len(), 1, "目标 UID 必须仅有一条玩家 CSPM 记录");
    assert_eq!(
        key_instance(matching[0].0),
        Some(fixture.target_instance),
        "插入记录必须使用目标 InstanceId"
    );
    assert_eq!(value_marker(matching[0].1), 101);
}

#[test]
fn failure_returns_no_candidate_and_does_not_mutate_inputs() {
    let mut fixture = Fixture::new();
    remove_map_entry(
        &mut fixture.source_level,
        "ItemContainerSaveData",
        fixture.source_inventory[0],
    );
    let source_before = fixture.source_level.clone();
    let target_before = fixture.target_level.clone();
    let player_before = fixture.target_player.clone();

    let error = build_full_character_transfer(fixture.input())
        .expect_err("缺少身份引用的源容器时必须整体失败");

    assert!(
        error.contains("CommonContainerId"),
        "错误应指出缺失容器: {error}"
    );
    assert_eq!(fixture.source_level, source_before);
    assert_eq!(fixture.target_level, target_before);
    assert_eq!(fixture.target_player, player_before);
}

#[test]
fn supports_real_cspm_rawdata_property_streams() {
    let mut fixture = Fixture::new();
    let source_pal_instance = guid(0x91);
    let source_cspm = cspm_entries_mut(&mut fixture.source_level);
    *source_cspm
        .get_mut(&cspm_key(fixture.source_uid, fixture.source_instance))
        .unwrap() = raw_cspm_value(true, fixture.source_uid, 101);
    *source_cspm
        .get_mut(&cspm_key(Guid::from_u8([0; 16]), source_pal_instance))
        .unwrap() = raw_cspm_value(false, fixture.source_uid, 401);
    let target_cspm = cspm_entries_mut(&mut fixture.target_level);
    *target_cspm
        .get_mut(&cspm_key(fixture.target_uid, fixture.target_instance))
        .unwrap() = raw_cspm_value(true, fixture.target_uid, 1);

    let result = build_full_character_transfer(fixture.input())
        .expect("真实 RawData 属性流应可解析、重映射并重新序列化");

    assert_eq!(result.stats.pals, 1);
    assert!(cspm_entries(&result.target_level).keys().any(|key| {
        key_player_uid(key) == Some(fixture.target_uid)
            && key_instance(key) == Some(fixture.target_instance)
    }));
    assert_eq!(
        world_field(&result.target_level, "GroupSaveDataMap"),
        world_field(&fixture.target_level, "GroupSaveDataMap")
    );
}

#[test]
fn migrated_world_source_records_are_rebound_instead_of_duplicated() {
    let mut fixture = Fixture::new();
    for field in [
        "CharacterSaveParameterMap",
        "ItemContainerSaveData",
        "CharacterContainerSaveData",
        "DynamicItemSaveData",
    ] {
        let source_entries = map_entries(&fixture.source_level, field).clone();
        for (key, value) in source_entries.0 {
            map_entries_mut(&mut fixture.target_level, field).insert(key, value);
        }
    }

    let result = build_full_character_transfer(fixture.input())
        .expect("世界迁移后的源记录应被重新绑定到目标身份");

    let cspm = cspm_entries(&result.target_level);
    assert_eq!(
        cspm.len(),
        2,
        "结果只能保留一个目标玩家和一个随角色转移的帕鲁，不能保留源身份副本"
    );
    assert!(!cspm.keys().any(|key| {
        key_player_uid(key) == Some(fixture.source_uid)
            && key_instance(key) == Some(fixture.source_instance)
    }));
    for source_container in fixture.source_inventory {
        assert!(
            !map_entries(&result.target_level, "ItemContainerSaveData")
                .keys()
                .any(|key| property_guid(key) == Some(source_container)),
            "源背包容器不得作为重复副本残留"
        );
    }
}

struct Fixture {
    source_uid: Guid,
    target_uid: Guid,
    source_instance: Guid,
    target_instance: Guid,
    target_pal_container: Guid,
    target_otomo_container: Guid,
    source_inventory: [Guid; 5],
    target_inventory: [Guid; 5],
    dynamic_item: Guid,
    selection: TransferSelection,
    source_level: GvasFile,
    target_level: GvasFile,
    source_player: GvasFile,
    target_player: GvasFile,
    source_dps: GvasFile,
}

impl Fixture {
    fn new() -> Self {
        let source_uid = guid(0x11);
        let target_uid = guid(0x22);
        let source_instance = guid(0x31);
        let target_instance = guid(0x32);
        let source_group = guid(0x41);
        let target_group = guid(0x42);
        let source_pal_container = guid(0x51);
        let target_pal_container = guid(0x52);
        let source_otomo_container = guid(0x53);
        let target_otomo_container = guid(0x54);
        let source_inventory = [guid(0x61), guid(0x62), guid(0x63), guid(0x64), guid(0x65)];
        let target_inventory = [guid(0x71), guid(0x72), guid(0x73), guid(0x74), guid(0x75)];
        let dynamic_item = guid(0x81);
        let source_pal_instance = guid(0x91);

        let source_player = player_file(
            source_uid,
            source_instance,
            source_group,
            source_pal_container,
            source_otomo_container,
            source_inventory,
            99,
            77,
        );
        let target_player = player_file(
            target_uid,
            target_instance,
            target_group,
            target_pal_container,
            target_otomo_container,
            target_inventory,
            1,
            2,
        );

        let mut source_level = empty_level(9001);
        cspm_entries_mut(&mut source_level).insert(
            cspm_key(source_uid, source_instance),
            cspm_value(true, source_uid, 101),
        );
        cspm_entries_mut(&mut source_level).insert(
            cspm_key(Guid::from_u8([0; 16]), source_pal_instance),
            cspm_value(false, source_uid, 401),
        );
        for (index, container) in source_inventory.iter().enumerate() {
            insert_map_value(
                &mut source_level,
                "ItemContainerSaveData",
                *container,
                200 + index as i32,
                Some(dynamic_item),
            );
        }
        insert_map_value(
            &mut source_level,
            "CharacterContainerSaveData",
            source_pal_container,
            501,
            Some(source_pal_instance),
        );
        insert_map_value(
            &mut source_level,
            "CharacterContainerSaveData",
            source_otomo_container,
            502,
            Some(source_pal_instance),
        );
        insert_map_value(
            &mut source_level,
            "DynamicItemSaveData",
            dynamic_item,
            601,
            None,
        );

        let mut target_level = empty_level(7777);
        cspm_entries_mut(&mut target_level).insert(
            cspm_key(target_uid, target_instance),
            cspm_value(true, target_uid, 1),
        );
        for (index, container) in target_inventory.iter().enumerate() {
            insert_map_value(
                &mut target_level,
                "ItemContainerSaveData",
                *container,
                300 + index as i32,
                None,
            );
        }
        insert_map_value(
            &mut target_level,
            "CharacterContainerSaveData",
            target_pal_container,
            701,
            None,
        );
        insert_map_value(
            &mut target_level,
            "CharacterContainerSaveData",
            target_otomo_container,
            702,
            None,
        );

        let source_dps = marker_file("DpsMarker", 801, Some(source_uid));
        Self {
            source_uid,
            target_uid,
            source_instance,
            target_instance,
            target_pal_container,
            target_otomo_container,
            source_inventory,
            target_inventory,
            dynamic_item,
            selection: TransferSelection {
                source_player_uid: source_uid,
                target_player_uid: target_uid,
            },
            source_level,
            target_level,
            source_player,
            target_player,
            source_dps,
        }
    }

    fn input(&self) -> FullCharacterTransferInput<'_> {
        FullCharacterTransferInput {
            source_level: &self.source_level,
            source_player: &self.source_player,
            source_dps: Some(&self.source_dps),
            target_level: &self.target_level,
            target_player: &self.target_player,
            selection: self.selection,
        }
    }

    fn inventory_pairs(&self) -> impl Iterator<Item = (Guid, Guid)> + '_ {
        self.source_inventory
            .iter()
            .copied()
            .zip(self.target_inventory.iter().copied())
    }
}

fn guid(byte: u8) -> Guid {
    Guid::from_u8([byte; 16])
}

fn zero() -> Guid {
    Guid::from_u8([0; 16])
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
    StructProperty::new(zero(), "Guid".to_string(), StructPropertyValue::Guid(value)).into()
}

fn custom_property(fields: HashableIndexMap<String, Vec<Property>>) -> Property {
    StructProperty::new(
        zero(),
        "StructProperty".to_string(),
        StructPropertyValue::CustomStruct(fields),
    )
    .into()
}

fn id_property(value: Guid) -> Property {
    let mut fields = HashableIndexMap::new();
    fields.insert("ID".to_string(), vec![guid_property(value)]);
    custom_property(fields)
}

#[allow(clippy::too_many_arguments)]
fn player_file(
    uid: Guid,
    instance: Guid,
    group: Guid,
    pal_container: Guid,
    otomo_container: Guid,
    inventory: [Guid; 5],
    character_marker: i32,
    technology_marker: i32,
) -> GvasFile {
    let mut individual = HashableIndexMap::new();
    individual.insert("PlayerUId".to_string(), vec![guid_property(uid)]);
    individual.insert("InstanceId".to_string(), vec![guid_property(instance)]);
    let mut inventory_fields = HashableIndexMap::new();
    for (name, id) in INVENTORY_FIELDS.iter().zip(inventory) {
        inventory_fields.insert((*name).to_string(), vec![id_property(id)]);
    }
    let mut save_data = HashableIndexMap::new();
    save_data.insert("PlayerUId".to_string(), vec![guid_property(uid)]);
    save_data.insert(
        "IndividualId".to_string(),
        vec![custom_property(individual)],
    );
    save_data.insert("GroupId".to_string(), vec![guid_property(group)]);
    save_data.insert(
        "PalStorageContainerId".to_string(),
        vec![id_property(pal_container)],
    );
    save_data.insert(
        "OtomoCharacterContainerId".to_string(),
        vec![id_property(otomo_container)],
    );
    save_data.insert(
        "InventoryInfo".to_string(),
        vec![custom_property(inventory_fields)],
    );
    save_data.insert(
        "CharacterMarker".to_string(),
        vec![IntProperty::new(character_marker).into()],
    );
    save_data.insert(
        "TechnologyMarker".to_string(),
        vec![IntProperty::new(technology_marker).into()],
    );
    let mut properties = HashableIndexMap::new();
    properties.insert("SaveData".to_string(), custom_property(save_data));
    test_gvas(properties)
}

fn remove_player_field(player: &mut GvasFile, field: &str) {
    let save_data = player
        .properties
        .get_mut("SaveData")
        .and_then(property_fields_mut)
        .expect("测试玩家文件应包含 SaveData");
    save_data.shift_remove(field);
}

fn empty_level(guild_marker: i32) -> GvasFile {
    let empty_map = || {
        Property::MapProperty(MapProperty::new(
            "StructProperty".to_string(),
            "StructProperty".to_string(),
            0,
            HashableIndexMap::new(),
        ))
    };
    let mut guild_entries = HashableIndexMap::new();
    guild_entries.insert(
        guid_property(guid(0xA1)),
        IntProperty::new(guild_marker).into(),
    );
    let mut world = HashableIndexMap::new();
    world.insert("CharacterSaveParameterMap".to_string(), vec![empty_map()]);
    world.insert("ItemContainerSaveData".to_string(), vec![empty_map()]);
    world.insert("CharacterContainerSaveData".to_string(), vec![empty_map()]);
    world.insert("DynamicItemSaveData".to_string(), vec![empty_map()]);
    world.insert(
        "GroupSaveDataMap".to_string(),
        vec![Property::MapProperty(MapProperty::new(
            "StructProperty".to_string(),
            "IntProperty".to_string(),
            0,
            guild_entries,
        ))],
    );
    let mut properties = HashableIndexMap::new();
    properties.insert("worldSaveData".to_string(), custom_property(world));
    test_gvas(properties)
}

fn cspm_key(uid: Guid, instance: Guid) -> Property {
    let mut fields = HashableIndexMap::new();
    fields.insert("PlayerUId".to_string(), vec![guid_property(uid)]);
    fields.insert("InstanceId".to_string(), vec![guid_property(instance)]);
    Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields))
}

fn cspm_value(is_player: bool, owner: Guid, marker: i32) -> Property {
    let mut fields = HashableIndexMap::new();
    fields.insert(
        "IsPlayer".to_string(),
        vec![BoolProperty::new(is_player).into()],
    );
    fields.insert("OwnerPlayerUId".to_string(), vec![guid_property(owner)]);
    fields.insert("Marker".to_string(), vec![IntProperty::new(marker).into()]);
    Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields))
}

fn raw_cspm_value(is_player: bool, owner: Guid, marker: i32) -> Property {
    let mut save_parameter = HashableIndexMap::new();
    save_parameter.insert(
        "IsPlayer".to_string(),
        vec![BoolProperty::new(is_player).into()],
    );
    save_parameter.insert("OwnerPlayerUId".to_string(), vec![guid_property(owner)]);
    save_parameter.insert("Marker".to_string(), vec![IntProperty::new(marker).into()]);
    let save_parameter: Property = StructProperty::new(
        zero(),
        "PalIndividualCharacterSaveParameter".to_string(),
        StructPropertyValue::CustomStruct(save_parameter),
    )
    .into();
    let mut writer = Cursor::new(Vec::new());
    let hints = HashMap::new();
    let mut stack = Vec::new();
    let versions = HashableIndexMap::new();
    let mut options = PropertyOptions {
        hints: &hints,
        properties_stack: &mut stack,
        custom_versions: &versions,
    };
    writer.write_string("SaveParameter").unwrap();
    save_parameter
        .write(&mut writer, true, &mut options)
        .unwrap();
    writer.write_string("None").unwrap();
    let mut fields = HashableIndexMap::new();
    fields.insert(
        "RawData".to_string(),
        vec![Property::ArrayProperty(ArrayProperty::Bytes {
            bytes: writer.into_inner(),
        })],
    );
    Property::StructPropertyValue(StructPropertyValue::CustomStruct(fields))
}

fn marker_file(name: &str, marker: i32, owner: Option<Guid>) -> GvasFile {
    let mut fields = HashableIndexMap::new();
    fields.insert(name.to_string(), vec![IntProperty::new(marker).into()]);
    if let Some(owner) = owner {
        fields.insert("OwnerPlayerUId".to_string(), vec![guid_property(owner)]);
    }
    let mut properties = HashableIndexMap::new();
    properties.insert("SaveParameterArray".to_string(), custom_property(fields));
    test_gvas(properties)
}

fn insert_map_value(
    level: &mut GvasFile,
    field: &str,
    id: Guid,
    marker: i32,
    reference: Option<Guid>,
) {
    let mut fields = HashableIndexMap::new();
    fields.insert("Marker".to_string(), vec![IntProperty::new(marker).into()]);
    if let Some(reference) = reference {
        fields.insert("ReferenceId".to_string(), vec![guid_property(reference)]);
    }
    map_entries_mut(level, field).insert(guid_property(id), custom_property(fields));
}

fn world_fields(level: &GvasFile) -> &HashableIndexMap<String, Vec<Property>> {
    match &level.properties["worldSaveData"] {
        Property::StructProperty(value) => match &value.value {
            StructPropertyValue::CustomStruct(fields) => fields,
            _ => panic!("worldSaveData 必须是 CustomStruct"),
        },
        _ => panic!("worldSaveData 必须是 StructProperty"),
    }
}

fn world_fields_mut(level: &mut GvasFile) -> &mut HashableIndexMap<String, Vec<Property>> {
    match level.properties.get_mut("worldSaveData").unwrap() {
        Property::StructProperty(value) => match &mut value.value {
            StructPropertyValue::CustomStruct(fields) => fields,
            _ => panic!("worldSaveData 必须是 CustomStruct"),
        },
        _ => panic!("worldSaveData 必须是 StructProperty"),
    }
}

fn map_entries<'a>(level: &'a GvasFile, field: &str) -> &'a HashableIndexMap<Property, Property> {
    match &world_fields(level)[field][0] {
        Property::MapProperty(MapProperty::Properties { value, .. }) => value,
        _ => panic!("{field} 必须是 MapProperty::Properties"),
    }
}

fn map_entries_mut<'a>(
    level: &'a mut GvasFile,
    field: &str,
) -> &'a mut HashableIndexMap<Property, Property> {
    match &mut world_fields_mut(level).get_mut(field).unwrap()[0] {
        Property::MapProperty(MapProperty::Properties { value, .. }) => value,
        _ => panic!("{field} 必须是 MapProperty::Properties"),
    }
}

fn cspm_entries(level: &GvasFile) -> &HashableIndexMap<Property, Property> {
    map_entries(level, "CharacterSaveParameterMap")
}

fn cspm_entries_mut(level: &mut GvasFile) -> &mut HashableIndexMap<Property, Property> {
    map_entries_mut(level, "CharacterSaveParameterMap")
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

fn key_player_uid(key: &Property) -> Option<Guid> {
    property_fields(key)?
        .get("PlayerUId")?
        .first()
        .and_then(property_guid)
}

fn key_instance(key: &Property) -> Option<Guid> {
    property_fields(key)?
        .get("InstanceId")?
        .first()
        .and_then(property_guid)
}

fn value_is_player(value: &Property) -> bool {
    property_fields(value)
        .and_then(|fields| fields.get("IsPlayer"))
        .and_then(|values| values.first())
        .map(|property| matches!(property, Property::BoolProperty(value) if value.value))
        .unwrap_or(false)
}

fn value_marker(value: &Property) -> i32 {
    property_fields(value)
        .and_then(|fields| fields.get("Marker"))
        .and_then(|values| values.first())
        .and_then(|property| match property {
            Property::IntProperty(value) => Some(value.value),
            _ => None,
        })
        .unwrap()
}

fn value_owner(value: &Property) -> Option<Guid> {
    property_fields(value)?
        .get("OwnerPlayerUId")?
        .first()
        .and_then(property_guid)
}

fn world_field<'a>(level: &'a GvasFile, field: &str) -> &'a Property {
    &world_fields(level)[field][0]
}

fn player_fields(player: &GvasFile) -> &HashableIndexMap<String, Vec<Property>> {
    property_fields(&player.properties["SaveData"]).unwrap()
}

fn player_field<'a>(player: &'a GvasFile, field: &str) -> &'a Vec<Property> {
    &player_fields(player)[field]
}

fn player_int(player: &GvasFile, field: &str) -> i32 {
    match &player_field(player, field)[0] {
        Property::IntProperty(value) => value.value,
        _ => panic!("{field} 必须是 IntProperty"),
    }
}

fn nested_int(gvas: &GvasFile, root: &str, field: &str) -> i32 {
    property_fields(&gvas.properties[root])
        .and_then(|fields| fields.get(field))
        .and_then(|values| values.first())
        .and_then(|property| match property {
            Property::IntProperty(value) => Some(value.value),
            _ => None,
        })
        .unwrap()
}

fn nested_guid(gvas: &GvasFile, root: &str, field: &str) -> Guid {
    property_fields(&gvas.properties[root])
        .and_then(|fields| fields.get(field))
        .and_then(|values| values.first())
        .and_then(property_guid)
        .unwrap()
}

fn map_value_marker(level: &GvasFile, field: &str, id: Guid) -> i32 {
    map_entries(level, field)
        .iter()
        .find_map(|(key, value)| (property_guid(key) == Some(id)).then(|| value_marker(value)))
        .unwrap_or_else(|| panic!("{field} 中应存在 {id}"))
}

fn remove_map_entry(level: &mut GvasFile, field: &str, id: Guid) {
    let entries = map_entries_mut(level, field);
    let key = entries
        .keys()
        .find(|key| property_guid(key) == Some(id))
        .cloned()
        .unwrap();
    entries.shift_remove(&key);
}
