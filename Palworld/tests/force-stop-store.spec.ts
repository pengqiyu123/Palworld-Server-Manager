import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useServerStore } from '@/stores/server'
import { api } from '@/api/tauri'

const stoppedStatus = {
  running: false,
  pid: null,
  server_path: 'E:/SteamLibrary/steamapps/common/PalServer',
  log_count: 0,
}

vi.mock('@/api/tauri', () => ({
  api: {
    server: {
      forceStop: vi.fn(async () => stoppedStatus),
    },
  },
}))

describe('server store 强制停止', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.mocked(api.server.forceStop).mockClear()
  })

  it('调用即时终止命令，并用后端返回的已停止状态更新界面', async () => {
    const store = useServerStore()

    await store.forceStop()

    expect(api.server.forceStop).toHaveBeenCalledTimes(1)
    expect(store.status).toEqual(stoppedStatus)
    expect(store.loading).toBe(false)
  })
})
