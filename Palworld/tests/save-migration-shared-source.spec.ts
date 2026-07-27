import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import SaveMigrationView from '@/views/SaveMigrationView.vue'

const { apiMock, settingsMock } = vi.hoisted(() => ({
  apiMock: {
    save: {
      discoverWorlds: vi.fn(async () => ({ save_root: 'C:/srv/SaveGames', auto_discovered: false, worlds: [] })),
      discoverLocalWorlds: vi.fn(async () => []),
      getBackupRoot: vi.fn(async () => 'F:/1'),
      listWorkflows: vi.fn(async () => []),
    },
    server: { getStatus: vi.fn(async () => ({ running: false })) },
    migration: { onProgress: vi.fn(async () => () => undefined) },
  },
  settingsMock: {
    settings: {
      server_path: 'C:/srv',
      local_save_roots: ['F:/1'],
      server_save_roots: [],
      backup_root: 'F:/1',
      backup_roots: [],
    },
  },
}))

vi.mock('@/api/tauri', () => ({ api: apiMock }))
vi.mock('@/stores/settings', () => ({ useSettingsStore: () => settingsMock }))
vi.mock('vue-router', () => ({ useRoute: () => ({ query: {} }) }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('@/components/ui/useToast', () => ({ useToast: () => ({ info: vi.fn(), error: vi.fn(), success: vi.fn() }) }))

describe('数据迁移与本地存档发现共用来源', () => {
  beforeEach(() => vi.clearAllMocks())

  it('不再提供重复的手动本地目录入口，并排除被设为备份位置的扫描根', async () => {
    const wrapper = mount(SaveMigrationView, {
      global: {
        stubs: {
          AppIcon: true,
          ConfirmDialog: true,
          PlayerPicker: true,
          RouterLink: true,
          TechEditorPanel: true,
        },
      },
    })
    await flushPromises()

    expect(wrapper.text()).not.toContain('手动选择本地目录')
    expect(apiMock.save.discoverLocalWorlds).toHaveBeenCalledTimes(1)
    expect(apiMock.save.discoverLocalWorlds).toHaveBeenCalledWith()
    expect(apiMock.save.discoverLocalWorlds).not.toHaveBeenCalledWith('F:/1')
  })
})
