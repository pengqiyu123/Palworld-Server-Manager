<template>
  <div class="port-card">
    <div class="p-head">
      <div class="p-left">
        <span class="p-icon" :style="{ background: iconBg }">
          <AppIcon :name="portIcon" :size="22" />
        </span>
        <span class="p-title">{{ title }}</span>
      </div>
      <StatusPill :status="status" :text="pillText" />
    </div>
    <div class="p-body">
      <div class="p-info">
        <span class="p-proto">{{ proto }}</span>
        <span class="p-desc">{{ desc }}</span>
      </div>
      <!-- 未放行时显示一键放行按钮 -->
      <button
        v-if="status === 'block' && allowAction"
        class="p-allow-btn"
        :disabled="allowing"
        @click="$emit('allow')"
      >
        {{ allowing ? '放行中…' : '一键放行' }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import AppIcon from './AppIcon.vue'
import StatusPill from './StatusPill.vue'

/** 端口状态卡：标题 / 协议方向 / 描述 / 状态 / 图标名 */
const props = withDefaults(
  defineProps<{
    title: string
    proto: string
    desc: string
    status: 'ok' | 'off' | 'block'
    iconName: 'game' | 'rcon' | 'rest'
    /** 是否显示一键放行按钮（默认 true） */
    allowAction?: boolean
    /** 放行操作进行中 */
    allowing?: boolean
  }>(),
  {
    allowAction: true,
    allowing: false,
  }
)

defineEmits<{ allow: [] }>()

// 图标名 → AppIcon 名称 + 图标底色（对齐原型内联样式）
const ICON_MAP: Record<string, { name: string; bg: string }> = {
  game: { name: 'port-game', bg: 'rgba(79,138,107,0.16)' },
  rcon: { name: 'port-rcon', bg: 'rgba(230,111,81,0.16)' },
  rest: { name: 'port-rest', bg: 'rgba(155,106,158,0.16)' },
}
const portIcon = computed(() => ICON_MAP[props.iconName].name)
const iconBg = computed(() => ICON_MAP[props.iconName].bg)

// 状态 → 药丸文案
const PILL_TEXT: Record<string, string> = { ok: '已通', off: '未启用', block: '被阻' }
const pillText = computed(() => PILL_TEXT[props.status])
</script>

<style scoped>
.p-body {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
}
.p-allow-btn {
  padding: 4px 12px;
  font-size: 12px;
  font-weight: 600;
  border-radius: 6px;
  border: 1px solid var(--palwarm-accent, #e66f51);
  background: rgba(230, 111, 81, 0.08);
  color: var(--palwarm-accent, #e66f51);
  cursor: pointer;
  transition: all 0.15s;
}
.p-allow-btn:hover:not(:disabled) {
  background: rgba(230, 111, 81, 0.16);
}
.p-allow-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
