/**
 * 独立 QA 验证：本地存档页「世界备份/恢复」改动（去 P0 标签 + 真实路径操作 + 世界选择器）。
 * 采用运行时挂载 SaveManagementView（jsdom + @vue/test-utils），mock @/api/tauri 与 vue-router，
 * 断言三点新 UI 行为。运行：npx vitest run tests/save-backup-choice.spec.ts
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'
import SaveManagementView from '@/views/SaveManagementView.vue'

// --- 被 mock 的 API 与测试数据（vi.hoisted 保证在 vi.mock 工厂前可用） ---
const { apiMock, serverWorld, localWorld } = vi.hoisted(() => {
  const serverWorld = {
    name: 'ServerWorld1',
    path: 'C:/srv/SaveGames/ServerWorld1',
    source: 'server',
    has_level_sav: true,
    player_count: 2,
    size_bytes: 1000,
    guid: null,
    modified_at: null,
  }
  const localWorld = {
    name: 'LocalWorld1',
    path: 'C:/Users/x/AppData/Local/Pal/Saved/SaveGames/LocalWorld1',
    source: 'appdata',
    has_level_sav: true,
    player_count: 1,
    size_bytes: 500,
    guid: null,
    modified_at: null,
  }
  const serverRes = {
    worlds: [serverWorld],
    save_root: 'C:/srv/SaveGames',
    auto_discovered: false,
  }
  return {
    serverWorld,
    localWorld,
    apiMock: {
      save: {
        discoverWorlds: vi.fn(async () => serverRes),
        discoverLocalWorlds: vi.fn(async () => [localWorld]),
        listWorldBackups: vi.fn(async () => []),
        backupWorld: vi.fn(async () => '已备份世界成功'),
        restoreWorld: vi.fn(async () => 'ok'),
        restoreWorldFrom: vi.fn(async () => 'ok'),
      },
      migration: { worldSummaryByPath: vi.fn(async () => null) },
    },
  }
})

// 必须 mock 的依赖
vi.mock('@/api/tauri', () => ({ api: apiMock }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('@/components/ui/useToast', () => ({
  useToast: () => ({ success: vi.fn(), info: vi.fn(), error: vi.fn(), warn: vi.fn() }),
}))
vi.mock('@/stores/settings', () => ({
  useSettingsStore: () => ({ settings: { server_path: '' } }),
}))

async function mountAndSettle() {
  const wrapper = mount(SaveManagementView, {
    global: {
      stubs: { AppIcon: true, ConfirmDialog: true, SaveDetailModal: true },
    },
  })
  // 等 onDiscover()（Promise.all(discoverWorlds, discoverLocalWorlds) + onSelectWorld）落地
  await flushPromises()
  await new Promise((r) => setTimeout(r, 0))
  return wrapper
}

describe('本地存档 · 世界备份/恢复改动', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('a) 备份区已去掉「（P0）」标签', async () => {
    const wrapper = await mountAndSettle()
    const html = wrapper.html()
    expect(html).not.toContain('（P0）')
    // 备份区标题仍正常呈现，且不含 P0
    const title = wrapper.find('.section-title')
    expect(title.exists()).toBe(true)
    expect(title.text()).not.toContain('（P0）')
  })

  it('b) 备份区存在 <select> 世界选择器，options 同时含「服务器」与「本地单机」世界（含来源标签）', async () => {
    const wrapper = await mountAndSettle()
    const select = wrapper.find('select.world-pick')
    expect(select.exists()).toBe(true)

    const options = select.findAll('option')
    expect(options.length).toBe(2)

    const text = select.text()
    // server → 专用服；appdata → AppData 单机（sourceLabel 映射）
    expect(text).toContain('专用服')
    expect(text).toContain('AppData 单机')

    // 两个世界路径都应作为 option 的 value 出现（真实路径，而非名字）
    const values = options.map((o) => o.attributes('value'))
    expect(values).toContain(serverWorld.path)
    expect(values).toContain(localWorld.path)
  })

  it('c) 选中某世界后点「备份当前世界」，backupWorld 以该世界的真实 .path 为第一参数（而非名字）', async () => {
    const wrapper = await mountAndSettle()

    // 默认选中第一个服务器世界；这里切换到本地单机世界，验证按真实路径操作
    const select = wrapper.find('select.world-pick')
    await select.setValue(localWorld.path)
    await flushPromises()

    // 点击「备份当前世界」按钮（btn-primary 在本页唯一）
    const backupBtn = wrapper.find('button.btn-primary')
    expect(backupBtn.exists()).toBe(true)
    await backupBtn.trigger('click')
    await flushPromises()

    expect(apiMock.save.backupWorld).toHaveBeenCalledTimes(1)
    const call = apiMock.save.backupWorld.mock.calls[0]
    // 第一参数应为该世界的真实路径
    expect(call[0]).toBe(localWorld.path)
    // 显式确认不是世界名（证明已改为按真实路径操作）
    expect(call[0]).not.toBe(localWorld.name)
    expect(call[0]).not.toBe(serverWorld.name)
  })
})
