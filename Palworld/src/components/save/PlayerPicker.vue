<template>
  <div class="player-picker">
    <div class="pp-toolbar">
      <button class="btn btn-ghost btn-sm" :disabled="loading || !worldName" @click="load">
        刷新玩家
      </button>
      <span v-if="players.length" class="pp-hint">共 {{ players.length }} 名玩家 · 已选 {{ modelValue.length }}</span>
    </div>

    <div v-if="loading" class="pp-empty">加载中…</div>
    <div v-else-if="!worldName" class="pp-empty">请先选择世界</div>
    <div v-else-if="!players.length" class="pp-empty">该世界暂无玩家数据</div>
    <div v-else class="pp-list">
      <label
        v-for="p in players"
        :key="p.guid"
        class="pp-item"
        :class="{ active: modelValue.includes(p.guid) }"
      >
        <input
          type="checkbox"
          :checked="modelValue.includes(p.guid)"
          @change="toggle(p.guid)"
        />
        <div class="pp-info">
          <span class="pp-name">
            {{ p.nickname || '(无名)' }}
            <em v-if="p.is_host" class="pp-host">主机</em>
          </span>
          <span class="pp-meta">
            Lv{{ p.level }} · 公会 {{ guildName(p.guild_id) }} · 帕鲁 {{ p.pal_count }} · {{ p.last_online }}
          </span>
        </div>
        <span class="pp-guid">{{ p.guid }}</span>
      </label>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { api } from '@/api/tauri'
import type { PlayerEntry, GuildEntry } from '@/types/tauri'

const props = defineProps<{
  worldName: string
  modelValue: string[]
}>()
const emit = defineEmits<{
  (e: 'update:modelValue', v: string[]): void
}>()

const loading = ref(false)
const players = ref<PlayerEntry[]>([])
const guilds = ref<GuildEntry[]>([])

async function load(): Promise<void> {
  if (!props.worldName) return
  loading.value = true
  try {
    const res = await api.migration.worldSummary(props.worldName)
    players.value = res.players
    guilds.value = res.guilds
  } catch {
    players.value = []
    guilds.value = []
  } finally {
    loading.value = false
  }
}

function toggle(guid: string): void {
  const set = new Set(props.modelValue)
  if (set.has(guid)) {
    set.delete(guid)
  } else {
    set.add(guid)
  }
  emit('update:modelValue', [...set])
}

function guildName(id: string | null): string {
  if (!id) return '无'
  const g = guilds.value.find((x) => x.guild_id === id)
  return g ? g.name || id : id
}

onMounted(load)
watch(() => props.worldName, () => {
  players.value = []
  guilds.value = []
  emit('update:modelValue', [])
})
</script>

<style scoped>
.pp-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;
}
.pp-hint {
  font-size: 12px;
  color: var(--text-mid2, #8a7a6e);
}
.pp-empty {
  padding: 14px;
  border-radius: 12px;
  background: var(--glass-bg-soft, rgba(255, 250, 244, 0.5));
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
  font-size: 13px;
  color: var(--text-mid2, #8a7a6e);
}
.pp-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 280px;
  overflow-y: auto;
}
.pp-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: 10px;
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
  cursor: pointer;
}
.pp-item.active {
  border-color: var(--primary, #e66f51);
  background: var(--glass-bg-strong, rgba(255, 250, 244, 0.88));
}
.pp-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}
.pp-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
}
.pp-host {
  font-style: normal;
  font-size: 10px;
  color: #fff;
  background: var(--primary, #e66f51);
  border-radius: 4px;
  padding: 0 5px;
  margin-left: 6px;
}
.pp-meta {
  font-size: 11px;
  color: var(--text-mid2, #8a7a6e);
}
.pp-guid {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-mid2, #8a7a6e);
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
