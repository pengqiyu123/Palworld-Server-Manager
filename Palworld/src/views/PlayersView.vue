<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">玩家管理 · 在线列表</div>
        <div class="page-sub">
          在线 {{ players.length }} 人 · 数据每 60 秒自动刷新
          <button class="btn btn-ghost btn-sm" style="margin-left: 8px" @click="onRefresh">手动刷新</button>
        </div>
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
      <div v-else class="players-empty">
        <AppIcon name="players" :size="48" class="ph-icon" />
        <p>当前没有在线玩家</p>
        <span class="empty-hint">玩家进服后将自动出现在列表中</span>
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
import { ref, computed } from 'vue'
import { useServerStore } from '@/stores/server'
import { useToast } from '@/components/ui/useToast'
import type { PlayerInfo } from '@/types/tauri'
import AppIcon from '@/components/ui/AppIcon.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'

const serverStore = useServerStore()
const toast = useToast()

const players = computed(() => serverStore.players)
const announceMsg = ref('')

function formatCoord(v: number): string {
  return v.toFixed(0)
}

async function onRefresh(): Promise<void> {
  try {
    await serverStore.pollOnce()
    toast.info('玩家列表已刷新')
  } catch (e) {
    toast.error(`刷新失败: ${e instanceof Error ? e.message : String(e)}`)
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
