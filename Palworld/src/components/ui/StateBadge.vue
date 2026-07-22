<template>
  <span class="state-badge" :class="`state-badge--${type}`">
    <span class="state-badge__dot" />
    <span v-if="text" class="state-badge__text">{{ text }}</span>
  </span>
</template>

<script setup lang="ts">
defineProps<{
  type: 'success' | 'warning' | 'error' | 'info'
  text?: string
}>()
</script>

<style scoped>
.state-badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 500;
  line-height: 1;
}
/* 颜色变体由全局 style.css 提供 */
.state-badge__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  animation: pulse 2s ease-in-out infinite;
}

/* 状态点脉冲动画：opacity 1 → 0.4 → 1 */
@keyframes pulse {
  0% { opacity: 1; }
  50% { opacity: 0.4; }
  100% { opacity: 1; }
}

@media (prefers-reduced-motion: reduce) {
  .state-badge__dot {
    animation: none;
  }
}
</style>
