<template>
  <div class="preset-selector glass-panel">
    <div class="preset-selector__header">
      <label class="form-label">配置预设</label>
      <select
        class="form-select preset-selector__select"
        :value="modelValue"
        @change="handleChange"
      >
        <option value="" disabled>请选择预设...</option>
        <option v-for="preset in presets" :key="preset.name" :value="preset.name">
          {{ preset.name }}
        </option>
      </select>
    </div>

    <div v-if="selectedPreset" class="preset-selector__body">
      <p class="preset-selector__description">{{ selectedPreset.description }}</p>
      <div v-if="selectedPreset.key_params.length" class="preset-selector__params">
        <div class="preset-selector__params-title">关键参数</div>
        <ul class="preset-selector__param-list">
          <li
            v-for="[key, value] in selectedPreset.key_params"
            :key="key"
            class="preset-selector__param"
          >
            <span class="preset-selector__param-key">{{ key }}</span>
            <span class="preset-selector__param-value">{{ value }}</span>
          </li>
        </ul>
      </div>
    </div>

    <div class="preset-selector__footer">
      <button
        class="btn btn-primary btn-md"
        :disabled="!modelValue"
        @click="handleApply"
      >
        应用预设
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { PresetMeta } from '@/types/tauri'

const props = defineProps<{
  modelValue: string
  presets: PresetMeta[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  apply: [name: string]
}>()

// 当前选中的预设元信息
const selectedPreset = computed(() =>
  props.presets.find(p => p.name === props.modelValue) ?? null,
)

function handleChange(event: Event) {
  const target = event.target as HTMLSelectElement
  emit('update:modelValue', target.value)
}

function handleApply() {
  if (!props.modelValue) return
  emit('apply', props.modelValue)
}
</script>

<style scoped>
.preset-selector {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px;
}

.preset-selector__header {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.preset-selector__select {
  min-height: 38px;
}

.preset-selector__body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  background: var(--palwarm-muted);
  border: 1px solid var(--palwarm-glass-edge);
  border-radius: var(--palwarm-radius-sm);
}

.preset-selector__description {
  margin: 0;
  font-size: 13px;
  line-height: 1.55;
  color: var(--palwarm-text-secondary);
}

.preset-selector__params-title {
  font-size: 11px;
  font-weight: 760;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--palwarm-text-muted);
}

.preset-selector__param-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.preset-selector__param {
  display: flex;
  gap: 8px;
  font-size: 12px;
  font-family: var(--palwarm-font-mono);
}

.preset-selector__param-key {
  color: var(--palwarm-text-primary);
  font-weight: 600;
}

.preset-selector__param-value {
  color: var(--palwarm-primary);
}

.preset-selector__footer {
  display: flex;
  justify-content: flex-end;
}
</style>
