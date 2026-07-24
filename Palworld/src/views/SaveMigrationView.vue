<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">数据迁移</div>
        <div class="page-sub">
          本地/联机存档 → 专用服迁移（整包世界迁移 + 修复主机存档）。F5 解析改写支线，与「本地存档」纯拷贝安全路径互不干扰。
        </div>
      </div>
      <div class="page-actions">
        <button class="btn btn-ghost btn-sm" :disabled="loading" @click="onDiscover">刷新检测</button>
        <button class="btn btn-ghost btn-sm" :disabled="busy" @click="onStopServer">停止服务器</button>
      </div>
    </div>

    <!-- 未设置服务器路径引导卡（Q8：不静默扫默认目录）-->
    <div v-if="serverPathMissing" class="sm-banner">
      <AppIcon name="info" :size="15" />
      <span>尚未设置服务器路径，请到【设置】填写 PalServer 根目录后再来本页操作。本页所有存档发现均基于该路径动态定位。</span>
    </div>

    <div v-if="autoDiscovered" class="sm-banner">
      <AppIcon name="info" :size="15" />
      <span>未在 server_path 直拼下找到存档，已自动向上扫描定位到：<code>{{ saveRoot }}</code>。请确认这正是你的帕鲁世界存档位置。</span>
    </div>
    <div v-else-if="saveRoot" class="sm-banner sm-banner--ok">
      <AppIcon name="info" :size="15" />
      <span>存档根目录：<code>{{ saveRoot }}</code></span>
    </div>
    <div class="sm-banner sm-banner--warn">
      <AppIcon name="info" :size="15" />
      <span><strong>改写类操作前会自动整目录备份（F4 机制）并在失败回滚，且需确保服务器已停止。</strong>UID/实例 ID 等底层术语对用户隐藏。</span>
    </div>
    <div v-if="pendingLocalSource" class="sm-banner sm-banner--info">
      <AppIcon name="info" :size="15" />
      <span>
        已接收到来自本地存档的迁移请求：<strong>{{ pendingLocalName }}</strong>（{{ pendingLocalSource }}）。
        请在上方「源世界」选择对应服务器世界后执行迁移（单机档需先整包迁移进服）。
      </span>
    </div>

    <!-- L1/L2 世界选择 -->
    <div class="sm-section">
      <div class="section-title">选择世界（L1 源 / L2 目标）</div>
      <div class="world-cols">
        <div class="world-col">
          <label class="char-label">源世界 / 迁移世界</label>
          <select v-model="sourceWorld" class="input" :disabled="!worlds.length">
            <option value="">— 请选择 —</option>
            <option v-for="w in worlds" :key="w.name" :value="w.name">{{ w.name }}（{{ w.player_count }} 角色）</option>
          </select>
        </div>
        <div class="world-col">
          <label class="char-label">目标世界（专用服）</label>
          <select v-model="targetWorld" class="input" :disabled="!worlds.length">
            <option value="">— 请选择 —</option>
            <option v-for="w in worlds" :key="w.name" :value="w.name">{{ w.name }}（{{ w.player_count }} 角色）</option>
          </select>
        </div>
      </div>
    </div>

    <!-- ② 整包世界迁移（P0，先行步骤） -->
    <div class="sm-section">
      <div class="section-title">② 整包世界迁移（文件级整目录拷贝，P0）</div>
      <div class="op-sub">把整个世界文件夹（Level.sav + Players/ + WorldOption.sav）原样复制到目标世界，不解析、不改动内部数据，安全且版本无关。</div>
      <div class="op-grid">
        <div class="op-field">
          <label class="char-label">源世界（要搬走的世界）</label>
          <select v-model="sourceWorld" class="input" :disabled="!worlds.length">
            <option value="">— 请选择 —</option>
            <option v-for="w in worlds" :key="w.name" :value="w.name">{{ w.name }}</option>
          </select>
        </div>
        <div class="op-field">
          <label class="char-label">目标世界（专用服）</label>
          <select v-model="targetWorld" class="input" :disabled="!worlds.length">
            <option value="">— 请选择 —</option>
            <option v-for="w in worlds" :key="w.name" :value="w.name">{{ w.name }}</option>
          </select>
        </div>
      </div>
      <label class="chk-row">
        <input type="checkbox" v-model="deleteWorldOption" />
        <span>拷贝后删除目标多余/过期的 WorldOption.sav（避免覆盖 PalWorldSettings）</span>
      </label>
      <div class="sm-toolbar">
        <button class="btn btn-primary btn-sm" :disabled="busy || !canMigrate" @click="onMigrate">
          {{ busy ? '处理中…' : '执行整包迁移' }}
        </button>
        <span class="sm-hint">迁移前自动整目录备份目标世界，失败自动回滚；需先停服。</span>
      </div>
    </div>

    <!-- ① 修复主机存档（P0 灵魂步骤，角色卡选择，不再手填 GUID） -->
    <div class="sm-section">
      <div class="section-title">① 修复主机存档（本地主机角色 ↔ 专用服新角色，P0 灵魂步骤）</div>
      <div class="op-sub">把本地单机的主机角色，重新映射为专用服能识别的新角色（GUID 互换）。先在专用服用原账号建好新角色并自动存档，再停服执行。</div>
      <div class="op-field" style="max-width: 340px; margin-bottom: 12px">
        <label class="char-label">操作世界（含旧主机角色与新角色，通常为迁移目标世界）</label>
        <select v-model="fixWorld" class="input" :disabled="!worlds.length">
          <option value="">— 请选择 —</option>
          <option v-for="w in worlds" :key="w.name" :value="w.name">{{ w.name }}</option>
        </select>
      </div>
      <div class="transfer-cols">
        <div class="transfer-col">
          <label class="char-label">旧主机角色</label>
          <PlayerPicker :world-name="fixWorld" v-model="oldHostGuids" />
        </div>
        <div class="transfer-arrow" aria-hidden="true">
          <span class="arrow-wide">➜</span>
          <span class="arrow-narrow">↓</span>
        </div>
        <div class="transfer-col">
          <label class="char-label">专用服新角色</label>
          <PlayerPicker :world-name="fixWorld" v-model="newCharGuids" />
        </div>
      </div>
      <div class="sm-toolbar">
        <button class="btn btn-primary btn-sm" :disabled="busy || !canFixHost" @click="onFixHost">
          {{ busy ? '处理中…' : '执行修复主机存档' }}
        </button>
        <span class="sm-hint">已选：旧主机 {{ oldHostGuids.length ? '1' : '0' }} 名 / 新角色 {{ newCharGuids.length ? '1' : '0' }} 名。</span>
      </div>
    </div>

    <!-- D. 科技点 + 玩家属性 -->
    <div class="sm-section">
      <div class="section-title">④ 数据修改：科技点 + 玩家属性（P1）</div>
      <div class="op-grid">
        <div class="op-field">
          <label class="char-label">世界</label>
          <select v-model="editWorld" class="input" :disabled="!worlds.length">
            <option value="">— 请选择 —</option>
            <option v-for="w in worlds" :key="w.name" :value="w.name">{{ w.name }}</option>
          </select>
        </div>
        <div class="op-field">
          <label class="char-label">玩家（单选）</label>
          <PlayerPicker :world-name="editWorld" v-model="editPlayers" />
        </div>
      </div>

      <div v-if="editPlayers.length" class="edit-block">
        <div class="op-sub">玩家基础属性</div>
        <div class="op-grid">
          <div class="op-field">
            <label class="char-label">改名（留空不改）</label>
            <input v-model="rename" class="input" placeholder="新昵称" :disabled="busy" />
          </div>
          <div class="op-field">
            <label class="char-label">等级（留空不改）</label>
            <input v-model="levelStr" class="input" placeholder="如 50" :disabled="busy" />
          </div>
        </div>
        <label class="chk-row">
          <input type="checkbox" v-model="maxAll" />
          <span>关键属性拉满（Max All）</span>
        </label>
        <div class="sm-toolbar">
          <button class="btn btn-ghost btn-sm" :disabled="busy || !canEditAttr" @click="onEditAttr">
            {{ busy ? '处理中…' : '应用属性' }}
          </button>
        </div>

        <div class="op-sub">科技点（解锁 / 移除，单项 + 批量）</div>
        <TechEditorPanel :world="editWorld" :player-guid="editPlayers[0]" />
      </div>
      <div v-else class="sm-empty sm-empty--sm">请选择世界与玩家以编辑科技点 / 属性。</div>
    </div>

    <ConfirmDialog
      v-model:visible="confirmVisible"
      :title="confirmTitle"
      :message="confirmMessage"
      :danger="true"
      @confirm="onConfirmOk"
    />
  </section>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useToast } from '@/components/ui/useToast'
import { useSettingsStore } from '@/stores/settings'
import { api } from '@/api/tauri'
import AppIcon from '@/components/ui/AppIcon.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import PlayerPicker from '@/components/save/PlayerPicker.vue'
import TechEditorPanel from '@/components/save/TechEditorPanel.vue'
import type { WorldInfo, EditResult } from '@/types/tauri'

const toast = useToast()
const settingsStore = useSettingsStore()
const route = useRoute()

// server_path 缺失时不静默扫默认目录，显式引导去设置（Q8）
const serverPathMissing = computed(() => !settingsStore.settings.server_path)

const loading = ref(false)
const busy = ref(false)
const worlds = ref<WorldInfo[]>([])
const saveRoot = ref('')
const autoDiscovered = ref(false)

// 来自「本地存档」页的迁移请求（route.query.source=路径 & type=local）
const pendingLocalSource = ref('')
const pendingLocalName = ref('')

const sourceWorld = ref('')
const targetWorld = ref('')
// 源类型：'server'（默认，sourceWorld 为世界名）| 'local'（来自本地存档页的本地绝对路径）
const sourceType = ref<'server' | 'local'>('server')

// A. 修复主机存档（角色卡选择，不再手填 GUID）
const fixWorld = ref('')
const oldHostGuids = ref<string[]>([])
const newCharGuids = ref<string[]>([])

// B. Migrate
const deleteWorldOption = ref(false)

// D. Tech / Attr
const editWorld = ref('')
const editPlayers = ref<string[]>([])
const rename = ref('')
const levelStr = ref('')
const maxAll = ref(false)

// Confirm dialog
const confirmVisible = ref(false)
const confirmTitle = ref('')
const confirmMessage = ref('')
let pendingFn: (() => Promise<EditResult>) | null = null

const canFixHost = computed(
  () =>
    !!sourceWorld.value &&
    !!targetWorld.value &&
    oldHostGuids.value.length === 1 &&
    newCharGuids.value.length === 1 &&
    oldHostGuids.value[0] !== newCharGuids.value[0],
)
const canMigrate = computed(() => {
  // 本地源：sourceType==='local' 时用 pendingLocalSource（绝对路径），仅需目标世界已选
  if (sourceType.value === 'local') {
    return !!pendingLocalSource.value && !!targetWorld.value
  }
  // 服务器源：仍需选源 + 目标且两者不同
  return (
    !!sourceWorld.value &&
    !!targetWorld.value &&
    sourceWorld.value !== targetWorld.value
  )
})
const canEditAttr = computed(() => !!editWorld.value && editPlayers.value.length > 0)

async function onDiscover(): Promise<void> {
  loading.value = true
  try {
    const res = await api.save.discoverWorlds()
    worlds.value = res.worlds
    saveRoot.value = res.save_root
    autoDiscovered.value = res.auto_discovered
    if (res.worlds.length && !res.worlds.some((w) => w.name === sourceWorld.value)) {
      sourceWorld.value = res.worlds[0].name
    }
    if (res.worlds.length && !res.worlds.some((w) => w.name === targetWorld.value)) {
      targetWorld.value = res.worlds[0].name
    }
    if (!fixWorld.value && targetWorld.value) {
      fixWorld.value = targetWorld.value
    }
    toast.info(`已发现 ${res.worlds.length} 个世界`)
  } catch (e) {
    toast.error(`检测世界失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    loading.value = false
  }
}

async function onStopServer(): Promise<void> {
  try {
    const st = await api.server.getStatus()
    if (st.running) {
      await api.server.stop()
      toast.info('已停止服务器')
    } else {
      toast.info('服务器本就未运行')
    }
  } catch (e) {
    toast.error(`停止服务器失败: ${e instanceof Error ? e.message : String(e)}`)
  }
}

async function ensureStopped(): Promise<void> {
  try {
    const st = await api.server.getStatus()
    if (st.running) {
      await api.server.stop()
    }
  } catch {
    // 忽略状态读取失败，交给后端做运行态断言
  }
}

async function runOp(label: string, fn: () => Promise<EditResult>): Promise<void> {
  busy.value = true
  try {
    await ensureStopped()
    const res = await fn()
    if (res.ok) {
      toast.success(
        `${label}成功（备份 ${res.backup_id || '—'}，round-trip ${res.roundtrip_ok ? '通过' : '有警告'}${
          res.warnings.length ? ' · ' + res.warnings.join('；') : ''
        }）`,
      )
    } else {
      toast.error(`${label}失败：${res.warnings.join('；')}`)
    }
  } catch (e) {
    toast.error(`${label}失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    busy.value = false
  }
}

function requestConfirm(title: string, message: string, fn: () => Promise<EditResult>): void {
  confirmTitle.value = title
  confirmMessage.value = message
  pendingFn = fn
  confirmVisible.value = true
}

async function onConfirmOk(): Promise<void> {
  if (pendingFn) {
    const fn = pendingFn
    pendingFn = null
    await runOp(confirmTitle.value, fn)
  }
}

function onFixHost(): void {
  if (!canFixHost.value) return
  const oldGuid = oldHostGuids.value[0]
  const newGuid = newCharGuids.value[0]
  requestConfirm(
    '修复主机存档',
    `即将把世界「${fixWorld.value}」内旧主机角色(${oldGuid}) 与 新角色(${newGuid}) 的 UID 互换。操作前自动备份，需确保服务器已停止。确认继续？`,
    () =>
      api.migration.fixHostSave({
        world: fixWorld.value,
        old_host_guid: oldGuid,
        new_char_guid: newGuid,
      }),
  )
}

function onMigrate(): void {
  if (!canMigrate.value) return
  const isLocal = sourceType.value === 'local'
  // 本地源用绝对路径（pendingLocalSource / route.query.source），服务器源用世界名下拉
  const srcVal = isLocal ? pendingLocalSource.value : sourceWorld.value
  const srcLabel = isLocal ? pendingLocalName.value || srcVal : sourceWorld.value
  requestConfirm(
    '整包世界迁移',
    `即将把世界「${srcLabel}」整体拷贝到「${targetWorld.value}」（含 Level.sav + Players/）。操作前自动备份目标世界。确认继续？`,
    () =>
      api.migration.migrateWorld({
        source_world: srcVal,
        target_world: targetWorld.value,
        delete_world_option: deleteWorldOption.value,
        source_type: isLocal ? 'local' : 'server',
      }),
  )
}

function onEditAttr(): void {
  if (!canEditAttr.value) return
  const guid = editPlayers.value[0]
  const level = levelStr.value.trim() === '' ? null : Number(levelStr.value)
  if (levelStr.value.trim() !== '' && (level === null || Number.isNaN(level) || level < 1)) {
    toast.error('等级需为正整数')
    return
  }
  void runOp('属性编辑', () =>
    api.migration.editPlayerAttr({
      world: editWorld.value,
      player_guid: guid,
      rename: rename.value.trim() === '' ? null : rename.value.trim(),
      level,
      max_all: maxAll.value,
    }),
  )
}

/** 接收来自「本地存档」页的迁移请求（route.query.source=路径 & type=local），预选源档 */
function applyPendingSource(): void {
  const src = (route.query.source as string) ?? ''
  const type = (route.query.type as string) ?? ''
  if (src && type === 'local') {
    pendingLocalSource.value = src
    sourceType.value = 'local' // 标记本地源类型，onMigrate 据此走本地绝对路径分支
    const name = src.split(/[\\/]/).filter(Boolean).pop() ?? ''
    pendingLocalName.value = name
    // 若服务器世界中存在同名世界，直接预选为源世界（便于仅目标选择场景）
    if (name && worlds.value.some((w) => w.name === name)) {
      sourceWorld.value = name
    }
  }
}

onMounted(() => {
  void onDiscover().then(applyPendingSource)
})
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
  margin-bottom: 8px;
}
.sm-banner--ok {
  background: var(--green-bg, rgba(79, 138, 107, 0.14));
  border-color: rgba(79, 138, 107, 0.3);
}
.sm-banner--warn {
  background: rgba(230, 111, 81, 0.1);
  border-color: rgba(230, 111, 81, 0.28);
}
.sm-banner--info {
  background: var(--primary-soft, rgba(230, 111, 81, 0.14));
  border-color: rgba(230, 111, 81, 0.32);
}
.sm-banner code {
  font-family: var(--font-mono);
  font-size: 12px;
  background: rgba(116, 88, 72, 0.1);
  padding: 1px 5px;
  border-radius: 5px;
}
.sm-section {
  margin-top: 14px;
  padding: 16px 18px;
  border-radius: var(--r-card);
  background: var(--glass-bg, rgba(255, 252, 247, 0.72));
  backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
}
.section-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
  margin-bottom: 12px;
}
.world-cols,
.op-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 12px;
  margin-bottom: 14px;
  align-items: start;
}
.op-field {
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.char-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-mid, #77675f);
  white-space: normal;
  line-height: 1.4;
}
.input {
  height: 34px;
  padding: 0 12px;
  border-radius: 9px;
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.2));
  background: rgba(255, 255, 255, 0.7);
  color: var(--palwarm-text-primary, #3f322c);
  font-size: 13px;
  outline: none;
}
.input:focus {
  border-color: var(--primary, #e66f51);
}
.chk-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text-mid, #77675f);
  margin: 4px 0 12px;
  cursor: pointer;
}
.op-sub {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-mid, #77675f);
  margin: 10px 0 8px;
}
.sm-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  margin-top: 10px;
}
.sm-hint {
  font-size: 12px;
  color: var(--text-mid2, #8a7a6e);
}
.edit-block {
  margin-top: 6px;
}
.sm-empty {
  padding: 16px;
  border-radius: 12px;
  background: var(--glass-bg-soft, rgba(255, 250, 244, 0.5));
  border: 1px solid var(--glass-border, rgba(116, 88, 72, 0.14));
  font-size: 13px;
  color: var(--text-mid2, #8a7a6e);
}
.sm-empty--sm {
  padding: 10px 14px;
  font-size: 12px;
}
/* ====== ① 修复主机存档：左旧主机角色 / 右新角色 / 中箭头 三段式 ====== */
.transfer-guide {
  font-size: 13px;
  color: var(--text-mid, #77675f);
  background: var(--primary-soft, rgba(230, 111, 81, 0.12));
  border: 1px solid rgba(230, 111, 81, 0.28);
  border-radius: 9px;
  padding: 8px 12px;
  margin-bottom: 12px;
  line-height: 1.5;
}
.transfer-cols {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  gap: 14px;
  align-items: start;
}
.transfer-col {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.transfer-col .input {
  width: 100%;
}
.transfer-arrow {
  align-self: center;
  display: flex;
  align-items: center;
  justify-content: center;
  padding-top: 26px;
  color: var(--primary, #e66f51);
  font-size: 28px;
}
.arrow-narrow {
  display: none;
}
@media (max-width: 720px) {
  .transfer-cols {
    grid-template-columns: 1fr;
  }
  .transfer-arrow {
    padding-top: 0;
    padding: 6px 0;
  }
  .arrow-wide {
    display: none;
  }
  .arrow-narrow {
    display: inline;
  }
}
.btn {
  border: none;
  border-radius: 9px;
  padding: 8px 16px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.btn-primary {
  background: var(--primary, #e66f51);
  color: #fff;
}
.btn-ghost {
  background: rgba(116, 88, 72, 0.08);
  color: var(--text-mid, #77675f);
}
.btn-sm {
  padding: 6px 12px;
  font-size: 12px;
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
