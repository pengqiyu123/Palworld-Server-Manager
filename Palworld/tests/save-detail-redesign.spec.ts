/**
 * TDD 守卫：本地存档详情改为「点击弹窗 + 按路径解析玩家列表」。
 * 读源码字符串断言（与现有 tests/*.spec.ts 风格一致），不依赖运行时挂载。
 */
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(__dirname, '..')
const read = (p: string) => readFileSync(resolve(root, p), 'utf-8')

const saveView = read('src/views/SaveManagementView.vue')
const modal = read('src/components/save/SaveDetailModal.vue')
const apiTauri = read('src/api/tauri.ts')
const worldCopy = read('src-tauri/src/save_edit/world_copy.rs')
const saveEdit = read('src-tauri/src/save_edit.rs')
const mainRs = read('src-tauri/src/main.rs')

describe('本地存档详情弹窗（Q 用户反馈修复）', () => {
  it('卡片不再内联「迁移到服务器 / 设为备份目标」按钮', () => {
    // 本地卡、服务器卡都不应再带 wc-migrate 行内按钮
    expect(saveView).not.toContain('wc-migrate')
    // 内联按钮形式（带实参调用）已移除；弹窗事件绑定 @migrate/@set-backup 仍保留
    expect(saveView).not.toContain('onMigrateToServer(w)')
    expect(saveView).not.toContain('@click.stop="onMigrateToServer')
    expect(saveView).not.toContain('@click.stop="onSelectServerWorld')
  })

  it('使用 SaveDetailModal 弹窗而非内联 world-detail 面板', () => {
    expect(saveView).toContain('SaveDetailModal')
    expect(saveView).not.toContain('class="world-detail"')
    expect(saveView).not.toContain('wd-grid')
  })

  it('世界详情复用修改器富解析结果展示玩家、公会和科技点', () => {
    expect(saveView).toContain(':modifier-state="detailModifierState"')
    expect(saveView).toContain('api.modifier.getWorld(world.path)')
    expect(modal).toContain('modifierState.players')
    expect(modal).toContain('modifierState.guilds')
    expect(modal).toContain('普通科技点')
    expect(modal).toContain('古代科技点')
    expect(modal).toContain('公会')
  })

  it('点击调用按路径解析的世界摘要（修复本地玩家列表为空）', () => {
    expect(saveView).toContain('worldSummaryByPath(world.path)')
    expect(saveView).not.toContain('worldSummary(w.name)')
  })

  it('弹窗组件存在，并按来源区分动作按钮', () => {
    expect(modal).toContain('sdm-overlay')
    expect(modal).toContain("$emit('migrate'")
    expect(modal).toContain("$emit('setBackup'")
    // 本地（非 server）显示迁移；服务器显示设为备份目标
    expect(modal).toContain("world.source !== 'server'")
  })

  it('前端 API 暴露 worldSummaryByPath', () => {
    expect(apiTauri).toContain('worldSummaryByPath')
    expect(apiTauri).toContain("'f5_world_summary_by_path'")
  })

  it('后端实现按路径解析摘要（本地/服务器通用）', () => {
    expect(worldCopy).toContain('f5_world_summary_by_path_impl')
    expect(worldCopy).toContain('find_world_data_dir')
    expect(saveEdit).toContain('#[tauri::command]')
    expect(saveEdit).toContain('pub async fn f5_world_summary_by_path')
  })

  it('后端命令已注册', () => {
    expect(mainRs).toContain('save_edit::f5_world_summary_by_path')
  })
})
