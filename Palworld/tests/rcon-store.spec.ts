import { describe, it, expect, beforeEach, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useRconStore } from '@/stores/rcon'
import { api } from '@/api/tauri'

// 隔离后端：只 mock api 层，验证 store 调用契约（host 固定 127.0.0.1、密码/端口从 ini 读，不进前端 JS）
vi.mock('@/api/tauri', () => ({
  api: {
    rcon: {
      connectUsingConfig: vi.fn(async () => 'RCON连接成功（使用配置文件，端口 25575）'),
      disconnect: vi.fn(async () => {}),
      isConnected: vi.fn(async () => true),
      send: vi.fn(async () => 'resp'),
    },
  },
}))

describe('rcon store（★D3 真实接线）', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.mocked(api.rcon.connectUsingConfig).mockClear()
  })

  it('connect 只调用 connectUsingConfig 且只传 server_path', async () => {
    const store = useRconStore()
    await store.connect('C://palworld-server')

    expect(api.rcon.connectUsingConfig).toHaveBeenCalledTimes(1)
    expect(api.rcon.connectUsingConfig).toHaveBeenCalledWith('C://palworld-server')
    expect(store.connected).toBe(true)
  })

  it('connect 空路径时直接抛错且不调用后端', async () => {
    const store = useRconStore()
    await expect(store.connect('')).rejects.toThrow()
    expect(api.rcon.connectUsingConfig).not.toHaveBeenCalled()
    expect(store.connected).toBe(false)
  })

  it('isConnected 是存在的轮询钩子函数', async () => {
    const store = useRconStore()
    expect(typeof store.isConnected).toBe('function')
    const r = await store.isConnected()
    expect(r).toBe(true)
  })

  it('send 在未连接时只记 err 不调用后端', async () => {
    const store = useRconStore()
    await store.send('Info')
    expect(api.rcon.send).not.toHaveBeenCalled()
    expect(store.lines.some((l) => l.kind === 'err')).toBe(true)
  })
})
