<template>
  <section class="screen active">
    <!-- 配置空白横幅（P1）：PalWorldSettings.ini 未初始化时提示 -->
    <div class="config-banner" v-if="serverPathDetected && isConfigInitialized === false">
      <AppIcon name="info" :size="16" />
      <span class="config-banner-text">
        检测到 <code>PalWorldSettings.ini</code> 为空或未含配置，开服前需先填充默认配置。
      </span>
      <button class="btn btn-ghost btn-sm" @click="onFillDefault">点此立即填充默认配置</button>
    </div>

    <div v-if="serverPathDetected" class="overview-live">
      <header class="overview-live-head">
        <div>
          <div class="page-title">服务器概览</div>
          <div class="page-sub">当前运行状态、联机数据和常用管理入口</div>
        </div>
        <button class="btn btn-ghost btn-sm" :disabled="refreshingStatus" @click="onRefreshNow">
          {{ refreshingStatus ? '正在检查…' : '立即刷新' }}
        </button>
      </header>

      <section class="server-truth-band" :class="liveStatusClass">
        <div class="truth-status">
          <span class="truth-dot" />
          <div>
            <span class="truth-label">当前运行状态</span>
            <strong>{{ liveStatusLabel }}</strong>
            <p>{{ liveStatusDescription }}</p>
          </div>
        </div>
        <div class="truth-facts">
          <div>
            <span>启动来源</span>
            <strong>{{ serverStore.status.managed_by_app ? '管理器启动' : isRunning ? '外部启动' : '—' }}</strong>
          </div>
          <div>
            <span>进程</span>
            <strong>{{ serverStore.status.pid ? `PID ${serverStore.status.pid}` : '未检测到' }}</strong>
          </div>
          <div>
            <span>最近检查</span>
            <strong>{{ lastCheckedText }}</strong>
          </div>
        </div>
        <div class="truth-actions">
          <button
            v-if="!isRunning"
            class="btn btn-primary"
            :disabled="serverStore.loading"
            @click="onStart"
          >
            {{ serverStore.starting ? '启动中…' : '启动服务器' }}
          </button>
          <template v-else>
            <button class="btn btn-ghost" :disabled="serverStore.loading" @click="onGracefulShutdown">优雅关服</button>
            <button class="btn btn-danger-ghost" :disabled="serverStore.loading" @click="onForceStop">强制停止</button>
          </template>
        </div>
      </section>

      <section class="overview-metrics" aria-label="服务器实时指标">
        <div class="overview-metric">
          <span>在线玩家</span>
          <strong>{{ livePlayersText }}</strong>
          <small>{{ livePlayersHint }}</small>
        </div>
        <div class="overview-metric">
          <span>服务器 FPS</span>
          <strong>{{ liveFpsText }}</strong>
          <small>{{ isReady ? '运行表现' : '服务器离线' }}</small>
        </div>
        <div class="overview-metric">
          <span>运行时间</span>
          <strong>{{ uptimeText }}</strong>
          <small>{{ serverStore.serverInfo?.version || '版本待读取' }}</small>
        </div>
        <div class="overview-metric">
          <span>配置人数</span>
          <strong>{{ configuredMaxPlayers ?? '—' }}</strong>
          <small>PalWorldSettings.ini</small>
        </div>
      </section>

      <div v-if="maxPlayersPendingRestart" class="runtime-config-warning">
        <AppIcon name="info" :size="16" />
        当前服务器上限为 {{ serverStore.serverMetrics?.maxplayernum }} 人，配置已改为 {{ configuredMaxPlayers }} 人；重启服务器后再检查实际值。
      </div>

      <div v-if="serverStore.playersState === 'error'" class="live-data-warning" role="alert">
        <AppIcon name="info" :size="16" />
        <div>
          <strong>联机数据读取失败</strong>
          <span>{{ serverStore.playersError }}</span>
        </div>
        <button type="button" class="btn btn-ghost btn-sm" @click="router.push('/players')">查看原因</button>
      </div>

      <section v-else-if="serverStore.playersState === 'live'" class="overview-player-strip" aria-label="在线玩家">
        <div>
          <strong>在线玩家</strong>
          <span>{{ serverStore.players.length ? '名单与玩家管理实时同步' : '当前无人在线' }}</span>
        </div>
        <div v-if="serverStore.players.length" class="overview-player-list">
          <span v-for="player in serverStore.players" :key="player.userId">
            {{ player.name || '未命名玩家' }}
          </span>
        </div>
        <button type="button" class="btn btn-ghost btn-sm" @click="router.push('/players')">玩家管理</button>
      </section>

      <section class="overview-section">
        <div class="overview-section-head">
          <div>
            <h2>服务器与联机</h2>
            <p>启动朋友联机所需程序，服务器状态由管理器持续跟踪。</p>
          </div>
        </div>
        <div class="overview-actions-row">
          <button class="btn btn-primary" :disabled="launchingRadmin" @click="onLaunchRadmin">
            <AppIcon name="vpn" :size="16" />
            {{ launchingRadmin ? '启动中…' : '启动 Radmin VPN' }}
          </button>
          <button class="btn btn-ghost" :disabled="launchingGame" @click="onLaunchGame">
            {{ launchingGame ? '启动中…' : '启动游戏' }}
          </button>
          <span class="overview-path" :title="settingsStore.settings.server_path">
            {{ settingsStore.settings.server_path }}
          </span>
        </div>
      </section>

      <section class="overview-section">
        <div class="overview-section-head">
          <div>
            <h2>管理工具</h2>
            <p>直接进入已经完成的配置、在线管理、存档与迁移功能。</p>
          </div>
        </div>
        <div class="overview-tool-grid">
          <button type="button" @click="router.push('/config')"><AppIcon name="config" :size="18" /><span><strong>服务器配置</strong><small>玩法预设与完整参数</small></span></button>
          <button type="button" @click="router.push('/rcon')"><AppIcon name="rcon" :size="18" /><span><strong>在线管理</strong><small>玩家、保存与公告</small></span></button>
          <button type="button" @click="router.push('/logs')"><AppIcon name="logs" :size="18" /><span><strong>实时日志</strong><small>查看启动与运行记录</small></span></button>
          <button type="button" @click="router.push('/saves')"><AppIcon name="save" :size="18" /><span><strong>世界存档</strong><small>详情、备份与回滚</small></span></button>
          <button type="button" @click="router.push('/migrate')"><AppIcon name="migration" :size="18" /><span><strong>存档迁移</strong><small>世界、角色与公会</small></span></button>
          <button type="button" @click="router.push('/modifier')"><AppIcon name="modifier" :size="18" /><span><strong>修改器</strong><small>玩家与公会管理</small></span></button>
        </div>
      </section>
    </div>

    <!-- 仅在尚未找到服务器时显示首次使用引导。 -->
      <div v-else class="s1-wrap">
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

          <!-- StepCard 1：获取三个应用路径（StepCard 当标题卡 + 下方 glass 操作区） -->
          <div class="sc-block">
            <StepCard :num="1" title="获取应用路径" desc="自动定位 服务器 / Radmin / 游戏" :state="step1State" />
            <div class="sc-body">
              <!-- 服务器：已定位显路径；未定位显「自动探测」(onDetect) + 「手动选目录」(onManualServer) -->
              <div class="sc-path-row" :class="{ ok: serverPathDetected }">
                <span class="sc-num">1</span>
                <div class="sc-path-info">
                  <span class="sc-path-name">服务器 (PalServer)</span>
                  <span class="sc-path-state" :class="serverPathDetected ? 'ok' : 'todo'">
                    {{ serverPathDetected ? '已定位 ✓' : '待定位' }}
                  </span>
                  <span v-if="serverPathDetected" class="sc-path-detail">{{ settingsStore.settings.server_path }}</span>
                </div>
                <button v-if="!serverPathDetected" class="btn btn-ghost btn-sm" :disabled="uiStore.wizard.detecting" @click="onDetect">{{ detectLabel }}</button>
                <button v-if="!serverPathDetected" class="btn btn-ghost btn-sm" @click="onManualServer">手动选目录</button>
              </div>
              <!-- Radmin：自动检测失败时可手动选择并由后端验证 exe。 -->
              <div class="sc-path-row" :class="{ ok: radminDetected, fail: radminNotInstalled }">
                <span class="sc-num">2</span>
                <div class="sc-path-info">
                  <span class="sc-path-name">Radmin VPN</span>
                  <span class="sc-path-state" :class="radminDetected ? 'ok' : radminNotInstalled ? 'fail' : 'todo'">
                    {{ radminDetected ? '已定位 ✓' : radminNotInstalled ? '未安装' : '待定位' }}
                  </span>
                  <span v-if="radminPath" class="sc-path-detail">{{ radminPath }}</span>
                  <span v-if="radminNotInstalled" class="sc-path-hint">请先安装 Radmin VPN</span>
                </div>
                <button v-if="!radminDetected" class="btn btn-ghost btn-sm" @click="onManualRadmin">手动选择 exe</button>
              </div>
              <!-- 游戏：从 Steam 库真实探测，不接受手动确认。 -->
              <div class="sc-path-row" :class="{ ok: gameDetected }">
                <span class="sc-num">3</span>
                <div class="sc-path-info">
                  <span class="sc-path-name">游戏 (Palworld)</span>
                  <span class="sc-path-state" :class="gameDetected ? 'ok' : 'todo'">
                    {{ gameDetected ? '已定位 ✓' : '待定位' }}
                  </span>
                  <span v-if="gameDetected" class="sc-path-detail">steam://rungameid/1623730</span>
                </div>
                <button v-if="!gameDetected" class="btn btn-ghost btn-sm" @click="onManualGame">检测游戏</button>
              </div>
            </div>
          </div>

          <!-- StepCard 2：启动并联机 -->
          <div class="sc-block">
            <StepCard :num="2" title="启动并联机" desc="开服 + Radmin + 游戏客户端" :state="step2State" />
            <div class="sc-body sc-launch-row">
              <!-- 服务器：!isRunning→启动服务器(onStart)；isRunning→优雅关服+强制停止 -->
              <div class="sc-launch-card" :class="{ done: isRunning }">
                <div class="sc-lc-head">
                  <span class="sc-num">1</span>
                  <span class="sc-lc-state" :class="isRunning ? 'ok' : 'todo'">{{ isRunning ? '运行中' : '未启动' }}</span>
                </div>
                <div class="sc-lc-title">服务器</div>
                <div class="sc-lc-desc">
                  {{ isRunning
                    ? serverStore.status.managed_by_app
                      ? '由管理器后台启动；日志在「实时日志」中。'
                      : '手动启动的服务器日志在黑色窗口中；管理器可查看状态和关服。'
                    : '启动 PalServer，让朋友能连入你的专用服。' }}
                </div>
                <div v-if="isRunning" class="sc-runtime">
                  <span>PID {{ serverStore.status.pid ?? '未知' }}</span>
                  <span>游戏端口使用 8211/UDP</span>
                </div>
                <button
                  v-if="!isRunning"
                  class="btn btn-primary btn-sm"
                  :disabled="!serverPathDetected || serverStore.loading"
                  @click="onStart"
                >
                  {{ serverStore.starting ? '启动中…' : '启动服务器' }}
                </button>
                <template v-else>
                  <button class="btn btn-ghost btn-sm" :disabled="serverStore.loading" @click="onGracefulShutdown">优雅关服</button>
                  <button class="btn btn-danger-ghost btn-sm" :disabled="serverStore.loading" @click="onForceStop">强制停止</button>
                </template>
              </div>
              <!-- Radmin：onLaunchRadmin -->
              <div class="sc-launch-card" :class="{ done: radminLaunched }">
                <div class="sc-lc-head">
                  <span class="sc-num">2</span>
                  <span class="sc-lc-state" :class="radminLaunched ? 'ok' : 'todo'">{{ radminLaunched ? '已就绪' : '待就绪' }}</span>
                </div>
                <div class="sc-lc-title">Radmin VPN</div>
                <div class="sc-lc-desc">拉起 Radmin VPN，建立虚拟局域网便于联机。</div>
                <button class="btn btn-primary btn-sm" :disabled="launchingRadmin" @click="onLaunchRadmin">
                  <AppIcon name="vpn" :size="16" />
                  <span>{{ launchingRadmin ? '启动中…' : '启动 Radmin VPN' }}</span>
                </button>
              </div>
              <!-- 游戏：onLaunchGame -->
              <div class="sc-launch-card">
                <div class="sc-lc-head">
                  <span class="sc-num">3</span>
                  <span class="sc-lc-state" :class="gameDetected ? 'ok' : 'todo'">{{ gameDetected ? '已定位' : '待定位' }}</span>
                </div>
                <div class="sc-lc-title">游戏</div>
                <div class="sc-lc-desc">启动本地 Steam 帕鲁客户端，进服联机。</div>
                <button class="btn btn-ghost btn-sm" :disabled="launchingGame" @click="onLaunchGame">
                  {{ launchingGame ? '启动中…' : '启动游戏' }}
                </button>
              </div>
            </div>
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
    <!-- ====== 确认弹窗 ====== -->
    <ConfirmDialog
      v-model:visible="confirmVisible"
      :title="confirmTitle"
      :message="confirmMessage"
      :danger="confirmDanger"
      @confirm="onConfirm"
    />

    <!-- F2 · 启动 Radmin VPN 后的加入引导弹窗（从网络页迁入） -->
    <RadminLaunchModal v-model:visible="radminLaunchVisible" />
  </section>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useUiStore } from '@/stores/ui'
import { useServerStore } from '@/stores/server'
import { useSettingsStore } from '@/stores/settings'
import { useConfigStore } from '@/stores/config'
import { useNetworkStore } from '@/stores/network'
import { useToast } from '@/components/ui/useToast'
import { useOnboardingStore } from '@/stores/onboarding'
import { api } from '@/api/tauri'
import { open } from '@tauri-apps/plugin-dialog'
import StepCard from '@/components/ui/StepCard.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import AppIcon from '@/components/ui/AppIcon.vue'
import RadminLaunchModal from '@/components/ui/RadminLaunchModal.vue'

const uiStore = useUiStore()
const serverStore = useServerStore()
const settingsStore = useSettingsStore()
const configStore = useConfigStore()
const networkStore = useNetworkStore()
const onboardingStore = useOnboardingStore()
const toast = useToast()
const router = useRouter()

const launchingGame = ref(false)
const launchingRadmin = ref(false)
const radminLaunched = ref(false)
const radminLaunchVisible = ref(false)
// R3 · 第 1 步三路径探测态
const radminDetected = ref(false)
const radminNotInstalled = ref(false)
const radminPath = ref('')
const gameDetected = ref(false)
const refreshingStatus = ref(false)

// 朋友首次加入时只提示一次。
// 第 7 步（S7 朋友入服）首次 players>=2 时由 onboardingStore 幂等触发（successFired 锁，仅一次）
onMounted(() => {
  onboardingStore.onSuccess(() => {
    toast.success('联机成功，朋友已加入服务器')
  })
  void detectPaths()
})

watch(() => settingsStore.settings.server_path, () => {
  void checkConfigInitialized()
}, { immediate: true })

// 启动区常驻所需的运行状态判定（与 Sidebar 同源）
const isRunning = computed(() => serverStore.status.running)
const isReady = computed(() => serverStore.status.ready)
const serverPathDetected = computed(() => !!settingsStore.settings.server_path)
const configuredMaxPlayers = computed(() => {
  const value = Number.parseInt(configStore.config.ServerPlayerMaxNum ?? '', 10)
  return Number.isFinite(value) && value > 0 ? value : null
})
const maxPlayersPendingRestart = computed(() => {
  const runtime = serverStore.serverMetrics?.maxplayernum
  return runtime !== undefined && configuredMaxPlayers.value !== null && runtime !== configuredMaxPlayers.value
})
const liveStatusLabel = computed(() => {
  if (isReady.value) return '服务器在线'
  if (isRunning.value) return '服务器正在启动'
  return '服务器离线'
})
const liveStatusDescription = computed(() => {
  if (isReady.value) return '游戏端口已就绪，可以接受玩家连接。'
  if (isRunning.value) return '已找到服务器进程，正在等待游戏端口就绪。'
  return '未检测到服务器进程，可以从这里启动或在外部手动启动。'
})
const liveStatusClass = computed(() => ({
  online: isReady.value,
  starting: isRunning.value && !isReady.value,
  offline: !isRunning.value,
}))
const lastCheckedText = computed(() => {
  const checkedAt = serverStore.lastCheckedAt
  return checkedAt ? checkedAt.toLocaleTimeString('zh-CN', { hour12: false }) : '等待检查'
})
const livePlayersText = computed(() => {
  if (serverStore.playersState === 'error' && !serverStore.serverMetrics) return '读取失败'
  const metrics = serverStore.serverMetrics
  const maxPlayers = metrics?.maxplayernum ?? configuredMaxPlayers.value
  const currentPlayers = metrics?.currentplayernum ??
    (serverStore.playersState === 'live' ? serverStore.players.length : 0)
  return `${currentPlayers}/${maxPlayers ?? '—'}`
})
const livePlayersHint = computed(() => {
  if (!isReady.value) return '服务器离线'
  if (serverStore.playersState === 'loading') return '正在读取服务器数据'
  if (serverStore.playersState === 'error') return '读取失败，不代表无人在线'
  if (serverStore.playersState === 'live') return '每 3 秒自动同步'
  return '等待服务器数据'
})
const liveFpsText = computed(() => {
  const fps = serverStore.serverMetrics?.serverfps
  return fps === undefined ? '—' : String(Math.round(fps))
})
const uptimeText = computed(() => {
  const seconds = serverStore.serverMetrics?.uptime
  if (seconds === undefined) return '—'
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  return hours > 0 ? `${hours} 小时 ${minutes} 分` : `${minutes} 分钟`
})

async function onRefreshNow(): Promise<void> {
  if (refreshingStatus.value) return
  refreshingStatus.value = true
  try {
    await serverStore.pollOnce()
  } finally {
    refreshingStatus.value = false
  }
}

// ====== F3 · 启动游戏本体 ======
async function onLaunchGame(): Promise<void> {
  if (launchingGame.value) return
  launchingGame.value = true
  try {
    const msg = await api.launcher.launchGame()
    toast.success(msg)
    gameDetected.value = true
  } catch (e) {
    toast.error(`启动游戏失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    launchingGame.value = false
  }
}

// ====== 向导模式 ======
const detectLabel = ref('自动探测 PalServer.exe')

const step1State = computed<'active' | 'locked' | 'done'>(() =>
  serverPathDetected.value && radminDetected.value && gameDetected.value ? 'done' : 'active'
)
const step2State = computed<'active' | 'locked' | 'done'>(() =>
  isRunning.value ? 'done' : serverPathDetected.value ? 'active' : 'locked'
)

const wizardStepText = computed(() =>
  isRunning.value
    ? '服务器运行中 · 可优雅关服'
    : serverPathDetected.value
      ? '路径已就绪 · 点击启动开服'
      : '首次设置 · 定位应用路径'
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

// ====== R3 · 第 1 步：自动探测三应用路径 ======
async function detectPaths(): Promise<void> {
  // 服务器：若已设置则已定位；否则自动探测并写回
  if (!serverPathDetected.value) {
    try {
      const paths = await api.steam.detect()
      if (paths.length > 0) {
        settingsStore.update({ server_path: paths[0] })
        await settingsStore.save()
        uiStore.finishDetect(paths[0])
        detectLabel.value = `✓ 已找到 ${paths[0]}`
      }
    } catch {
      // 探测失败静默，显示「待定位」+ 手动选目录
    }
  }
  // 优先验证用户保存的 Radmin 路径，再检查标准安装位置与虚拟网卡。
  try {
    const savedRadminPath = settingsStore.settings.radmin_path
    if (savedRadminPath) {
      const executable = await api.launcher.validateRadminPath(savedRadminPath)
      radminDetected.value = Boolean(executable)
      radminNotInstalled.value = false
      radminPath.value = executable
    }
  } catch {
    settingsStore.update({ radmin_path: '' })
    await settingsStore.save()
  }
  try {
    await networkStore.checkRadmin()
    radminDetected.value = radminDetected.value || networkStore.radmin.installed
    radminNotInstalled.value = !radminDetected.value
    if (!radminPath.value) radminPath.value = networkStore.radmin.installed
      ? networkStore.radmin.virtual_ip
        ? `虚拟网卡 ${networkStore.radmin.virtual_ip}`
        : '已安装 Radmin VPN'
      : ''
  } catch {
    // 静默
  }
  try {
    const gamePath = await api.launcher.detectGame()
    gameDetected.value = Boolean(gamePath)
  } catch {
    gameDetected.value = false
  }
}

// 手动选目录（dialog，不写死默认路径，R1③）
async function onManualServer(): Promise<void> {
  const dir = await open({ directory: true })
  if (typeof dir !== 'string' || !dir) return
  settingsStore.update({ server_path: dir })
  await settingsStore.save()
  uiStore.finishDetect(dir)
  detectLabel.value = `✓ 已指定 ${dir}`
}

async function onManualRadmin(): Promise<void> {
  const selected = await open({
    multiple: false,
    filters: [{ name: 'Radmin VPN', extensions: ['exe'] }],
  })
  if (typeof selected !== 'string' || !selected) return
  try {
    const executable = await api.launcher.validateRadminPath(selected)
    settingsStore.update({ radmin_path: executable })
    await settingsStore.save()
    radminDetected.value = Boolean(executable)
    radminNotInstalled.value = false
    radminPath.value = executable
    toast.success('Radmin VPN 路径已验证')
  } catch (error) {
    toast.error(`Radmin VPN 路径无效: ${error instanceof Error ? error.message : String(error)}`)
  }
}

async function onManualGame(): Promise<void> {
  try {
    const gamePath = await api.launcher.detectGame()
    gameDetected.value = Boolean(gamePath)
    toast.success('已在 Steam 库中找到游戏')
  } catch (error) {
    gameDetected.value = false
    toast.error(`未找到游戏: ${error instanceof Error ? error.message : String(error)}`)
  }
}

async function onStart(): Promise<void> {
  const path = settingsStore.settings.server_path
  if (!path) {
    toast.error('请先定位服务器目录')
    return
  }
  await checkConfigInitialized()
  if (isConfigInitialized.value !== true) {
    toast.info('首次开服，请先完成配置')
    await router.push('/config?firstTime=true')
    return
  }
  try {
    await serverStore.start(path)
    if (serverStore.status.running) {
      serverStore.startPolling()
      toast.success(`服务器已就绪（PID ${serverStore.status.pid ?? '未知'}）`)
    } else {
      // spawn 返回但 running=false（罕见，如进程立即退出）——诚实告知
      toast.warning('服务器启动异常：进程未持续运行，请检查日志')
    }
  } catch (e) {
    toast.error(`启动失败: ${e instanceof Error ? e.message : String(e)}`)
  }
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

// ====== 常驻启动区：Radmin VPN 启动（从网络页迁入）======
async function onLaunchRadmin(): Promise<void> {
  if (launchingRadmin.value) return
  launchingRadmin.value = true
  try {
    const msg = await api.launcher.launchRadminVpn(settingsStore.settings.radmin_path)
    toast.success(msg)
    radminLaunched.value = true
    radminDetected.value = true
    radminNotInstalled.value = false
    radminLaunchVisible.value = true
  } catch (e) {
    toast.error(`启动 Radmin VPN 失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    launchingRadmin.value = false
  }
}

// ====== 配置空白横幅（P1）：探测 live 是否已含 OptionSettings=( ======
const isConfigInitialized = ref<boolean | null>(null)
async function checkConfigInitialized(): Promise<void> {
  if (!serverPathDetected.value) {
    isConfigInitialized.value = null
    return
  }
  try {
    isConfigInitialized.value = await api.config.isInitialized(settingsStore.settings.server_path)
  } catch (error) {
    isConfigInitialized.value = null
    const message = error instanceof Error ? error.message : String(error)
    toast.error(`读取服务器配置失败: ${message}`)
  }
}

// ====== 一键填充默认配置（仅手动触发，绝不接入 start_server 自动守卫）======
async function onFillDefault(): Promise<void> {
  if (!serverPathDetected.value) {
    toast.warning('尚未设置服务器路径，请先到【设置】填写 PalServer 根目录')
    return
  }
  try {
    const res = await api.config.fillDefault(settingsStore.settings.server_path)
    toast.success(res.message)
    isConfigInitialized.value = true
  } catch (e) {
    toast.error(`填充默认配置失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}
</script>

<style scoped>
.overview-live { display: grid; gap: 16px; min-width: 0; }
.overview-live-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.server-truth-band { display: grid; grid-template-columns: minmax(240px, 1.25fr) minmax(300px, 1fr) auto; align-items: center; gap: 20px; padding: 18px 20px; border: 1px solid var(--glass-border); border-radius: 8px; background: var(--glass-bg-soft); }
.server-truth-band.online { border-color: rgba(79, 138, 107, .42); background: rgba(79, 138, 107, .08); }
.server-truth-band.starting { border-color: rgba(184, 120, 47, .38); background: rgba(184, 120, 47, .08); }
.truth-status { display: flex; align-items: flex-start; gap: 12px; min-width: 0; }
.truth-dot { width: 10px; height: 10px; margin-top: 6px; flex: 0 0 10px; border-radius: 50%; background: var(--text-lo); box-shadow: 0 0 0 4px rgba(116, 88, 72, .1); }
.online .truth-dot { background: var(--green); box-shadow: 0 0 0 4px rgba(79, 138, 107, .15); }
.starting .truth-dot { background: var(--amber); box-shadow: 0 0 0 4px rgba(184, 120, 47, .15); }
.truth-label, .truth-facts span { display: block; color: var(--text-lo); font-size: 11px; line-height: 16px; }
.truth-status strong { display: block; margin-top: 1px; color: var(--text-hi); font-size: 18px; line-height: 25px; }
.truth-status p { margin: 2px 0 0; color: var(--text-mid2); font-size: 12px; line-height: 18px; }
.truth-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; }
.truth-facts strong { display: block; margin-top: 2px; overflow: hidden; color: var(--text-hi); font-size: 12px; line-height: 18px; text-overflow: ellipsis; white-space: nowrap; }
.truth-actions { display: flex; justify-content: flex-end; gap: 8px; }
.overview-metrics { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); border-top: 1px solid var(--glass-border); border-bottom: 1px solid var(--glass-border); }
.overview-metric { min-width: 0; padding: 14px 18px; border-right: 1px solid var(--glass-border); }
.overview-metric:last-child { border-right: 0; }
.overview-metric span, .overview-metric small { display: block; overflow: hidden; color: var(--text-lo); font-size: 11px; line-height: 16px; text-overflow: ellipsis; white-space: nowrap; }
.overview-metric strong { display: block; margin: 3px 0; color: var(--text-hi); font-size: 21px; line-height: 28px; }
.runtime-config-warning { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border: 1px solid rgba(184, 120, 47, .32); border-radius: 7px; background: rgba(184, 120, 47, .09); color: var(--amber, #9b5c14); font-size: 12px; line-height: 18px; }
.live-data-warning, .overview-player-strip { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border: 1px solid var(--glass-border); border-radius: 7px; background: var(--glass-bg-soft); }
.live-data-warning { border-color: rgba(201, 85, 77, .34); background: rgba(201, 85, 77, .08); color: var(--red, #c9554d); }
.live-data-warning > div, .overview-player-strip > div:first-child { min-width: 0; flex: 1; }
.live-data-warning strong, .live-data-warning span, .overview-player-strip strong, .overview-player-strip > div:first-child span { display: block; }
.live-data-warning span, .overview-player-strip > div:first-child span { margin-top: 1px; color: var(--text-lo); font-size: 11px; line-height: 16px; }
.overview-player-list { display: flex; flex: 2; flex-wrap: wrap; justify-content: flex-end; gap: 6px; min-width: 0; }
.overview-player-list span { max-width: 160px; overflow: hidden; padding: 3px 8px; border: 1px solid rgba(79, 138, 107, .2); border-radius: 999px; background: rgba(79, 138, 107, .09); color: var(--green, #4f8a6b); font-size: 11px; line-height: 16px; text-overflow: ellipsis; white-space: nowrap; }
.overview-section { padding-top: 4px; }
.overview-section + .overview-section { padding-top: 14px; border-top: 1px solid var(--glass-border); }
.overview-section-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 10px; }
.overview-section-head h2 { margin: 0; color: var(--text-hi); font-size: 15px; line-height: 22px; }
.overview-section-head p { margin: 2px 0 0; color: var(--text-lo); font-size: 11px; line-height: 17px; }
.overview-actions-row { display: flex; align-items: center; gap: 8px; }
.overview-path { min-width: 0; margin-left: 4px; overflow: hidden; color: var(--text-lo); font-family: var(--font-mono); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.overview-tool-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
.overview-tool-grid button { display: flex; align-items: center; gap: 10px; min-width: 0; padding: 11px 12px; border: 1px solid var(--glass-border); border-radius: 7px; background: var(--glass-bg-soft); color: var(--text-mid); cursor: pointer; font: inherit; text-align: left; transition: border-color .15s ease, background .15s ease; }
.overview-tool-grid button:hover, .overview-tool-grid button:focus-visible { border-color: rgba(230, 111, 81, .5); background: rgba(255, 250, 244, .9); outline: none; }
.overview-tool-grid button :deep(svg) { flex: 0 0 auto; }
.overview-tool-grid button span { min-width: 0; }
.overview-tool-grid strong, .overview-tool-grid small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.overview-tool-grid strong { color: var(--text-hi); font-size: 12px; line-height: 18px; }
.overview-tool-grid small { color: var(--text-lo); font-size: 10px; line-height: 15px; }
@media (max-width: 920px) {
  .server-truth-band { grid-template-columns: 1fr auto; }
  .truth-facts { grid-column: 1 / -1; grid-row: 2; }
  .overview-tool-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
@media (max-width: 680px) {
  .overview-live-head, .overview-actions-row { align-items: stretch; flex-direction: column; }
  .server-truth-band { grid-template-columns: 1fr; }
  .truth-facts { grid-column: auto; grid-row: auto; grid-template-columns: 1fr 1fr; }
  .truth-actions { justify-content: stretch; }
  .truth-actions .btn { flex: 1; }
  .overview-metrics { grid-template-columns: 1fr 1fr; }
  .overview-metric:nth-child(2) { border-right: 0; }
  .overview-metric:nth-child(-n+2) { border-bottom: 1px solid var(--glass-border); }
  .overview-tool-grid { grid-template-columns: 1fr; }
  .live-data-warning, .overview-player-strip { align-items: stretch; flex-direction: column; }
  .overview-player-list { justify-content: flex-start; }
  .overview-path { margin: 2px 0 0; white-space: normal; word-break: break-all; }
}
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

/* ====== 配置空白横幅（P1）====== */
.config-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-radius: 10px;
  background: var(--amber-bg, rgba(184, 120, 47, 0.14));
  border: 1px solid rgba(184, 120, 47, 0.32);
  font-size: 13px;
  line-height: 1.5;
  color: var(--palwarm-text-primary, #3f322c);
}
.config-banner :deep(svg) {
  flex: 0 0 16px;
  color: var(--amber, #b8782f);
}
.config-banner-text { flex: 1; min-width: 0; }
.config-banner code {
  font-family: var(--font-mono);
  font-size: 12px;
  background: rgba(116, 88, 72, 0.1);
  padding: 1px 5px;
  border-radius: 5px;
}

/* ====== 向导操作区（StepCard 标题卡 + 下方 glass 操作区）====== */
.sc-block {
  margin-top: 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.sc-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
/* 路径行 */
.sc-path-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  padding: 12px 16px;
  border-radius: var(--r-card, 12px);
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
}
.sc-path-row.ok {
  border-color: rgba(79, 138, 107, 0.4);
  background: var(--green-bg, rgba(79, 138, 107, 0.08));
}
.sc-path-row.fail {
  border-color: rgba(201, 85, 77, 0.4);
  background: var(--red-bg, rgba(201, 85, 77, 0.08));
}
.sc-num {
  width: 24px;
  height: 24px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--primary, #e66f51);
  color: #fff;
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
  font-weight: 700;
  flex: 0 0 24px;
}
.sc-path-row.ok .sc-num,
.sc-launch-card.done .sc-num { background: var(--green, #4f8a6b); }
.sc-path-info {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  flex: 1;
  min-width: 0;
}
.sc-path-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
}
.sc-path-state {
  font-size: 12px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 999px;
}
.sc-path-state.ok { background: var(--green-bg, rgba(79, 138, 107, 0.14)); color: var(--green, #4f8a6b); }
.sc-path-state.todo { background: rgba(116, 88, 72, 0.10); color: var(--text-mid2, #8a7a6e); }
.sc-path-state.fail { background: var(--red-bg, rgba(201, 85, 77, 0.14)); color: var(--red, #c9554d); }
.sc-path-hint {
  font-size: 12px;
  color: var(--red, #c9554d);
  font-weight: 500;
}
.sc-path-detail {
  font-size: 12px;
  color: var(--text-mid2, #8a7a6e);
  font-family: var(--font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}
/* 启动卡片行 — 固定三列一行（老板要 1 2 3 不折成 1 / 2 3） */
.sc-launch-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}
@media (max-width: 720px) {
  .sc-launch-row { grid-template-columns: 1fr; }
}
.sc-launch-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
  border-radius: var(--r-card, 12px);
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
}
.sc-launch-card.done {
  background: var(--green-bg, rgba(79, 138, 107, 0.10));
  border-color: rgba(79, 138, 107, 0.4);
}
.sc-lc-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.sc-lc-state {
  font-size: 12px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 999px;
}
.sc-lc-state.ok { background: var(--green-bg, rgba(79, 138, 107, 0.14)); color: var(--green, #4f8a6b); }
.sc-lc-state.todo { background: rgba(116, 88, 72, 0.10); color: var(--text-mid2, #8a7a6e); }
.sc-lc-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
}
.sc-lc-desc {
  font-size: 12px;
  color: var(--text-mid2, #8a7a6e);
  line-height: 1.5;
  flex: 1;
}
.sc-runtime {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 12px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-mid2, #8a7a6e);
}
.sc-launch-card .btn { justify-content: center; }
/* §2.5 仪表盘关服按钮组（右对齐） */
.dash-proc-actions {
  margin-left: auto;
  display: flex;
  gap: 8px;
}
</style>
