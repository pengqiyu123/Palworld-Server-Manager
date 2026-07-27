<template>
  <div class="cfg-item">
    <div class="row">
      <div class="label">
        <span class="name">{{ name }}</span>
        <InfoTip v-if="tipText" :text="tipText" />
      </div>

      <!-- 数字：点击进入内联输入，回车/失焦提交，校验 min/max，带 'x' 后缀 -->
      <template v-if="editable === 'number'">
        <div v-if="!editing" class="cfg-value" @click="startEditNumber">{{ displayValue }}</div>
        <input
          v-else
          ref="numInputRef"
          class="cfg-value"
          v-model="editBuffer"
          @keyup.enter="commitNumber"
          @keyup.esc="editing = false"
          @blur="commitNumber"
        />
      </template>

      <!-- 文本：点击进入内联输入，回车/失焦提交 -->
      <template v-else-if="editable === 'text'">
        <div v-if="!editing" class="cfg-value" @click="startEditText">{{ displayValue }}</div>
        <input
          v-else
          ref="textInputRef"
          class="cfg-value"
          v-model="editBuffer"
          @keyup.enter="commitText"
          @keyup.esc="editing = false"
          @blur="commitText"
        />
      </template>

      <!-- 下拉：点击循环切换选项 -->
      <div v-else-if="editable === 'select'" class="cfg-value select" @click="cycleSelect">
        {{ displayValue }}
      </div>

      <!-- 开关 -->
      <div v-else class="toggle" :class="{ on: !!modelValue }" @click="toggle">
        <span class="knob" />
      </div>
    </div>

    <div class="cfg-def">{{ defaultText }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick } from 'vue'
import InfoTip from './InfoTip.vue'

/**
 * 配置项：name 名称 / editable 编辑类型 / modelValue 当前值
 * 数字内联编辑、下拉循环、开关切换，均 emit update:modelValue（本地 state，不落盘）
 */
const props = defineProps<{
  name: string
  editable: 'number' | 'select' | 'toggle' | 'text'
  modelValue: string | boolean
  min?: number
  max?: number
  step?: number
  options?: string[]
  tipText?: string
  defaultText?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string | boolean]
}>()

const editing = ref(false)
const editBuffer = ref('')
const numInputRef = ref<HTMLInputElement | null>(null)
const textInputRef = ref<HTMLInputElement | null>(null)

// 显示值：数字带 'x' 后缀（当 step 为小数倍率时）；文本去引号显示
const displayValue = computed(() => {
  if (props.editable === 'number') {
    const suffix = props.step !== undefined && String(props.step).includes('.') ? 'x' : ''
    return `${props.modelValue}${suffix}`
  }
  if (props.editable === 'text') {
    return stripQuotes(String(props.modelValue))
  }
  return String(props.modelValue)
})

// 去除配置值两端的引号（ini 文件中字符串值带引号）
function stripQuotes(s: string): string {
  if (s.length >= 2 && s.startsWith('"') && s.endsWith('"')) {
    return s.slice(1, -1)
  }
  return s
}

// 给字符串值加引号（写回 ini 时需要）
function wrapQuotes(s: string): string {
  if (s.startsWith('"') && s.endsWith('"')) return s
  return `"${s}"`
}

function startEditNumber(): void {
  editing.value = true
  editBuffer.value = String(props.modelValue)
  nextTick(() => {
    numInputRef.value?.focus()
    numInputRef.value?.select()
  })
}

function commitNumber(): void {
  if (!editing.value) return
  let n = parseFloat(editBuffer.value)
  if (isNaN(n)) n = parseFloat(String(props.modelValue)) || 0
  if (props.min !== undefined) n = Math.max(props.min, n)
  if (props.max !== undefined) n = Math.min(props.max, n)
  editing.value = false
  emit('update:modelValue', String(n))
}

function cycleSelect(): void {
  if (!props.options || props.options.length === 0) return
  const cur = props.options.indexOf(String(props.modelValue))
  const next = props.options[(cur + 1) % props.options.length]
  emit('update:modelValue', next)
}

function toggle(): void {
  emit('update:modelValue', !props.modelValue)
}

// 文本编辑：进入编辑、提交（加引号写回）
function startEditText(): void {
  editing.value = true
  editBuffer.value = stripQuotes(String(props.modelValue))
  nextTick(() => {
    textInputRef.value?.focus()
    textInputRef.value?.select()
  })
}

function commitText(): void {
  if (!editing.value) return
  editing.value = false
  emit('update:modelValue', wrapQuotes(editBuffer.value))
}
</script>
