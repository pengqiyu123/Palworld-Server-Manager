<template>
  <div class="diagnostic-report">
    <div v-if="sortedItems.length === 0" class="diagnostic-report__empty">
      暂无诊断结果
    </div>
    <div
      v-for="item in sortedItems"
      :key="item.key"
      class="diagnostic-card"
      :class="`diagnostic-card--${item.status}`"
    >
      <button
        class="diagnostic-card__header"
        :aria-expanded="expandedKeys.has(item.key) ? 'true' : 'false'"
        @click="toggle(item.key)"
      >
        <span
          class="diagnostic-card__icon"
          :class="`diagnostic-card__icon--${item.status}`"
        >
          {{ iconFor(item.status) }}
        </span>
        <span class="diagnostic-card__title">{{ item.title }}</span>
        <span
          class="diagnostic-card__badge state-badge"
          :class="`state-badge--${item.status}`"
        >
          {{ labelFor(item.status) }}
        </span>
        <span class="diagnostic-card__chevron">
          {{ expandedKeys.has(item.key) ? '▾' : '▸' }}
        </span>
      </button>

      <div v-if="expandedKeys.has(item.key)" class="diagnostic-card__body">
        <div v-if="item.detail" class="diagnostic-card__section">
          <div class="diagnostic-card__section-label">详情</div>
          <div class="diagnostic-card__section-text">{{ item.detail }}</div>
        </div>
        <div v-if="item.suggestion" class="diagnostic-card__section">
          <div class="diagnostic-card__section-label">修复建议</div>
          <div class="diagnostic-card__section-text">{{ item.suggestion }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script lang="ts">
// 诊断项类型（前端独立类型，不对应 Rust struct）
export interface DiagnosticItem {
  key: string
  title: string
  status: 'ok' | 'warn' | 'error'
  detail?: string
  suggestion?: string
}
</script>

<script setup lang="ts">
import { ref, computed } from 'vue'

const props = defineProps<{
  items: DiagnosticItem[]
}>()

// 展开状态集合
const expandedKeys = ref<Set<string>>(new Set())

// 状态优先级：error 优先 → warn 次之 → ok 最后
const statusOrder: Record<DiagnosticItem['status'], number> = {
  error: 0,
  warn: 1,
  ok: 2,
}

// 排序后的诊断项
const sortedItems = computed(() =>
  [...props.items].sort((a, b) => statusOrder[a.status] - statusOrder[b.status]),
)

function toggle(key: string) {
  if (expandedKeys.value.has(key)) {
    expandedKeys.value.delete(key)
  } else {
    expandedKeys.value.add(key)
  }
}

function iconFor(status: DiagnosticItem['status']): string {
  return { ok: '✓', warn: '!', error: '✕' }[status]
}

function labelFor(status: DiagnosticItem['status']): string {
  return { ok: '正常', warn: '警告', error: '错误' }[status]
}
</script>

<style scoped>
.diagnostic-report {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.diagnostic-report__empty {
  padding: 32px;
  text-align: center;
  font-size: 13px;
  color: var(--palwarm-text-muted);
}

.diagnostic-card {
  background: var(--palwarm-glass-soft);
  border: 1px solid var(--palwarm-glass-edge);
  border-radius: var(--palwarm-radius-sm);
  overflow: hidden;
}

/* 左侧状态竖线，颜色由状态决定 */
.diagnostic-card--error {
  border-left: 3px solid var(--palwarm-state-error);
}

.diagnostic-card--warn {
  border-left: 3px solid var(--palwarm-state-warning);
}

.diagnostic-card--ok {
  border-left: 3px solid var(--palwarm-state-success);
}

.diagnostic-card__header {
  display: flex;
  width: 100%;
  align-items: center;
  padding: 12px 14px;
  gap: 10px;
  background: transparent;
  border: none;
  text-align: left;
  cursor: pointer;
  font: inherit;
}

.diagnostic-card__header:hover {
  background: var(--palwarm-muted);
}

.diagnostic-card__icon {
  display: inline-flex;
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-size: 12px;
  font-weight: 700;
  color: var(--palwarm-primary-foreground);
}

.diagnostic-card__icon--ok {
  background: var(--palwarm-state-success);
}

.diagnostic-card__icon--warn {
  background: var(--palwarm-state-warning);
}

.diagnostic-card__icon--error {
  background: var(--palwarm-state-error);
}

.diagnostic-card__title {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--palwarm-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.diagnostic-card__badge {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 720;
}

.diagnostic-card__chevron {
  color: var(--palwarm-text-muted);
  font-size: 14px;
}

.diagnostic-card__body {
  padding: 0 14px 14px 46px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.diagnostic-card__section-label {
  font-size: 11px;
  font-weight: 760;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--palwarm-text-muted);
  margin-bottom: 4px;
}

.diagnostic-card__section-text {
  font-size: 12px;
  line-height: 1.55;
  color: var(--palwarm-text-secondary);
  white-space: pre-wrap;
}
</style>
