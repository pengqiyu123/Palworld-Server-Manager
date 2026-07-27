import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const read = (path: string) => readFileSync(resolve(__dirname, '..', path), 'utf-8')
const saveManagement = read('src/views/SaveManagementView.vue')
const saveMigration = read('src/views/SaveMigrationView.vue')

describe('V4 世界存档统一设计', () => {
  it('不再出现旧角色跨服转移和预置存档区块', () => {
    expect(saveManagement).not.toContain('角色跨服转移')
    expect(saveManagement).not.toContain('预置存档（接口预留）')
  })

  it('本机与服务器世界使用相同的详情入口', () => {
    const binds = saveManagement.match(/@click="openWorld\(world\)"/g) ?? []
    expect(binds.length).toBeGreaterThanOrEqual(2)
    expect(saveManagement).toContain('worldSummaryByPath(world.path)')
  })

  it('世界备份采用 V4 两类列表', () => {
    expect(saveManagement).toContain('listFullBackups')
    expect(saveManagement).toContain('listSnapshots')
    expect(saveManagement).not.toContain('listWorldBackups')
  })
})

describe('V4 世界与角色迁移边界', () => {
  it('使用三个独立主任务', () => {
    expect(saveMigration).toContain("{ id: 'world', label: '迁移整个世界' }")
    expect(saveMigration).toContain("{ id: 'character', label: '转移完整角色' }")
    expect(saveMigration).toContain("{ id: 'guild', label: '恢复原公会' }")
  })

  it('不再调用旧合并迁移', () => {
    expect(saveMigration).not.toContain('migrateWorldV2')
    expect(saveMigration).not.toContain('run_phase_')
    expect(saveMigration).not.toContain('Fix Host Save')
  })

  it('完整角色与公会使用不同命令', () => {
    expect(saveMigration).toContain('transferFullCharacterV4')
    expect(saveMigration).toContain('restoreOriginalGuildV4')
    expect(saveMigration).toContain('公会关系不会改变')
  })

  it('角色选择使用单选项', () => {
    expect(saveMigration).toContain('type="radio"')
    expect(saveMigration).not.toContain('type="checkbox"')
  })

  it('朋友角色导入独立调用且不携带公会参数', () => {
    expect(saveMigration).toContain('importFriendCharacterV4')
    expect(saveMigration).toContain('不会更改任何公会关系')
  })
})
