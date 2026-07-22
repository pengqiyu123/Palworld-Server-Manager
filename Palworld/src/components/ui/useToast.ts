import { computed } from 'vue'
import { useToastStore, type ToastType, type ToastItem } from '@/stores/toast'
import type { ErrorClass } from '@/types/tauri'

// 重新导出类型以保持现有导入兼容
export type { ToastType, ToastItem }

/**
 * 全局错误分类（M3-C 全局错误 toast）。
 * 把后端返回的人话中文错误，按关键字映射到 5 类，供 Toast 图标 / 文案 + 60s 防抖使用。
 * 映射依据：design §1.5.1（通用）+ §7.4（RCON 专项）。
 */
export function classifyError(msg: string): ErrorClass {
  const m = msg.toLowerCase()
  if (m.includes('认证失败') || m.includes('401')) {
    return 'AuthFailed'
  }
  if (m.includes('未放行') || m.includes('blocked')) {
    return 'PortBlocked'
  }
  if (m.includes('未运行') || m.includes('进程不存在') || m.includes('未连接')) {
    return 'ProcessDown'
  }
  if (
    m.includes('不可达') ||
    m.includes('连接失败') ||
    m.includes('connection refused') ||
    m.includes('timeout') ||
    m.includes('超时')
  ) {
    return 'NetworkUnreachable'
  }
  return 'Other'
}

// 同错误类 60s 内只弹一次（防抖 Map）
const errorClassMap = new Map<ErrorClass, number>()
const DEBOUNCE_MS = 60_000

/**
 * Toast 组合式 API：内部委托 useToastStore（Pinia）管理队列，
 * 对外保持原有 success / warning / error / info / remove / toasts API 不变。
 *
 * 注意：必须在 pinia 已安装后调用（组件 setup 内或 app.mount 之后）。
 */
export function useToast() {
  const store = useToastStore()

  return {
    // 暴露为只读 ref，保持与原模块级 toasts 一致的消费方式
    toasts: computed(() => store.items),
    success: (msg: string) => store.push('success', msg),
    warning: (msg: string) => store.push('warning', msg),
    /**
     * 错误 toast：自动分类 + 60s 防抖（同一 ErrorClass 在 60s 内只弹一次）。
     * 轮询失败（server-log 高频）不走本方法（pollOnce 用 console.error，不 rethrow、不 toast）。
     */
    error: (msg: string) => {
      const cls = classifyError(msg)
      const now = Date.now()
      const last = errorClassMap.get(cls) ?? 0
      if (now - last < DEBOUNCE_MS) return
      errorClassMap.set(cls, now)
      store.push('error', msg)
    },
    info: (msg: string) => store.push('info', msg),
    remove: (id: number) => store.remove(id),
    clear: () => store.clear(),
  }
}
