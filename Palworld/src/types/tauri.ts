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
  /** 服务器进程已绑定游戏端口，可接受玩家连接。 */
  ready: boolean
  pid: number | null
  managed_by_app: boolean
  server_path: string
  log_count: number
}

// ==================== config.rs ====================

/** 本机 REST 管理接口连接结果。端点和服务器信息均来自真实探测。 */
export interface ManagementConnectionInfo {
  message: string
  host: string
  port: number
  servername: string
  version: string
}

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
  radmin_path: string
  local_save_roots: string[]
  server_save_roots: string[]
  backup_root: string
  backup_roots: string[]
  migration_backup_notice_seen: boolean
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

export type ManagementCommand =
  | 'rest_management_connect'
  | 'rest_execute_management_command'
  | 'rest_save_world'

/**
 * 启动类命令（收官 F2 启动 Radmin VPN / F3 启动游戏本体）
 */
export type LauncherCommand =
  | 'launch_radmin_vpn'
  | 'validate_radmin_path'
  | 'detect_palworld_game'
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

export type BackupWorldClass = 'local' | 'server'
export type BackupKind = 'full' | 'snapshot'
export type BackupState = 'applying' | 'committed' | 'recovery_required'

export interface BackupFileFingerprint {
  relative_path: string
  size: number
  sha256: string | null
  absent: boolean
}

export interface BackupManifest {
  schema_version: number
  id: string
  world_id: string
  world_name: string
  world_path: string
  world_class: BackupWorldClass
  kind: BackupKind
  state: BackupState
  source: string
  created_at_ms: number
  total_size: number
  player_count: number | null
  save_version: string | null
  files: BackupFileFingerprint[]
  workflow_id: string | null
}

export interface BackupIndex {
  schema_version: number
  rebuilt_at_ms: number
  backups: BackupManifest[]
}

export type MigrationWorkflowStatus =
  | 'prepared'
  | 'applying'
  | 'awaiting_verification'
  | 'committed'
  | 'recovery_required'
  | 'rolled_back'

export type MigrationWorkflowStage =
  | 'created'
  | 'backup_created'
  | 'world_migrated'
  | 'awaiting_server_character'
  | 'character_transferred'
  | 'guild_restored'
  | 'awaiting_game_verification'
  | 'completed'

export interface WorkflowCharacterIdentity {
  source_player_file: string
  target_player_file: string
  source_player_uid: string
  source_instance_id: string
  source_group_id: string
  source_was_guild_admin: boolean
  target_player_uid: string
  target_instance_id: string
  target_group_id: string
}

export interface MigrationWorkflow {
  schema_version: number
  id: string
  world_id: string
  source_world_path: string
  target_world_path: string
  full_backup_id: string | null
  snapshot_ids: string[]
  identity: WorkflowCharacterIdentity | null
  status: MigrationWorkflowStatus
  stage: MigrationWorkflowStage
  current_step: string
  created_at_ms: number
  updated_at_ms: number
  error: string | null
}

export interface SaveOperationProgress {
  request_id: string
  phase: string
  label: string
}

export interface MigrateWorldV4Request {
  request_id: string
  source_path: string
  source_name: string
  target_world: string
  preserve_server_config: boolean
}

export interface WorldMigrationV4Outcome {
  workflow: MigrationWorkflow
  backup: BackupManifest
  copied_files: number
}

export interface TransferFullCharacterV4Request {
  request_id: string
  workflow_id: string
  source_player_file: string
  target_player_file: string
}

export interface ImportFriendCharacterV4Request {
  request_id: string
  source_world_path: string
  target_world_path: string
  source_player_file: string
  target_player_file: string
}

export interface TransferFullCharacterV4Outcome {
  workflow: MigrationWorkflow
  snapshot: BackupManifest
  inventory_containers: number
  character_containers: number
  pals: number
  dynamic_items: number
}

export interface WorkflowActionV4Request {
  request_id: string
  workflow_id: string
}

export interface RestoreOriginalGuildV4Outcome {
  workflow: MigrationWorkflow
  snapshot: BackupManifest
  changed_references: number
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

/** 修改指定角色的普通科技点和古代科技点。 */
export interface UpdatePlayerTechnologyPointsRequest {
  world_path: string
  player_guid: string
  technology_points: number
  ancient_technology_points: number
}

/** 改写类命令的统一返回（对应 Rust: save_edit::models::EditResult） */
export interface EditResult {
  ok: boolean
  backup_id: string
  roundtrip_ok: boolean
  warnings: string[]
}

/** 三阶段迁移的单个 UID 映射：旧角色 UID → 新角色 UID（均为 32-hex registry 串）。
 *  对应 Rust: save_edit::models::UidMapping */
export interface UidMapping {
  old_uid: string
  new_uid: string
}

/** 分阶段迁移请求（A 整包拷贝；或 B 角色身份交换 + C 公会绑定）。
 *  对应 Rust: save_edit::models::ThreePhaseMigrationRequest。
 *  source_type / delete_world_option / run_phase_a / run_phase_b / run_phase_c 均有 Rust 端 serde 默认值，
 *  前端必须显式传递阶段开关；A 与 B/C 不得在同一次请求中执行。 */
export interface ThreePhaseMigrationRequest {
  source_world: string
  target_world: string
  /** 源类型："server"（默认，source_world 为世界名）| "local"（本地世界绝对路径）。 */
  source_type?: string
  delete_world_option?: boolean
  mappings: UidMapping[]
  /** 是否执行阶段 A（整包世界拷贝）。默认 true。 */
  run_phase_a?: boolean
  /** 是否执行阶段 B（旧/新角色身份对称交换）。默认 true。 */
  run_phase_b?: boolean
  /** 是否执行阶段 C（公会管理员、成员、角色句柄与所有者重绑）。默认 true。 */
  run_phase_c?: boolean
}

/** 三阶段迁移回滚请求（用整份快照还原 SaveGames/0/）。
 *  对应 Rust: save_edit::models::RollbackRequest */
export interface RollbackRequest {
  backup_id: string
}

/** 三阶段迁移返回（对应 Rust: save_edit::models::MigrationResult） */
export interface MigrationResult {
  ok: boolean
  backup_id: string
  phase_a_copied: number
  phase_b_changed: number
  phase_c_changed: number
  warnings: string[]
}

/** 修改器读取的角色科技点。 */
export interface PlayerTechnologyPoints {
  technology_points: number
  ancient_technology_points: number
}

export interface PlayerTechnologyPointsRequest {
  world_path: string
  player_guid: string
}

export type ModifierAction =
  | 'rename_player'
  | 'set_player_level'
  | 'set_technology_points'
  | 'unlock_all_technologies'
  | 'delete_player'
  | 'make_guild_leader'
  | 'rename_guild'
  | 'delete_guild'

export interface ModifierPlayer {
  player_uid: string
  guid: string
  nickname: string
  level: number
  pal_count: number
  guild_id: string | null
  guild_name: string | null
  role: string
  is_leader: boolean
  last_online: string | null
  technology_points: number
  ancient_technology_points: number
}

export interface ModifierGuild {
  guild_id: string
  name: string
  level: number
  leader_uid: string
  leader_name: string
  member_count: number
  base_count: number
}

export interface ModifierWorldState {
  world_name: string
  players: ModifierPlayer[]
  guilds: ModifierGuild[]
  server_running: boolean
  game_running: boolean
}

export interface ModifierOperationProgress {
  phase:
    | 'checking_processes'
    | 'creating_snapshot'
    | 'building_changes'
    | 'writing_save'
    | 'verifying_save'
    | 'refreshing_data'
  label: string
}

export interface ModifierWorldEntry {
  name: string
  path: string
}

export interface ModifierActionRequest {
  world_path: string
  action: ModifierAction
  player_uid?: string
  guild_id?: string
  value?: string
  level?: number
  technology_points?: number
  ancient_technology_points?: number
}

export interface ModifierActionPreview {
  confirmation_name: string
  player_count: number
  pal_count: number
  base_count: number
  file_count: number
  summary: string
}

export interface ModifierActionResult {
  ok: boolean
  snapshot_id: string
  roundtrip_ok: boolean
  message: string
}

/**
 * 存档迁移/改写命令（F5）
 */
export type MigrationCommand =
  | 'f5_world_summary'
  | 'get_player_technology_points'
  | 'fix_host_save'
  | 'migrate_world_to_server'
  | 'migrate_world_v2'
  | 'rollback_migration_v2'
  | 'transfer_character'
  | 'update_player_technology_points'
