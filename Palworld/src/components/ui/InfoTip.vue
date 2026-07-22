<template>
  <span
    class="tip"
    @mouseenter="onEnter"
    @mouseleave="onLeave"
  >
    <AppIcon name="info" :size="15" />
  </span>
</template>

<script setup lang="ts">
import { useUiStore } from '@/stores/ui'
import AppIcon from './AppIcon.vue'

/** ⓘ 触发按钮：鼠标悬停时计算锚点坐标并写入全局 tooltip 状态 */
const props = defineProps<{
  /** 气泡内联 HTML（原型 data-tip 原文） */
  html: string
}>()

const uiStore = useUiStore()

let hideTimer: number | null = null

function onEnter(e: MouseEvent): void {
  if (hideTimer !== null) {
    clearTimeout(hideTimer)
    hideTimer = null
  }
  const target = e.currentTarget as HTMLElement
  const r = target.getBoundingClientRect()
  const tw = 280 // 与 .app-tooltip max-width 一致，用于水平翻转估算
  let left = r.right + 10
  if (left + tw > window.innerWidth - 8) {
    left = r.left - tw - 10
  }
  const top = r.top + r.height / 2
  uiStore.setTooltip(true, props.html, left, top)
}

function onLeave(): void {
  // 80ms 延迟消失，避免鼠标移入气泡瞬间消失（本轮气泡无交互，纯展示）
  hideTimer = window.setTimeout(() => uiStore.hideTooltip(), 80)
}
</script>
