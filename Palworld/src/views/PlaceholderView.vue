<template>
  <section class="screen active">
    <div class="placeholder">
      <AppIcon :name="iconName" :size="56" class="ph-icon" />
      <h3>{{ title }}</h3>
      <p>{{ desc }}</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import AppIcon from '@/components/ui/AppIcon.vue'

/** 通用占位屏：logs / backup / settings 三路由复用，统一设计语言 */
defineProps<{
  title: string
  desc: string
  iconName: string
}>()
</script>
