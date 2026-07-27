import { defineStore } from 'pinia'
import { reactive } from 'vue'

/**
 * 全局 UI 状态：
 * - tooltip：单一浮动气泡（由 InfoTip 写入，Tooltip 读取渲染）
 * - wizard：S1 首屏向导（自动探测 / 手动指定 的本地状态）
 *
 * 本轮为纯视觉还原，所有状态均为前端本地 state / 硬编码，不调用任何 Tauri 命令。
 */

export interface TooltipState {
  /** 是否可见 */
  visible: boolean
  /** 只允许纯文本，避免把外部描述送入 HTML 解析器。 */
  text: string
  /** 视口坐标 */
  x: number
  y: number
}

export interface WizardState {
  /** 当前激活步 1..3 */
  step: number
  /** 是否已探测 / 手动指定 */
  detected: boolean
  /** 已探测到的服务器目录 */
  detectedPath: string
  /** 探测中（禁用按钮 + 切换文案） */
  detecting: boolean
  /** 是否手动模式 */
  manual: boolean
  /** 双模式：wizard（首屏向导）/ dashboard（仪表盘） */
  mode: 'wizard' | 'dashboard'
}

export const useUiStore = defineStore('ui', () => {
  const tooltip = reactive<TooltipState>({
    visible: false,
    text: '',
    x: 0,
    y: 0,
  })

  const wizard = reactive<WizardState>({
    step: 1,
    detected: false,
    detectedPath: '',
    detecting: false,
    manual: false,
    mode: 'wizard',
  })

  /** 显示浮动 tooltip（由 InfoTip 的鼠标锚点坐标驱动） */
  function setTooltip(visible: boolean, text: string, x: number, y: number): void {
    tooltip.visible = visible
    tooltip.text = text
    tooltip.x = x
    tooltip.y = y
  }

  /** 隐藏浮动 tooltip（InfoTip mouseleave 调用，可带延迟） */
  function hideTooltip(): void {
    tooltip.visible = false
  }

  /** 开始探测：进入探测中态，禁用按钮 */
  function startDetect(): void {
    wizard.detecting = true
  }

  /**
   * 探测完成：解锁第 2 步、侧边状态卡变绿。
   * @param path 探测到的服务器目录
   */
  function finishDetect(path: string): void {
    wizard.detecting = false
    wizard.detected = true
    wizard.detectedPath = path
    wizard.step = 2
    wizard.manual = false
  }

  /** 手动指定目录：同样置 detected，文案为「（手动）」 */
  function setManual(path: string): void {
    wizard.detecting = false
    wizard.detected = true
    wizard.detectedPath = path
    wizard.step = 2
    wizard.manual = true
  }

  /** 切换双模式：wizard（首屏向导）/ dashboard（仪表盘） */
  function setMode(mode: 'wizard' | 'dashboard'): void {
    wizard.mode = mode
  }

  return {
    tooltip,
    wizard,
    setTooltip,
    hideTooltip,
    startDetect,
    finishDetect,
    setManual,
    setMode,
  }
})
