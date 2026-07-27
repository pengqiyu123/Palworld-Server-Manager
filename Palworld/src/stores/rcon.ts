import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/api/tauri'
import type { ManagementConnectionInfo } from '@/types/tauri'

export interface RconLogLine {
  kind: 'info' | 'cmd' | 'resp' | 'err' | 'sys'
  text: string
}

const COMMAND_HISTORY_KEY = 'management-command-history'
const COMMAND_HISTORY_MAX = 50

const MANAGEMENT_SEED: RconLogLine[] = [
  { kind: 'info', text: '> [演示] 服务器管理接口已就绪' },
  { kind: 'cmd', text: '> [演示] Save' },
  { kind: 'resp', text: '< [演示] 世界保存请求已完成' },
]

function loadCommandHistory(): string[] {
  try {
    const raw = localStorage.getItem(COMMAND_HISTORY_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed)
      ? parsed.filter((value): value is string => typeof value === 'string').slice(-COMMAND_HISTORY_MAX)
      : []
  } catch {
    return []
  }
}

function saveCommandHistory(history: string[]): void {
  try {
    localStorage.setItem(COMMAND_HISTORY_KEY, JSON.stringify(history.slice(-COMMAND_HISTORY_MAX)))
  } catch {
    // 命令历史不是关键数据，浏览器拒绝本地存储时保持内存状态即可。
  }
}

export const useRconStore = defineStore('management-console', () => {
  const connected = ref(false)
  const connecting = ref(false)
  const connectionInfo = ref<ManagementConnectionInfo | null>(null)
  const lines = ref<RconLogLine[]>([])
  const commandHistory = ref<string[]>(loadCommandHistory())
  const historyCursor = ref(-1)
  const activeServerPath = ref('')

  function appendLine(line: RconLogLine): void {
    lines.value.push(line)
  }

  function seedMock(): void {
    lines.value = [...MANAGEMENT_SEED]
  }

  function log(text: string, kind: RconLogLine['kind'] = 'sys'): void {
    appendLine({ kind, text })
  }

  async function connect(serverPath: string): Promise<ManagementConnectionInfo> {
    if (!serverPath.trim()) {
      appendLine({ kind: 'err', text: '< 服务器路径为空，请先在概览中完成探测' })
      throw new Error('服务器路径为空')
    }

    connecting.value = true
    try {
      const info = await api.management.connect(serverPath)
      activeServerPath.value = serverPath
      connected.value = true
      connectionInfo.value = info
      appendLine({ kind: 'info', text: `> ${info.message}（${info.host}:${info.port}）` })
      return info
    } catch (error) {
      connected.value = false
      connectionInfo.value = null
      const message = error instanceof Error ? error.message : String(error)
      appendLine({ kind: 'err', text: `< ${message}` })
      throw error
    } finally {
      connecting.value = false
    }
  }

  async function disconnect(): Promise<void> {
    connected.value = false
    connectionInfo.value = null
    activeServerPath.value = ''
    appendLine({ kind: 'sys', text: '< 已断开本机管理接口' })
  }

  async function isConnected(): Promise<boolean> {
    if (!activeServerPath.value) return false
    try {
      connectionInfo.value = await api.management.connect(activeServerPath.value)
      connected.value = true
      return true
    } catch {
      connected.value = false
      connectionInfo.value = null
      return false
    }
  }

  function pushHistory(command: string): void {
    const trimmed = command.trim()
    if (!trimmed || commandHistory.value[commandHistory.value.length - 1] === trimmed) return
    commandHistory.value.push(trimmed)
    while (commandHistory.value.length > COMMAND_HISTORY_MAX) commandHistory.value.shift()
    saveCommandHistory(commandHistory.value)
  }

  async function send(command: string): Promise<void> {
    const raw = command.trim()
    if (!raw) return
    if (!connected.value || !activeServerPath.value) {
      appendLine({ kind: 'err', text: '< 尚未连接，请先点“连接管理接口”' })
      return
    }

    appendLine({ kind: 'cmd', text: `> ${raw}` })
    try {
      const response = await api.management.execute(activeServerPath.value, raw)
      appendLine({ kind: 'resp', text: `< ${response}` })
      pushHistory(raw)
      historyCursor.value = commandHistory.value.length
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      appendLine({ kind: 'err', text: `< ${message}` })
    }
  }

  function navigateHistory(direction: 'up' | 'down'): string {
    if (commandHistory.value.length === 0) return ''
    historyCursor.value = direction === 'up'
      ? Math.max(0, historyCursor.value - 1)
      : Math.min(commandHistory.value.length, historyCursor.value + 1)
    return commandHistory.value[historyCursor.value] ?? ''
  }

  function clearOutput(): void {
    lines.value = []
  }

  return {
    connected,
    connecting,
    connectionInfo,
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
