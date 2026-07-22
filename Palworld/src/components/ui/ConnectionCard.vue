<template>
  <div class="conn-card">
    <div class="cc-head">
      <AppIcon name="vpn" :size="22" class="cc-icon" />
      <span class="cc-title">给朋友的连法卡片</span>
    </div>

    <div class="cc-ip-box">
      <div class="cc-ip-line">
        <span class="cc-ip-key">服务器地址</span>
        <span class="cc-ip-val">{{ displayIp }}:8211</span>
      </div>
      <div v-if="networkName" class="cc-net-line">
        <span class="cc-ip-key">Radmin 网络</span>
        <span class="cc-net-val">{{ networkName }}</span>
      </div>
    </div>

    <p class="cc-tip">
      把下面这段发朋友：他装好 Radmin VPN → 加入你的虚拟网络 → 游戏内填
      <b>{{ displayIp }}:8211</b> 直连。注意双方游戏版本需一致。
    </p>

    <button class="btn btn-primary cc-copy" @click="onCopy">
      <AppIcon name="info" :size="14" />
      <span>一键复制连法</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import AppIcon from '@/components/ui/AppIcon.vue'

const props = defineProps<{
  virtual_ip: string
  networkName?: string
}>()

const emit = defineEmits<{
  (e: 'copy', text: string): void
}>()

const displayIp = computed(() => props.virtual_ip || '你的虚拟IP')

function onCopy(): void {
  const text = `朋友连我帕鲁服：①装 Radmin VPN（radmin-vpn.com）②我拉你进我的虚拟网络 ③进游戏→多人→专用服务器→填 ${displayIp.value}:8211 直连（双方游戏版本需一致）`
  emit('copy', text)
}
</script>

<style scoped>
.conn-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
  border-radius: 12px;
  background: linear-gradient(135deg, #fdf3ec 0%, #faf6f0 100%);
  border: 1px solid var(--palwarm-border, #e8ddd0);
}
.cc-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.cc-icon {
  color: var(--palwarm-accent, #e66f51);
}
.cc-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--palwarm-text-primary, #3f322c);
}
.cc-ip-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 14px;
  border-radius: 10px;
  background: #fff;
  border: 1px dashed var(--palwarm-accent, #e66f51);
}
.cc-ip-line,
.cc-net-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.cc-ip-key {
  font-size: 12px;
  color: var(--text-mid2, #a39383);
}
.cc-ip-val {
  font-family: 'JetBrains Mono', monospace;
  font-size: 18px;
  font-weight: 700;
  color: var(--palwarm-accent, #e66f51);
}
.cc-net-val {
  font-size: 13px;
  font-weight: 600;
  color: var(--palwarm-text-primary, #3f322c);
}
.cc-tip {
  margin: 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-mid2, #a39383);
}
.cc-tip b {
  color: var(--palwarm-accent, #e66f51);
}
.cc-copy {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}
</style>
