<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">实时日志</div>
        <div class="page-sub">
          服务器控制台实时流。朋友连入、世界保存、报错都会在下方留下日志行。
        </div>
      </div>
      <div class="page-actions">
        <button class="btn btn-sm btn-ghost" @click="togglePause">
          {{ paused ? '继续' : '暂停' }}
        </button>
        <button class="btn btn-sm btn-ghost" @click="clearLogs">清空</button>
        <label class="autoscroll-toggle">
          <input type="checkbox" v-model="autoScroll" />
          <span>自动滚动</span>
        </label>
      </div>
    </div>

    <!-- 日志来源提示条（★D4）：wrapper 模式日志不可用 -->
    <div v-if="source === 'wrapper'" class="src-banner warn">
      <AppIcon name="info" :size="16" />
      <span>
        日志不可用：当前使用老版本启动器（wrapper 模式），实时日志已关闭。请更新为专用服
        （PalServer-Win64-Shipping-Cmd.exe）以启用实时日志。
      </span>
    </div>
    <div v-else-if="source === 'cmd'" class="src-banner ok">
      <AppIcon name="info" :size="16" />
      <span>Cmd 模式：日志可用（PalServer-Win64-Shipping-Cmd.exe 标准输出已接入）。</span>
    </div>
    <div v-else-if="source === 'console'" class="src-banner ok">
      <AppIcon name="info" :size="16" />
      <span>服务器控制台已接入；黑色日志窗口已隐藏。</span>
    </div>

    <!-- 日志面板 -->
    <div class="terminal logs-terminal">
      <div class="t-bar">
        <span>server-log · 实时</span>
        <span class="t-spacer" />
        <span class="t-count">{{ logs.length }} 行</span>
      </div>
      <div class="t-body logs-body" ref="logBody">
        <div v-for="(line, i) in logs" :key="i" class="log-line" :class="lineClass(line)">
          {{ line }}
        </div>
        <div v-if="logs.length === 0" class="log-sys">
          （暂无日志，启动服务器后将在此实时显示）
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { api } from '@/api/tauri'
import AppIcon from '@/components/ui/AppIcon.vue'

/**
 * 实时日志面板（收官 M3-B · ★D4）。
 * - 挂载时先调 get_server_logs 拉一次全量（最多 500 行），
 *   之后走 listen('server-log') 流式 append，前端 500 行环形缓冲。
 * - listen('server-log-source') 拿到 "console" | "wrapper"：
 *   wrapper 模式顶部显示「日志不可用」横条。
 * - 提供 暂停 / 清空 / 自动滚动 等基础交互。
 */
const logs = ref<string[]>([])
const source = ref<string | null>(null)
const paused = ref(false)
const autoScroll = ref(true)
const logBody = ref<HTMLElement | null>(null)

const MAX_LINES = 500

let unlistenLog: UnlistenFn | null = null
let unlistenSource: UnlistenFn | null = null

function appendLog(payload: string): void {
  if (paused.value) return
  logs.value.push(payload)
  if (logs.value.length > MAX_LINES) {
    logs.value.splice(0, logs.value.length - MAX_LINES)
  }
}

/** 根据关键字给每行上色（与 .terminal .log-* 全局类对齐） */
function lineClass(line: string): string {
  const l = line.toLowerCase()
  if (l.includes('error') || l.includes('[error]') || l.includes('失败') || l.includes('panic')) {
    return 'log-err'
  }
  if (l.includes('warn') || l.includes('[warn]') || l.includes('警告')) {
    return 'log-warn'
  }
  if (l.includes('[log]') || l.includes('connected') || l.includes('joined') || l.includes('saved')) {
    return 'log-info'
  }
  return 'log-sys'
}

function clearLogs(): void {
  logs.value = []
}

function togglePause(): void {
  paused.value = !paused.value
}

// 自动滚底：日志增长时滚动到最底部
watch(
  () => logs.value.length,
  async () => {
    if (autoScroll.value && logBody.value) {
      await nextTick()
      logBody.value.scrollTop = logBody.value.scrollHeight
    }
  }
)

onMounted(async () => {
  // 先拉一次全量（最多 500 行）
  try {
    const initial = await api.server.getLogs()
    if (Array.isArray(initial)) {
      logs.value = initial.slice(-MAX_LINES)
    }
  } catch {
    logs.value = []
  }
  // 订阅实时流
  unlistenLog = await listen<string>('server-log', (event) => appendLog(event.payload))
  unlistenSource = await listen<string>('server-log-source', (event) => {
    source.value = event.payload
  })
})

onUnmounted(() => {
  unlistenLog?.()
  unlistenSource?.()
})
</script>

<style scoped>
.btn-sm {
  padding: 5px 14px;
  font-size: 12px;
  height: 30px;
}
.autoscroll-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-mid2, #a39383);
  cursor: pointer;
  user-select: none;
}
.autoscroll-toggle input {
  accent-color: var(--palwarm-primary, #e66f51);
}

/* 日志来源横条 */
.src-banner {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 12px 16px;
  border-radius: 12px;
  font-size: 13px;
  line-height: 1.55;
}
.src-banner :deep(svg) {
  flex-shrink: 0;
  margin-top: 2px;
}
.src-banner.ok {
  background: var(--green-bg, rgba(79, 138, 107, 0.14));
  color: var(--green, #4f8a6b);
}
.src-banner.warn {
  background: var(--red-bg, rgba(201, 85, 77, 0.12));
  color: var(--red-soft, #b8463f);
}

.logs-terminal {
  flex: 1;
  min-height: 240px;
}
.logs-body {
  color: var(--text-mid2, #a39383);
}
/* log-warn 未被全局 .terminal .log-* 覆盖，此处补色（勿写死十六进制） */
.logs-body .log-warn {
  color: var(--amber, #b8782f);
}
.log-line {
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
