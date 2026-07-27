<template>
  <div class="terminal">
    <div class="t-bar">服务器管理响应 · 实时记录</div>
    <div class="t-body" ref="bodyRef">
      <div v-for="(line, i) in lines" :key="i" :class="'log-' + line.kind">{{ line.text }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import type { RconLogLine } from '@/stores/rcon'

/** 终端输出区：渲染 RconLogLine[]，等宽、彩色分级，自动滚到底部 */
const props = defineProps<{ lines: RconLogLine[] }>()

const bodyRef = ref<HTMLElement | null>(null)

watch(
  () => props.lines.length,
  async () => {
    await nextTick()
    if (bodyRef.value) {
      bodyRef.value.scrollTop = bodyRef.value.scrollHeight
    }
  }
)
</script>
