<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">玩家管理 · 在线列表</div>
        <div class="page-sub">
          <template v-if="serverStore.playersState === 'live'">
            在线 {{ players.length }} 人 · 每 3 秒自动刷新 · {{ playersUpdatedText }}
          </template>
          <template v-else-if="serverStore.playersState === 'loading'">正在读取在线玩家…</template>
          <template v-else-if="serverStore.playersState === 'error'">在线名单读取失败</template>
          <template v-else>服务器离线，在线名单未读取</template>
          <button
            class="btn btn-ghost btn-sm"
            style="margin-left: 8px"
            :disabled="manualRefreshing"
            @click="onRefresh"
          >
            {{ manualRefreshing ? '刷新中…' : '立即刷新' }}
          </button>
        </div>
      </div>
    </div>

    <div v-if="serverStore.playersState === 'error'" class="players-live-error" role="alert">
      <AppIcon name="info" :size="18" />
      <div>
        <strong>无法读取服务器在线数据</strong>
        <p>{{ serverStore.playersError }}</p>
        <span>请检查配置中的管理员密码是否与当前服务器实际使用的密码一致，然后重启服务器。</span>
      </div>
    </div>

    <!-- 广播输入 -->
    <div class="announce-row">
      <input
        v-model="announceMsg"
        type="text"
        class="announce-input"
        placeholder="输入全服广播消息…"
        @keydown.enter="onAnnounce"
      />
      <button class="btn btn-primary" :disabled="!announceMsg.trim()" @click="onAnnounce">
        发送广播
      </button>
    </div>

    <!-- 玩家表 -->
    <div class="players-table-wrap">
      <table v-if="players.length > 0" class="players-table">
        <thead>
          <tr>
            <th>昵称</th>
            <th>Steam ID</th>
            <th>等级</th>
            <th>Ping</th>
            <th>坐标 (X, Y)</th>
            <th class="th-action">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in players" :key="p.userId">
            <td class="td-name">{{ p.name || '(未命名)' }}</td>
            <td class="td-uid mono">{{ p.userId }}</td>
            <td>{{ p.level }}</td>
            <td>{{ p.ping.toFixed(0) }} ms</td>
            <td class="td-coord mono">{{ formatCoord(p.location_x) }}, {{ formatCoord(p.location_y) }}</td>
            <td class="td-action">
              <button class="btn btn-ghost btn-sm" @click="onKick(p)">踢出</button>
              <button class="btn btn-danger btn-sm" @click="onBan(p)">封禁</button>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-else-if="serverStore.playersState === 'live'" class="players-empty">
        <AppIcon name="players" :size="48" class="ph-icon" />
        <p>当前没有在线玩家</p>
        <span class="empty-hint">玩家进服后将自动出现在列表中</span>
      </div>
      <div v-else class="players-empty">
        <AppIcon name="players" :size="48" class="ph-icon" />
        <p>{{ serverStore.playersState === 'loading' ? '正在读取在线名单' : '暂无可显示的在线数据' }}</p>
        <span class="empty-hint">{{ serverStore.playersState === 'error' ? '读取失败不等于无人在线' : '服务器就绪后会自动刷新' }}</span>
      </div>
    </div>

    <!-- 确认弹窗 -->
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
import { useServerStore } from '@/stores/server'
import { useToast } from '@/components/ui/useToast'
import type { PlayerInfo } from '@/types/tauri'
import AppIcon from '@/components/ui/AppIcon.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'

const serverStore = useServerStore()
const toast = useToast()

const players = computed(() => serverStore.players)
const announceMsg = ref('')
const manualRefreshing = ref(false)
const playersUpdatedText = computed(() => {
  const updatedAt = serverStore.playersLastUpdatedAt
  return updatedAt
    ? `${updatedAt.toLocaleTimeString('zh-CN', { hour12: false })} 已同步`
    : '等待首次同步'
})

onMounted(() => {
  void serverStore.pollOnce()
})

function formatCoord(v: number): string {
  return v.toFixed(0)
}

async function onRefresh(): Promise<void> {
  manualRefreshing.value = true
  try {
    const outcome = await serverStore.pollOnce()
    if (outcome === 'error' || serverStore.playersState === 'error') {
      toast.error(`刷新失败: ${serverStore.playersError}`)
    } else if (outcome === 'updated') {
      toast.info('玩家列表已刷新')
    } else {
      toast.info('服务器尚未就绪，未刷新玩家列表')
    }
  } catch (e) {
    toast.error(`刷新失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    manualRefreshing.value = false
  }
}

async function onAnnounce(): Promise<void> {
  const msg = announceMsg.value.trim()
  if (!msg) return
  try {
    await serverStore.announcePlayer(msg)
    toast.success('广播已发送')
    announceMsg.value = ''
  } catch (e) {
    toast.error(`广播失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

// ====== 踢人/封人确认弹窗 ======
const confirmVisible = ref(false)
const confirmTitle = ref('')
const confirmMessage = ref('')
const confirmDanger = ref(false)
let confirmAction: (() => Promise<void>) | null = null

function onKick(player: PlayerInfo): void {
  confirmTitle.value = '确认踢出玩家'
  confirmMessage.value = `确定要踢出玩家「${player.name || player.userId}」吗？该玩家将立即被移出服务器，但可以重新加入。`
  confirmDanger.value = false
  confirmAction = async () => {
    try {
      await serverStore.kickPlayer(player.userId)
      toast.success(`已踢出 ${player.name || player.userId}`)
      await serverStore.pollOnce()
    } catch (e) {
      toast.error(`踢出失败: ${e instanceof Error ? e.message : String(e)}`)
    }
  }
  confirmVisible.value = true
}

function onBan(player: PlayerInfo): void {
  confirmTitle.value = '确认封禁玩家'
  confirmMessage.value = `确定要封禁玩家「${player.name || player.userId}」吗？该玩家将被移出服务器且无法再次加入。`
  confirmDanger.value = true
  confirmAction = async () => {
    try {
      await serverStore.banPlayer(player.userId)
      toast.success(`已封禁 ${player.name || player.userId}`)
      await serverStore.pollOnce()
    } catch (e) {
      toast.error(`封禁失败: ${e instanceof Error ? e.message : String(e)}`)
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
.announce-row {
  display: flex;
  gap: 10px;
  margin-bottom: 16px;
}
.players-live-error {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  margin-bottom: 14px;
  padding: 12px 14px;
  border: 1px solid rgba(201, 85, 77, 0.35);
  border-radius: 8px;
  background: rgba(201, 85, 77, 0.08);
  color: var(--red, #c9554d);
}
.players-live-error strong,
.players-live-error p,
.players-live-error span { display: block; }
.players-live-error p { margin: 2px 0; font-size: 12px; }
.players-live-error span { color: var(--text-mid2, #8a7a6e); font-size: 11px; }
.announce-input {
  flex: 1;
  padding: 8px 14px;
  border-radius: 8px;
  border: 1px solid var(--palwarm-border, #e8ddd0);
  background: var(--palwarm-surface, #faf6f0);
  color: var(--palwarm-text-primary, #3f322c);
  font-size: 14px;
  outline: none;
}
.announce-input:focus {
  border-color: var(--palwarm-accent, #e66f51);
}
.players-table-wrap {
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid var(--palwarm-border, #e8ddd0);
  background: var(--palwarm-surface, #faf6f0);
}
.players-table {
  width: 100%;
  border-collapse: collapse;
}
.players-table th {
  padding: 10px 14px;
  text-align: left;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-mid2, #a39383);
  background: rgba(0, 0, 0, 0.02);
  border-bottom: 1px solid var(--palwarm-border, #e8ddd0);
}
.players-table td {
  padding: 10px 14px;
  font-size: 13px;
  color: var(--palwarm-text-primary, #3f322c);
  border-bottom: 1px solid rgba(232, 221, 208, 0.5);
}
.players-table tbody tr:last-child td {
  border-bottom: none;
}
.players-table tbody tr:hover {
  background: rgba(230, 111, 81, 0.04);
}
.td-name {
  font-weight: 600;
}
.td-uid {
  font-size: 12px;
  color: var(--text-mid2, #a39383);
}
.td-action {
  display: flex;
  gap: 6px;
}
.th-action {
  text-align: right;
}
.mono {
  font-family: 'JetBrains Mono', monospace;
}
.players-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 48px 20px;
}
.players-empty p {
  font-size: 15px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
  margin: 0;
}
.empty-hint {
  font-size: 13px;
  color: var(--text-mid2, #a39383);
}
.btn-sm {
  padding: 4px 10px;
  font-size: 12px;
}
</style>
