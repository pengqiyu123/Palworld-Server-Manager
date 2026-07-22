<template>
  <div class="readiness-card" :class="`lvl-${level}`">
    <div class="rc-head">
      <span class="rc-dot" :style="{ background: colorHex }" />
      <div class="rc-titles">
        <span class="rc-label">{{ label }}</span>
        <span class="rc-sub">{{ subText }}</span>
      </div>
      <AppIcon :name="iconName" :size="30" class="rc-icon" :style="{ color: colorHex }" />
    </div>

    <div v-if="hasIp" class="rc-ip">
      <span class="rc-ip-key">虚拟 IP</span>
      <span class="rc-ip-val">{{ readiness?.virtual_ip }}</span>
    </div>

    <div v-if="readiness?.reason" class="rc-reason">
      <AppIcon name="info" :size="14" />
      <span>{{ readiness.reason }}</span>
    </div>

    <div class="rc-actions">
      <button class="btn btn-ghost btn-sm" :disabled="checking" @click="emit('recheck')">
        {{ checking ? '检测中…' : '重新检测' }}
      </button>
      <button
        v-if="readiness?.next_action"
        class="btn btn-primary btn-sm"
        @click="onAction(readiness!.next_action!)"
      >
        {{ readiness.next_action.label }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import AppIcon from '@/components/ui/AppIcon.vue'
import {
  READINESS_LABEL,
  type RadminReadiness,
  type NextAction,
  type ReadinessLevel,
} from '@/types/tauri'

const props = defineProps<{
  readiness: RadminReadiness | null
  checking?: boolean
}>()

const emit = defineEmits<{
  (e: 'recheck'): void
  (e: 'invokeAction', action: NextAction): void
}>()

const checking = computed(() => props.checking ?? false)
const level = computed<ReadinessLevel>(() => props.readiness?.level ?? 'L0')
const colorHex = computed(() => COLOR_HEX[level.value])
const iconName = computed(() => (level.value === 'L4' ? 'vpn' : 'vpn'))

const hasIp = computed(() => !!props.readiness?.virtual_ip)

const subText = computed(() => {
  if (!props.readiness) return '尚未检测'
  switch (props.readiness.level) {
    case 'L0':
      return 'Radmin VPN 未安装'
    case 'L1':
      return '已安装但未启动'
    case 'L2':
      return '已启动但未加入虚拟网络'
    case 'L3':
      return '已入网，等待联机就绪'
    case 'L4':
      return '联机就绪，可发给朋友'
    default:
      return ''
  }
})

const label = computed(() => READINESS_LABEL[level.value])

function onAction(action: NextAction): void {
  emit('invokeAction', action)
}

// 各档语义色（与 READINESS_COLOR 对齐，scoped 样式里再细化边框）
const COLOR_HEX: Record<ReadinessLevel, string> = {
  L0: '#d9534f',
  L1: '#e08e3c',
  L2: '#e08e3c',
  L3: '#d8b53a',
  L4: '#4f8a6b',
}
</script>

<style scoped>
.readiness-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
  border-radius: 12px;
  background: var(--palwarm-surface, #faf6f0);
  border: 1px solid var(--palwarm-border, #e8ddd0);
  border-left-width: 4px;
}
.readiness-card.lvl-L0 {
  border-left-color: #d9534f;
}
.readiness-card.lvl-L1,
.readiness-card.lvl-L2 {
  border-left-color: #e08e3c;
}
.readiness-card.lvl-L3 {
  border-left-color: #d8b53a;
}
.readiness-card.lvl-L4 {
  border-left-color: #4f8a6b;
  background: rgba(79, 138, 107, 0.06);
}
.rc-head {
  display: flex;
  align-items: center;
  gap: 12px;
}
.rc-dot {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  flex-shrink: 0;
}
.rc-titles {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
}
.rc-label {
  font-size: 15px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
}
.rc-sub {
  font-size: 12px;
  color: var(--text-mid2, #a39383);
}
.rc-icon {
  flex-shrink: 0;
  opacity: 0.85;
}
.rc-ip {
  display: flex;
  align-items: center;
  gap: 10px;
}
.rc-ip-key {
  font-size: 12px;
  color: var(--text-mid2, #a39383);
}
.rc-ip-val {
  font-family: 'JetBrains Mono', monospace;
  font-size: 14px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
}
.rc-reason {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.03);
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-mid2, #a39383);
}
.rc-reason :deep(svg) {
  flex-shrink: 0;
  margin-top: 2px;
}
.rc-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}
.btn-sm {
  padding: 5px 14px;
  font-size: 12px;
}
</style>
