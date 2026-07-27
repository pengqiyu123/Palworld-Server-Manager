<template>
  <aside class="sidebar">
    <!-- 状态卡：读取真实服务器状态 -->
    <div class="status-card">
      <span class="status-dot" :class="{ online: isOnline }" />
      <div class="status-text">
        <span class="status-name">{{ serverName }}</span>
        <span class="status-state">{{ statusState }}</span>
      </div>
    </div>

    <!-- 运行时指标卡（仅仪表盘模式显示） -->
    <div v-if="hasServerProcess" class="sidebar-stats">
      <div class="ss-item">
        <span class="ss-label">在线</span>
        <span class="ss-value">{{ playersSummary }}</span>
      </div>
      <div v-if="serverStore.playersState === 'error'" class="ss-live-error">联机数据读取失败</div>
      <div v-if="maxPlayersPendingRestart" class="ss-config-note">
        配置为 {{ configuredMaxPlayers }}，重启后生效
      </div>
      <div class="ss-item">
        <span class="ss-label">FPS</span>
        <span class="ss-value">{{ fps }}</span>
      </div>
    </div>

    <div class="nav-label">导航</div>

    <router-link
      v-for="item in navItems"
      :key="item.path"
      :to="item.path"
      class="nav-item"
      active-class="active"
      :title="item.label"
    >
      <AppIcon :name="item.icon" :size="18" />
      <span class="nav-item-label">{{ item.label }}</span>
    </router-link>

    <div class="nav-spacer" />
    <div class="nav-divider" />
    <router-link
      to="/settings"
      class="nav-item"
      active-class="active"
    >
      <AppIcon name="settings" :size="18" />
      <span class="nav-item-label">设置</span>
    </router-link>
    <div class="nav-version">v1.0.0</div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUiStore } from '@/stores/ui'
import { useServerStore } from '@/stores/server'
import { useConfigStore } from '@/stores/config'
import AppIcon from '@/components/ui/AppIcon.vue'

const uiStore = useUiStore()
const serverStore = useServerStore()
const configStore = useConfigStore()

// 主导航（配置文件备份仍由配置页负责，世界备份统一进入世界存档页）
const navItems = [
  { path: '/overview', label: '概览', icon: 'overview' },
  { path: '/config', label: '配置', icon: 'config' },
  { path: '/players', label: '玩家管理', icon: 'players' },
  { path: '/rcon', label: '服务器控制台', icon: 'rcon' },
  { path: '/logs', label: '实时日志', icon: 'logs' },
  { path: '/saves', label: '世界存档', icon: 'save' },
  { path: '/migrate', label: '世界与角色迁移', icon: 'migration' },
  { path: '/modifier', label: '修改器', icon: 'modifier' },
]

const hasServerProcess = computed(() => serverStore.status.running)
const isOnline = computed(() => serverStore.status.ready)

const serverName = computed(() => {
  return serverStore.serverInfo?.servername || configStore.config.ServerName?.replace(/^"|"$/g, '') || '我的 Palworld 服务器'
})

const configuredMaxPlayers = computed(() => {
  const value = Number.parseInt(configStore.config.ServerPlayerMaxNum ?? '', 10)
  return Number.isFinite(value) && value > 0 ? value : null
})
const maxPlayers = computed(() => serverStore.serverMetrics?.maxplayernum ?? configuredMaxPlayers.value ?? '—')
const playersSummary = computed(() => {
  if (serverStore.playersState === 'error' && !serverStore.serverMetrics) return '读取失败'
  const current = serverStore.serverMetrics?.currentplayernum ??
    (serverStore.playersState === 'live' ? serverStore.players.length : 0)
  return `${current}/${maxPlayers.value}`
})
const maxPlayersPendingRestart = computed(() => {
  const runtime = serverStore.serverMetrics?.maxplayernum
  return runtime !== undefined && configuredMaxPlayers.value !== null && runtime !== configuredMaxPlayers.value
})
const fps = computed(() => {
  const f = serverStore.serverMetrics?.serverfps
  return f !== null && f !== undefined ? (Number.isInteger(f) ? String(f) : f.toFixed(0)) : '—'
})

// 侧边状态卡文案：随运行状态变化
const statusState = computed(() => {
  if (isOnline.value) return serverStore.status.managed_by_app ? '管理器启动 · 在线' : '外部启动 · 在线'
  if (hasServerProcess.value) return '进程已启动 · 等待端口'
  if (uiStore.wizard.detected) return uiStore.wizard.manual ? '已配置（手动）· 待启动' : '已配置 · 待启动'
  return '尚未配置 · 待设置'
})
</script>

<style scoped>
.sidebar-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 12px;
}
.ss-config-note {
  flex: 0 0 100%;
  color: var(--amber, #9b5c14);
  font-size: 10px;
  line-height: 15px;
}
.ss-live-error {
  flex: 0 0 100%;
  color: var(--red, #c9554d);
  font-size: 10px;
  line-height: 15px;
}
.ss-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgba(79, 138, 107, 0.08);
  border: 1px solid rgba(79, 138, 107, 0.15);
}
.ss-label {
  font-size: 11px;
  color: var(--text-mid2, #a39383);
}
.ss-value {
  font-size: 16px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
}
</style>
