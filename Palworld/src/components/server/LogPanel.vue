<template>
  <div class="log-panel glass-panel">
    <div class="log-panel__header">
      <button class="log-panel__toggle" @click="collapsed = !collapsed">
        <ChevronDown v-if="!collapsed" :size="16" />
        <ChevronRight v-else :size="16" />
        <span class="log-panel__title">服务器日志</span>
        <span class="log-panel__count">{{ serverStore.logs.length }}</span>
      </button>
      <div class="log-panel__actions">
        <BaseButton variant="secondary" size="sm" @click="handleClear" :disabled="serverStore.logs.length === 0">
          清空
        </BaseButton>
      </div>
    </div>
    <div v-show="!collapsed" class="log-panel__body scroll-container" ref="bodyRef" @scroll="handleScroll">
      <div
        v-for="(line, idx) in serverStore.logs"
        :key="idx"
        class="log-panel__line"
        :class="{ 'log-panel__line--new': newLineIds.has(idx) }"
      >
        <span class="log-panel__line-number">{{ idx + 1 }}</span>
        <span class="log-panel__line-content">{{ line }}</span>
      </div>
      <div v-if="serverStore.logs.length === 0" class="log-panel__empty">
        暂无日志输出
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import { ChevronDown, ChevronRight } from 'lucide-vue-next'
import { useServerStore } from '@/stores/server'
import BaseButton from '@/components/ui/BaseButton.vue'

const serverStore = useServerStore()
const collapsed = ref(false)
const bodyRef = ref<HTMLElement | null>(null)
const autoScroll = ref(true)
const newLineIds = ref<Set<number>>(new Set())

// log 监听已移至 main.ts 全局初始化，此处仅负责渲染
// 监听 logs 变化自动滚动 + 标记新行
watch(() => serverStore.logs.length, async (newLen, oldLen = 0) => {
  // 标记新行
  for (let i = oldLen; i < newLen; i++) {
    newLineIds.value.add(i)
    // 1 秒后移除高亮
    setTimeout(() => {
      newLineIds.value.delete(i)
    }, 1000)
  }
  if (autoScroll.value) {
    await nextTick()
    if (bodyRef.value) {
      bodyRef.value.scrollTop = bodyRef.value.scrollHeight
    }
  }
})

function handleScroll() {
  if (!bodyRef.value) return
  const { scrollTop, scrollHeight, clientHeight } = bodyRef.value
  // 距底部小于 30 像素则视为"在底部"
  autoScroll.value = scrollHeight - scrollTop - clientHeight < 30
}

async function handleClear() {
  await serverStore.clearLogs()
  newLineIds.value.clear()
}
</script>

<style scoped>
.log-panel {
  display: flex;
  flex-direction: column;
  padding: 0;
  overflow: hidden;
}

.log-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--palwarm-glass-edge);
  background: var(--palwarm-glass-soft);
}

.log-panel__toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  color: var(--palwarm-text-primary);
  font-size: 14px;
  font-weight: 500;
  background: none;
  border: none;
  padding: 0;
}

.log-panel__title {
  user-select: none;
}

.log-panel__count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  height: 18px;
  padding: 0 6px;
  border-radius: 9px;
  background: var(--palwarm-primary-soft);
  color: var(--palwarm-primary);
  font-size: 11px;
  font-weight: 600;
}

.log-panel__actions {
  display: flex;
  gap: 8px;
}

.log-panel__body {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
  font-family: var(--palwarm-font-mono, 'Cascadia Code', Consolas, monospace);
  font-size: 12px;
  line-height: 1.5;
  min-height: 200px;
  max-height: 400px;
}

.log-panel__line {
  display: flex;
  padding: 2px 16px;
  transition: background-color 0.3s;
}

.log-panel__line--new {
  background: var(--palwarm-primary-soft);
}

.log-panel__line-number {
  flex-shrink: 0;
  width: 40px;
  margin-right: 12px;
  color: var(--palwarm-text-muted);
  text-align: right;
  user-select: none;
}

.log-panel__line-content {
  flex: 1;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--palwarm-text-primary);
}

.log-panel__empty {
  padding: 32px 16px;
  text-align: center;
  color: var(--palwarm-text-muted);
  font-size: 13px;
}
</style>
