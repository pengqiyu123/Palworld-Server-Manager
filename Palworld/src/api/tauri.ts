import { invoke } from '@tauri-apps/api/core'
import type {
  ServerStatus,
  FirewallStatus,
  ConfigValue,
  RadminLanStatus,
  RadminReadiness,
  AppSettings,
  PresetMeta,
  BackupInfo,
  ServerInfo,
  ServerMetrics,
  PlayerInfo,
  DiscoverResult,
  WorldBackupInfo,
} from '@/types/tauri'

/**
 * 通用 invoke 封装：捕获 Tauri 后端错误并统一转换为 Error，
 * 同时为调用方提供泛型化的返回类型。
 *
 * Tauri 的 invoke 在出错时 reject 的值通常是字符串，
 * 也可能是 { message: string } 形式的对象；本函数将其归一为 Error。
 */
async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (err) {
    if (typeof err === 'string') {
      throw new Error(err)
    }
    if (err && typeof err === 'object' && 'message' in err) {
      throw new Error((err as { message: string }).message)
    }
    throw new Error(String(err))
  }
}

/**
 * 按功能分组的 Tauri 命令封装，覆盖全部 #[command] 函数。
 * 参数名与 Rust 端 snake_case 参数名保持一致，invoke 时透传。
 * 注意：Tauri 2 默认会将 JS 端 camelCase 参数名映射到 Rust snake_case，
 * 单词参数（path/port/name 等）两种风格一致；多词参数使用 camelCase（如 serverPath）。
 */
export const api = {
  // === server.rs (7 个) ===
  server: {
    init: () => tauriInvoke<ServerStatus>('init_server_state'),
    start: (path: string) => tauriInvoke<ServerStatus>('start_server', { path }),
    stop: () => tauriInvoke<ServerStatus>('stop_server'),
    getStatus: () => tauriInvoke<ServerStatus>('get_server_status'),
    getLogs: () => tauriInvoke<string[]>('get_server_logs'),
    clearLogs: () => tauriInvoke<void>('clear_server_logs'),
    // 导出日志到指定路径，返回写入的字节数
    exportLogs: (path: string) => tauriInvoke<number>('export_server_logs', { path }),
  },

  // === config.rs + presets.rs (8 个) ===
  // 预设与备份命令归入 config 命名空间，便于 ConfigView 统一调用
  config: {
    read: (path: string) => tauriInvoke<Record<string, string>>('read_config', { path }),
    write: (path: string, config: Record<string, string>) =>
      tauriInvoke<string>('write_config', { path, config }),
    getDefault: () => tauriInvoke<Record<string, string>>('get_default_config'),
    getDescriptions: () => tauriInvoke<ConfigValue[]>('get_config_descriptions'),
    // 列出全部预设元信息
    listPresets: () => tauriInvoke<PresetMeta[]>('list_presets'),
    // 套用预设：将预设参数合并到调用方传入的 config（缺失不覆盖）
    applyPreset: (name: string, config: Record<string, string>) =>
      tauriInvoke<Record<string, string>>('apply_preset', { name, config }),
    // 列出全部配置备份
    listBackups: () => tauriInvoke<BackupInfo[]>('list_config_backups'),
    // 恢复指定备份到 server_path 下的配置文件
    restoreBackup: (name: string, serverPath: string) =>
      tauriInvoke<string>('restore_config_backup', { name, serverPath }),
  },

  // === firewall.rs (2 个) ===
  firewall: {
    check: () => tauriInvoke<FirewallStatus>('check_firewall_rules'),
    addRules: () => tauriInvoke<string>('add_firewall_rules'),
  },

  // === network.rs (3 个) ===
  network: {
    // Rust 返回 Option<String>，未占用时为 None，已占用时为 Some(String)
    checkPortUsage: (port: number) => tauriInvoke<string | null>('check_port_usage', { port }),
    checkRadminLan: () => tauriInvoke<RadminLanStatus>('check_radmin_lan_status'),
    // 收官 5 档分级检测（前端零多次调用，一次拿全）
    checkReadiness: (serverPath: string) =>
      tauriInvoke<RadminReadiness>('check_radmin_readiness', { serverPath }),
  },

  // === rcon.rs (5 个) ===
  rcon: {
    // 返回成功提示字符串，如 "RCON连接成功"
    connect: (host: string, port: number, password: string) =>
      tauriInvoke<string>('rcon_connect', { host, port, password }),
    // 新命令（Q2）：仅传 server_path，host 固定 127.0.0.1，密码/端口从 ini 读取（不进前端 JS）
    connectUsingConfig: (serverPath: string) =>
      tauriInvoke<string>('rcon_connect_using_config', { serverPath }),
    // 返回 RCON 服务器的响应文本
    send: (command: string) => tauriInvoke<string>('rcon_send_command', { command }),
    disconnect: () => tauriInvoke<void>('rcon_disconnect'),
    isConnected: () => tauriInvoke<boolean>('rcon_is_connected'),
  },

  // === launcher.rs (2 个 · 收官 F2/F3) ===
  launcher: {
    /** F2：启动 Radmin VPN 应用（仅拉起，加入网络由用户操作）。返回 "已启动 Radmin VPN" 或明确错误。 */
    launchRadminVpn: () => tauriInvoke<string>('launch_radmin_vpn'),
    /** F3：启动游戏本体（优先 Steam 协议 steam://rungame/1623730，兜底直接拉起 Palworld.exe）。 */
    launchGame: () => tauriInvoke<string>('launch_game'),
  },

  // === save_transfer.rs (6 个 · 收官 F4 存档/角色转移 MVP) ===
  save: {
    /** 扫描 SaveGames 下含 Level.sav 的世界列表（自动发现存档根，回报路径）。 */
    discoverWorlds: () => tauriInvoke<DiscoverResult>('discover_worlds'),
    /** 整目录备份世界到备份夹（默认 <SaveGames>/_backups/<world>/<timestamp>/）。 */
    backupWorld: (worldName: string, dest?: string) =>
      tauriInvoke<string>('backup_world', { worldName, dest }),
    /** 列出某世界的已有备份。 */
    listWorldBackups: (worldName: string) =>
      tauriInvoke<WorldBackupInfo[]>('list_world_backups', { worldName }),
    /** 把备份整体覆盖回当前世界目录（UI 侧需二次确认 + 提醒先停服）。 */
    restoreWorld: (worldName: string, backupId: string) =>
      tauriInvoke<string>('restore_world', { worldName, backupId }),
    /** 导出单个角色存档到指定路径（保持 <steam_id>.sav）。 */
    exportCharacter: (worldName: string, steamId: string, destPath: string) =>
      tauriInvoke<string>('export_character', { worldName, steamId, destPath }),
    /** 导入角色存档（保持 steam_id 不变，覆盖同名文件）。 */
    importCharacter: (worldName: string, steamId: string, srcPath: string) =>
      tauriInvoke<string>('import_character', { worldName, steamId, srcPath }),
  },

  // === settings.rs (2 个) ===
  settings: {
    load: () => tauriInvoke<AppSettings>('load_app_settings'),
    save: (settings: AppSettings) => tauriInvoke<void>('save_app_settings', { settings }),
  },

  // === steam_detect.rs (1 个) ===
  steam: {
    // 自动探测 Steam 安装的 PalServer.exe 目录，返回命中的 server_path 列表
    detect: () => tauriInvoke<string[]>('detect_palserver_path'),
  },

  // === rest_proxy.rs (8 个) ===
  // REST API 代理：AdminPassword 在 Rust 侧从 ini 读取，前端只传 server_path
  rest: {
    getInfo: (serverPath: string) =>
      tauriInvoke<ServerInfo>('rest_get_info', { serverPath }),
    getMetrics: (serverPath: string) =>
      tauriInvoke<ServerMetrics>('rest_get_metrics', { serverPath }),
    getPlayers: (serverPath: string) =>
      tauriInvoke<PlayerInfo[]>('rest_get_players', { serverPath }),
    kick: (serverPath: string, userid: string) =>
      tauriInvoke<void>('rest_kick_player', { serverPath, userid }),
    ban: (serverPath: string, userid: string) =>
      tauriInvoke<void>('rest_ban_player', { serverPath, userid }),
    unban: (serverPath: string, userid: string) =>
      tauriInvoke<void>('rest_unban_player', { serverPath, userid }),
    announce: (serverPath: string, message: string) =>
      tauriInvoke<void>('rest_announce', { serverPath, message }),
    shutdown: (serverPath: string, waittime: number, message: string) =>
      tauriInvoke<void>('rest_shutdown', { serverPath, waittime, message }),
  },
}

export { tauriInvoke }
