<template>
  <section class="screen active save-screen">
    <div class="page-head">
      <div>
        <div class="page-title">世界存档</div>
        <div class="page-sub">查看本机与服务器世界，并管理完整备份和最近操作回滚点。</div>
      </div>
      <div class="page-actions">
        <button class="btn btn-ghost btn-sm" :disabled="loading" @click="refreshActiveTab">
          <RefreshCw :size="15" />
          {{ loading ? '正在刷新' : '刷新' }}
        </button>
      </div>
    </div>

    <nav class="page-tabs" aria-label="世界存档页面">
      <button :class="{ active: activeTab === 'worlds' }" @click="setTab('worlds')">世界存档</button>
      <button :class="{ active: activeTab === 'backup' }" @click="setTab('backup')">备份与回滚</button>
    </nav>

    <div v-if="serverPathMissing" class="notice notice--warn">
      <AppIcon name="info" :size="16" />
      <span>尚未设置服务器目录，服务器世界可能无法读取。</span>
      <router-link to="/settings">前往设置</router-link>
    </div>

    <div v-if="errorMessage" class="notice notice--error" role="alert">
      <AppIcon name="info" :size="16" />
      <span>{{ errorMessage }}</span>
      <button class="btn btn-ghost btn-sm" @click="errorMessage = ''">关闭</button>
    </div>

    <div v-if="serverNeedsStop" class="notice notice--action">
      <AppIcon name="info" :size="16" />
      <span>服务器正在运行。为保证存档完整，需要先安全停服。</span>
      <button class="btn btn-primary btn-sm" :disabled="pendingKey !== ''" @click="stopServerAndContinue">
        {{ pendingKey === 'stop-server' ? '正在停止服务器' : '停止服务器并继续' }}
      </button>
      <button class="btn btn-ghost btn-sm" :disabled="pendingKey !== ''" @click="cancelDeferredAction">取消</button>
    </div>

    <template v-if="activeTab === 'worlds'">
      <section class="world-section" aria-labelledby="local-worlds-title">
        <div class="section-heading">
          <div>
            <h2 id="local-worlds-title">本机单机世界</h2>
            <p>自动读取 Steam、本机保存目录和已添加的位置。</p>
          </div>
          <button class="btn btn-ghost btn-sm" :disabled="loading" @click="pickWorldDirectory('local')">
            <FolderOpen :size="15" />选择目录
          </button>
        </div>
        <div v-if="localWorlds.length" class="world-grid">
          <button v-for="world in localWorlds" :key="world.path" class="world-card" @click="openWorld(world)">
            <AppIcon name="save" :size="20" class="world-icon" />
            <span class="world-card__body">
              <strong>{{ world.name }}</strong>
              <small>{{ world.player_count }} 名角色 · {{ formatSize(world.size_bytes) }} · {{ sourceLabel(world.source) }}</small>
            </span>
          </button>
        </div>
        <div v-else-if="loading" class="empty-state">正在读取本机世界...</div>
        <div v-else class="empty-state">未找到本机单机世界。可选择其他存档目录后重试。</div>
      </section>

      <section class="world-section" aria-labelledby="server-worlds-title">
        <div class="section-heading">
          <div>
            <h2 id="server-worlds-title">服务器世界</h2>
            <p>当前专用服务器和已添加目录中的世界。</p>
          </div>
          <button class="btn btn-ghost btn-sm" :disabled="loading" @click="pickWorldDirectory('server')">
            <FolderOpen :size="15" />选择目录
          </button>
        </div>
        <div v-if="serverWorlds.length" class="world-grid">
          <button v-for="world in serverWorlds" :key="world.path" class="world-card" @click="openWorld(world)">
            <AppIcon name="save" :size="20" class="world-icon" />
            <span class="world-card__body">
              <strong>{{ world.name }}</strong>
              <small>{{ world.player_count }} 名角色 · {{ formatSize(world.size_bytes) }} · 服务器世界</small>
            </span>
          </button>
        </div>
        <div v-else-if="loading" class="empty-state">正在读取服务器世界...</div>
        <div v-else class="empty-state">未找到服务器世界。请确认服务器目录，或先启动一次服务器创建世界。</div>
      </section>

      <SaveDetailModal
        :world="detailWorld"
        :summary="detailSummary"
        :modifier-state="detailModifierState"
        :loading="detailLoading"
        @close="closeWorldDetail"
        @migrate="migrateWorld"
        @set-backup="openBackupForWorld"
      />
    </template>

    <template v-else>
      <section class="backup-location" aria-labelledby="backup-location-title">
        <div>
          <h2 id="backup-location-title">备份位置</h2>
          <p>默认使用项目或程序旁的备份目录，也可以更改后续备份的存放位置。</p>
          <code>{{ backupRoot || '正在读取...' }}</code>
        </div>
        <button class="btn btn-ghost btn-sm" :disabled="loading || pendingKey !== ''" @click="pickBackupRoot">
          <FolderOpen :size="15" />更改位置
        </button>
      </section>

      <section class="create-backup" aria-labelledby="create-backup-title">
        <div>
          <h2 id="create-backup-title">创建完整备份</h2>
          <p>完整保存当前世界，保留至手动删除，不会自动清理。</p>
        </div>
        <div class="create-controls">
          <select v-model="backupWorldPath" class="input" :disabled="loading || pendingKey !== ''">
            <option value="">选择要备份的世界</option>
            <option v-for="world in allWorlds" :key="world.path" :value="world.path">
              {{ world.name }} · {{ sourceLabel(world.source) }}
            </option>
          </select>
          <button class="btn btn-primary" :disabled="!backupWorldPath || pendingKey !== ''" @click="createBackup">
            {{ pendingKey === 'create-backup' ? '正在创建备份' : '创建备份' }}
          </button>
        </div>
      </section>

      <section class="backup-section" aria-labelledby="full-backup-title">
        <div class="section-heading">
          <div>
            <h2 id="full-backup-title">完整备份</h2>
            <p>保留至手动删除，不会自动清理。</p>
          </div>
          <span class="count-label">{{ fullBackups.length }} 份</span>
        </div>
        <div v-if="fullBackups.length" class="backup-list">
          <article v-for="item in fullBackups" :key="item.id" class="backup-row">
            <div class="backup-main">
              <strong>{{ item.world_name || '未知世界' }}</strong>
              <span class="type-badge">{{ worldClassLabel(item.world_class) }}</span>
              <span v-if="item.state === 'recovery_required'" class="state-badge">需要处理</span>
            </div>
            <div class="backup-meta">
              <span>{{ formatDate(item.created_at_ms) }}</span>
              <span>{{ formatSize(item.total_size) }}</span>
              <span>{{ nullableCount(item.player_count) }} 名角色</span>
              <span>版本 {{ item.save_version || '未知' }}</span>
              <span>{{ sourceReason(item.source) }}</span>
            </div>
            <div class="backup-actions">
              <button class="btn btn-ghost btn-sm" :disabled="pendingKey !== ''" @click="requestBackupAction(item, 'restore-full')">
                <ArchiveRestore :size="15" />{{ pendingKey === `restore-${item.id}` ? '恢复中' : '恢复' }}
              </button>
              <button
                class="icon-button icon-button--danger"
                :disabled="pendingKey !== ''"
                title="删除完整备份"
                aria-label="删除完整备份"
                @click="requestBackupAction(item, 'delete-full')"
              >
                <Trash2 :size="16" />
              </button>
            </div>
          </article>
        </div>
        <div v-else-if="loading" class="empty-state">正在读取完整备份...</div>
        <div v-else class="empty-state">还没有完整备份。创建后会一直保留，直到你删除。</div>
      </section>

      <section class="backup-section" aria-labelledby="rollback-title">
        <div class="section-heading">
          <div>
            <h2 id="rollback-title">操作回滚点</h2>
            <p>迁移和角色操作前自动创建；每个世界保留最近 3 份。</p>
          </div>
          <span class="count-label">{{ operationBackups.length }} 份</span>
        </div>
        <div v-if="operationBackups.length" class="backup-list">
          <article v-for="item in operationBackups" :key="item.id" class="backup-row">
            <div class="backup-main">
              <strong>{{ item.world_name || '未知世界' }}</strong>
              <span class="type-badge">{{ worldClassLabel(item.world_class) }}</span>
              <span v-if="item.state === 'recovery_required'" class="state-badge">需要处理</span>
            </div>
            <div class="backup-meta">
              <span>{{ formatDate(item.created_at_ms) }}</span>
              <span>{{ formatSize(item.total_size) }}</span>
              <span>{{ nullableCount(item.player_count) }} 名角色</span>
              <span>版本 {{ item.save_version || '未知' }}</span>
              <span>{{ sourceReason(item.source) }}</span>
            </div>
            <div class="backup-actions">
              <button class="btn btn-ghost btn-sm" :disabled="pendingKey !== ''" @click="requestBackupAction(item, 'restore-operation')">
                <RotateCcw :size="15" />{{ pendingKey === `restore-${item.id}` ? '回滚中' : '回滚' }}
              </button>
              <button
                class="icon-button icon-button--danger"
                :disabled="pendingKey !== ''"
                title="删除操作回滚点"
                aria-label="删除操作回滚点"
                @click="requestBackupAction(item, 'delete-operation')"
              >
                <Trash2 :size="16" />
              </button>
            </div>
          </article>
        </div>
        <div v-else-if="loading" class="empty-state">正在读取操作回滚点...</div>
        <div v-else class="empty-state">最近没有可回滚的操作。迁移或角色操作前会自动创建。</div>
      </section>
    </template>

    <ConfirmDialog
      v-model:visible="confirmVisible"
      :title="confirmTitle"
      :message="confirmMessage"
      :confirm-text="confirmButton"
      :danger="confirmDanger"
      @confirm="confirmBackupAction"
      @cancel="clearConfirmAction"
    />
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { ArchiveRestore, FolderOpen, RefreshCw, RotateCcw, Trash2 } from '@lucide/vue'
import { api } from '@/api/tauri'
import AppIcon from '@/components/ui/AppIcon.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import SaveDetailModal from '@/components/save/SaveDetailModal.vue'
import { useSettingsStore } from '@/stores/settings'
import type { BackupManifest, ModifierWorldState, WorldInfo, WorldSummary } from '@/types/tauri'

type SaveTab = 'worlds' | 'backup'
type DirectoryKind = 'local' | 'server'
type BackupAction = 'restore-full' | 'delete-full' | 'restore-operation' | 'delete-operation'

const route = useRoute()
const router = useRouter()
const settingsStore = useSettingsStore()

const activeTab = ref<SaveTab>(route.query.tab === 'backup' ? 'backup' : 'worlds')
const loading = ref(false)
const errorMessage = ref('')
const localWorlds = ref<WorldInfo[]>([])
const serverWorlds = ref<WorldInfo[]>([])
const detailWorld = ref<WorldInfo | null>(null)
const detailSummary = ref<WorldSummary | null>(null)
const detailModifierState = ref<ModifierWorldState | null>(null)
const detailLoading = ref(false)

const backupRoot = ref('')
const backupWorldPath = ref('')
const fullBackups = ref<BackupManifest[]>([])
const operationBackups = ref<BackupManifest[]>([])
const pendingKey = ref('')
const serverNeedsStop = ref(false)
const deferredAction = ref<(() => Promise<void>) | null>(null)

const confirmVisible = ref(false)
const confirmTitle = ref('')
const confirmMessage = ref('')
const confirmButton = ref('确认')
const confirmDanger = ref(false)
const confirmTarget = ref<BackupManifest | null>(null)
const confirmAction = ref<BackupAction | null>(null)

const serverPathMissing = computed(() => !settingsStore.settings.server_path)
const allWorlds = computed(() => [...localWorlds.value, ...serverWorlds.value])

watch(() => route.query.tab, (tab) => {
  activeTab.value = tab === 'backup' ? 'backup' : 'worlds'
  if (activeTab.value === 'backup') void loadBackups()
})

function setTab(tab: SaveTab): void {
  activeTab.value = tab
  void router.replace({ path: '/saves', query: tab === 'backup' ? { tab: 'backup' } : {} })
  if (tab === 'backup') void loadBackups()
}

function refreshActiveTab(): void {
  if (activeTab.value === 'backup') void loadBackups()
  else void discoverWorlds()
}

async function discoverWorlds(): Promise<void> {
  loading.value = true
  errorMessage.value = ''
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
    const [localResults, serverResults] = await Promise.all([
      Promise.all([api.save.discoverLocalWorlds(), ...localRoots.map((root) => api.save.discoverLocalWorlds(root))]),
      Promise.all([api.save.discoverWorlds(), ...serverRoots.map((root) => api.save.discoverWorlds(root))]),
    ])
    localWorlds.value = dedupeWorlds(localResults.flat())
    serverWorlds.value = dedupeWorlds(serverResults.flatMap((result) => result.worlds))
  } catch (error) {
    errorMessage.value = `读取世界失败：${toMessage(error)}`
  } finally {
    loading.value = false
  }
}

function sameDirectory(left: string, right: string): boolean {
  const normalize = (value: string) => value.replace(/[\\/]+$/, '').toLocaleLowerCase()
  return normalize(left) === normalize(right)
}

function dedupeWorlds(worlds: WorldInfo[]): WorldInfo[] {
  return [...new Map(worlds.map((world) => [world.path, world])).values()].sort((left, right) => left.name.localeCompare(right.name))
}

async function pickWorldDirectory(kind: DirectoryKind): Promise<void> {
  const directory = await open({ directory: true })
  if (typeof directory !== 'string' || !directory) return
  loading.value = true
  errorMessage.value = ''
  try {
    if (kind === 'local') {
      const worlds = await api.save.discoverLocalWorlds(directory)
      localWorlds.value = dedupeWorlds([...localWorlds.value, ...worlds])
      settingsStore.update({ local_save_roots: uniqueRoots(settingsStore.settings.local_save_roots, directory) })
    } else {
      const result = await api.save.discoverWorlds(directory)
      serverWorlds.value = dedupeWorlds([...serverWorlds.value, ...result.worlds])
      settingsStore.update({ server_save_roots: uniqueRoots(settingsStore.settings.server_save_roots, directory) })
    }
    await settingsStore.save()
  } catch (error) {
    errorMessage.value = `读取所选目录失败：${toMessage(error)}`
  } finally {
    loading.value = false
  }
}

function uniqueRoots(roots: string[] | undefined, directory: string): string[] {
  return [...new Set([...(roots ?? []), directory])]
}

async function openWorld(world: WorldInfo): Promise<void> {
  detailWorld.value = world
  detailSummary.value = null
  detailModifierState.value = null
  detailLoading.value = true
  try {
    const [summary, modifierState] = await Promise.all([
      api.migration.worldSummaryByPath(world.path),
      api.modifier.getWorld(world.path),
    ])
    detailSummary.value = summary
    detailModifierState.value = modifierState
  } catch {
    detailSummary.value = null
    detailModifierState.value = null
  } finally {
    detailLoading.value = false
  }
}

function closeWorldDetail(): void {
  detailWorld.value = null
  detailSummary.value = null
  detailModifierState.value = null
}

function migrateWorld(world: WorldInfo): void {
  void router.push({ path: '/migrate', query: { source: world.path, type: 'local' } })
}

function openBackupForWorld(world: WorldInfo): void {
  backupWorldPath.value = world.path
  closeWorldDetail()
  setTab('backup')
}

async function loadBackups(): Promise<void> {
  loading.value = true
  errorMessage.value = ''
  try {
    const [root, full, recent] = await Promise.all([
      api.save.getBackupRoot(),
      api.save.listFullBackups(),
      api.save.listSnapshots(),
    ])
    backupRoot.value = root
    fullBackups.value = sortNewest(full)
    operationBackups.value = sortNewest(recent)
  } catch (error) {
    errorMessage.value = `读取备份失败：${toMessage(error)}`
  } finally {
    loading.value = false
  }
}

function sortNewest(items: BackupManifest[]): BackupManifest[] {
  return [...items].sort((left, right) => right.created_at_ms - left.created_at_ms)
}

async function pickBackupRoot(): Promise<void> {
  const directory = await open({ directory: true })
  if (typeof directory !== 'string' || !directory) return
  pendingKey.value = 'change-root'
  errorMessage.value = ''
  try {
    settingsStore.update({ backup_root: directory })
    await settingsStore.save()
    await api.save.rebuildBackupIndex()
    await loadBackups()
  } catch (error) {
    errorMessage.value = `更改备份位置失败：${toMessage(error)}`
  } finally {
    pendingKey.value = ''
  }
}

function createBackup(): void {
  const world = allWorlds.value.find((item) => item.path === backupWorldPath.value)
  if (!world) return
  void gateServerAndRun(async () => {
    pendingKey.value = 'create-backup'
    errorMessage.value = ''
    try {
      await api.save.createFullBackup(
        world.path,
        world.guid || world.name,
        world.name,
        world.source === 'server' ? 'server' : 'local',
        'manual',
      )
      await loadBackups()
    } catch (error) {
      errorMessage.value = `创建备份失败：${toMessage(error)}`
    } finally {
      pendingKey.value = ''
    }
  })
}

function requestBackupAction(item: BackupManifest, action: BackupAction): void {
  confirmTarget.value = item
  confirmAction.value = action
  const deleting = action.startsWith('delete')
  const operation = action === 'restore-operation' ? '回滚' : action === 'restore-full' ? '恢复' : '删除'
  confirmTitle.value = `${operation}“${item.world_name || '未知世界'}”？`
  confirmMessage.value = deleting
    ? '删除后无法恢复，但不会影响当前世界。'
    : `将使用 ${formatDate(item.created_at_ms)} 的记录覆盖当前世界。操作前请确保没有玩家在线。`
  confirmButton.value = operation
  confirmDanger.value = deleting
  confirmVisible.value = true
}

function confirmBackupAction(): void {
  const target = confirmTarget.value
  const action = confirmAction.value
  if (!target || !action) return
  clearConfirmAction()
  if (action.startsWith('restore')) void gateServerAndRun(() => restoreBackup(target, action))
  else void deleteBackup(target, action)
}

async function restoreBackup(item: BackupManifest, action: BackupAction): Promise<void> {
  pendingKey.value = `restore-${item.id}`
  errorMessage.value = ''
  try {
    if (action === 'restore-full') await api.save.restoreFullBackup(item.id)
    else await api.save.restoreSnapshot(item.id)
    await loadBackups()
  } catch (error) {
    errorMessage.value = `恢复失败：${toMessage(error)}`
  } finally {
    pendingKey.value = ''
  }
}

async function deleteBackup(item: BackupManifest, action: BackupAction): Promise<void> {
  pendingKey.value = `delete-${item.id}`
  errorMessage.value = ''
  try {
    if (action === 'delete-full') await api.save.deleteFullBackup(item.id)
    else await api.save.deleteSnapshot(item.id)
    await loadBackups()
  } catch (error) {
    errorMessage.value = `删除失败：${toMessage(error)}`
  } finally {
    pendingKey.value = ''
  }
}

function clearConfirmAction(): void {
  confirmTarget.value = null
  confirmAction.value = null
}

async function gateServerAndRun(action: () => Promise<void>): Promise<void> {
  if (pendingKey.value) return
  pendingKey.value = 'check-server'
  errorMessage.value = ''
  try {
    const status = await api.server.getStatus()
    if (status.running) {
      deferredAction.value = action
      serverNeedsStop.value = true
      return
    }
    pendingKey.value = ''
    await action()
  } catch (error) {
    errorMessage.value = `无法检查服务器：${toMessage(error)}`
  } finally {
    if (pendingKey.value === 'check-server') pendingKey.value = ''
  }
}

async function stopServerAndContinue(): Promise<void> {
  const action = deferredAction.value
  if (!action || pendingKey.value) return
  pendingKey.value = 'stop-server'
  try {
    await api.server.stop()
    serverNeedsStop.value = false
    deferredAction.value = null
    pendingKey.value = ''
    await action()
  } catch (error) {
    errorMessage.value = `停止服务器失败：${toMessage(error)}`
    pendingKey.value = ''
  }
}

function cancelDeferredAction(): void {
  deferredAction.value = null
  serverNeedsStop.value = false
}

function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
}

function formatDate(timestamp: number): string {
  if (!timestamp) return '时间未知'
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit',
  }).format(new Date(timestamp))
}

function nullableCount(value: number | null): string {
  return value === null ? '未知' : String(value)
}

function sourceLabel(source: string): string {
  if (source === 'server') return '服务器世界'
  if (source === 'steam') return 'Steam 单机'
  if (source === 'appdata') return '本机单机'
  return '本机目录'
}

function worldClassLabel(worldClass: string): string {
  return worldClass === 'server' ? '服务器世界' : '本机世界'
}

function sourceReason(source: string): string {
  const labels: Record<string, string> = {
    manual: '手动创建',
    world_migration: '世界迁移',
    character_transfer: '完整角色转移',
    character_import: '朋友角色导入',
    guild_recovery: '恢复原公会',
  }
  return labels[source] ?? '来源未知'
}

function toMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

onMounted(async () => {
  await discoverWorlds()
  if (activeTab.value === 'backup') await loadBackups()
})
</script>

<style scoped>
.save-screen { gap: 16px; }
.page-tabs { display: flex; gap: 4px; padding-bottom: 8px; border-bottom: 1px solid var(--glass-border); }
.page-tabs button { height: 34px; padding: 0 14px; border: 0; border-radius: 8px; background: transparent; color: var(--text-mid); font: 600 13px var(--font-ui); cursor: pointer; }
.page-tabs button.active { background: var(--primary-soft); color: var(--primary-active); }
.notice { display: flex; align-items: center; gap: 10px; padding: 11px 14px; border: 1px solid var(--glass-border); border-radius: 8px; background: rgba(255, 252, 247, .74); color: var(--text-mid); font-size: 13px; }
.notice > span { flex: 1; }
.notice a { color: var(--primary-active); font-weight: 600; }
.notice--warn { border-color: rgba(184, 120, 47, .34); background: var(--amber-bg); }
.notice--error { border-color: rgba(201, 85, 77, .38); background: var(--red-bg); color: var(--red-soft); }
.notice--action { border-color: rgba(230, 111, 81, .34); }
.world-section,
.backup-section { display: flex; flex-direction: column; gap: 12px; }
.section-heading { display: flex; align-items: center; justify-content: space-between; gap: 14px; }
.section-heading h2,
.backup-location h2,
.create-backup h2 { margin: 0; font-size: 15px; color: var(--text-hi); }
.section-heading p,
.backup-location p,
.create-backup p { margin-top: 4px; color: var(--text-mid2); font-size: 12px; line-height: 1.5; }
.world-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 10px; }
.world-card { display: flex; align-items: center; gap: 11px; min-width: 0; padding: 13px 14px; border: 1px solid var(--glass-border); border-radius: 8px; background: var(--glass-bg); text-align: left; cursor: pointer; }
.world-card:hover { border-color: rgba(230, 111, 81, .4); }
.world-icon { flex: 0 0 auto; color: var(--primary); }
.world-card__body { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.world-card__body strong { overflow: hidden; color: var(--text-hi); font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.world-card__body small { color: var(--text-mid2); font-size: 11px; }
.empty-state { padding: 16px; border: 1px dashed var(--glass-border); border-radius: 8px; color: var(--text-mid2); font-size: 13px; }
.backup-location,
.create-backup { display: flex; align-items: center; justify-content: space-between; gap: 18px; padding-bottom: 16px; border-bottom: 1px solid var(--glass-border); }
.backup-location > div,
.create-backup > div { min-width: 0; }
.backup-location code { display: block; max-width: 720px; margin-top: 7px; overflow: hidden; color: var(--text-hi); font: 12px var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
.create-controls { display: flex; align-items: center; gap: 8px; flex: 0 1 520px; }
.create-controls .input { flex: 1; min-width: 210px; font-family: var(--font-ui); }
.count-label { color: var(--text-mid2); font-size: 12px; }
.backup-list { display: flex; flex-direction: column; gap: 7px; }
.backup-row { display: grid; grid-template-columns: minmax(160px, .8fr) minmax(360px, 2fr) auto; align-items: center; gap: 14px; padding: 12px 14px; border: 1px solid var(--glass-border); border-radius: 8px; background: var(--glass-bg); }
.backup-main { display: flex; align-items: center; gap: 7px; min-width: 0; }
.backup-main strong { overflow: hidden; color: var(--text-hi); font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.type-badge,
.state-badge { flex: 0 0 auto; padding: 3px 6px; border-radius: 5px; background: var(--primary-soft); color: var(--primary-active); font-size: 10px; font-weight: 600; }
.state-badge { background: var(--red-bg); color: var(--red-soft); }
.backup-meta { display: flex; align-items: center; gap: 8px 14px; min-width: 0; flex-wrap: wrap; color: var(--text-mid2); font-size: 11px; }
.backup-meta span { white-space: nowrap; }
.backup-actions { display: flex; justify-content: flex-end; align-items: center; gap: 6px; }
.icon-button { display: inline-flex; align-items: center; justify-content: center; width: 34px; height: 34px; border: 1px solid var(--glass-border); border-radius: 8px; background: var(--glass-bg-soft); color: var(--text-mid); cursor: pointer; }
.icon-button--danger { color: var(--red-soft); }
.icon-button:disabled { opacity: .5; cursor: not-allowed; }
@media (max-width: 980px) {
  .backup-row { grid-template-columns: minmax(0, 1fr) auto; }
  .backup-meta { grid-column: 1 / -1; grid-row: 2; }
  .backup-actions { grid-column: 2; grid-row: 1; }
}
@media (max-width: 720px) {
  .page-head,
  .section-heading,
  .backup-location,
  .create-backup { align-items: stretch; flex-direction: column; }
  .create-controls { flex: 0 0 auto; flex-direction: column; align-items: stretch; }
  .create-controls .input { min-width: 0; width: 100%; }
  .world-grid { grid-template-columns: 1fr; }
  .backup-row { grid-template-columns: minmax(0, 1fr); }
  .backup-actions { grid-column: 1; grid-row: 3; justify-content: flex-start; }
  .backup-meta { grid-column: 1; }
  .notice { align-items: stretch; flex-wrap: wrap; }
}
</style>
