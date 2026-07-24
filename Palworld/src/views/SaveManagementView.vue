<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">本地存档</div>
        <div class="page-sub">
          双检测：本机单机存档（Steam）与服务器存档（专用服）并列呈现。所有操作均为纯文件拷贝，安全且版本无关；单机档可一键迁移到服务器。
        </div>
      </div>
      <div class="page-actions">
        <button class="btn btn-ghost" :disabled="loading" @click="onDiscover">刷新检测</button>
      </div>
    </div>

    <!-- 未设置服务器路径引导卡（Q8：不静默扫默认目录）-->
    <div v-if="serverPathMissing" class="sm-banner">
      <AppIcon name="info" :size="15" />
      <span>尚未设置服务器路径，请到【设置】填写 PalServer 根目录后再来本页操作。本页所有存档发现均基于该路径动态定位。</span>
    </div>

    <!-- 自动发现提示 -->
    <div v-if="autoDiscovered" class="sm-banner">
      <AppIcon name="info" :size="15" />
      <span>
        未在 server_path 直拼下找到存档，已自动向上扫描定位到：
        <code>{{ saveRoot }}</code>。请确认这正是你的帕鲁世界存档位置。
      </span>
    </div>
    <div v-else-if="saveRoot" class="sm-banner sm-banner--ok">
      <AppIcon name="info" :size="15" />
      <span>存档根目录：<code>{{ saveRoot }}</code></span>
    </div>

    <!-- 区块 A：本地单机存档（Steam + AppData） -->
    <div class="sm-section">
      <div class="section-title">本地单机存档</div>
      <div v-if="localWorlds.length" class="world-grid">
        <div
          v-for="w in localWorlds"
          :key="'local-' + w.path"
          class="world-card"
          :class="{ active: expanded && expanded.path === w.path }"
          @click="onToggleExpand(w)"
        >
          <AppIcon name="save" :size="20" class="wc-icon" />
          <div class="wc-info">
            <span class="wc-name">{{ w.name }}</span>
            <span class="wc-meta">{{ w.player_count }} 名角色 · {{ formatSize(w.size_bytes) }}</span>
            <span class="wc-source" :class="'src-' + w.source">{{ sourceLabel(w.source) }}</span>
          </div>
        </div>
      </div>
      <div v-else class="sm-empty">
        未发现本机单机存档（已扫描 Steam 库与 AppData Local Pal）。如有单机档在其他位置，可手动选择目录。
      </div>

      <!-- 手动选择目录兜底 -->
      <div class="sm-toolbar" style="margin-top: 12px">
        <button class="btn btn-ghost btn-sm" :disabled="loading" @click="onPickLocalDir">手动选择目录…</button>
        <span class="sm-hint">选中的目录将作为额外扫描根重扫并合并（支持 AppData / Steam 库 / 自定义位置）</span>
      </div>
    </div>

    <!-- 区块 B：服务器存档（专用服）— 与本地单机同款可展开（Q3 统一设计） -->
    <div class="sm-section">
      <div class="section-title">服务器存档（专用服）</div>
      <div v-if="serverWorlds.length" class="world-grid">
        <div
          v-for="w in serverWorlds"
          :key="'srv-' + w.name"
          class="world-card"
          :class="{ active: expanded && expanded.path === w.path }"
          @click="onToggleExpand(w)"
        >
          <AppIcon name="save" :size="20" class="wc-icon" />
          <div class="wc-info">
            <span class="wc-name">{{ w.name }}</span>
            <span class="wc-meta">{{ w.player_count }} 名角色 · {{ formatSize(w.size_bytes) }}</span>
            <span class="wc-source src-server">专用服</span>
          </div>
        </div>
      </div>
      <div v-else class="sm-empty">
        尚未发现任何服务器世界（需含 Level.sav）。请先在设置中确认 server_path，或启动过一次服务器生成世界。
      </div>

      <!-- 手动选择目录兜底（与本地单机一致，防止发现失败） -->
      <div class="sm-toolbar" style="margin-top: 12px">
        <button class="btn btn-ghost btn-sm" :disabled="loading" @click="onPickServerDir">手动选择目录…</button>
        <span class="sm-hint">选中的目录将作为额外扫描根重扫并合并（支持直接选 SaveGames，或选含 SaveGames 的父目录）</span>
      </div>
    </div>

    <!-- 存档详情弹窗（点击 world 卡弹出，本地/服务器通用；按真实路径解析玩家列表） -->
    <SaveDetailModal
      :world="expanded"
      :summary="worldSummary"
      :loading="worldSummaryLoading"
      @close="expanded = null"
      @migrate="onMigrateToServer"
      @set-backup="(w) => onSelectWorld(w)"
    />

    <div v-if="selectedWorld" class="sm-section">
      <div class="section-title">世界备份 / 恢复 · {{ selectedWorld?.name }}</div>
      <div class="sm-toolbar" style="margin-bottom:10px">
        <span class="sm-hint">选择要备份/恢复的世界：</span>
        <select class="world-pick" :value="selectedWorld?.path" @change="onPickWorld($event)">
          <option v-for="w in allWorlds" :key="w.path" :value="w.path">
            {{ w.name }}（{{ sourceLabel(w.source) }}）
          </option>
        </select>
      </div>
      <div class="sm-toolbar">
        <button class="btn btn-primary btn-sm" :disabled="loading" @click="onBackup">
          {{ loading ? '处理中…' : '备份当前世界' }}
        </button>
        <button class="btn btn-ghost btn-sm" :disabled="loading" @click="onPickBackupDir">选择存放目录…</button>
        <span class="sm-hint" v-if="backupDest">
          将备份到：<code>{{ backupDest }}</code>（取消可选默认位置）
        </span>
        <span class="sm-hint" v-else>默认备份到 &lt;世界同级&gt;/_backups/{{ selectedWorld?.name }}/&lt;时间戳&gt;/</span>
      </div>

      <div v-if="backups.length" class="backup-list">
        <div v-for="b in backups" :key="b.backup_id" class="backup-row">
          <div class="backup-info">
            <span class="backup-id">{{ b.backup_id }}</span>
            <span class="backup-meta">{{ b.created_at }} · {{ formatSize(b.size_bytes) }}</span>
          </div>
          <button class="btn btn-ghost btn-sm" @click="onRestoreClick(b)">恢复到此世界</button>
        </div>
      </div>
      <div v-else class="sm-empty sm-empty--sm">该世界暂无默认位置备份，点上方按钮创建第一个备份（或指定自定义存放目录）。</div>

      <!-- 从自定义目录恢复（与指定文件夹存放对应） -->
      <div class="sm-toolbar" style="margin-top: 12px">
        <button class="btn btn-ghost btn-sm" @click="onRestoreFromDir">从自定义目录恢复…</button>
        <span class="sm-hint">选择先前「指定文件夹」备份出来的世界目录，整体覆盖回当前世界。</span>
      </div>
    </div>

    <!-- 恢复二次确认 -->
    <ConfirmDialog
      v-model:visible="restoreVisible"
      :title="restoreTitle"
      :message="restoreMessage"
      :danger="true"
      @confirm="onRestoreConfirm"
    />
  </section>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useToast } from '@/components/ui/useToast'
import { useSettingsStore } from '@/stores/settings'
import { api } from '@/api/tauri'
import { open } from '@tauri-apps/plugin-dialog'
import AppIcon from '@/components/ui/AppIcon.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import SaveDetailModal from '@/components/save/SaveDetailModal.vue'
import type { WorldInfo, WorldBackupInfo, WorldSummary } from '@/types/tauri'

const toast = useToast()
const settingsStore = useSettingsStore()
const router = useRouter()

// server_path 缺失时不静默扫默认目录，显式引导去设置（Q8）
const serverPathMissing = computed(() => !settingsStore.settings.server_path)

const loading = ref(false)
const serverWorlds = ref<WorldInfo[]>([])
const localWorlds = ref<WorldInfo[]>([])
const selectedWorld = ref<WorldInfo | null>(null)
// 所有世界（服务器 + 本地单机）供备份/恢复世界选择器使用
const allWorlds = computed(() => [...serverWorlds.value, ...localWorlds.value])
const saveRoot = ref('')
const autoDiscovered = ref(false)
const backups = ref<WorldBackupInfo[]>([])
// 备份自定义存放目录（指定文件夹存放场景）
const backupDest = ref('')
// 从自定义目录恢复时的源路径
const restoreFromPath = ref('')

// R5：点击展开的本地存档信息面板
const expanded = ref<WorldInfo | null>(null)
const worldSummary = ref<WorldSummary | null>(null)
const worldSummaryLoading = ref(false)

// 恢复二次确认状态
const restoreVisible = ref(false)
const restoreTarget = ref<WorldBackupInfo | null>(null)
const restoreTitle = ref('恢复世界备份')
const restoreMessage = ref('')

function formatSize(bytes: number): string {
  if (!bytes) return '0 MB'
  const mb = bytes / (1024 * 1024)
  if (mb < 1) return `${(bytes / 1024).toFixed(0)} KB`
  return `${mb.toFixed(1)} MB`
}

// R5：来源标签
function sourceLabel(s: string): string {
  if (s === 'steam') return 'Steam 单机'
  if (s === 'appdata') return 'AppData 单机'
  if (s === 'server') return '专用服'
  return '本机'
}

// 并行加载：专用服存档（discoverWorlds）+ 本机单机存档（discoverLocalWorlds）
async function onDiscover(): Promise<void> {
  loading.value = true
  try {
    const [serverRes, localRes] = await Promise.all([
      api.save.discoverWorlds(),
      api.save.discoverLocalWorlds(),
    ])
    serverWorlds.value = serverRes.worlds
    saveRoot.value = serverRes.save_root
    autoDiscovered.value = serverRes.auto_discovered
    localWorlds.value = localRes

    // 维持已选世界，否则默认选中第一个服务器世界（没有则第一个本地世界）
    if (!selectedWorld.value) {
      const first = serverRes.worlds[0] ?? localRes[0]
      if (first) await onSelectWorld(first)
    }
    toast.info(`服务器 ${serverRes.worlds.length} 个世界 · 单机 ${localRes.length} 个世界`)
  } catch (e) {
    toast.error(`检测世界失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    loading.value = false
  }
}

async function onSelectWorld(w: WorldInfo): Promise<void> {
  selectedWorld.value = w
  await loadBackups()
}

// 选择器变更：按真实路径定位世界
function onPickWorld(e: Event): void {
  const p = (e.target as HTMLSelectElement).value
  const w = allWorlds.value.find((x) => x.path === p)
  if (w) onSelectWorld(w)
}

async function loadBackups(): Promise<void> {
  if (!selectedWorld.value?.path) return
  try {
    backups.value = await api.save.listWorldBackups(selectedWorld.value.path)
  } catch (e) {
    toast.error(`读取备份列表失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

async function onBackup(): Promise<void> {
  if (!selectedWorld.value) return
  loading.value = true
  try {
    const msg = await api.save.backupWorld(selectedWorld.value.path, backupDest.value || undefined)
    toast.success(msg)
    backupDest.value = ''
    await loadBackups()
  } catch (e) {
    toast.error(`备份失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    loading.value = false
  }
}

function onRestoreClick(b: WorldBackupInfo): void {
  restoreTarget.value = b
  restoreFromPath.value = ''
  restoreTitle.value = `恢复世界「${selectedWorld.value?.name ?? ''}」`
  restoreMessage.value = `即将用备份「${b.backup_id}」（${formatSize(b.size_bytes)}）整体覆盖当前世界目录。此操作会替换现有世界数据，且需确保服务器已停止。确认继续?`
  restoreVisible.value = true
}

async function onRestoreConfirm(): Promise<void> {
  if (!selectedWorld.value) return
  try {
    if (restoreFromPath.value) {
      // 从自定义目录恢复
      const msg = await api.save.restoreWorldFrom(selectedWorld.value.path, restoreFromPath.value)
      toast.success(msg)
    } else if (restoreTarget.value) {
      // 从默认 _backups 列表恢复
      const msg = await api.save.restoreWorld(selectedWorld.value.path, restoreTarget.value.backup_id)
      toast.success(msg)
    }
  } catch (e) {
    toast.error(`恢复失败: ${e instanceof Error ? e.message : String(e)}`)
  }
  restoreTarget.value = null
  restoreFromPath.value = ''
}

// 本地单机存档 → 服务器：跳转迁移页并预选源（path + type=local）
function onMigrateToServer(w: WorldInfo): void {
  router.push({ path: '/migrate', query: { source: w.path, type: 'local' } })
  toast.info(`已跳转迁移页，源档预选：${w.name}`)
}

// R5：点击 world 卡展开信息面板（Level.sav 概要用现有 save_edit 能力，单机档失败优雅降级）
async function onToggleExpand(w: WorldInfo): Promise<void> {
  if (expanded.value && expanded.value.path === w.path) {
    expanded.value = null
    worldSummary.value = null
    return
  }
  expanded.value = w
  worldSummaryLoading.value = true
  worldSummary.value = null
  try {
    // 本地单机存档不在服务器 SaveGames 根下，必须按真实路径解析（f5_world_summary_by_path）
    worldSummary.value = await api.migration.worldSummaryByPath(w.path)
  } catch {
    worldSummary.value = null
  } finally {
    worldSummaryLoading.value = false
  }
}

// R5：手动选目录兜底——选中目录作为额外扫描根重扫并合并
async function onPickLocalDir(): Promise<void> {
  const dir = await open({ directory: true })
  if (typeof dir !== 'string' || !dir) return
  loading.value = true
  try {
    const extra = await api.save.discoverLocalWorlds(dir)
    const map = new Map(localWorlds.value.map((w) => [w.path, w]))
    for (const w of extra) {
      if (!map.has(w.path)) map.set(w.path, w)
    }
    localWorlds.value = [...map.values()].sort((a, b) => a.name.localeCompare(b.name))
    toast.info(`手动目录扫描到 ${extra.length} 个世界，已合并`)
  } catch (e) {
    toast.error(`手动扫描失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    loading.value = false
  }
}

// 服务器存档手动选目录兜底（与本地单机一致，防止发现失败）
async function onPickServerDir(): Promise<void> {
  const dir = await open({ directory: true })
  if (typeof dir !== 'string' || !dir) return
  loading.value = true
  try {
    const res = await api.save.discoverWorlds(dir)
    const map = new Map(serverWorlds.value.map((w) => [w.path, w]))
    for (const w of res.worlds) {
      if (!map.has(w.path)) map.set(w.path, w)
    }
    serverWorlds.value = [...map.values()].sort((a, b) => a.name.localeCompare(b.name))
    saveRoot.value = res.save_root
    autoDiscovered.value = res.auto_discovered
    toast.info(`手动目录扫描到 ${res.worlds.length} 个服务器世界，已合并`)
  } catch (e) {
    toast.error(`服务器目录扫描失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    loading.value = false
  }
}

// 选择备份存放目录（指定文件夹存放）
async function onPickBackupDir(): Promise<void> {
  const dir = await open({ directory: true })
  if (typeof dir !== 'string' || !dir) return
  backupDest.value = dir
}

// 从自定义目录恢复（与指定文件夹存放对应）
async function onRestoreFromDir(): Promise<void> {
  if (!selectedWorld.value) return
  const dir = await open({ directory: true })
  if (typeof dir !== 'string' || !dir) return
  restoreFromPath.value = dir
  restoreTarget.value = null
  restoreTitle.value = `从自定义目录恢复世界「${selectedWorld.value?.name ?? ''}」`
  restoreMessage.value = `即将用目录「${dir}」整体覆盖当前世界目录。此操作会替换现有世界数据，且需确保服务器已停止。确认继续?`
  restoreVisible.value = true
}

// 进入页面即自动检测一次
void onDiscover()
</script>

<style scoped>
.sm-banner {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 14px;
  border-radius: var(--r-card);
  background: var(--amber-bg, rgba(184, 120, 47, 0.14));
  border: 1px solid rgba(184, 120, 47, 0.3);
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-mid, #77675f);
}
.sm-banner--ok {
  background: var(--green-bg, rgba(79, 138, 107, 0.14));
  border-color: rgba(79, 138, 107, 0.3);
}
.sm-banner code,
.sm-hint code,
.sm-warn code,
.preset-desc code {
  font-family: var(--font-mono);
  font-size: 12px;
  background: rgba(116, 88, 72, 0.1);
  padding: 1px 5px;
  border-radius: 5px;
}
.sm-section {
  margin-top: 8px;
}
.section-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
  margin-bottom: 12px;
}
.world-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
}
.world-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
  border-radius: var(--r-card);
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s ease, background 0.15s ease;
}
.world-card:hover {
  border-color: rgba(230, 111, 81, 0.4);
}
.world-card.active {
  border-color: var(--primary, #e66f51);
  background: var(--glass-bg-strong, rgba(255, 250, 244, 0.88));
  box-shadow: 0 0 0 1px rgba(230, 111, 81, 0.12);
}
.wc-icon {
  color: var(--primary, #e66f51);
  flex: 0 0 20px;
}
.wc-info {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}
.wc-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wc-meta {
  font-size: 12px;
  color: var(--text-mid2, #8a7a6e);
}
.sm-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}
.sm-hint {
  font-size: 12px;
  color: var(--text-mid2, #8a7a6e);
}
.backup-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.backup-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 12px;
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
}
.backup-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.backup-id {
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
}
.backup-meta {
  font-size: 12px;
  color: var(--text-mid2, #8a7a6e);
}
.sm-empty {
  padding: 16px;
  border-radius: 12px;
  background: var(--glass-bg-soft, rgba(255, 250, 244, 0.5));
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
  font-size: 13px;
  color: var(--text-mid2, #8a7a6e);
  line-height: 1.6;
}
.sm-empty--sm {
  padding: 10px 14px;
  font-size: 12px;
}
.char-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}
.char-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-mid, #77675f);
  white-space: nowrap;
}
.char-input {
  max-width: 420px;
}
.sm-warn {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  margin-top: 10px;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.03);
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-mid2, #8a7a6e);
}
.sm-warn :deep(svg),
.sm-banner :deep(svg) {
  flex: 0 0 14px;
  margin-top: 2px;
  color: var(--primary, #e66f51);
}
.preset-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 16px 18px;
  border-radius: var(--r-card);
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: 1px dashed var(--glass-border, rgba(116, 88, 72, 0.2));
}
.preset-icon {
  color: var(--primary, #e66f51);
  flex: 0 0 22px;
}
.preset-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}
.preset-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
}
.preset-desc {
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-mid2, #8a7a6e);
}

/* ====== R5：本地存档来源标签 / 展开信息面板 / 手动目录 ====== */
.wc-source {
  font-size: 11px;
  font-weight: 600;
  padding: 1px 8px;
  border-radius: 999px;
  width: fit-content;
}
.wc-source.src-steam { background: rgba(155, 106, 158, 0.14); color: #9b6a9e; }
.wc-source.src-appdata { background: rgba(79, 138, 107, 0.14); color: var(--green, #4f8a6b); }
.wc-source.src-server { background: rgba(230, 111, 81, 0.14); color: var(--primary, #e66f51); }
.wc-source.src-unknown { background: rgba(116, 88, 72, 0.12); color: var(--text-mid2, #8a7a6e); }

/* ====== 备份/恢复世界选择器（暖色玻璃风，复用既有 token） ====== */
.world-pick {
  max-width: 320px;
  padding: 8px 12px;
  border-radius: 10px;
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.18));
  color: var(--palwarm-text-primary, #3f322c);
  font-size: 13px;
}
</style>
