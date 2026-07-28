import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useServerStore } from '@/stores/server'
import { api } from '@/api/tauri'

const stoppedStatus = {
  running: false,
  ready: false,
  pid: null,
  managed_by_app: false,
  server_path: 'E:/PalServer',
  log_count: 0,
}

vi.mock('@/api/tauri', () => ({
  api: {
    server: {
      init: vi.fn(),
      start: vi.fn(),
      getLogs: vi.fn(async () => []),
    },
  },
}))

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

describe('server store 启动状态', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('初始化服务器状态时不显示为正在启动服务器', async () => {
    const pending = deferred<typeof stoppedStatus>()
    vi.mocked(api.server.init).mockReturnValueOnce(pending.promise)
    const store = useServerStore()

    const initPromise = store.init('E:/PalServer')

    expect(store.loading).toBe(true)
    expect(store.starting).toBe(false)
    pending.resolve(stoppedStatus)
    await initPromise
  })

  it('只在 start 调用进行中显示启动状态，并在失败后复位', async () => {
    const pending = deferred<typeof stoppedStatus>()
    vi.mocked(api.server.start).mockReturnValueOnce(pending.promise)
    const store = useServerStore()

    const startPromise = store.start('E:/PalServer')

    expect(store.starting).toBe(true)
    pending.reject(new Error('启动失败'))
    await expect(startPromise).rejects.toThrow('启动失败')
    expect(store.starting).toBe(false)
  })
})
