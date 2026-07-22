import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/api/tauri'

/** RCON 日志行（本地 state 形状） */
export interface RconLogLine {
  kind: 'info' | 'cmd' | 'resp' | 'err' | 'sys'
  text: string
}

// localStorage 持久化 key
const COMMAND_HISTORY_KEY = 'rcon-command-history'
// 历史命令最大条数
const COMMAND_HISTORY_MAX = 50

// 初始日志种子（MOCK 模式视觉用）
const RCON_SEED: RconLogLine[] = [
  { kind: 'info', text: '> [INFO] RCON 终端已就绪（点「连接」接入 127.0.0.1:25575）' },
  { kind: 'cmd', text: '> [CMD]  Broadcast "服务器将在 5 分钟后维护重启，请提前保存"' },
  { kind: 'resp', text: '< Response: Broadcast sent to 4 players' },
  { kind: 'cmd', text: '> [CMD]  SaveWorld' },
  { kind: 'resp', text: '< Response: World saved successfully' },
]

function loadCommandHistory(): string[] {
  try {
    const raw = localStorage.getItem(COMMAND_HISTORY_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (Array.isArray(parsed)) {
      return parsed.filter((s): s is string => typeof s === 'string').slice(0, COMMAND_HISTORY_MAX)
    }
    return []
  } catch {
    return []
  }
}

function saveCommandHistory(history: string[]) {
  try {
    localStorage.setItem(COMMAND_HISTORY_KEY, JSON.stringify(history.slice(-COMMAND_HISTORY_MAX)))
  } catch {
    // localStorage 写入失败（如隐私模式）忽略
  }
}

/**
 * RCON store（收官：真实接线 rcon.rs 后端，★D3）。
 * connect / disconnect / send 均调用真实 api.rcon.*（密码/端口从 ini 读取，不进前端 JS）。
 * 命令历史持久化到 localStorage；终端输出（lines）仅内存。
 */
export const useRconStore = defineStore('rcon', () => {
  const connected = ref(false)
  const connecting = ref(false)
  const lines = ref<RconLogLine[]>([])
  const commandHistory = ref<string[]>(loadCommandHistory())
  const historyCursor = ref(-1)

  function appendLine(line: RconLogLine): void {
    lines.value.push(line)
  }

  /** 注入样例数据（main.ts MOCK 模式调用） */
  function seedMock(): void {
    lines.value = [...RCON_SEED]
  }

  function log(text: string, kind: RconLogLine['kind'] = 'sys'): void {
    appendLine({ kind, text })
  }

  /** 连接：调用后端 rcon_connect_using_config（host 固定 127.0.0.1，密码/端口从 ini 读） */
  async function connect(serverPath: string): Promise<void> {
    if (!serverPath) {
      appendLine({ kind: 'err', text: '< Error: 服务器路径为空，请先在设置中配置' })
      throw new Error('服务器路径为空')
    }
    connecting.value = true
    try {
      const msg = await api.rcon.connectUsingConfig(serverPath)
      connected.value = true
      appendLine({ kind: 'info', text: `> [INFO] ${msg}` })
    } catch (e) {
      connected.value = false
      const message = e instanceof Error ? e.message : String(e)
      appendLine({ kind: 'err', text: `< Error: ${message}` })
      throw e
    } finally {
      connecting.value = false
    }
  }

  /** 断开：调用后端 rcon_disconnect */
  async function disconnect(): Promise<void> {
    try {
      await api.rcon.disconnect()
    } catch {
      // 忽略后端错误，前端直接置未连接
    }
    connected.value = false
    appendLine({ kind: 'sys', text: '< [INFO] RCON connection closed' })
  }

  async function isConnected(): Promise<boolean> {
    try {
      return await api.rcon.isConnected()
    } catch {
      return false
    }
  }

  function pushHistory(cmd: string): void {
    const trimmed = cmd.trim()
    if (!trimmed) return
    if (commandHistory.value[commandHistory.value.length - 1] === trimmed) return
    commandHistory.value.push(trimmed)
    while (commandHistory.value.length > COMMAND_HISTORY_MAX) {
      commandHistory.value.shift()
    }
    saveCommandHistory(commandHistory.value)
  }

  /** 发送命令：追加 cmd 行，调用后端 rcon_send_command，回显响应或错误 */
  async function send(command: string): Promise<void> {
    const raw = command.trim()
    if (!raw) return
    if (!connected.value) {
      appendLine({ kind: 'err', text: '< Error: 尚未连接，请先点「连接 RCON」' })
      return
    }
    appendLine({ kind: 'cmd', text: `> [CMD]  ${raw}` })
    try {
      const resp = await api.rcon.send(raw)
      appendLine({ kind: 'resp', text: `< Response: ${resp}` })
      pushHistory(raw)
      historyCursor.value = commandHistory.value.length
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e)
      appendLine({ kind: 'err', text: `< Error: ${message}` })
    }
  }

  function navigateHistory(direction: 'up' | 'down'): string {
    if (commandHistory.value.length === 0) return ''
    if (direction === 'up') {
      historyCursor.value = Math.max(0, historyCursor.value - 1)
    } else {
      historyCursor.value = Math.min(commandHistory.value.length, historyCursor.value + 1)
    }
    return historyCursor.value >= 0 && historyCursor.value < commandHistory.value.length
      ? commandHistory.value[historyCursor.value]
      : ''
  }

  function clearOutput(): void {
    lines.value = []
  }

  return {
    connected,
    connecting,
    lines,
    commandHistory,
    historyCursor,
    seedMock,
    connect,
    disconnect,
    isConnected,
    send,
    navigateHistory,
    pushHistory,
    log,
    clearOutput,
  }
})
