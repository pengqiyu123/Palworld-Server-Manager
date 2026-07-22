import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/api/tauri'
import type { AppSettings } from '@/types/tauri'

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<AppSettings>({
    server_path: '',
    config_path: '',
    rcon_host: '127.0.0.1',
    rcon_port: 25575,
    rcon_password: '',
  })

  async function load() {
    try {
      settings.value = await api.settings.load()
    } catch (e) {
      console.warn('加载设置失败，使用默认值:', e)
    }
  }

  async function save() {
    await api.settings.save(settings.value)
  }

  function update(patch: Partial<AppSettings>) {
    settings.value = { ...settings.value, ...patch }
  }

  /**
   * 从 server_path 拼接 PalWorldSettings.ini 配置文件路径。
   * 路径约定：{server_path}/Pal/Saved/Config/WindowsServer/PalWorldSettings.ini
   */
  function computeConfigPath(serverPath: string): string {
    return `${serverPath}/Pal/Saved/Config/WindowsServer/PalWorldSettings.ini`
  }

  /**
   * 启动探测：加载 settings.json → 若 server_path 为空则自动探测 Steam 安装路径。
   * 命中后保存到 settings.json，供后续 server/config/rest_proxy 使用。
   */
  async function initDetectSettings(): Promise<void> {
    await load()
    if (!settings.value.server_path) {
      try {
        const paths = await api.steam.detect()
        if (paths.length > 0) {
          update({ server_path: paths[0] })
          await save()
        }
      } catch (e) {
        console.warn('Steam 探测失败:', e)
      }
    }
    // 同步 config_path 到 settings（供 configStore.load 使用）
    if (settings.value.server_path && !settings.value.config_path) {
      settings.value.config_path = computeConfigPath(settings.value.server_path)
    }
  }

  return {
    settings,
    load,
    save,
    update,
    computeConfigPath,
    initDetectSettings,
  }
})
