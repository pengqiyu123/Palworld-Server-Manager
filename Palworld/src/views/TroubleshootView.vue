<template>
  <div class="troubleshoot-view scroll-container">
    <GlassPanel radius="lg" class="troubleshoot-header">
      <div class="header-content">
        <div>
          <h2>故障排查</h2>
          <p class="header-subtitle">一键诊断常见问题，或选择问题分类查看解决方案</p>
        </div>
        <div class="header-actions">
          <BaseButton
            variant="primary"
            size="md"
            :loading="diagnosing"
            @click="handleDiagnose"
          >
            <Stethoscope :size="16" />
            <span style="margin-left: 6px;">一键诊断</span>
          </BaseButton>
          <BaseButton
            variant="secondary"
            size="md"
            :disabled="exporting || serverStore.logs.length === 0"
            :loading="exporting"
            @click="handleExportLogs"
          >
            <Download :size="16" />
            <span style="margin-left: 6px;">导出日志</span>
          </BaseButton>
        </div>
      </div>
    </GlassPanel>

    <!-- 诊断报告区（仅在一键诊断后显示） -->
    <GlassPanel
      v-if="diagnosticItems.length > 0"
      radius="lg"
      class="diagnostic-section"
    >
      <div class="section-header">
        <h2>诊断结果</h2>
        <span class="diagnostic-summary">
          共 {{ diagnosticItems.length }} 项 ·
          错误 {{ errorCount }} · 警告 {{ warnCount }} · 正常 {{ okCount }}
        </span>
      </div>
      <DiagnosticReport :items="diagnosticItems" />
    </GlassPanel>

    <div class="troubleshoot-grid">
      <GlassPanel
        v-for="issue in issues"
        :key="issue.id"
        radius="md"
        class="troubleshoot-card"
        :class="{ 'troubleshoot-card--expanded': expandedId === issue.id }"
      >
        <button class="troubleshoot-card__header" @click="toggleExpand(issue.id)">
          <div class="troubleshoot-card__title">
            <component :is="issue.icon" :size="18" />
            <span>{{ issue.title }}</span>
          </div>
          <ChevronDown
            :size="16"
            class="troubleshoot-card__chevron"
            :class="{ 'troubleshoot-card__chevron--open': expandedId === issue.id }"
          />
        </button>
        <Transition name="expand">
          <div v-if="expandedId === issue.id" class="troubleshoot-card__body">
            <div class="troubleshoot-card__description">{{ issue.description }}</div>
            <ol class="troubleshoot-card__steps">
              <li v-for="(step, idx) in issue.steps" :key="idx">{{ step }}</li>
            </ol>
          </div>
        </Transition>
      </GlassPanel>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  AlertTriangle,
  WifiOff,
  Gauge,
  Bug,
  Terminal,
  DatabaseBackup,
  ChevronDown,
  Download,
  Stethoscope,
} from '@lucide/vue'
import { save as dialogSave } from '@tauri-apps/plugin-dialog'
import { useServerStore } from '@/stores/server'
import { useSettingsStore } from '@/stores/settings'
import { useToast } from '@/components/ui/useToast'
import { api } from '@/api/tauri'
import GlassPanel from '@/components/ui/GlassPanel.vue'
import BaseButton from '@/components/ui/BaseButton.vue'
import DiagnosticReport, { type DiagnosticItem } from '@/components/ui/DiagnosticReport.vue'
import type {
  FirewallStatus,
  RadminLanStatus,
  ServerStatus,
} from '@/types/tauri'

interface TroubleshootIssue {
  id: string
  title: string
  icon: typeof AlertTriangle
  description: string
  steps: string[]
}

const serverStore = useServerStore()
const settingsStore = useSettingsStore()
const toast = useToast()

const expandedId = ref<string | null>('startup')
const diagnosing = ref(false)
const exporting = ref(false)
const diagnosticItems = ref<DiagnosticItem[]>([])

const errorCount = computed(() => diagnosticItems.value.filter(i => i.status === 'error').length)
const warnCount = computed(() => diagnosticItems.value.filter(i => i.status === 'warn').length)
const okCount = computed(() => diagnosticItems.value.filter(i => i.status === 'ok').length)

const issues: TroubleshootIssue[] = [
  {
    id: 'startup',
    title: '启动失败',
    icon: AlertTriangle,
    description: '服务器无法启动或启动后立即退出。',
    steps: [
      '检查 PalServer.exe 路径是否正确（在仪表盘选择正确的服务器目录）',
      '确认 Windows 防火墙未阻止 PalServer.exe（在网络页一键放行端口）',
      '查看服务器日志是否有错误信息（在仪表盘查看实时日志）',
      '尝试以管理员身份运行本管理器',
      '验证游戏文件完整性（Steam 库 → 右键 PalServer → 属性 → 已安装文件 → 验证）',
      '检查 8211 端口是否被其他程序占用（在网络页检测端口占用）',
    ],
  },
  {
    id: 'connection',
    title: '连接不上',
    icon: WifiOff,
    description: '玩家无法从游戏客户端连接到服务器。',
    steps: [
      '确认服务器已启动且正在运行（仪表盘显示"运行中"）',
      '检查防火墙是否放行 8211/UDP 和 27015/UDP 端口',
      '若使用 Radmin VPN，确认所有玩家都加入了同一个网络',
      '检查路由器端口转发设置（如跨公网连接）',
      '让玩家使用 "IP:端口" 格式手动添加服务器',
      '检查 Windows Defender 是否阻止了 PalServer.exe',
    ],
  },
  {
    id: 'lag',
    title: '卡顿',
    icon: Gauge,
    description: '服务器运行但游戏体验卡顿、延迟高。',
    steps: [
      '降低服务器参数中的 PalSpawnNumRate（减少帕鲁生成数量）',
      '检查服务器 CPU / 内存占用是否过高',
      '确认网络带宽是否充足（尤其是玩家较多时）',
      '调整 PalWorldSettings.ini 中的 MaxPlayers 参数',
      '在服务器启动参数中使用 -useperfthreads -NoAsyncLoadingThread -UseMultithreadForDS',
      '定期重启服务器释放内存',
    ],
  },
  {
    id: 'crash',
    title: '崩溃',
    icon: Bug,
    description: '服务器运行中突然崩溃退出。',
    steps: [
      '查看服务器日志最后几行（在仪表盘查看日志或导出）',
      '更新 PalServer 到最新版本（Steam 库 → 右键 → 属性 → 更新）',
      '检查服务器内存是否充足（推荐至少 16GB RAM）',
      '尝试移除可能导致崩溃的存档 mod',
      '减少同时在线玩家数量',
      '若反复崩溃，考虑备份存档后重置服务器',
    ],
  },
  {
    id: 'rcon',
    title: 'RCON 失败',
    icon: Terminal,
    description: '无法通过 RCON 远程管理服务器。',
    steps: [
      '确认 PalWorldSettings.ini 中 RCONEnabled=True',
      '检查 RCON 端口（默认 25575）是否在防火墙放行（TCP）',
      '确认 RCON 密码正确（区分大小写）',
      '在 RCON 页面尝试重新连接',
      '检查服务器日志是否显示 RCON 已启动',
      '若使用远程连接，确认路由器转发了 25575/TCP',
    ],
  },
  {
    id: 'save',
    title: '存档损坏',
    icon: DatabaseBackup,
    description: '玩家进度丢失或存档无法加载。',
    steps: [
      '停止服务器（重要：不要在运行时操作存档文件）',
      '导航到 {服务器目录}/Pal/Saved/SaveGames/0/<GUID>/ 备份整个目录',
      '查找 .sav.bak 备份文件，重命名为 .sav 尝试恢复',
      '若所有备份均损坏，可能需要重置存档（玩家进度会丢失）',
      '建议定期手动备份 SaveGames 目录',
      '确认磁盘空间充足，避免写入失败',
    ],
  },
]

function toggleExpand(id: string) {
  expandedId.value = expandedId.value === id ? null : id
}

// ============ 一键诊断 ============

// 并行执行 5 项检测，使用 allSettled 容忍单项失败
async function handleDiagnose() {
  if (diagnosing.value) return
  diagnosing.value = true
  diagnosticItems.value = []
  try {
    const [firewallResult, portResult, radminResult, serverStatusResult] = await Promise.allSettled([
      api.firewall.check(),
      api.network.checkPortUsage(8211),
      api.network.checkRadminLan(),
      api.server.getStatus(),
    ])

    const items: DiagnosticItem[] = []

    // 1. 防火墙规则检测
    if (firewallResult.status === 'fulfilled') {
      items.push(buildFirewallItem(firewallResult.value))
    } else {
      items.push({
        key: 'firewall',
        title: '防火墙端口',
        status: 'error',
        detail: `检测失败: ${String(firewallResult.reason)}`,
        suggestion: '请检查 Windows 防火墙服务是否运行，或以管理员身份运行本管理器。',
      })
    }

    // 2. 8211 端口占用检测
    if (portResult.status === 'fulfilled') {
      items.push(buildPortItem(portResult.value))
    } else {
      items.push({
        key: 'port',
        title: '游戏端口占用 (8211)',
        status: 'error',
        detail: `检测失败: ${String(portResult.reason)}`,
        suggestion: '请检查 netstat 命令是否可用，或以管理员身份运行。',
      })
    }

    // 3. Radmin LAN 状态检测
    if (radminResult.status === 'fulfilled') {
      items.push(buildRadminItem(radminResult.value))
    } else {
      items.push({
        key: 'radmin',
        title: 'Radmin LAN',
        status: 'warn',
        detail: `检测失败: ${String(radminResult.reason)}`,
        suggestion: 'Radmin VPN 为联机辅助工具，未安装不影响本地/LAN 联机。如需跨网联机请安装 Radmin VPN。',
      })
    }

    // 4 & 5. 服务器运行状态 + PalServer.exe 存在性（基于 server_path）
    const serverPath = settingsStore.settings.server_path
    if (serverStatusResult.status === 'fulfilled') {
      items.push(buildServerStatusItem(serverStatusResult.value))
    } else {
      items.push({
        key: 'server-status',
        title: '服务器运行状态',
        status: 'error',
        detail: `检测失败: ${String(serverStatusResult.reason)}`,
        suggestion: '无法获取服务器进程状态，请检查后端服务。',
      })
    }
    items.push(buildServerPathItem(serverPath, serverStatusResult))

    diagnosticItems.value = items
    const summary = `错误 ${items.filter(i => i.status === 'error').length} · 警告 ${items.filter(i => i.status === 'warn').length} · 正常 ${items.filter(i => i.status === 'ok').length}`
    toast.success(`诊断完成：${summary}`)
  } catch (err) {
    toast.error(`诊断失败: ${err instanceof Error ? err.message : String(err)}`)
  } finally {
    diagnosing.value = false
  }
}

function buildFirewallItem(status: FirewallStatus): DiagnosticItem {
  const closed: string[] = []
  if (!status.port_8211_open) closed.push('8211/UDP')
  if (!status.port_27015_open) closed.push('27015/UDP')
  if (!status.port_25575_open) closed.push('25575/TCP')
  if (closed.length === 0) {
    return {
      key: 'firewall',
      title: '防火墙端口',
      status: 'ok',
      detail: '8211/UDP、27015/UDP、25575/TCP 均已放行。',
    }
  }
  const isAllClosed = closed.length === 3
  return {
    key: 'firewall',
    title: '防火墙端口',
    status: isAllClosed ? 'error' : 'warn',
    detail: `以下端口未放行：${closed.join('、')}`,
    suggestion: '前往「网络」页面点击「一键放行」自动添加防火墙规则，或手动在 Windows 防火墙中允许这些端口。',
  }
}

function buildPortItem(usage: string | null): DiagnosticItem {
  if (usage === null) {
    return {
      key: 'port',
      title: '游戏端口占用 (8211)',
      status: 'ok',
      detail: '8211 端口空闲，可正常启动服务器。',
    }
  }
  // 端口被占用：若服务器正在运行，则是正常情况
  const isServerRunning = serverStore.status.running
  return {
    key: 'port',
    title: '游戏端口占用 (8211)',
    status: isServerRunning ? 'ok' : 'warn',
    detail: `8211 端口被占用：${usage}`,
    suggestion: isServerRunning
      ? '当前服务器正在运行，端口被本进程占用属正常现象。'
      : '端口被其他程序占用，请关闭占用进程或在配置中修改 PublicPort。',
  }
}

function buildRadminItem(status: RadminLanStatus): DiagnosticItem {
  if (!status.installed) {
    return {
      key: 'radmin',
      title: 'Radmin LAN',
      status: 'warn',
      detail: '未检测到 Radmin VPN 适配器。',
      suggestion: 'Radmin VPN 为可选的跨网联机工具。如需异地联机，请前往 https://www.radmin-vpn.com/ 下载安装。',
    }
  }
  if (status.adapter_status !== 'Up' && status.adapter_status !== 'up') {
    return {
      key: 'radmin',
      title: 'Radmin LAN',
      status: 'warn',
      detail: `Radmin 适配器状态：${status.adapter_status || '未知'}，虚拟 IP：${status.virtual_ip || '未分配'}`,
      suggestion: 'Radmin 适配器未启用，请在「网络」页面点击「重启 Radmin 服务」或在 Windows 服务管理中重启 RadminVPN 服务。',
    }
  }
  return {
    key: 'radmin',
    title: 'Radmin LAN',
    status: 'ok',
    detail: `已安装，虚拟 IP：${status.virtual_ip || '（未分配）'}`,
  }
}

function buildServerStatusItem(status: ServerStatus): DiagnosticItem {
  if (status.running) {
    return {
      key: 'server-status',
      title: '服务器运行状态',
      status: 'ok',
      detail: `服务器运行中，PID: ${status.pid ?? '未知'}，日志数: ${status.log_count}`,
    }
  }
  return {
    key: 'server-status',
    title: '服务器运行状态',
    status: 'warn',
    detail: '服务器未运行。',
    suggestion: '前往「仪表盘」点击「启动服务器」。若启动失败，请查看「启动失败」分类的排查步骤。',
  }
}

// 检查 PalServer.exe 是否存在：通过 server_path 拼接路径判断
function buildServerPathItem(
  serverPath: string,
  serverStatusResult: PromiseSettledResult<ServerStatus>,
): DiagnosticItem {
  if (!serverPath) {
    return {
      key: 'server-path',
      title: '服务器程序路径',
      status: 'error',
      detail: '未配置服务器目录。',
      suggestion: '前往「仪表盘」点击「选择目录」选择 PalServer.exe 所在的文件夹（通常为 Steam/steamapps/common/PalServer）。',
    }
  }
  // 若 server 状态检测成功，server_path 已被后端校验过；这里基于已知 server_path 进一步判断
  if (serverStatusResult.status === 'fulfilled') {
    const running = serverStatusResult.value.running
    if (running) {
      return {
        key: 'server-path',
        title: '服务器程序路径',
        status: 'ok',
        detail: `服务器目录：${serverPath}（PalServer.exe 已启动）`,
      }
    }
    // 未运行时不能确认 exe 存在，但目录已配置
    return {
      key: 'server-path',
      title: '服务器程序路径',
      status: 'warn',
      detail: `服务器目录：${serverPath}（服务器未运行，无法确认 PalServer.exe 是否存在）`,
      suggestion: '尝试在仪表盘启动服务器。若提示「服务器程序不存在」，请重新选择正确的目录。',
    }
  }
  return {
    key: 'server-path',
    title: '服务器程序路径',
    status: 'warn',
    detail: `服务器目录：${serverPath}（无法确认 PalServer.exe 是否存在）`,
    suggestion: '尝试在仪表盘启动服务器，启动失败会提示路径错误。',
  }
}

// ============ 导出日志 ============

async function handleExportLogs() {
  if (exporting.value) return
  if (serverStore.logs.length === 0) {
    toast.warning('当前没有日志可导出')
    return
  }
  exporting.value = true
  try {
    // 默认文件名：palworld-logs-YYYYMMDD-HHmmss.txt
    const now = new Date()
    const ts = `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, '0')}${String(now.getDate()).padStart(2, '0')}-${String(now.getHours()).padStart(2, '0')}${String(now.getMinutes()).padStart(2, '0')}${String(now.getSeconds()).padStart(2, '0')}`
    const filePath = await dialogSave({
      defaultPath: `palworld-logs-${ts}.txt`,
      filters: [
        { name: '文本文件', extensions: ['txt'] },
        { name: '日志文件', extensions: ['log'] },
        { name: '所有文件', extensions: ['*'] },
      ],
    })
    if (!filePath) {
      // 用户取消保存
      toast.info('已取消导出')
      return
    }
    const count = await api.server.exportLogs(filePath)
    toast.success(`已导出 ${count} 条日志到：${filePath}`)
  } catch (err) {
    toast.error(`导出日志失败: ${err instanceof Error ? err.message : String(err)}`)
  } finally {
    exporting.value = false
  }
}
</script>

<style scoped>
.troubleshoot-view {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
  overflow-y: auto;
}

.troubleshoot-header {
  padding: 20px 24px;
}

.header-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.header-actions {
  display: flex;
  gap: 8px;
}

.troubleshoot-header h2 {
  margin: 0 0 4px;
  font-size: 18px;
  font-weight: 600;
  color: var(--palwarm-text-primary);
}

.header-subtitle {
  margin: 0;
  font-size: 12px;
  color: var(--palwarm-text-muted);
}

/* ============ 诊断报告区 ============ */
.diagnostic-section {
  padding: 20px 24px;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--palwarm-glass-edge);
}

.section-header h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--palwarm-text-primary);
}

.diagnostic-summary {
  font-size: 12px;
  color: var(--palwarm-text-muted);
}

/* ============ 问题分类卡片 ============ */
.troubleshoot-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
}

.troubleshoot-card {
  padding: 0;
  overflow: hidden;
}

.troubleshoot-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 16px 20px;
  background: none;
  border: none;
  cursor: pointer;
  text-align: left;
  transition: background 0.15s;
}

.troubleshoot-card__header:hover {
  background: var(--palwarm-primary-soft);
}

.troubleshoot-card__title {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--palwarm-text-primary);
  font-size: 14px;
  font-weight: 500;
}

.troubleshoot-card__chevron {
  color: var(--palwarm-text-muted);
  transition: transform 0.2s;
}

.troubleshoot-card__chevron--open {
  transform: rotate(180deg);
}

.troubleshoot-card__body {
  padding: 0 20px 16px;
  border-top: 1px solid var(--palwarm-glass-edge);
}

.troubleshoot-card__description {
  padding: 12px 0;
  font-size: 13px;
  color: var(--palwarm-text-secondary);
  line-height: 1.5;
}

.troubleshoot-card__steps {
  margin: 0;
  padding-left: 20px;
  font-size: 13px;
  color: var(--palwarm-text-primary);
  line-height: 1.7;
}

.troubleshoot-card__steps li {
  margin-bottom: 4px;
}

.expand-enter-active, .expand-leave-active {
  transition: all 0.25s ease;
  overflow: hidden;
}

.expand-enter-from, .expand-leave-to {
  opacity: 0;
  max-height: 0;
  padding-top: 0;
  padding-bottom: 0;
}

.expand-enter-to, .expand-leave-from {
  opacity: 1;
  max-height: 600px;
}

@media (max-width: 1100px) {
  .troubleshoot-grid {
    grid-template-columns: 1fr;
  }
}
</style>
