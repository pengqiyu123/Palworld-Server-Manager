import { defineStore } from 'pinia'
import { ref, computed, watch } from 'vue'
import { useSettingsStore } from '@/stores/settings'
import { useConfigStore } from '@/stores/config'
import { useServerStore } from '@/stores/server'
import { useNetworkStore } from '@/stores/network'
import type { StepId, OnboardingStepState } from '@/types/tauri'

/**
 * 联机流程 7 步派生 store（收官 M2）。
 *
 * 设计要点：
 * - 全部状态由底层 store（settings/config/server/network）派生，**不允许**外部手动 setStatus，避免漂移。
 * - 不持久化：状态是实时的，页面刷新即重算。
 * - s7（朋友入服）首次 players>=2 触发一次性 onSuccess（幂等锁，详见审计项⑤）。
 */
export const useOnboardingStore = defineStore('onboarding', () => {
  const settingsStore = useSettingsStore()
  const configStore = useConfigStore()
  const serverStore = useServerStore()
  const networkStore = useNetworkStore()

  // 7 步状态（全派生）
  const steps = computed<Record<StepId, OnboardingStepState>>(() => {
    const cfg = configStore.config
    const cfgLoaded = Object.keys(cfg).length > 0

    // S1 · 检测服务器路径
    const s1: OnboardingStepState = settingsStore.settings.server_path
      ? { status: 'pass' }
      : { status: 'idle' }

    // S2 · 配置就绪（RESTAPIEnabled / AdminPassword）
    let s2: OnboardingStepState
    if (!cfgLoaded) {
      s2 = { status: 'idle' }
    } else {
      const rest =
        (cfg['RESTAPIEnabled'] ?? '').toString().trim().toLowerCase() === 'true'
      const pw = (cfg['AdminPassword'] ?? '').toString().trim()
      s2 =
        rest && pw
          ? { status: 'pass' }
          : {
              status: 'fail',
              reason:
                '配置不完整：需 RESTAPIEnabled=True、AdminPassword 非空',
            }
    }

    // S3 · 服务器运行（进程 running + REST info 首次成功）
    const s3: OnboardingStepState =
      serverStore.status.running && serverStore.serverInfo !== null
        ? { status: 'pass' }
        : { status: 'idle' }

    // S4 · 游戏端口放行。REST 只在本机访问，不应公开管理端口。
    const s4: OnboardingStepState =
      networkStore.firewall.port_8211_open
        ? { status: 'pass' }
        : { status: 'idle' }

    // S5 · Radmin 5 档就绪（L4 才 pass）
    const r = networkStore.readiness
    let s5: OnboardingStepState
    if (!r) {
      s5 = { status: 'idle' }
    } else if (r.level === 'L4') {
      s5 = { status: 'pass', action: r.next_action }
    } else {
      s5 = { status: 'fail', reason: r.reason, action: r.next_action }
    }

    // S6 · 生成连法卡（S1-S5 全绿）
    const allPre =
      s1.status === 'pass' &&
      s2.status === 'pass' &&
      s3.status === 'pass' &&
      s4.status === 'pass' &&
      s5.status === 'pass'
    const s6: OnboardingStepState = allPre ? { status: 'pass' } : { status: 'idle' }

    // S7 · 朋友入服（players >= 2，即 D1 验收标准）
    const s7: OnboardingStepState =
      serverStore.players.length >= 2 ? { status: 'pass' } : { status: 'idle' }

    return { s1, s2, s3, s4, s5, s6, s7 }
  })

  /** 连法卡片用的虚拟 IP（S6/S7 复制时取 readiness 的虚拟 IP） */
  const connectionVirtualIp = computed(
    () => networkStore.readiness?.virtual_ip ?? settingsStore.settings.server_path
  )

  // ==================== 一次性成功事件（幂等锁 · 审计项⑤） ====================
  const successFired = ref(false)
  let successCallback: (() => void) | null = null

  /** 注册 S7 达成时的一次性回调（首次 players>=2 触发后置 true，永不再触发） */
  function onSuccess(cb: () => void): void {
    successCallback = cb
  }

  /** 重置成功锁（一般用于重新开局测试） */
  function resetSuccess(): void {
    successFired.value = false
    successCallback = null
  }

  watch(
    () => steps.value.s7.status,
    (status) => {
      if (status === 'pass' && !successFired.value) {
        successFired.value = true
        successCallback?.()
      }
    }
  )

  // ==================== 触发底层检测 ====================
  async function ensureConfigLoaded(): Promise<void> {
    if (Object.keys(configStore.config).length === 0) {
      const path = settingsStore.settings.config_path
      if (path) {
        try {
          await configStore.load(path)
        } catch (e) {
          console.warn('加载配置失败:', e)
        }
      }
    }
  }

  /** 刷新派生所需的底层状态（配置 + Radmin 就绪度） */
  async function refresh(): Promise<void> {
    await Promise.all([ensureConfigLoaded(), networkStore.checkReadiness()])
  }

  return {
    steps,
    connectionVirtualIp,
    onSuccess,
    resetSuccess,
    refresh,
  }
})
