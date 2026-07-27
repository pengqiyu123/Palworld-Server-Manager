import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const saveView = readFileSync(resolve(__dirname, '../src/views/SaveManagementView.vue'), 'utf-8')
const apiSource = readFileSync(resolve(__dirname, '../src/api/tauri.ts'), 'utf-8')

describe('世界目录手动选择', () => {
  it('本机与服务器世界共用目录选择入口', () => {
    expect(saveView).toContain("pickWorldDirectory('local')")
    expect(saveView).toContain("pickWorldDirectory('server')")
    expect(apiSource).toContain('discoverWorlds: (extraRoot?')
  })
})

describe('V4 备份位置与恢复', () => {
  it('允许更改后续备份位置并保留历史根', () => {
    expect(saveView).toContain('pickBackupRoot')
    expect(saveView).toContain('backup_root: directory')
    expect(saveView).toContain('settingsStore.save()')
    expect(apiSource).toContain("'backup_get_root'")
  })

  it('只恢复索引中列出的完整备份或操作回滚点', () => {
    expect(saveView).toContain('restoreFullBackup(item.id)')
    expect(saveView).toContain('restoreSnapshot(item.id)')
    expect(saveView).not.toContain('restoreWorldFrom')
  })
})
