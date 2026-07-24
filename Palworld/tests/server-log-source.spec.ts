import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('服务器日志来源提示', () => {
  it('区分管理器启动和手动启动的服务器日志入口', async () => {
    const [types, view] = await Promise.all([
      readFile(resolve(process.cwd(), 'src/types/tauri.ts'), 'utf8'),
      readFile(resolve(process.cwd(), 'src/views/OverviewView.vue'), 'utf8'),
    ])

    expect(types).toContain('managed_by_app: boolean')
    expect(view).toContain('serverStore.status.managed_by_app')
    expect(view).toContain('手动启动的服务器日志在黑色窗口中')
  })
})
