import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  ServerStatus,
  FirewallStatus,
  ConfigValue,
  RadminLanStatus,
  RadminReadiness,
  ManagementConnectionInfo,
  AppSettings,
  PresetMeta,
  BackupInfo,
  FillConfigResult,
  ServerInfo,
  ServerMetrics,
  PlayerInfo,
  DiscoverResult,
  WorldInfo,
  WorldBackupInfo,
  WorldSummary,
  EditResult,
  PlayerTechnologyPoints,
  PlayerTechnologyPointsRequest,
  FixHostRequest,
  MigrateRequest,
  TransferRequest,
  UpdatePlayerTechnologyPointsRequest,
  ThreePhaseMigrationRequest,
  RollbackRequest,
  MigrationResult,
  BackupManifest,
  BackupIndex,
  BackupWorldClass,
  MigrationWorkflow,
  SaveOperationProgress,
  MigrateWorldV4Request,
  WorldMigrationV4Outcome,
  TransferFullCharacterV4Request,
  ImportFriendCharacterV4Request,
  TransferFullCharacterV4Outcome,
  WorkflowActionV4Request,
  RestoreOriginalGuildV4Outcome,
  ModifierWorldState,
  ModifierWorldEntry,
  ModifierActionRequest,
  ModifierActionPreview,
  ModifierActionResult,
  ModifierOperationProgress,
} from '@/types/tauri'

/**
 * 通用 invoke 封装：捕获 Tauri 后端错误并统一转换为 Error，
 * 同时为调用方提供泛型化的返回类型。
 *
 * Tauri 的 invoke 在出错时 reject 的值通常是字符串，
 * 也可能是 { message: string } 形式的对象；本函数将其归一为 Error。
 */
async function tauriInvoke<T>(
  cmd: string,
  args?: Record<string, unknown> | object,
): Promise<T> {
  try {
    return await invoke<T>(cmd, args as Record<string, unknown> | undefined)
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
  // === server.rs (8 个) ===
  server: {
    init: (path: string) => tauriInvoke<ServerStatus>('init_server_state', { path }),
    start: (path: string) => tauriInvoke<ServerStatus>('start_server', { path }),
    stop: () => tauriInvoke<ServerStatus>('stop_server'),
    forceStop: () => tauriInvoke<ServerStatus>('force_stop_server'),
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
    // 一键填充默认配置（仅手动按钮触发，不接入 start_server 自动守卫）
    fillDefault: (serverPath: string) =>
      tauriInvoke<FillConfigResult>('fill_default_config', { serverPath }),
    // 只读探测：live 配置是否已初始化（含 OptionSettings=( ），供空白横幅判断
    isInitialized: (serverPath: string) =>
      tauriInvoke<boolean>('is_config_initialized', { serverPath }),
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

  // 核心管理动作使用 Palworld 官方 REST API；密码只在 Rust 侧读取。
  management: {
    connect: (serverPath: string) =>
      tauriInvoke<ManagementConnectionInfo>('rest_management_connect', { serverPath }),
    execute: (serverPath: string, command: string) =>
      tauriInvoke<string>('rest_execute_management_command', { serverPath, command }),
  },

  // === launcher.rs (2 个 · 收官 F2/F3) ===
  launcher: {
    /** F2：启动 Radmin VPN 应用（仅拉起，加入网络由用户操作）。返回 "已启动 Radmin VPN" 或明确错误。 */
    launchRadminVpn: (preferredPath = '') =>
      tauriInvoke<string>('launch_radmin_vpn', { preferredPath: preferredPath || null }),
    validateRadminPath: (path: string) =>
      tauriInvoke<string>('validate_radmin_path', { path }),
    detectGame: () => tauriInvoke<string>('detect_palworld_game'),
    /** F3：启动游戏本体（优先 Steam 协议 steam://rungame/1623730，兜底直接拉起 Palworld.exe）。 */
    launchGame: () => tauriInvoke<string>('launch_game'),
  },

  // === save_transfer.rs (6 个 · 收官 F4 存档/角色转移 MVP) ===
  save: {
    /** 扫描 SaveGames 下含 Level.sav 的世界列表（自动发现存档根，回报路径）。
     *  `extraRoot` 为「手动选目录」兜底：作为额外扫描根合并去重（与单机一致）。 */
    discoverWorlds: (extraRoot?: string) =>
      tauriInvoke<DiscoverResult>('discover_worlds', extraRoot != null ? { extraRoot } : undefined),
    /** R5：扫描本机单机（Steam 库 + AppData）存档，返回 WorldInfo 列表，无档返回空数组。
     *  `extraRoot` 为「手动选目录」兜底：作为额外扫描根合并去重。 */
    discoverLocalWorlds: (extraRoot?: string) =>
      tauriInvoke<WorldInfo[]>('discover_local_worlds', extraRoot != null ? { extraRoot } : undefined),
    /** 整目录备份世界到备份夹（默认 <世界同级>/_backups/<world>/<timestamp>/；dest 为自定义存放目录）。 */
    backupWorld: (worldPath: string, dest?: string) =>
      tauriInvoke<string>('backup_world', { worldPath, dest }),
    /** 列出某世界的已有备份。 */
    listWorldBackups: (worldPath: string) =>
      tauriInvoke<WorldBackupInfo[]>('list_world_backups', { worldPath }),
    /** 把备份整体覆盖回当前世界目录（UI 侧需二次确认 + 提醒先停服）。 */
    restoreWorld: (worldPath: string, backupId: string) =>
      tauriInvoke<string>('restore_world', { worldPath, backupId }),
    /** 从用户指定的自定义备份目录整体覆盖回当前世界目录（指定文件夹存放场景）。 */
    restoreWorldFrom: (worldPath: string, src: string) =>
      tauriInvoke<string>('restore_world_from', { worldPath, src }),
    /** 导出单个角色存档到指定路径（保持 <steam_id>.sav）。 */
    exportCharacter: (worldName: string, steamId: string, destPath: string) =>
      tauriInvoke<string>('export_character', { worldName, steamId, destPath }),
    /** 导入角色存档（保持 steam_id 不变，覆盖同名文件）。 */
    importCharacter: (worldName: string, steamId: string, srcPath: string) =>
      tauriInvoke<string>('import_character', { worldName, steamId, srcPath }),
    createFullBackup: (
      sourceWorld: string,
      worldId: string,
      worldName: string,
      worldClass: BackupWorldClass,
      source: string,
    ) => tauriInvoke<BackupManifest>('backup_create_full', {
      sourceWorld,
      worldId,
      worldName,
      worldClass,
      source,
    }),
    getBackupRoot: () => tauriInvoke<string>('backup_get_root'),
    listFullBackups: () => tauriInvoke<BackupManifest[]>('backup_list_full'),
    deleteFullBackup: (backupId: string) =>
      tauriInvoke<void>('backup_delete_full', { backupId }),
    restoreFullBackup: (backupId: string) =>
      tauriInvoke<void>('backup_restore_full', { backupId }),
    listSnapshots: (worldId?: string) =>
      tauriInvoke<BackupManifest[]>('backup_list_snapshots', { worldId }),
    restoreSnapshot: (snapshotId: string) =>
      tauriInvoke<void>('backup_restore_snapshot', { snapshotId }),
    deleteSnapshot: (snapshotId: string) =>
      tauriInvoke<void>('backup_delete_snapshot', { snapshotId }),
    loadWorkflow: (workflowId: string) =>
      tauriInvoke<MigrationWorkflow>('backup_load_workflow', { workflowId }),
    listWorkflows: () => tauriInvoke<MigrationWorkflow[]>('backup_list_workflows'),
    rebuildBackupIndex: () => tauriInvoke<BackupIndex>('backup_rebuild_index'),
  },

  // === save_edit.rs (F5 · 存档迁移/改写 MVP，独立命名空间，不触碰 F4) ===
  migration: {
    /** 解析世界玩家/公会列表（L1–L3）。 */
    worldSummary: (worldName: string) =>
      tauriInvoke<WorldSummary>('f5_world_summary', { worldName }),
    /** 按真实世界目录路径解析玩家/公会列表（本地单机 / 服务器通用）。 */
    worldSummaryByPath: (path: string) =>
      tauriInvoke<WorldSummary>('f5_world_summary_by_path', { path }),
    /** 读取所选角色的普通科技点和古代科技点；只读，不修改存档。 */
    playerTechnologyPoints: (req: PlayerTechnologyPointsRequest) =>
      tauriInvoke<PlayerTechnologyPoints>('get_player_technology_points', { req }),
    /** Fix Host Save：旧主机角色 ↔ 新角色 UID 互换（灵魂步骤）。 */
    fixHostSave: (req: FixHostRequest) => tauriInvoke<EditResult>('fix_host_save', req),
    /** 整包世界迁移（文件级拷贝 + WorldOption/DedicatedServerName 提示）。 */
    migrateWorld: (req: MigrateRequest) =>
      tauriInvoke<EditResult>('migrate_world_to_server', req),
    /** 跨服角色转移（按 L4 子集）。 */
    transferCharacter: (req: TransferRequest) =>
      tauriInvoke<EditResult>('transfer_character', req),
    /** 原子更新所选角色的普通科技点和古代科技点。 */
    updatePlayerTechnologyPoints: (req: UpdatePlayerTechnologyPointsRequest) =>
      tauriInvoke<EditResult>('update_player_technology_points', { req }),
    /** 三阶段迁移（A 整包拷贝 + B 角色替换 + C 公会绑定）。
     *  显式传入 old→new UID 映射；后端自动整份快照 SaveGames/0/，失败整份回滚，且需停服。
     *  对应 Rust: save_edit::migrate_world_v2 */
    migrateWorldV2: (req: ThreePhaseMigrationRequest) =>
      tauriInvoke<MigrationResult>('migrate_world_v2', req),
    /** 用整份快照回滚三阶段迁移（backup_id 来自 migrateWorldV2 返回）。
     *  对应 Rust: save_edit::rollback_migration_v2（返回 EditResult） */
    rollbackMigrationV2: (req: RollbackRequest) =>
      tauriInvoke<EditResult>('rollback_migration_v2', req),
    migrateWorldV4: (req: MigrateWorldV4Request) =>
      tauriInvoke<WorldMigrationV4Outcome>('migrate_world_v4', { req }),
    transferFullCharacterV4: (req: TransferFullCharacterV4Request) =>
      tauriInvoke<TransferFullCharacterV4Outcome>('transfer_full_character_v4', { req }),
    importFriendCharacterV4: (req: ImportFriendCharacterV4Request) =>
      tauriInvoke<TransferFullCharacterV4Outcome>('import_friend_character_v4', { req }),
    restoreOriginalGuildV4: (req: WorkflowActionV4Request) =>
      tauriInvoke<RestoreOriginalGuildV4Outcome>('restore_original_guild_v4', { req }),
    completeMigrationWorkflowV4: (req: WorkflowActionV4Request) =>
      tauriInvoke<MigrationWorkflow>('complete_migration_workflow_v4', { req }),
    rollbackMigrationWorkflowV4: (req: WorkflowActionV4Request) =>
      tauriInvoke<MigrationWorkflow>('rollback_migration_workflow_v4', { req }),
    onProgress: (handler: (progress: SaveOperationProgress) => void): Promise<UnlistenFn> =>
      listen<SaveOperationProgress>('save-operation-progress', (event) => handler(event.payload)),
  },

  modifier: {
    discoverWorlds: () => tauriInvoke<ModifierWorldEntry[]>('discover_modifier_worlds'),
    getWorld: (path: string) =>
      tauriInvoke<ModifierWorldState>('get_modifier_world', { path }),
    previewAction: (request: ModifierActionRequest) =>
      tauriInvoke<ModifierActionPreview>('preview_modifier_action', { request }),
    applyAction: (request: ModifierActionRequest) =>
      tauriInvoke<ModifierActionResult>('apply_modifier_action', { request }),
    onProgress: (handler: (progress: ModifierOperationProgress) => void): Promise<UnlistenFn> =>
      listen<ModifierOperationProgress>('modifier-operation-progress', (event) => handler(event.payload)),
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
    save: (serverPath: string) => tauriInvoke<void>('rest_save_world', { serverPath }),
    shutdown: (serverPath: string, waittime: number, message: string) =>
      tauriInvoke<void>('rest_shutdown', { serverPath, waittime, message }),
  },
}

export { tauriInvoke }
