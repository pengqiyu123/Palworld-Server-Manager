import { defineStore } from 'pinia'
import { ref } from 'vue'

// Toast 类型与 useToast.ts 中的 ToastType 保持兼容
export type ToastType = 'success' | 'warning' | 'error' | 'info'

// 单条 Toast 队列项
export interface ToastItem {
  id: number
  type: ToastType
  message: string
  duration: number
}

// 队列最大长度，超过则丢弃最旧的
const MAX_QUEUE_SIZE = 5
// success / warning / info 自动消失时间（毫秒）
const AUTO_DISMISS_DURATION = 3000

export const useToastStore = defineStore('toast', () => {
  const items = ref<ToastItem[]>([])
  let nextId = 0

  // 推入一条 Toast；error 类型不自动消失，其余 3s 后自动移除
  function push(type: ToastType, message: string) {
    const id = ++nextId
    const duration = type === 'error' ? 0 : AUTO_DISMISS_DURATION
    items.value.push({ id, type, message, duration })
    // 队列长度上限：超过 5 删除最旧的
    while (items.value.length > MAX_QUEUE_SIZE) {
      items.value.shift()
    }
    if (duration > 0) {
      setTimeout(() => remove(id), duration)
    }
  }

  function remove(id: number) {
    const idx = items.value.findIndex(t => t.id === id)
    if (idx !== -1) {
      items.value.splice(idx, 1)
    }
  }

  function clear() {
    items.value = []
  }

  return {
    items,
    push,
    remove,
    clear,
  }
})
