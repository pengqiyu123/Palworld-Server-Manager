<template>
  <div class="cfg-group" :class="{ collapsed }">
    <div class="cfg-group-head" @click="$emit('toggle')">
      <AppIcon :name="iconName" :size="18" class="g-icon" />
      <span class="g-title">{{ title }}</span>
      <span class="g-count">{{ countText }}</span>
      <AppIcon name="chevron" :size="16" class="chevron" />
    </div>
    <div class="cfg-group-body">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import AppIcon from './AppIcon.vue'

/** 可折叠配置分组：title 标题 / iconName 图标 / count 项数 / collapsed 折叠态 */
const props = withDefaults(
  defineProps<{
    title: string
    iconName: string
    count: number
    collapsed?: boolean
  }>(),
  { collapsed: false }
)

defineEmits<{ toggle: [] }>()

// 折叠态切换「已折叠 / 已展开」文案，对齐原型
const countText = computed(() =>
  props.collapsed ? `${props.count} 项 · 已折叠` : `${props.count} 项 · 已展开`
)
</script>
