<template>
  <div class="titlebar">
    <!-- 拖拽区：程序化调用 startDragging()，透明窗下比 data-tauri-drag-region 可靠 -->
    <div class="drag-zone" @mousedown="startDrag">
      <AppIcon name="logo" :size="22" class="app-icon" />
      <span class="app-name">Palworld 服务器管理器</span>
      <span class="drag-hint">拖拽此处移动窗口</span>
    </div>

    <div class="win-controls">
      <div class="win-btn" title="最小化" @click="minimize">
        <svg width="40" height="32" viewBox="0 0 40 32" fill="none">
          <path d="M15 16H25" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" />
        </svg>
      </div>
      <div class="win-btn" title="最大化" @click="toggleMaximize">
        <svg width="40" height="32" viewBox="0 0 40 32" fill="none">
          <rect x="15" y="11" width="10" height="10" rx="2" stroke="#a39383" stroke-width="1.4" />
        </svg>
      </div>
      <div class="win-btn close" title="关闭" @click="close">
        <svg width="40" height="32" viewBox="0 0 40 32" fill="none">
          <path d="M15.5 11.5L24.5 20.5M24.5 11.5L15.5 20.5" stroke="#a39383" stroke-width="1.4" stroke-linecap="round" />
        </svg>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import AppIcon from '@/components/ui/AppIcon.vue'

// 窗口控制按钮：调用 @tauri-apps/api/window 的真实窗口 API（安全，非业务后端命令）
const appWindow = getCurrentWindow()

// 程序化拖拽：透明无边框窗下最可靠，比 data-tauri-drag-region 稳定
function startDrag(e: MouseEvent): void {
  if (e.button !== 0) return // 仅左键
  // 双击最大化，单击拖拽（官方手动实现示例，原生体验）
  e.detail === 2 ? void appWindow.toggleMaximize() : void appWindow.startDragging()
}

function minimize(): void {
  void appWindow.minimize()
}

function toggleMaximize(): void {
  void appWindow.toggleMaximize()
}

function close(): void {
  void appWindow.close()
}
</script>
