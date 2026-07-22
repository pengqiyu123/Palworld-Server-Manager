<template>
  <Teleport to="body">
    <Transition name="rl">
      <div v-if="visible" class="rl-overlay" @click.self="close">
        <div class="rl-modal" role="dialog" aria-modal="true">
          <div class="rl-head">
            <AppIcon name="vpn" :size="28" class="rl-icon" />
            <h3 class="rl-title">Radmin VPN 已启动</h3>
          </div>

          <div class="rl-body">
            <p class="rl-para">
              如果你要<strong>加入别人的服</strong>：让开服者在他的 Radmin 里创建一个网络，把<strong>网络名 + 密码</strong>发给你，你打开 Radmin 加入该网络即可。
            </p>
            <p class="rl-para">
              如果你是<strong>自己开服</strong>：正常在 Radmin 创建一个网络，把网络名告诉朋友，让他加入你的网络。
            </p>
            <div class="rl-tip">
              <AppIcon name="info" :size="15" class="rl-tip-icon" />
              <span>
                加入同一网络后，游戏内「直接连接」填<strong>开服者的 Radmin 虚拟 IP:8211</strong>
                （你的虚拟 IP 通常是 26.x.x.x）。
              </span>
            </div>
          </div>

          <div class="rl-footer">
            <button class="btn btn-primary" @click="close">知道了</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import AppIcon from '@/components/ui/AppIcon.vue'

withDefaults(
  defineProps<{
    /** 是否显示弹窗 */
    visible: boolean
  }>(),
  { visible: false }
)

const emit = defineEmits<{
  'update:visible': [value: boolean]
}>()

function close(): void {
  emit('update:visible', false)
}
</script>

<style scoped>
.rl-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  /* 暖色磨砂玻璃遮罩（与 .terminal 同族 rgba 调性） */
  background: rgba(54, 42, 34, 0.3);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
}
.rl-modal {
  width: 460px;
  max-width: 100%;
  padding: 24px;
  border-radius: var(--palwarm-radius-md);
  background: var(--palwarm-popover);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--palwarm-border);
  box-shadow: var(--palwarm-static-shadow);
  text-align: left;
}
.rl-head {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}
.rl-icon {
  flex: 0 0 28px;
  color: var(--palwarm-primary);
}
.rl-title {
  margin: 0;
  font-size: 17px;
  font-weight: 700;
  color: var(--palwarm-foreground);
}
.rl-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.rl-para {
  margin: 0;
  font-size: 14px;
  line-height: 1.65;
  color: var(--palwarm-muted-foreground);
}
.rl-para strong {
  color: var(--palwarm-primary);
  font-weight: 700;
}
.rl-tip {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 12px 14px;
  border-radius: var(--palwarm-radius-sm);
  background: var(--primary-soft);
  font-size: 13px;
  line-height: 1.6;
  color: var(--palwarm-muted-foreground);
}
.rl-tip-icon {
  flex: 0 0 15px;
  margin-top: 2px;
  color: var(--palwarm-primary);
}
.rl-tip strong {
  color: var(--palwarm-primary);
  font-weight: 700;
}
.rl-footer {
  display: flex;
  justify-content: flex-end;
  margin-top: 20px;
}

/* 与 ConfirmDialog 一致的弹窗过渡 */
.rl-enter-active,
.rl-leave-active {
  transition: opacity 0.2s ease;
}
.rl-enter-active .rl-modal,
.rl-leave-active .rl-modal {
  transition: transform 0.2s ease;
}
.rl-enter-from,
.rl-leave-to {
  opacity: 0;
}
.rl-enter-from .rl-modal,
.rl-leave-to .rl-modal {
  transform: scale(0.96);
}
</style>
