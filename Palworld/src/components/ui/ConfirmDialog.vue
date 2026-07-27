<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="visible" class="modal-overlay" @click.self="handleCancel" @keydown.esc="handleCancel">
        <div
          ref="dialogRef"
          class="modal-content"
          role="dialog"
          aria-modal="true"
          aria-labelledby="confirm-dialog-title"
          aria-describedby="confirm-dialog-message"
          tabindex="-1"
        >
          <h3 id="confirm-dialog-title" class="modal-content__title">{{ title }}</h3>
          <p id="confirm-dialog-message" class="modal-content__message">{{ message }}</p>
          <div class="modal-content__footer">
            <button type="button" class="btn btn-secondary btn-md" @click="handleCancel">
              {{ cancelText }}
            </button>
            <button
              class="btn"
              type="button"
              :class="danger ? 'btn-danger' : 'btn-primary'"
              @click="handleConfirm"
            >
              {{ confirmText }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'

const props = withDefaults(defineProps<{
  visible: boolean
  title?: string
  message: string
  confirmText?: string
  cancelText?: string
  danger?: boolean
}>(), {
  title: '确认',
  confirmText: '确认',
  cancelText: '取消',
  danger: false,
})

const dialogRef = ref<HTMLElement | null>(null)

const emit = defineEmits<{
  confirm: []
  cancel: []
  'update:visible': [value: boolean]
}>()

function handleConfirm() {
  emit('confirm')
  emit('update:visible', false)
}

function handleCancel() {
  emit('cancel')
  emit('update:visible', false)
}

watch(() => props.visible, async (visible) => {
  if (!visible) return
  await nextTick()
  dialogRef.value?.focus()
})
</script>

<style scoped>
.modal-content {
  max-width: 420px;
  padding: 24px;
}
.modal-content__title {
  margin: 0 0 12px;
  font-size: 16px;
  font-weight: 600;
  color: var(--palwarm-text-primary);
}
.modal-content__message {
  margin: 0 0 20px;
  font-size: 14px;
  line-height: 1.5;
  color: var(--palwarm-text-secondary);
}
.modal-content__footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.modal-enter-active, .modal-leave-active {
  transition: opacity 0.2s;
}
.modal-enter-active .modal-content, .modal-leave-active .modal-content {
  transition: transform 0.2s;
}
.modal-enter-from, .modal-leave-to {
  opacity: 0;
}
.modal-enter-from .modal-content {
  transform: scale(0.95);
}
.modal-leave-to .modal-content {
  transform: scale(0.95);
}
</style>
