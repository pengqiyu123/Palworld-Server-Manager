<template>
  <div class="step-card" :class="state">
    <div class="step-num">{{ num }}</div>
    <h4>{{ title }}</h4>
    <p>{{ desc }}</p>
  </div>
</template>

<script setup lang="ts">
/** S1 向导步骤卡：num 序号 / title 标题 / desc 描述 / state 当前态 */
defineProps<{
  num: number
  title: string
  desc: string
  state: 'active' | 'locked' | 'done'
}>()
</script>
