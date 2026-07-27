import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(__dirname, '..')
const read = (path: string) => readFileSync(resolve(root, path), 'utf-8')

const migrationView = read('src/views/SaveMigrationView.vue')
const managementView = read('src/views/SaveManagementView.vue')
const apiSource = read('src/api/tauri.ts')
const typesSource = read('src/types/tauri.ts')
const routerSource = read('src/router/index.ts')
const sidebarSource = read('src/components/layout/Sidebar.vue')

describe('V4 存档迁移产品契约', () => {
  it('只通过 typed Tauri API 调用所有 V4 命令', () => {
    for (const command of [
      'migrate_world_v4',
      'transfer_full_character_v4',
      'import_friend_character_v4',
      'restore_original_guild_v4',
      'complete_migration_workflow_v4',
      'rollback_migration_workflow_v4',
      'backup_create_full',
      'backup_get_root',
      'backup_list_full',
      'backup_delete_full',
      'backup_restore_full',
      'backup_list_snapshots',
      'backup_restore_snapshot',
      'backup_delete_snapshot',
      'backup_load_workflow',
      'backup_list_workflows',
      'backup_rebuild_index',
    ]) {
      expect(apiSource).toContain(`'${command}'`)
    }
    expect(typesSource).toContain('export interface BackupManifest')
    expect(typesSource).toContain('export interface MigrationWorkflow')
    expect(typesSource).toContain('backup_roots: string[]')
  })

  it('将世界、完整角色和原公会明确拆开', () => {
    expect(migrationView).toContain('迁移整个世界')
    expect(migrationView).toContain('转移完整角色')
    expect(migrationView).toContain('恢复原公会')
    expect(migrationView).toContain('公会关系不会改变')
    expect(migrationView).toContain('api.migration.transferFullCharacterV4')
    expect(migrationView).toContain('api.migration.restoreOriginalGuildV4')
    expect(migrationView).not.toContain('api.migration.migrateWorldV2')
  })

  it('世界迁移不会激活来源 WorldOption 并明确保留服务器配置', () => {
    expect(migrationView).toContain('来源世界规则不会直接应用')
    expect(migrationView).toContain('由“服务器配置”继续管理')
    expect(migrationView).not.toContain('使用来源世界设置')
    expect(migrationView).not.toContain('v-model="preserveServerConfig"')
    expect(migrationView).toContain('preserve_server_config: true')
  })

  it('公会恢复请求只携带 request_id 与 workflow_id', () => {
    const request = migrationView.match(/restoreOriginalGuildV4\(\{([\s\S]*?)\}\)/)?.[1] ?? ''
    expect(request).toContain('request_id:')
    expect(request).toContain('workflow_id:')
    expect(request).not.toMatch(/guild|permission|admin_player/i)
  })

  it('朋友角色导入独立且不提供公会恢复', () => {
    expect(migrationView).toContain('导入朋友角色')
    expect(migrationView).toContain('api.migration.importFriendCharacterV4')
    expect(migrationView).toContain('friend-transfer-panel')
    const friendPanel = migrationView.match(/<section v-else class="task-panel friend-transfer-panel"([\s\S]*?)<\/section>/)?.[1] ?? ''
    expect(friendPanel).not.toContain('startGuildRestore')
    expect(friendPanel).not.toContain('恢复原公会')
  })

  it('默认备份只说明项目或程序旁的 backups 目录', () => {
    expect(migrationView).toContain('api.save.getBackupRoot()')
    expect(migrationView).toContain('backupRoot')
    expect(migrationView).toContain('开始迁移')
    expect(migrationView).not.toMatch(/LOCALAPPDATA/i)
    expect(managementView).not.toMatch(/LOCALAPPDATA/i)
  })

  it('用户界面不暴露旧迁移内部术语', () => {
    const visibleTemplates = [migrationView.split('<script setup')[0], managementView.split('<script setup')[0]]
      .join('\n')
      .replace(/\{\{[\s\S]*?\}\}/g, '')
    expect(visibleTemplates).not.toMatch(/>[^<]*(Fix Host|Phase|UID|GUID|_system|snapshot|manifest|workflow)[^<]*</i)
  })
})

describe('V4 世界存档与备份入口', () => {
  it('世界存档页包含两个页签与两类备份列表', () => {
    expect(managementView).toContain('世界存档')
    expect(managementView).toContain('备份与回滚')
    expect(managementView).toContain('完整备份')
    expect(managementView).toContain('操作回滚点')
    expect(managementView).toContain('保留至手动删除')
  })

  it('/backup 重定向到世界存档的备份页签', () => {
    expect(routerSource).toContain("redirect: { path: '/saves', query: { tab: 'backup' } }")
    expect(sidebarSource).toContain("label: '世界存档'")
    expect(sidebarSource).toContain("label: '世界与角色迁移'")
    expect(sidebarSource).not.toContain("label: '配置备份'")
  })

  it('角色选择使用 radio 单选，不再使用 checkbox', () => {
    expect(migrationView).toContain('type="radio"')
    expect(migrationView).not.toContain('type="checkbox"')
  })
})
