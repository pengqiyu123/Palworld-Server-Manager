//! 玩家/公会修改器的数据读取与安全写入。
//!
//! 公会 RawData V1/V2 编解码改编自 PalworldSaveTools 的 GPL-3.0-or-later
//! `src/palsav` 子包：https://github.com/deafdudecomputers/PalworldSaveTools

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use gvas::cursor_ext::{ReadExt, WriteExt};
use gvas::properties::int_property::BytePropertyValue;
use gvas::properties::struct_property::StructPropertyValue;
use gvas::properties::{Property, PropertyOptions, PropertyTrait};
use gvas::types::Guid;
use serde::{Deserialize, Serialize};

use crate::backup_service::{self, BackupState, WorldClass};
use crate::save_edit::atomic_write::{self, FileMutation};
use crate::save_edit::models::{
    PlayerTechnologyPointsRequest, UpdatePlayerTechnologyPointsRequest,
};
use crate::save_edit::tech_edit;
use crate::save_edit::{path_util, sav_io, world_copy};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierGuildMember {
    pub player_uid: String,
    pub player_name: String,
    pub last_online: i64,
    pub role: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierGuild {
    pub guild_id: String,
    pub name: String,
    pub level: i32,
    pub leader_uid: String,
    pub leader_name: String,
    pub member_count: usize,
    pub base_count: usize,
    #[serde(skip)]
    pub admin_player_uid: String,
    #[serde(skip)]
    pub members: Vec<ModifierGuildMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierPlayer {
    pub player_uid: String,
    pub guid: String,
    pub nickname: String,
    pub level: u32,
    pub pal_count: u32,
    pub guild_id: Option<String>,
    pub guild_name: Option<String>,
    pub role: String,
    pub is_leader: bool,
    pub last_online: Option<String>,
    pub technology_points: i32,
    pub ancient_technology_points: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierWorldState {
    pub world_name: String,
    pub players: Vec<ModifierPlayer>,
    pub guilds: Vec<ModifierGuild>,
    pub server_running: bool,
    pub game_running: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModifierProgressPhase {
    CheckingProcesses,
    CreatingSnapshot,
    BuildingChanges,
    WritingSave,
    VerifyingSave,
    RefreshingData,
}

impl ModifierProgressPhase {
    #[cfg(test)]
    pub fn ordered() -> [Self; 6] {
        [
            Self::CheckingProcesses,
            Self::CreatingSnapshot,
            Self::BuildingChanges,
            Self::WritingSave,
            Self::VerifyingSave,
            Self::RefreshingData,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CheckingProcesses => "检查游戏和服务器",
            Self::CreatingSnapshot => "创建回滚点",
            Self::BuildingChanges => "生成修改",
            Self::WritingSave => "写入存档",
            Self::VerifyingSave => "重新解析验证",
            Self::RefreshingData => "刷新数据",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierOperationProgress {
    pub phase: ModifierProgressPhase,
    pub label: String,
}

impl From<ModifierProgressPhase> for ModifierOperationProgress {
    fn from(phase: ModifierProgressPhase) -> Self {
        Self {
            phase,
            label: phase.label().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierWorldEntry {
    pub name: String,
    pub path: String,
}

fn is_reserved_world_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            name.starts_with('_')
                || matches!(
                    name.to_ascii_lowercase().as_str(),
                    "backup" | "backups" | "snapshots"
                )
        })
        .unwrap_or(true)
}

fn collect_modifier_world_dirs(directory: &Path, depth: usize, worlds: &mut Vec<PathBuf>) {
    if depth > 2 || is_reserved_world_directory(directory) {
        return;
    }
    if directory.join("Level.sav").is_file() {
        worlds.push(directory.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            collect_modifier_world_dirs(&entry.path(), depth + 1, worlds);
        }
    }
}

pub fn discover_modifier_worlds_in(save_root: &Path) -> Result<Vec<ModifierWorldEntry>, String> {
    if !save_root.is_dir() {
        return Err(format!("服务器存档目录不存在：{}", save_root.display()));
    }
    let mut data_dirs = Vec::new();
    let entries =
        std::fs::read_dir(save_root).map_err(|error| format!("读取服务器存档目录失败：{error}"))?;
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            collect_modifier_world_dirs(&entry.path(), 1, &mut data_dirs);
        }
    }
    data_dirs.sort();
    data_dirs.dedup();
    let multiple = data_dirs.len() > 1;
    Ok(data_dirs
        .into_iter()
        .enumerate()
        .map(|(index, path)| ModifierWorldEntry {
            name: if multiple {
                format!("服务器世界 {}", index + 1)
            } else {
                "服务器世界".to_string()
            },
            path: path.to_string_lossy().into_owned(),
        })
        .collect())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModifierAction {
    RenamePlayer,
    SetPlayerLevel,
    SetTechnologyPoints,
    UnlockAllTechnologies,
    DeletePlayer,
    MakeGuildLeader,
    RenameGuild,
    DeleteGuild,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModifierActionRequest {
    pub world_path: String,
    pub action: ModifierAction,
    pub player_uid: Option<String>,
    pub guild_id: Option<String>,
    pub value: Option<String>,
    pub level: Option<u32>,
    pub technology_points: Option<i32>,
    pub ancient_technology_points: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierActionPreview {
    pub confirmation_name: String,
    pub player_count: usize,
    pub pal_count: u32,
    pub base_count: usize,
    pub file_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierActionResult {
    pub ok: bool,
    pub snapshot_id: String,
    pub roundtrip_ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuildHandle {
    player_uid: [u8; 16],
    instance_id: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuildMarker {
    marker_id: [u8; 16],
    location: [u8; 24],
    icon_type: i32,
    owner_player_uid: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuildPlayer {
    player_uid: [u8; 16],
    last_online_real_time: i64,
    player_name: String,
    role: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RolePermission {
    role: u8,
    permissions: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuildRaw {
    group_id: [u8; 16],
    group_name: String,
    handles: Vec<GuildHandle>,
    org_type: u8,
    leading_bytes: [u8; 4],
    base_ids: Vec<[u8; 16]>,
    unknown_1: i32,
    base_camp_level: i32,
    base_point_ids: Vec<[u8; 16]>,
    guild_name: String,
    last_modifier_player_uid: [u8; 16],
    markers: Vec<GuildMarker>,
    guild_chest_allowed_roles: Option<Vec<u8>>,
    unknown_i32: Option<i32>,
    admin_player_uid: [u8; 16],
    players: Vec<GuildPlayer>,
    role_permissions: Vec<RolePermission>,
    trailing_bytes: [u8; 4],
}

struct RawReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> RawReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn read<const N: usize>(&mut self, label: &str) -> Result<[u8; N], String> {
        let end = self
            .position
            .checked_add(N)
            .ok_or_else(|| format!("{label} 位置溢出"))?;
        if end > self.bytes.len() {
            return Err(format!("公会数据不完整：无法读取{label}"));
        }
        let value = self.bytes[self.position..end].try_into().unwrap();
        self.position = end;
        Ok(value)
    }

    fn bytes(&mut self, length: usize, label: &str) -> Result<Vec<u8>, String> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| format!("{label} 长度溢出"))?;
        if end > self.bytes.len() {
            return Err(format!("公会数据不完整：无法读取{label}"));
        }
        let value = self.bytes[self.position..end].to_vec();
        self.position = end;
        Ok(value)
    }

    fn u8(&mut self, label: &str) -> Result<u8, String> {
        Ok(self.read::<1>(label)?[0])
    }

    fn i32(&mut self, label: &str) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.read(label)?))
    }

    fn i64(&mut self, label: &str) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.read(label)?))
    }

    fn guid(&mut self, label: &str) -> Result<[u8; 16], String> {
        self.read(label)
    }

    fn count(&mut self, label: &str) -> Result<usize, String> {
        let value = self.i32(label)?;
        if value < 0 || value as usize > self.bytes.len().saturating_sub(self.position) {
            return Err(format!("公会数据中的{label}数量无效：{value}"));
        }
        Ok(value as usize)
    }

    fn fstring(&mut self, label: &str) -> Result<String, String> {
        let length = self.i32(label)?;
        if length == 0 {
            return Ok(String::new());
        }
        if length > 0 {
            let raw = self.bytes(length as usize, label)?;
            let content = raw.strip_suffix(&[0]).unwrap_or(&raw);
            return String::from_utf8(content.to_vec())
                .map_err(|_| format!("{label}不是有效 UTF-8 字符串"));
        }
        let units = length
            .checked_neg()
            .ok_or_else(|| format!("{label}长度溢出"))? as usize;
        let raw = self.bytes(
            units
                .checked_mul(2)
                .ok_or_else(|| format!("{label}长度溢出"))?,
            label,
        )?;
        let mut utf16 = raw
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        if utf16.last() == Some(&0) {
            utf16.pop();
        }
        String::from_utf16(&utf16).map_err(|_| format!("{label}不是有效 UTF-16 字符串"))
    }

    fn eof(&self) -> bool {
        self.position == self.bytes.len()
    }
}

fn decode_players(reader: &mut RawReader<'_>, has_role: bool) -> Result<Vec<GuildPlayer>, String> {
    let count = reader.count("成员")?;
    let mut players = Vec::with_capacity(count);
    for _ in 0..count {
        players.push(GuildPlayer {
            player_uid: reader.guid("成员 UID")?,
            last_online_real_time: reader.i64("成员最后在线时间")?,
            player_name: reader.fstring("成员名称")?,
            role: has_role.then(|| reader.u8("成员身份")).transpose()?,
        });
    }
    Ok(players)
}

fn decode_guild_tail(
    reader: &mut RawReader<'_>,
    version_two: bool,
) -> Result<
    (
        Option<Vec<u8>>,
        Option<i32>,
        [u8; 16],
        Vec<GuildPlayer>,
        Vec<RolePermission>,
        [u8; 4],
    ),
    String,
> {
    let chest_roles = if version_two {
        let count = reader.count("公会仓库身份")?;
        Some(reader.bytes(count, "公会仓库身份")?)
    } else {
        None
    };
    let unknown_i32 = if version_two {
        Some(reader.i32("新版公会标记")?)
    } else {
        None
    };
    let admin = reader.guid("公会队长")?;
    let players = decode_players(reader, version_two)?;
    let mut permissions = Vec::new();
    if version_two {
        let count = reader.count("身份权限")?;
        for _ in 0..count {
            let role = reader.u8("身份")?;
            let permission_count = reader.count("权限")?;
            permissions.push(RolePermission {
                role,
                permissions: reader.bytes(permission_count, "权限")?,
            });
        }
    }
    let trailing = reader.read("公会尾部")?;
    if !reader.eof() {
        return Err("公会数据尾部存在无法识别的内容".to_string());
    }
    Ok((
        chest_roles,
        unknown_i32,
        admin,
        players,
        permissions,
        trailing,
    ))
}

fn decode_guild_raw(bytes: &[u8]) -> Result<GuildRaw, String> {
    let mut reader = RawReader::new(bytes);
    let group_id = reader.guid("组 ID")?;
    let group_name = reader.fstring("组名称")?;
    let handle_count = reader.count("角色句柄")?;
    let mut handles = Vec::with_capacity(handle_count);
    for _ in 0..handle_count {
        handles.push(GuildHandle {
            player_uid: reader.guid("句柄玩家 UID")?,
            instance_id: reader.guid("句柄实例 ID")?,
        });
    }
    let org_type = reader.u8("组织类型")?;
    let leading_bytes = reader.read("公会前导数据")?;
    let base_count = reader.count("据点")?;
    let mut base_ids = Vec::with_capacity(base_count);
    for _ in 0..base_count {
        base_ids.push(reader.guid("据点 ID")?);
    }
    let unknown_1 = reader.i32("公会保留值")?;
    let base_camp_level = reader.i32("公会等级")?;
    let base_point_count = reader.count("据点地图对象")?;
    let mut base_point_ids = Vec::with_capacity(base_point_count);
    for _ in 0..base_point_count {
        base_point_ids.push(reader.guid("据点地图对象 ID")?);
    }
    let guild_name = reader.fstring("公会名称")?;
    let last_modifier_player_uid = reader.guid("最后修改名称的玩家")?;
    let marker_count = reader.count("公会标记")?;
    let mut markers = Vec::with_capacity(marker_count);
    for _ in 0..marker_count {
        markers.push(GuildMarker {
            marker_id: reader.guid("标记 ID")?,
            location: reader.read("标记位置")?,
            icon_type: reader.i32("标记类型")?,
            owner_player_uid: reader.guid("标记所有者")?,
        });
    }
    let tail_position = reader.position;
    let tail = decode_guild_tail(&mut reader, true).or_else(|_| {
        reader.position = tail_position;
        decode_guild_tail(&mut reader, false)
    })?;
    Ok(GuildRaw {
        group_id,
        group_name,
        handles,
        org_type,
        leading_bytes,
        base_ids,
        unknown_1,
        base_camp_level,
        base_point_ids,
        guild_name,
        last_modifier_player_uid,
        markers,
        guild_chest_allowed_roles: tail.0,
        unknown_i32: tail.1,
        admin_player_uid: tail.2,
        players: tail.3,
        role_permissions: tail.4,
        trailing_bytes: tail.5,
    })
}

fn write_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn write_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn write_count(bytes: &mut Vec<u8>, value: usize, label: &str) -> Result<(), String> {
    let value = i32::try_from(value).map_err(|_| format!("{label}数量过大"))?;
    write_i32(bytes, value);
    Ok(())
}
fn write_fstring(bytes: &mut Vec<u8>, value: &str) -> Result<(), String> {
    if value.is_ascii() {
        write_count(bytes, value.len() + 1, "字符串")?;
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
    } else {
        let utf16 = value.encode_utf16().collect::<Vec<_>>();
        let count = i32::try_from(utf16.len() + 1).map_err(|_| "字符串过长".to_string())?;
        write_i32(bytes, -count);
        for unit in utf16 {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u16.to_le_bytes());
    }
    Ok(())
}

fn encode_guild_raw(guild: &GuildRaw) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&guild.group_id);
    write_fstring(&mut bytes, &guild.group_name)?;
    write_count(&mut bytes, guild.handles.len(), "角色句柄")?;
    for handle in &guild.handles {
        bytes.extend_from_slice(&handle.player_uid);
        bytes.extend_from_slice(&handle.instance_id);
    }
    bytes.push(guild.org_type);
    bytes.extend_from_slice(&guild.leading_bytes);
    write_count(&mut bytes, guild.base_ids.len(), "据点")?;
    for id in &guild.base_ids {
        bytes.extend_from_slice(id);
    }
    write_i32(&mut bytes, guild.unknown_1);
    write_i32(&mut bytes, guild.base_camp_level);
    write_count(&mut bytes, guild.base_point_ids.len(), "据点地图对象")?;
    for id in &guild.base_point_ids {
        bytes.extend_from_slice(id);
    }
    write_fstring(&mut bytes, &guild.guild_name)?;
    bytes.extend_from_slice(&guild.last_modifier_player_uid);
    write_count(&mut bytes, guild.markers.len(), "公会标记")?;
    for marker in &guild.markers {
        bytes.extend_from_slice(&marker.marker_id);
        bytes.extend_from_slice(&marker.location);
        write_i32(&mut bytes, marker.icon_type);
        bytes.extend_from_slice(&marker.owner_player_uid);
    }
    let version_two = guild.guild_chest_allowed_roles.is_some();
    if let Some(roles) = &guild.guild_chest_allowed_roles {
        write_count(&mut bytes, roles.len(), "公会仓库身份")?;
        bytes.extend_from_slice(roles);
        write_i32(
            &mut bytes,
            guild
                .unknown_i32
                .ok_or_else(|| "新版公会缺少保留值".to_string())?,
        );
    }
    bytes.extend_from_slice(&guild.admin_player_uid);
    write_count(&mut bytes, guild.players.len(), "成员")?;
    for player in &guild.players {
        bytes.extend_from_slice(&player.player_uid);
        write_i64(&mut bytes, player.last_online_real_time);
        write_fstring(&mut bytes, &player.player_name)?;
        if version_two {
            bytes.push(
                player
                    .role
                    .ok_or_else(|| "新版公会成员缺少身份".to_string())?,
            );
        }
    }
    if version_two {
        write_count(&mut bytes, guild.role_permissions.len(), "身份权限")?;
        for permission in &guild.role_permissions {
            bytes.push(permission.role);
            write_count(&mut bytes, permission.permissions.len(), "权限")?;
            bytes.extend_from_slice(&permission.permissions);
        }
    }
    bytes.extend_from_slice(&guild.trailing_bytes);
    Ok(bytes)
}

fn rename_guild_raw(guild: &mut GuildRaw, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 50 {
        return Err("名称必须为 1–50 个字符".to_string());
    }
    guild.guild_name = name.to_string();
    Ok(())
}

fn make_guild_leader_raw(guild: &mut GuildRaw, player_uid: &[u8; 16]) -> Result<(), String> {
    if !guild
        .players
        .iter()
        .any(|player| &player.player_uid == player_uid)
    {
        return Err("所选玩家不属于该公会".to_string());
    }

    let previous_admin = guild.admin_player_uid;
    guild.admin_player_uid = *player_uid;
    if guild.guild_chest_allowed_roles.is_some() {
        for player in &mut guild.players {
            if player.player_uid == *player_uid {
                player.role = Some(1);
            } else if player.player_uid == previous_admin && player.role == Some(1) {
                player.role = Some(2);
            }
        }
    }
    Ok(())
}

fn total_exp_for_level(level: u32) -> Result<i64, String> {
    if !(1..=80).contains(&level) {
        return Err("玩家等级必须为 1–80".to_string());
    }
    let table: serde_json::Value = serde_json::from_str(include_str!(
        "../../resources/palworld-save-tools/pal_exp_table.json"
    ))
    .map_err(|error| format!("读取同行经验表失败: {error}"))?;
    table
        .get(level.to_string())
        .and_then(|entry| entry.get("TotalEXP"))
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| format!("同行经验表缺少 {level} 级累计经验"))
}

fn technology_assets() -> Result<Vec<String>, String> {
    let world: serde_json::Value = serde_json::from_str(include_str!(
        "../../resources/palworld-save-tools/world.json"
    ))
    .map_err(|error| format!("读取同行科技表失败: {error}"))?;
    let assets = world
        .get("technology")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "同行科技表缺少 technology 数组".to_string())?
        .iter()
        .map(|item| {
            item.get("asset")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "同行科技表存在缺少 asset 的条目".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let unique = assets.iter().collect::<std::collections::HashSet<_>>();
    if unique.len() != assets.len() {
        return Err("同行科技表存在重复 asset".to_string());
    }
    Ok(assets)
}

fn merge_technology_in_fields(
    fields: &mut gvas::types::map::HashableIndexMap<String, Vec<Property>>,
    assets: &[String],
) -> usize {
    let mut matches = 0;
    for (name, properties) in fields.iter_mut() {
        for property in properties {
            matches += merge_technology_in_property(name, property, assets);
        }
    }
    matches
}

fn merge_technology_in_struct(value: &mut StructPropertyValue, assets: &[String]) -> usize {
    match value {
        StructPropertyValue::CustomStruct(fields) => merge_technology_in_fields(fields, assets),
        _ => 0,
    }
}

fn merge_technology_in_property(name: &str, property: &mut Property, assets: &[String]) -> usize {
    let mut matches = 0;
    if name == "UnlockedRecipeTechnologyNames" {
        let values = match property {
            Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Names {
                names,
            }) => Some(names),
            Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Strings {
                strings,
            }) => Some(strings),
            _ => None,
        };
        if let Some(strings) = values {
            let mut existing = strings
                .iter()
                .filter_map(Clone::clone)
                .collect::<std::collections::HashSet<_>>();
            for asset in assets {
                if existing.insert(asset.clone()) {
                    strings.push(Some(asset.clone()));
                }
            }
            matches += 1;
        }
    }
    matches
        + match property {
            Property::StructProperty(value) => merge_technology_in_struct(&mut value.value, assets),
            Property::StructPropertyValue(value) => merge_technology_in_struct(value, assets),
            Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Structs {
                structs,
                ..
            }) => structs
                .iter_mut()
                .map(|value| merge_technology_in_struct(value, assets))
                .sum(),
            Property::ArrayProperty(
                gvas::properties::array_property::ArrayProperty::Properties { properties, .. },
            ) => properties
                .iter_mut()
                .map(|value| merge_technology_in_property("", value, assets))
                .sum(),
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => value
                .0
                .iter_mut()
                .map(|(_, value)| merge_technology_in_property("", value, assets))
                .sum(),
            Property::SetProperty(value) => value
                .properties
                .iter_mut()
                .map(|value| merge_technology_in_property("", value, assets))
                .sum(),
            _ => 0,
        }
}

#[cfg(test)]
fn collect_technology_in_fields(
    fields: &gvas::types::map::HashableIndexMap<String, Vec<Property>>,
    output: &mut std::collections::HashSet<String>,
) {
    for (name, properties) in fields.iter() {
        for property in properties {
            collect_technology_in_property(name, property, output);
        }
    }
}

#[cfg(test)]
fn collect_technology_in_struct(
    value: &StructPropertyValue,
    output: &mut std::collections::HashSet<String>,
) {
    if let StructPropertyValue::CustomStruct(fields) = value {
        collect_technology_in_fields(fields, output);
    }
}

#[cfg(test)]
fn collect_technology_in_property(
    name: &str,
    property: &Property,
    output: &mut std::collections::HashSet<String>,
) {
    if name == "UnlockedRecipeTechnologyNames" {
        match property {
            Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Names {
                names,
            }) => output.extend(names.iter().filter_map(Clone::clone)),
            Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Strings {
                strings,
            }) => output.extend(strings.iter().filter_map(Clone::clone)),
            _ => {}
        }
    }
    match property {
        Property::StructProperty(value) => collect_technology_in_struct(&value.value, output),
        Property::StructPropertyValue(value) => collect_technology_in_struct(value, output),
        Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Structs {
            structs,
            ..
        }) => {
            for value in structs {
                collect_technology_in_struct(value, output);
            }
        }
        Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Properties {
            properties,
            ..
        }) => {
            for value in properties {
                collect_technology_in_property("", value, output);
            }
        }
        Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
            value,
            ..
        }) => {
            for (_, value) in value.iter() {
                collect_technology_in_property("", value, output);
            }
        }
        Property::SetProperty(value) => {
            for value in &value.properties {
                collect_technology_in_property("", value, output);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
fn player_unlocked_technologies(
    data_dir: &Path,
    player_guid: &str,
) -> Result<std::collections::HashSet<String>, String> {
    let player_guid = path_util::normalize_player_guid(player_guid)
        .ok_or_else(|| "角色文件名非法".to_string())?;
    let gvas = sav_io::SavFile::load(&data_dir.join("Players").join(format!("{player_guid}.sav")))?
        .parse()?;
    let mut output = std::collections::HashSet::new();
    for (name, property) in gvas.properties.iter() {
        collect_technology_in_property(name, property, &mut output);
    }
    Ok(output)
}

fn build_unlock_technologies_candidate(
    data_dir: &Path,
    player_guid: &str,
) -> Result<Vec<u8>, String> {
    let player_guid = path_util::normalize_player_guid(player_guid)
        .ok_or_else(|| "角色文件名非法".to_string())?;
    let player_path = data_dir.join("Players").join(format!("{player_guid}.sav"));
    let sav = sav_io::SavFile::load(&player_path)?;
    let mut gvas = sav.parse()?;
    let assets = technology_assets()?;
    let mut matches = 0;
    for (name, property) in gvas.properties.iter_mut() {
        matches += merge_technology_in_property(name, property, &assets);
    }
    if matches == 0 {
        return Err("玩家存档缺少 UnlockedRecipeTechnologyNames".to_string());
    }
    let candidate = sav_io::SavFile::from_gvas(&gvas, sav.compression)?;
    let bytes = candidate.to_bytes()?;
    sav_io::SavFile::from_bytes(&bytes)?
        .parse()
        .map_err(|error| format!("候选玩家存档校验失败: {error}"))?;
    Ok(bytes)
}

fn group_fields(
    property: &Property,
) -> Option<&gvas::types::map::HashableIndexMap<String, Vec<Property>>> {
    match world_copy::as_struct_value(property) {
        Some(StructPropertyValue::CustomStruct(fields)) => Some(fields),
        _ => None,
    }
}

fn is_guild_group(property: &Property) -> bool {
    matches!(
        group_fields(property)
            .and_then(|fields| fields.get("GroupType"))
            .and_then(|values| values.first()),
        Some(Property::EnumProperty(value)) if value.value == "EPalGroupType::Guild"
    )
}

fn format_last_seen(last_tick: i64, world_tick: i64) -> Option<String> {
    if last_tick <= 0 || world_tick <= 0 {
        return None;
    }

    let elapsed_seconds = world_tick.saturating_sub(last_tick) / 10_000_000;
    Some(match elapsed_seconds {
        0..=59 => "刚刚".to_string(),
        60..=3_599 => format!("{} 分钟前", elapsed_seconds / 60),
        3_600..=86_399 => format!("{} 小时前", elapsed_seconds / 3_600),
        _ => format!("{} 天前", elapsed_seconds / 86_400),
    })
}

pub(crate) fn read_modifier_guilds(level_path: &Path) -> Result<Vec<ModifierGuild>, String> {
    let gvas = world_copy::parse_level_gvas(level_path)
        .ok_or_else(|| format!("无法解析世界存档：{}", level_path.display()))?;
    let group_map = sav_io::top_field(&gvas, "worldSaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .and_then(|fields| sav_io::field(fields, "GroupSaveDataMap"))
        .and_then(world_copy::as_props_map)
        .ok_or_else(|| "世界存档缺少 GroupSaveDataMap".to_string())?;

    let mut guilds = Vec::new();
    for (_, group) in group_map.iter().filter(|(_, group)| is_guild_group(group)) {
        let raw_bytes = world_copy::extract_rawdata_bytes(group)
            .ok_or_else(|| "公会记录缺少 RawData".to_string())?;
        let raw = decode_guild_raw(&raw_bytes)?;
        let admin_player_uid = world_copy::guid_std(&raw.admin_player_uid);
        let members = raw
            .players
            .iter()
            .map(|member| ModifierGuildMember {
                player_uid: world_copy::guid_std(&member.player_uid),
                player_name: member.player_name.clone(),
                last_online: member.last_online_real_time,
                role: member.role,
            })
            .collect::<Vec<_>>();
        let leader_name = members
            .iter()
            .find(|member| member.player_uid == admin_player_uid)
            .map(|member| member.player_name.clone())
            .unwrap_or_default();
        guilds.push(ModifierGuild {
            guild_id: world_copy::guid_std(&raw.group_id),
            name: raw.guild_name,
            level: raw.base_camp_level,
            leader_uid: admin_player_uid.clone(),
            leader_name,
            member_count: members.len(),
            base_count: raw.base_ids.len(),
            admin_player_uid,
            members,
        });
    }
    Ok(guilds)
}

fn guid_field(
    fields: &gvas::types::map::HashableIndexMap<String, Vec<Property>>,
    name: &str,
) -> Option<[u8; 16]> {
    fields
        .get(name)?
        .first()
        .and_then(world_copy::as_struct_value)
        .and_then(|value| match value {
            StructPropertyValue::Guid(guid) => Some(guid.to_u8()),
            _ => None,
        })
}

struct CharacterOwner {
    owner: Option<[u8; 16]>,
    is_player: bool,
}

fn character_owner(
    value: &Property,
    custom_versions: &gvas::types::map::HashableIndexMap<Guid, u32>,
) -> Result<Option<CharacterOwner>, String> {
    let Some(bytes) = world_copy::extract_rawdata_bytes(value) else {
        return Ok(None);
    };
    let stream = parse_property_stream(&bytes, custom_versions)?;
    let Some(save_parameter) = stream
        .properties
        .iter()
        .find(|(name, _)| name == "SaveParameter")
        .and_then(|(_, property)| world_copy::as_struct_value(property))
        .and_then(|value| match value {
            StructPropertyValue::CustomStruct(fields) => Some(fields),
            _ => None,
        })
    else {
        return Ok(None);
    };
    let is_player = save_parameter
        .get("IsPlayer")
        .and_then(|values| values.first())
        .map(|value| matches!(value, Property::BoolProperty(flag) if flag.value))
        .unwrap_or(false);
    let owner = guid_field(save_parameter, "OwnerPlayerUId");
    Ok(Some(CharacterOwner { owner, is_player }))
}

fn collect_container_instances_in_fields(
    fields: &gvas::types::map::HashableIndexMap<String, Vec<Property>>,
    instance_ids: &mut Vec<[u8; 16]>,
) {
    for (name, properties) in fields.iter() {
        for property in properties {
            if name == "RawData" {
                if let Property::ArrayProperty(
                    gvas::properties::array_property::ArrayProperty::Bytes { bytes },
                ) = property
                {
                    if bytes.len() >= 32 {
                        let instance_id: [u8; 16] = bytes[16..32].try_into().unwrap();
                        if instance_id != [0; 16] {
                            instance_ids.push(instance_id);
                        }
                    }
                }
            }
            collect_container_instances_in_property(property, instance_ids);
        }
    }
}

fn collect_container_instances_in_struct(
    value: &StructPropertyValue,
    instance_ids: &mut Vec<[u8; 16]>,
) {
    if let StructPropertyValue::CustomStruct(fields) = value {
        collect_container_instances_in_fields(fields, instance_ids);
    }
}

fn collect_container_instances_in_property(property: &Property, instance_ids: &mut Vec<[u8; 16]>) {
    match property {
        Property::StructProperty(value) => {
            collect_container_instances_in_struct(&value.value, instance_ids)
        }
        Property::StructPropertyValue(value) => {
            collect_container_instances_in_struct(value, instance_ids)
        }
        Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Structs {
            structs,
            ..
        }) => {
            for value in structs {
                collect_container_instances_in_struct(value, instance_ids);
            }
        }
        Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Properties {
            properties,
            ..
        }) => {
            for value in properties {
                collect_container_instances_in_property(value, instance_ids);
            }
        }
        Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
            value,
            ..
        }) => {
            for (key, value) in value.iter() {
                collect_container_instances_in_property(key, instance_ids);
                collect_container_instances_in_property(value, instance_ids);
            }
        }
        Property::SetProperty(value) => {
            for value in &value.properties {
                collect_container_instances_in_property(value, instance_ids);
            }
        }
        _ => {}
    }
}

fn key_instance_id(property: &Property) -> Option<[u8; 16]> {
    let StructPropertyValue::CustomStruct(fields) = world_copy::as_struct_value(property)? else {
        return None;
    };
    guid_field(fields, "InstanceId")
}

fn key_container_id(property: &Property) -> Option<[u8; 16]> {
    let StructPropertyValue::CustomStruct(fields) = world_copy::as_struct_value(property)? else {
        return None;
    };
    guid_field(fields, "ID")
}

struct ContainerOwnershipIndex {
    instance_containers: HashMap<[u8; 16], [u8; 16]>,
    container_owners: HashMap<[u8; 16], [u8; 16]>,
}

impl ContainerOwnershipIndex {
    fn effective_owner(
        &self,
        instance_id: Option<[u8; 16]>,
        fallback: Option<[u8; 16]>,
    ) -> Option<[u8; 16]> {
        instance_id
            .and_then(|instance_id| self.instance_containers.get(&instance_id))
            .and_then(|container_id| self.container_owners.get(container_id).copied())
            .or(fallback)
    }
}

fn build_container_ownership(gvas: &gvas::GvasFile) -> Result<ContainerOwnershipIndex, String> {
    let custom_versions = gvas.header.get_custom_versions();
    let cspm = sav_io::top_field(&gvas, "worldSaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .and_then(|fields| sav_io::field(fields, "CharacterSaveParameterMap"))
        .and_then(world_copy::as_props_map)
        .ok_or_else(|| "世界存档缺少 CharacterSaveParameterMap".to_string())?;
    let mut instance_containers = HashMap::new();
    if let Some(containers) = sav_io::top_field(&gvas, "worldSaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .and_then(|fields| sav_io::field(fields, "CharacterContainerSaveData"))
        .and_then(world_copy::as_props_map)
    {
        for (key, value) in containers.iter() {
            let Some(container_id) = key_container_id(key) else {
                continue;
            };
            let mut instance_ids = Vec::new();
            collect_container_instances_in_property(value, &mut instance_ids);
            for instance_id in instance_ids {
                instance_containers.insert(instance_id, container_id);
            }
        }
    }

    // PalworldSaveTools determines a container owner by majority vote from the
    // explicit owners of the Pals already assigned to that container.
    let mut owner_votes: HashMap<[u8; 16], Vec<([u8; 16], u32)>> = HashMap::new();
    for (key, value) in cspm.iter() {
        let Some(character) = character_owner(value, custom_versions)? else {
            continue;
        };
        let (Some(instance_id), Some(owner)) = (key_instance_id(key), character.owner) else {
            continue;
        };
        let Some(container_id) = instance_containers.get(&instance_id) else {
            continue;
        };
        let votes = owner_votes.entry(*container_id).or_default();
        if let Some((_, count)) = votes.iter_mut().find(|(candidate, _)| *candidate == owner) {
            *count += 1;
        } else {
            votes.push((owner, 1));
        }
    }
    let container_owners = owner_votes
        .into_iter()
        .filter_map(|(container_id, votes)| {
            votes
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(owner, _)| (container_id, owner))
        })
        .collect::<HashMap<_, _>>();
    Ok(ContainerOwnershipIndex {
        instance_containers,
        container_owners,
    })
}

fn read_owned_pal_counts(level_path: &Path) -> Result<HashMap<String, u32>, String> {
    let sav = sav_io::SavFile::load(level_path)?;
    let gvas = sav.parse()?;
    let custom_versions = gvas.header.get_custom_versions();
    let cspm = sav_io::top_field(&gvas, "worldSaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .and_then(|fields| sav_io::field(fields, "CharacterSaveParameterMap"))
        .and_then(world_copy::as_props_map)
        .ok_or_else(|| "世界存档缺少 CharacterSaveParameterMap".to_string())?;
    let ownership = build_container_ownership(&gvas)?;

    let mut counts = HashMap::new();
    for (key, value) in cspm.iter() {
        let Some(character) = character_owner(value, custom_versions)? else {
            continue;
        };
        if character.is_player {
            continue;
        }
        let owner = ownership.effective_owner(key_instance_id(key), character.owner);
        let Some(owner) = owner else {
            continue;
        };
        *counts.entry(world_copy::guid_std(&owner)).or_insert(0) += 1;
    }
    Ok(counts)
}

pub fn get_modifier_world_impl(path: &str) -> Result<ModifierWorldState, String> {
    let requested_path = Path::new(path);
    let data_dir = path_util::find_world_data_dir(requested_path)
        .ok_or_else(|| format!("未找到世界数据(Level.sav)：{}", requested_path.display()))?;
    let level_path = data_dir.join("Level.sav");
    let world_tick = world_copy::parse_level_gvas(&level_path)
        .and_then(|gvas| {
            sav_io::top_field(&gvas, "worldSaveData")
                .and_then(sav_io::struct_value)
                .and_then(sav_io::custom_fields)
                .and_then(|fields| sav_io::field(fields, "GameTimeSaveData"))
                .and_then(sav_io::struct_value)
                .and_then(sav_io::custom_fields)
                .and_then(|fields| sav_io::field(fields, "RealDateTimeTicks"))
                .and_then(|property| match property {
                    Property::Int64Property(value) => Some(value.value),
                    Property::UInt64Property(value) => i64::try_from(value.value).ok(),
                    _ => None,
                })
        })
        .unwrap_or_default();
    let guilds = read_modifier_guilds(&level_path)?;
    let pal_counts = read_owned_pal_counts(&level_path)?;
    let memberships = guilds
        .iter()
        .flat_map(|guild| {
            guild.members.iter().map(move |member| {
                (
                    member.player_uid.clone(),
                    (
                        guild.guild_id.clone(),
                        guild.name.clone(),
                        guild.admin_player_uid == member.player_uid,
                        member.role,
                        member.last_online,
                    ),
                )
            })
        })
        .collect::<HashMap<_, _>>();

    let players = world_copy::read_players_from_level(&level_path)
        .into_iter()
        .map(|player| -> Result<ModifierPlayer, String> {
            let membership = memberships.get(&player.player_uid);
            let points =
                tech_edit::player_technology_points_impl(&PlayerTechnologyPointsRequest {
                    world_path: data_dir.to_string_lossy().into_owned(),
                    player_guid: player.guid.clone(),
                })?;
            Ok(ModifierPlayer {
                player_uid: player.player_uid.clone(),
                guid: player.guid,
                nickname: player.nickname,
                level: player.level,
                pal_count: pal_counts.get(&player.player_uid).copied().unwrap_or(0),
                guild_id: membership.map(|value| value.0.clone()),
                guild_name: membership.map(|value| value.1.clone()),
                role: membership
                    .and_then(|value| value.3)
                    .map(|role| role.to_string())
                    .unwrap_or_else(|| "成员".to_string()),
                is_leader: membership.map(|value| value.2).unwrap_or(false),
                last_online: membership.and_then(|value| format_last_seen(value.4, world_tick)),
                technology_points: points.technology_points,
                ancient_technology_points: points.ancient_technology_points,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let world_name = data_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    Ok(ModifierWorldState {
        world_name,
        players,
        guilds,
        server_running: false,
        game_running: false,
    })
}

fn required_player<'a>(
    state: &'a ModifierWorldState,
    request: &ModifierActionRequest,
) -> Result<&'a ModifierPlayer, String> {
    let uid = request
        .player_uid
        .as_deref()
        .ok_or_else(|| "请选择玩家".to_string())?;
    state
        .players
        .iter()
        .find(|player| player.player_uid.eq_ignore_ascii_case(uid))
        .ok_or_else(|| "所选玩家不存在".to_string())
}

fn required_guild<'a>(
    state: &'a ModifierWorldState,
    request: &ModifierActionRequest,
) -> Result<&'a ModifierGuild, String> {
    let guild_id = request
        .guild_id
        .as_deref()
        .ok_or_else(|| "请选择公会".to_string())?;
    state
        .guilds
        .iter()
        .find(|guild| guild.guild_id.eq_ignore_ascii_case(guild_id))
        .ok_or_else(|| "所选公会不存在".to_string())
}

fn validated_name(request: &ModifierActionRequest) -> Result<&str, String> {
    let value = request.value.as_deref().unwrap_or_default().trim();
    if value.is_empty() || value.chars().count() > 50 {
        return Err("名称必须为 1–50 个字符".to_string());
    }
    Ok(value)
}

fn validate_action_request(request: &ModifierActionRequest) -> Result<(), String> {
    match request.action {
        ModifierAction::RenamePlayer | ModifierAction::RenameGuild => {
            validated_name(request)?;
        }
        ModifierAction::SetPlayerLevel => {
            if !matches!(request.level, Some(1..=80)) {
                return Err("玩家等级必须为 1–80".to_string());
            }
        }
        ModifierAction::SetTechnologyPoints => {
            const MAX_POINTS: i32 = 9_999_999;
            if !matches!(request.technology_points, Some(0..=MAX_POINTS))
                || !matches!(request.ancient_technology_points, Some(0..=MAX_POINTS))
            {
                return Err(format!("科技点必须为 0–{MAX_POINTS}"));
            }
        }
        _ => {}
    }
    Ok(())
}

fn player_file_count(data_dir: &Path, player: &ModifierPlayer) -> usize {
    let players_dir = data_dir.join("Players");
    [
        players_dir.join(format!("{}.sav", player.guid)),
        players_dir.join(format!("{}_dps.sav", player.guid)),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .count()
}

pub fn preview_modifier_action_impl(
    request: &ModifierActionRequest,
) -> Result<ModifierActionPreview, String> {
    validate_action_request(request)?;
    let state = get_modifier_world_impl(&request.world_path)?;
    let data_dir = path_util::find_world_data_dir(Path::new(&request.world_path))
        .ok_or_else(|| "未找到世界数据(Level.sav)".to_string())?;

    let preview = match request.action {
        ModifierAction::RenamePlayer => {
            let player = required_player(&state, request)?;
            ModifierActionPreview {
                confirmation_name: player.nickname.clone(),
                player_count: 1,
                pal_count: player.pal_count,
                base_count: 0,
                file_count: 1,
                summary: format!(
                    "将玩家 {} 重命名为 {}。",
                    player.nickname,
                    validated_name(request)?
                ),
            }
        }
        ModifierAction::SetPlayerLevel => {
            let player = required_player(&state, request)?;
            ModifierActionPreview {
                confirmation_name: player.nickname.clone(),
                player_count: 1,
                pal_count: player.pal_count,
                base_count: 0,
                file_count: 1,
                summary: format!(
                    "将玩家 {} 的等级设为 {}。",
                    player.nickname,
                    request.level.unwrap()
                ),
            }
        }
        ModifierAction::SetTechnologyPoints | ModifierAction::UnlockAllTechnologies => {
            let player = required_player(&state, request)?;
            ModifierActionPreview {
                confirmation_name: player.nickname.clone(),
                player_count: 1,
                pal_count: player.pal_count,
                base_count: 0,
                file_count: 1,
                summary: if request.action == ModifierAction::UnlockAllTechnologies {
                    format!("将为玩家 {} 解锁全部科技。", player.nickname)
                } else {
                    format!("将修改玩家 {} 的科技点。", player.nickname)
                },
            }
        }
        ModifierAction::DeletePlayer => {
            let player = required_player(&state, request)?;
            ModifierActionPreview {
                confirmation_name: player.nickname.clone(),
                player_count: 1,
                pal_count: player.pal_count,
                base_count: 0,
                file_count: 1 + player_file_count(&data_dir, player),
                summary: format!("将删除玩家 {} 及其关联存档。", player.nickname),
            }
        }
        ModifierAction::MakeGuildLeader => {
            let player = required_player(&state, request)?;
            let guild = player
                .guild_id
                .as_deref()
                .and_then(|guild_id| state.guilds.iter().find(|guild| guild.guild_id == guild_id))
                .ok_or_else(|| "所选玩家不属于任何公会".to_string())?;
            ModifierActionPreview {
                confirmation_name: player.nickname.clone(),
                player_count: 1,
                pal_count: 0,
                base_count: guild.base_count,
                file_count: 1,
                summary: format!("将 {} 设为公会 {} 的队长。", player.nickname, guild.name),
            }
        }
        ModifierAction::RenameGuild => {
            let guild = required_guild(&state, request)?;
            ModifierActionPreview {
                confirmation_name: guild.name.clone(),
                player_count: guild.member_count,
                pal_count: 0,
                base_count: guild.base_count,
                file_count: 1,
                summary: format!(
                    "将公会 {} 重命名为 {}。",
                    guild.name,
                    validated_name(request)?
                ),
            }
        }
        ModifierAction::DeleteGuild => {
            let guild = required_guild(&state, request)?;
            let members = state
                .players
                .iter()
                .filter(|player| player.guild_id.as_deref() == Some(&guild.guild_id));
            let pal_count = members.clone().map(|player| player.pal_count).sum();
            let player_files = members
                .map(|player| player_file_count(&data_dir, player))
                .sum::<usize>();
            ModifierActionPreview {
                confirmation_name: guild.name.clone(),
                player_count: guild.member_count,
                pal_count,
                base_count: guild.base_count,
                file_count: 1 + player_files,
                summary: format!("将删除公会 {}、全部成员及关联数据。", guild.name),
            }
        }
    };
    Ok(preview)
}

fn struct_value_mut(property: &mut Property) -> Option<&mut StructPropertyValue> {
    match property {
        Property::StructProperty(value) => Some(&mut value.value),
        Property::StructPropertyValue(value) => Some(value),
        _ => None,
    }
}

fn raw_data_bytes_mut(property: &mut Property) -> Option<&mut Vec<u8>> {
    let StructPropertyValue::CustomStruct(fields) = struct_value_mut(property)? else {
        return None;
    };
    match fields.get_mut("RawData")?.first_mut()? {
        Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Bytes {
            bytes,
        }) => Some(bytes),
        _ => None,
    }
}

struct PropertyStream {
    properties: Vec<(String, Property)>,
    tail: Vec<u8>,
}

fn parse_property_stream(
    bytes: &[u8],
    custom_versions: &gvas::types::map::HashableIndexMap<Guid, u32>,
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

fn encode_property_stream(
    stream: &PropertyStream,
    custom_versions: &gvas::types::map::HashableIndexMap<Guid, u32>,
) -> Result<Vec<u8>, String> {
    let mut writer = Cursor::new(Vec::new());
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

fn key_player_uid(property: &Property) -> Option<[u8; 16]> {
    let StructPropertyValue::CustomStruct(fields) = world_copy::as_struct_value(property)? else {
        return None;
    };
    fields
        .get("PlayerUId")?
        .first()
        .and_then(world_copy::as_struct_value)
        .and_then(|value| match value {
            StructPropertyValue::Guid(guid) => Some(guid.to_u8()),
            _ => None,
        })
}

fn mutate_player_stream(
    bytes: &[u8],
    custom_versions: &gvas::types::map::HashableIndexMap<Guid, u32>,
    request: &ModifierActionRequest,
) -> Result<Vec<u8>, String> {
    let mut stream = parse_property_stream(bytes, custom_versions)?;
    let save_parameter = stream
        .properties
        .iter_mut()
        .find(|(name, _)| name == "SaveParameter")
        .and_then(|(_, property)| struct_value_mut(property))
        .and_then(|value| match value {
            StructPropertyValue::CustomStruct(fields) => Some(fields),
            _ => None,
        })
        .ok_or_else(|| "玩家记录缺少 SaveParameter".to_string())?;

    match request.action {
        ModifierAction::RenamePlayer => {
            let name = validated_name(request)?.to_string();
            match save_parameter
                .get_mut("NickName")
                .and_then(|items| items.first_mut())
            {
                Some(Property::StrProperty(value)) => value.value = Some(name),
                _ => return Err("玩家记录缺少可写的 NickName".to_string()),
            }
        }
        ModifierAction::SetPlayerLevel => {
            let level = request.level.ok_or_else(|| "请设置玩家等级".to_string())?;
            match save_parameter
                .get_mut("Level")
                .and_then(|items| items.first_mut())
            {
                Some(Property::ByteProperty(value)) => match &mut value.value {
                    BytePropertyValue::Byte(current) => *current = level as u8,
                    _ => return Err("玩家 Level 字段格式不受支持".to_string()),
                },
                Some(Property::IntProperty(value)) => value.value = level as i32,
                _ => return Err("玩家记录缺少可写的 Level".to_string()),
            }
            let exp = i32::try_from(total_exp_for_level(level)?)
                .map_err(|_| "累计经验超出存档整数范围".to_string())?;
            match save_parameter
                .get_mut("Exp")
                .and_then(|items| items.first_mut())
            {
                Some(Property::IntProperty(value)) => value.value = exp,
                Some(Property::Int64Property(value)) => value.value = i64::from(exp),
                _ => return Err("玩家记录缺少可写的 Exp".to_string()),
            }
        }
        _ => return Err("该操作不属于玩家 CSPM 写入".to_string()),
    }
    encode_property_stream(&stream, custom_versions)
}

fn build_level_player_candidate(
    data_dir: &Path,
    request: &ModifierActionRequest,
) -> Result<Vec<u8>, String> {
    let player_uid = request
        .player_uid
        .as_deref()
        .ok_or_else(|| "请选择玩家".to_string())?;
    let player_uid_bytes = sav_io::guid_bytes(player_uid)?;
    let level_path = data_dir.join("Level.sav");
    let level_sav = sav_io::SavFile::load(&level_path)?;
    let mut gvas = level_sav.parse()?;
    let custom_versions = gvas.header.get_custom_versions().clone();
    let cspm = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, "CharacterSaveParameterMap"))
        .and_then(|property| match property {
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => Some(value),
            _ => None,
        })
        .ok_or_else(|| "世界存档缺少 CharacterSaveParameterMap".to_string())?;

    let mut player_changed = false;
    for (key, value) in cspm.0.iter_mut() {
        if key_player_uid(key) != Some(player_uid_bytes) {
            continue;
        }
        let raw_bytes =
            raw_data_bytes_mut(value).ok_or_else(|| "玩家记录缺少 RawData".to_string())?;
        *raw_bytes = mutate_player_stream(raw_bytes, &custom_versions, request)?;
        player_changed = true;
        break;
    }
    if !player_changed {
        return Err("所选玩家不存在".to_string());
    }

    if request.action == ModifierAction::RenamePlayer {
        let new_name = validated_name(request)?.to_string();
        let group_map = sav_io::top_field_mut(&mut gvas, "worldSaveData")
            .and_then(sav_io::struct_value_mut)
            .and_then(sav_io::custom_fields_mut)
            .and_then(|fields| sav_io::field_mut(fields, "GroupSaveDataMap"))
            .and_then(|property| match property {
                Property::MapProperty(
                    gvas::properties::map_property::MapProperty::Properties { value, .. },
                ) => Some(value),
                _ => None,
            })
            .ok_or_else(|| "世界存档缺少 GroupSaveDataMap".to_string())?;
        for (_, value) in group_map.0.iter_mut() {
            if !is_guild_group(value) {
                continue;
            }
            let Some(raw_bytes) = raw_data_bytes_mut(value) else {
                continue;
            };
            let mut guild = decode_guild_raw(raw_bytes)?;
            let Some(member) = guild
                .players
                .iter_mut()
                .find(|member| member.player_uid == player_uid_bytes)
            else {
                continue;
            };
            member.player_name = new_name.clone();
            *raw_bytes = encode_guild_raw(&guild)?;
            break;
        }
    }

    let candidate = sav_io::SavFile::from_gvas(&gvas, level_sav.compression)?;
    let bytes = candidate.to_bytes()?;
    sav_io::SavFile::from_bytes(&bytes)?
        .parse()
        .map_err(|error| format!("候选 Level.sav 校验失败: {error}"))?;
    Ok(bytes)
}

fn property_guid_id(property: &Property) -> Option<[u8; 16]> {
    match world_copy::as_struct_value(property)? {
        StructPropertyValue::Guid(guid) => Some(guid.to_u8()),
        StructPropertyValue::CustomStruct(fields) => ["ID", "GroupId", "BaseCampId"]
            .into_iter()
            .find_map(|name| guid_field(fields, name)),
        _ => None,
    }
}

fn clear_deleted_character_slots_in_fields(
    fields: &mut gvas::types::map::HashableIndexMap<String, Vec<Property>>,
    player_uids: &HashSet<[u8; 16]>,
    deleted_instances: &HashSet<[u8; 16]>,
) {
    for (name, properties) in fields.iter_mut() {
        for property in properties {
            if name == "RawData" {
                if let Property::ArrayProperty(
                    gvas::properties::array_property::ArrayProperty::Bytes { bytes },
                ) = property
                {
                    if bytes.len() >= 32 {
                        let slot_player_uid: [u8; 16] = bytes[0..16].try_into().unwrap();
                        let instance_id: [u8; 16] = bytes[16..32].try_into().unwrap();
                        if player_uids.contains(&slot_player_uid)
                            || deleted_instances.contains(&instance_id)
                        {
                            bytes.clear();
                            continue;
                        }
                    }
                }
            }
            clear_deleted_character_slots_in_property(property, player_uids, deleted_instances);
        }
    }
}

fn clear_deleted_character_slots_in_struct(
    value: &mut StructPropertyValue,
    player_uids: &HashSet<[u8; 16]>,
    deleted_instances: &HashSet<[u8; 16]>,
) {
    if let StructPropertyValue::CustomStruct(fields) = value {
        clear_deleted_character_slots_in_fields(fields, player_uids, deleted_instances);
    }
}

fn clear_deleted_character_slots_in_property(
    property: &mut Property,
    player_uids: &HashSet<[u8; 16]>,
    deleted_instances: &HashSet<[u8; 16]>,
) {
    match property {
        Property::StructProperty(value) => clear_deleted_character_slots_in_struct(
            &mut value.value,
            player_uids,
            deleted_instances,
        ),
        Property::StructPropertyValue(value) => {
            clear_deleted_character_slots_in_struct(value, player_uids, deleted_instances)
        }
        Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Structs {
            structs,
            ..
        }) => {
            for value in structs {
                clear_deleted_character_slots_in_struct(value, player_uids, deleted_instances);
            }
        }
        Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Properties {
            properties,
            ..
        }) => {
            for value in properties {
                clear_deleted_character_slots_in_property(value, player_uids, deleted_instances);
            }
        }
        Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
            value,
            ..
        }) => {
            for (_, value) in value.0.iter_mut() {
                clear_deleted_character_slots_in_property(value, player_uids, deleted_instances);
            }
        }
        Property::SetProperty(value) => {
            for value in &mut value.properties {
                clear_deleted_character_slots_in_property(value, player_uids, deleted_instances);
            }
        }
        _ => {}
    }
}

fn remove_map_entries_by_guid(
    gvas: &mut gvas::GvasFile,
    field_name: &str,
    removed_ids: &HashSet<[u8; 16]>,
) {
    if removed_ids.is_empty() {
        return;
    }
    let Some(entries) = sav_io::top_field_mut(gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, field_name))
        .and_then(|property| match property {
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => Some(value),
            _ => None,
        })
    else {
        return;
    };
    entries
        .0
        .retain(|key, _| property_guid_id(key).is_none_or(|id| !removed_ids.contains(&id)));
}

fn build_delete_player_candidate(
    data_dir: &Path,
    request: &ModifierActionRequest,
) -> Result<Vec<u8>, String> {
    let player_uid = request
        .player_uid
        .as_deref()
        .ok_or_else(|| "请选择玩家".to_string())?;
    let player_uid_bytes = sav_io::guid_bytes(player_uid)?;
    let level_path = data_dir.join("Level.sav");
    let level_sav = sav_io::SavFile::load(&level_path)?;
    let mut gvas = level_sav.parse()?;
    let ownership = build_container_ownership(&gvas)?;
    let custom_versions = gvas.header.get_custom_versions().clone();

    let cspm = sav_io::top_field(&gvas, "worldSaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .and_then(|fields| sav_io::field(fields, "CharacterSaveParameterMap"))
        .and_then(world_copy::as_props_map)
        .ok_or_else(|| "世界存档缺少 CharacterSaveParameterMap".to_string())?;
    let mut deleted_instances = HashSet::new();
    let mut found_player = false;
    for (key, value) in cspm.iter() {
        if key_player_uid(key) == Some(player_uid_bytes) {
            found_player = true;
            if let Some(instance_id) = key_instance_id(key) {
                deleted_instances.insert(instance_id);
            }
            continue;
        }
        let Some(character) = character_owner(value, &custom_versions)? else {
            continue;
        };
        if character.is_player {
            continue;
        }
        let instance_id = key_instance_id(key);
        if ownership.effective_owner(instance_id, character.owner) == Some(player_uid_bytes) {
            if let Some(instance_id) = instance_id {
                deleted_instances.insert(instance_id);
            }
        }
    }
    if !found_player {
        return Err("所选玩家不存在".to_string());
    }

    let cspm = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, "CharacterSaveParameterMap"))
        .and_then(|property| match property {
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => Some(value),
            _ => None,
        })
        .ok_or_else(|| "世界存档缺少 CharacterSaveParameterMap".to_string())?;
    cspm.0.retain(|key, _| {
        key_player_uid(key) != Some(player_uid_bytes)
            && key_instance_id(key).is_none_or(|id| !deleted_instances.contains(&id))
    });

    if let Some(containers) = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, "CharacterContainerSaveData"))
    {
        let deleted_player_uids = HashSet::from([player_uid_bytes]);
        clear_deleted_character_slots_in_property(
            containers,
            &deleted_player_uids,
            &deleted_instances,
        );
    }

    let mut removed_guild_ids = HashSet::new();
    let mut removed_base_ids = HashSet::new();
    let group_map = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, "GroupSaveDataMap"))
        .and_then(|property| match property {
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => Some(value),
            _ => None,
        })
        .ok_or_else(|| "世界存档缺少 GroupSaveDataMap".to_string())?;
    for (_, value) in group_map.0.iter_mut() {
        if !is_guild_group(value) {
            continue;
        }
        let Some(raw_bytes) = raw_data_bytes_mut(value) else {
            continue;
        };
        let mut guild = decode_guild_raw(raw_bytes)?;
        let previous_count = guild.players.len();
        guild
            .players
            .retain(|member| member.player_uid != player_uid_bytes);
        if guild.players.len() == previous_count {
            continue;
        }
        guild
            .handles
            .retain(|handle| handle.player_uid != player_uid_bytes);
        if guild.players.is_empty() {
            removed_guild_ids.insert(guild.group_id);
            removed_base_ids.extend(guild.base_ids.iter().copied());
            continue;
        }
        if guild.admin_player_uid == player_uid_bytes {
            guild.admin_player_uid = guild.players[0].player_uid;
            if guild.guild_chest_allowed_roles.is_some() {
                for member in &mut guild.players {
                    member.role = Some(if member.player_uid == guild.admin_player_uid {
                        1
                    } else {
                        3
                    });
                }
            }
        }
        *raw_bytes = encode_guild_raw(&guild)?;
    }
    group_map.0.retain(|_, value| {
        world_copy::extract_rawdata_bytes(value)
            .and_then(|bytes| decode_guild_raw(&bytes).ok())
            .is_none_or(|guild| !removed_guild_ids.contains(&guild.group_id))
    });

    remove_map_entries_by_guid(&mut gvas, "GuildExtraSaveDataMap", &removed_guild_ids);
    remove_map_entries_by_guid(&mut gvas, "BaseCampSaveData", &removed_base_ids);
    remove_map_entries_by_guid(&mut gvas, "InvaderSaveData", &removed_base_ids);

    let candidate = sav_io::SavFile::from_gvas(&gvas, level_sav.compression)?;
    let bytes = candidate.to_bytes()?;
    sav_io::SavFile::from_bytes(&bytes)?
        .parse()
        .map_err(|error| format!("候选 Level.sav 校验失败: {error}"))?;
    Ok(bytes)
}

fn build_delete_guild_candidate(
    data_dir: &Path,
    request: &ModifierActionRequest,
) -> Result<Vec<u8>, String> {
    let target_guild_id = request
        .guild_id
        .as_deref()
        .ok_or_else(|| "请选择公会".to_string())?;
    let target_guild_id = sav_io::guid_bytes(target_guild_id)?;
    let level_path = data_dir.join("Level.sav");
    let level_sav = sav_io::SavFile::load(&level_path)?;
    let mut gvas = level_sav.parse()?;
    let ownership = build_container_ownership(&gvas)?;
    let custom_versions = gvas.header.get_custom_versions().clone();

    let group_map = sav_io::top_field(&gvas, "worldSaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .and_then(|fields| sav_io::field(fields, "GroupSaveDataMap"))
        .and_then(world_copy::as_props_map)
        .ok_or_else(|| "世界存档缺少 GroupSaveDataMap".to_string())?;
    let target_guild = group_map
        .iter()
        .filter(|(_, value)| is_guild_group(value))
        .filter_map(|(_, value)| world_copy::extract_rawdata_bytes(value))
        .filter_map(|bytes| decode_guild_raw(&bytes).ok())
        .find(|guild| guild.group_id == target_guild_id)
        .ok_or_else(|| "所选公会不存在".to_string())?;
    let deleted_player_uids = target_guild
        .players
        .iter()
        .map(|player| player.player_uid)
        .collect::<HashSet<_>>();
    let removed_guild_ids = HashSet::from([target_guild.group_id]);
    let removed_base_ids = target_guild
        .base_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();

    let cspm = sav_io::top_field(&gvas, "worldSaveData")
        .and_then(sav_io::struct_value)
        .and_then(sav_io::custom_fields)
        .and_then(|fields| sav_io::field(fields, "CharacterSaveParameterMap"))
        .and_then(world_copy::as_props_map)
        .ok_or_else(|| "世界存档缺少 CharacterSaveParameterMap".to_string())?;
    let mut deleted_instances = HashSet::new();
    for (key, value) in cspm.iter() {
        if key_player_uid(key).is_some_and(|uid| deleted_player_uids.contains(&uid)) {
            if let Some(instance_id) = key_instance_id(key) {
                deleted_instances.insert(instance_id);
            }
            continue;
        }
        let Some(character) = character_owner(value, &custom_versions)? else {
            continue;
        };
        if character.is_player {
            continue;
        }
        let instance_id = key_instance_id(key);
        if ownership
            .effective_owner(instance_id, character.owner)
            .is_some_and(|owner| deleted_player_uids.contains(&owner))
        {
            if let Some(instance_id) = instance_id {
                deleted_instances.insert(instance_id);
            }
        }
    }

    let cspm = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, "CharacterSaveParameterMap"))
        .and_then(|property| match property {
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => Some(value),
            _ => None,
        })
        .ok_or_else(|| "世界存档缺少 CharacterSaveParameterMap".to_string())?;
    cspm.0.retain(|key, _| {
        !key_player_uid(key).is_some_and(|uid| deleted_player_uids.contains(&uid))
            && key_instance_id(key).is_none_or(|id| !deleted_instances.contains(&id))
    });

    if let Some(containers) = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, "CharacterContainerSaveData"))
    {
        clear_deleted_character_slots_in_property(
            containers,
            &deleted_player_uids,
            &deleted_instances,
        );
    }

    let group_map = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, "GroupSaveDataMap"))
        .and_then(|property| match property {
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => Some(value),
            _ => None,
        })
        .ok_or_else(|| "世界存档缺少 GroupSaveDataMap".to_string())?;
    for (_, value) in group_map.0.iter_mut() {
        if !is_guild_group(value) {
            continue;
        }
        let Some(raw_bytes) = raw_data_bytes_mut(value) else {
            continue;
        };
        let mut guild = decode_guild_raw(raw_bytes)?;
        if removed_guild_ids.contains(&guild.group_id) {
            continue;
        }
        let previous_players = guild.players.len();
        let previous_handles = guild.handles.len();
        guild
            .players
            .retain(|member| !deleted_player_uids.contains(&member.player_uid));
        guild
            .handles
            .retain(|handle| !deleted_player_uids.contains(&handle.player_uid));
        if guild.players.len() != previous_players || guild.handles.len() != previous_handles {
            *raw_bytes = encode_guild_raw(&guild)?;
        }
    }
    group_map.0.retain(|_, value| {
        world_copy::extract_rawdata_bytes(value)
            .and_then(|bytes| decode_guild_raw(&bytes).ok())
            .is_none_or(|guild| !removed_guild_ids.contains(&guild.group_id))
    });

    remove_map_entries_by_guid(&mut gvas, "GuildExtraSaveDataMap", &removed_guild_ids);
    remove_map_entries_by_guid(&mut gvas, "BaseCampSaveData", &removed_base_ids);
    remove_map_entries_by_guid(&mut gvas, "InvaderSaveData", &removed_base_ids);

    let candidate = sav_io::SavFile::from_gvas(&gvas, level_sav.compression)?;
    let bytes = candidate.to_bytes()?;
    sav_io::SavFile::from_bytes(&bytes)?
        .parse()
        .map_err(|error| format!("候选 Level.sav 校验失败: {error}"))?;
    Ok(bytes)
}

fn build_level_guild_candidate(
    data_dir: &Path,
    request: &ModifierActionRequest,
) -> Result<Vec<u8>, String> {
    let level_path = data_dir.join("Level.sav");
    let level_sav = sav_io::SavFile::load(&level_path)?;
    let mut gvas = level_sav.parse()?;
    let group_map = sav_io::top_field_mut(&mut gvas, "worldSaveData")
        .and_then(sav_io::struct_value_mut)
        .and_then(sav_io::custom_fields_mut)
        .and_then(|fields| sav_io::field_mut(fields, "GroupSaveDataMap"))
        .and_then(|property| match property {
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => Some(value),
            _ => None,
        })
        .ok_or_else(|| "世界存档缺少 GroupSaveDataMap".to_string())?;

    let target_guild_id = if let Some(guild_id) = request.guild_id.as_deref() {
        guild_id.to_string()
    } else {
        let player_uid = request
            .player_uid
            .as_deref()
            .ok_or_else(|| "请选择玩家".to_string())?;
        get_modifier_world_impl(data_dir.to_string_lossy().as_ref())?
            .players
            .into_iter()
            .find(|player| player.player_uid.eq_ignore_ascii_case(player_uid))
            .and_then(|player| player.guild_id)
            .ok_or_else(|| "所选玩家不属于任何公会".to_string())?
    };

    let mut changed = false;
    for (_, property) in group_map.0.iter_mut() {
        if !is_guild_group(property) {
            continue;
        }
        let raw_bytes =
            raw_data_bytes_mut(property).ok_or_else(|| "公会记录缺少 RawData".to_string())?;
        let mut guild = decode_guild_raw(raw_bytes)?;
        if !world_copy::guid_std(&guild.group_id).eq_ignore_ascii_case(&target_guild_id) {
            continue;
        }
        match request.action {
            ModifierAction::RenameGuild => rename_guild_raw(&mut guild, validated_name(request)?)?,
            ModifierAction::MakeGuildLeader => {
                let player_uid = request
                    .player_uid
                    .as_deref()
                    .ok_or_else(|| "请选择玩家".to_string())?;
                let player_uid = sav_io::guid_bytes(player_uid)?;
                make_guild_leader_raw(&mut guild, &player_uid)?;
            }
            _ => return Err("该操作不属于公会存档写入".to_string()),
        }
        *raw_bytes = encode_guild_raw(&guild)?;
        changed = true;
        break;
    }
    if !changed {
        return Err("所选公会不存在".to_string());
    }

    let candidate = sav_io::SavFile::from_gvas(&gvas, level_sav.compression)?;
    let bytes = candidate.to_bytes()?;
    sav_io::SavFile::from_bytes(&bytes)?
        .parse()
        .map_err(|error| format!("候选 Level.sav 校验失败: {error}"))?;
    Ok(bytes)
}

fn action_source(action: ModifierAction) -> &'static str {
    match action {
        ModifierAction::RenamePlayer => "player-rename",
        ModifierAction::SetPlayerLevel => "player-level",
        ModifierAction::SetTechnologyPoints => "technology-points",
        ModifierAction::UnlockAllTechnologies => "technology-unlock-all",
        ModifierAction::DeletePlayer => "player-delete",
        ModifierAction::MakeGuildLeader => "guild-leader",
        ModifierAction::RenameGuild => "guild-rename",
        ModifierAction::DeleteGuild => "guild-delete",
    }
}

fn operation_id() -> String {
    format!(
        "modifier-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

#[cfg(test)]
pub(crate) fn apply_modifier_action_in_dir(
    request: &ModifierActionRequest,
    backup_root: &Path,
) -> Result<ModifierActionResult, String> {
    apply_modifier_action_in_dir_with_progress(request, backup_root, |_| {})
}

fn affected_files_for_request(
    data_dir: &Path,
    request: &ModifierActionRequest,
) -> Result<Vec<PathBuf>, String> {
    match request.action {
        ModifierAction::RenameGuild
        | ModifierAction::MakeGuildLeader
        | ModifierAction::RenamePlayer
        | ModifierAction::SetPlayerLevel => Ok(vec![PathBuf::from("Level.sav")]),
        ModifierAction::SetTechnologyPoints | ModifierAction::UnlockAllTechnologies => {
            let state = get_modifier_world_impl(&request.world_path)?;
            let player = required_player(&state, request)?;
            Ok(vec![
                PathBuf::from("Players").join(format!("{}.sav", player.guid))
            ])
        }
        ModifierAction::DeletePlayer => {
            let state = get_modifier_world_impl(&request.world_path)?;
            let player = required_player(&state, request)?;
            let mut files = vec![PathBuf::from("Level.sav")];
            for relative in [
                PathBuf::from("Players").join(format!("{}.sav", player.guid)),
                PathBuf::from("Players").join(format!("{}_dps.sav", player.guid)),
            ] {
                if data_dir.join(&relative).is_file() {
                    files.push(relative);
                }
            }
            Ok(files)
        }
        ModifierAction::DeleteGuild => {
            let state = get_modifier_world_impl(&request.world_path)?;
            let guild = required_guild(&state, request)?;
            let mut files = vec![PathBuf::from("Level.sav")];
            for player in state
                .players
                .iter()
                .filter(|player| player.guild_id.as_deref() == Some(&guild.guild_id))
            {
                for relative in [
                    PathBuf::from("Players").join(format!("{}.sav", player.guid)),
                    PathBuf::from("Players").join(format!("{}_dps.sav", player.guid)),
                ] {
                    if data_dir.join(&relative).is_file() {
                        files.push(relative);
                    }
                }
            }
            Ok(files)
        }
    }
}

pub(crate) fn apply_modifier_action_in_dir_with_progress<F>(
    request: &ModifierActionRequest,
    backup_root: &Path,
    mut progress: F,
) -> Result<ModifierActionResult, String>
where
    F: FnMut(ModifierProgressPhase),
{
    validate_action_request(request)?;
    let data_dir = path_util::find_world_data_dir(Path::new(&request.world_path))
        .ok_or_else(|| "未找到世界数据(Level.sav)".to_string())?;
    let preview = preview_modifier_action_impl(request)?;
    let affected_files = affected_files_for_request(&data_dir, request)?;

    progress(ModifierProgressPhase::CreatingSnapshot);
    std::fs::create_dir_all(backup_root.join("snapshots"))
        .map_err(|error| format!("创建修改器备份目录失败: {error}"))?;
    let world_name = data_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("world");
    let world_id = backup_service::world_directory_id(world_name, &data_dir);
    let workflow_id = operation_id();
    let snapshot = backup_service::create_snapshot(
        backup_root,
        &data_dir,
        &world_id,
        world_name,
        WorldClass::Server,
        &workflow_id,
        action_source(request.action),
        &affected_files,
        BackupState::Applying,
    )?;

    progress(ModifierProgressPhase::BuildingChanges);
    let mutations = match request.action {
        ModifierAction::RenameGuild | ModifierAction::MakeGuildLeader => {
            let level_bytes = build_level_guild_candidate(&data_dir, request)?;
            let relative = PathBuf::from("Level.sav");
            vec![FileMutation {
                relative_path: relative,
                content: Some(level_bytes),
            }]
        }
        ModifierAction::RenamePlayer | ModifierAction::SetPlayerLevel => {
            let level_bytes = build_level_player_candidate(&data_dir, request)?;
            let relative = PathBuf::from("Level.sav");
            vec![FileMutation {
                relative_path: relative,
                content: Some(level_bytes),
            }]
        }
        ModifierAction::SetTechnologyPoints => Vec::new(),
        ModifierAction::UnlockAllTechnologies => {
            let player =
                required_player(&get_modifier_world_impl(&request.world_path)?, request)?.clone();
            let relative = PathBuf::from("Players").join(format!("{}.sav", player.guid));
            let player_bytes = build_unlock_technologies_candidate(&data_dir, &player.guid)?;
            vec![FileMutation {
                relative_path: relative,
                content: Some(player_bytes),
            }]
        }
        ModifierAction::DeletePlayer => {
            let state = get_modifier_world_impl(&request.world_path)?;
            let player = required_player(&state, request)?.clone();
            let level_bytes = build_delete_player_candidate(&data_dir, request)?;
            let mut mutations = vec![FileMutation {
                relative_path: PathBuf::from("Level.sav"),
                content: Some(level_bytes),
            }];
            for relative in [
                PathBuf::from("Players").join(format!("{}.sav", player.guid)),
                PathBuf::from("Players").join(format!("{}_dps.sav", player.guid)),
            ] {
                if data_dir.join(&relative).is_file() {
                    mutations.push(FileMutation {
                        relative_path: relative,
                        content: None,
                    });
                }
            }
            mutations
        }
        ModifierAction::DeleteGuild => {
            let state = get_modifier_world_impl(&request.world_path)?;
            let guild = required_guild(&state, request)?.clone();
            let level_bytes = build_delete_guild_candidate(&data_dir, request)?;
            let mut mutations = vec![FileMutation {
                relative_path: PathBuf::from("Level.sav"),
                content: Some(level_bytes),
            }];
            for player in state
                .players
                .iter()
                .filter(|player| player.guild_id.as_deref() == Some(&guild.guild_id))
            {
                for relative in [
                    PathBuf::from("Players").join(format!("{}.sav", player.guid)),
                    PathBuf::from("Players").join(format!("{}_dps.sav", player.guid)),
                ] {
                    if data_dir.join(&relative).is_file() {
                        mutations.push(FileMutation {
                            relative_path: relative,
                            content: None,
                        });
                    }
                }
            }
            mutations
        }
    };

    progress(ModifierProgressPhase::WritingSave);
    let operation = match request.action {
        ModifierAction::RenameGuild
        | ModifierAction::MakeGuildLeader
        | ModifierAction::RenamePlayer
        | ModifierAction::SetPlayerLevel
        | ModifierAction::UnlockAllTechnologies
        | ModifierAction::DeletePlayer
        | ModifierAction::DeleteGuild => atomic_write::commit_file_set(&data_dir, &mutations),
        ModifierAction::SetTechnologyPoints => {
            let player =
                required_player(&get_modifier_world_impl(&request.world_path)?, request)?.clone();
            tech_edit::update_player_technology_points_impl(&UpdatePlayerTechnologyPointsRequest {
                world_path: request.world_path.clone(),
                player_guid: player.guid,
                technology_points: request.technology_points.unwrap(),
                ancient_technology_points: request.ancient_technology_points.unwrap(),
            })
            .map(|_| ())
        }
    };
    if let Err(error) = operation {
        let restored = backup_service::restore_snapshot(backup_root, &snapshot.id, &data_dir);
        let _ = backup_service::update_snapshot_state(
            backup_root,
            &snapshot.id,
            BackupState::RecoveryRequired,
        );
        return match restored {
            Ok(()) => Err(format!("修改失败，已从操作回滚点恢复: {error}")),
            Err(restore_error) => Err(format!(
                "修改失败且自动恢复失败: {error}；恢复错误: {restore_error}"
            )),
        };
    }

    progress(ModifierProgressPhase::VerifyingSave);
    let roundtrip_ok = if request.action == ModifierAction::SetTechnologyPoints {
        affected_files.iter().all(|relative| {
            sav_io::SavFile::load(&data_dir.join(relative))
                .and_then(|sav| sav.parse().map(|_| true))
                .unwrap_or(false)
        })
    } else {
        mutations.iter().all(|mutation| match mutation.content {
            Some(_) => sav_io::SavFile::load(&data_dir.join(&mutation.relative_path))
                .and_then(|sav| sav.parse().map(|_| true))
                .unwrap_or(false),
            None => !data_dir.join(&mutation.relative_path).exists(),
        })
    };
    if !roundtrip_ok {
        backup_service::restore_snapshot(backup_root, &snapshot.id, &data_dir)?;
        let _ = backup_service::update_snapshot_state(
            backup_root,
            &snapshot.id,
            BackupState::RecoveryRequired,
        );
        return Err("写入后校验失败，已从操作回滚点恢复".to_string());
    }
    backup_service::update_snapshot_state(backup_root, &snapshot.id, BackupState::Committed)?;
    progress(ModifierProgressPhase::RefreshingData);
    Ok(ModifierActionResult {
        ok: true,
        snapshot_id: snapshot.id,
        roundtrip_ok: true,
        message: preview.summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn modifier_progress_phases_are_real_and_user_facing() {
        assert_eq!(
            ModifierProgressPhase::ordered(),
            [
                ModifierProgressPhase::CheckingProcesses,
                ModifierProgressPhase::CreatingSnapshot,
                ModifierProgressPhase::BuildingChanges,
                ModifierProgressPhase::WritingSave,
                ModifierProgressPhase::VerifyingSave,
                ModifierProgressPhase::RefreshingData,
            ]
        );
        assert_eq!(
            ModifierProgressPhase::CreatingSnapshot.label(),
            "创建回滚点"
        );
    }

    #[test]
    fn modifier_world_discovery_returns_data_dirs_and_skips_backups() {
        let root = std::env::temp_dir().join(format!(
            "palworld-modifier-discovery-{}",
            std::process::id(),
        ));
        let _ = std::fs::remove_dir_all(&root);
        let live = root.join("0").join("1A91A61548C7B6FD7B58B2B70710F7EE");
        let stale = root
            .join("_migration_backups")
            .join("operation")
            .join("0")
            .join("DEADBEEFDEADBEEFDEADBEEFDEADBEEF");
        let nested_backup = live.join("backup").join("world").join("old");
        std::fs::create_dir_all(&live).unwrap();
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::create_dir_all(&nested_backup).unwrap();
        std::fs::write(live.join("Level.sav"), b"live").unwrap();
        std::fs::write(stale.join("Level.sav"), b"stale").unwrap();
        std::fs::write(nested_backup.join("Level.sav"), b"old").unwrap();

        let worlds = discover_modifier_worlds_in(&root).unwrap();

        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].name, "服务器世界");
        assert_eq!(Path::new(&worlds[0].path), live);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn last_seen_uses_world_ticks_instead_of_exposing_raw_values() {
        const SECOND: i64 = 10_000_000;

        assert_eq!(format_last_seen(0, 20 * SECOND), None);
        assert_eq!(
            format_last_seen(19 * SECOND, 20 * SECOND),
            Some("刚刚".to_string())
        );
        assert_eq!(
            format_last_seen(20 * SECOND, 20 * SECOND + 3_600 * SECOND),
            Some("1 小时前".to_string()),
        );
        assert_eq!(
            format_last_seen(20 * SECOND, 20 * SECOND + 2 * 86_400 * SECOND),
            Some("2 天前".to_string()),
        );
    }

    fn push_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i64(bytes: &mut Vec<u8>, value: i64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_fstring(bytes: &mut Vec<u8>, value: &str) {
        if value.is_ascii() {
            push_i32(bytes, value.len() as i32 + 1);
            bytes.extend_from_slice(value.as_bytes());
            bytes.push(0);
        } else {
            let utf16 = value.encode_utf16().collect::<Vec<_>>();
            push_i32(bytes, -(utf16.len() as i32 + 1));
            for unit in utf16 {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            bytes.extend_from_slice(&0_u16.to_le_bytes());
        }
    }

    fn guild_fixture(version_two: bool) -> Vec<u8> {
        let group_id = [1_u8; 16];
        let player_uid = [2_u8; 16];
        let instance_id = [3_u8; 16];
        let admin_uid = player_uid;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&group_id);
        push_fstring(&mut bytes, "Group");
        push_i32(&mut bytes, 1);
        bytes.extend_from_slice(&player_uid);
        bytes.extend_from_slice(&instance_id);
        bytes.push(1);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 7);
        push_i32(&mut bytes, 12);
        push_i32(&mut bytes, 0);
        push_fstring(&mut bytes, "测试公会");
        bytes.extend_from_slice(&player_uid);
        push_i32(&mut bytes, 0);
        if version_two {
            push_i32(&mut bytes, 2);
            bytes.extend_from_slice(&[1, 2]);
            push_i32(&mut bytes, 99);
        }
        bytes.extend_from_slice(&admin_uid);
        push_i32(&mut bytes, 1);
        bytes.extend_from_slice(&player_uid);
        push_i64(&mut bytes, 123_456);
        push_fstring(&mut bytes, "煜");
        if version_two {
            bytes.push(1);
            push_i32(&mut bytes, 1);
            bytes.push(1);
            push_i32(&mut bytes, 2);
            bytes.extend_from_slice(&[4, 5]);
        }
        bytes.extend_from_slice(&[9, 8, 7, 6]);
        bytes
    }

    #[test]
    fn guild_raw_v1_round_trips_and_exposes_membership() {
        let original = guild_fixture(false);
        let guild = decode_guild_raw(&original).expect("旧版公会应可解析");
        assert_eq!(guild.guild_name, "测试公会");
        assert_eq!(guild.base_camp_level, 12);
        assert_eq!(guild.players.len(), 1);
        assert_eq!(guild.players[0].player_name, "煜");
        assert_eq!(guild.players[0].role, None);
        assert_eq!(guild.admin_player_uid, [2_u8; 16]);
        assert_eq!(encode_guild_raw(&guild).unwrap(), original);
    }

    #[test]
    fn guild_raw_v2_round_trips_and_exposes_roles() {
        let original = guild_fixture(true);
        let guild = decode_guild_raw(&original).expect("新版公会应可解析");
        assert_eq!(guild.players[0].role, Some(1));
        assert_eq!(guild.guild_chest_allowed_roles, Some(vec![1, 2]));
        assert_eq!(guild.role_permissions.len(), 1);
        assert_eq!(encode_guild_raw(&guild).unwrap(), original);
    }

    #[test]
    fn guild_rename_preserves_membership_and_unknown_fields() {
        let original = decode_guild_raw(&guild_fixture(true)).unwrap();
        let mut candidate = original.clone();

        rename_guild_raw(&mut candidate, "新的公会名").expect("应重命名公会");

        assert_eq!(candidate.guild_name, "新的公会名");
        assert_eq!(candidate.players, original.players);
        assert_eq!(candidate.handles, original.handles);
        assert_eq!(candidate.base_ids, original.base_ids);
        assert_eq!(candidate.role_permissions, original.role_permissions);
        assert_eq!(candidate.trailing_bytes, original.trailing_bytes);
        let reparsed = decode_guild_raw(&encode_guild_raw(&candidate).unwrap()).unwrap();
        assert_eq!(reparsed, candidate);
    }

    #[test]
    fn guild_leader_change_updates_v2_roles_and_demotes_old_leader() {
        let mut guild = decode_guild_raw(&guild_fixture(true)).unwrap();
        let previous_admin = guild.admin_player_uid;
        let new_admin = [4_u8; 16];
        guild.players.push(GuildPlayer {
            player_uid: new_admin,
            last_online_real_time: 654_321,
            player_name: "刘".to_string(),
            role: Some(3),
        });

        make_guild_leader_raw(&mut guild, &new_admin).expect("应设置新队长");

        assert_eq!(guild.admin_player_uid, new_admin);
        assert_eq!(
            guild
                .players
                .iter()
                .find(|player| player.player_uid == new_admin)
                .unwrap()
                .role,
            Some(1)
        );
        assert_eq!(
            guild
                .players
                .iter()
                .find(|player| player.player_uid == previous_admin)
                .unwrap()
                .role,
            Some(2)
        );
    }

    #[test]
    fn modifier_world_reads_real_guild_memberships_from_backup() {
        let source =
            std::path::Path::new("F:/1/server/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE");
        if !source.join("Level.sav").is_file() {
            eprintln!("[skip] F:\\1 真实服务器备份不存在");
            return;
        }

        let state =
            get_modifier_world_impl(source.to_str().unwrap()).expect("应解析真实服务器备份");
        assert!(!state.players.is_empty(), "应读取玩家");
        assert!(!state.guilds.is_empty(), "应读取真实公会");
        assert!(state
            .guilds
            .iter()
            .all(|guild| !guild.admin_player_uid.is_empty()));
        assert!(state.guilds.iter().all(|guild| !guild.members.is_empty()));
        assert!(
            state.players.iter().any(|player| player.guild_id.is_some()),
            "至少一名玩家应关联到公会",
        );
        assert!(
            state.players.iter().all(|player| player.pal_count == 0),
            "空角色容器不能生成虚假的帕鲁数量",
        );
    }

    #[test]
    fn modifier_world_reads_owned_pal_counts_from_local_backup() {
        let source = std::path::Path::new(
            "F:/1/local/SaveGames/76561199381352956/1D5D1F304D3AA1FE2818BA98D5223DFE",
        );
        if !source.join("Level.sav").is_file() {
            eprintln!("[skip] F:\\1 真实单机备份不存在");
            return;
        }

        let state = get_modifier_world_impl(source.to_str().unwrap()).expect("应解析真实单机备份");
        assert!(!state.players.is_empty(), "应读取单机玩家");
        assert!(
            state.players.iter().any(|player| player.pal_count > 0),
            "真实单机世界中至少一名玩家应显示其拥有的帕鲁数量",
        );
    }

    #[test]
    fn preview_rejects_level_outside_supported_range_before_reading_world() {
        let request = ModifierActionRequest {
            world_path: "missing".to_string(),
            action: ModifierAction::SetPlayerLevel,
            player_uid: Some("PLAYER".to_string()),
            guild_id: None,
            value: None,
            level: Some(81),
            technology_points: None,
            ancient_technology_points: None,
        };

        let error = preview_modifier_action_impl(&request).expect_err("81 级必须被拒绝");
        assert!(error.contains("1–80"));
    }

    #[test]
    fn delete_player_preview_uses_real_guild_and_file_impact() {
        let source =
            std::path::Path::new("F:/1/server/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE");
        if !source.join("Level.sav").is_file() {
            eprintln!("[skip] F:\\1 真实服务器备份不存在");
            return;
        }
        let state = get_modifier_world_impl(source.to_str().unwrap()).unwrap();
        let player = state.players.first().expect("样本应含玩家");
        let preview = preview_modifier_action_impl(&ModifierActionRequest {
            world_path: source.to_string_lossy().into_owned(),
            action: ModifierAction::DeletePlayer,
            player_uid: Some(player.player_uid.clone()),
            guild_id: None,
            value: None,
            level: None,
            technology_points: None,
            ancient_technology_points: None,
        })
        .expect("删除预览应成功");

        assert_eq!(preview.confirmation_name, player.nickname);
        assert_eq!(preview.player_count, 1);
        assert!(preview.file_count >= 2, "至少影响 Level.sav 和玩家文件");
    }

    #[test]
    fn apply_rename_guild_uses_snapshot_and_only_changes_backup_copy() {
        let source =
            std::path::Path::new("F:/1/server/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE");
        if !source.join("Level.sav").is_file() {
            eprintln!("[skip] F:\\1 真实服务器备份不存在");
            return;
        }
        let source_hash = sha2::Sha256::digest(std::fs::read(source.join("Level.sav")).unwrap());
        let test_root = std::env::temp_dir().join(format!(
            "palworld-modifier-rename-guild-{}",
            std::process::id(),
        ));
        let world_copy = test_root.join("world");
        let backup_root = test_root.join("backups");
        let _ = std::fs::remove_dir_all(&test_root);
        let mut copied = 0;
        path_util::copy_dir_recursive(source, &world_copy, &mut copied).unwrap();

        let before = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        let guild = before.guilds.first().expect("样本应有公会");
        let result = apply_modifier_action_in_dir(
            &ModifierActionRequest {
                world_path: world_copy.to_string_lossy().into_owned(),
                action: ModifierAction::RenameGuild,
                player_uid: None,
                guild_id: Some(guild.guild_id.clone()),
                value: Some("副本测试公会".to_string()),
                level: None,
                technology_points: None,
                ancient_technology_points: None,
            },
            &backup_root,
        )
        .expect("应在副本上原子重命名公会");

        assert!(result.ok && result.roundtrip_ok);
        assert!(!result.snapshot_id.is_empty());
        let after = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        assert_eq!(after.guilds.first().unwrap().name, "副本测试公会");
        assert_eq!(
            sha2::Sha256::digest(std::fs::read(source.join("Level.sav")).unwrap()),
            source_hash,
            "F:\\1 原件不得变化",
        );
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn apply_technology_points_creates_snapshot_and_reloads_player_file() {
        let source =
            std::path::Path::new("F:/1/server/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE");
        if !source.join("Level.sav").is_file() {
            eprintln!("[skip] F:\\1 真实服务器备份不存在");
            return;
        }
        let test_root = std::env::temp_dir().join(format!(
            "palworld-modifier-tech-points-{}",
            std::process::id(),
        ));
        let world_copy = test_root.join("world");
        let backup_root = test_root.join("backups");
        let _ = std::fs::remove_dir_all(&test_root);
        let mut copied = 0;
        path_util::copy_dir_recursive(source, &world_copy, &mut copied).unwrap();
        let player = get_modifier_world_impl(world_copy.to_str().unwrap())
            .unwrap()
            .players
            .into_iter()
            .next()
            .expect("样本应有玩家");

        let result = apply_modifier_action_in_dir(
            &ModifierActionRequest {
                world_path: world_copy.to_string_lossy().into_owned(),
                action: ModifierAction::SetTechnologyPoints,
                player_uid: Some(player.player_uid.clone()),
                guild_id: None,
                value: None,
                level: None,
                technology_points: Some(777),
                ancient_technology_points: Some(31),
            },
            &backup_root,
        )
        .expect("科技点修改应走统一快照写入");

        assert!(result.ok && result.roundtrip_ok);
        assert!(!result.snapshot_id.is_empty());
        let after = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        let player_after = after
            .players
            .iter()
            .find(|candidate| candidate.player_uid == player.player_uid)
            .unwrap();
        assert_eq!(player_after.technology_points, 777);
        assert_eq!(player_after.ancient_technology_points, 31);
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn reference_exp_table_returns_cumulative_exp_for_level_42() {
        assert_eq!(total_exp_for_level(42).unwrap(), 1_038_842);
        assert!(total_exp_for_level(0).is_err());
        assert!(total_exp_for_level(81).is_err());
    }

    #[test]
    fn reference_technology_table_contains_588_unique_assets() {
        let assets = technology_assets().expect("应加载同行科技表");
        assert_eq!(assets.len(), 588);
        assert_eq!(
            assets
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            588
        );
    }

    #[test]
    fn apply_player_rename_updates_character_and_guild_member_name() {
        let (test_root, world_copy, backup_root) = real_world_copy("rename-player");
        if !world_copy.join("Level.sav").is_file() {
            return;
        }
        let before = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        let player = before
            .players
            .iter()
            .find(|player| player.guild_id.is_some())
            .expect("样本应有公会玩家");
        let new_name = "副本玩家";

        apply_modifier_action_in_dir(
            &ModifierActionRequest {
                world_path: world_copy.to_string_lossy().into_owned(),
                action: ModifierAction::RenamePlayer,
                player_uid: Some(player.player_uid.clone()),
                guild_id: None,
                value: Some(new_name.to_string()),
                level: None,
                technology_points: None,
                ancient_technology_points: None,
            },
            &backup_root,
        )
        .expect("玩家重命名应成功");

        let after = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        let player_after = after
            .players
            .iter()
            .find(|candidate| candidate.player_uid == player.player_uid)
            .unwrap();
        assert_eq!(player_after.nickname, new_name);
        let guild_after = after
            .guilds
            .iter()
            .find(|guild| Some(&guild.guild_id) == player_after.guild_id.as_ref())
            .unwrap();
        assert_eq!(
            guild_after
                .members
                .iter()
                .find(|member| member.player_uid == player.player_uid)
                .unwrap()
                .player_name,
            new_name,
        );
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn apply_player_level_updates_visible_level_on_world_copy() {
        let (test_root, world_copy, backup_root) = real_world_copy("set-level");
        if !world_copy.join("Level.sav").is_file() {
            return;
        }
        let player = get_modifier_world_impl(world_copy.to_str().unwrap())
            .unwrap()
            .players
            .into_iter()
            .next()
            .expect("样本应有玩家");

        apply_modifier_action_in_dir(
            &ModifierActionRequest {
                world_path: world_copy.to_string_lossy().into_owned(),
                action: ModifierAction::SetPlayerLevel,
                player_uid: Some(player.player_uid.clone()),
                guild_id: None,
                value: None,
                level: Some(42),
                technology_points: None,
                ancient_technology_points: None,
            },
            &backup_root,
        )
        .expect("玩家等级修改应成功");

        let after = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        assert_eq!(
            after
                .players
                .iter()
                .find(|candidate| candidate.player_uid == player.player_uid)
                .unwrap()
                .level,
            42,
        );
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn apply_unlock_all_technologies_merges_all_reference_assets() {
        let (test_root, world_copy, backup_root) = real_world_copy("unlock-tech");
        if !world_copy.join("Level.sav").is_file() {
            return;
        }
        let player = get_modifier_world_impl(world_copy.to_str().unwrap())
            .unwrap()
            .players
            .into_iter()
            .next()
            .expect("样本应有玩家");

        apply_modifier_action_in_dir(
            &ModifierActionRequest {
                world_path: world_copy.to_string_lossy().into_owned(),
                action: ModifierAction::UnlockAllTechnologies,
                player_uid: Some(player.player_uid.clone()),
                guild_id: None,
                value: None,
                level: None,
                technology_points: None,
                ancient_technology_points: None,
            },
            &backup_root,
        )
        .expect("解锁全部科技应成功");

        let unlocked = player_unlocked_technologies(&world_copy, &player.guid).unwrap();
        let expected = technology_assets().unwrap();
        assert!(expected.iter().all(|asset| unlocked.contains(asset)));
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn delete_player_removes_character_owned_pals_files_and_last_guild_on_copy() {
        let source =
            Path::new("F:/1/local/SaveGames/76561199381352956/1D5D1F304D3AA1FE2818BA98D5223DFE");
        let (test_root, world_copy, backup_root) = isolated_world_copy(source, "delete-player");
        if !world_copy.join("Level.sav").is_file() {
            return;
        }
        let source_hash = sha2::Sha256::digest(std::fs::read(source.join("Level.sav")).unwrap());
        let before = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        let player = before.players.first().expect("单机样本应有玩家").clone();
        assert!(player.pal_count > 0, "单机样本应有玩家拥有的帕鲁");
        let before_cspm = cspm_entry_count(&world_copy);

        apply_modifier_action_in_dir(
            &ModifierActionRequest {
                world_path: world_copy.to_string_lossy().into_owned(),
                action: ModifierAction::DeletePlayer,
                player_uid: Some(player.player_uid.clone()),
                guild_id: None,
                value: None,
                level: None,
                technology_points: None,
                ancient_technology_points: None,
            },
            &backup_root,
        )
        .expect("应在副本上删除玩家及其关联数据");

        let after = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        assert!(after.players.is_empty());
        assert!(after.guilds.is_empty(), "最后一名成员被删时应移除公会");
        assert!(
            cspm_entry_count(&world_copy)
                <= before_cspm.saturating_sub(player.pal_count as usize + 1),
            "玩家及其拥有的帕鲁 CSPM 必须被删除",
        );
        assert!(!world_copy
            .join("Players")
            .join(format!("{}.sav", player.guid))
            .exists());
        assert!(!world_copy
            .join("Players")
            .join(format!("{}_dps.sav", player.guid))
            .exists());
        assert_eq!(
            sha2::Sha256::digest(std::fs::read(source.join("Level.sav")).unwrap()),
            source_hash,
            "F:\\1 原件不得变化",
        );
        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn delete_guild_removes_members_owned_pals_files_and_base_indexes_on_copy() {
        let source =
            Path::new("F:/1/local/SaveGames/76561199381352956/1D5D1F304D3AA1FE2818BA98D5223DFE");
        let (test_root, world_copy, backup_root) = isolated_world_copy(source, "delete-guild");
        if !world_copy.join("Level.sav").is_file() {
            return;
        }
        let before = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        let guild = before.guilds.first().expect("单机样本应有公会").clone();
        let guild_raw = guild_raw_from_world(&world_copy, &guild.guild_id);
        let member_guids = before
            .players
            .iter()
            .filter(|player| player.guild_id.as_deref() == Some(&guild.guild_id))
            .map(|player| player.guid.clone())
            .collect::<Vec<_>>();
        let before_cspm = cspm_entry_count(&world_copy);
        let before_bases = world_map_entry_count(&world_copy, "BaseCampSaveData");

        apply_modifier_action_in_dir(
            &ModifierActionRequest {
                world_path: world_copy.to_string_lossy().into_owned(),
                action: ModifierAction::DeleteGuild,
                player_uid: None,
                guild_id: Some(guild.guild_id.clone()),
                value: None,
                level: None,
                technology_points: None,
                ancient_technology_points: None,
            },
            &backup_root,
        )
        .expect("应在副本上删除公会及其关联数据");

        let after = get_modifier_world_impl(world_copy.to_str().unwrap()).unwrap();
        assert!(after
            .guilds
            .iter()
            .all(|candidate| candidate.guild_id != guild.guild_id));
        assert!(after
            .players
            .iter()
            .all(|player| player.guild_id.as_deref() != Some(&guild.guild_id)),);
        assert!(cspm_entry_count(&world_copy) < before_cspm);
        assert_eq!(
            world_map_entry_count(&world_copy, "BaseCampSaveData"),
            before_bases.saturating_sub(guild.base_count),
        );
        let after_sav = sav_io::SavFile::load(&world_copy.join("Level.sav")).unwrap();
        let after_gvas = after_sav.parse().unwrap();
        for base_id in guild_raw.base_ids {
            let residual_paths = guid_paths_in_gvas(&after_gvas, &base_id);
            assert_eq!(
                sav_io::count_guid_in_gvas(&after_gvas, &base_id),
                0,
                "删除公会后不得保留据点 GUID 的结构化引用: {residual_paths:?}",
            );
        }
        for guid in member_guids {
            assert!(!world_copy
                .join("Players")
                .join(format!("{guid}.sav"))
                .exists());
            assert!(!world_copy
                .join("Players")
                .join(format!("{guid}_dps.sav"))
                .exists());
        }
        let _ = std::fs::remove_dir_all(&test_root);
    }

    fn cspm_entry_count(world: &Path) -> usize {
        let sav = sav_io::SavFile::load(&world.join("Level.sav")).unwrap();
        let gvas = sav.parse().unwrap();
        sav_io::top_field(&gvas, "worldSaveData")
            .and_then(sav_io::struct_value)
            .and_then(sav_io::custom_fields)
            .and_then(|fields| sav_io::field(fields, "CharacterSaveParameterMap"))
            .and_then(world_copy::as_props_map)
            .map(|entries| entries.len())
            .unwrap_or_default()
    }

    fn world_map_entry_count(world: &Path, field_name: &str) -> usize {
        let sav = sav_io::SavFile::load(&world.join("Level.sav")).unwrap();
        let gvas = sav.parse().unwrap();
        sav_io::top_field(&gvas, "worldSaveData")
            .and_then(sav_io::struct_value)
            .and_then(sav_io::custom_fields)
            .and_then(|fields| sav_io::field(fields, field_name))
            .and_then(world_copy::as_props_map)
            .map(|entries| entries.len())
            .unwrap_or_default()
    }

    fn guild_raw_from_world(world: &Path, guild_id: &str) -> GuildRaw {
        let sav = sav_io::SavFile::load(&world.join("Level.sav")).unwrap();
        let gvas = sav.parse().unwrap();
        sav_io::top_field(&gvas, "worldSaveData")
            .and_then(sav_io::struct_value)
            .and_then(sav_io::custom_fields)
            .and_then(|fields| sav_io::field(fields, "GroupSaveDataMap"))
            .and_then(world_copy::as_props_map)
            .and_then(|entries| {
                entries
                    .iter()
                    .filter_map(|(_, value)| world_copy::extract_rawdata_bytes(value))
                    .filter_map(|bytes| decode_guild_raw(&bytes).ok())
                    .find(|guild| world_copy::guid_std(&guild.group_id) == guild_id)
            })
            .expect("应读取公会 RawData")
    }

    fn guid_paths_in_gvas(gvas: &gvas::GvasFile, target: &[u8; 16]) -> Vec<String> {
        let mut paths = Vec::new();
        for (name, property) in gvas.properties.iter() {
            collect_guid_paths(property, target, name, &mut paths);
        }
        paths
    }

    fn collect_guid_paths(
        property: &Property,
        target: &[u8; 16],
        path: &str,
        output: &mut Vec<String>,
    ) {
        match property {
            Property::StructProperty(value) => {
                collect_guid_paths_in_struct(&value.value, target, path, output)
            }
            Property::StructPropertyValue(value) => {
                collect_guid_paths_in_struct(value, target, path, output)
            }
            Property::ArrayProperty(gvas::properties::array_property::ArrayProperty::Structs {
                structs,
                ..
            }) => {
                for (index, value) in structs.iter().enumerate() {
                    collect_guid_paths_in_struct(
                        value,
                        target,
                        &format!("{path}[{index}]"),
                        output,
                    );
                }
            }
            Property::ArrayProperty(
                gvas::properties::array_property::ArrayProperty::Properties { properties, .. },
            ) => {
                for (index, value) in properties.iter().enumerate() {
                    collect_guid_paths(value, target, &format!("{path}[{index}]"), output);
                }
            }
            Property::MapProperty(gvas::properties::map_property::MapProperty::Properties {
                value,
                ..
            }) => {
                for (index, (key, value)) in value.iter().enumerate() {
                    collect_guid_paths(key, target, &format!("{path}.key[{index}]"), output);
                    collect_guid_paths(value, target, &format!("{path}.value[{index}]"), output);
                }
            }
            Property::SetProperty(value) => {
                for (index, value) in value.properties.iter().enumerate() {
                    collect_guid_paths(value, target, &format!("{path}[{index}]"), output);
                }
            }
            _ => {}
        }
    }

    fn collect_guid_paths_in_struct(
        value: &StructPropertyValue,
        target: &[u8; 16],
        path: &str,
        output: &mut Vec<String>,
    ) {
        match value {
            StructPropertyValue::Guid(guid) if guid.to_u8() == *target => {
                output.push(path.to_string())
            }
            StructPropertyValue::CustomStruct(fields) => {
                for (name, properties) in fields.iter() {
                    for property in properties {
                        collect_guid_paths(property, target, &format!("{path}.{name}"), output);
                    }
                }
            }
            _ => {}
        }
    }

    fn isolated_world_copy(source: &Path, label: &str) -> (PathBuf, PathBuf, PathBuf) {
        let test_root =
            std::env::temp_dir().join(format!("palworld-modifier-{label}-{}", std::process::id(),));
        let world_copy = test_root.join("world");
        let backup_root = test_root.join("backups");
        let _ = std::fs::remove_dir_all(&test_root);
        if source.join("Level.sav").is_file() {
            std::fs::create_dir_all(&world_copy).unwrap();
            std::fs::copy(source.join("Level.sav"), world_copy.join("Level.sav")).unwrap();
            if source.join("Players").is_dir() {
                let mut copied = 0;
                path_util::copy_dir_recursive(
                    &source.join("Players"),
                    &world_copy.join("Players"),
                    &mut copied,
                )
                .unwrap();
            }
        } else {
            eprintln!("[skip] {} 不存在", source.display());
        }
        (test_root, world_copy, backup_root)
    }

    fn real_world_copy(label: &str) -> (PathBuf, PathBuf, PathBuf) {
        let source = Path::new("F:/1/server/SaveGames/0/1A91A61548C7B6FD7B58B2B70710F7EE");
        let test_root =
            std::env::temp_dir().join(format!("palworld-modifier-{label}-{}", std::process::id(),));
        let world_copy = test_root.join("world");
        let backup_root = test_root.join("backups");
        let _ = std::fs::remove_dir_all(&test_root);
        if source.join("Level.sav").is_file() {
            let mut copied = 0;
            path_util::copy_dir_recursive(source, &world_copy, &mut copied).unwrap();
        } else {
            eprintln!("[skip] F:\\1 真实服务器备份不存在");
        }
        (test_root, world_copy, backup_root)
    }
}
