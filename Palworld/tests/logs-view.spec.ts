import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'
import LogsView from '@/views/LogsView.vue'

// 捕获组件挂载时注册的 server-log / server-log-source 事件回调
const { handlers } = vi.hoisted(() => ({
  handlers: {} as Record<string, (e: { payload: string }) => void>,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (event: string, cb: (e: { payload: string }) => void) => {
    handlers[event] = cb
    return () => {}
  }),
}))

vi.mock('@/api/tauri', () => ({
  api: {
    server: {
      getLogs: vi.fn(async () => []),
    },
  },
}))

async function mountAndSettle() {
  const wrapper = mount(LogsView, {
    global: { stubs: { AppIcon: true } },
  })
  // 等 onMounted 内的 getLogs + listen 异步注册完成
  await flushPromises()
  await new Promise((r) => setTimeout(r, 0))
  return wrapper
}

describe('LogsView（★D4 实时日志面板）', () => {
  beforeEach(() => {
    handlers['server-log'] = () => {}
    handlers['server-log-source'] = () => {}
  })

  it('500 行环形截断', async () => {
    const wrapper = await mountAndSettle()
    for (let i = 0; i < 600; i++) {
      handlers['server-log']({ payload: `line ${i}` })
    }
    await nextTick()
    const lines = wrapper.findAll('.log-line')
    expect(lines.length).toBe(500)
  })

  it('来源提示横条：wrapper 显示不可用，cmd 显示可用', async () => {
    const wrapper = await mountAndSettle()

    handlers['server-log-source']({ payload: 'wrapper' })
    await nextTick()
    expect(wrapper.text()).toContain('日志不可用')

    handlers['server-log-source']({ payload: 'cmd' })
    await nextTick()
    expect(wrapper.text()).toContain('Cmd 模式')
  })

  it('普通日志行实时追加', async () => {
    const wrapper = await mountAndSettle()
    handlers['server-log']({ payload: '[LOG] friend connected' })
    await nextTick()
    expect(wrapper.findAll('.log-line').length).toBe(1)
    expect(wrapper.text()).toContain('[LOG] friend connected')
  })
})
