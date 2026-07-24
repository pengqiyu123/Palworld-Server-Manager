//! F5 领域模型类型（与架构文档类图 §3 一致）。
//!
//! 所有结构体均 `serde` 派生，且字段保持 **snake_case**（Rust 端未启用
//! `rename_all = "camelCase"`），与现有 `src/types/tauri.ts` 的约定一致，
//! 前端用 camelCase 传参时由 Tauri 自动映射为 snake_case。

use serde::{Deserialize, Serialize};

/// 世界中的单个玩家（L3 玩家列表 / 角色转移目标）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerEntry {
    /// 玩家 UID（存档内 SaveData.PlayerUId 的格式化字符串）。
    pub player_uid: String,
    /// 角色实例 ID（IndividualId.InstanceId）。
    pub instance_id: String,
    /// 角色存档文件名（去 .sav），即 `Players/<guid>.sav` 的 guid。
    pub guid: String,
    /// 昵称（NickName）。
    #[serde(default)]
    pub nickname: String,
    /// 等级。
    #[serde(default)]
    pub level: u32,
    /// 所属公会 ID（无则为 None）。
    #[serde(default)]
    pub guild_id: Option<String>,
    /// 拥有的帕鲁数（估算）。
    #[serde(default)]
    pub pal_count: u32,
    /// 最近在线（人类可读，如 "3d 2h" 或 "Unknown"）。
    #[serde(default)]
    pub last_online: String,
    /// 是否为当前世界主机（host）。
    #[serde(default)]
    pub is_host: bool,
}

/// 公会（GroupSaveDataMap 解析结果）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildEntry {
    /// 公会 ID（GUID 字符串）。
    pub guild_id: String,
    /// 公会名。
    #[serde(default)]
    pub name: String,
    /// 管理员玩家 UID。
    #[serde(default)]
    pub admin_player_uid: String,
    /// 成员玩家 UID 列表。
    #[serde(default)]
    pub players: Vec<String>,
    /// 公会角色句柄 ID（instance id）列表。
    #[serde(default)]
    pub handle_ids: Vec<String>,
}

/// 世界摘要（f5_world_summary 返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSummary {
    pub world_name: String,
    #[serde(default)]
    pub players: Vec<PlayerEntry>,
    #[serde(default)]
    pub guilds: Vec<GuildEntry>,
}

/// 角色转移子集勾选（L4）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSubset {
    /// 主角（含 SaveParameter 基础信息）。
    #[serde(default = "default_true")]
    pub character: bool,
    /// 工会（同世界保留；跨世界合并留 P2，仅提示）。
    #[serde(default)]
    pub guild: bool,
    /// 科技点。
    #[serde(default)]
    pub tech: bool,
    /// 背包。
    #[serde(default)]
    pub inventory: bool,
    /// 帕鲁。
    #[serde(default)]
    pub pals: bool,
    /// 外观。
    #[serde(default)]
    pub appearance: bool,
}

fn default_true() -> bool {
    true
}

/// Fix Host Save 请求（U01 灵魂步骤）。
///
/// `old_host_guid` = 旧主机角色存档文件名（去 .sav，多为 `00000001`）；
/// `new_char_guid` = 专用服新建角色的存档文件名（去 .sav）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixHostRequest {
    pub world: String,
    pub old_host_guid: String,
    pub new_char_guid: String,
}

/// 整包世界迁移请求（T03）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateRequest {
    pub source_world: String,
    pub target_world: String,
    /// 源类型：
    /// - `"server"`（默认）：`source_world` 为服务器世界名，按服务器 SaveGames 根解析。
    /// - `"local"`：`source_world` 为**本地世界绝对路径**，内部用 `find_world_data_dir`
    ///   有界穿透定位数据层（兼容本机单机 2 层嵌套 `<SteamID>/<timestamp>/<GUID>`）。
    /// serde default = "server"，保持旧调用（未传 source_type）向后兼容。
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// 是否在拷贝后删除目标世界多余/过期的 WorldOption.sav（R5）。
    #[serde(default)]
    pub delete_world_option: bool,
}

/// 默认源类型：保持旧调用兼容（未传 source_type 时按服务器世界处理）。
fn default_source_type() -> String {
    "server".to_string()
}

/// 跨服角色转移请求（T04）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub source_world: String,
    pub target_world: String,
    /// 选中的源玩家 GUID 列表（去 .sav）。
    #[serde(default)]
    pub selected_players: Vec<String>,
    pub subset: TransferSubset,
    /// 覆盖策略："overwrite" | "new_instance"。
    #[serde(default)]
    pub strategy: String,
}

/// 科技点编辑请求（T05）。
///
/// `add_assets` / `remove_assets` 为科技 asset 名（如 "StonePile"），
/// 来自 `world_data.json` 的 `technology[]`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechEditRequest {
    pub world: String,
    pub player_guid: String,
    #[serde(default)]
    pub add_assets: Vec<String>,
    #[serde(default)]
    pub remove_assets: Vec<String>,
    /// "single" | "batch"。
    #[serde(default)]
    pub mode: String,
}

/// 玩家基础属性编辑请求（T05）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAttrRequest {
    pub world: String,
    pub player_guid: String,
    /// 改名（None 表示不改）。
    #[serde(default)]
    pub rename: Option<String>,
    /// 设等级（None 表示不改）。
    #[serde(default)]
    pub level: Option<u32>,
    /// 是否将关键属性拉满（Max All）。
    #[serde(default)]
    pub max_all: bool,
}

/// 改写类命令的统一返回（含备份 id 与 round-trip 校验结果）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EditResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub backup_id: String,
    #[serde(default)]
    pub roundtrip_ok: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// 科技信息（f5_tech_list 返回，来自 vendored world_data.json）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechInfo {
    pub name: String,
    pub asset: String,
    #[serde(default)]
    pub tech_type: String,
}
