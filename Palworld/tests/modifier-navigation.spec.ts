import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(__dirname, '..')
const router = readFileSync(resolve(root, 'src/router/index.ts'), 'utf-8')
const sidebar = readFileSync(resolve(root, 'src/components/layout/Sidebar.vue'), 'utf-8')
const modifier = readFileSync(resolve(root, 'src/views/ModifierView.vue'), 'utf-8')
const tauriApi = readFileSync(resolve(root, 'src/api/tauri.ts'), 'utf-8')

describe('修改器入口', () => {
  it('在导航中提供独立的角色修改器页面', () => {
    expect(router).toContain("path: '/modifier'")
    expect(router).toContain("name: 'modifier'")
    expect(sidebar).toContain("path: '/modifier'")
    expect(sidebar).toContain("label: '修改器'")
  })
})

describe('修改器世界状态', () => {
  it('读取真实世界状态并提供玩家与公会两个视图', () => {
    expect(modifier).toContain('modifierApi.getWorld')
    expect(modifier).toContain('玩家')
    expect(modifier).toContain('公会')
    expect(modifier).toContain('technology_points')
  })

  it('修改前预览影响，写入成功后重新读取世界', () => {
    expect(modifier).toContain('operationStatus')
    expect(modifier).toContain('modifierApi.previewAction')
    expect(modifier).toContain('检查游戏和服务器')
    expect(modifier).toContain('modifierApi.onProgress')
    expect(modifier).toContain('modifier-progress-overlay')
    expect(modifier).toContain('modifierApi.applyAction')
    expect(modifier).toContain('await loadCurrentWorld()')
    expect(modifier).toContain('finally')
  })

  it('通过真实 Tauri API 调用读取、预览和写入命令', () => {
    expect(tauriApi).toContain("get_modifier_world")
    expect(tauriApi).toContain("preview_modifier_action")
    expect(tauriApi).toContain("apply_modifier_action")
  })
})
