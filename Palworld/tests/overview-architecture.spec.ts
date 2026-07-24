import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('OverviewView architecture', () => {
  it('keeps overview actions mounted when server status changes', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/OverviewView.vue'), 'utf8')

    expect(source).not.toContain('<template v-if="isDashboard">')
    expect(source).not.toContain("uiStore.setMode('dashboard')")
    expect(source).not.toContain('概览始终保留路径与操作区；运行态只更新服务器卡片。 -->\n    <template>')
  })
})
