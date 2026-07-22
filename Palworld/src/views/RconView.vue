<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">RCON 远程控制台</div>
        <div class="page-sub">
          通过 Valve Source RCON（端口 25575）直连服务器，发送管理员指令。密码由后端从配置文件读取，不进入前端 JS。
        </div>
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
          {{ rconStore.connecting ? '连接中…' : '连接 RCON' }}
        </button>
        <button v-else class="btn btn-danger-ghost" @click="onDisconnect">断开</button>
      </div>
    </div>

    <!-- 未配置服务器路径时的引导提示 -->
    <div v-if="!serverPath" class="rcon-hint">
      <AppIcon name="info" :size="16" />
      <span>尚未配置服务器路径，请先在「概览」页自动探测或手动指定 PalServer 目录。</span>
    </div>

    <!-- 常用命令 -->
    <div class="quick-row">
      <button class="qbtn" @click="quick('Info')">Info</button>
      <button class="qbtn" @click="quick('ShowPlayers')">ShowPlayers</button>
      <button class="qbtn" @click="quick('Save')">Save</button>
      <button class="qbtn" @click="prefill('Broadcast ')">Broadcast</button>
      <button class="qbtn danger" @click="quick('Shutdown')">Shutdown</button>
    </div>

    <!-- 终端输出 -->
    <div class="terminal">
      <div class="t-bar">
        <span>RCON 终端 · 127.0.0.1:25575</span>
        <span class="t-spacer" />
        <button class="t-clear" @click="rconStore.clearOutput()">清空</button>
      </div>
      <div class="t-body" ref="termBody">
        <div v-for="(line, i) in rconStore.lines" :key="i" :class="'log-' + line.kind">
          {{ line.text }}
        </div>
        <div v-if="rconStore.lines.length === 0" class="log-sys">
          （暂无输出，点「连接 RCON」接入 127.0.0.1:25575 后即可发送命令）
        </div>
      </div>
    </div>

    <!-- 命令输入 -->
    <div class="cmd-row">
      <span class="cmd-prompt">&gt;</span>
      <div class="cmd-box">
        <input
          ref="inputEl"
          v-model="cmdInput"
          :disabled="!rconStore.connected"
          type="text"
          spellcheck="false"
          autocomplete="off"
          placeholder="输入 RCON 命令，如 Info / ShowPlayers / Save / Shutdown，↑↓ 翻历史"
          @keydown.enter="onSend"
          @keydown.up.prevent="onHistoryUp"
          @keydown.down.prevent="onHistoryDown"
        />
      </div>
      <button
        class="btn btn-primary"
        :disabled="!rconStore.connected || !cmdInput.trim()"
        @click="onSend"
      >
        发送
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { useRconStore } from '@/stores/rcon'
import { useSettingsStore } from '@/stores/settings'
import { useToast } from '@/components/ui/useToast'
import AppIcon from '@/components/ui/AppIcon.vue'

/**
 * RCON 终端视图（收官 M3-A · ★D3）。
 * 改接新后端命令：
 *   - rcon_connect_using_config(server_path)  // 密码/端口后端从 ini 读
 *   - rcon_send_command(command)
 *   - rcon_is_connected()  // 轮询感知断连（审计项④）
 *   - rcon_disconnect()
 * 全部经由 rconStore 封装；连接失败错误用全局 toast（5 类前缀分类）展示。
 */
const rconStore = useRconStore()
const settingsStore = useSettingsStore()
const toast = useToast()

const cmdInput = ref('')
const inputEl = ref<HTMLInputElement | null>(null)
const termBody = ref<HTMLElement | null>(null)

const serverPath = computed(() => settingsStore.settings.server_path)

// 连接状态轮询定时器（断连可感知）
let pollTimer: number | null = null
const POLL_INTERVAL_MS = 3000

async function onConnect(): Promise<void> {
  const path = settingsStore.settings.server_path
  if (!path) {
    toast.error('请先在设置中配置服务器路径')
    return
  }
  try {
    await rconStore.connect(path)
    toast.success('RCON 已连接（127.0.0.1:25575）')
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e)
    // 全局错误分类 toast（5 类前缀匹配：认证失败/未放行/未连接/不可达/其他）
    toast.error(message)
  }
}

async function onDisconnect(): Promise<void> {
  await rconStore.disconnect()
}

async function onSend(): Promise<void> {
  const cmd = cmdInput.value.trim()
  if (!cmd) return
  cmdInput.value = ''
  await rconStore.send(cmd)
}

/** 常用命令：直接发送 */
function quick(command: string): void {
  cmdInput.value = ''
  void rconStore.send(command)
}

/** 需要附参的命令：预填输入框并聚焦 */
function prefill(prefix: string): void {
  cmdInput.value = prefix
  void nextTick(() => inputEl.value?.focus())
}

function onHistoryUp(): void {
  cmdInput.value = rconStore.navigateHistory('up')
}

function onHistoryDown(): void {
  cmdInput.value = rconStore.navigateHistory('down')
}

/** 轮询后端连接状态，保持前端 connected 与后端一致（断连可感知） */
async function pollConnection(): Promise<void> {
  const ok = await rconStore.isConnected()
  rconStore.connected = ok
}

onMounted(() => {
  pollTimer = window.setInterval(() => void pollConnection(), POLL_INTERVAL_MS)
  void pollConnection()
})

onUnmounted(() => {
  if (pollTimer !== null) {
    clearInterval(pollTimer)
    pollTimer = null
  }
})
</script>

<style scoped>
.rcon-hint {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-radius: 12px;
  background: var(--amber-bg, rgba(184, 120, 47, 0.14));
  color: var(--amber, #b8782f);
  font-size: 13px;
  line-height: 1.5;
}
.rcon-hint :deep(svg) {
  flex-shrink: 0;
}
.t-spacer {
  flex: 1;
}
.t-clear {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.18);
  color: var(--text-lo, #a39383);
  border-radius: 8px;
  padding: 3px 10px;
  font-size: 11px;
  cursor: pointer;
  font-family: var(--font-ui, inherit);
}
.t-clear:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}
.t-count {
  color: var(--text-lo, #a39383);
}
</style>
