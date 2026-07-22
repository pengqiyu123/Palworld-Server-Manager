<template>
  <Teleport to="body">
    <Transition name="tooltip">
      <div
        v-if="uiStore.tooltip.visible"
        class="app-tooltip"
        :style="{ left: uiStore.tooltip.x + 'px', top: uiStore.tooltip.y + 'px' }"
        v-html="uiStore.tooltip.html"
      />
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { useUiStore } from '@/stores/ui'

/** 全局浮动 tooltip：读取 uiStore.tooltip，Teleport 到 body 渲染于指定坐标（垂直居中于锚点） */
const uiStore = useUiStore()
</script>

<style scoped>
.app-tooltip {
  transform: translateY(-50%);
}
</style>
