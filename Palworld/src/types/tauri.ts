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
