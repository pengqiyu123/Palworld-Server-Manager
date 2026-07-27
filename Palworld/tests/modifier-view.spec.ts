import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import ModifierView from '@/views/ModifierView.vue'

const { apiMock, serverMock, toastMock, worldState } = vi.hoisted(() => {
  const worldState = {
    world_name: '1A91A61548C7B6FD7B58B2B70710F7EE',
    server_running: false,
    game_running: false,
    players: [
      {
        player_uid: 'uid-alice',
        guid: 'guid-alice',
        nickname: 'Alice',
        level: 42,
        pal_count: 12,
        guild_id: 'guild-builders',
        guild_name: 'Builders',
        role: 'Admin',
        is_leader: true,
        last_online: '2026-07-26T10:00:00Z',
        technology_points: 30,
        ancient_technology_points: 8,
      },
    ],
    guilds: [
      {
        guild_id: 'guild-builders',
        name: 'Builders',
        leader_name: 'Alice',
        admin_player_uid: 'uid-alice',
        member_count: 4,
        base_count: 2,
        level: 7,
      },
    ],
  }

  return {
    worldState,
    serverMock: { status: { running: false } },
    toastMock: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
    apiMock: {
      modifier: {
        discoverWorlds: vi.fn(async () => [
          { name: 'Island One', path: 'C:/Pal/Saved/SaveGames/IslandOne' },
        ]),
        getWorld: vi.fn(async () => worldState),
        previewAction: vi.fn(async () => ({
          confirmation_name: 'Alice',
          player_count: 1,
          pal_count: 12,
          base_count: 0,
          file_count: 3,
          summary: '将删除玩家 Alice 及其关联存档。',
        })),
        applyAction: vi.fn(async () => ({
          ok: true,
          snapshot_id: 'snapshot-1',
          roundtrip_ok: true,
          message: '修改完成',
        })),
        onProgress: vi.fn(async () => () => {}),
      },
    },
  }
})

vi.mock('@/api/tauri', () => ({ api: apiMock }))
vi.mock('@/stores/server', () => ({ useServerStore: () => serverMock }))
vi.mock('@/components/ui/useToast', () => ({ useToast: () => toastMock }))

async function mountLoadedView() {
  const wrapper = mount(ModifierView, {
    attachTo: document.body,
    global: { stubs: { AppIcon: true } },
  })
  await flushPromises()
  await wrapper.get('[data-testid="world-select"]').setValue('C:/Pal/Saved/SaveGames/IslandOne')
  await flushPromises()
  return wrapper
}

async function openPlayerMenu(wrapper: Awaited<ReturnType<typeof mountLoadedView>>) {
  await wrapper.get('[data-testid="player-menu-trigger-uid-alice"]').trigger('click')
  await flushPromises()
}

describe('修改器工作台', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    serverMock.status.running = false
    apiMock.modifier.discoverWorlds.mockResolvedValue([
      { name: 'Island One', path: 'C:/Pal/Saved/SaveGames/IslandOne' },
    ])
    apiMock.modifier.getWorld.mockResolvedValue(worldState)
    apiMock.modifier.previewAction.mockResolvedValue({
      confirmation_name: 'Alice',
      player_count: 1,
      pal_count: 12,
      base_count: 0,
      file_count: 3,
      summary: '将删除玩家 Alice 及其关联存档。',
    })
    apiMock.modifier.applyAction.mockResolvedValue({
      ok: true,
      snapshot_id: 'snapshot-1',
      roundtrip_ok: true,
      message: '修改完成',
    })
    worldState.server_running = false
    worldState.game_running = false
  })

  it('按玩家和公会标签展示当前世界的真实表格数据', async () => {
    const wrapper = await mountLoadedView()

    expect(apiMock.modifier.discoverWorlds).toHaveBeenCalledTimes(1)
    expect(apiMock.modifier.getWorld).toHaveBeenCalledWith('C:/Pal/Saved/SaveGames/IslandOne')
    expect(wrapper.get('[data-testid="players-table"]').text()).toContain('Alice')
    expect(wrapper.get('[data-testid="players-table"]').text()).toContain('Builders')

    await wrapper.get('[data-testid="guilds-tab"]').trigger('click')
    expect(wrapper.get('[data-testid="guilds-table"]').text()).toContain('Builders')
    expect(wrapper.get('[data-testid="guilds-table"]').text()).toContain('Alice')
    wrapper.unmount()
  })

  it('默认界面使用可读世界名并隐藏 UID 与 GUID', async () => {
    const wrapper = await mountLoadedView()

    expect(wrapper.get('.workbench-head h2').text()).toBe('Island One')
    expect(wrapper.text()).not.toContain('1A91A61548C7B6FD7B58B2B70710F7EE')
    expect(wrapper.text()).not.toContain('uid-alice')
    expect(wrapper.text()).not.toContain('guild-builders')
    wrapper.unmount()
  })

  it('右键和省略号按钮打开同一个行操作菜单', async () => {
    const wrapper = await mountLoadedView()
    const row = wrapper.get('[data-testid="player-row-uid-alice"]')

    await row.trigger('contextmenu', { clientX: 180, clientY: 220 })
    expect(wrapper.findAll('[data-testid="row-action-menu"]')).toHaveLength(1)
    expect(wrapper.get('[data-testid="row-action-menu"]').text()).toContain('重命名玩家')

    await wrapper.get('[data-testid="player-menu-trigger-uid-alice"]').trigger('click')
    expect(wrapper.findAll('[data-testid="row-action-menu"]')).toHaveLength(1)
    expect(wrapper.get('[data-testid="row-action-menu"]').text()).toContain('删除玩家')
    wrapper.unmount()
  })

  it('为名称、等级和科技点提供对应输入对话框', async () => {
    const wrapper = await mountLoadedView()

    await openPlayerMenu(wrapper)
    await wrapper.get('[data-action="rename_player"]').trigger('click')
    expect(wrapper.get('[data-testid="name-input"]').attributes('type')).toBe('text')
    await wrapper.get('[data-testid="form-cancel"]').trigger('click')

    await openPlayerMenu(wrapper)
    await wrapper.get('[data-action="set_player_level"]').trigger('click')
    expect(wrapper.get('[data-testid="level-input"]').attributes('type')).toBe('number')
    await wrapper.get('[data-testid="form-cancel"]').trigger('click')

    await openPlayerMenu(wrapper)
    await wrapper.get('[data-action="set_technology_points"]').trigger('click')
    expect(wrapper.get('[data-testid="technology-points-input"]').element).toBeTruthy()
    expect(wrapper.get('[data-testid="ancient-technology-points-input"]').element).toBeTruthy()
    wrapper.unmount()
  })

  it('先预览删除影响，并要求输入精确名称后才能执行', async () => {
    const wrapper = await mountLoadedView()
    await openPlayerMenu(wrapper)
    await wrapper.get('[data-action="delete_player"]').trigger('click')
    await flushPromises()

    expect(apiMock.modifier.previewAction).toHaveBeenCalledWith({
      world_path: 'C:/Pal/Saved/SaveGames/IslandOne',
      action: 'delete_player',
      player_uid: 'uid-alice',
    })
    expect(wrapper.get('[data-testid="preview-dialog"]').text()).toContain('帕鲁12')
    expect(wrapper.get('[data-testid="preview-dialog"]').text()).toContain('文件3')
    expect(wrapper.get('[data-testid="apply-action"]').attributes()).toHaveProperty('disabled')

    await wrapper.get('[data-testid="confirmation-name-input"]').setValue('Alice')
    expect(wrapper.get('[data-testid="apply-action"]').attributes()).not.toHaveProperty('disabled')
    await wrapper.get('[data-testid="apply-action"]').trigger('click')
    await flushPromises()

    expect(apiMock.modifier.applyAction).toHaveBeenCalledWith({
      world_path: 'C:/Pal/Saved/SaveGames/IslandOne',
      action: 'delete_player',
      player_uid: 'uid-alice',
    })
    expect(apiMock.modifier.getWorld).toHaveBeenCalledTimes(2)
    expect(wrapper.get('[role="status"]').text()).toContain('修改完成')
    wrapper.unmount()
  })

  it('显示写入阶段，并在成功后刷新当前世界', async () => {
    let resolveApply: ((value: { ok: boolean; snapshot_id: string; roundtrip_ok: boolean; message: string }) => void) | undefined
    apiMock.modifier.applyAction.mockImplementationOnce(() => new Promise((resolve) => {
      resolveApply = resolve
    }))
    const wrapper = await mountLoadedView()

    await openPlayerMenu(wrapper)
    await wrapper.get('[data-action="rename_player"]').trigger('click')
    await wrapper.get('[data-testid="name-input"]').setValue('Alicia')
    await wrapper.get('[data-testid="form-submit"]').trigger('click')
    await flushPromises()
    expect(apiMock.modifier.previewAction).toHaveBeenCalledWith({
      world_path: 'C:/Pal/Saved/SaveGames/IslandOne',
      action: 'rename_player',
      player_uid: 'uid-alice',
      value: 'Alicia',
    })

    await wrapper.get('[data-testid="apply-action"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('[data-testid="modifier-progress-overlay"]').text()).toContain('检查游戏和服务器')

    const progressHandler = apiMock.modifier.onProgress.mock.calls[0]?.[0]
    progressHandler?.({ phase: 'creating_snapshot', label: '创建回滚点' })
    await flushPromises()
    expect(wrapper.get('[data-testid="modifier-progress-overlay"]').text()).toContain('创建回滚点')

    resolveApply?.({ ok: true, snapshot_id: 'snapshot-2', roundtrip_ok: true, message: '玩家名称已修改' })
    await flushPromises()
    expect(apiMock.modifier.getWorld).toHaveBeenCalledTimes(2)
    expect(wrapper.get('[role="status"]').text()).toContain('玩家名称已修改')
    wrapper.unmount()
  })

  it('服务器运行时保留查看能力并禁用全部写入操作', async () => {
    serverMock.status.running = true
    const wrapper = await mountLoadedView()
    await openPlayerMenu(wrapper)

    expect(wrapper.text()).toContain('服务器正在运行')
    for (const button of wrapper.findAll('[data-testid="row-action-menu"] button')) {
      expect(button.attributes()).toHaveProperty('disabled')
    }
    expect(apiMock.modifier.previewAction).not.toHaveBeenCalled()
    expect(apiMock.modifier.applyAction).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('游戏客户端运行时同样保留查看能力并禁用写入', async () => {
    worldState.game_running = true
    const wrapper = await mountLoadedView()
    await openPlayerMenu(wrapper)

    expect(wrapper.text()).toContain('游戏客户端正在运行')
    for (const button of wrapper.findAll('[data-testid="row-action-menu"] button')) {
      expect(button.attributes()).toHaveProperty('disabled')
    }
    expect(apiMock.modifier.previewAction).not.toHaveBeenCalled()
    expect(apiMock.modifier.applyAction).not.toHaveBeenCalled()
    wrapper.unmount()
  })
})
