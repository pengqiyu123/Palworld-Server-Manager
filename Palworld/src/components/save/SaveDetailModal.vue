<template>
  <teleport to="body">
    <div v-if="world" class="sdm-overlay" @click.self="$emit('close')">
      <div class="sdm-dialog" role="dialog" aria-modal="true">
        <div class="sdm-head">
          <span class="sdm-title">{{ world.name }} · 存档信息</span>
          <button class="btn btn-ghost btn-sm" @click="$emit('close')">关闭</button>
        </div>

        <div class="sdm-grid">
          <div class="sdm-item"><span class="sdm-k">来源</span><span class="sdm-v">{{ sourceLabel(world.source) }}</span></div>
          <div class="sdm-item"><span class="sdm-k">世界名</span><span class="sdm-v">{{ world.name }}</span></div>
          <div class="sdm-item"><span class="sdm-k">GUID</span><span class="sdm-v mono">{{ world.guid || '（扁平布局无 GUID 层）' }}</span></div>
          <div class="sdm-item"><span class="sdm-k">玩家数</span><span class="sdm-v">{{ world.player_count }}</span></div>
          <div class="sdm-item"><span class="sdm-k">修改时间</span><span class="sdm-v">{{ world.modified_at || '未知' }}</span></div>
          <div class="sdm-item sdm-item--full"><span class="sdm-k">路径</span><span class="sdm-v mono">{{ world.path }}</span></div>
        </div>

        <div class="sdm-players">
          <div class="op-sub">Level.sav 解析概要（玩家列表）</div>
          <div v-if="loading" class="sm-empty sm-empty--sm">解析中…</div>
          <template v-else-if="summary">
            <div v-if="summary.players.length" class="pp-list-mini">
              <div v-for="p in summary.players" :key="p.guid" class="pp-item-mini">
                <span class="pp-name-mini">{{ p.nickname || '(无名)' }}<em v-if="p.is_host" class="pp-host-mini">主机</em></span>
                <span class="pp-meta-mini">Lv{{ p.level }} · 公会 {{ p.guild_id || '无' }} · 帕鲁 {{ p.pal_count }} · {{ p.last_online }}</span>
              </div>
            </div>
            <div v-else class="sm-empty sm-empty--sm">该世界暂无玩家数据</div>
          </template>
          <div v-else class="sm-empty sm-empty--sm">该世界概要解析失败，可重试刷新检测</div>
        </div>

        <div class="sdm-actions">
          <button
            v-if="world.source !== 'server'"
            class="btn btn-primary btn-sm"
            @click="$emit('migrate', world)"
          >
            迁移到服务器
          </button>
          <button
            v-else
            class="btn btn-primary btn-sm"
            @click="$emit('setBackup', world)"
          >
            设为备份目标
          </button>
        </div>
      </div>
    </div>
  </teleport>
</template>

<script setup lang="ts">
import type { WorldInfo, WorldSummary } from '@/types/tauri'

defineProps<{
  world: WorldInfo | null
  summary: WorldSummary | null
  loading: boolean
}>()

defineEmits<{
  (e: 'close'): void
  (e: 'migrate', world: WorldInfo): void
  (e: 'setBackup', world: WorldInfo): void
}>()

function sourceLabel(s: string): string {
  if (s === 'steam') return 'Steam 单机'
  if (s === 'appdata') return 'AppData 单机'
  if (s === 'server') return '专用服'
  return '本机'
}
</script>

<style scoped>
.sdm-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(46, 36, 30, 0.4);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
}
.sdm-dialog {
  width: min(560px, 100%);
  max-height: 86vh;
  overflow-y: auto;
  padding: 20px 22px;
  border-radius: var(--r-card, 18px);
  background: var(--glass-bg, rgba(255, 252, 247, 0.96));
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.18));
  box-shadow: 0 18px 48px rgba(46, 36, 30, 0.22);
}
.sdm-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
}
.sdm-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
}
.sdm-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 10px 16px;
}
.sdm-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.sdm-item--full {
  grid-column: 1 / -1;
}
.sdm-k {
  font-size: 11px;
  color: var(--text-mid2, #8a7a6e);
}
.sdm-v {
  font-size: 13px;
  color: var(--palwarm-text-primary, #3f322c);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sdm-v.mono {
  font-family: var(--font-mono);
  font-size: 12px;
}
.sdm-players {
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px dashed var(--glass-border, rgba(116, 88, 72, 0.2));
}
.op-sub {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-mid, #77675f);
  margin-bottom: 8px;
}
.sdm-actions {
  margin-top: 18px;
  display: flex;
  justify-content: flex-end;
}
</style>
