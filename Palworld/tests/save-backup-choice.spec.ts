import { describe, expect, it, vi, beforeEach } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import SaveManagementView from '@/views/SaveManagementView.vue'

const { apiMock, settingsMock, serverWorld, localWorld } = vi.hoisted(() => {
  const serverWorld = {
    name: 'ServerWorld1', path: 'C:/srv/SaveGames/ServerWorld1', source: 'server',
    has_level_sav: true, player_count: 2, size_bytes: 1000, guid: 'server-world', modified_at: null,
  }
  const localWorld = {
    name: 'LocalWorld1', path: 'C:/Users/x/Pal/LocalWorld1', source: 'appdata',
    has_level_sav: true, player_count: 1, size_bytes: 500, guid: 'local-world', modified_at: null,
  }
  const settings = {
    server_path: 'C:/srv', local_save_roots: [], server_save_roots: [],
    backup_root: '', backup_roots: [],
  }
  return {
    serverWorld,
    localWorld,
    settingsMock: {
      settings,
      update: vi.fn((patch: Record<string, unknown>) => Object.assign(settings, patch)),
      save: vi.fn(async () => undefined),
    },
    apiMock: {
      server: { getStatus: vi.fn(async () => ({ running: false })), stop: vi.fn() },
      save: {
        discoverWorlds: vi.fn(async () => ({ worlds: [serverWorld], save_root: 'C:/srv/SaveGames', auto_discovered: false })),
        discoverLocalWorlds: vi.fn(async () => [localWorld]),
        getBackupRoot: vi.fn(async () => 'F:/app/backups'),
        listFullBackups: vi.fn(async () => []),
        listSnapshots: vi.fn(async () => []),
        createFullBackup: vi.fn(async () => ({})),
        rebuildBackupIndex: vi.fn(async () => ({ backups: [] })),
      },
      migration: { worldSummaryByPath: vi.fn(async () => ({ players: [], guilds: [] })) },
    },
  }
})

vi.mock('@/api/tauri', () => ({ api: apiMock }))
vi.mock('@/stores/settings', () => ({ useSettingsStore: () => settingsMock }))
vi.mock('vue-router', () => ({
  useRoute: () => ({ query: { tab: 'backup' } }),
  useRouter: () => ({ replace: vi.fn(async () => undefined), push: vi.fn(async () => undefined) }),
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))

async function mountAndSettle() {
  const wrapper = mount(SaveManagementView, {
    global: {
      stubs: {
        AppIcon: true,
        ConfirmDialog: true,
        SaveDetailModal: true,
        RouterLink: true,
      },
    },
  })
  await flushPromises()
  return wrapper
}

describe('V4 备份页选择与创建', () => {
  beforeEach(() => vi.clearAllMocks())

  it('展示真实备份根和两类列表，不含旧阶段标签', async () => {
    const wrapper = await mountAndSettle()
    expect(wrapper.text()).toContain('F:/app/backups')
    expect(wrapper.text()).toContain('完整备份')
    expect(wrapper.text()).toContain('操作回滚点')
    expect(wrapper.text()).not.toContain('（P0）')
  })

  it('创建备份的世界选择器同时包含服务器与本机世界', async () => {
    const wrapper = await mountAndSettle()
    const select = wrapper.find('.create-controls select')
    const options = select.findAll('option')
    expect(options).toHaveLength(3)
    expect(select.text()).toContain('ServerWorld1')
    expect(select.text()).toContain('LocalWorld1')
  })

  it('创建完整备份使用选中世界的真实位置和 V4 元数据', async () => {
    const wrapper = await mountAndSettle()
    await wrapper.find('.create-controls select').setValue(localWorld.path)
    await wrapper.find('.create-controls .btn-primary').trigger('click')
    await flushPromises()

    expect(apiMock.save.createFullBackup).toHaveBeenCalledWith(
      localWorld.path,
      localWorld.guid,
      localWorld.name,
      'local',
      'manual',
    )
  })
})
