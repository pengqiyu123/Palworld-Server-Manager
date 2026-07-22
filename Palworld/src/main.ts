import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
// 本地打包字体（离线 / CSP 安全，规避 Google Fonts CDN 限制）
import '@fontsource/noto-sans-sc/400.css'
import '@fontsource/noto-sans-sc/500.css'
import '@fontsource/noto-sans-sc/600.css'
import '@fontsource/noto-sans-sc/700.css'
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/500.css'
import '@fontsource/jetbrains-mono/700.css'
import './style.css'
import { useToast } from '@/components/ui/useToast'

// Store 导入（真实接线轮：bootstrapStores 需要全部 store）
import { useSettingsStore } from '@/stores/settings'
import { useServerStore } from '@/stores/server'
import { useConfigStore } from '@/stores/config'
import { useNetworkStore } from '@/stores/network'
import { useUiStore } from '@/stores/ui'

const app = createApp(App)

app.config.errorHandler = (err, _instance, info) => {
  console.error('[Vue Error]', err, info)
  try {
    const toast = useToast()
    const message = err instanceof Error ? err.message : String(err)
    toast.error(`应用错误: ${message}`)
  } catch {
    // Pinia 未就绪时仅输出到控制台
  }
}

app.use(createPinia())
app.use(router)
app.mount('#app')

/**
 * VITE_MOCK 开关：
 * - 'true'（默认）：注入样例数据，不调用任何真实 Tauri 命令。
 * - 'false'：走 src/api/tauri.ts 的真实 invoke（成品模式）。
 */
const env = (import.meta as unknown as { env?: Record<string, string | undefined> }).env
const MOCK = (env?.VITE_MOCK ?? 'true') !== 'false'

/**
 * 全局初始化各 store。
 *
 * MOCK 模式：注入样例数据（视觉还原轮遗留）。
 * 真实模式（成品）：
 *   1. settingsStore.initDetectSettings() — 加载 settings.json + 探测 server_path
 *   2. serverStore.init() — 获取进程状态
 *   3. serverStore.setupLogListener() + setupStatusChangeListener() — 订阅 Rust 事件
 *   4. configStore.loadDescriptions() — 加载配置项元信息
 *   5. networkStore.checkAll() — 防火墙 + Radmin 并行检测
 *   6. 双模式判定：running → dashboard + startPolling / 否则 wizard
 */
async function bootstrapStores(): Promise<void> {
  if (MOCK) {
    // 模拟模式：保留视觉还原轮的 mock 注入逻辑（仅 rcon 种子）
    const { useRconStore } = await import('@/stores/rcon')
    const rconStore = useRconStore()
    rconStore.seedMock()
    return
  }

  // 真实模式
  const settingsStore = useSettingsStore()
  const serverStore = useServerStore()
  const configStore = useConfigStore()
  const networkStore = useNetworkStore()
  const uiStore = useUiStore()

  // 1. 加载设置 + 探测服务器路径
  try {
    await settingsStore.initDetectSettings()
  } catch (e) {
    console.warn('初始化设置失败:', e)
  }

  // 2. 初始化服务器进程状态
  try {
    await serverStore.init()
  } catch (e) {
    console.warn('初始化服务器状态失败:', e)
  }

  // 3. 订阅 Rust 事件（log + status-change）
  try {
    await serverStore.setupLogListener()
    await serverStore.setupStatusChangeListener()
  } catch (e) {
    console.warn('订阅事件失败:', e)
  }

  // 4. 加载配置项描述
  try {
    await configStore.loadDescriptions()
  } catch (e) {
    console.warn('加载配置描述失败:', e)
  }

  // 5. 网络初始检测（firewall + radmin 并行）
  try {
    await networkStore.checkAll()
  } catch (e) {
    console.warn('网络检测失败:', e)
  }

  // 6. 双模式判定
  if (serverStore.status.running) {
    uiStore.setMode('dashboard')
    serverStore.startPolling()
  } else {
    uiStore.setMode('wizard')
  }
}

void bootstrapStores()

// 暴露 router 供外部自动化（Tauri WebviewWindow::eval，E2E 验收用）
;(window as unknown as { __router: typeof router }).__router = router
