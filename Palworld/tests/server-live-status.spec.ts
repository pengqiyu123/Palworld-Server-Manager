import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { useServerStore } from '@/stores/server'
import { useSettingsStore } from '@/stores/settings'
import { useConfigStore } from '@/stores/config'
import Sidebar from '@/components/layout/Sidebar.vue'
import { api } from '@/api/tauri'

const offlineStatus = {
  running: false,
  ready: false,
  pid: null,
  managed_by_app: false,
  server_path: 'E:/PalServer',
  log_count: 0,
}

const externalStatus = {
  ...offlineStatus,
  running: true,
  ready: true,
  pid: 4242,
}

vi.mock('@/api/tauri', () => ({
  api: {
    server: {
      getStatus: vi.fn(),
    },
    rest: {
      getInfo: vi.fn(async () => ({
        version: 'v1', servername: '测试服', description: '', worldguid: 'world',
      })),
      getMetrics: vi.fn(async () => ({
        currentplayernum: 0,
        serverfps: 60,
        serverfpsaverage: 60,
        serverframetime: 16,
        days: 1,
        maxplayernum: 32,
        basecampnum: 0,
        uptime: 10,
      })),
      getPlayers: vi.fn(async () => []),
    },
  },
}))

describe('服务器实时状态', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    setActivePinia(createPinia())
    vi.clearAllMocks()
    const settings = useSettingsStore()
    settings.settings.server_path = 'E:/PalServer'
  })

  it('持续发现外部启动，并在进程退出后清空在线数据', async () => {
    vi.mocked(api.server.getStatus)
      .mockResolvedValueOnce(externalStatus as never)
      .mockResolvedValueOnce(offlineStatus as never)
    const store = useServerStore()

    store.startLiveMonitoring()
    await vi.advanceTimersByTimeAsync(0)
    expect(store.status).toMatchObject({ running: true, ready: true, pid: 4242 })
    expect(store.serverMetrics?.maxplayernum).toBe(32)

    await vi.advanceTimersByTimeAsync(3_000)
    expect(store.status).toMatchObject({ running: false, ready: false, pid: null })
    expect(store.serverInfo).toBeNull()
    expect(store.serverMetrics).toBeNull()
    expect(store.players).toEqual([])
  })

  it('REST 不可用时显示保存的最大人数，不回退成硬编码 32', () => {
    const server = useServerStore()
    const config = useConfigStore()
    server.status = externalStatus as never
    server.serverMetrics = null
    config.config = { ServerPlayerMaxNum: '4' }

    const wrapper = mount(Sidebar, {
      global: {
        stubs: {
          RouterLink: { template: '<a><slot /></a>' },
          AppIcon: true,
        },
      },
    })

    expect(wrapper.text()).toContain('0/4')
    expect(wrapper.text()).not.toContain('0/32')
  })

  it('运行上限和新配置不一致时同时说明当前值与重启后的值', () => {
    const server = useServerStore()
    const config = useConfigStore()
    server.status = externalStatus as never
    server.serverMetrics = {
      currentplayernum: 0,
      serverfps: 60,
      serverfpsaverage: 60,
      serverframetime: 16,
      days: 1,
      maxplayernum: 32,
      basecampnum: 0,
      uptime: 10,
    }
    config.config = { ServerPlayerMaxNum: '4' }

    const wrapper = mount(Sidebar, {
      global: {
        stubs: {
          RouterLink: { template: '<a><slot /></a>' },
          AppIcon: true,
        },
      },
    })

    expect(wrapper.text()).toContain('0/32')
    expect(wrapper.text()).toContain('配置为 4，重启后生效')
  })
})
