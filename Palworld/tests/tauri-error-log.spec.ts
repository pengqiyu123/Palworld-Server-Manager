import { beforeEach, describe, expect, it, vi } from 'vitest'

const invokeMock = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}))

import { tauriInvoke } from '@/api/tauri'

describe('Tauri 命令错误日志', () => {
  beforeEach(() => {
    invokeMock.mockReset()
  })

  it('真实操作失败时写入脱敏系统日志并继续抛出原错误', async () => {
    invokeMock
      .mockRejectedValueOnce('找不到 PalServer.exe password=secret')
      .mockResolvedValueOnce(undefined)

    await expect(tauriInvoke('start_server', { path: 'E:/PalServer', password: 'secret' }))
      .rejects.toThrow('找不到 PalServer.exe password=secret')

    expect(invokeMock).toHaveBeenNthCalledWith(2, 'write_app_log', {
      level: 'ERROR',
      operation: 'command.start_server',
      message: '找不到 PalServer.exe password=secret',
    })
    expect(JSON.stringify(invokeMock.mock.calls[1])).not.toContain('E:/PalServer')
  })

  it('轮询读取失败不反复写入系统日志', async () => {
    invokeMock.mockRejectedValueOnce('服务器离线')

    await expect(tauriInvoke('get_server_status')).rejects.toThrow('服务器离线')

    expect(invokeMock).toHaveBeenCalledTimes(1)
  })
})
