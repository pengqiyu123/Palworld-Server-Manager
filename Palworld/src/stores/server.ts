import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/api/tauri'
import type { ServerStatus, ServerInfo, ServerMetrics, PlayerInfo } from '@/types/tauri'
import { listen } from '@tauri-apps/api/event'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'

/**
 * 服务器状态 Store。
 *
 * 管理三层状态：
 * 1. 进程状态（status）— 来自 Rust server.rs 的 init/start/stop/getStatus
 * 2. REST 数据（serverInfo / serverMetrics / players）— 来自 60s 轮询 rest_proxy
 * 3. 日志（logs）— 来自 server-log 事件流
 *
 * 60s 轮询引擎：startPolling → 立即 pollOnce → setInterval(60s)。
 * 轮询失败自动 stopPolling + refreshStatus；server-status-change 事件也自动 stopPolling。
 */
export const useServerStore = defineStore('server', () => {
  const status = ref<ServerStatus>({
    running: false,
    pid: null,
    server_path: '',
    log_count: 0,
  })
  const logs = ref<string[]>([])
  // 日志来源标记（★D4）："cmd" = Cmd 版日志可用；"wrapper" = 包装器模式日志不可用（仅提示横条）
  const logSource = ref<string | null>(null)
  const loading = ref(false)

  // REST 数据（60s 轮询刷新）
  const serverInfo = ref<ServerInfo | null>(null)
  const serverMetrics = ref<ServerMetrics | null>(null)
  const players = ref<PlayerInfo[]>([])

  // 全局事件监听取消函数（log + status-change + log-source）
  let unlistenLog: (() => void) | null = null
  let unlistenStatus: (() => void) | null = null
  let unlistenLogSource: (() => void) | null = null

  // 轮询定时器 ID（null = 未轮询）
  let pollTimer: number | null = null

  /** 从 settingsStore 获取 server_path（REST 调用需要） */
  function getServerPath(): string {
    const settingsStore = useSettingsStore()
    return settingsStore.settings.server_path
  }

  async function init() {
    try {
      loading.value = true
      status.value = await api.server.init()
      logs.value = await api.server.getLogs()
    } finally {
      loading.value = false
    }
  }

  async function start(path: string) {
    loading.value = true
    try {
      status.value = await api.server.start(path)
    } finally {
      loading.value = false
    }
  }

  async function stop() {
    loading.value = true
    try {
      status.value = await api.server.stop()
      stopPolling()
    } finally {
      loading.value = false
    }
  }

  async function refreshStatus() {
    try {
      status.value = await api.server.getStatus()
    } catch (e) {
      console.error('刷新状态失败:', e)
    }
  }

  // ==================== 60s 轮询引擎 ====================

  /** 启动轮询：防重复 + 立即执行一次 + setInterval(60s) */
  function startPolling(): void {
    if (pollTimer !== null) return // 防重复
    void pollOnce() // 立即执行第一次（不等 60s）
    pollTimer = window.setInterval(() => void pollOnce(), 60_000)
  }

  /** 停止轮询：clearInterval + 置 null */
  function stopPolling(): void {
    if (pollTimer !== null) {
      clearInterval(pollTimer)
      pollTimer = null
    }
  }

  /**
   * 单次轮询（Q1 遗留清理）：Promise.allSettled 并行请求 info/metrics/players。
   * - 任一项 fulfilled → 更新对应 state；
   * - 任一项 rejected → 仅清空对应 state（避免陈旧数据），不中断整体轮询；
   * - 仅当三项「全部 rejected」时才停止轮询 + 刷新进程状态。
   */
  async function pollOnce(): Promise<void> {
    const serverPath = getServerPath()
    if (!serverPath) {
      console.error('轮询失败: 服务器路径为空')
      stopPolling()
      return
    }
    const results = await Promise.allSettled([
      api.rest.getInfo(serverPath),
      api.rest.getMetrics(serverPath),
      api.rest.getPlayers(serverPath),
    ])
    const [infoR, metricsR, playersR] = results

    if (infoR.status === 'fulfilled') serverInfo.value = infoR.value
    else serverInfo.value = null

    if (metricsR.status === 'fulfilled') serverMetrics.value = metricsR.value
    else serverMetrics.value = null

    if (playersR.status === 'fulfilled') players.value = playersR.value
    else players.value = []

    const allRejected = results.every((r) => r.status === 'rejected')
    if (allRejected) {
      console.error('轮询失败：REST 三项全部不可达，停止轮询')
      stopPolling()
      await refreshStatus()
    }
  }

  // ==================== 优雅关服 / 强制停止 ====================

  /**
   * 优雅关服（两段式）：
   * 1. 首选 REST /shutdown（带倒计时）→ 服务器在 waittime 秒后自行退出
   * 2. REST 不可用时兜底 api.server.stop()（force kill）
   *
   * 进程退出由 server-status-change 事件检测 → 自动 stopPolling。
   */
  async function gracefulShutdown(waittime: number, message: string): Promise<void> {
    const serverPath = getServerPath()
    if (!serverPath) {
      throw new Error('服务器路径为空，请先在设置中配置')
    }
    try {
      await api.rest.shutdown(serverPath, waittime, message)
      // REST /shutdown 是异步的——服务器在 waittime 秒后自行退出
      // server-status-change 事件到达后自动 stopPolling
    } catch (e) {
      // REST 不可用 → 兜底 force kill
      console.warn('REST 关服失败，尝试强制停止:', e)
      await api.server.stop()
      stopPolling()
    }
  }

  /** 强制停止（force kill 兜底） */
  async function forceStop(): Promise<void> {
    await api.server.stop()
    stopPolling()
  }

  // ==================== 玩家管理动作（供 PlayersView 调用） ====================

  async function kickPlayer(userid: string): Promise<void> {
    const serverPath = getServerPath()
    if (!serverPath) throw new Error('服务器路径为空')
    await api.rest.kick(serverPath, userid)
    // 下次轮询自动刷新玩家列表
  }

  async function banPlayer(userid: string): Promise<void> {
    const serverPath = getServerPath()
    if (!serverPath) throw new Error('服务器路径为空')
    await api.rest.ban(serverPath, userid)
  }

  async function announcePlayer(message: string): Promise<void> {
    const serverPath = getServerPath()
    if (!serverPath) throw new Error('服务器路径为空')
    await api.rest.announce(serverPath, message)
  }

  // ==================== 事件监听 ====================

  /** 订阅 server-log 事件，全局追加日志（在 main.ts 中调用一次） */
  async function setupLogListener() {
    if (unlistenLog) return
    unlistenLog = await listen<string>('server-log', (event) => {
      logs.value.push(event.payload)
      if (logs.value.length > 500) {
        logs.value.shift()
      }
    })
  }

  /**
   * 订阅 server-status-change 事件。
   * 进程退出时后端 emit 最新 ServerStatus → 更新 status + 自动 stopPolling。
   */
  async function setupStatusChangeListener() {
    if (unlistenStatus) return
    unlistenStatus = await listen<ServerStatus>('server-status-change', (event) => {
      status.value = event.payload
      // 进程退出 → 自动停止轮询 + 切回向导模式
      if (!event.payload.running) {
        stopPolling()
        // 清空 REST 数据（避免仪表盘显示过期数据）
        serverInfo.value = null
        serverMetrics.value = null
        players.value = []
        // 切回向导模式
        const uiStore = useUiStore()
        uiStore.setMode('wizard')
      }
    })
  }

  async function clearLogs() {
    await api.server.clearLogs()
    logs.value = []
  }

  /**
   * 订阅 server-log-source 事件（★D4）：记录当前日志来源（cmd / wrapper）。
   * 供 LogsView 顶部提示条使用（wrapper 模式提示"日志不可用"）。
   */
  async function setupLogSourceListener() {
    if (unlistenLogSource) return
    unlistenLogSource = await listen<string>('server-log-source', (event) => {
      logSource.value = event.payload
    })
  }

  function destroyListener() {
    if (unlistenLog) {
      unlistenLog()
      unlistenLog = null
    }
    if (unlistenStatus) {
      unlistenStatus()
      unlistenStatus = null
    }
    if (unlistenLogSource) {
      unlistenLogSource()
      unlistenLogSource = null
    }
    stopPolling()
  }

  return {
    status,
    logs,
    logSource,
    loading,
    serverInfo,
    serverMetrics,
    players,
    init,
    start,
    stop,
    refreshStatus,
    startPolling,
    stopPolling,
    pollOnce,
    gracefulShutdown,
    forceStop,
    kickPlayer,
    banPlayer,
    announcePlayer,
    setupLogListener,
    setupStatusChangeListener,
    setupLogSourceListener,
    clearLogs,
    destroyListener,
  }
})
