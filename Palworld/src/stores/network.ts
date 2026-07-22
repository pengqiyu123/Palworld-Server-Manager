import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/api/tauri'
import type { FirewallStatus, RadminLanStatus, RadminReadiness } from '@/types/tauri'
import { useSettingsStore } from '@/stores/settings'

export const useNetworkStore = defineStore('network', () => {
  const firewall = ref<FirewallStatus>({
    port_8211_open: false,
    port_27015_open: false,
    port_25575_open: false,
    port_8212_open: false,
  })
  const radmin = ref<RadminLanStatus>({
    installed: false,
    virtual_ip: '',
    adapter_status: '未知',
  })
  // 收官新增：5 档 Radmin 联机就绪度（一次拿全）
  const readiness = ref<RadminReadiness | null>(null)

  async function checkFirewall() {
    firewall.value = await api.firewall.check()
  }

  async function addFirewallRules() {
    return await api.firewall.addRules()
  }

  /** R2 旧接口（兼容期）：仅 installed / virtual_ip / adapter_status */
  async function checkRadmin() {
    radmin.value = await api.network.checkRadminLan()
  }

  /** 收官新接口：5 档分级检测，返回完整 RadminReadiness */
  async function checkReadiness(): Promise<RadminReadiness | null> {
    const settingsStore = useSettingsStore()
    const serverPath = settingsStore.settings.server_path
    if (!serverPath) {
      readiness.value = null
      return null
    }
    readiness.value = await api.network.checkReadiness(serverPath)
    return readiness.value
  }

  async function checkPort(port: number): Promise<string | null> {
    return await api.network.checkPortUsage(port)
  }

  /** 统一检测：firewall + radmin(旧) + readiness(新) 并行 */
  async function checkAll(): Promise<void> {
    await Promise.all([checkFirewall(), checkRadmin(), checkReadiness()])
  }

  return {
    firewall,
    radmin,
    readiness,
    checkFirewall,
    addFirewallRules,
    checkRadmin,
    checkReadiness,
    checkPort,
    checkAll,
  }
})
