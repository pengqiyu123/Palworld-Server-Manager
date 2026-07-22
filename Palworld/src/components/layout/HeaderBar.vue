<template>
  <header class="header-bar glass-panel">
    <div class="header-bar__title">
      <h1>{{ pageTitle }}</h1>
    </div>
    <div class="header-bar__actions">
      <slot />
    </div>
  </header>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()

const pageTitle = computed(() => (route.meta.title as string) || 'Palworld Server Manager')
</script>

<style scoped>
.header-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 24px;
  background: var(--palwarm-glass-soft);
  border-bottom: 1px solid var(--palwarm-glass-edge);
  border-radius: 0;
  min-height: 56px;
}

.header-bar__title h1 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--palwarm-text-primary);
}

.header-bar__actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
</style>
