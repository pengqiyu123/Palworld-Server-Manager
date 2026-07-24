// 与 src-tauri/src/ 下的 Rust struct 一一对应的 TypeScript 类型声明
// 注意：Rust 端所有 struct 均未使用 #[serde(rename_all = "camelCase")]，
// 因此字段名保持 snake_case 风格，与 serde 默认序列化结果一致。

// ==================== server.rs ====================

/**
 * 服务器运行状态
 * 对应 Rust: server.rs#ServerStatus
 */
export interface ServerStatus {
  running: boolean
  pid: number | null
  managed_by_app: boolean
  server_path: string
  log_count: number
}

// ==================== config.rs ====================

/**
 * 单条配置项的元信息（名称、当前值、描述、字段类型及范围）
 * 对应 Rust: config.rs#ConfigValue
 */
export interface ConfigValue {
  name: string
  value: string
  description: string
  field_type: string
  min: number | null
  max: number | null
  step: number | null
}

/**
 * 配置备份文件信息
 * 对应 Rust: config.rs#BackupInfo
 * size_bytes 在 Rust 端为 u64，JSON 序列化为 number
 */
export interface BackupInfo {
  name: string
  timestamp: string
  size_bytes: number
}

/**
 * 一键填充默认配置的结果
 * 对应 Rust: config.rs#FillConfigResult
 */
export interface FillConfigResult {
  /** "already_filled" | "filled_from_template" | "filled_from_defaults" */
  status: string
  /** 实际命中/写入的来源路径或来源标识 */
  source: string
  /** 面向用户的中文提示（直接用于 Toast） */
  message: string
}

// ==================== firewall.rs ====================

/**
 * 防火墙端口开放状态
 * 对应 Rust: firewall.rs#FirewallStatus
 */
export interface FirewallStatus {
  port_8211_open: boolean
  port_27015_open: boolean
  port_25575_open: boolean
  port_8212_open: boolean
}

// ==================== rest_proxy.rs ====================

/** GET /v1/api/info 响应：服名/版本/世界GUID */
export interface ServerInfo {
  version: string
  servername: string
  description: string
  worldguid: string
}

/** GET /v1/api/metrics 响应：FPS/人数/天数/运行时长 */
export interface ServerMetrics {
  currentplayernum: number
  serverfps: number
  serverfpsaverage: number
  serverframetime: number
  days: number
  maxplayernum: number
  basecampnum: number
  uptime: number
}

/** GET /v1/api/players 响应中的单个玩家。
 *  字段名与 REST API 原始 JSON 一致（含大小写混合：playerId / userId / iP）。 */
export interface PlayerInfo {
  name: string
  playerId: string
  userId: string
  iP: string
  ping: number
  location_x: number
  location_y: number
  level: number
}

// ==================== network.rs ====================

/**
 * Radmin VPN 内部检测状态（非 #[command] 返回值，作为公开 struct 一并声明）
 * 对应 Rust: network.rs#RadminStatus
 */
export interface RadminStatus {
  installed: boolean
  virtual_ip: string
  adapter_status: string
}

/**
 * Radmin LAN 综合状态（由 check_radmin_lan_status 命令返回，R2 兼容命令，R3 删除）
 * 对应 Rust: network.rs#RadminLanStatus
 */
export interface RadminLanStatus {
  installed: boolean
  virtual_ip: string
  adapter_status: string
}

/**
 * Radmin 联机就绪度分级（收官新增 5 档检测）
 * 对应 Rust: network.rs#ReadinessLevel（serde rename_all = "UPPERCASE" → "L0".."L4"）
 */
export type ReadinessLevel = 'L0' | 'L1' | 'L2' | 'L3' | 'L4'

/**
 * 下一步引导动作
 * 对应 Rust: network.rs#NextAction
 */
export interface NextAction {
  /** "open_url" | "launch_app" | "show_guide" | "auto_recheck" | "copy_card" */
  action_type: string
  label: string
  payload?: string
}

/**
 * Radmin 联机就绪度完整结构（check_radmin_readiness 命令返回）
 * 对应 Rust: network.rs#RadminReadiness
 */
export interface RadminReadiness {
  level: ReadinessLevel
  virtual_ip: string
  adapter_status: string
  reason?: string
  next_action?: NextAction
}

/** Radmin 虚拟网段前缀（老板确认：26.x.x.x）。前端用于判定虚拟 IP 是否属于 Radmin。 */
export const RADMIN_IP_PATTERN = /^26\./
/** 各档位的中文标签（网络视图渲染用） */
export const READINESS_LABEL: Record<ReadinessLevel, string> = {
  L0: '未装 Radmin',
  L1: 'Radmin 已装未启动',
  L2: 'Radmin 已启动未入网',
  L3: '已入网（等待就绪）',
  L4: '联机就绪 ✅',
}
/** 各档位的语义配色（红/橙/橙/黄/绿） */
export const READINESS_COLOR: Record<ReadinessLevel, string> = {
  L0: 'red',
  L1: 'orange',
  L2: 'orange',
  L3: 'yellow',
  L4: 'green',
}

// ==================== presets.rs ====================

/**
 * 预设元信息
 * 对应 Rust: presets.rs#PresetMeta
 * key_params 在 Rust 端为 Vec<(String, String)>，
 * serde 默认将 tuple 序列化为 JSON 数组，因此 TS 类型为 [string, string][]
 */
export interface PresetMeta {
  name: string
  description: string
  key_params: [string, string][]
}

// ==================== settings.rs ====================

/**
 * 应用本地设置（持久化到 settings.json）
 * 对应 Rust: settings.rs#AppSettings
 */
export interface AppSettings {
  server_path: string
  config_path: string
  rcon_host: string
  rcon_port: number
  rcon_password: string
}

// ==================== Tauri 命令字符串字面量联合类型 ====================

/**
 * 服务器进程相关命令
 */
export type ServerCommand =
  | 'init_server_state'
  | 'start_server'
  | 'stop_server'
  | 'force_stop_server'
  | 'get_server_status'
  | 'get_server_logs'
  | 'clear_server_logs'
  | 'export_server_logs'

/**
 * 配置文件读写相关命令
 */
export type ConfigCommand =
  | 'read_config'
  | 'write_config'
  | 'get_default_config'
  | 'get_config_descriptions'
  | 'list_config_backups'
  | 'restore_config_backup'
  | 'fill_default_config'
  | 'is_config_initialized'

/**
 * 预设相关命令
 */
export type PresetCommand =
  | 'list_presets'
  | 'apply_preset'

/**
 * 防火墙规则相关命令
 */
export type FirewallCommand =
  | 'check_firewall_rules'
  | 'add_firewall_rules'

/**
 * 网络与端口相关命令
 */
export type NetworkCommand =
  | 'check_port_usage'
  | 'check_radmin_lan_status'
  | 'check_radmin_readiness'

/**
 * RCON 远程控制相关命令
 */
export type RconCommand =
  | 'rcon_connect'
  | 'rcon_send_command'
  | 'rcon_disconnect'
  | 'rcon_is_connected'

/**
 * 启动类命令（收官 F2 启动 Radmin VPN / F3 启动游戏本体）
 */
export type LauncherCommand =
  | 'launch_radmin_vpn'
  | 'launch_game'

// ==================== save_transfer.rs (F4 存档/角色转移 MVP) ====================

/**
 * 世界基本信息（discover_worlds 返回，对应 Rust: save_transfer.rs#WorldInfo）
 */
export interface WorldInfo {
  name: string
  path: string
  has_level_sav: boolean
  /** Players/ 下角色存档(.sav)数量 */
  player_count: number
  /** 整目录字节数 */
  size_bytes: number
  /** 来源：专用服="server"；本机单机="appdata"(AppData) | "steam"(Steam 库) */
  source: string
  /** 世界 GUID（GUID 嵌套布局下的子层目录名；扁平布局为 null） */
  guid: string | null
  /** 世界目录最后修改时间（格式化字符串） */
  modified_at: string | null
}

/**
 * discover_worlds 返回：含实际发现的存档根（对应 Rust: save_transfer.rs#DiscoverResult）
 */
export interface DiscoverResult {
  /** 实际使用的 SaveGames 根目录 */
  save_root: string
  /** 是否非 server_path 直拼（向上扫描/默认位置发现）→ 前端应提示核对 */
  auto_discovered: boolean
  worlds: WorldInfo[]
}

/**
 * 单个世界备份条目（list_world_backups 返回，对应 Rust: save_transfer.rs#WorldBackupInfo）
 */
export interface WorldBackupInfo {
  /** 备份 id（= _backups/<world>/ 下的时间戳子目录名） */
  backup_id: string
  path: string
  created_at: string
  size_bytes: number
}

/**
 * 存档/角色转移命令（收官 F4 MVP）
 */
export type SaveTransferCommand =
  | 'discover_worlds'
  | 'backup_world'
  | 'list_world_backups'
  | 'restore_world'
  | 'export_character'
  | 'import_character'

/**
 * 应用设置相关命令
 */
export type SettingsCommand =
  | 'load_app_settings'
  | 'save_app_settings'

// ==================== 全局错误分类（M3-C 全局错误 toast） ====================

/**
 * 前端错误分类：用于全局 toast 的图标 / 文案与 60s 防抖。
 * 映射规则见 useToast.ts 的 classifyError。
 */
export type ErrorClass =
  | 'NetworkUnreachable'
  | 'AuthFailed'
  | 'PortBlocked'
  | 'ProcessDown'
  | 'Other'

// ==================== onboarding（7 步联机流程，收官新增） ====================

/** 7 步流程的步标识 */
export type StepId = 's1' | 's2' | 's3' | 's4' | 's5' | 's6' | 's7'

/** 单步状态（全由底层 store 派生，不允许外部手动 setStatus） */
export interface OnboardingStepState {
  status: 'idle' | 'pass' | 'fail'
  reason?: string
  action?: NextAction
}

// ==================== save_edit.rs (F5 存档迁移/改写 MVP) ====================
// 字段名保持 snake_case，与 Rust 端结构体（未启用 rename_all）序列化结果一致。

/** 世界中的单个玩家（f5_world_summary 返回，对应 Rust: save_edit::models::PlayerEntry） */
export interface PlayerEntry {
  player_uid: string
  instance_id: string
  guid: string
  nickname: string
  level: number
  guild_id: string | null
  pal_count: number
  last_online: string
  is_host: boolean
}

/** 公会（f5_world_summary 返回，对应 Rust: save_edit::models::GuildEntry） */
export interface GuildEntry {
  guild_id: string
  name: string
  admin_player_uid: string
  players: string[]
  handle_ids: string[]
}

/** 世界摘要（f5_world_summary 返回，对应 Rust: save_edit::models::WorldSummary） */
export interface WorldSummary {
  world_name: string
  players: PlayerEntry[]
  guilds: GuildEntry[]
}

/** 角色转移子集勾选（L4，对应 Rust: save_edit::models::TransferSubset） */
export interface TransferSubset {
  character: boolean
  guild: boolean
  tech: boolean
  inventory: boolean
  pals: boolean
  appearance: boolean
}

/** Fix Host Save 请求（对应 Rust: save_edit::models::FixHostRequest） */
export interface FixHostRequest {
  world: string
  old_host_guid: string
  new_char_guid: string
}

/** 整包世界迁移请求（对应 Rust: save_edit::models::MigrateRequest） */
export interface MigrateRequest {
  source_world: string
  target_world: string
  /** 源类型："server"（默认，source_world 为世界名）| "local"（source_world 为本地世界绝对路径，后端穿透定位数据层）。 */
  source_type: string
  delete_world_option: boolean
}

/** 跨服角色转移请求（对应 Rust: save_edit::models::TransferRequest） */
export interface TransferRequest {
  source_world: string
  target_world: string
  selected_players: string[]
  subset: TransferSubset
  strategy: string
}

/** 科技点编辑请求（对应 Rust: save_edit::models::TechEditRequest） */
export interface TechEditRequest {
  world: string
  player_guid: string
  add_assets: string[]
  remove_assets: string[]
  mode: string
}

/** 玩家基础属性编辑请求（对应 Rust: save_edit::models::PlayerAttrRequest） */
export interface PlayerAttrRequest {
  world: string
  player_guid: string
  rename: string | null
  level: number | null
  max_all: boolean
}

/** 改写类命令的统一返回（对应 Rust: save_edit::models::EditResult） */
export interface EditResult {
  ok: boolean
  backup_id: string
  roundtrip_ok: boolean
  warnings: string[]
}

/** 科技信息（f5_tech_list 返回，对应 Rust: save_edit::models::TechInfo） */
export interface TechInfo {
  name: string
  asset: string
  tech_type: string
}

/**
 * 存档迁移/改写命令（F5）
 */
export type MigrationCommand =
  | 'f5_world_summary'
  | 'f5_tech_list'
  | 'fix_host_save'
  | 'migrate_world_to_server'
  | 'transfer_character'
  | 'edit_tech'
  | 'edit_player_attr'
