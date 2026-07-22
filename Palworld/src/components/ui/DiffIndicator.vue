<template>
  <span
    v-if="dirty"
    class="diff-indicator"
    :title="originalValue ? `原值：${originalValue}` : '已修改'"
    :aria-label="originalValue ? `原值：${originalValue}` : '已修改'"
  />
</template>

<script setup lang="ts">
defineProps<{
  dirty: boolean
  originalValue?: string
}>()
</script>

<style scoped>
/* 橙色竖线：dirty 时显示，hover 显示原值 tooltip（由原生 title 提供） */
.diff-indicator {
  display: inline-block;
  width: 0;
  height: 1em;
  border-left: 3px solid var(--palwarm-primary);
  margin-right: 6px;
  vertical-align: middle;
  cursor: help;
}
</style>
