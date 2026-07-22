<template>
  <span class="status-pill" :class="status">
    <span class="s-dot" />
    {{ text }}
  </span>
</template>

<script setup lang="ts">
/** 状态药丸：status 语义色 + text 文案 */
defineProps<{
  status: 'ok' | 'off' | 'block'
  text: string
}>()
</script>
