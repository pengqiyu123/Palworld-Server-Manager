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

/// 单条 UID 映射（阶段 B/C：本地玩家旧 UID → 服务器新 UID）。
///
/// `old_uid` / `new_uid` 可为 registry 32-hex（如 `3F5D130B...`）或带连字符标准 GUID 形式，
/// 后端统一经 `sav_io::guid_bytes` 解析为原始 16 字节（registry 格式，与磁盘文件名一致）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UidMapping {
    /// 旧 UID（本地玩家角色存档文件名，去 .sav）。
    pub old_uid: String,
    /// 新 UID（服务器新建角色的存档文件名，去 .sav）。
    pub new_uid: String,
}

/// 分阶段迁移请求（v2：A 世界；或 B 角色 + C 公会）。
///
/// 阶段 A 等于整包世界拷贝（复用 `MigrateRequest` 语义）；阶段 B/C 由 `mappings` 驱动。
/// 阶段 A 与 B/C 必须分两次执行：先迁移世界，进入专用服创建目标角色后再执行 B+C。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreePhaseMigrationRequest {
    /// 源世界名或本地世界绝对路径（见 `source_type`）。
    pub source_world: String,
    /// 目标世界名（专用服世界，通常为 `0`）。
    pub target_world: String,
    /// 源类型："server"（默认，source_world 为世界名）| "local"（source_world 为本地世界绝对路径）。
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// 拷贝后是否删除目标 WorldOption.sav（R5）。
    #[serde(default)]
    pub delete_world_option: bool,
    /// 旧→新 UID 映射（阶段 B 角色替换 + 阶段 C 公会绑定共用）。
    #[serde(default)]
    pub mappings: Vec<UidMapping>,
    /// 是否执行阶段 A（整包世界拷贝）。默认 true 以兼容旧调用。
    #[serde(default = "default_true")]
    pub run_phase_a: bool,
    /// 是否执行阶段 B（旧/新角色身份对称交换）。
    #[serde(default = "default_true")]
    pub run_phase_b: bool,
    /// 是否执行阶段 C（公会 RawData 内管理员、成员、角色句柄与所有者重绑）。
    #[serde(default = "default_true")]
    pub run_phase_c: bool,
}

/// 迁移回滚请求（v2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRequest {
    /// 迁移前生成的整份快照 id（`migrate_world_v2` 返回的 `backup_id`）。
    pub backup_id: String,
}

/// 三阶段迁移结果（v2）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationResult {
    #[serde(default)]
    pub ok: bool,
    /// 整份快照 id（用于回滚）。
    #[serde(default)]
    pub backup_id: String,
    /// 阶段 A 拷贝的文件数。
    #[serde(default)]
    pub phase_a_copied: usize,
    /// 阶段 B 改写的文件数。
    #[serde(default)]
    pub phase_b_changed: usize,
    /// 阶段 C 改写（命中并替换）的 UID 映射数。
    #[serde(default)]
    pub phase_c_changed: usize,
    /// 提示 / 告警。
    #[serde(default)]
    pub warnings: Vec<String>,
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

/// 修改指定角色的普通科技点和古代科技点。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePlayerTechnologyPointsRequest {
    /// 由世界列表返回的世界目录；后端会定位实际含 Level.sav 的数据层。
    pub world_path: String,
    pub player_guid: String,
    pub technology_points: i32,
    pub ancient_technology_points: i32,
}

/// 修改器读取角色科技点时使用的只读请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerTechnologyPointsRequest {
    /// 由世界列表返回的世界目录；后端会定位实际含 Level.sav 的数据层。
    pub world_path: String,
    /// Players/<guid>.sav 的文件名基底。
    pub player_guid: String,
}

/// 角色科技点摘要。字段名与参考项目的 `SaveData` 中真实字段对应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerTechnologyPoints {
    pub technology_points: i32,
    pub ancient_technology_points: i32,
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
