<template>
  <div class="window">
    <TitleBar />
    <AppShell>
      <template #sidebar>
        <Sidebar />
      </template>
      <router-view v-slot="{ Component }">
        <Transition name="page" mode="out-in">
          <component :is="Component" />
        </Transition>
      </router-view>
    </AppShell>
    <Toast />
  </div>
</template>

<script setup lang="ts">
import { onUnmounted } from 'vue'
import AppShell from '@/components/layout/AppShell.vue'
import Sidebar from '@/components/layout/Sidebar.vue'
import TitleBar from '@/components/layout/TitleBar.vue'
import Toast from '@/components/ui/Toast.vue'
import { useServerStore } from '@/stores/server'

const serverStore = useServerStore()

// App 卸载时清理：停止轮询 + 销毁事件监听
onUnmounted(() => {
  serverStore.stopPolling()
  serverStore.destroyListener()
})
</script>

<style>
/* 页面过渡：路由切换淡入 */
.page-enter-active,
.page-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.page-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.page-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
