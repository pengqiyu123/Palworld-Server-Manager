<template>
  <div class="tech-editor">
    <div class="te-toolbar">
      <input v-model="query" class="input" placeholder="搜索科技名 / asset…" />
      <span class="te-hint">已选 {{ selected.size }} 项</span>
    </div>

    <div v-if="loading" class="te-empty">加载科技列表中…</div>
    <div v-else-if="!techs.length" class="te-empty">未加载到科技列表</div>
    <div v-else class="te-list">
      <label
        v-for="t in filtered"
        :key="t.asset"
        class="te-item"
        :class="{ active: selected.has(t.asset) }"
      >
        <input
          type="checkbox"
          :checked="selected.has(t.asset)"
          @change="toggle(t.asset)"
        />
        <span class="te-name">{{ t.name }}</span>
        <span v-if="t.tech_type" class="te-type">{{ t.tech_type }}</span>
      </label>
    </div>

    <div class="te-actions">
      <button class="btn btn-primary btn-sm" :disabled="!canApply" @click="applyAdd">
        解锁选中
      </button>
      <button class="btn btn-ghost btn-sm" :disabled="!canApply" @click="applyRemove">
        移除选中
      </button>
    </div>

    <div v-if="msg" class="te-msg" :class="msgType">{{ msg }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useToast } from '@/components/ui/useToast'
import { api } from '@/api/tauri'
import type { TechInfo } from '@/types/tauri'

const props = defineProps<{
  world: string
  playerGuid: string
}>()
const toast = useToast()

const techs = ref<TechInfo[]>([])
const query = ref('')
const selected = ref<Set<string>>(new Set())
const msg = ref('')
const msgType = ref<'ok' | 'err'>('ok')
const loading = ref(false)

const filtered = computed<TechInfo[]>(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return techs.value
  return techs.value.filter(
    (t) => t.name.toLowerCase().includes(q) || t.asset.toLowerCase().includes(q),
  )
})
const canApply = computed(
  () => !!props.world && !!props.playerGuid && selected.value.size > 0,
)

async function load(): Promise<void> {
  loading.value = true
  try {
    techs.value = await api.migration.techList()
  } catch (e) {
    toast.error(`读取科技列表失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    loading.value = false
  }
}

function toggle(asset: string): void {
  const s = new Set(selected.value)
  if (s.has(asset)) {
    s.delete(asset)
  } else {
    s.add(asset)
  }
  selected.value = s
}

async function applyAdd(): Promise<void> {
  await doEdit([...selected.value], [], 'batch')
}
async function applyRemove(): Promise<void> {
  await doEdit([], [...selected.value], 'batch')
}

async function doEdit(add: string[], remove: string[], mode: string): Promise<void> {
  if (!canApply.value) return
  msg.value = ''
  try {
    const res = await api.migration.editTech({
      world: props.world,
      player_guid: props.playerGuid,
      add_assets: add,
      remove_assets: remove,
      mode,
    })
    if (res.ok) {
      msg.value = `科技点已更新（round-trip: ${res.roundtrip_ok ? '通过' : '有警告'}）${
        res.warnings.length ? ' · ' + res.warnings.join('；') : ''
      }`
      msgType.value = 'ok'
      toast.success('科技点已更新')
      selected.value = new Set()
    } else {
      msg.value = '更新失败：' + res.warnings.join('；')
      msgType.value = 'err'
    }
  } catch (e) {
    msg.value = String(e instanceof Error ? e.message : e)
    msgType.value = 'err'
    toast.error(`科技点更新失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

onMounted(load)
</script>

<style scoped>
.te-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;
}
.te-hint {
  font-size: 12px;
  color: var(--text-mid2, #8a7a6e);
}
.te-empty {
  padding: 14px;
  border-radius: 12px;
  background: var(--glass-bg-soft, rgba(255, 250, 244, 0.5));
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
  font-size: 13px;
  color: var(--text-mid2, #8a7a6e);
}
.te-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 260px;
  overflow-y: auto;
}
.te-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 10px;
  border-radius: 8px;
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
  cursor: pointer;
}
.te-item.active {
  border-color: var(--primary, #e66f51);
  background: var(--glass-bg-strong, rgba(255, 250, 244, 0.88));
}
.te-name {
  flex: 1;
  font-size: 13px;
  color: var(--palwarm-text-primary, #3f322c);
}
.te-type {
  font-size: 11px;
  color: var(--text-mid2, #8a7a6e);
  background: rgba(116, 88, 72, 0.1);
  border-radius: 5px;
  padding: 1px 6px;
}
.te-actions {
  display: flex;
  gap: 10px;
  margin-top: 10px;
}
.te-msg {
  margin-top: 8px;
  font-size: 12px;
  padding: 8px 10px;
  border-radius: 8px;
}
.te-msg.ok {
  background: var(--green-bg, rgba(79, 138, 107, 0.14));
  border: 1px solid rgba(79, 138, 107, 0.3);
  color: var(--palwarm-text-primary, #3f322c);
}
.te-msg.err {
  background: var(--amber-bg, rgba(184, 120, 47, 0.14));
  border: 1px solid rgba(184, 120, 47, 0.3);
  color: var(--palwarm-text-primary, #3f322c);
}
</style>
