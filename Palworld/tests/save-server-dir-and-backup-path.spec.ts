// TDD 守卫：服务器存档手动选目录 + 备份/恢复可指定文件夹路径
// 风格与现有 save-*.spec.ts 一致：直接读源码字符串做断言，不依赖运行时。

import { readFileSync } from 'fs'
import { resolve } from 'path'

const saveView = readFileSync(resolve(__dirname, '../src/views/SaveManagementView.vue'), 'utf-8')
const apiSrc = readFileSync(resolve(__dirname, '../src/api/tauri.ts'), 'utf-8')

describe('本地存档/服务器存档 手动选目录一致', () => {
  it('服务器存档区（区块B）也有「手动选择目录」兜底按钮，与本地单机一致', () => {
    // 本地单机已存在 onPickLocalDir；服务器区应新增对称的 onPickServerDir
    expect(saveView).toContain('onPickLocalDir')
    expect(saveView).toContain('onPickServerDir')
    // 服务器区提示里应出现「手动选择目录」文案（已在区块B内）
    expect(saveView).toContain('手动选择目录…')
  })

  it('discoverWorlds 后端命令支持额外根（extraRoot）参数', () => {
    expect(apiSrc).toContain('discoverWorlds: (extraRoot?')
    expect(apiSrc).toContain("'discover_worlds'")
  })
})

describe('世界备份/恢复 可指定文件夹路径存放', () => {
  it('备份区有「选择存放目录」拾取器，并把路径传给 backupWorld(dest)', () => {
    expect(saveView).toContain('onPickBackupDir')
    expect(saveView).toContain('backupDest')
    // backupWorld 调用应传入所选世界的真实路径（selectedWorld.path）+ 自定义目标（dest 非空时）
    // 注：原 selectedServerName 已重构为 selectedWorld（WorldInfo|null），按真实路径操作。
    expect(saveView).toContain('backupWorld(selectedWorld.value.path, backupDest.value')
  })

  it('恢复区有「从自定义目录恢复」入口，调用 restoreWorldFrom', () => {
    expect(saveView).toContain('onRestoreFromDir')
    expect(saveView).toContain('从自定义目录恢复')
    expect(apiSrc).toContain('restoreWorldFrom')
  })
})
