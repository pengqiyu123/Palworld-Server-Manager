<template>
  <div class="subset-selector">
    <label v-for="key in keys" :key="key" class="ss-item">
      <input
        type="checkbox"
        :checked="modelValue[key]"
        @change="onToggle(key, ($event.target as HTMLInputElement).checked)"
      />
      <span class="ss-label">{{ labels[key] }}</span>
    </label>
  </div>
</template>

<script setup lang="ts">
import type { TransferSubset } from '@/types/tauri'

const props = defineProps<{ modelValue: TransferSubset }>()
const emit = defineEmits<{
  (e: 'update:modelValue', v: TransferSubset): void
}>()

const keys: (keyof TransferSubset)[] = [
  'character',
  'guild',
  'tech',
  'inventory',
  'pals',
  'appearance',
]
const labels: Record<keyof TransferSubset, string> = {
  character: '主角',
  guild: '工会',
  tech: '科技点',
  inventory: '背包',
  pals: '帕鲁',
  appearance: '外观',
}

function onToggle(key: keyof TransferSubset, val: boolean): void {
  emit('update:modelValue', { ...props.modelValue, [key]: val })
}
</script>

<style scoped>
.subset-selector {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
}
.ss-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--text-mid, #77675f);
  cursor: pointer;
}
.ss-label {
  user-select: none;
}
</style>
