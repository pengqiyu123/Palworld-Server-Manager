<template>
  <section class="screen active">
    <!-- ====== 仪表盘模式 ====== -->
    <template v-if="isDashboard">
      <div class="page-head">
        <div>
          <div class="page-title">服务器概览</div>
          <div class="page-sub">服务器运行中 · 实时数据每 60 秒自动刷新</div>
        </div>
        <div class="page-actions">
          <button class="btn btn-ghost" :disabled="launchingGame" @click="onLaunchGame">
            {{ launchingGame ? '启动中…' : '启动游戏' }}
          </button>
          <button class="btn btn-ghost" :disabled="serverStore.loading" @click="onGracefulShutdown">
            优雅关服
          </button>
          <button class="btn btn-danger" :disabled="serverStore.loading" @click="onForceStop">
            强制停止
          </button>
        </div>
      </div>

      <!-- 服务器信息卡片 -->
      <div class="dash-info-row">
        <div class="dash-info-card">
          <span class="di-label">服务器名称</span>
          <span class="di-value">{{ serverInfo?.servername ?? '—' }}</span>
        </div>
        <div class="dash-info-card">
          <span class="di-label">游戏版本</span>
          <span class="di-value">{{ serverInfo?.version ?? '—' }}</span>
        </div>
        <div class="dash-info-card">
          <span class="di-label">世界 GUID</span>
          <span class="di-value mono">{{ serverInfo?.worldguid ? serverInfo.worldguid.slice(0, 12) + '…' : '—' }}</span>
        </div>
      </div>

      <!-- 指标卡片 -->
      <div class="dash-metrics-row">
        <div class="metric-card">
          <span class="m-label">FPS</span>
          <span class="m-value">{{ formatNum(serverMetrics?.serverfps) }}</span>
        </div>
        <div class="metric-card">
          <span class="m-label">平均 FPS</span>
          <span class="m-value">{{ formatNum(serverMetrics?.serverfpsaverage) }}</span>
        </div>
        <div class="metric-card">
          <span class="m-label">在线人数</span>
          <span class="m-value">{{ serverMetrics?.currentplayernum ?? 0 }} / {{ serverMetrics?.maxplayernum ?? 32 }}</span>
        </div>
        <div class="metric-card">
          <span class="m-label">游戏天数</span>
          <span class="m-value">{{ serverMetrics?.days ?? '—' }}</span>
        </div>
        <div class="metric-card">
          <span class="m-label">运行时长</span>
          <span class="m-value">{{ formatUptime(serverMetrics?.uptime) }}</span>
        </div>
        <div class="metric-card">
          <span class="m-label">帧时间</span>
          <span class="m-value">{{ formatNum(serverMetrics?.serverframetime) }} ms</span>
        </div>
      </div>

      <!-- 进程状态 -->
      <div class="dash-proc-row">
        <StatusPill status="ok" :text="`运行中 · PID ${serverStore.status.pid ?? '?'}`" />
        <span class="dash-proc-path" v-if="settingsStore.settings.server_path">
          {{ settingsStore.settings.server_path }}
        </span>
      </div>

      <!-- 联机健康总览（7 步引导，收官 M2） -->
      <div class="guide-section">
        <div class="guide-title">联机健康总览 · 7 步引导</div>
        <OnboardingProgress :steps="onboardingStore.steps" />
        <div class="guide-legend">
          <span><i class="lg-dot pass" /> 已通过</span>
          <span><i class="lg-dot fail" /> 待处理</span>
          <span><i class="lg-dot idle" /> 未开始</span>
        </div>
      </div>
    </template>

    <!-- ====== 向导模式 ====== -->
    <template v-else>
      <div class="s1-wrap">
        <div class="s1-col">
          <div class="step-badge">
            <span class="dot" />
            <span>{{ wizardStepText }}</span>
          </div>

          <svg class="s1-hero" viewBox="0 0 76 76" fill="none">
            <circle cx="38" cy="38" r="34" fill="#f0e2d4" stroke="#e8b9a8" stroke-width="1" />
            <circle cx="38" cy="38" r="22" fill="#f7eadd" stroke="#e66f51" stroke-width="2" />
            <path d="M17 38H59" stroke="#e66f51" stroke-width="2" />
            <circle cx="38" cy="38" r="6" fill="#3f322c" stroke="#e66f51" stroke-width="2" />
          </svg>

          <h1 class="s1-title">欢迎使用 Palworld 服务器管理器</h1>
          <p class="s1-sub">
            不用记命令行，也能开关服、图形化改配置、和朋友联机。先告诉我你的 PalServer.exe 在哪里——点下方"自动探测"，我来帮你找。
          </p>

          <div class="steps-row">
            <StepCard
              :num="1"
              title="定位服务器"
              desc="自动找到 PalServer.exe 所在目录"
              :state="step1State"
            />
            <StepCard
              :num="2"
              title="检查网络端口"
              desc="确认游戏/RCON/REST 端口已放行、可连通"
              :state="step2State"
            />
            <StepCard
              :num="3"
              title="启动并联机"
              desc="一键开服，把虚拟局域网 IP 发给朋友"
              :state="step3State"
            />
          </div>

          <div class="cta-row">
            <button
              class="btn-detect"
              :disabled="uiStore.wizard.detecting"
              @click="onDetect"
            >
              <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
                <circle cx="8" cy="8" r="5" stroke="#FFFFFF" stroke-width="1.6" />
                <path d="M12 12L15.5 15.5" stroke="#FFFFFF" stroke-width="1.6" stroke-linecap="round" />
              </svg>
              <span>{{ detectLabel }}</span>
            </button>
            <button class="btn-ghost" @click="onManual">手动选择目录…</button>
          </div>

          <!-- Step 2: 端口状态（探测完成后显示） -->
          <div v-if="uiStore.wizard.detected" class="wizard-step2">
            <div class="ws2-title">网络端口状态</div>
            <div class="ws2-ports">
              <StatusPill :status="networkStore.firewall.port_8211_open ? 'ok' : 'block'" text="游戏 8211 UDP" />
              <StatusPill :status="networkStore.firewall.port_25575_open ? 'ok' : 'block'" text="RCON 25575 TCP" />
              <StatusPill :status="networkStore.firewall.port_8212_open ? 'ok' : 'block'" text="REST 8212 TCP" />
            </div>
          </div>

          <!-- Step 3: 启动按钮 -->
          <div v-if="uiStore.wizard.detected" class="wizard-step3">
            <button
              class="btn btn-primary btn-lg"
              :disabled="serverStore.loading"
              @click="onStart"
            >
              {{ serverStore.loading ? '启动中…' : '启动服务器' }}
            </button>
            <span class="ws3-path">{{ detectedPath }}</span>
          </div>

          <div class="tip-row">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <circle cx="8" cy="8" r="6.2" stroke="#a39383" stroke-width="1.3" />
              <path d="M8 7.2V11" stroke="#a39383" stroke-width="1.3" stroke-linecap="round" />
              <circle cx="8" cy="5" r="0.7" fill="#a39383" />
            </svg>
            <span>探测不到？可手动指定目录。遇到不懂的词，悬停"ⓘ 这是什么"即可看解释。</span>
          </div>
        </div>
      </div>
    </template>

    <!-- ====== 确认弹窗 ====== -->
    <ConfirmDialog
      v-model:visible="confirmVisible"
      :title="confirmTitle"
      :message="confirmMessage"
      :danger="confirmDanger"
      @confirm="onConfirm"
    />
  </section>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useUiStore } from '@/stores/ui'
import { useServerStore } from '@/stores/server'
import { useSettingsStore } from '@/stores/settings'
import { useNetworkStore } from '@/stores/network'
import { useToast } from '@/components/ui/useToast'
import { useOnboardingStore } from '@/stores/onboarding'
import { api } from '@/api/tauri'
import StepCard from '@/components/ui/StepCard.vue'
import StatusPill from '@/components/ui/StatusPill.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import OnboardingProgress from '@/components/ui/OnboardingProgress.vue'

const uiStore = useUiStore()
const serverStore = useServerStore()
const settingsStore = useSettingsStore()
const networkStore = useNetworkStore()
const onboardingStore = useOnboardingStore()
const toast = useToast()

const launchingGame = ref(false)

// ====== 联机成功一次性回调（D1 验收时刻） ======
// 第 7 步（S7 朋友入服）首次 players>=2 时由 onboardingStore 幂等触发（successFired 锁，仅一次）
onMounted(() => {
  onboardingStore.onSuccess(() => {
    toast.success('🎉 联机成功！朋友已连入你的帕鲁服，D1 验收达成 🏆')
  })
})

// ====== 双模式判定 ======
const isDashboard = computed(() => uiStore.wizard.mode === 'dashboard')

// ====== F3 · 启动游戏本体 ======
async function onLaunchGame(): Promise<void> {
  if (launchingGame.value) return
  launchingGame.value = true
  try {
    const msg = await api.launcher.launchGame()
    toast.success(msg)
  } catch (e) {
    toast.error(`启动游戏失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    launchingGame.value = false
  }
}

// ====== 向导模式 ======
const detectLabel = ref('自动探测 PalServer.exe')

const detectedPath = computed(() => {
  return settingsStore.settings.server_path || uiStore.wizard.detectedPath
})

const step1State = computed<'active' | 'locked'>(() =>
  uiStore.wizard.detected ? 'locked' : 'active'
)
const step2State = computed<'active' | 'locked'>(() =>
  uiStore.wizard.detected ? 'active' : 'locked'
)
const step3State = computed(() => 'locked' as const)

const wizardStepText = computed(() =>
  uiStore.wizard.detected ? '第 1 步已完成 ✓ · 进入第 2 步' : '首次设置 · 第 1 步，共 3 步'
)

async function onDetect(): Promise<void> {
  if (uiStore.wizard.detecting) return
  uiStore.startDetect()
  detectLabel.value = '正在探测 PalServer.exe…'
  try {
    const paths = await api.steam.detect()
    if (paths.length > 0) {
      const path = paths[0]
      settingsStore.update({ server_path: path })
      await settingsStore.save()
      uiStore.finishDetect(path)
      detectLabel.value = `✓ 已找到 ${path}`
      // 探测完成后检查网络端口
      await networkStore.checkAll()
    } else {
      uiStore.wizard.detecting = false
      detectLabel.value = '未找到，请手动指定'
      toast.warning('未检测到 PalServer 安装，请手动指定目录')
    }
  } catch (e) {
    uiStore.wizard.detecting = false
    detectLabel.value = '探测失败，请手动指定'
    toast.error(`探测失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

function onManual(): void {
  const path = settingsStore.settings.server_path || 'D:\\Steam\\steamapps\\common\\Palworld\\PalServer'
  uiStore.setManual(path)
  detectLabel.value = `✓ 已指定 ${path}`
}

async function onStart(): Promise<void> {
  const path = settingsStore.settings.server_path
  if (!path) {
    toast.error('请先定位服务器目录')
    return
  }
  try {
    await serverStore.start(path)
    if (serverStore.status.running) {
      uiStore.setMode('dashboard')
      serverStore.startPolling()
      toast.success('服务器已启动')
    }
  } catch (e) {
    toast.error(`启动失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

// ====== 仪表盘模式 ======
const serverInfo = computed(() => serverStore.serverInfo)
const serverMetrics = computed(() => serverStore.serverMetrics)

function formatNum(v: number | null | undefined): string {
  if (v === null || v === undefined) return '—'
  return Number.isInteger(v) ? String(v) : v.toFixed(1)
}

function formatUptime(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined) return '—'
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${s}s`
  return `${s}s`
}

// ====== 确认弹窗 ======
const confirmVisible = ref(false)
const confirmTitle = ref('')
const confirmMessage = ref('')
const confirmDanger = ref(false)
let confirmAction: (() => Promise<void>) | null = null

async function onGracefulShutdown(): Promise<void> {
  confirmTitle.value = '优雅关服'
  confirmMessage.value = '服务器将在 30 秒后关闭，并广播关服通知给所有在线玩家。确认关闭?'
  confirmDanger.value = false
  confirmAction = async () => {
    try {
      await serverStore.gracefulShutdown(30, '服务器即将关闭，请保存进度')
      toast.success('关服指令已发送，服务器将在 30 秒后关闭')
      // server-status-change 事件会自动 stopPolling + 切回向导
    } catch (e) {
      toast.error(`关服失败: ${e instanceof Error ? e.message : String(e)}`)
    }
  }
  confirmVisible.value = true
}

async function onForceStop(): Promise<void> {
  confirmTitle.value = '强制停止服务器'
  confirmMessage.value = '强制停止会立即终止服务器进程，可能导致未保存的进度丢失。确认强制停止?'
  confirmDanger.value = true
  confirmAction = async () => {
    try {
      await serverStore.forceStop()
      uiStore.setMode('wizard')
      toast.info('服务器已强制停止')
    } catch (e) {
      toast.error(`停止失败: ${e instanceof Error ? e.message : String(e)}`)
    }
  }
  confirmVisible.value = true
}

async function onConfirm(): Promise<void> {
  if (confirmAction) {
    await confirmAction()
    confirmAction = null
  }
}
</script>

<style scoped>
.dash-info-row {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
}
.dash-info-card {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 16px 20px;
  border-radius: 12px;
  background: var(--palwarm-surface, #faf6f0);
  border: 1px solid var(--palwarm-border, #e8ddd0);
}
.di-label {
  font-size: 12px;
  color: var(--text-mid2, #a39383);
}
.di-value {
  font-size: 18px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.di-value.mono {
  font-family: 'JetBrains Mono', monospace;
  font-size: 14px;
}
.dash-metrics-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
  margin-bottom: 16px;
}
.metric-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px 18px;
  border-radius: 10px;
  background: var(--palwarm-surface, #faf6f0);
  border: 1px solid var(--palwarm-border, #e8ddd0);
}
.m-label {
  font-size: 12px;
  color: var(--text-mid2, #a39383);
}
.m-value {
  font-size: 22px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
}
.dash-proc-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.dash-proc-path {
  font-size: 12px;
  color: var(--text-mid2, #a39383);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wizard-step2 {
  margin-top: 16px;
  padding: 14px 18px;
  border-radius: 10px;
  background: var(--palwarm-surface, #faf6f0);
  border: 1px solid var(--palwarm-border, #e8ddd0);
}
.ws2-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
  margin-bottom: 10px;
}
.ws2-ports {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}
.wizard-step3 {
  margin-top: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: flex-start;
}
.ws3-path {
  font-size: 12px;
  color: var(--text-mid2, #a39383);
  word-break: break-all;
}
.guide-section {
  margin-top: 16px;
  padding: 18px 22px;
  border-radius: 12px;
  background: var(--palwarm-surface, #faf6f0);
  border: 1px solid var(--palwarm-border, #e8ddd0);
}
.guide-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
  margin-bottom: 16px;
}
.guide-legend {
  display: flex;
  gap: 18px;
  margin-top: 14px;
  font-size: 12px;
  color: var(--text-mid2, #a39383);
}
.guide-legend span {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.lg-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  display: inline-block;
}
.lg-dot.pass {
  background: var(--green, #4f8a6b);
}
.lg-dot.fail {
  background: var(--red, #c9554d);
}
.lg-dot.idle {
  background: rgba(0, 0, 0, 0.12);
}
</style>
