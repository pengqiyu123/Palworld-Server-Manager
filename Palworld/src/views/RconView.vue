<template>
  <section class="screen active rcon-screen">
    <div class="page-head">
      <div>
        <div class="page-title">服务器控制台</div>
        <div class="page-sub">通过本机 REST 管理接口查看状态、保存世界、发送公告或安排关服。</div>
      </div>
      <div class="page-actions">
        <span class="status-pill" :class="rconStore.connected ? 'ok' : 'block'">
          <span class="s-dot" />
          {{ rconStore.connected ? '已连接' : '未连接' }}
        </span>
        <button
          v-if="!rconStore.connected"
          class="btn btn-primary"
          :disabled="rconStore.connecting"
          @click="onConnect"
        >
          {{ rconStore.connecting ? '连接中…' : '连接管理接口' }}
        </button>
        <button v-else class="btn btn-danger-ghost" @click="onDisconnect">断开</button>
      </div>
    </div>

    <div v-if="!serverPath" class="rcon-hint">
      <AppIcon name="info" :size="16" />
      <span>尚未配置服务器路径，请先在「概览」页自动探测或手动指定 PalServer 目录。</span>
    </div>

    <div class="rcon-workbench">
      <section class="rcon-shortcuts" aria-label="服务器快捷操作">
        <div class="shortcut-heading">
          <div>
            <h2>快捷操作</h2>
            <p>点击后填入下方命令框，再由你确认发送</p>
          </div>
          <div class="shortcut-heading-actions">
            <span v-if="commandPending" class="shortcut-progress" role="status">发送中</span>
            <button
              class="shortcut-collapse-toggle"
              type="button"
              :aria-expanded="!shortcutsCollapsed"
              @click="shortcutsCollapsed = !shortcutsCollapsed"
            >
              <AppIcon class="shortcut-chevron" :class="{ collapsed: shortcutsCollapsed }" name="chevron" :size="16" />
              {{ shortcutsCollapsed ? '展开快捷操作' : '收起快捷操作' }}
            </button>
          </div>
        </div>
        <Transition name="shortcut-collapse">
          <div v-if="!shortcutsCollapsed" class="shortcut-list">
            <button class="shortcut-card" type="button" @click="prefill('Info')">
              <strong>查看服务器信息</strong>
              <span>填入 Info，读取名称、版本和状态</span>
            </button>
            <button class="shortcut-card" type="button" @click="prefill('ShowPlayers')">
              <strong>查看在线玩家</strong>
              <span>填入 ShowPlayers，查看当前连接</span>
            </button>
            <button class="shortcut-card" type="button" @click="prefill('Save')">
              <strong>保存世界</strong>
              <span>填入 Save，请求服务器保存进度</span>
            </button>
            <button class="shortcut-card" type="button" @click="prefill('Broadcast ')">
              <strong>发送公告</strong>
              <span>填入公告命令后补充消息内容</span>
            </button>
            <button class="shortcut-card shortcut-card-danger" type="button" @click="shutdownConfirmOpen = true">
              <strong>安排关服</strong>
              <span>先确认，再填入倒计时关服命令</span>
            </button>
          </div>
        </Transition>
        <p v-if="shortcutsCollapsed" class="shortcuts-collapsed-copy">快捷操作已收起</p>
      </section>

      <div class="rcon-console">
        <div class="terminal">
          <div class="t-bar">
            <span>REST 管理 · {{ managementEndpoint }}</span>
            <span class="t-spacer" />
            <span class="command-state" aria-live="polite">{{ commandStatus }}</span>
            <button class="t-clear" type="button" @click="rconStore.clearOutput()">清空</button>
          </div>
          <div ref="termBody" class="t-body" aria-live="polite">
            <div v-for="(line, i) in rconStore.lines" :key="i" :class="'log-' + line.kind">
              {{ line.text }}
            </div>
            <div v-if="rconStore.lines.length === 0" class="log-sys">
              暂无操作记录。可从上方快捷操作填入命令，或在下方输入受支持的管理命令。
            </div>
          </div>
        </div>

        <div class="raw-command">
          <div class="raw-command-copy">
            <label for="rcon-command">管理命令</label>
            <span>↑↓ 翻历史</span>
          </div>
          <div class="cmd-row">
            <span class="cmd-prompt">&gt;</span>
            <div class="cmd-box">
              <input
                id="rcon-command"
                ref="inputEl"
                v-model="cmdInput"
                :disabled="!rconStore.connected || commandPending"
                type="text"
                spellcheck="false"
                autocomplete="off"
                placeholder="例如：Info、ShowPlayers、Save 或 Broadcast 公告内容"
                @keydown.enter="onSend"
                @keydown.up.prevent="onHistoryUp"
                @keydown.down.prevent="onHistoryDown"
              />
            </div>
            <button
              class="btn btn-primary"
              :disabled="!rconStore.connected || commandPending || !cmdInput.trim()"
              @click="onSend"
            >
              {{ commandPending ? '发送中…' : '发送' }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <div v-if="shutdownConfirmOpen" class="shutdown-dialog-backdrop" @click.self="shutdownConfirmOpen = false">
      <section class="shutdown-dialog" role="dialog" aria-modal="true" aria-labelledby="shutdown-dialog-title">
        <h2 id="shutdown-dialog-title">确认关服</h2>
        <p>这不会立即关闭服务器。确认后会在输入框填入 60 秒倒计时和公告，你可以修改内容或直接取消。</p>
        <div class="shutdown-dialog-actions">
          <button class="btn btn-secondary" type="button" @click="shutdownConfirmOpen = false">取消</button>
          <button class="btn btn-danger" type="button" @click="prepareShutdown">填入关服命令</button>
        </div>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { useRconStore } from '@/stores/rcon'
import { useSettingsStore } from '@/stores/settings'
import { useToast } from '@/components/ui/useToast'
import AppIcon from '@/components/ui/AppIcon.vue'

const rconStore = useRconStore()
const settingsStore = useSettingsStore()
const toast = useToast()

const cmdInput = ref('')
const inputEl = ref<HTMLInputElement | null>(null)
const termBody = ref<HTMLElement | null>(null)
const commandPending = ref(false)
const commandStatus = ref('连接后可发送快捷操作')
const shutdownConfirmOpen = ref(false)
const shortcutsCollapsed = ref(false)

const serverPath = computed(() => settingsStore.settings.server_path)
const managementEndpoint = computed(() => {
  const info = rconStore.connectionInfo
  return info ? `${info.host}:${info.port}` : '未连接'
})

let pollTimer: number | null = null
const POLL_INTERVAL_MS = 3000

async function onConnect(): Promise<void> {
  const path = settingsStore.settings.server_path
  if (!path) {
    toast.error('请先在设置中配置服务器路径')
    return
  }
  try {
    const info = await rconStore.connect(path)
    commandStatus.value = '已连接，可使用快捷操作'
    toast.success(`管理接口已连接（${info.host}:${info.port}）`)
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e)
    commandStatus.value = '连接失败，请查看控制台提示'
    toast.error(message)
  }
}

async function onDisconnect(): Promise<void> {
  await rconStore.disconnect()
  commandStatus.value = '已断开连接'
}

async function runCommand(command: string, pendingText: string): Promise<void> {
  if (!rconStore.connected || commandPending.value) return
  commandPending.value = true
  commandStatus.value = pendingText
  try {
    await rconStore.send(command)
    const lastLine = rconStore.lines[rconStore.lines.length - 1]
    commandStatus.value = lastLine?.kind === 'resp'
      ? '服务器已响应，结果已写入控制台'
      : '命令未完成，请查看控制台提示'
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    commandStatus.value = '命令发送失败，请查看控制台提示'
    toast.error(message)
  } finally {
    commandPending.value = false
  }
}

async function onSend(): Promise<void> {
  const command = cmdInput.value.trim()
  if (!command) return
  cmdInput.value = ''
  await runCommand(command, '正在执行管理命令…')
}

function prefill(prefix: string): void {
  cmdInput.value = prefix
  commandStatus.value = '命令已填入，请确认内容后发送'
  void nextTick(() => inputEl.value?.focus())
}

function prepareShutdown(): void {
  shutdownConfirmOpen.value = false
  prefill('Shutdown 60 ')
}

function onHistoryUp(): void {
  cmdInput.value = rconStore.navigateHistory('up')
}

function onHistoryDown(): void {
  cmdInput.value = rconStore.navigateHistory('down')
}

async function pollConnection(): Promise<void> {
  const connected = await rconStore.isConnected()
  rconStore.connected = connected
  if (!connected && commandPending.value) {
    commandPending.value = false
    commandStatus.value = '连接已断开，命令未完成'
  }
}

watch(
  () => rconStore.lines.length,
  () => void nextTick(() => {
    if (termBody.value) termBody.value.scrollTop = termBody.value.scrollHeight
  }),
)

onMounted(() => {
  pollTimer = window.setInterval(() => void pollConnection(), POLL_INTERVAL_MS)
  void pollConnection()
})

onUnmounted(() => {
  if (pollTimer !== null) clearInterval(pollTimer)
})
</script>

<style scoped>
.rcon-screen { min-width: 0; }
.rcon-hint { display: flex; align-items: center; gap: 8px; padding: 12px 16px; border-radius: 8px; background: var(--amber-bg, rgba(184, 120, 47, 0.14)); color: var(--amber, #b8782f); font-size: 13px; line-height: 1.5; }
.rcon-hint :deep(svg) { flex-shrink: 0; }
.rcon-workbench { display: flex; flex-direction: column; gap: 14px; min-height: 420px; }
.rcon-shortcuts, .rcon-console { min-width: 0; }
.rcon-shortcuts { padding: 14px; border: 1px solid var(--glass-border); border-radius: 8px; background: var(--glass-bg-soft); }
.shortcut-heading { display: flex; justify-content: space-between; align-items: flex-start; gap: 10px; margin-bottom: 12px; }
.shortcut-heading h2 { margin: 0; font-size: 14px; line-height: 20px; color: var(--text-hi); }
.shortcut-heading p { margin: 3px 0 0; color: var(--text-lo); font-size: 11px; line-height: 16px; }
.shortcut-heading-actions { display: flex; align-items: center; gap: 10px; }
.shortcut-progress { color: var(--primary); font-size: 11px; white-space: nowrap; }
.shortcut-collapse-toggle { display: inline-flex; align-items: center; gap: 4px; border: 0; background: transparent; color: var(--text-mid); cursor: pointer; font: inherit; font-size: 12px; white-space: nowrap; }
.shortcut-collapse-toggle:hover, .shortcut-collapse-toggle:focus-visible { color: var(--primary); outline: none; }
.shortcut-chevron { transition: transform .18s ease; }.shortcut-chevron.collapsed { transform: rotate(-90deg); }
.shortcut-list { display: flex; gap: 8px; overflow-x: auto; padding: 2px 2px 5px; scrollbar-width: thin; }
.shortcut-card { flex: 0 0 190px; min-height: 66px; padding: 10px; text-align: left; border: 1px solid var(--glass-border); border-radius: 7px; background: rgba(255, 250, 244, 0.52); color: var(--text-hi); cursor: pointer; font-family: var(--font-ui); transition: background .15s ease, border-color .15s ease, transform .15s ease; }
.shortcut-card:hover:not(:disabled), .shortcut-card:focus-visible { border-color: rgba(230, 111, 81, 0.6); background: rgba(255, 250, 244, 0.9); outline: none; transform: translateY(-1px); }
.shortcut-card:disabled { cursor: not-allowed; opacity: .5; }
.shortcut-card strong, .shortcut-card span { display: block; }
.shortcut-card strong { font-size: 13px; font-weight: 650; line-height: 18px; }
.shortcut-card span { margin-top: 2px; color: var(--text-mid2); font-size: 11px; line-height: 16px; }
.shortcut-card-danger { border-color: rgba(201, 85, 77, 0.35); background: var(--red-bg); }
.shortcut-card-danger strong { color: var(--red-soft); }
.shortcuts-collapsed-copy { margin: 0; color: var(--text-lo); font-size: 12px; }
.shortcut-collapse-enter-active, .shortcut-collapse-leave-active { overflow: hidden; transition: max-height .18s ease, opacity .14s ease, transform .18s ease; }.shortcut-collapse-enter-from, .shortcut-collapse-leave-to { max-height: 0; opacity: 0; transform: translateY(-5px); }.shortcut-collapse-enter-to, .shortcut-collapse-leave-from { max-height: 86px; opacity: 1; transform: translateY(0); }
.rcon-console { display: flex; flex-direction: column; gap: 10px; }
.terminal { flex: 1; min-height: 300px; }
.t-spacer { flex: 1; }
.command-state { max-width: 240px; overflow: hidden; color: var(--text-lo, #a39383); font-family: var(--font-ui); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.t-clear { background: transparent; border: 1px solid rgba(255, 255, 255, 0.18); color: var(--text-lo, #a39383); border-radius: 6px; padding: 3px 10px; font-size: 11px; cursor: pointer; font-family: var(--font-ui, inherit); }
.t-clear:hover { background: rgba(255, 255, 255, 0.08); color: #fff; }
.raw-command { padding: 10px 12px 12px; border: 1px solid var(--glass-border); border-radius: 8px; background: var(--glass-bg-soft); }
.raw-command-copy { display: flex; justify-content: space-between; gap: 12px; margin-bottom: 6px; color: var(--text-lo); font-size: 11px; }
.raw-command-copy label { color: var(--text-mid); font-weight: 600; }
.cmd-row { margin: 0; }
.shutdown-dialog-backdrop { position: fixed; z-index: 30; inset: 0; display: grid; place-items: center; padding: 24px; background: rgba(35, 25, 20, .52); }
.shutdown-dialog { width: min(420px, 100%); padding: 20px; border: 1px solid rgba(201, 85, 77, .55); border-radius: 8px; background: var(--surface, #fffaf4); box-shadow: 0 20px 48px rgba(35, 25, 20, .24); }
.shutdown-dialog h2 { margin: 0; color: var(--text-hi); font-size: 17px; }
.shutdown-dialog p { margin: 10px 0 18px; color: var(--text-mid2); font-size: 13px; line-height: 20px; }
.shutdown-dialog-actions { display: flex; justify-content: flex-end; gap: 8px; }

@media (max-width: 680px) { .shortcut-heading { align-items: center; }.shortcut-heading p, .shortcut-progress { display: none; }.shortcut-card { flex-basis: 172px; }.terminal { min-height: 260px; } }

@media (max-width: 680px) {
  .command-state { display: none; }
  .cmd-row { height: auto; align-items: stretch; flex-wrap: wrap; }
  .cmd-box { min-width: 180px; }
}
</style>
