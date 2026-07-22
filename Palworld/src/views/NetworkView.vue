<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">网络与端口 · 联机连通性</div>
        <div class="page-sub">朋友要连你的服，这三个端口必须通。每个卡显示用途、协议、方向和当前状态。</div>
      </div>
      <div class="page-actions">
        <button class="btn btn-ghost" :disabled="checking" @click="onRefresh">刷新检测</button>
      </div>
    </div>

    <!-- 端口三卡 -->
    <div class="ports-row">
      <PortCard
        title="游戏端口 · UDP 8211"
        proto="UDP · 入站(Inbound)"
        desc="玩家游戏客户端连接入口 —— 朋友进服走这里"
        :status="networkStore.firewall.port_8211_open ? 'ok' : 'block'"
        icon-name="game"
        :allowing="addingRules"
        @allow="onAddRules"
      />
      <PortCard
        title="RCON 控制台 · TCP 25575"
        proto="TCP · 入站(Inbound)"
        desc="管理器向服务器发管理员指令的通道"
        :status="networkStore.firewall.port_25575_open ? 'ok' : 'block'"
        icon-name="rcon"
        :allowing="addingRules"
        @allow="onAddRules"
      />
      <PortCard
        title="REST API · TCP 8212"
        proto="TCP · 入站(Inbound)"
        desc="RESTful API 接口 —— 本工具读取服名/FPS/玩家列表走这里"
        :status="networkStore.firewall.port_8212_open ? 'ok' : 'block'"
        icon-name="rest"
        :allowing="addingRules"
        @allow="onAddRules"
      />
    </div>

    <!-- Radmin 5 档分级检测（收官 M1） -->
    <div class="radmin-section">
      <div class="section-head">
        <div class="section-title">Radmin 联机就绪度 · 5 档分级检测</div>
        <button class="btn btn-primary btn-sm" :disabled="launchingRadmin" @click="onLaunchRadmin">
          <AppIcon name="vpn" :size="16" />
          <span>{{ launchingRadmin ? '启动中…' : '启动 Radmin VPN' }}</span>
        </button>
      </div>
      <div class="radmin-grid">
        <RadminReadinessCard
          :readiness="networkStore.readiness"
          :checking="checking"
          @recheck="onRefresh"
          @invoke-action="onInvokeAction"
        />
        <ConnectionCard
          v-if="networkStore.readiness?.level === 'L4'"
          :virtual_ip="networkStore.readiness.virtual_ip"
          @copy="copyText"
        />
      </div>
    </div>

    <!-- F2 · 启动 Radmin VPN 后的加入引导弹窗 -->
    <RadminLaunchModal v-model:visible="radminLaunchVisible" />

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
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useNetworkStore } from '@/stores/network'
import { useOnboardingStore } from '@/stores/onboarding'
import { useToast } from '@/components/ui/useToast'
import { api } from '@/api/tauri'
import AppIcon from '@/components/ui/AppIcon.vue'
import PortCard from '@/components/ui/PortCard.vue'
import RadminReadinessCard from '@/components/ui/RadminReadinessCard.vue'
import ConnectionCard from '@/components/ui/ConnectionCard.vue'
import OnboardingProgress from '@/components/ui/OnboardingProgress.vue'
import RadminLaunchModal from '@/components/ui/RadminLaunchModal.vue'
import type { NextAction } from '@/types/tauri'

const networkStore = useNetworkStore()
const onboardingStore = useOnboardingStore()
const toast = useToast()

const checking = ref(false)
const addingRules = ref(false)
const launchingRadmin = ref(false)
const radminLaunchVisible = ref(false)

onMounted(async () => {
  // 确保配置已加载（S2 派生需要）+ 触发 Radmin 5 档检测
  await onboardingStore.refresh().catch(() => {})
})

/** F2 · 启动 Radmin VPN：拉起应用，成功后弹出加入引导弹窗。 */
async function onLaunchRadmin(): Promise<void> {
  if (launchingRadmin.value) return
  launchingRadmin.value = true
  try {
    const msg = await api.launcher.launchRadminVpn()
    toast.success(msg)
    radminLaunchVisible.value = true
  } catch (e) {
    toast.error(`启动 Radmin VPN 失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    launchingRadmin.value = false
  }
}

async function onRefresh(): Promise<void> {
  checking.value = true
  try {
    await networkStore.checkAll()
    await onboardingStore.refresh().catch(() => {})
    toast.info('网络检测已刷新')
  } catch (e) {
    toast.error(`检测失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    checking.value = false
  }
}

async function onAddRules(): Promise<void> {
  addingRules.value = true
  try {
    const msg = await networkStore.addFirewallRules()
    await networkStore.checkFirewall()
    toast.success(msg)
  } catch (e) {
    toast.error(`放行失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    addingRules.value = false
  }
}

/** RadminReadinessCard 的下一步动作分发 */
function onInvokeAction(action: NextAction): void {
  switch (action.action_type) {
    case 'open_url':
      if (action.payload) window.open(action.payload, '_blank')
      break
    case 'copy_card':
      if (networkStore.readiness?.virtual_ip) {
        copyText(
          `朋友连我帕鲁服：①装 Radmin VPN（radmin-vpn.com）②我拉你进我的虚拟网络 ③进游戏→多人→专用服务器→填 ${networkStore.readiness.virtual_ip}:8211 直连（双方游戏版本需一致）`
        )
      }
      break
    case 'auto_recheck':
      void onRefresh()
      break
    case 'launch_app':
      toast.info('请手动打开 Radmin VPN 客户端并加入/创建虚拟网络')
      break
    case 'show_guide':
      toast.info('在 Radmin VPN 客户端里「创建网络」或「加入网络」，拿到虚拟 IP 后回来重新检测')
      break
    default:
      break
  }
}

/** 复制文本到剪贴板（优先 navigator.clipboard，Tauri 下回退 textarea） */
async function copyText(text: string): Promise<void> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text)
    } else {
      const textarea = document.createElement('textarea')
      textarea.value = text
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand('copy')
      document.body.removeChild(textarea)
    }
    toast.success('连法已复制到剪贴板')
  } catch {
    toast.error('复制失败，请手动复制')
  }
}
</script>

<style scoped>
.radmin-section {
  margin-top: 20px;
}
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}
.section-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
}
.radmin-grid {
  display: grid;
  grid-template-columns: minmax(280px, 1fr) minmax(280px, 1fr);
  gap: 14px;
  align-items: start;
}
.guide-section {
  margin-top: 24px;
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
  background: #4f8a6b;
}
.lg-dot.fail {
  background: #d9534f;
}
.lg-dot.idle {
  background: rgba(0, 0, 0, 0.12);
}
@media (max-width: 720px) {
  .radmin-grid {
    grid-template-columns: 1fr;
  }
}
</style>
