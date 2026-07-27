import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { useServerStore } from '@/stores/server'
import { useSettingsStore } from '@/stores/settings'
import { useConfigStore } from '@/stores/config'
import PlayersView from '@/views/PlayersView.vue'
import { api } from '@/api/tauri'

const readyStatus = {
  running: true,
  ready: true,
  pid: 4242,
  managed_by_app: false,
  server_path: 'E:/PalServer',
  log_count: 0,
}

const livePlayer = {
  name: '煜',
  playerId: '4E239D4F000000000000000000000000',
  userId: 'steam_76561199381352956',
  iP: '127.0.0.1',
  ping: 12,
  location_x: 100,
  location_y: 200,
  level: 55,
}

vi.mock('@/api/tauri', () => ({
  api: {
    server: { getStatus: vi.fn() },
    rest: {
      getInfo: vi.fn(),
      getMetrics: vi.fn(),
      getPlayers: vi.fn(),
      announce: vi.fn(),
      kick: vi.fn(),
      ban: vi.fn(),
    },
  },
}))

describe('在线玩家实时同步', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    useSettingsStore().settings.server_path = 'E:/PalServer'
    useConfigStore().config = { ServerName: '同步测试服', ServerPlayerMaxNum: '4' }
    vi.mocked(api.server.getStatus).mockResolvedValue(readyStatus as never)
    vi.mocked(api.rest.getInfo).mockResolvedValue({
      version: 'v1', servername: '同步测试服', description: '', worldguid: 'world',
    })
    vi.mocked(api.rest.getMetrics).mockResolvedValue({
      currentplayernum: 1,
      serverfps: 60,
      serverfpsaverage: 60,
      serverframetime: 16,
      days: 1,
      maxplayernum: 4,
      basecampnum: 1,
      uptime: 120,
    })
  })

  it('玩家接口失败时不把读取失败伪装成无人在线', async () => {
    vi.mocked(api.rest.getPlayers).mockRejectedValue(new Error('REST 认证失败'))
    const store = useServerStore()

    await store.pollOnce()

    expect(store.playersState).toBe('error')
    expect(store.playersError).toContain('REST 认证失败')

    const wrapper = mount(PlayersView, {
      global: { stubs: { AppIcon: true, ConfirmDialog: true } },
    })
    expect(wrapper.text()).toContain('在线名单读取失败')
    expect(wrapper.text()).not.toContain('当前没有在线玩家')
  })

  it('一次成功轮询同步更新实时指标和玩家管理', async () => {
    vi.mocked(api.rest.getPlayers).mockResolvedValue([livePlayer])
    const store = useServerStore()

    const outcome = await store.pollOnce()

    expect(outcome).toBe('updated')
    expect(store.playersState).toBe('live')
    expect(store.players).toEqual([livePlayer])

    const playersView = mount(PlayersView, {
      global: { stubs: { AppIcon: true, ConfirmDialog: true } },
    })

    expect(store.serverMetrics?.currentplayernum).toBe(1)
    expect(store.serverMetrics?.maxplayernum).toBe(4)
    expect(playersView.text()).toContain('煜')
    expect(playersView.text()).toContain('每 3 秒自动刷新')
  })

  it('缺少服务器路径时明确返回跳过而不是虚报刷新成功', async () => {
    useSettingsStore().settings.server_path = ''
    const store = useServerStore()

    const outcome = await store.pollOnce()

    expect(outcome).toBe('skipped')
    expect(api.server.getStatus).not.toHaveBeenCalled()
  })

  it('后台自动轮询不占用手动刷新按钮的反馈状态', () => {
    vi.mocked(api.rest.getPlayers).mockResolvedValue([])
    const store = useServerStore()
    store.liveDataRefreshing = true

    const wrapper = mount(PlayersView, {
      global: { stubs: { AppIcon: true, ConfirmDialog: true } },
    })

    expect(wrapper.get('button.btn-ghost').text()).toBe('立即刷新')
    expect(wrapper.get('button.btn-ghost').attributes('disabled')).toBeUndefined()
  })
})
