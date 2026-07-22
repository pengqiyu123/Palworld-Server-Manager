import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/api/tauri'
import type { ConfigValue } from '@/types/tauri'

export const useConfigStore = defineStore('config', () => {
  const configPath = ref('')
  const config = ref<Record<string, string>>({})
  // 原始配置（加载时的快照），用于 DiffIndicator 显示原值
  const originalConfig = ref<Record<string, string>>({})
  const descriptions = ref<ConfigValue[]>([])
  const dirty = ref<Set<string>>(new Set())

  const dirtyCount = computed(() => dirty.value.size)

  async function load(path?: string) {
    const targetPath = path || configPath.value
    if (!targetPath) {
      throw new Error('配置文件路径为空')
    }
    configPath.value = targetPath
    config.value = await api.config.read(targetPath)
    // 保存原始快照，DiffIndicator 据此显示原值
    originalConfig.value = { ...config.value }
    if (descriptions.value.length === 0) {
      descriptions.value = await api.config.getDescriptions()
    }
    dirty.value.clear()
  }

  // 应用预设合并结果后，刷新原始快照与 dirty 状态
  async function applyPreset(name: string) {
    const merged = await api.config.applyPreset(name, config.value)
    config.value = merged
    // 预设视为新的"原始值"，避免整页全橙
    originalConfig.value = { ...merged }
    dirty.value.clear()
  }

  // 从备份恢复后，重新加载配置（调用方负责触发 Rust 端 restore_config_backup）
  async function reloadFromBackup() {
    if (!configPath.value) {
      throw new Error('配置文件路径为空')
    }
    config.value = await api.config.read(configPath.value)
    originalConfig.value = { ...config.value }
    dirty.value.clear()
  }

  async function loadDescriptions() {
    if (descriptions.value.length === 0) {
      descriptions.value = await api.config.getDescriptions()
    }
  }

  async function save() {
    if (!configPath.value) {
      throw new Error('配置文件路径为空')
    }
    await api.config.write(configPath.value, config.value)
    // 保存成功后刷新原始快照（Rust 端 write_config 会自动备份旧文件）
    originalConfig.value = { ...config.value }
    dirty.value.clear()
  }

  async function resetToDefault() {
    config.value = await api.config.getDefault()
    dirty.value = new Set(Object.keys(config.value))
  }

  function update(key: string, value: string) {
    if (config.value[key] !== value) {
      config.value[key] = value
      // 与原始值比较：相同则从 dirty 移除，不同则加入
      if (originalConfig.value[key] === value) {
        dirty.value.delete(key)
      } else {
        dirty.value.add(key)
      }
    }
  }

  function cancelEdits() {
    // 还原到原始快照，并清空 dirty
    config.value = { ...originalConfig.value }
    dirty.value.clear()
  }

  return {
    configPath,
    config,
    originalConfig,
    descriptions,
    dirty,
    dirtyCount,
    load,
    applyPreset,
    reloadFromBackup,
    loadDescriptions,
    save,
    resetToDefault,
    update,
    cancelEdits,
  }
})
