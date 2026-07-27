import { describe, it, expect, beforeEach, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useRconStore } from '@/stores/rcon'
import { api } from '@/api/tauri'

// 隔离后端：只 mock api 层，验证 store 调用契约（host 固定 127.0.0.1、密码/端口从 ini 读，不进前端 JS）
vi.mock('@/api/tauri', () => ({
  api: {
    management: {
      connect: vi.fn(async () => ({
        message: '服务器管理接口已连接',
        host: '127.0.0.1',
        port: 8212,
        servername: '测试服',
        version: 'v1.0',
      })),
      execute: vi.fn(async () => '操作成功'),
    },
  },
}))

describe('服务器管理控制台 store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('connect 通过 REST 管理接口连接且只传 server_path', async () => {
    const store = useRconStore()
    await store.connect('C://palworld-server')

    expect(api.management.connect).toHaveBeenCalledTimes(1)
    expect(api.management.connect).toHaveBeenCalledWith('C://palworld-server')
    expect(store.connected).toBe(true)
  })

  it('connect 保存 REST 后端返回的真实端点，而不是猜测默认端口', async () => {
    vi.mocked(api.management.connect).mockResolvedValueOnce({
      message: '服务器管理接口已连接',
      host: '192.168.10.24',
      port: 8213,
      servername: '朋友服',
      version: 'v1.0',
    } as never)
    const store = useRconStore()

    await store.connect('C://palworld-server')

    expect(store.connectionInfo).toEqual({
      message: '服务器管理接口已连接',
      host: '192.168.10.24',
      port: 8213,
      servername: '朋友服',
      version: 'v1.0',
    })
    expect(store.lines.at(-1)?.text).toContain('192.168.10.24:8213')
  })

  it('connect 失败时不保留虚构端点，并原样记录后端错误', async () => {
    vi.mocked(api.management.connect).mockRejectedValueOnce(new Error('REST 认证失败：请检查 AdminPassword 配置'))
    const store = useRconStore()

    await expect(store.connect('C://palworld-server')).rejects.toThrow('REST 认证失败')

    expect(store.connected).toBe(false)
    expect(store.connectionInfo).toBeNull()
    expect(store.lines.at(-1)?.text).toContain('REST 认证失败')
  })

  it('connect 空路径时直接抛错且不调用后端', async () => {
    const store = useRconStore()
    await expect(store.connect('')).rejects.toThrow()
    expect(api.management.connect).not.toHaveBeenCalled()
    expect(store.connected).toBe(false)
  })

  it('isConnected 用同一 REST 入口重新核验，不依赖内存连接标记', async () => {
    const store = useRconStore()
    await store.connect('C://palworld-server')
    vi.mocked(api.management.connect).mockClear()

    expect(await store.isConnected()).toBe(true)
    expect(api.management.connect).toHaveBeenCalledWith('C://palworld-server')
  })

  it('send 在未连接时只记 err 不调用 REST 后端', async () => {
    const store = useRconStore()
    await store.send('Info')
    expect(api.management.execute).not.toHaveBeenCalled()
    expect(store.lines.some((l) => l.kind === 'err')).toBe(true)
  })

  it('send 把受支持命令交给 REST 后端并记录真实响应', async () => {
    const store = useRconStore()
    await store.connect('C://palworld-server')

    await store.send('Save')

    expect(api.management.execute).toHaveBeenCalledWith('C://palworld-server', 'Save')
    expect(store.lines.at(-1)).toEqual({ kind: 'resp', text: '< 操作成功' })
  })
})
