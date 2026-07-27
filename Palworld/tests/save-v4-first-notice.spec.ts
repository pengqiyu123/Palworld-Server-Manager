import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(__dirname, '..')
const read = (path: string) => readFileSync(resolve(root, path), 'utf-8')

const migrationView = read('src/views/SaveMigrationView.vue')
const settingsStore = read('src/stores/settings.ts')
const settingsTypes = read('src/types/tauri.ts')
const rustSettings = read('src-tauri/src/settings.rs')

describe('V4 首次迁移说明', () => {
  it('将已读状态持久化，应用重启后不重复显示', () => {
    for (const source of [settingsStore, settingsTypes, rustSettings]) {
      expect(source).toContain('migration_backup_notice_seen')
    }
    expect(migrationView).toContain('settingsStore.settings.migration_backup_notice_seen')
    expect(migrationView).toContain('migration_backup_notice_seen: true')
    expect(migrationView).toContain('await settingsStore.save()')
  })

  it('按真实路径而非显示名称判断来源和目标是否相同', () => {
    expect(migrationView).toContain('target.path !== source.path')
    expect(migrationView).not.toContain('source.name !== targetWorldName.value')
  })
})
