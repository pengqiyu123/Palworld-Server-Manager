//! F5 · T03 整包世界迁移 + 世界摘要（L1/L2 枚举）。
//!
//! - `f5_world_summary_impl`：枚举世界内玩家（Level.sav CharacterSaveParameterMap，对齐同行实现）与公会
//!   （Level.sav GroupSaveDataMap），用于前端 L1/L2 列表与角色转移选择。
//!   解析采用防御式：任一层级解析失败仅导致该部分为空，不会整体报错。
//! - `migrate_world_impl`：将源世界目录整包拷贝到目标世界目录（深层拷贝），
//!   迁移前对目标世界做备份（回滚由 `save_edit.rs` 统一编排）。

use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use gvas::cursor_ext::ReadExt;
use gvas::game_version::{DeserializedGameVersion, PalworldCompressionType};
use gvas::properties::array_property::ArrayProperty;
use gvas::properties::int_property::BytePropertyValue;
use gvas::properties::map_property::MapProperty;
use gvas::properties::struct_property::{StructProperty, StructPropertyValue};
use gvas::properties::{Property, PropertyOptions};
use gvas::types::map::HashableIndexMap;
use gvas::types::Guid;
use gvas::GvasFile;
use gvas::GvasHeader;

use crate::save_edit::models::{GuildEntry, MigrateRequest, PlayerEntry, WorldSummary};
use crate::save_edit::path_util;
use crate::save_edit::sav_io::{self, SavFile};

/// 从字节流读取小端 u32（避免额外引入 `byteorder` 依赖）。
fn read_u32_le<R: Read>(cursor: &mut R) -> Result<u32, String> {
    let mut b = [0u8; 4];
    cursor
        .read_exact(&mut b)
        .map_err(|e| format!("读取 u32 失败: {}", e))?;
    Ok(u32::from_le_bytes(b))
}

/// 读取一个 FString（仅用于属性头中的类型名等，绝不会是 null）。
fn read_fstring<R: Read + Seek>(cursor: &mut R) -> Result<String, String> {
    cursor
        .read_string()
        .map_err(|e| format!("读取 FString 失败: {}", e))
}

/// 跳过「非目标」巨型子字段（如 `MapObjectSaveData`）：
///
/// 调用方已读取子字段的 `name` / `type`，本函数继续读取属性头
/// （`length(u32)` / `array_index(u32)` / 类型特定头 / `terminator(u8)`），
/// 随后 `seek` 越过 `body_start + length` 个字节，使游标落在下一个子字段的 `name` 处。
///
/// 之所以需要手动跳过：这些字段内部嵌套的 struct 缺少 hint，`Property::new` 解析会
/// 整体失败且游标位置未知；而我们只关心 CSPM / GSM / CCSD / GameTimeSaveData 四个字段，
/// 其余字段必须可安全跳过才能继续解析后续目标字段。
///
/// 返回 `Err` 表示无法读取头（文件损坏或类型头格式不符），此时调用方应放弃后续子字段。
fn skip_property_body<R: Read + Seek>(cursor: &mut R, sub_type: &str) -> Result<(), String> {
    let length = read_u32_le(cursor)? as u64; // 体长度（body 字节数）
    let _array_index = read_u32_le(cursor)?; // array_index（期望为 0）
    match sub_type {
        "StructProperty" => {
            // 头：type_name(FString) + guid(16) + terminator(u8)
            let _type_name = read_fstring(cursor)?;
            let mut guid = [0u8; 16];
            cursor
                .read_exact(&mut guid)
                .map_err(|e| format!("读取 StructProperty guid 失败: {}", e))?;
            let mut term = [0u8; 1];
            cursor
                .read_exact(&mut term)
                .map_err(|e| format!("读取 StructProperty terminator 失败: {}", e))?;
        }
        "MapProperty" => {
            // 头：key_type(FString) + value_type(FString) + terminator(u8)
            let _key_type = read_fstring(cursor)?;
            let _value_type = read_fstring(cursor)?;
            let mut term = [0u8; 1];
            cursor
                .read_exact(&mut term)
                .map_err(|e| format!("读取 MapProperty terminator 失败: {}", e))?;
        }
        "ArrayProperty" | "SetProperty" => {
            // 头：property_type(FString) + terminator(u8)
            let _inner = read_fstring(cursor)?;
            let mut term = [0u8; 1];
            cursor
                .read_exact(&mut term)
                .map_err(|e| format!("读取 ArrayProperty terminator 失败: {}", e))?;
        }
        _ => {
            // 标量 / 字符串类型：仅 terminator(u8)，体长度已含内部 size/separator。
            let mut term = [0u8; 1];
            cursor
                .read_exact(&mut term)
                .map_err(|e| format!("读取标量 terminator 失败: {}", e))?;
        }
    }
    let body_start = cursor
        .stream_position()
        .map_err(|e| format!("读取流位置失败: {}", e))?;
    cursor
        .seek(SeekFrom::Start(body_start + length))
        .map_err(|e| format!("seek 跳过字段失败: {}", e))?;
    Ok(())
}

/// 仅解析 Level.sav 中 `worldSaveData` 内的关键子字段
/// （`CharacterSaveParameterMap` / `GroupSaveDataMap` /
/// `CharacterContainerSaveData` / `GameTimeSaveData`），遇到后续无法解析的巨型字段
/// （`MapObjectSaveData` 等）直接 `seek` 跳过，避免 `gvas` crate 对 Palworld 新结构的
/// 个别嵌套 Guid 键产生字节错位而整体失败。
///
/// 返回构造好的 `GvasFile`（仅含 `worldSaveData` 一个顶层属性）。任何一层解析失败都返回
/// `None`（防御式：调用方据此回退为空列表，绝不整体 panic）。
fn parse_level_gvas(level_path: &Path) -> Option<GvasFile> {
    let sav = SavFile::load(level_path).ok()?;
    let mut cursor = Cursor::new(sav.raw.clone());

    // 1) GVAS 头（magic / 版本 / custom versions）。
    let header = GvasHeader::read(&mut cursor).ok()?;

    // 2) hints：仅覆盖我们关心的 Map 键/值类型。
    //
    // ⚠️ 关键：GroupSaveDataMap / CharacterContainerSaveData 的 Map 键是 **裸 16 字节 GUID**
    //    （无 StructProperty 头：type_name/guid/terminator 皆缺），gvas 在 Map 键处走
    //    `read_body` 时完全依赖 hint 决定如何解析；hint 必须是 "Guid" 才能正确读 16 字节。
    //    若写成其它名（如 "GroupSaveDataMapKey"），gvas 会改走 `read_custom` 把前 4 字节
    //    当 FString 长度 → 字节错位 / Invalid string size，整个 GSM 解析失败。
    //    而 CharacterSaveParameterMap 的键是「自描述结构体」（含 InstanceId/PlayerUId 字段），
    //    必须走 `read_custom`，hint 用非 Guid 名。
    let mut hints: HashMap<String, String> = HashMap::new();
    hints.insert(
        "worldSaveData.StructProperty.CharacterSaveParameterMap.MapProperty.Key.StructProperty"
            .to_string(),
        "CharacterSaveParameterMapKey".to_string(),
    );
    hints.insert(
        "worldSaveData.StructProperty.CharacterSaveParameterMap.MapProperty.Value.StructProperty"
            .to_string(),
        "CharacterSaveParameter".to_string(),
    );
    hints.insert(
        "worldSaveData.StructProperty.GroupSaveDataMap.MapProperty.Key.StructProperty".to_string(),
        "Guid".to_string(),
    );
    hints.insert(
        "worldSaveData.StructProperty.GroupSaveDataMap.MapProperty.Value.StructProperty"
            .to_string(),
        "GroupSaveData".to_string(),
    );
    hints.insert(
        "worldSaveData.StructProperty.CharacterContainerSaveData.MapProperty.Key.StructProperty"
            .to_string(),
        "CharacterContainerSaveDataKey".to_string(),
    );
    hints.insert(
        "worldSaveData.StructProperty.CharacterContainerSaveData.MapProperty.Value.StructProperty"
            .to_string(),
        "ContainerSaveData".to_string(),
    );

    let mut stack: Vec<String> = Vec::new();
    let mut options = PropertyOptions {
        hints: &hints,
        properties_stack: &mut stack,
        custom_versions: header.get_custom_versions(),
    };

    // 3) 顶层循环：定位 `worldSaveData`，手动读其 StructProperty 头以获得 body 长度，
    //    再逐子字段解析，解析完所需字段后 seek 跳过尾部巨型字段。
    let needed: [&str; 4] = [
        "CharacterSaveParameterMap",
        "GroupSaveDataMap",
        "CharacterContainerSaveData",
        "GameTimeSaveData",
    ];
    let mut collected: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut wsd_fields: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
    let mut wsd_type_name: String = String::new();

    loop {
        let property_name = cursor.read_string().ok()?;
        if property_name == "None" {
            break;
        }
        let property_type = cursor.read_string().ok()?;
        options.properties_stack.push(property_name.clone());

        if property_name == "worldSaveData" {
            // 手动读 StructProperty 头：length / array_index / type_name / guid / terminator。
            let length = read_u32_le(&mut cursor).ok()?;
            let _array_index = read_u32_le(&mut cursor).ok()?;
            let type_name = cursor.read_string().ok()?;
            let _guid = cursor.read_guid().ok()?;
            let mut term = [0u8; 1];
            cursor.read_exact(&mut term).ok()?;
            let _terminator = term[0];
            let body_start = cursor.stream_position().ok()?;
            let body_end = body_start + length as u64;
            wsd_type_name = type_name;

            // 复刻 gvas 栈：worldSaveData 自身是 StructProperty（Property::new 会再压入子字段类型）。
            options.properties_stack.push("StructProperty".to_string());
            loop {
                let sub_name = cursor.read_string().ok()?;
                if sub_name == "None" {
                    break;
                }
                let sub_type = cursor.read_string().ok()?;

                // 仅我们关心的四个字段才真正解析；其余（如 MapObjectSaveData 等巨型字段）
                // 内部嵌套 struct 缺少 hint 会整体失败，必须读头后 seek 跳过并继续。
                let is_target = matches!(
                    sub_name.as_str(),
                    "CharacterSaveParameterMap"
                        | "GroupSaveDataMap"
                        | "CharacterContainerSaveData"
                        | "GameTimeSaveData"
                );

                options.properties_stack.push(sub_name.clone());
                let header_pos = cursor.stream_position().ok()?; // name+type 之后，字段头起点
                if is_target {
                    match Property::new(&mut cursor, &sub_type, true, &mut options, None) {
                        Ok(sub_prop) => {
                            wsd_fields.insert(sub_name.clone(), vec![sub_prop]);
                            collected.insert(sub_name.clone());
                        }
                        Err(e) => {
                            // 目标字段解析失败（防御式）：回退到字段头起点，读头后 seek 越过，
                            // 避免游标错位传染后续字段。
                            let _ = cursor.seek(SeekFrom::Start(header_pos));
                            if skip_property_body(&mut cursor, &sub_type).is_err() {
                                options.properties_stack.pop();
                                break;
                            }
                        }
                    }
                } else {
                    // 非目标字段：读头取长度后 seek 越过。
                    if skip_property_body(&mut cursor, &sub_type).is_err() {
                        options.properties_stack.pop();
                        break;
                    }
                }
                options.properties_stack.pop();

                // 四个目标字段全部收集完毕即可提前结束（其余同层字段无需解析）。
                if needed.iter().all(|n| collected.contains(*n)) {
                    break;
                }
            }
            options.properties_stack.pop(); // "StructProperty"

            // 跳过 worldSaveData body 剩余部分（未解析的巨型字段）。
            let _ = cursor.seek(SeekFrom::Start(body_end));
            break; // worldSaveData 是唯一关心的顶层属性
        } else {
            // 其它顶层属性（通常不会出现，或位于 worldSaveData 之前）：尽力解析后丢弃。
            let _ = Property::new(&mut cursor, &property_type, true, &mut options, None);
        }
        options.properties_stack.pop();
    }

    let wsd = Property::StructProperty(StructProperty::new(
        Guid([0u8; 16]),
        wsd_type_name,
        StructPropertyValue::CustomStruct(wsd_fields),
    ));
    let mut props: HashableIndexMap<String, Property> = HashableIndexMap::new();
    props.insert("worldSaveData".to_string(), wsd);

    Some(GvasFile {
        deserialized_game_version: DeserializedGameVersion::Palworld(PalworldCompressionType::None),
        header,
        properties: props,
    })
}

/// 取 `MapProperty::Properties` 的键值表（公会/角色 Map 通常是 Properties 变体）。
fn as_props_map(p: &Property) -> Option<&HashableIndexMap<Property, Property>> {
    match p {
        Property::MapProperty(MapProperty::Properties { value, .. }) => Some(value),
        _ => None,
    }
}

/// 从 `Property` 取出其内部 `StructPropertyValue`。
///
/// ⚠️ gvas 在 Map 的 **键/值** 处走 `include_header=false`，返回的是
/// `Property::StructPropertyValue` 裸变体（而非 `Property::StructProperty`）；
/// 而顶层属性 / 嵌套字段（`include_header=true`）则是 `Property::StructProperty`。
/// 两个包装都要兼容，否则对 Map 条目取字段会整体失败。
fn as_struct_value(p: &Property) -> Option<&StructPropertyValue> {
    match p {
        Property::StructProperty(s) => Some(&s.value),
        Property::StructPropertyValue(spv) => Some(spv),
        _ => None,
    }
}

/// 从 Map 键取 GUID 字符串。
///
/// 兼容两种键形态：
/// - 纯 `Guid`（`StructPropertyValue::Guid`，GSM 键经 hint="Guid" 解析所得）；
/// - 自描述 `CustomStruct` 键（CSPM 键经 hint 解析所得）：
///   优先取名为 `GroupId` / `InstanceId` / `PlayerUId` 的 Guid 字段，否则取第一个 Guid 字段。
fn extract_key_guid(k: &Property) -> String {
    match as_struct_value(k) {
        Some(StructPropertyValue::Guid(g)) => g.to_string(),
        Some(StructPropertyValue::CustomStruct(map)) => {
            for name in ["GroupId", "InstanceId", "PlayerUId"] {
                if let Some(props) = map.get(name) {
                    if let Some(p) = props.first() {
                        if let Some(StructPropertyValue::Guid(g)) = as_struct_value(p) {
                            return g.to_string();
                        }
                    }
                }
            }
            for props in map.values() {
                if let Some(p) = props.first() {
                    if let Some(StructPropertyValue::Guid(g)) = as_struct_value(p) {
                        return g.to_string();
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// 从公会值结构取 RawData 二进制块（ArrayProperty::Bytes）。
///
/// 兼容 `Property::StructProperty` 与 `Property::StructPropertyValue` 两种包装。
fn extract_rawdata_bytes(v: &Property) -> Option<Vec<u8>> {
    if let Some(StructPropertyValue::CustomStruct(map)) = as_struct_value(v) {
        if let Some(raw) = map.get("RawData") {
            if let Some(first) = raw.first() {
                if let Property::ArrayProperty(ArrayProperty::Bytes { bytes }) = first {
                    return Some(bytes.clone());
                }
            }
        }
    }
    None
}

/// 标准 GUID registry 格式（前三组小端）：用于与磁盘文件名对齐。
///
/// gvas 的 `Guid::to_string()` / `to_u8()` 采用不同字节序（如 `4E239D4F` 经 gvas
/// 输出为 `4F9D234E`），无法与磁盘玩家文件 `Players/<UID>.sav` 的 stem 直接对应。
/// 本函数按 Windows 注册表 GUID 约定（前 3 组小端、后 2 组大端）重新格式化，
/// 使 `player_uid` / `guid` 与磁盘文件名一致。
fn guid_std(raw: &[u8; 16]) -> String {
    let g1 = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let g2 = u16::from_le_bytes([raw[4], raw[5]]);
    let g3 = u16::from_le_bytes([raw[6], raw[7]]);
    let g4 = u16::from_be_bytes([raw[8], raw[9]]);
    let g5 = u64::from_be_bytes([raw[10], raw[11], raw[12], raw[13], raw[14], raw[15], 0, 0]);
    format!("{:08X}{:04X}{:04X}{:04X}{:012X}", g1, g2, g3, g4, g5)
}

/// 从 CSPM Map 键（`FPalInstanceId` 自定义结构体）取出 `(PlayerUId, InstanceId)`
/// 两个裸 GUID。键形态兼容 `Property::StructProperty` 与 `Property::StructPropertyValue`
/// 两种包装；任一 GUID 缺失返回 `None`（调用方据此跳过该条目）。
fn key_player_instance(k: &Property) -> Option<([u8; 16], [u8; 16])> {
    let km = match as_struct_value(k) {
        Some(StructPropertyValue::CustomStruct(m)) => m,
        _ => return None,
    };
    let pu =
        km.get("PlayerUId")
            .and_then(|x| x.first())
            .and_then(|p| match as_struct_value(p) {
                Some(StructPropertyValue::Guid(g)) => Some(g.to_u8()),
                _ => None,
            })?;
    let ii =
        km.get("InstanceId")
            .and_then(|x| x.first())
            .and_then(|p| match as_struct_value(p) {
                Some(StructPropertyValue::Guid(g)) => Some(g.to_u8()),
                _ => None,
            })?;
    Some((pu, ii))
}

/// 解析 CSPM 值 / GSM 组 的 RawData 裸属性流（无 GVAS 头），返回顶层字段表。
///
/// RawData 是纯属性流，必须用 `Property::new(include_header=true)` + 父 GVAS 的
/// `custom_versions` 重新解析（不能走 `GvasFile::read_with_hints`，后者要求 GVAS 头）。
/// 任一层字段解析失败仅跳过并继续（防御式），绝不整体报错。
fn parse_rawdata_stream(
    bytes: &[u8],
    custom_versions: &HashableIndexMap<Guid, u32>,
) -> HashableIndexMap<String, Vec<Property>> {
    let mut cursor = Cursor::new(bytes.to_vec());
    let hints: HashMap<String, String> = HashMap::new();
    let mut stack: Vec<String> = Vec::new();
    let mut options = PropertyOptions {
        hints: &hints,
        properties_stack: &mut stack,
        custom_versions,
    };
    let mut top: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
    loop {
        let name = match cursor.read_string() {
            Ok(n) => n,
            Err(_) => break,
        };
        if name == "None" {
            break;
        }
        let ptype = match cursor.read_string() {
            Ok(t) => t,
            Err(_) => break,
        };
        let header_pos = match cursor.stream_position() {
            Ok(p) => p,
            Err(_) => break,
        };
        match Property::new(&mut cursor, &ptype, true, &mut options, None) {
            Ok(p) => {
                top.entry(name.clone()).or_default().push(p);
            }
            Err(_) => {
                if cursor.seek(SeekFrom::Start(header_pos)).is_err() {
                    break;
                }
                if skip_property_body(&mut cursor, &ptype).is_err() {
                    break;
                }
            }
        }
    }
    top
}

/// 容忍式读取整数属性值（覆盖 gvas 全部 int 变体），失败 / 非整数返回 0。
fn read_int_value(p: Option<&Property>) -> u32 {
    match p {
        Some(Property::Int8Property(v)) => v.value as u32,
        Some(Property::Int16Property(v)) => v.value as u32,
        Some(Property::IntProperty(v)) => v.value as u32,
        Some(Property::Int64Property(v)) => v.value as u32,
        Some(Property::UInt16Property(v)) => v.value as u32,
        Some(Property::UInt32Property(v)) => v.value as u32,
        Some(Property::UInt64Property(v)) => v.value as u32,
        Some(Property::ByteProperty(v)) => match v.value {
            BytePropertyValue::Byte(b) => b as u32,
            _ => 0,
        },
        _ => 0,
    }
}

/// 从公会 RawData 二进制块中尽力解码公会名（group_id 之后紧跟 FString）。
/// 任何解析异常都返回空串，绝不影响整体摘要。
fn decode_guild_name(bytes: &[u8]) -> String {
    let mut cur = 16usize; // 跳过 group_id (16 字节)
    if bytes.len() < cur + 4 {
        return String::new();
    }
    let len = i32::from_le_bytes(bytes[cur..cur + 4].try_into().unwrap());
    cur += 4;
    if len >= 0 {
        let n = len as usize;
        if cur + n > bytes.len() {
            return String::new();
        }
        String::from_utf8_lossy(&bytes[cur..cur + n]).to_string()
    } else {
        let units = (-(len + 1)) as usize;
        if cur + units * 2 > bytes.len() {
            return String::new();
        }
        let mut s = String::new();
        for i in 0..units {
            let u = u16::from_le_bytes(bytes[cur + i * 2..cur + i * 2 + 2].try_into().unwrap());
            s.push(char::from_u32(u as u32).unwrap_or('\u{FFFD}'));
        }
        s
    }
}

/// 从 Level.sav 解析公会列表（防御式，失败返回空）。
fn read_guilds(level_path: &Path) -> Vec<GuildEntry> {
    let gvas = match parse_level_gvas(level_path) {
        Some(g) => g,
        None => return Vec::new(),
    };

    let mut guilds: Vec<GuildEntry> = Vec::new();
    if let Some(wsd) = sav_io::top_field(&gvas, "worldSaveData") {
        if let Some(csv) = sav_io::struct_value(wsd) {
            if let Some(fields) = sav_io::custom_fields(csv) {
                if let Some(gm) = sav_io::field(fields, "GroupSaveDataMap") {
                    if let Some(props_map) = as_props_map(gm) {
                        for (k, v) in props_map.iter() {
                            let guild_id = extract_key_guid(k);
                            let name = extract_rawdata_bytes(v)
                                .map(|b| decode_guild_name(&b))
                                .unwrap_or_default();
                            guilds.push(GuildEntry {
                                guild_id,
                                name,
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }
    }
    guilds
}

/// 生成世界摘要（玩家 + 公会）。按世界名解析（仅服务器 SaveGames 根下的世界）。
pub fn f5_world_summary_impl(world: &str) -> Result<WorldSummary, String> {
    let data_dir = path_util::world_data_dir(world)?;
    let mut summary = read_world_summary_from(&data_dir);
    summary.world_name = world.to_string();
    Ok(summary)
}

/// 按给定世界目录路径生成摘要（本地单机 / 服务器通用，不依赖 SaveGames 根解析）。
/// 用于前端「本地存档」详情弹窗：本地世界不在服务器 SaveGames 根下，必须按真实路径解析。
pub fn f5_world_summary_by_path_impl(path: &str) -> Result<WorldSummary, String> {
    let p = Path::new(path);
    let data_dir = path_util::find_world_data_dir(p)
        .ok_or_else(|| format!("未找到世界数据(Level.sav)：{}", p.display()))?;
    Ok(read_world_summary_from(&data_dir))
}

/// 从已定位的世界数据层目录读取玩家 + 公会摘要（防御式，单层失败仅该部分为空）。
///
/// 玩家读源已改为 **Level.sav 权威源**（`read_players_from_level`）：不再逐个解析
/// `Players/*.sav` 顶层 `SaveData`（该路径对真实存档从未成功，且信息不完整），而是
/// 从 `Level.sav` 的 `GroupSaveDataMap` (Guild) 与 `CharacterSaveParameterMap` 一次性枚举。
fn read_world_summary_from(data_dir: &Path) -> WorldSummary {
    // 玩家：Level.sav 权威源（缺 Level.sav 时返回空列表，绝不 panic）。
    let level_path = data_dir.join("Level.sav");
    let mut players = if level_path.is_file() {
        read_players_from_level(&level_path)
    } else {
        Vec::new()
    };
    players.sort_by(|a, b| a.nickname.cmp(&b.nickname));

    // 公会：解析 Level.sav
    let guilds = if level_path.is_file() {
        read_guilds(&level_path)
    } else {
        Vec::new()
    };

    WorldSummary {
        world_name: String::new(),
        players,
        guilds,
    }
}

/// 从 `Level.sav` 权威源枚举世界内玩家列表。
///
/// 数据来源（对齐 `fix_host_save.py::_build_player_list_from_level`）：
/// `worldSaveData.CharacterSaveParameterMap` 中 `IsPlayer == true` 的角色，
/// 其 `PlayerUId` / `InstanceId` 取自 Map 键（`FPalInstanceId`），
/// `NickName` / `Level` / `IsPlayer` 取自 CSPM 值的 `RawData` 裸流中的
/// `SaveParameter` 结构体（`Level` 缺键默认 1，`ByteProperty` 亦兼容）。
///
/// `guild_id` / `is_host` / `last_online` / `pal_count` 为 best-effort 留空 / 0：
/// 公会 `GroupSaveDataMap` 的 `RawData.players[]` 在 Rust/gvas 下不可结构化解析，
/// 故暂不取。任一层解析失败仅使该玩家被跳过，绝不整体报错 / panic。
fn read_players_from_level(level_path: &Path) -> Vec<PlayerEntry> {
    let gvas = match parse_level_gvas(level_path) {
        Some(g) => g,
        None => return Vec::new(),
    };

    // 防御式：各层取不到即回退为空列表，绝不整体 panic。
    let cspm = match sav_io::top_field(&gvas, "worldSaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .and_then(|fields| sav_io::field(fields, "CharacterSaveParameterMap"))
    {
        Some(m) => m,
        None => return Vec::new(),
    };
    let props_map = match as_props_map(cspm) {
        Some(m) => m,
        None => return Vec::new(),
    };

    let custom_versions = gvas.header.get_custom_versions().clone();
    let mut players: Vec<PlayerEntry> = Vec::new();

    for (k, v) in props_map.iter() {
        // 键：FPalInstanceId -> (PlayerUId, InstanceId) 裸 GUID。
        let (pu_raw, ii_raw) = match key_player_instance(k) {
            Some(x) => x,
            None => continue,
        };

        // 值：FPalSaveParameter 的 RawData 裸流（内含 SaveParameter 结构体）。
        let bytes = match extract_rawdata_bytes(v) {
            Some(b) => b,
            None => continue,
        };
        let top = parse_rawdata_stream(&bytes, &custom_versions);

        // SaveParameter 内嵌结构体（仅玩家角色含该结构）。
        let sp_inner = match top
            .get("SaveParameter")
            .and_then(|x| x.first())
            .and_then(|p| match as_struct_value(p) {
                Some(StructPropertyValue::CustomStruct(m)) => Some(m),
                _ => None,
            }) {
            Some(m) => m,
            None => continue,
        };

        // 仅 `IsPlayer == true` 计入玩家列表（Pal 等 NPC 跳过）。
        let is_player = sp_inner
            .get("IsPlayer")
            .and_then(|x| x.first())
            .map(|p| matches!(p, Property::BoolProperty(b) if b.value))
            .unwrap_or(false);
        if !is_player {
            continue;
        }

        let nickname = sp_inner
            .get("NickName")
            .and_then(|x| x.first())
            .and_then(|p| match p {
                Property::StrProperty(s) => s.value.clone(),
                _ => None,
            })
            .unwrap_or_default();

        let level = if sp_inner.contains_key("Level") {
            read_int_value(sp_inner.get("Level").and_then(|x| x.first()))
        } else {
            1
        };

        players.push(PlayerEntry {
            player_uid: guid_std(&pu_raw),
            instance_id: guid_std(&ii_raw),
            guid: guid_std(&pu_raw),
            nickname,
            level,
            guild_id: None,
            pal_count: 0,
            last_online: String::new(),
            is_host: false,
        });
    }

    players
}

/// 整包世界迁移：把源世界目录深层拷贝到目标世界目录。
/// 返回拷贝的文件数。
///
/// - 源：`source_type == "local"` 时 `source_world` 为本地世界**绝对路径**，
///   用 `find_world_data_dir` 有界穿透定位数据层（兼容 2 层嵌套）；否则按服务器世界名解析。
/// - 目标：`world_data_dir` 正确解析专用服 `0/<GUID>` 数据层，避免平铺破坏 GUID 嵌套。
pub fn migrate_world_impl(req: &MigrateRequest) -> Result<usize, String> {
    let (save_root, _auto) = path_util::resolve_save_games_root()?;
    migrate_world_with_root(req, &save_root)
}

/// 在指定的 SaveGames 根目录内迁移世界数据层。
///
/// 将路径解析与文件复制分开，使测试能使用隔离的临时存档根，同时保证生产路径
/// 与测试路径共享完全相同的迁移逻辑。
fn migrate_world_with_root(req: &MigrateRequest, save_root: &Path) -> Result<usize, String> {
    // 源解析：local = 本地绝对路径（穿透定位数据层）；server = 服务器世界名。
    let src = if req.source_type == "local" {
        path_util::find_world_data_dir(Path::new(&req.source_world))
            .ok_or_else(|| format!("未找到本地世界数据(Level.sav)：{}", req.source_world))?
    } else {
        path_util::world_data_dir_with_root(&req.source_world, save_root)?
    };

    // 目标解析：专用服 0/<GUID> 层 —— world_data_dir 正确解析数据层，避免平铺破坏 GUID 嵌套。
    let tgt = path_util::world_data_dir_with_root(&req.target_world, save_root)?;

    if src == tgt {
        return Err("源世界与目标世界相同，无需迁移".to_string());
    }

    // 目标已存在：迁移前由调用方负责备份；此处仅做覆盖前清理。
    if tgt.exists() {
        path_util::remove_dir_recursive(&tgt)?;
    } else {
        std::fs::create_dir_all(&tgt)
            .map_err(|e| format!("创建目标目录 {} 失败: {}", tgt.display(), e))?;
    }

    let mut copied = 0usize;
    path_util::copy_dir_recursive(&src, &tgt, &mut copied)?;

    if req.delete_world_option {
        // R5：删除目标世界的 WorldOption.sav（避免旧设置污染）
        let candidates: [Option<std::path::PathBuf>; 2] = [
            Some(tgt.join("WorldOption.sav")),
            tgt.join("Level.sav")
                .parent()
                .map(|p| p.join("WorldOption.sav")),
        ];
        for cand in candidates.into_iter().flatten() {
            if cand.is_file() {
                let _ = std::fs::remove_file(&cand);
            }
        }
    }

    Ok(copied)
}

// ===========================================================================
// F5 单元测试（QA · 严过关）
// 覆盖：decode_guild_name 从公会 RawData 二进制块解码公会名、
// extract_key_guid 从 Map 键（Guid StructProperty）取 GUID。
// （整包迁移/备份回滚的基础原语已在 path_util 测试中覆盖。）
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use gvas::properties::struct_property::{StructProperty, StructPropertyValue};
    use gvas::properties::Property;
    use gvas::types::Guid;
    use std::path::PathBuf;

    #[test]
    fn decode_guild_name_ascii() {
        let name = "MyGuild";
        let mut b = vec![0u8; 16]; // group_id (16 字节，跳过)
        b.extend_from_slice(&(name.len() as i32).to_le_bytes()); // FString 长度
        b.extend_from_slice(name.as_bytes());
        assert_eq!(decode_guild_name(&b), name, "应解码 ASCII 公会名");
    }

    #[test]
    fn decode_guild_name_short_returns_empty() {
        let b = vec![0u8; 10]; // 不足 16+4 头，应安全返回空串
        assert_eq!(decode_guild_name(&b), "");
    }

    #[test]
    fn extract_key_guid_from_struct() {
        let g = Guid::from_u32([1, 2, 3, 4]);
        let p = Property::StructProperty(StructProperty::new(
            g,
            "Guid".to_string(),
            StructPropertyValue::Guid(g),
        ));
        assert_eq!(extract_key_guid(&p), g.to_string());
    }

    #[test]
    fn migrate_server_to_server_copies_data_layer_without_double_nesting() {
        let root = std::env::temp_dir().join(format!(
            "palworld_server_to_server_migration_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);

        let source_guid = "A1B2C3D4-0000-1111-2222-333344445555";
        let target_guid = "B1C2D3E4-0000-1111-2222-333344445555";
        let source_data = root.join("SourceWorld").join(source_guid);
        let target_data = root.join("TargetWorld").join(target_guid);
        std::fs::create_dir_all(source_data.join("Players")).unwrap();
        std::fs::create_dir_all(&target_data).unwrap();
        std::fs::write(source_data.join("Level.sav"), b"source-level").unwrap();
        std::fs::write(
            source_data.join("Players").join("player.sav"),
            b"source-player",
        )
        .unwrap();
        std::fs::write(target_data.join("Level.sav"), b"target-level").unwrap();

        let request = MigrateRequest {
            source_world: "SourceWorld".to_string(),
            target_world: "TargetWorld".to_string(),
            source_type: "server".to_string(),
            delete_world_option: false,
        };

        let copied = migrate_world_with_root(&request, &root).expect("服务端世界迁移应成功");

        assert_eq!(copied, 2, "应只复制数据层中的两个文件");
        assert_eq!(
            std::fs::read(target_data.join("Level.sav")).unwrap(),
            b"source-level",
            "Level.sav 应直接位于目标数据层"
        );
        assert_eq!(
            std::fs::read(target_data.join("Players").join("player.sav")).unwrap(),
            b"source-player",
            "Players/*.sav 应位于目标数据层"
        );
        assert!(
            !target_data.join(source_guid).exists(),
            "目标数据层内不得再嵌套源 GUID 目录"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // -----------------------------------------------------------------------
    // T2 正式测试：从 Level.sav 权威源枚举玩家。
    // -----------------------------------------------------------------------

    /// 真实样本：从 `Level.sav` 的 `CharacterSaveParameterMap` 枚举玩家，
    /// 断言能识别两名真实玩家（`3F5D130B...` / `4E239D4F...`），且昵称非空、等级 > 0。
    #[test]
    fn real_sample_players_from_level() {
        let src = PathBuf::from("F:/1/0/20260723-235259/1A91A61548C7B6FD7B58B2B70710F7EE");
        if !src.is_dir() {
            eprintln!("[skip] 样本不存在");
            return;
        }
        let tmp = std::env::temp_dir().join(format!("palworld_players_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let mut n = 0;
        path_util::copy_dir_recursive(&src, &tmp, &mut n).unwrap();
        let level = tmp.join("Level.sav");
        assert!(level.is_file(), "样本 Level.sav 应存在");

        let players = read_players_from_level(&level);
        let uids: Vec<String> = players.iter().map(|p| p.player_uid.clone()).collect();
        assert!(
            uids.iter().any(|u| u.starts_with("3F5D130B")),
            "应枚举出玩家 3F5D130B...，实际: {:?}",
            uids
        );
        assert!(
            uids.iter().any(|u| u.starts_with("4E239D4F")),
            "应枚举出玩家 4E239D4F...，实际: {:?}",
            uids
        );
        // 昵称非空 + 等级 > 0（仅 IsPlayer==true 计入）。
        for p in &players {
            assert!(
                !p.nickname.is_empty(),
                "玩家 {} 的 nickname 应非空",
                p.player_uid
            );
            assert!(p.level > 0, "玩家 {} 的 level 应 > 0", p.player_uid);
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// 缺 `Level.sav` 时（空目录 / 不存在）应返回空 Vec，绝不 panic。
    #[test]
    fn missing_level_yields_empty_players() {
        let tmp = std::env::temp_dir().join(format!("palworld_empty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let level = tmp.join("Level.sav"); // 不存在
        let players = read_players_from_level(&level);
        assert!(players.is_empty(), "缺 Level.sav 时应返回空 Vec");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
