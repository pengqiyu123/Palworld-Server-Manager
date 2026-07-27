<template>
  <section class="screen active migration-screen">
    <div class="page-head">
      <div>
        <div class="page-title">世界与角色迁移</div>
        <div class="page-sub">按需迁移世界、转移完整角色，或恢复单机主角色原来的公会。</div>
      </div>
      <div class="page-actions">
        <button class="btn btn-ghost btn-sm" :disabled="loading || operationPending" @click="onDiscover">
          {{ loading ? '正在刷新' : '刷新' }}
        </button>
      </div>
    </div>

    <div v-if="serverPathMissing" class="notice notice--warn">
      <AppIcon name="info" :size="16" />
      <span>尚未设置服务器目录，请先在设置中完成配置。</span>
      <router-link class="notice-action" to="/settings">前往设置</router-link>
    </div>

    <nav class="task-nav" aria-label="迁移任务">
      <button
        v-for="task in mainTasks"
        :key="task.id"
        class="task-tab"
        :class="{ active: activeTask === task.id }"
        :aria-pressed="activeTask === task.id"
        @click="selectTask(task.id)"
      >
        {{ task.label }}
      </button>
      <button class="friend-entry" :class="{ active: activeTask === 'friend' }" @click="selectTask('friend')">
        导入朋友角色
      </button>
    </nav>

    <div v-if="serverNeedsStop" class="notice notice--action">
      <AppIcon name="info" :size="16" />
      <span>服务器正在运行。存档操作需要先安全停服。</span>
      <button class="btn btn-primary btn-sm" :disabled="operationPending" @click="stopServerAndContinue">
        {{ operationPending ? progressLabel : '停止服务器并继续' }}
      </button>
      <button class="btn btn-ghost btn-sm" :disabled="operationPending" @click="cancelDeferredAction">取消</button>
    </div>

    <div v-if="operationPending" class="operation-status" role="status" aria-live="polite">
      <span class="status-spinner" aria-hidden="true" />
      <div>
        <strong>{{ operationTitle }}</strong>
        <span>{{ progressLabel }}</span>
      </div>
    </div>

    <div v-if="operationError" class="notice notice--error" role="alert">
      <AppIcon name="info" :size="16" />
      <span>{{ operationError }}</span>
      <button class="btn btn-ghost btn-sm" @click="operationError = ''">关闭</button>
    </div>

    <section v-if="activeTask === 'world'" class="task-panel" aria-labelledby="world-task-title">
      <header class="task-head">
        <div>
          <h2 id="world-task-title">迁移整个世界</h2>
          <p>将单机或其他服务器世界复制到当前专用服务器。目标世界会先完整备份。</p>
        </div>
      </header>

      <div class="field-grid">
        <label class="field">
          <span>来源世界</span>
          <select v-model="sourcePath" class="input" :disabled="loading || operationPending">
            <option value="">请选择来源世界</option>
            <optgroup v-if="localWorlds.length" label="本机单机">
              <option v-for="world in localWorlds" :key="world.path" :value="world.path">
                {{ world.name }} · {{ world.player_count }} 名角色
              </option>
            </optgroup>
            <optgroup v-if="serverWorlds.length" label="服务器世界">
              <option v-for="world in serverWorlds" :key="world.path" :value="world.path">
                {{ world.name }} · {{ world.player_count }} 名角色
              </option>
            </optgroup>
          </select>
        </label>

        <label class="field">
          <span>目标服务器世界</span>
          <select v-model="targetWorldName" class="input" :disabled="loading || operationPending">
            <option value="">请选择目标世界</option>
            <option v-for="world in serverWorlds" :key="world.path" :value="world.name">
              {{ world.name }} · {{ world.player_count }} 名角色
            </option>
          </select>
        </label>
      </div>

      <div class="notice notice--config">
        <AppIcon name="info" :size="16" />
        <span><strong>保留当前服务器设置</strong>。来源世界规则不会直接应用，迁移完成后由“服务器配置”继续管理。</span>
      </div>

      <div class="task-actions">
        <button class="btn btn-primary" :disabled="!canMigrateWorld || operationPending" @click="onMigrateIntent">
          {{ operationPending && operationTitle === '迁移世界' ? progressLabel : '开始迁移' }}
        </button>
        <span>迁移不会在没有备份的情况下继续。</span>
      </div>
    </section>

    <section v-else-if="activeTask === 'character'" class="task-panel" aria-labelledby="character-task-title">
      <header class="task-head">
        <div>
          <h2 id="character-task-title">转移完整角色</h2>
          <p>把原角色的等级、物品、帕鲁和科技等数据转到用于登录服务器的角色。</p>
        </div>
        <span class="contract-badge">公会关系不会改变</span>
      </header>

      <label class="field field--compact">
        <span>迁移记录</span>
        <select v-model="selectedWorkflowId" class="input" :disabled="operationPending" @change="loadWorkflowPlayers">
          <option value="">请选择已迁移的世界</option>
          <option v-for="(item, index) in availableWorkflows" :key="item.id" :value="item.id">
            {{ workflowLabel(item, index) }}
          </option>
        </select>
      </label>

      <div v-if="playerLoading" class="empty-state">正在读取角色...</div>
      <div v-else-if="selectedWorkflowId && !workflowPlayers.length" class="empty-state">
        暂未读取到角色。请先用原账号进入服务器创建角色并正常退出，然后刷新。
      </div>
      <div v-else-if="workflowPlayers.length" class="player-columns">
        <fieldset class="player-group">
          <legend>要保留的原角色</legend>
          <label v-for="player in workflowPlayers" :key="`source-${player.guid}`" class="player-option">
            <input v-model="sourcePlayerFile" type="radio" name="source-player" :value="player.guid" />
            <span><strong>{{ player.nickname || '未命名角色' }}</strong><small>{{ playerDescription(player) }}</small></span>
          </label>
        </fieldset>
        <div class="transfer-direction" aria-hidden="true">→</div>
        <fieldset class="player-group">
          <legend>用于登录的服务器角色</legend>
          <label v-for="player in workflowPlayers" :key="`target-${player.guid}`" class="player-option">
            <input v-model="targetPlayerFile" type="radio" name="target-player" :value="player.guid" />
            <span><strong>{{ player.nickname || '未命名角色' }}</strong><small>{{ playerDescription(player) }}</small></span>
          </label>
        </fieldset>
      </div>

      <div class="task-actions">
        <button class="btn btn-primary" :disabled="!canTransferCharacter || operationPending" @click="startCharacterTransfer">
          {{ operationPending && operationTitle === '转移完整角色' ? progressLabel : '转移完整角色' }}
        </button>
        <button class="btn btn-ghost" :disabled="!selectedWorkflowId || operationPending" @click="loadWorkflowPlayers">刷新角色</button>
      </div>
    </section>

    <section v-else-if="activeTask === 'guild'" class="task-panel" aria-labelledby="guild-task-title">
      <header class="task-head">
        <div>
          <h2 id="guild-task-title">恢复原公会</h2>
          <p>仅把已经转移的单机主角色恢复到原公会，不改变角色数据。</p>
        </div>
      </header>

      <label class="field field--compact">
        <span>迁移记录</span>
        <select v-model="selectedWorkflowId" class="input" :disabled="operationPending" @change="loadGuildSummary">
          <option value="">请选择已完成角色转移的世界</option>
          <option v-for="(item, index) in guildReadyWorkflows" :key="item.id" :value="item.id">
            {{ workflowLabel(item, index) }}
          </option>
        </select>
      </label>

      <div v-if="selectedWorkflowId" class="identity-summary">
        <div><span>单机主角色</span><strong>{{ guildSummary.playerName }}</strong></div>
        <div><span>原公会</span><strong>{{ guildSummary.guildName }}</strong></div>
        <p v-if="!selectedWorkflow?.identity">当前记录缺少可用的角色识别信息，无法恢复原公会。</p>
      </div>
      <div v-else class="empty-state">请选择一条已完成完整角色转移的迁移记录。</div>

      <div class="task-actions">
        <button class="btn btn-primary" :disabled="!canRestoreGuild || operationPending" @click="startGuildRestore">
          {{ operationPending && operationTitle === '恢复原公会' ? progressLabel : '恢复原公会' }}
        </button>
      </div>
    </section>

    <section v-else class="task-panel friend-transfer-panel" aria-labelledby="friend-task-title">
      <header class="task-head">
        <div>
          <h2 id="friend-task-title">导入朋友角色</h2>
          <p>把朋友的完整角色数据导入目标世界。此操作独立于单机主角色迁移。</p>
        </div>
      </header>

      <div class="field-grid">
        <label class="field">
          <span>朋友角色所在世界</span>
          <select v-model="friendSourceWorldPath" class="input" @change="loadFriendSourcePlayers">
            <option value="">请选择来源世界</option>
            <option v-for="world in allWorlds" :key="world.path" :value="world.path">{{ world.name }}</option>
          </select>
        </label>
        <label class="field">
          <span>目标服务器世界</span>
          <select v-model="friendTargetWorldPath" class="input" @change="loadFriendTargetPlayers">
            <option value="">请选择目标世界</option>
            <option v-for="world in serverWorlds" :key="world.path" :value="world.path">{{ world.name }}</option>
          </select>
        </label>
      </div>

      <div class="player-columns">
        <fieldset class="player-group">
          <legend>朋友的原角色</legend>
          <div v-if="!friendSourcePlayers.length" class="empty-state">选择来源世界后读取角色。</div>
          <label v-for="player in friendSourcePlayers" :key="`friend-source-${player.guid}`" class="player-option">
            <input v-model="friendSourcePlayerFile" type="radio" name="friend-source-player" :value="player.guid" />
            <span><strong>{{ player.nickname || '未命名角色' }}</strong><small>{{ playerDescription(player) }}</small></span>
          </label>
        </fieldset>
        <div class="transfer-direction" aria-hidden="true">→</div>
        <fieldset class="player-group">
          <legend>朋友用于登录的服务器角色</legend>
          <div v-if="!friendTargetPlayers.length" class="empty-state">选择目标世界后读取角色。</div>
          <label v-for="player in friendTargetPlayers" :key="`friend-target-${player.guid}`" class="player-option">
            <input v-model="friendTargetPlayerFile" type="radio" name="friend-target-player" :value="player.guid" />
            <span><strong>{{ player.nickname || '未命名角色' }}</strong><small>{{ playerDescription(player) }}</small></span>
          </label>
        </fieldset>
      </div>

      <div class="task-actions">
        <button class="btn btn-primary" :disabled="!canImportFriend || operationPending" @click="startFriendImport">
          {{ operationPending && operationTitle === '导入朋友角色' ? progressLabel : '导入朋友角色' }}
        </button>
        <span>不会更改任何公会关系。</span>
      </div>
    </section>

    <section v-if="result" class="result-panel" aria-live="polite">
      <div>
        <strong>{{ result.title }}</strong>
        <p>{{ result.message }}</p>
      </div>
      <div class="result-actions">
        <button v-if="result.nextTask" class="btn btn-primary btn-sm" @click="continueTo(result.nextTask)">
          {{ result.nextTask === 'character' ? '继续转移角色' : '继续恢复原公会' }}
        </button>
        <button v-if="result.canComplete" class="btn btn-success btn-sm" :disabled="operationPending" @click="completeWorkflow">
          验证正常，完成
        </button>
        <button class="btn btn-danger-ghost btn-sm" :disabled="operationPending" @click="rollbackWorkflow">
          发现问题，回滚
        </button>
      </div>
    </section>

    <ConfirmDialog
      v-model:visible="migrationNoticeVisible"
      title="迁移前会自动备份"
      :message="`目标世界会先备份到 ${backupRoot || '程序默认备份目录'}。只有备份成功后才会开始迁移；如果该目录不可用，迁移会停止。`"
      confirm-text="开始迁移"
      cancel-text="取消"
      @confirm="confirmMigration"
    />
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { api } from '@/api/tauri'
import AppIcon from '@/components/ui/AppIcon.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import { useSettingsStore } from '@/stores/settings'
import type {
  MigrationWorkflow,
  PlayerEntry,
  SaveOperationProgress,
  WorldInfo,
} from '@/types/tauri'

type MainTask = 'world' | 'character' | 'guild'
type ActiveTask = MainTask | 'friend'
type DeferredAction = () => Promise<void>

interface OperationResultState {
  title: string
  message: string
  workflowId: string
  nextTask?: MainTask
  canComplete: boolean
}

const route = useRoute()
const settingsStore = useSettingsStore()

const mainTasks: Array<{ id: MainTask; label: string }> = [
  { id: 'world', label: '迁移整个世界' },
  { id: 'character', label: '转移完整角色' },
  { id: 'guild', label: '恢复原公会' },
]

const activeTask = ref<ActiveTask>('world')
const loading = ref(false)
const operationPending = ref(false)
const operationTitle = ref('')
const progressLabel = ref('')
const operationError = ref('')
const currentRequestId = ref('')
const deferredAction = ref<DeferredAction | null>(null)
const serverNeedsStop = ref(false)
const migrationNoticeVisible = ref(false)
const result = ref<OperationResultState | null>(null)

const localWorlds = ref<WorldInfo[]>([])
const serverWorlds = ref<WorldInfo[]>([])
const workflows = ref<MigrationWorkflow[]>([])
const sourcePath = ref('')
const targetWorldName = ref('')
const backupRoot = ref('')

const selectedWorkflowId = ref('')
const workflowPlayers = ref<PlayerEntry[]>([])
const playerLoading = ref(false)
const sourcePlayerFile = ref('')
const targetPlayerFile = ref('')
const guildSummary = ref({ playerName: '未知', guildName: '未知' })

const friendSourceWorldPath = ref('')
const friendTargetWorldPath = ref('')
const friendSourcePlayers = ref<PlayerEntry[]>([])
const friendTargetPlayers = ref<PlayerEntry[]>([])
const friendSourcePlayerFile = ref('')
const friendTargetPlayerFile = ref('')

let unlistenProgress: (() => void) | null = null

const serverPathMissing = computed(() => !settingsStore.settings.server_path)
const allWorlds = computed(() => [...localWorlds.value, ...serverWorlds.value])
const availableWorkflows = computed(() => workflows.value.filter((item) =>
  !['committed', 'rolled_back', 'recovery_required'].includes(item.status),
))
const guildReadyWorkflows = computed(() => availableWorkflows.value.filter((item) =>
  ['character_transferred', 'guild_restored', 'awaiting_game_verification'].includes(item.stage),
))
const selectedWorkflow = computed(() => workflows.value.find((item) => item.id === selectedWorkflowId.value) ?? null)
const canMigrateWorld = computed(() => {
  const source = allWorlds.value.find((item) => item.path === sourcePath.value)
  const target = serverWorlds.value.find((item) => item.name === targetWorldName.value)
  return !!source && !!target && target.path !== source.path && !serverPathMissing.value
})
const canTransferCharacter = computed(() =>
  !!selectedWorkflowId.value && !!sourcePlayerFile.value && !!targetPlayerFile.value && sourcePlayerFile.value !== targetPlayerFile.value,
)
const canRestoreGuild = computed(() => !!selectedWorkflowId.value && !!selectedWorkflow.value?.identity)
const canImportFriend = computed(() =>
  !!friendSourceWorldPath.value &&
  !!friendTargetWorldPath.value &&
  !!friendSourcePlayerFile.value &&
  !!friendTargetPlayerFile.value &&
  friendSourcePlayerFile.value !== friendTargetPlayerFile.value,
)

function newRequestId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `save-${Date.now()}-${Math.random().toString(16).slice(2)}`
}

function selectTask(task: ActiveTask): void {
  activeTask.value = task
  operationError.value = ''
}

async function onDiscover(): Promise<void> {
  loading.value = true
  operationError.value = ''
  try {
    const resolvedBackupRoot = await api.save.getBackupRoot()
    const excludedBackupRoots = [
      resolvedBackupRoot,
      settingsStore.settings.backup_root,
      ...(settingsStore.settings.backup_roots ?? []),
    ].filter(Boolean)
    const localRoots = (settingsStore.settings.local_save_roots ?? []).filter((root) =>
      !excludedBackupRoots.some((backup) => sameDirectory(root, backup)),
    )
    const serverRoots = settingsStore.settings.server_save_roots ?? []
    const [localResults, serverResults, workflowResults] = await Promise.all([
      Promise.all([api.save.discoverLocalWorlds(), ...localRoots.map((root) => api.save.discoverLocalWorlds(root))]),
      Promise.all([api.save.discoverWorlds(), ...serverRoots.map((root) => api.save.discoverWorlds(root))]),
      api.save.listWorkflows(),
    ])
    localWorlds.value = dedupeWorlds(localResults.flat())
    serverWorlds.value = dedupeWorlds(serverResults.flatMap((item) => item.worlds))
    workflows.value = workflowResults
    backupRoot.value = resolvedBackupRoot

    const requestedSource = typeof route.query.source === 'string' ? route.query.source : ''
    if (requestedSource && allWorlds.value.some((world) => world.path === requestedSource)) sourcePath.value = requestedSource
    if (!sourcePath.value) sourcePath.value = localWorlds.value[0]?.path ?? ''
    if (!targetWorldName.value) targetWorldName.value = serverWorlds.value[0]?.name ?? ''
  } catch (error) {
    operationError.value = `读取存档失败：${errorMessage(error)}`
  } finally {
    loading.value = false
  }
}

function sameDirectory(left: string, right: string): boolean {
  const normalize = (value: string) => value.replace(/[\\/]+$/, '').toLocaleLowerCase()
  return normalize(left) === normalize(right)
}

function dedupeWorlds(items: WorldInfo[]): WorldInfo[] {
  return [...new Map(items.map((item) => [item.path, item])).values()].sort((left, right) => left.name.localeCompare(right.name))
}

function workflowLabel(item: MigrationWorkflow, index: number): string {
  const world = serverWorlds.value.find((candidate) => candidate.path === item.target_world_path)
  return world?.name ?? `迁移记录 ${index + 1}`
}

function playerDescription(player: PlayerEntry): string {
  const guild = player.guild_id ? '已有公会' : '无公会'
  return `等级 ${player.level} · ${guild} · ${player.pal_count} 只帕鲁 · ${player.last_online || '上次在线未知'}`
}

function onMigrateIntent(): void {
  if (!canMigrateWorld.value) return
  if (!settingsStore.settings.migration_backup_notice_seen) {
    migrationNoticeVisible.value = true
    return
  }
  void gateServerAndRun(runWorldMigration)
}

async function confirmMigration(): Promise<void> {
  settingsStore.update({ migration_backup_notice_seen: true })
  try {
    await settingsStore.save()
  } catch (error) {
    settingsStore.update({ migration_backup_notice_seen: false })
    operationError.value = `无法保存迁移说明状态：${errorMessage(error)}`
    return
  }
  void gateServerAndRun(runWorldMigration)
}

async function runWorldMigration(): Promise<void> {
  const source = allWorlds.value.find((item) => item.path === sourcePath.value)
  if (!source) return
  await runOperation('迁移世界', async () => {
    const outcome = await api.migration.migrateWorldV4({
      request_id: currentRequestId.value,
      source_path: source.path,
      source_name: source.name,
      target_world: targetWorldName.value,
      preserve_server_config: true,
    })
    selectedWorkflowId.value = outcome.workflow.id
    workflows.value = [outcome.workflow, ...workflows.value.filter((item) => item.id !== outcome.workflow.id)]
    result.value = {
      title: '世界迁移完成',
      message: '目标世界已写入并保留操作前备份。服务器规则仍由配置页管理。请进入服务器创建用于登录的新角色，再继续转移完整角色。',
      workflowId: outcome.workflow.id,
      nextTask: 'character',
      canComplete: false,
    }
  })
}

async function loadWorkflowPlayers(): Promise<void> {
  sourcePlayerFile.value = ''
  targetPlayerFile.value = ''
  workflowPlayers.value = []
  if (!selectedWorkflow.value) return
  playerLoading.value = true
  try {
    const summary = await api.migration.worldSummaryByPath(selectedWorkflow.value.target_world_path)
    workflowPlayers.value = summary.players
  } catch (error) {
    operationError.value = `读取角色失败：${errorMessage(error)}`
  } finally {
    playerLoading.value = false
  }
}

function startCharacterTransfer(): void {
  if (!canTransferCharacter.value) return
  void gateServerAndRun(async () => {
    await runOperation('转移完整角色', async () => {
      const outcome = await api.migration.transferFullCharacterV4({
        request_id: currentRequestId.value,
        workflow_id: selectedWorkflowId.value,
        source_player_file: sourcePlayerFile.value,
        target_player_file: targetPlayerFile.value,
      })
      replaceWorkflow(outcome.workflow)
      result.value = {
        title: '完整角色转移完成',
        message: '角色数据已转移，公会关系未改变。请进入游戏检查角色。',
        workflowId: outcome.workflow.id,
        nextTask: outcome.workflow.identity ? 'guild' : undefined,
        canComplete: true,
      }
    })
  })
}

async function loadGuildSummary(): Promise<void> {
  guildSummary.value = { playerName: '未知', guildName: '未知' }
  const item = selectedWorkflow.value
  if (!item?.identity) return
  try {
    const summary = await api.migration.worldSummaryByPath(item.target_world_path)
    const player = summary.players.find((candidate) => candidate.player_uid === item.identity?.target_player_uid)
    const guild = summary.guilds.find((candidate) => candidate.guild_id === item.identity?.source_group_id)
    guildSummary.value = {
      playerName: player?.nickname || '名称未知',
      guildName: guild?.name || '名称未知',
    }
  } catch {
    guildSummary.value = { playerName: '名称未知', guildName: '名称未知' }
  }
}

function startGuildRestore(): void {
  if (!canRestoreGuild.value) return
  void gateServerAndRun(async () => {
    await runOperation('恢复原公会', async () => {
      const outcome = await api.migration.restoreOriginalGuildV4({
        request_id: currentRequestId.value,
        workflow_id: selectedWorkflowId.value,
      })
      replaceWorkflow(outcome.workflow)
      result.value = {
        title: '原公会已恢复',
        message: '单机主角色已恢复到后端识别的原公会。请进入游戏检查。',
        workflowId: outcome.workflow.id,
        canComplete: true,
      }
    })
  })
}

async function loadFriendSourcePlayers(): Promise<void> {
  friendSourcePlayerFile.value = ''
  friendSourcePlayers.value = await loadPlayersAt(friendSourceWorldPath.value)
}

async function loadFriendTargetPlayers(): Promise<void> {
  friendTargetPlayerFile.value = ''
  friendTargetPlayers.value = await loadPlayersAt(friendTargetWorldPath.value)
}

async function loadPlayersAt(path: string): Promise<PlayerEntry[]> {
  if (!path) return []
  try {
    return (await api.migration.worldSummaryByPath(path)).players
  } catch (error) {
    operationError.value = `读取角色失败：${errorMessage(error)}`
    return []
  }
}

function startFriendImport(): void {
  if (!canImportFriend.value) return
  void gateServerAndRun(async () => {
    await runOperation('导入朋友角色', async () => {
      const outcome = await api.migration.importFriendCharacterV4({
        request_id: currentRequestId.value,
        source_world_path: friendSourceWorldPath.value,
        target_world_path: friendTargetWorldPath.value,
        source_player_file: friendSourcePlayerFile.value,
        target_player_file: friendTargetPlayerFile.value,
      })
      replaceWorkflow(outcome.workflow)
      result.value = {
        title: '朋友角色导入完成',
        message: '完整角色数据已导入，任何公会关系都未更改。请让朋友进入游戏检查。',
        workflowId: outcome.workflow.id,
        canComplete: true,
      }
    })
  })
}

async function gateServerAndRun(action: DeferredAction): Promise<void> {
  if (operationPending.value) return
  operationPending.value = true
  operationTitle.value = '准备操作'
  progressLabel.value = '正在检查服务器'
  operationError.value = ''
  try {
    const status = await api.server.getStatus()
    if (status.running) {
      deferredAction.value = action
      serverNeedsStop.value = true
      return
    }
    await action()
  } catch (error) {
    operationError.value = `无法检查服务器：${errorMessage(error)}`
  } finally {
    if (serverNeedsStop.value || operationTitle.value === '准备操作') operationPending.value = false
  }
}

async function stopServerAndContinue(): Promise<void> {
  const action = deferredAction.value
  if (!action || operationPending.value) return
  operationPending.value = true
  operationTitle.value = '停止服务器'
  progressLabel.value = '正在停止服务器'
  operationError.value = ''
  try {
    await api.server.stop()
    serverNeedsStop.value = false
    deferredAction.value = null
    await action()
  } catch (error) {
    operationError.value = `停止服务器失败：${errorMessage(error)}`
  } finally {
    if (operationTitle.value === '停止服务器') operationPending.value = false
  }
}

function cancelDeferredAction(): void {
  deferredAction.value = null
  serverNeedsStop.value = false
}

async function runOperation(title: string, operation: () => Promise<void>): Promise<void> {
  operationPending.value = true
  operationTitle.value = title
  progressLabel.value = '正在开始'
  operationError.value = ''
  result.value = null
  currentRequestId.value = newRequestId()
  try {
    await operation()
  } catch (error) {
    operationError.value = `${title}未完成：${errorMessage(error)}`
  } finally {
    operationPending.value = false
    currentRequestId.value = ''
  }
}

function onProgress(progress: SaveOperationProgress): void {
  if (progress.request_id !== currentRequestId.value) return
  progressLabel.value = progress.label
}

function replaceWorkflow(item: MigrationWorkflow): void {
  workflows.value = [item, ...workflows.value.filter((existing) => existing.id !== item.id)]
  selectedWorkflowId.value = item.id
}

function continueTo(task: MainTask): void {
  if (!result.value) return
  selectedWorkflowId.value = result.value.workflowId
  activeTask.value = task
  if (task === 'character') void loadWorkflowPlayers()
  if (task === 'guild') void loadGuildSummary()
}

async function completeWorkflow(): Promise<void> {
  if (!result.value) return
  await runOperation('完成迁移', async () => {
    const item = await api.migration.completeMigrationWorkflowV4({
      request_id: currentRequestId.value,
      workflow_id: result.value!.workflowId,
    })
    replaceWorkflow(item)
    result.value = null
  })
}

function rollbackWorkflow(): void {
  if (!result.value) return
  const workflowId = result.value.workflowId
  void gateServerAndRun(async () => {
    await runOperation('回滚迁移', async () => {
      const item = await api.migration.rollbackMigrationWorkflowV4({
        request_id: currentRequestId.value,
        workflow_id: workflowId,
      })
      replaceWorkflow(item)
      result.value = null
    })
  })
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

onMounted(async () => {
  unlistenProgress = await api.migration.onProgress(onProgress)
  await onDiscover()
})

onBeforeUnmount(() => unlistenProgress?.())
</script>

<style scoped>
.migration-screen { gap: 16px; }
.notice,
.operation-status,
.result-panel {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  border: 1px solid var(--glass-border);
  border-radius: 8px;
  background: rgba(255, 252, 247, 0.74);
  color: var(--text-mid);
  font-size: 13px;
}
.notice > span { flex: 1; }
.notice--warn { border-color: rgba(184, 120, 47, 0.34); background: var(--amber-bg); }
.notice--error { border-color: rgba(201, 85, 77, 0.38); background: var(--red-bg); color: var(--red-soft); }
.notice--action { border-color: rgba(230, 111, 81, 0.34); }
.notice--config { align-items: flex-start; border-color: rgba(75, 120, 150, 0.28); background: rgba(75, 120, 150, 0.07); }
.notice--config strong { color: var(--text-hi); }
.notice-action { margin-left: auto; color: var(--primary-active); font-weight: 600; }
.task-nav {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 42px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--glass-border);
  overflow-x: auto;
}
.task-tab,
.friend-entry {
  flex: 0 0 auto;
  height: 34px;
  padding: 0 14px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--text-mid);
  font: 600 13px var(--font-ui);
  cursor: pointer;
}
.task-tab.active,
.friend-entry.active { background: var(--primary-soft); color: var(--primary-active); }
.friend-entry { margin-left: auto; border: 1px solid var(--glass-border); }
.task-panel {
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 4px 0 12px;
}
.task-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 18px; }
.task-head h2 { margin: 0; font-size: 17px; color: var(--text-hi); }
.task-head p { margin-top: 6px; color: var(--text-mid2); font-size: 13px; line-height: 1.6; }
.contract-badge { flex: 0 0 auto; padding: 5px 9px; border-radius: 999px; background: var(--green-bg); color: var(--green); font-size: 12px; font-weight: 600; }
.field-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
.field { display: flex; flex-direction: column; gap: 6px; min-width: 0; color: var(--text-mid); font-size: 13px; font-weight: 600; }
.field--compact { max-width: 460px; }
.field .input { width: 100%; font-family: var(--font-ui); }
.choice-group,
.player-group { min-width: 0; margin: 0; padding: 0; border: 0; }
.choice-group legend,
.player-group legend { margin-bottom: 8px; color: var(--text-mid); font-size: 13px; font-weight: 700; }
.choice-group { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.choice-group legend { grid-column: 1 / -1; }
.choice-row,
.player-option {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  min-width: 0;
  padding: 11px 12px;
  border: 1px solid var(--glass-border);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.45);
  cursor: pointer;
}
.choice-row:has(input:checked),
.player-option:has(input:checked) { border-color: var(--primary); background: var(--primary-soft); }
.choice-row input,
.player-option input { margin-top: 3px; accent-color: var(--primary); }
.choice-row span,
.player-option span { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.choice-row strong,
.player-option strong { color: var(--text-hi); font-size: 13px; }
.choice-row small,
.player-option small { color: var(--text-mid2); font-size: 11px; line-height: 1.45; }
.player-columns { display: grid; grid-template-columns: minmax(0, 1fr) 28px minmax(0, 1fr); gap: 12px; align-items: start; }
.player-group { display: flex; flex-direction: column; gap: 7px; max-height: 340px; overflow-y: auto; }
.transfer-direction { align-self: center; color: var(--primary); font-size: 22px; text-align: center; }
.task-actions { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.task-actions > span { color: var(--text-mid2); font-size: 12px; }
.empty-state { padding: 16px; border: 1px dashed var(--glass-border); border-radius: 8px; color: var(--text-mid2); font-size: 13px; }
.identity-summary { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; max-width: 680px; }
.identity-summary > div { display: flex; flex-direction: column; gap: 4px; padding: 12px 14px; border-left: 3px solid var(--primary); background: rgba(255, 255, 255, 0.42); }
.identity-summary span { color: var(--text-mid2); font-size: 11px; }
.identity-summary strong { color: var(--text-hi); font-size: 14px; }
.identity-summary p { grid-column: 1 / -1; color: var(--red-soft); font-size: 12px; }
.operation-status { border-color: rgba(75, 120, 150, 0.34); color: var(--palwarm-state-info); }
.operation-status > div { display: flex; flex-direction: column; gap: 3px; }
.operation-status span { font-size: 12px; }
.status-spinner { width: 16px; height: 16px; border: 2px solid rgba(75, 120, 150, 0.24); border-top-color: var(--palwarm-state-info); border-radius: 50%; animation: spin .8s linear infinite; }
.result-panel { justify-content: space-between; border-color: rgba(79, 138, 107, 0.36); background: var(--green-bg); }
.result-panel > div:first-child { min-width: 0; }
.result-panel strong { color: var(--green); }
.result-panel p { margin-top: 4px; line-height: 1.5; }
.result-actions { display: flex; justify-content: flex-end; gap: 8px; flex-wrap: wrap; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .status-spinner { animation: none; } }
@media (max-width: 720px) {
  .page-head,
  .task-head,
  .result-panel { align-items: stretch; flex-direction: column; }
  .friend-entry { margin-left: 0; }
  .field-grid,
  .choice-group,
  .identity-summary { grid-template-columns: 1fr; }
  .choice-group legend,
  .identity-summary p { grid-column: 1; }
  .player-columns { grid-template-columns: 1fr; }
  .transfer-direction { transform: rotate(90deg); }
  .task-actions .btn { flex: 1 1 auto; justify-content: center; }
  .result-actions { justify-content: stretch; }
  .result-actions .btn { flex: 1 1 160px; justify-content: center; }
}
</style>
