<template>
  <!-- 全量内联 SVG，像素级还原原型；用 span 包一层 v-html 保证 SVG 命名空间正确解析 -->
  <span class="app-icon-wrap" v-html="svgMarkup" />
</template>

<script setup lang="ts">
import { computed } from 'vue'

/**
 * 统一图标组件：按 name 渲染原型内联 SVG。
 * 复用 ui-redesign/index.html 中的 SVG path，避免混用 lucide。
 */
const props = withDefaults(
  defineProps<{
    /** 图标名称 */
    name: string
    /** 渲染尺寸（px），默认 18 */
    size?: number
  }>(),
  { size: 18 }
)

// 每个图标：viewBox + 内部 SVG 片段（已对齐原型原色）
const ICONS: Record<string, { vb: string; inner: string }> = {
  // —— 标题栏 logo ——
  logo: {
    vb: '0 0 22 22',
    inner: '<circle cx="11" cy="11" r="9" fill="#3a2e26" stroke="#e66f51" stroke-width="1.5"/><path d="M2.5 11H19.5" stroke="#e66f51" stroke-width="1.5"/><circle cx="11" cy="11" r="2.6" fill="#e66f51" stroke="#e66f51" stroke-width="1.5"/>',
  },
  // —— 侧边导航 ——
  overview: {
    vb: '0 0 18 18',
    inner: '<path d="M3 8L9 3L15 8V15H11V10.5H7V15H3V8Z" stroke="#a39383" stroke-width="1.4" stroke-linejoin="round" fill="none"/>',
  },
  config: {
    vb: '0 0 18 18',
    inner: '<path d="M3 5.5H15M3 12.5H15" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" fill="none"/><circle cx="11" cy="5.5" r="2.2" fill="#3f322c" stroke="#a39383" stroke-width="1.4"/><circle cx="7" cy="12.5" r="2.2" fill="#3f322c" stroke="#a39383" stroke-width="1.4"/>',
  },
  network: {
    vb: '0 0 18 18',
    inner: '<circle cx="9" cy="9" r="6.5" stroke="#a39383" stroke-width="1.4" fill="none"/><path d="M2.5 9H15.5M9 2.5C11.2 5 11.2 13 9 15.5M9 2.5C6.8 5 6.8 13 9 15.5" stroke="#a39383" stroke-width="1.2" fill="none"/>',
  },
  rcon: {
    vb: '0 0 18 18',
    inner: '<rect x="2.5" y="3.5" width="13" height="11" rx="2" stroke="#a39383" stroke-width="1.4" fill="none"/><path d="M5.5 7.5L8 9.5L5.5 11.5" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/><path d="M9.5 11.5H12.5" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" fill="none"/>',
  },
  players: {
    vb: '0 0 18 18',
    inner: '<circle cx="6.5" cy="6" r="2.2" stroke="#a39383" stroke-width="1.3" fill="none"/><circle cx="12.5" cy="6" r="2.2" stroke="#a39383" stroke-width="1.3" fill="none"/><path d="M2.5 14.5C2.5 11.7 4.3 9.5 6.5 9.5C8.7 9.5 10.5 11.7 10.5 14.5" stroke="#a39383" stroke-width="1.3" stroke-linecap="round" fill="none"/><path d="M10.5 14.5C10.5 11.7 12.3 9.5 14.5 9.5C15.5 9.5 16.5 10 16.5 10" stroke="#a39383" stroke-width="1.3" stroke-linecap="round" fill="none"/>',
  },
  logs: {
    vb: '0 0 18 18',
    inner: '<path d="M6 5H15M6 9H15M6 13H12" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" fill="none"/><circle cx="3" cy="5" r="0.9" fill="#a39383"/><circle cx="3" cy="9" r="0.9" fill="#a39383"/><circle cx="3" cy="13" r="0.9" fill="#a39383"/>',
  },
  backup: {
    vb: '0 0 18 18',
    inner: '<rect x="3" y="3" width="12" height="12" rx="2" stroke="#a39383" stroke-width="1.4" fill="none"/><path d="M6 3V6H12V3" stroke="#a39383" stroke-width="1.4" fill="none"/><circle cx="9" cy="10.5" r="2.2" stroke="#a39383" stroke-width="1.4" fill="none"/>',
  },
  settings: {
    vb: '0 0 18 18',
    inner: '<circle cx="9" cy="9" r="2.5" stroke="#a39383" stroke-width="1.4" fill="none"/><path d="M9 2V4M9 14V16M16 9H14M4 9H2M13.9 4.1L12.5 5.5M5.5 12.5L4.1 13.9M13.9 13.9L12.5 12.5M5.5 5.5L4.1 4.1" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" fill="none"/>',
  },
  // —— 配置分组头 ——
  'group-basic': {
    vb: '0 0 18 18',
    inner: '<rect x="3" y="3" width="12" height="12" rx="3" stroke="#a39383" stroke-width="1.4" fill="none"/><path d="M6 10L12 10M10 7V13" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" fill="none"/>',
  },
  'group-rules': {
    vb: '0 0 18 18',
    inner: '<path d="M9 3L11.5 7H16L12 10.5L13.5 15L9 12L4.5 15L6 10.5L2 7H6.5L9 3Z" stroke="#a39383" stroke-width="1.4" stroke-linejoin="round" fill="none"/>',
  },
  'group-perf': {
    vb: '0 0 18 18',
    inner: '<rect x="3" y="5" width="12" height="8" rx="1.5" stroke="#a39383" stroke-width="1.4" fill="none"/><path d="M6 9H12M9 6V12" stroke="#a39383" stroke-width="1.4" fill="none"/>',
  },
  // —— 端口图标 ——
  'port-game': {
    vb: '0 0 22 22',
    inner: '<circle cx="11" cy="11" r="7" stroke="#4f8a6b" stroke-width="1.5" fill="none"/><path d="M5 11H17M11 5V17" stroke="#4f8a6b" stroke-width="1.4" fill="none"/>',
  },
  'port-rcon': {
    vb: '0 0 22 22',
    inner: '<rect x="4" y="5" width="14" height="12" rx="2" stroke="#e66f51" stroke-width="1.4" fill="none"/><path d="M7 11H15M10 8V14" stroke="#e66f51" stroke-width="1.4" fill="none"/>',
  },
  'port-rest': {
    vb: '0 0 22 22',
    inner: '<path d="M5 7H17M5 12H13M5 17H10" stroke="#9b6a9e" stroke-width="1.4" stroke-linecap="round" fill="none"/>',
  },
  // —— 杂项 ——
  search: {
    vb: '0 0 16 16',
    inner: '<circle cx="7" cy="7" r="4.5" stroke="#a39383" stroke-width="1.4" fill="none"/><path d="M11 11L14 14" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" fill="none"/>',
  },
  chevron: {
    vb: '0 0 16 16',
    inner: '<path d="M5 6L8 9L11 6" stroke="#a39383" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>',
  },
  info: {
    vb: '0 0 15 15',
    inner: '<circle cx="7.5" cy="7.5" r="6" stroke="#a39383" stroke-width="1.2" fill="none"/><path d="M7.5 6.8V10" stroke="#a39383" stroke-width="1.2" stroke-linecap="round" fill="none"/><circle cx="7.5" cy="4.6" r="0.6" fill="#a39383"/>',
  },
  vpn: {
    vb: '0 0 36 36',
    inner: '<circle cx="18" cy="12" r="6" stroke="#e66f51" stroke-width="1.8" fill="none"/><path d="M10 24C10 19.6 13.6 16 18 16C22.4 16 26 19.6 26 24V28H10V24Z" stroke="#e66f51" stroke-width="1.8" stroke-linejoin="round" fill="none"/>',
  },
  firewall: {
    vb: '0 0 22 22',
    inner: '<path d="M5 10H17M7 6H15M9 15V19H13V15" stroke="#e66f51" stroke-width="1.4" stroke-linecap="round" fill="none"/>',
  },
  // —— 占位屏图标 ——
  'ph-logs': {
    vb: '0 0 56 56',
    inner: '<path d="M14 18H44M14 28H44M14 38H36" stroke="#a39383" stroke-width="2.4" stroke-linecap="round" fill="none"/><circle cx="9" cy="18" r="1.6" fill="#a39383"/><circle cx="9" cy="28" r="1.6" fill="#a39383"/><circle cx="9" cy="38" r="1.6" fill="#a39383"/>',
  },
  // —— 存档管理（暖橙，复用品牌色） ——
  save: {
    vb: '0 0 22 22',
    inner: '<path d="M4 5H15L18 8V17C18 17.6 17.6 18 17 18H5C4.4 18 4 17.6 4 17V5Z" stroke="#e66f51" stroke-width="1.4" fill="none" stroke-linejoin="round"/><path d="M7 5V9H13V5" stroke="#e66f51" stroke-width="1.4" fill="none"/><circle cx="11" cy="13" r="1.8" stroke="#e66f51" stroke-width="1.4" fill="none"/>',
  },
  // —— 存档迁移（暖橙，F5 新增） ——
  migration: {
    vb: '0 0 22 22',
    inner: '<rect x="3" y="6.5" width="7" height="10" rx="1.5" stroke="#e66f51" stroke-width="1.4" fill="none"/><path d="M12 11.5H15.5M14 9.5L16.5 11.5L14 13.5" stroke="#e66f51" stroke-width="1.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/><rect x="17" y="6.5" width="2.5" height="10" rx="1" stroke="#e66f51" stroke-width="1.4" fill="none"/>',
  },
  'ph-backup': {
    vb: '0 0 56 56',
    inner: '<rect x="9" y="9" width="38" height="38" rx="5" stroke="#a39383" stroke-width="2.4" fill="none"/><path d="M18 9V18H38V9" stroke="#a39383" stroke-width="2.4" fill="none"/><circle cx="28" cy="32" r="6" stroke="#a39383" stroke-width="2.4" fill="none"/>',
  },
  'ph-settings': {
    vb: '0 0 56 56',
    inner: '<circle cx="28" cy="28" r="8" stroke="#a39383" stroke-width="2.4" fill="none"/><path d="M28 6V14M28 42V50M50 28H42M14 28H6M44 12L38 18M18 38L12 44M44 44L38 38M18 18L12 12" stroke="#a39383" stroke-width="2.4" stroke-linecap="round" fill="none"/>',
  },
}

const svgMarkup = computed(() => {
  const def = ICONS[props.name]
  if (!def) return ''
  const size = props.size
  return `<svg viewBox="${def.vb}" width="${size}" height="${size}" fill="none" xmlns="http://www.w3.org/2000/svg">${def.inner}</svg>`
})
</script>

<style scoped>
.app-icon-wrap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 0;
}
.app-icon-wrap :deep(svg) {
  display: block;
}
</style>
