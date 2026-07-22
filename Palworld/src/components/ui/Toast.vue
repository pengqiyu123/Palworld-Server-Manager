<template>
  <Teleport to="body">
    <div class="toast-container">
      <TransitionGroup name="toast">
        <div
          v-for="toast in toasts"
          :key="toast.id"
          class="toast"
          :class="`toast--${toast.type}`"
          @click="remove(toast.id)"
        >
          <span class="toast__icon">{{ iconFor(toast.type) }}</span>
          <span class="toast__message">{{ toast.message }}</span>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { useToast, type ToastType } from './useToast'

const { toasts, remove } = useToast()

function iconFor(type: ToastType): string {
  const icons = {
    success: '✓',
    warning: '!',
    error: '✕',
    info: 'i',
  }
  return icons[type]
}
</script>

<style scoped>
/* 右上角固定定位，垂直堆叠，8px 间距 */
.toast-container {
  position: fixed;
  top: 20px;
  right: 20px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  pointer-events: none;
  max-width: 480px;
}
.toast {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 16px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: var(--r-card);
  box-shadow: var(--shadow-win);
  cursor: pointer;
  pointer-events: auto;
  min-width: 200px;
  max-width: 480px;
  font-size: 13px;
}
.toast--success { border-left: 3px solid var(--green); }
.toast--warning { border-left: 3px solid var(--amber); }
.toast--error   { border-left: 3px solid var(--red); }
.toast--info    { border-left: 3px solid var(--primary); }
.toast__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  font-size: 11px;
  font-weight: bold;
  color: var(--text-hi);
  background: var(--green);
}
.toast--warning .toast__icon { background: var(--amber); }
.toast--error   .toast__icon { background: var(--red); }
.toast--info    .toast__icon { background: var(--primary); }
.toast__message { color: var(--text-hi); }
.toast-enter-active, .toast-leave-active {
  transition: all 0.3s ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translateY(-12px);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(20px);
}
</style>
