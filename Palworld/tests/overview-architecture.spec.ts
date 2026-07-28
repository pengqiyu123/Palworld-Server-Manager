import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('OverviewView architecture', () => {
  it('uses the server start action state for start buttons', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/OverviewView.vue'), 'utf8')

    expect(source).toContain("serverStore.starting ? '启动中…' : '启动服务器'")
    expect(source).not.toContain("serverStore.loading ? '启动中…' : '启动服务器'")
  })

  it('keeps overview actions mounted when server status changes', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/OverviewView.vue'), 'utf8')

    expect(source).not.toContain('<template v-if="isDashboard">')
    expect(source).not.toContain("uiStore.setMode('dashboard')")
    expect(source).not.toContain('概览始终保留路径与操作区；运行态只更新服务器卡片。 -->\n    <template>')
  })

  it('shows a live operations dashboard after setup instead of the initial welcome screen', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/OverviewView.vue'), 'utf8')

    expect(source).toContain('当前运行状态')
    expect(source).toContain('外部启动')
    expect(source).toContain('最近检查')
    expect(source).toContain("router.push('/rcon')")
    expect(source).toContain("router.push('/logs')")
    expect(source).toContain("router.push('/saves')")
    expect(source).toContain("router.push('/migrate')")
    expect(source).toContain("router.push('/modifier')")
  })

  it('shows the shared live-player state instead of presenting REST failures as zero players', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/OverviewView.vue'), 'utf8')

    expect(source).toContain("serverStore.playersState === 'error'")
    expect(source).toContain('联机数据读取失败')
    expect(source).toContain('serverStore.players')
    expect(source).toContain('在线玩家')
  })

  it('does not expose internal acceptance language or mark unchecked applications as detected', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/OverviewView.vue'), 'utf8')
    const manualApps = source.slice(
      source.indexOf('async function onManualRadmin'),
      source.indexOf('async function onStart'),
    )

    expect(source).not.toContain('D1 验收')
    expect(manualApps).toContain('api.launcher.validateRadminPath')
    expect(manualApps).toContain('api.launcher.detectGame')
    expect(manualApps).not.toContain('radminDetected.value = true')
    expect(manualApps).not.toContain('gameDetected.value = true')
  })

  it('describes an installed game as located instead of claiming it is running', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/OverviewView.vue'), 'utf8')

    expect(source).not.toContain("gameDetected ? '已启动' : '待启动'")
    expect(source).toContain("gameDetected ? '已定位' : '待定位'")
  })
})
