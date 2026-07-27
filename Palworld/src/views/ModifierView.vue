<template>
  <section class="screen active modifier-view">
    <div class="page-head">
      <div>
        <div class="page-title">修改器</div>
        <div class="page-sub">管理世界中的玩家与公会。所有修改都会先显示影响范围并创建回滚点。</div>
      </div>
      <div class="head-actions">
        <label class="world-field" for="modifier-world">
          <span>服务器世界</span>
          <select
            id="modifier-world"
            v-model="selectedWorldPath"
            data-testid="world-select"
            :disabled="loadingWorlds || writing"
            @change="loadCurrentWorld"
          >
            <option value="">选择世界</option>
            <option v-for="world in worlds" :key="world.path" :value="world.path">{{ world.name }}</option>
          </select>
        </label>
        <button class="btn btn-ghost" :disabled="loadingWorlds || loadingWorld || writing" @click="refresh">
          {{ loadingWorlds || loadingWorld ? '刷新中...' : '刷新' }}
        </button>
      </div>
    </div>

    <div v-if="writeBlockMessage" class="modifier-warning">
      <AppIcon name="info" :size="16" />
      <span>{{ writeBlockMessage }}</span>
    </div>

    <p v-if="operationStatus" class="operation-status" role="status">{{ operationStatus }}</p>
    <p v-if="worldError" class="panel-error" role="alert">{{ worldError }}</p>

    <div
      v-if="writing"
      class="modifier-progress-overlay"
      data-testid="modifier-progress-overlay"
      role="status"
      aria-live="polite"
    >
      <div class="modifier-progress-panel">
        <span class="progress-spinner" aria-hidden="true" />
        <div>
          <strong>{{ currentProgressLabel }}</strong>
          <p>请保持应用开启，完成前不要启动游戏或服务器。</p>
        </div>
      </div>
    </div>

    <div class="modifier-workbench">
      <div class="workbench-head">
        <div>
          <span class="eyebrow">当前世界</span>
          <h2>{{ selectedWorldName || '尚未选择' }}</h2>
        </div>
        <div class="entity-tabs" role="tablist" aria-label="修改器数据类型">
          <button
            type="button"
            role="tab"
            data-testid="players-tab"
            :aria-selected="activeTab === 'players'"
            :class="{ active: activeTab === 'players' }"
            @click="activeTab = 'players'"
          >
            玩家 <span>{{ worldState?.players.length ?? 0 }}</span>
          </button>
          <button
            type="button"
            role="tab"
            data-testid="guilds-tab"
            :aria-selected="activeTab === 'guilds'"
            :class="{ active: activeTab === 'guilds' }"
            @click="activeTab = 'guilds'"
          >
            公会 <span>{{ worldState?.guilds.length ?? 0 }}</span>
          </button>
        </div>
      </div>

      <div v-if="loadingWorld" class="empty-state">正在读取世界存档...</div>
      <div v-else-if="!selectedWorldPath" class="empty-state">选择服务器世界后即可查看玩家和公会。</div>
      <template v-else-if="worldState">
        <div v-if="activeTab === 'players'" class="table-wrap" data-testid="players-table">
          <table>
            <thead>
              <tr>
                <th>玩家</th>
                <th>等级</th>
                <th>公会</th>
                <th>身份</th>
                <th>帕鲁</th>
                <th>科技点</th>
                <th>最后上线</th>
                <th class="actions-column"><span class="sr-only">操作</span></th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="player in worldState.players"
                :key="player.player_uid"
                :data-testid="`player-row-${player.player_uid}`"
                @contextmenu.prevent.stop="openContextMenu({ kind: 'player', player }, $event)"
              >
                <td>
                  <strong>{{ player.nickname || '未命名玩家' }}</strong>
                </td>
                <td>{{ player.level }}</td>
                <td>{{ player.guild_name || '未加入' }}</td>
                <td>{{ player.is_leader ? '会长' : player.role || '成员' }}</td>
                <td>{{ player.pal_count }}</td>
                <td>{{ player.technology_points }} / {{ player.ancient_technology_points }}</td>
                <td>{{ formatLastOnline(player.last_online) }}</td>
                <td class="actions-column">
                  <button
                    type="button"
                    class="menu-trigger"
                    :data-testid="`player-menu-trigger-${player.player_uid}`"
                    :aria-label="`打开 ${player.nickname || '未命名玩家'} 操作菜单`"
                    title="更多操作"
                    @click.stop="openButtonMenu({ kind: 'player', player }, $event)"
                  >
                    &#8942;
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
          <div v-if="worldState.players.length === 0" class="empty-state">这个世界中没有玩家。</div>
        </div>

        <div v-else class="table-wrap" data-testid="guilds-table">
          <table>
            <thead>
              <tr>
                <th>公会</th>
                <th>等级</th>
                <th>会长</th>
                <th>成员</th>
                <th>据点</th>
                <th class="actions-column"><span class="sr-only">操作</span></th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="guild in worldState.guilds"
                :key="guild.guild_id"
                :data-testid="`guild-row-${guild.guild_id}`"
                @contextmenu.prevent.stop="openContextMenu({ kind: 'guild', guild }, $event)"
              >
                <td>
                  <strong>{{ guild.name || '未命名公会' }}</strong>
                </td>
                <td>{{ guild.level }}</td>
                <td>{{ guild.leader_name || '未知' }}</td>
                <td>{{ guild.member_count }}</td>
                <td>{{ guild.base_count }}</td>
                <td class="actions-column">
                  <button
                    type="button"
                    class="menu-trigger"
                    :data-testid="`guild-menu-trigger-${guild.guild_id}`"
                    :aria-label="`打开 ${guild.name || '未命名公会'} 操作菜单`"
                    title="更多操作"
                    @click.stop="openButtonMenu({ kind: 'guild', guild }, $event)"
                  >
                    &#8942;
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
          <div v-if="worldState.guilds.length === 0" class="empty-state">这个世界中没有公会。</div>
        </div>
      </template>
    </div>

    <div
      v-if="menuTarget"
      class="row-action-menu"
      data-testid="row-action-menu"
      :style="{ left: `${menuPosition.left}px`, top: `${menuPosition.top}px` }"
      role="menu"
      @click.stop
    >
      <button
        v-for="action in availableActions"
        :key="action.action"
        type="button"
        role="menuitem"
        :data-action="action.action"
        :class="{ danger: action.danger }"
        :disabled="writeBlocked || writing || previewLoading"
        @click="selectAction(action.action)"
      >
        {{ action.label }}
      </button>
    </div>

    <div v-if="formDialog" class="dialog-backdrop" @click.self="closeFormDialog">
      <div class="modifier-dialog" role="dialog" aria-modal="true" :aria-labelledby="'modifier-form-title'">
        <h3 id="modifier-form-title">{{ formDialog.title }}</h3>
        <p class="dialog-copy">{{ formDialog.entityName }}</p>

        <label v-if="isNameAction(formDialog.action)" class="dialog-field">
          <span>新名称</span>
          <input v-model="formName" data-testid="name-input" type="text" maxlength="64" autocomplete="off">
        </label>
        <label v-else-if="formDialog.action === 'set_player_level'" class="dialog-field">
          <span>玩家等级</span>
          <input v-model.number="formLevel" data-testid="level-input" type="number" min="1" max="999" step="1">
        </label>
        <div v-else class="technology-fields">
          <label class="dialog-field">
            <span>普通科技点</span>
            <input
              v-model.number="formTechnologyPoints"
              data-testid="technology-points-input"
              type="number"
              min="0"
              :max="MAX_TECHNOLOGY_POINTS"
              step="1"
            >
          </label>
          <label class="dialog-field">
            <span>古代科技点</span>
            <input
              v-model.number="formAncientTechnologyPoints"
              data-testid="ancient-technology-points-input"
              type="number"
              min="0"
              :max="MAX_TECHNOLOGY_POINTS"
              step="1"
            >
          </label>
        </div>

        <p v-if="formError" class="panel-error" role="alert">{{ formError }}</p>
        <div class="dialog-actions">
          <button type="button" class="btn btn-ghost" data-testid="form-cancel" @click="closeFormDialog">取消</button>
          <button
            type="button"
            class="btn btn-primary"
            data-testid="form-submit"
            :disabled="previewLoading"
            @click="submitForm"
          >
            {{ previewLoading ? '正在预览...' : '预览更改' }}
          </button>
        </div>
      </div>
    </div>

    <div v-if="previewDialog" class="dialog-backdrop" @click.self="closePreviewDialog">
      <div
        class="modifier-dialog preview-dialog"
        data-testid="preview-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="modifier-preview-title"
      >
        <h3 id="modifier-preview-title">{{ previewDialog.destructive ? '确认危险操作' : '确认修改' }}</h3>
        <p class="preview-summary">{{ previewDialog.preview.summary }}</p>
        <dl class="impact-grid">
          <div><dt>玩家</dt><dd>{{ previewDialog.preview.player_count }}</dd></div>
          <div><dt>帕鲁</dt><dd>{{ previewDialog.preview.pal_count }}</dd></div>
          <div><dt>据点</dt><dd>{{ previewDialog.preview.base_count }}</dd></div>
          <div><dt>文件</dt><dd>{{ previewDialog.preview.file_count }}</dd></div>
        </dl>

        <label v-if="previewDialog.destructive" class="dialog-field confirmation-field">
          <span>输入“{{ previewDialog.preview.confirmation_name }}”确认</span>
          <input
            v-model="confirmationName"
            data-testid="confirmation-name-input"
            type="text"
            autocomplete="off"
          >
        </label>

        <div class="dialog-actions">
          <button type="button" class="btn btn-ghost" :disabled="writing" @click="closePreviewDialog">取消</button>
          <button
            type="button"
            class="btn"
            data-testid="apply-action"
            :class="previewDialog.destructive ? 'btn-danger-ghost' : 'btn-primary'"
            :disabled="!canApplyPreview"
            @click="applyPreviewedAction"
          >
            {{ writing ? '正在执行...' : previewDialog.destructive ? '确认执行' : '应用修改' }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import AppIcon from '@/components/ui/AppIcon.vue'
import { api } from '@/api/tauri'
import { useServerStore } from '@/stores/server'
import { useToast } from '@/components/ui/useToast'
import type {
  ModifierAction,
  ModifierActionPreview,
  ModifierActionRequest,
  ModifierGuild,
  ModifierPlayer,
  ModifierWorldEntry,
  ModifierWorldState,
  ModifierOperationProgress,
} from '@/types/tauri'

type MenuTarget =
  | { kind: 'player'; player: ModifierPlayer }
  | { kind: 'guild'; guild: ModifierGuild }

interface ActionOption {
  action: ModifierAction
  label: string
  danger?: boolean
}

interface FormDialog {
  action: ModifierAction
  title: string
  entityName: string
  target: MenuTarget
}

interface PreviewDialog {
  request: ModifierActionRequest
  preview: ModifierActionPreview
  destructive: boolean
}

const MAX_TECHNOLOGY_POINTS = 9_999_999
const PLAYER_ACTIONS: ActionOption[] = [
  { action: 'rename_player', label: '重命名玩家' },
  { action: 'set_player_level', label: '修改等级' },
  { action: 'set_technology_points', label: '修改科技点' },
  { action: 'unlock_all_technologies', label: '解锁全部科技' },
  { action: 'make_guild_leader', label: '设为公会会长' },
  { action: 'delete_player', label: '删除玩家', danger: true },
]
const GUILD_ACTIONS: ActionOption[] = [
  { action: 'rename_guild', label: '重命名公会' },
  { action: 'delete_guild', label: '删除公会', danger: true },
]
const DESTRUCTIVE_ACTIONS = new Set<ModifierAction>(['delete_player', 'delete_guild'])

const serverStore = useServerStore()
const toast = useToast()
const modifierApi = api.modifier

const worlds = ref<ModifierWorldEntry[]>([])
const selectedWorldPath = ref('')
const worldState = ref<ModifierWorldState | null>(null)
const activeTab = ref<'players' | 'guilds'>('players')
const loadingWorlds = ref(false)
const loadingWorld = ref(false)
const writing = ref(false)
const previewLoading = ref(false)
const worldError = ref('')
const operationStatus = ref('')
const currentProgressLabel = ref('检查游戏和服务器')

const menuTarget = ref<MenuTarget | null>(null)
const menuPosition = ref({ left: 0, top: 0 })
const formDialog = ref<FormDialog | null>(null)
const previewDialog = ref<PreviewDialog | null>(null)
const confirmationName = ref('')
const formName = ref('')
const formLevel = ref(1)
const formTechnologyPoints = ref(0)
const formAncientTechnologyPoints = ref(0)
const formError = ref('')
let unlistenProgress: (() => void) | null = null

const selectedWorldName = computed(() =>
  worlds.value.find((world) => world.path === selectedWorldPath.value)?.name ?? '',
)
const writeBlocked = computed(() => Boolean(
  serverStore.status.running
  || worldState.value?.server_running
  || worldState.value?.game_running,
))
const writeBlockMessage = computed(() => {
  const serverRunning = serverStore.status.running || worldState.value?.server_running
  const gameRunning = worldState.value?.game_running
  if (serverRunning && gameRunning) {
    return '服务器和游戏客户端正在运行。当前数据仍可查看，全部关闭后才能修改。'
  }
  if (serverRunning) return '服务器正在运行。当前数据仍可查看，停止服务器后才能修改。'
  if (gameRunning) return '游戏客户端正在运行。当前数据仍可查看，退出游戏后才能修改。'
  return ''
})
const availableActions = computed(() => menuTarget.value?.kind === 'guild' ? GUILD_ACTIONS : PLAYER_ACTIONS)
const canApplyPreview = computed(() => {
  if (!previewDialog.value || writing.value || writeBlocked.value) return false
  if (!previewDialog.value.destructive) return true
  return confirmationName.value === previewDialog.value.preview.confirmation_name
})

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function formatLastOnline(value: string | null): string {
  if (!value) return '未知'
  const timestamp = Date.parse(value)
  return Number.isNaN(timestamp) ? value : new Date(timestamp).toLocaleString('zh-CN')
}

function isNameAction(action: ModifierAction): boolean {
  return action === 'rename_player' || action === 'rename_guild'
}

function isValidInteger(value: number, minimum: number, maximum: number): boolean {
  return Number.isInteger(value) && value >= minimum && value <= maximum
}

async function loadWorlds(): Promise<void> {
  loadingWorlds.value = true
  worldError.value = ''
  try {
    worlds.value = await modifierApi.discoverWorlds()
    if (selectedWorldPath.value && !worlds.value.some((world) => world.path === selectedWorldPath.value)) {
      selectedWorldPath.value = ''
      worldState.value = null
    }
  } catch (error) {
    worldError.value = `读取世界列表失败: ${errorMessage(error)}`
  } finally {
    loadingWorlds.value = false
  }
}

async function loadCurrentWorld(): Promise<void> {
  closeMenu()
  worldError.value = ''
  if (!selectedWorldPath.value) {
    worldState.value = null
    return
  }
  loadingWorld.value = true
  try {
    worldState.value = await modifierApi.getWorld(selectedWorldPath.value)
  } catch (error) {
    worldState.value = null
    worldError.value = `读取世界失败: ${errorMessage(error)}`
  } finally {
    loadingWorld.value = false
  }
}

async function refresh(): Promise<void> {
  await loadWorlds()
  if (selectedWorldPath.value) await loadCurrentWorld()
}

function clampMenuPosition(left: number, top: number): { left: number; top: number } {
  const menuWidth = 190
  const menuHeight = 280
  return {
    left: Math.max(8, Math.min(left, window.innerWidth - menuWidth - 8)),
    top: Math.max(8, Math.min(top, window.innerHeight - menuHeight - 8)),
  }
}

function openContextMenu(target: MenuTarget, event: MouseEvent): void {
  menuTarget.value = target
  menuPosition.value = clampMenuPosition(event.clientX, event.clientY)
}

function openButtonMenu(target: MenuTarget, event: MouseEvent): void {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
  menuTarget.value = target
  menuPosition.value = clampMenuPosition(rect.right - 180, rect.bottom + 4)
}

function closeMenu(): void {
  menuTarget.value = null
}

function baseRequest(action: ModifierAction, target: MenuTarget): ModifierActionRequest {
  const request: ModifierActionRequest = { world_path: selectedWorldPath.value, action }
  if (target.kind === 'player') request.player_uid = target.player.player_uid
  else request.guild_id = target.guild.guild_id
  return request
}

function targetName(target: MenuTarget): string {
  return target.kind === 'player'
    ? target.player.nickname || '未命名玩家'
    : target.guild.name || '未命名公会'
}

function selectAction(action: ModifierAction): void {
  const target = menuTarget.value
  closeMenu()
  if (!target || writeBlocked.value || writing.value) return

  if (action === 'rename_player' || action === 'rename_guild') {
    formName.value = targetName(target)
    openFormDialog(action, action === 'rename_player' ? '重命名玩家' : '重命名公会', target)
    return
  }
  if (action === 'set_player_level' && target.kind === 'player') {
    formLevel.value = target.player.level
    openFormDialog(action, '修改玩家等级', target)
    return
  }
  if (action === 'set_technology_points' && target.kind === 'player') {
    formTechnologyPoints.value = target.player.technology_points
    formAncientTechnologyPoints.value = target.player.ancient_technology_points
    openFormDialog(action, '修改科技点', target)
    return
  }
  void previewRequest(baseRequest(action, target))
}

function openFormDialog(action: ModifierAction, title: string, target: MenuTarget): void {
  formError.value = ''
  formDialog.value = { action, title, entityName: targetName(target), target }
}

function closeFormDialog(): void {
  if (previewLoading.value) return
  formDialog.value = null
  formError.value = ''
}

async function submitForm(): Promise<void> {
  const dialog = formDialog.value
  if (!dialog || previewLoading.value) return
  const request = baseRequest(dialog.action, dialog.target)

  if (isNameAction(dialog.action)) {
    const name = formName.value.trim()
    if (!name) {
      formError.value = '名称不能为空。'
      return
    }
    request.value = name
  } else if (dialog.action === 'set_player_level') {
    if (!isValidInteger(formLevel.value, 1, 999)) {
      formError.value = '等级必须是 1 到 999 之间的整数。'
      return
    }
    request.level = formLevel.value
  } else if (dialog.action === 'set_technology_points') {
    if (
      !isValidInteger(formTechnologyPoints.value, 0, MAX_TECHNOLOGY_POINTS)
      || !isValidInteger(formAncientTechnologyPoints.value, 0, MAX_TECHNOLOGY_POINTS)
    ) {
      formError.value = '科技点必须是有效的非负整数。'
      return
    }
    request.technology_points = formTechnologyPoints.value
    request.ancient_technology_points = formAncientTechnologyPoints.value
  }

  await previewRequest(request)
}

async function previewRequest(request: ModifierActionRequest): Promise<void> {
  if (writeBlocked.value) return
  previewLoading.value = true
  operationStatus.value = '正在预览修改影响'
  try {
    const preview = await modifierApi.previewAction(request)
    formDialog.value = null
    confirmationName.value = ''
    previewDialog.value = {
      request,
      preview,
      destructive: DESTRUCTIVE_ACTIONS.has(request.action),
    }
    operationStatus.value = '预览已就绪'
  } catch (error) {
    operationStatus.value = '预览失败'
    toast.error(`无法预览修改: ${errorMessage(error)}`)
  } finally {
    previewLoading.value = false
  }
}

function closePreviewDialog(): void {
  if (writing.value) return
  previewDialog.value = null
  confirmationName.value = ''
}

async function applyPreviewedAction(): Promise<void> {
  const dialog = previewDialog.value
  if (!dialog || !canApplyPreview.value) return

  writing.value = true
  previewDialog.value = null
  currentProgressLabel.value = '检查游戏和服务器'
  operationStatus.value = currentProgressLabel.value
  try {
    const result = await modifierApi.applyAction(dialog.request)
    if (!result.ok || !result.roundtrip_ok) {
      throw new Error(result.message || '存档写入后未通过验证')
    }
    operationStatus.value = '正在刷新世界数据'
    await loadCurrentWorld()
    operationStatus.value = result.message || '修改完成'
    toast.success(result.message || '修改完成')
  } catch (error) {
    operationStatus.value = '修改失败，未报告成功'
    toast.error(`修改失败: ${errorMessage(error)}`)
  } finally {
    writing.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', closeMenu)
  void modifierApi.onProgress((progress: ModifierOperationProgress) => {
    if (!writing.value) return
    currentProgressLabel.value = progress.label
    operationStatus.value = progress.label
  }).then((unlisten) => {
    unlistenProgress = unlisten
  })
  void loadWorlds()
})

onBeforeUnmount(() => {
  document.removeEventListener('click', closeMenu)
  unlistenProgress?.()
})
</script>

<style scoped>
.modifier-view { position: relative; }
.head-actions { display: flex; align-items: flex-end; gap: 8px; }
.world-field { display: grid; gap: 5px; min-width: min(320px, 42vw); color: var(--text-mid2); font-size: 12px; font-weight: 600; }
.world-field select, .dialog-field input { box-sizing: border-box; width: 100%; border: 1px solid var(--palwarm-border, #e8ddd0); border-radius: 6px; background: var(--input-bg, #fff); color: var(--text-hi); padding: 9px 10px; font: inherit; }
.modifier-warning { display: flex; align-items: center; gap: 8px; padding: 11px 14px; border: 1px solid rgba(184, 120, 47, .32); border-radius: 8px; background: rgba(184, 120, 47, .1); color: var(--amber, #9b5c14); font-size: 13px; }
.operation-status { margin: 0; color: var(--text-mid2); font-size: 13px; }
.panel-error { margin: 0; color: var(--red-soft, #ba4d47); font-size: 13px; }
.modifier-progress-overlay { position: fixed; z-index: 95; inset: 0; display: grid; place-items: center; padding: 20px; background: rgba(35, 25, 20, .38); }
.modifier-progress-panel { display: grid; grid-template-columns: 28px minmax(0, 1fr); align-items: center; gap: 13px; width: min(420px, 100%); box-sizing: border-box; border: 1px solid var(--palwarm-border, #e8ddd0); border-radius: 8px; background: var(--palwarm-surface, #faf6f0); box-shadow: 0 18px 55px rgba(35, 25, 20, .25); padding: 18px 20px; }
.modifier-progress-panel strong { display: block; color: var(--text-hi); font-size: 15px; }
.modifier-progress-panel p { margin: 4px 0 0; color: var(--text-mid2); font-size: 12px; line-height: 1.5; }
.progress-spinner { width: 24px; height: 24px; box-sizing: border-box; border: 3px solid rgba(230, 111, 81, .2); border-top-color: var(--accent, #e66f51); border-radius: 50%; animation: modifier-spin .8s linear infinite; }
@keyframes modifier-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .progress-spinner { animation-duration: 1.8s; } }
.modifier-workbench { min-height: 430px; border: 1px solid var(--palwarm-border, #e8ddd0); border-radius: 8px; background: var(--palwarm-surface, #faf6f0); overflow: hidden; }
.workbench-head { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; padding: 16px 18px 0; border-bottom: 1px solid var(--palwarm-border, #e8ddd0); }
.eyebrow { display: block; color: var(--text-mid2); font-size: 11px; font-weight: 600; }
h2 { margin: 3px 0 14px; color: var(--text-hi); font-size: 18px; }
.entity-tabs { display: flex; align-self: stretch; gap: 4px; }
.entity-tabs button { min-width: 88px; border: 0; border-bottom: 2px solid transparent; background: transparent; color: var(--text-mid2); cursor: pointer; font: inherit; font-size: 13px; font-weight: 600; }
.entity-tabs button.active { border-bottom-color: var(--accent, #e66f51); color: var(--text-hi); }
.entity-tabs span { margin-left: 4px; color: var(--text-mid2); font-size: 11px; }
.table-wrap { width: 100%; overflow-x: auto; }
table { width: 100%; min-width: 900px; border-collapse: collapse; color: var(--text-hi); font-size: 13px; }
th { background: rgba(69, 51, 41, .035); color: var(--text-mid2); font-size: 11px; font-weight: 600; text-align: left; }
th, td { padding: 11px 14px; border-bottom: 1px solid var(--palwarm-border, #e8ddd0); vertical-align: middle; }
tbody tr:hover { background: rgba(230, 111, 81, .05); }
td strong, .secondary { display: block; }
.secondary { margin-top: 3px; color: var(--text-mid2); font-size: 11px; }
.mono { max-width: 190px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono, monospace); }
.actions-column { width: 46px; padding-left: 4px; padding-right: 8px; text-align: right; }
.menu-trigger { width: 36px; height: 36px; border: 0; border-radius: 6px; background: transparent; color: var(--text-mid2); cursor: pointer; font-size: 22px; line-height: 1; }
.menu-trigger:hover, .menu-trigger:focus-visible { background: rgba(69, 51, 41, .08); color: var(--text-hi); }
.row-action-menu { position: fixed; z-index: 80; display: grid; width: 190px; padding: 5px; border: 1px solid var(--palwarm-border, #e8ddd0); border-radius: 7px; background: var(--input-bg, #fff); box-shadow: 0 12px 30px rgba(49, 35, 28, .18); }
.row-action-menu button { min-height: 36px; border: 0; border-radius: 5px; background: transparent; color: var(--text-hi); cursor: pointer; padding: 7px 10px; text-align: left; font: inherit; font-size: 13px; }
.row-action-menu button:hover:not(:disabled) { background: rgba(69, 51, 41, .07); }
.row-action-menu button.danger { color: var(--red-soft, #ba4d47); }
.row-action-menu button:disabled { cursor: not-allowed; opacity: .45; }
.empty-state { display: grid; min-height: 260px; place-items: center; color: var(--text-mid2); font-size: 13px; }
.dialog-backdrop { position: fixed; z-index: 90; inset: 0; display: grid; place-items: center; padding: 20px; background: rgba(35, 25, 20, .42); }
.modifier-dialog { width: min(430px, 100%); max-height: calc(100vh - 40px); overflow-y: auto; box-sizing: border-box; border: 1px solid var(--palwarm-border, #e8ddd0); border-radius: 8px; background: var(--palwarm-surface, #faf6f0); box-shadow: 0 18px 55px rgba(35, 25, 20, .25); padding: 20px; }
.modifier-dialog h3 { margin: 0; color: var(--text-hi); font-size: 17px; }
.dialog-copy, .preview-summary { margin: 6px 0 18px; color: var(--text-mid2); font-size: 13px; line-height: 1.55; }
.dialog-field { display: grid; gap: 6px; color: var(--text-mid2); font-size: 12px; font-weight: 600; }
.technology-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 20px; }
.impact-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; margin: 0; }
.impact-grid div { border: 1px solid var(--palwarm-border, #e8ddd0); border-radius: 6px; padding: 9px; }
.impact-grid dt { color: var(--text-mid2); font-size: 11px; }
.impact-grid dd { margin: 4px 0 0; color: var(--text-hi); font-size: 16px; font-weight: 700; }
.confirmation-field { margin-top: 18px; }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
@media (max-width: 760px) {
  .page-head, .workbench-head { align-items: stretch; flex-direction: column; }
  .head-actions { align-items: stretch; }
  .world-field { min-width: 0; flex: 1; }
  .workbench-head { padding-top: 14px; }
  .entity-tabs { min-height: 44px; }
  .technology-fields { grid-template-columns: 1fr; }
}
</style>
