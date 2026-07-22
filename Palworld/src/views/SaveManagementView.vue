<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">存档管理</div>
        <div class="page-sub">
          整包世界备份/恢复（P0）+ 角色跨服导出/导入（P1）。所有操作均为纯文件拷贝，不解析/改写存档内容，安全且版本无关。
        </div>
      </div>
      <div class="page-actions">
        <button class="btn btn-ghost" :disabled="loading" @click="onDiscover">刷新检测</button>
      </div>
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

    <!-- 世界选择 -->
    <div class="sm-section">
      <div class="section-title">选择世界</div>
      <div v-if="worlds.length" class="world-grid">
        <button
          v-for="w in worlds"
          :key="w.name"
          class="world-card"
          :class="{ active: w.name === worldName }"
          @click="onSelectWorld(w.name)"
        >
          <AppIcon name="save" :size="20" class="wc-icon" />
          <div class="wc-info">
            <span class="wc-name">{{ w.name }}</span>
            <span class="wc-meta">{{ w.player_count }} 名角色 · {{ formatSize(w.size_bytes) }}</span>
          </div>
        </button>
      </div>
      <div v-else class="sm-empty">
        尚未发现任何世界（需含 Level.sav）。请先在设置中确认 server_path，或启动过一次服务器生成世界。
      </div>
    </div>

    <!-- P0 世界备份 / 恢复 -->
    <div v-if="worldName" class="sm-section">
      <div class="section-title">世界备份 / 恢复（P0）</div>
      <div class="sm-toolbar">
        <button class="btn btn-primary btn-sm" :disabled="loading" @click="onBackup">
          {{ loading ? '处理中…' : '备份当前世界' }}
        </button>
        <span class="sm-hint">默认备份到 &lt;SaveGames&gt;/_backups/{{ worldName }}/&lt;时间戳&gt;/</span>
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
      <div v-else class="sm-empty sm-empty--sm">该世界暂无备份，点上方按钮创建第一个备份。</div>
    </div>

    <!-- P1 角色导出 / 导入 -->
    <div v-if="worldName" class="sm-section">
      <div class="section-title">角色跨服转移（P1）</div>
      <div class="char-row">
        <label class="char-label">SteamID / 玩家 ID</label>
        <input
          v-model="steamId"
          class="input char-input"
          placeholder="如 7656119xxxxxxxxxx（保持原 ID，不跨账号改写）"
        />
      </div>
      <div class="sm-toolbar">
        <button class="btn btn-ghost btn-sm" :disabled="!canCharOp" @click="onExport">导出角色存档…</button>
        <button class="btn btn-ghost btn-sm" :disabled="!canCharOp" @click="onImport">导入角色存档…</button>
      </div>
      <div class="sm-warn">
        <AppIcon name="info" :size="14" />
        <span>
          首版仅拷贝 <code>Players/&lt;id&gt;.sav</code>，<strong>不迁移公会归属</strong>（GroupSaveDataMap），
          且需<strong>先停服</strong>再操作，避免被自动保存覆盖。导入前请先在该服用同一账号建好角色。
        </span>
      </div>
    </div>

    <!-- 预置存档（老板要的接口占位，不做完整 presets 体系） -->
    <div class="sm-section">
      <div class="section-title">预置存档（接口预留）</div>
      <div class="preset-card">
        <AppIcon name="save" :size="22" class="preset-icon" />
        <div class="preset-info">
          <span class="preset-title">整包世界模板分发（预留）</span>
          <span class="preset-desc">
            把整套世界存档（Level.sav + LevelMeta.sav + Players/）做成"种子服"分发的入口已预留在此。
            注意：此处的"预置"指<strong>游戏存档模板</strong>，与配置预设（presets.rs）完全不同。当前版本仅预留接口，完整预设体系后续开放。
          </span>
        </div>
        <button class="btn btn-ghost btn-sm" @click="onPresetReserved">敬请期待</button>
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
import { useToast } from '@/components/ui/useToast'
import { api } from '@/api/tauri'
import { open, save } from '@tauri-apps/plugin-dialog'
import AppIcon from '@/components/ui/AppIcon.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import type { WorldInfo, WorldBackupInfo } from '@/types/tauri'

const toast = useToast()

const loading = ref(false)
const worlds = ref<WorldInfo[]>([])
const worldName = ref('')
const saveRoot = ref('')
const autoDiscovered = ref(false)
const backups = ref<WorldBackupInfo[]>([])

const steamId = ref('')

// 恢复二次确认状态
const restoreVisible = ref(false)
const restoreTarget = ref<WorldBackupInfo | null>(null)
const restoreTitle = ref('恢复世界备份')
const restoreMessage = ref('')

const canCharOp = computed(() => !!worldName.value && !!steamId.value.trim())

function formatSize(bytes: number): string {
  if (!bytes) return '0 MB'
  const mb = bytes / (1024 * 1024)
  if (mb < 1) return `${(bytes / 1024).toFixed(0)} KB`
  return `${mb.toFixed(1)} MB`
}

async function onDiscover(): Promise<void> {
  loading.value = true
  try {
    const res = await api.save.discoverWorlds()
    worlds.value = res.worlds
    saveRoot.value = res.save_root
    autoDiscovered.value = res.auto_discovered
    if (res.worlds.length && !res.worlds.some((w) => w.name === worldName.value)) {
      await onSelectWorld(res.worlds[0].name)
    } else if (worldName.value) {
      await loadBackups()
    }
    toast.info(`已发现 ${res.worlds.length} 个世界`)
  } catch (e) {
    toast.error(`检测世界失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    loading.value = false
  }
}

async function onSelectWorld(name: string): Promise<void> {
  worldName.value = name
  await loadBackups()
}

async function loadBackups(): Promise<void> {
  if (!worldName.value) return
  try {
    backups.value = await api.save.listWorldBackups(worldName.value)
  } catch (e) {
    toast.error(`读取备份列表失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

async function onBackup(): Promise<void> {
  if (!worldName.value) return
  loading.value = true
  try {
    const msg = await api.save.backupWorld(worldName.value)
    toast.success(msg)
    await loadBackups()
  } catch (e) {
    toast.error(`备份失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    loading.value = false
  }
}

function onRestoreClick(b: WorldBackupInfo): void {
  restoreTarget.value = b
  restoreTitle.value = `恢复世界「${worldName.value}」`
  restoreMessage.value = `即将用备份「${b.backup_id}」（${formatSize(b.size_bytes)}）整体覆盖当前世界目录。此操作会替换现有世界数据，且需确保服务器已停止。确认继续?`
  restoreVisible.value = true
}

async function onRestoreConfirm(): Promise<void> {
  if (!restoreTarget.value || !worldName.value) return
  const bid = restoreTarget.value.backup_id
  try {
    const msg = await api.save.restoreWorld(worldName.value, bid)
    toast.success(msg)
  } catch (e) {
    toast.error(`恢复失败: ${e instanceof Error ? e.message : String(e)}`)
  }
  restoreTarget.value = null
}

async function onExport(): Promise<void> {
  if (!canCharOp.value) return
  const defaultName = `${steamId.value.trim()}.sav`
  const out = await save({
    defaultPath: defaultName,
    filters: [{ name: 'Palworld 角色存档', extensions: ['sav'] }],
  })
  if (typeof out !== 'string') return
  try {
    const msg = await api.save.exportCharacter(worldName.value, steamId.value.trim(), out)
    toast.success(msg)
  } catch (e) {
    toast.error(`导出失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

async function onImport(): Promise<void> {
  if (!canCharOp.value) return
  const picked = await open({
    multiple: false,
    filters: [{ name: 'Palworld 角色存档', extensions: ['sav'] }],
  })
  if (typeof picked !== 'string') return
  try {
    const msg = await api.save.importCharacter(worldName.value, steamId.value.trim(), picked)
    toast.success(msg)
  } catch (e) {
    toast.error(`导入失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

function onPresetReserved(): void {
  toast.info('预置存档为预留接口，完整模板分发体系将在后续版本开放')
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
</style>
