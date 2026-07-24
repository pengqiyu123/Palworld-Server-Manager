<template>
  <aside class="sidebar">
    <!-- 状态卡：读取真实服务器状态 -->
    <div class="status-card">
      <span class="status-dot" :class="{ online: isRunning }" />
      <div class="status-text">
        <span class="status-name">{{ serverName }}</span>
        <span class="status-state">{{ statusState }}</span>
      </div>
    </div>

    <!-- 运行时指标卡（仅仪表盘模式显示） -->
    <div v-if="isRunning" class="sidebar-stats">
      <div class="ss-item">
        <span class="ss-label">在线</span>
        <span class="ss-value">{{ currentPlayers }}/{{ maxPlayers }}</span>
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
    >
      <AppIcon :name="item.icon" :size="18" />
      <span>{{ item.label }}</span>
    </router-link>

    <div class="nav-spacer" />
    <div class="nav-divider" />
    <router-link
      to="/settings"
      class="nav-item"
      active-class="active"
    >
      <AppIcon name="settings" :size="18" />
      <span>设置</span>
    </router-link>
    <div class="nav-version">v2.4.0</div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUiStore } from '@/stores/ui'
import { useServerStore } from '@/stores/server'
import AppIcon from '@/components/ui/AppIcon.vue'

const uiStore = useUiStore()
const serverStore = useServerStore()

// 主导航（按开服真实手感排序：概览→配置→玩家管理→RCON→实时日志→本地存档→数据迁移→配置备份）
const navItems = [
  { path: '/overview', label: '概览', icon: 'overview' },
  { path: '/config', label: '配置', icon: 'config' },
  { path: '/players', label: '玩家管理', icon: 'players' },
  { path: '/rcon', label: 'RCON 控制台', icon: 'rcon' },
  { path: '/logs', label: '实时日志', icon: 'logs' },
  { path: '/saves', label: '本地存档', icon: 'save' },
  { path: '/migrate', label: '数据迁移', icon: 'migration' },
  { path: '/backup', label: '配置备份', icon: 'backup' },
]

const isRunning = computed(() => serverStore.status.running)

const serverName = computed(() => {
  return serverStore.serverInfo?.servername || '我的 Palworld 服务器'
})

const currentPlayers = computed(() => serverStore.serverMetrics?.currentplayernum ?? 0)
const maxPlayers = computed(() => serverStore.serverMetrics?.maxplayernum ?? 32)
const fps = computed(() => {
  const f = serverStore.serverMetrics?.serverfps
  return f !== null && f !== undefined ? (Number.isInteger(f) ? String(f) : f.toFixed(0)) : '—'
})

// 侧边状态卡文案：随运行状态变化
const statusState = computed(() => {
  if (isRunning.value) return '运行中 · 在线'
  if (uiStore.wizard.detected) return uiStore.wizard.manual ? '已配置（手动）· 待启动' : '已配置 · 待启动'
  return '尚未配置 · 待设置'
})
</script>

<style scoped>
.sidebar-stats {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
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
