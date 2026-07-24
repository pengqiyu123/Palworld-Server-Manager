<template>
  <section class="screen active">
    <div class="page-head">
      <div>
        <div class="page-title">服务器配置 · PalWorldSettings.ini</div>
        <div class="page-sub">
          修改后点保存写入配置文件。运行中改配置需重启服才生效。
        </div>
      </div>
      <div class="page-actions">
        <button class="btn btn-ghost" :disabled="saving" @click="onFillDefault">
          一键填充默认配置
        </button>
        <button class="btn btn-primary" :disabled="configStore.dirtyCount === 0 || saving" @click="onSave">
          {{ saving ? '保存中…' : '保存配置' }}
        </button>
        <button class="btn btn-ghost" :disabled="configStore.dirtyCount === 0" @click="onCancel">
          撤销修改
        </button>
      </div>
    </div>

    <!-- 运行中警告 -->
    <div v-if="serverStore.status.running" class="cfg-warning">
      <AppIcon name="info" :size="16" />
      <span>服务器正在运行，修改配置后需重启服务器才能生效。</span>
    </div>

    <div v-else-if="isFirstTime" class="cfg-warning">
      <AppIcon name="info" :size="16" />
      <span>首次配置：填写完成并保存后会返回概览页启动服务器。</span>
    </div>

    <!-- AdminPassword 专属区 -->
    <div class="admin-pw-section">
      <div class="apw-head">
        <span class="apw-title">管理员密码 (AdminPassword)</span>
        <span class="apw-tag">REST/RCON 认证用</span>
        <InfoTip :html="adminPasswordTip" />
      </div>
      <div class="apw-body">
        <div class="apw-input-row">
          <input
            :type="showPw ? 'text' : 'password'"
            class="apw-input"
            :value="adminPasswordDisplay"
            readonly
          />
          <button class="btn btn-ghost btn-sm" @click="showPw = !showPw">
            {{ showPw ? '隐藏' : '显示' }}
          </button>
          <button class="btn btn-ghost btn-sm" :disabled="!adminPasswordDisplay.trim()" @click="onCopyPw">复制</button>
          <button class="btn btn-primary btn-sm" @click="onEditPw">修改</button>
        </div>
        <div class="apw-hint">
          此密码用于 REST API / RCON 认证（不是游戏进服密码）。改密码需重启服才生效。复制后可粘到游戏聊天框做游戏内管理员认证。
        </div>
        <!-- 修改密码内联编辑 -->
        <div v-if="editingPw" class="apw-edit-row">
          <input
            ref="pwInputRef"
            v-model="pwBuffer"
            type="text"
            class="apw-input"
            placeholder="输入新管理员密码…"
            @keyup.enter="commitPw"
            @keyup.esc="editingPw = false"
          />
          <button class="btn btn-primary btn-sm" @click="commitPw">确认</button>
          <button class="btn btn-ghost btn-sm" @click="editingPw = false">取消</button>
        </div>
      </div>
    </div>

    <!-- 搜索栏 -->
    <div class="search-bar">
      <AppIcon name="search" :size="16" />
      <input
        v-model="search"
        type="text"
        placeholder="搜索配置项…"
      />
    </div>

    <!-- 配置分组 -->
    <CfgGroup
      v-for="g in groups"
      :key="g.id"
      :title="g.title"
      :icon-name="g.iconName"
      :count="g.items.length"
      :collapsed="g.collapsed"
      @toggle="g.collapsed = !g.collapsed"
    >
      <CfgItem
        v-for="it in g.items"
        :key="it.key"
        v-show="matchSearch(it.label)"
        :class="{ dirty: configStore.dirty.has(it.key) }"
        :name="it.label"
        :editable="it.editable"
        :model-value="getDisplayValue(it.key, it.editable)"
        :min="it.min"
        :max="it.max"
        :step="it.step"
        :options="it.options"
        :tip-html="it.tip"
        :default-text="it.defaultText"
        @update:model-value="(v) => onUpdate(it.key, v, it.editable)"
      />
    </CfgGroup>

    <!-- 确认弹窗 -->
    <ConfirmDialog
      v-model:visible="confirmVisible"
      :title="confirmTitle"
      :message="confirmMessage"
      :danger="confirmDanger"
      @confirm="onConfirmSave"
    />
  </section>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useConfigStore } from '@/stores/config'
import { useSettingsStore } from '@/stores/settings'
import { useServerStore } from '@/stores/server'
import { useToast } from '@/components/ui/useToast'
import { api } from '@/api/tauri'
import AppIcon from '@/components/ui/AppIcon.vue'
import CfgGroup from '@/components/ui/CfgGroup.vue'
import CfgItem from '@/components/ui/CfgItem.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import InfoTip from '@/components/ui/InfoTip.vue'

type Editable = 'number' | 'select' | 'toggle' | 'text'

interface CfgItemDef {
  key: string
  label: string
  editable: Editable
  min?: number
  max?: number
  step?: number
  options?: string[]
  tip: string
  defaultText: string
  visible: boolean
}

interface CfgGroupDef {
  id: string
  title: string
  iconName: string
  collapsed: boolean
  items: CfgItemDef[]
}

const configStore = useConfigStore()
const settingsStore = useSettingsStore()
const serverStore = useServerStore()
const toast = useToast()
const route = useRoute()
const router = useRouter()
const isFirstTime = computed(() => route.query.firstTime === 'true')

const saving = ref(false)
const search = ref('')
const showPw = ref(false)
const editingPw = ref(false)
const pwBuffer = ref('')
const pwInputRef = ref<HTMLInputElement | null>(null)

// 确认弹窗
const confirmVisible = ref(false)
const confirmTitle = ref('')
const confirmMessage = ref('')
const confirmDanger = ref(false)

// ====== AdminPassword 处理 ======
const adminPasswordDisplay = computed(() => {
  const raw = configStore.config['AdminPassword'] ?? ''
  // 去引号
  if (raw.length >= 2 && raw.startsWith('"') && raw.endsWith('"')) {
    return raw.slice(1, -1)
  }
  return raw
})

function onCopyPw(): void {
  const raw = adminPasswordDisplay.value.trim()
  if (!raw) {
    toast.warning('管理员密码为空')
    return
  }
  // 去引号后拼接斜杠指令（写纯密码改为 /AdminPassword <密码>）
  const pw = raw.replace(/^"|"$/g, '')
  const cmd = `/AdminPassword ${pw}`
  try {
    navigator.clipboard.writeText(cmd)
    toast.success('管理员指令已复制，粘贴到游戏聊天框回车即可获管理员权限')
  } catch {
    const textarea = document.createElement('textarea')
    textarea.value = cmd
    document.body.appendChild(textarea)
    textarea.select()
    try {
      document.execCommand('copy')
      toast.success('管理员指令已复制，粘贴到游戏聊天框回车即可获管理员权限')
    } catch {
      toast.error('复制失败')
    }
    document.body.removeChild(textarea)
  }
}

function onEditPw(): void {
  pwBuffer.value = adminPasswordDisplay.value
  editingPw.value = true
  nextTick(() => pwInputRef.value?.focus())
}

// ====== R4 · 配置项 ⓘ 简介（来自后端 get_config_descriptions）======
const descriptions = ref<Map<string, string>>(new Map())

// description 缺失时的兜底文案（代码未列的选项用 web 标准文档补充，写前端常量）
const FALLBACK_DESC: Record<string, string> = {
  ServerName: '服务器在列表中显示的名称。',
  ServerDescription: '服务器描述信息。',
  ServerPassword: '玩家进服密码（局域网可留空）。',
  ServerPlayerMaxNum: '服务器同时最多容纳的玩家数（上限 32）。',
  AdminPassword: '用于 REST API / RCON 认证（不是游戏进服密码）。',
}

const adminPasswordTip = computed(() => {
  const base = descriptions.value.get('AdminPassword') || FALLBACK_DESC['AdminPassword']
  return `${base}<br><br>复制按钮生成 <code>/AdminPassword &lt;密码&gt;</code> 可直接在游戏聊天框回车认证。`
})

// 把后端 descriptions 接到每个配置项的 tip（CfgItem 已用 InfoTip 渲染 tip-html）
function applyDescriptions(): void {
  for (const g of groups) {
    for (const it of g.items) {
      const desc = descriptions.value.get(it.key)
      it.tip = desc && desc.trim() ? desc : (it.tip || FALLBACK_DESC[it.key] || '')
    }
  }
}

function commitPw(): void {
  const newVal = `"${pwBuffer.value.trim()}"`
  configStore.update('AdminPassword', newVal)
  editingPw.value = false
  toast.info('密码已修改，点「保存配置」生效（需重启服）')
}

// ====== 配置分组定义 ======
const groups = reactive<CfgGroupDef[]>([
  {
    id: 'basic',
    title: '基础设置',
    iconName: 'group-basic',
    collapsed: false,
    items: [
      { key: 'ServerName', label: 'ServerName 服务器名称', editable: 'text', tip: '<b>用途</b>：服务器列表中显示的名称。', defaultText: '默认 Default Palworld Server', visible: true },
      { key: 'ServerDescription', label: 'ServerDescription 服务器描述', editable: 'text', tip: '<b>用途</b>：服务器描述信息。', defaultText: '默认空', visible: true },
      { key: 'ServerPassword', label: 'ServerPassword 进服密码', editable: 'text', tip: '<b>用途</b>：玩家进服密码（Radmin 局域网可留空）。', defaultText: '默认空', visible: true },
      { key: 'ServerPlayerMaxNum', label: 'ServerPlayerMaxNum 最大人数', editable: 'number', min: 1, max: 32, step: 1, tip: '<b>用途</b>：服务器同时最多容纳的玩家数。', defaultText: '默认 32 · 范围 1-32', visible: true },
    ],
  },
  {
    id: 'gameplay',
    title: '玩法规则',
    iconName: 'group-rules',
    collapsed: true,
    items: [
      { key: 'ExpRate', label: 'ExpRate 经验倍率', editable: 'number', step: 0.1, tip: '<b>用途</b>：玩家获得经验的倍率。', defaultText: '默认 1.0', visible: true },
      { key: 'PalCaptureRate', label: 'PalCaptureRate 捕获倍率', editable: 'number', step: 0.1, tip: '<b>用途</b>：帕鲁捕获率倍率。', defaultText: '默认 1.0', visible: true },
      { key: 'PalSpawnNumRate', label: 'PalSpawnNumRate 帕鲁出现率', editable: 'number', step: 0.1, tip: '<b>用途</b>：帕鲁出现率（过高影响性能）。', defaultText: '默认 1.0', visible: true },
      { key: 'DeathPenalty', label: 'DeathPenalty 死亡惩罚', editable: 'select', options: ['None', 'Item', 'ItemAndEquipment', 'All'], tip: '<b>用途</b>：玩家死亡时损失的物品/属性。', defaultText: '默认 Item', visible: true },
      { key: 'bIsPvP', label: 'bIsPvP 允许 PVP', editable: 'toggle', tip: '<b>用途</b>：是否允许玩家互相攻击。', defaultText: '默认关闭', visible: true },
      { key: 'bHardcore', label: 'bHardcore 硬核模式', editable: 'toggle', tip: '<b>用途</b>：死亡不可复活。', defaultText: '默认关闭', visible: true },
      { key: 'bEnableInvaderEnemy', label: 'bEnableInvaderEnemy 入侵敌人', editable: 'toggle', tip: '<b>用途</b>：启用袭击事件。', defaultText: '默认开启', visible: true },
      { key: 'PalEggDefaultHatchingTime', label: 'PalEggDefaultHatchingTime 孵化时间', editable: 'number', step: 0.1, tip: '<b>用途</b>：帕鲁蛋孵化时间（小时，0=即时）。', defaultText: '默认 1.0', visible: true },
      { key: 'WorkSpeedRate', label: 'WorkSpeedRate 工作速度', editable: 'number', step: 0.1, tip: '<b>用途</b>：工作速度倍率。', defaultText: '默认 1.0', visible: true },
    ],
  },
  {
    id: 'combat',
    title: '战斗与生存',
    iconName: 'group-rules',
    collapsed: true,
    items: [
      { key: 'PalDamageRateAttack', label: 'PalDamageRateAttack 帕鲁攻击', editable: 'number', step: 0.1, tip: '<b>用途</b>：帕鲁攻击伤害倍率。', defaultText: '默认 1.0', visible: true },
      { key: 'PalDamageRateDefense', label: 'PalDamageRateDefense 帕鲁防御', editable: 'number', step: 0.1, tip: '<b>用途</b>：对帕鲁的防御伤害倍率。', defaultText: '默认 1.0', visible: true },
      { key: 'PlayerDamageRateAttack', label: 'PlayerDamageRateAttack 玩家攻击', editable: 'number', step: 0.1, tip: '<b>用途</b>：玩家攻击伤害倍率。', defaultText: '默认 1.0', visible: true },
      { key: 'PlayerStomachDecreaceRate', label: 'PlayerStomachDecreaceRate 饥饿消耗', editable: 'number', step: 0.1, tip: '<b>用途</b>：玩家饥饿消耗率。', defaultText: '默认 1.0', visible: true },
      { key: 'PlayerStaminaDecreaceRate', label: 'PlayerStaminaDecreaceRate 耐力消耗', editable: 'number', step: 0.1, tip: '<b>用途</b>：玩家耐力消耗率。', defaultText: '默认 1.0', visible: true },
      { key: 'PlayerAutoHPRegeneRate', label: 'PlayerAutoHPRegeneRate HP恢复', editable: 'number', step: 0.1, tip: '<b>用途</b>：玩家自动 HP 恢复率。', defaultText: '默认 1.0', visible: true },
    ],
  },
  {
    id: 'server',
    title: '服务器与网络',
    iconName: 'group-perf',
    collapsed: true,
    items: [
      { key: 'PublicPort', label: 'PublicPort 游戏端口', editable: 'number', min: 1, max: 65535, step: 1, tip: '<b>用途</b>：玩家连接的 UDP 端口。', defaultText: '默认 8211', visible: true },
      { key: 'RCONEnabled', label: 'RCONEnabled 启用 RCON', editable: 'toggle', tip: '<b>用途</b>：启用 RCON 远程管理。', defaultText: '默认关闭', visible: true },
      { key: 'RCONPort', label: 'RCONPort RCON 端口', editable: 'number', min: 1, max: 65535, step: 1, tip: '<b>用途</b>：RCON 远程管理端口。', defaultText: '默认 25575', visible: true },
      { key: 'RESTAPIEnabled', label: 'RESTAPIEnabled 启用 REST API', editable: 'toggle', tip: '<b>用途</b>：启用 REST API（本工具需要）。', defaultText: '默认关闭', visible: true },
      { key: 'RESTAPIPort', label: 'RESTAPIPort REST 端口', editable: 'number', min: 1, max: 65535, step: 1, tip: '<b>用途</b>：REST API 端口。', defaultText: '默认 8212', visible: true },
      { key: 'AutoSaveSpan', label: 'AutoSaveSpan 自动存档间隔', editable: 'number', min: 10, max: 3600, step: 10, tip: '<b>用途</b>：自动保存间隔（秒）。', defaultText: '默认 30 秒', visible: true },
      { key: 'bIsShowJoinLeftMessage', label: 'bIsShowJoinLeftMessage 进出消息', editable: 'toggle', tip: '<b>用途</b>：显示加入/退出消息。', defaultText: '默认开启', visible: true },
    ],
  },
])

// 搜索过滤：v-show 逐条匹配
function matchSearch(label: string): boolean {
  const q = search.value.trim().toLowerCase()
  if (!q) return true
  return label.toLowerCase().includes(q)
}

// ====== 值转换：config store 的字符串 ↔ CfgItem 的类型 ======
function getDisplayValue(key: string, editable: Editable): string | boolean {
  const raw = configStore.config[key] ?? ''
  if (editable === 'toggle') {
    return raw === 'True'
  }
  return raw
}

function onUpdate(key: string, value: string | boolean, editable: Editable): void {
  let strValue: string
  if (editable === 'toggle') {
    strValue = value ? 'True' : 'False'
  } else {
    strValue = String(value)
  }
  configStore.update(key, strValue)
}

// ====== 保存 ======
async function onSave(): Promise<void> {
  // 运行中弹警告
  if (serverStore.status.running) {
    confirmTitle.value = '服务器运行中'
    confirmMessage.value = '服务器正在运行，保存的配置需要重启服务器才能生效。是否继续保存?'
    confirmDanger.value = false
    confirmVisible.value = true
    return
  }
  await doSave()
}

async function onConfirmSave(): Promise<void> {
  confirmVisible.value = false
  await doSave()
}

async function doSave(): Promise<void> {
  saving.value = true
  try {
    await configStore.save()
    toast.success('配置已保存')
    if (isFirstTime.value) {
      await router.push('/overview')
    }
  } catch (e) {
    toast.error(`保存失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    saving.value = false
  }
}

function onCancel(): void {
  configStore.cancelEdits()
  toast.info('已撤销所有修改')
}

// ====== 一键填充默认配置（仅手动触发，绝不接入 start_server 自动守卫）======
async function onFillDefault(): Promise<void> {
  const serverPath = settingsStore.settings.server_path
  if (!serverPath) {
    toast.warning('尚未设置服务器路径，请先到【设置】填写 PalServer 根目录')
    return
  }
  saving.value = true
  try {
    const res = await api.config.fillDefault(serverPath)
    toast.success(res.message)
    if (isFirstTime.value) {
      await router.push('/overview')
    }
  } catch (e) {
    toast.error(`填充默认配置失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    saving.value = false
  }
}

// ====== 加载配置 ======
onMounted(async () => {
  // R4：拉取配置项描述，构建 name→description 索引并接到每个配置项 ⓘ
  try {
    const descs = await api.config.getDescriptions()
    descriptions.value = new Map(descs.map((d) => [d.name, d.description]))
    applyDescriptions()
  } catch (e) {
    toast.error(`加载配置描述失败: ${e instanceof Error ? e.message : String(e)}`)
  }
  const configPath = settingsStore.settings.config_path ||
    settingsStore.computeConfigPath(settingsStore.settings.server_path)
  if (configPath) {
    try {
      await configStore.load(configPath)
    } catch (e) {
      toast.error(`加载配置失败: ${e instanceof Error ? e.message : String(e)}`)
    }
  }
})
</script>

<style scoped>
.cfg-warning {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-radius: 8px;
  background: rgba(230, 111, 81, 0.08);
  border: 1px solid rgba(230, 111, 81, 0.2);
  margin-bottom: 16px;
  font-size: 13px;
  color: var(--palwarm-text-primary, #3f322c);
}
.admin-pw-section {
  margin-bottom: 16px;
  padding: 16px 20px;
  border-radius: 12px;
  background: var(--palwarm-surface, #faf6f0);
  border: 1px solid var(--palwarm-border, #e8ddd0);
}
.apw-head {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}
.apw-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
}
.apw-tag {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 4px;
  background: rgba(155, 106, 158, 0.12);
  color: #9b6a9e;
}
.apw-input-row {
  display: flex;
  gap: 8px;
  align-items: center;
}
.apw-input {
  flex: 1;
  padding: 8px 12px;
  border-radius: 8px;
  border: 1px solid var(--palwarm-border, #e8ddd0);
  background: var(--palwarm-bg, #fff);
  color: var(--palwarm-text-primary, #3f322c);
  font-size: 14px;
  font-family: 'JetBrains Mono', monospace;
  outline: none;
}
.apw-input:focus {
  border-color: var(--palwarm-accent, #e66f51);
}
.apw-hint {
  margin-top: 8px;
  font-size: 12px;
  color: var(--text-mid2, #a39383);
  line-height: 1.5;
}
.apw-edit-row {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-top: 10px;
}
.btn-sm {
  padding: 4px 12px;
  font-size: 12px;
}
</style>
