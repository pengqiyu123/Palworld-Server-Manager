import { describe, it, expect, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { nextTick } from 'vue'
import { useOnboardingStore } from '@/stores/onboarding'
import { useServerStore } from '@/stores/server'
import { useNetworkStore } from '@/stores/network'

// 等待 watch（onSuccess 幂等锁）在响应式更新后执行
function flush() {
  return new Promise((r) => setTimeout(r, 0))
}

describe('onboarding store（收官 7 步派生）', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('S7 仅当 players >= 2 才置 pass', async () => {
    const ob = useOnboardingStore()
    const server = useServerStore()

    server.players = [] as any
    await nextTick()
    await flush()
    expect(ob.steps.s7.status).toBe('idle')

    // 1 个玩家仍未达里程碑
    server.players = [{ name: 'a' }] as any
    await nextTick()
    await flush()
    expect(ob.steps.s7.status).toBe('idle')

    // 2 个玩家 → 达标
    server.players = [{ name: 'a' }, { name: 'b' }] as any
    await nextTick()
    await flush()
    expect(ob.steps.s7.status).toBe('pass')
  })

  it('onSuccess 回调只触发一次（幂等锁 successFired）', async () => {
    const ob = useOnboardingStore()
    const server = useServerStore()
    let count = 0
    ob.onSuccess(() => {
      count++
    })

    // 第一次达标：触发一次
    server.players = [{ name: 'a' }, { name: 'b' }] as any
    await nextTick()
    await flush()
    expect(count).toBe(1)

    // 回落到 1 人再回到 2 人：不应再次触发
    server.players = [{ name: 'a' }] as any
    await nextTick()
    await flush()
    server.players = [{ name: 'a' }, { name: 'b' }, { name: 'c' }] as any
    await nextTick()
    await flush()
    expect(count).toBe(1)
  })

  it('resetSuccess 后可重新注册并再次触发', async () => {
    const ob = useOnboardingStore()
    const server = useServerStore()
    let count = 0
    ob.onSuccess(() => {
      count++
    })

    server.players = [{ name: 'a' }, { name: 'b' }] as any
    await nextTick()
    await flush()
    expect(count).toBe(1)

    // 重置成功锁后需重新注册回调（生产用法：重新开局）
    ob.resetSuccess()
    ob.onSuccess(() => {
      count++
    })

    server.players = [] as any
    await nextTick()
    await flush()
    server.players = [{ name: 'a' }, { name: 'b' }] as any
    await nextTick()
    await flush()
    expect(count).toBe(2)
  })

  it('S5 非 L4 时 fail（带 reason/action），L4 时 pass', async () => {
    const ob = useOnboardingStore()
    const network = useNetworkStore()

    network.readiness = {
      level: 'L3',
      virtual_ip: '26.1.2.3',
      adapter_status: 'Up',
      reason: '8211 未放行',
      next_action: { action_type: 'auto_recheck', label: '复查', payload: undefined },
    }
    await nextTick()
    await flush()
    expect(ob.steps.s5.status).toBe('fail')
    expect(ob.steps.s5.reason).toBe('8211 未放行')

    network.readiness = {
      level: 'L4',
      virtual_ip: '26.1.2.3',
      adapter_status: 'Up',
      reason: undefined,
      next_action: { action_type: 'copy_card', label: '复制', payload: undefined },
    }
    await nextTick()
    await flush()
    expect(ob.steps.s5.status).toBe('pass')
  })
})
