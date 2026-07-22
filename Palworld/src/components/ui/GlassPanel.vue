<template>
  <div
    class="glass-panel"
    :class="{ 'glass-panel--elevated': elevated, 'glass-panel--no-padding': !padding }"
    :style="radiusStyle"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  radius?: 'sm' | 'md' | 'lg' | 'panel'
  elevated?: boolean
  padding?: boolean
}>(), {
  radius: 'panel',
  elevated: false,
  padding: true,
})

const radiusStyle = computed(() => {
  const map = {
    sm: 'var(--palwarm-radius-sm)',
    md: 'var(--palwarm-radius-md)',
    lg: 'var(--palwarm-radius-lg)',
    panel: 'var(--palwarm-radius-panel)',
  }
  return { borderRadius: map[props.radius] }
})
</script>

<style scoped>
/* 对照 palworld-warm-glass-preview/pages/dashboard.html 校准视觉参数 */
.glass-panel {
  background: var(--palwarm-card);
  border: 1px solid var(--palwarm-border);
  box-shadow: var(--palwarm-static-shadow);
  backdrop-filter: blur(24px) saturate(145%);
  -webkit-backdrop-filter: blur(24px) saturate(145%);
}

.glass-panel--no-padding {
  padding: 0;
}
</style>
