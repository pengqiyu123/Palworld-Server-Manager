import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('server startup readiness', () => {
  it('waits for the spawned server process to bind UDP before reporting success', async () => {
    const source = await readFile(resolve(process.cwd(), 'src-tauri/src/server.rs'), 'utf8')

    expect(source).toContain('wait_for_udp_binding')
    expect(source).toContain('has_udp_binding(pid, PAL_SERVER_UDP_PORT)')
    expect(source).toContain('-LocalPort')
    expect(source).toContain('服务器进程未能在启动期绑定 UDP 端口')
  })
})
