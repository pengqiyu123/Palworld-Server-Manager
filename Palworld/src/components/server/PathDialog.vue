<template>
  <div class="path-dialog">
    <div class="path-dialog__input">
      <input
        type="text"
        class="form-input"
        :value="modelValue"
        placeholder="请选择 PalServer 服务器目录"
        readonly
      />
      <BaseButton variant="primary" size="md" :loading="loading" @click="handleSelect">
        <FolderOpen :size="16" />
        <span style="margin-left: 6px;">浏览</span>
      </BaseButton>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { FolderOpen } from 'lucide-vue-next'
import BaseButton from '@/components/ui/BaseButton.vue'

withDefaults(defineProps<{
  modelValue?: string
}>(), {
  modelValue: '',
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
  select: [value: string]
}>()

const loading = ref(false)

async function handleSelect() {
  loading.value = true
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择 PalServer 服务器目录',
    })
    if (!selected || typeof selected !== 'string') {
      return
    }
    emit('update:modelValue', selected)
    emit('select', selected)
  } catch (err) {
    console.error('选择目录失败:', err)
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.path-dialog {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.path-dialog__input {
  display: flex;
  gap: 8px;
  align-items: stretch;
}

.path-dialog__input .form-input {
  flex: 1;
}
</style>
