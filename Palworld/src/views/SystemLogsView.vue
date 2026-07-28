<template>
  <section class="screen active system-logs-view">
    <header class="system-logs-header">
      <div>
        <h1>系统日志</h1>
        <p>记录管理器的启动、操作和错误信息，可直接复制给开发者。</p>
      </div>
      <div class="system-log-actions">
        <button class="btn btn-ghost btn-sm" :disabled="loading" @click="loadLogs">
          <RefreshCw :size="15" :class="{ spinning: loading }" />
          {{ loading ? '刷新中…' : '刷新' }}
        </button>
        <button class="btn btn-ghost btn-sm" :disabled="errorLogs.length === 0" @click="copyErrors">
          <Copy :size="15" />
          复制错误信息
        </button>
        <button class="btn btn-ghost btn-sm" :disabled="logs.length === 0 || exporting" @click="exportLogs">
          <Download :size="15" />
          {{ exporting ? '导出中…' : '导出日志' }}
        </button>
        <button class="btn btn-danger-ghost btn-sm" :disabled="logs.length === 0 || clearing" @click="clearLogs">
          <Trash2 :size="15" />
          {{ clearing ? '清空中…' : '清空日志' }}
        </button>
      </div>
    </header>

    <div class="system-log-summary" aria-live="polite">
      <span>{{ logs.length }} 条记录</span>
      <span :class="{ hasErrors: errorLogs.length > 0 }">{{ errorLogs.length }} 条错误</span>
      <span>{{ lastUpdatedText }}</span>
    </div>

    <div v-if="loadError" class="system-log-load-error" role="alert">
      <AlertCircle :size="17" />
      <div>
        <strong>系统日志读取失败</strong>
        <span>{{ loadError }}</span>
      </div>
      <button class="btn btn-ghost btn-sm" @click="copyText(loadError)">复制错误</button>
    </div>

    <div v-if="!loading && logs.length === 0 && !loadError" class="system-log-empty">
      <FileText :size="26" />
      <strong>暂无系统日志</strong>
      <span>后续启动、操作或错误会自动记录在这里。</span>
    </div>

    <div v-else-if="logs.length" class="system-log-list" role="log" aria-label="系统日志内容">
      <div
        v-for="(line, index) in logs"
        :key="`${index}-${line}`"
        class="system-log-line"
        :class="logLevel(line)"
      >
        <span class="system-log-level">{{ levelLabel(line) }}</span>
        <code>{{ line }}</code>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { confirm as dialogConfirm, save as dialogSave } from '@tauri-apps/plugin-dialog'
import { AlertCircle, Copy, Download, FileText, RefreshCw, Trash2 } from '@lucide/vue'
import { api } from '@/api/tauri'
import { useToast } from '@/components/ui/useToast'

const toast = useToast()
const logs = ref<string[]>([])
const loading = ref(false)
const exporting = ref(false)
const clearing = ref(false)
const loadError = ref('')
const lastUpdatedAt = ref<Date | null>(null)

const errorLogs = computed(() => logs.value.filter((line) => line.includes('[ERROR]')))
const lastUpdatedText = computed(() => lastUpdatedAt.value
  ? `更新于 ${lastUpdatedAt.value.toLocaleTimeString('zh-CN', { hour12: false })}`
  : '尚未刷新')

function logLevel(line: string): 'error' | 'warn' | 'info' {
  if (line.includes('[ERROR]')) return 'error'
  if (line.includes('[WARN]')) return 'warn'
  return 'info'
}

function levelLabel(line: string): string {
  const level = logLevel(line)
  if (level === 'error') return '错误'
  if (level === 'warn') return '警告'
  return '信息'
}

async function loadLogs(): Promise<void> {
  if (loading.value) return
  loading.value = true
  loadError.value = ''
  try {
    logs.value = await api.appLog.getLogs()
    lastUpdatedAt.value = new Date()
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : String(error)
  } finally {
    loading.value = false
  }
}

async function copyText(text: string): Promise<void> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      const textarea = document.createElement('textarea')
      textarea.value = text
      textarea.style.position = 'fixed'
      textarea.style.opacity = '0'
      document.body.appendChild(textarea)
      textarea.select()
      const copied = document.execCommand('copy')
      textarea.remove()
      if (!copied) throw new Error('系统拒绝了复制操作')
    }
    toast.success('错误信息已复制')
  } catch (error) {
    toast.error(`复制失败: ${error instanceof Error ? error.message : String(error)}`)
  }
}

async function copyErrors(): Promise<void> {
  if (!errorLogs.value.length) return
  await copyText(errorLogs.value.join('\n'))
}

async function exportLogs(): Promise<void> {
  if (exporting.value || !logs.value.length) return
  exporting.value = true
  try {
    const stamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
    const path = await dialogSave({
      defaultPath: `palworld-manager-system-logs-${stamp}.txt`,
      filters: [{ name: '文本文件', extensions: ['txt'] }],
    })
    if (!path) return
    const count = await api.appLog.exportLogs(path)
    toast.success(`已导出 ${count} 条日志`)
  } catch (error) {
    toast.error(`导出日志失败: ${error instanceof Error ? error.message : String(error)}`)
  } finally {
    exporting.value = false
  }
}

async function clearLogs(): Promise<void> {
  if (clearing.value || !logs.value.length) return
  const confirmed = await dialogConfirm('清空后无法恢复，确定继续吗？', {
    title: '清空系统日志',
    kind: 'warning',
  })
  if (!confirmed) return
  clearing.value = true
  try {
    await api.appLog.clearLogs()
    logs.value = []
    lastUpdatedAt.value = new Date()
    toast.success('系统日志已清空')
  } catch (error) {
    toast.error(`清空日志失败: ${error instanceof Error ? error.message : String(error)}`)
  } finally {
    clearing.value = false
  }
}

onMounted(() => {
  void loadLogs()
})
</script>

<style scoped>
.system-logs-view { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
.system-logs-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 20px; }
.system-logs-header h1 { margin: 0; color: var(--text-hi); font-size: 22px; line-height: 30px; }
.system-logs-header p { margin: 3px 0 0; color: var(--text-lo); font-size: 12px; line-height: 18px; }
.system-log-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 8px; }
.system-log-actions .btn { display: inline-flex; align-items: center; gap: 6px; }
.system-log-summary { display: flex; flex-wrap: wrap; gap: 18px; padding: 9px 0; border-top: 1px solid var(--glass-border); border-bottom: 1px solid var(--glass-border); color: var(--text-lo); font-size: 11px; }
.system-log-summary .hasErrors { color: var(--red); font-weight: 700; }
.system-log-load-error { display: flex; align-items: center; gap: 10px; padding: 12px; border: 1px solid rgba(201, 85, 77, .35); border-radius: 7px; background: rgba(201, 85, 77, .08); color: var(--red); }
.system-log-load-error > div { min-width: 0; flex: 1; }
.system-log-load-error strong, .system-log-load-error span { display: block; }
.system-log-load-error span { margin-top: 2px; font-family: var(--font-mono); font-size: 11px; word-break: break-word; }
.system-log-empty { display: grid; place-items: center; align-content: center; flex: 1; min-height: 260px; color: var(--text-lo); text-align: center; }
.system-log-empty strong { margin-top: 8px; color: var(--text-mid); font-size: 14px; }
.system-log-empty span { margin-top: 3px; font-size: 11px; }
.system-log-list { min-height: 260px; overflow: auto; border: 1px solid var(--glass-border); border-radius: 7px; background: #211f1d; }
.system-log-line { display: grid; grid-template-columns: 46px minmax(0, 1fr); gap: 10px; padding: 7px 10px; border-bottom: 1px solid rgba(255, 255, 255, .06); }
.system-log-line:last-child { border-bottom: 0; }
.system-log-line.error { background: rgba(201, 85, 77, .14); }
.system-log-line.warn { background: rgba(184, 120, 47, .1); }
.system-log-level { color: #aaa19b; font-size: 10px; line-height: 18px; }
.system-log-line.error .system-log-level { color: #f09a93; }
.system-log-line.warn .system-log-level { color: #e1b06e; }
.system-log-line code { color: #e8e2dc; font-family: var(--font-mono); font-size: 11px; line-height: 18px; white-space: pre-wrap; word-break: break-word; }
.spinning { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 760px) {
  .system-logs-header { align-items: stretch; flex-direction: column; }
  .system-log-actions { justify-content: flex-start; }
}
@media (prefers-reduced-motion: reduce) { .spinning { animation: none; } }
</style>
