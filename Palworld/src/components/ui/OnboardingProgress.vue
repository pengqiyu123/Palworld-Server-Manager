<template>
  <div class="onboarding-progress">
    <div v-for="(meta, idx) in stepMeta" :key="meta.id" class="op-step" :class="statusOf(meta.id)">
      <div class="op-node">
        <span class="op-index" v-if="statusOf(meta.id) !== 'pass'">{{ idx + 1 }}</span>
        <AppIcon v-else name="info" :size="14" class="op-check" />
      </div>
      <div class="op-body">
        <span class="op-title">{{ meta.title }}</span>
        <span class="op-desc">{{ meta.desc }}</span>
        <span v-if="statusOf(meta.id) === 'fail' && reasonOf(meta.id)" class="op-fail">
          {{ reasonOf(meta.id) }}
        </span>
      </div>
      <div v-if="idx < stepMeta.length - 1" class="op-line" :class="{ done: statusOf(meta.id) === 'pass' }" />
    </div>
  </div>
</template>

<script setup lang="ts">
import AppIcon from '@/components/ui/AppIcon.vue'
import type { StepId, OnboardingStepState } from '@/types/tauri'

const props = defineProps<{
  steps: Record<StepId, OnboardingStepState>
}>()

interface StepMeta {
  id: StepId
  title: string
  desc: string
}

const stepMeta: StepMeta[] = [
  { id: 's1', title: '检测服务器路径', desc: '定位 PalServer.exe' },
  { id: 's2', title: '配置已就绪', desc: 'REST / 管理密码' },
  { id: 's3', title: '启动服务器', desc: '进程运行 + REST 就绪' },
  { id: 's4', title: '放行防火墙', desc: '8211 / 25575 / 8212' },
  { id: 's5', title: 'Radmin 就绪', desc: '5 档检测 L4' },
  { id: 's6', title: '生成连法卡', desc: '复制给朋友' },
  { id: 's7', title: '朋友进服', desc: 'players ≥ 2' },
]

function statusOf(id: StepId): 'idle' | 'pass' | 'fail' {
  return props.steps[id]?.status ?? 'idle'
}

function reasonOf(id: StepId): string | undefined {
  return props.steps[id]?.reason
}
</script>

<style scoped>
.onboarding-progress {
  display: flex;
  align-items: flex-start;
  gap: 0;
  overflow-x: auto;
  padding: 4px 0 8px;
}
.op-step {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  flex: 1 0 130px;
  min-width: 130px;
  position: relative;
}
.op-node {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.06);
  color: var(--text-mid2, #a39383);
  font-size: 12px;
  font-weight: 700;
  flex-shrink: 0;
  border: 1px solid transparent;
}
.op-step.pass .op-node {
  background: #4f8a6b;
  color: #fff;
}
.op-step.fail .op-node {
  background: #d9534f;
  color: #fff;
}
.op-check {
  color: #fff;
}
.op-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-top: 2px;
}
.op-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
}
.op-step.fail .op-title {
  color: #d9534f;
}
.op-desc {
  font-size: 11px;
  color: var(--text-mid2, #a39383);
}
.op-fail {
  font-size: 11px;
  color: #d9534f;
  line-height: 1.4;
  margin-top: 2px;
}
.op-line {
  position: absolute;
  top: 12px;
  left: calc(100% - 6px);
  width: 12px;
  height: 2px;
  background: rgba(0, 0, 0, 0.1);
}
.op-line.done {
  background: #4f8a6b;
}
</style>
