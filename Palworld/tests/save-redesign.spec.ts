/**
 * TDD 红绿测试：本地/服务器存档统一设计 + 迁移重设计（老板 Q3/Q4/Q6）
 * 风格与现有 tests/*.spec.ts 一致：直接读源码字符串做断言（不依赖组件挂载）。
 * 运行：npx vitest run tests/save-redesign.spec.ts
 */
import { readFileSync } from 'fs'
import { resolve } from 'path'

const SAVE_MGMT = readFileSync(
  resolve(__dirname, '../src/views/SaveManagementView.vue'),
  'utf-8',
)
const SAVE_MIG = readFileSync(
  resolve(__dirname, '../src/views/SaveMigrationView.vue'),
  'utf-8',
)

describe('SaveManagementView（Q3 统一设计 + Q6 删除）', () => {
  it('Q6: 已删除「角色跨服转移」区块', () => {
    expect(SAVE_MGMT).not.toContain('角色跨服转移')
  })

  it('Q6: 已删除「预置存档（接口预留）」区块', () => {
    expect(SAVE_MGMT).not.toContain('预置存档（接口预留）')
  })

  it('Q6: 已移除 onExport / onImport 导出导入回调', () => {
    expect(SAVE_MGMT).not.toContain('onExport')
    expect(SAVE_MGMT).not.toContain('onImport')
    expect(SAVE_MGMT).not.toContain('onPresetReserved')
  })

  it('Q3: 服务器世界卡也绑定 onToggleExpand（修复「世界0没角色」）', () => {
    // 本地 + 服务器两处 world 卡都应调用 onToggleExpand(w)
    const binds = (SAVE_MGMT.match(/@click="onToggleExpand\(w\)"/g) || []).length
    expect(binds).toBeGreaterThanOrEqual(2)
  })
})

describe('SaveMigrationView（Q4 重设计 + Q6 删除）', () => {
  it('Q4: 英文标签「Fix Host Save」已改为中文「修复主机存档」', () => {
    expect(SAVE_MIG).not.toContain('Fix Host Save')
    expect(SAVE_MIG).toContain('修复主机存档')
  })

  it('Q6: 已删除「跨服角色转移」区块', () => {
    expect(SAVE_MIG).not.toContain('跨服角色转移')
  })

  it('Q6: 已移除 TransferSubsetSelector 组件与 import', () => {
    expect(SAVE_MIG).not.toContain('TransferSubsetSelector')
    expect(SAVE_MIG).not.toContain('TransferSubset')
  })

  it('Q4: 顺序应为「整包世界迁移（②）」在「修复主机存档（①）」之前', () => {
    const iWorld = SAVE_MIG.indexOf('整包世界迁移')
    const iFix = SAVE_MIG.indexOf('修复主机存档')
    expect(iWorld).toBeGreaterThan(-1)
    expect(iFix).toBeGreaterThan(-1)
    expect(iWorld).toBeLessThan(iFix)
  })

  it('Q4: 修复主机存档改用 PlayerPicker 角色卡选择，不再手填 GUID', () => {
    expect(SAVE_MIG).toContain('PlayerPicker')
    // 旧的手动 GUID 文本输入应移除
    expect(SAVE_MIG).not.toContain('旧主机角色 GUID')
    expect(SAVE_MIG).not.toContain('专用服新角色 GUID')
  })
})
