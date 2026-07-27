import { beforeEach, describe, expect, it, vi } from 'vitest'
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createPinia, setActivePinia } from 'pinia'

const applyPreset = vi.fn()

vi.mock('@/api/tauri', () => ({
  api: {
    config: {
      applyPreset,
    },
  },
}))

describe('configuration presets', () => {
  it('shows a selectable beginner preset with preview and a separate apply action', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/ConfigView.vue'), 'utf8')

    expect(source).toContain('listPresets')
    expect(source).toContain('selectedPreset')
    expect(source).toContain('presetPreview')
    expect(source).toContain('onApplyPreset')
    expect(source).toContain('应用预设')

    const mountedStart = source.indexOf('onMounted(async () => {')
    expect(source.indexOf('api.config.listPresets()', mountedStart)).toBeGreaterThan(mountedStart)
    expect(source).not.toContain("selectedPreset.value = presets.value[0]")
  })
})

describe('configuration preset state', () => {
  beforeEach(() => {
    applyPreset.mockReset()
    setActivePinia(createPinia())
  })

  it('keeps a preset application pending until the user saves it', async () => {
    const { useConfigStore } = await import('@/stores/config')
    const store = useConfigStore()
    store.config = { ExpRate: '1.000000', UnknownFutureOption: 'KeepMe' }
    store.originalConfig = { ...store.config }
    applyPreset.mockResolvedValue({ ExpRate: '1.300000', UnknownFutureOption: 'KeepMe' })

    await store.applyPreset('casual')

    expect(store.config.ExpRate).toBe('1.300000')
    expect(store.config.UnknownFutureOption).toBe('KeepMe')
    expect(store.dirtyCount).toBe(1)
  })

  it('uses the game PvP field and explains the WorldOption override risk', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/ConfigView.vue'), 'utf8')
    const presets = await Promise.all([
      readFile(resolve(process.cwd(), 'src-tauri/presets/casual.json'), 'utf8'),
      readFile(resolve(process.cwd(), 'src-tauri/presets/normal.json'), 'utf8'),
      readFile(resolve(process.cwd(), 'src-tauri/presets/challenge.json'), 'utf8'),
    ])

    expect(source).toContain('bEnablePlayerToPlayerDamage')
    expect(source).not.toContain("key: 'bIsPvP'")
    expect(source).toContain('WorldOption.sav')
    for (const preset of presets) {
      expect(preset).toContain('bEnablePlayerToPlayerDamage')
      expect(preset).not.toContain('"bIsPvP"')
    }
  })

  it('matches the researched casual, normal, and challenge setting matrix', async () => {
    const files = await Promise.all([
      readFile(resolve(process.cwd(), 'src-tauri/presets/casual.json'), 'utf8'),
      readFile(resolve(process.cwd(), 'src-tauri/presets/normal.json'), 'utf8'),
      readFile(resolve(process.cwd(), 'src-tauri/presets/challenge.json'), 'utf8'),
    ])
    const maps = files.map((file) => Object.fromEntries(
      (JSON.parse(file) as Array<{ name: string; value: string }>).map(({ name, value }) => [name, value]),
    ))
    const expected = [
      {
        Difficulty: 'None', DayTimeSpeedRate: '0.800000', NightTimeSpeedRate: '1.200000',
        ExpRate: '1.800000', PalCaptureRate: '2.000000', PalSpawnNumRate: '1.500000',
        PalDamageRateAttack: '1.200000', PalDamageRateDefense: '0.700000',
        PlayerDamageRateAttack: '1.500000', PlayerDamageRateDefense: '0.700000',
        PlayerStomachDecreaceRate: '0.500000', PlayerStaminaDecreaceRate: '0.700000',
        PalStomachDecreaceRate: '0.500000', PalStaminaDecreaceRate: '0.700000',
        CollectionDropRate: '1.500000', EnemyDropItemRate: '1.500000',
        ItemWeightRate: '0.500000', PalEggDefaultHatchingTime: '0.000000',
        WorkSpeedRate: '1.500000', DeathPenalty: 'None',
        BaseCampMaxNumInGuild: '10', BaseCampWorkerMaxNum: '50',
        bEnableFriendlyFire: 'False', bEnablePlayerToPlayerDamage: 'False',
        bEnableInvaderEnemy: 'True',
      },
      {
        Difficulty: 'None', DayTimeSpeedRate: '1.000000', NightTimeSpeedRate: '1.000000',
        ExpRate: '1.200000', PalCaptureRate: '1.000000', PalSpawnNumRate: '1.000000',
        PalDamageRateAttack: '1.000000', PalDamageRateDefense: '1.000000',
        PlayerDamageRateAttack: '1.000000', PlayerDamageRateDefense: '1.000000',
        PlayerStomachDecreaceRate: '1.000000', PlayerStaminaDecreaceRate: '1.000000',
        PalStomachDecreaceRate: '1.000000', PalStaminaDecreaceRate: '1.000000',
        CollectionDropRate: '1.000000', EnemyDropItemRate: '1.000000',
        ItemWeightRate: '1.000000', PalEggDefaultHatchingTime: '1.000000',
        WorkSpeedRate: '1.000000', DeathPenalty: 'ItemAndEquipment',
        BaseCampMaxNumInGuild: '4', BaseCampWorkerMaxNum: '25',
        bEnableFriendlyFire: 'False', bEnablePlayerToPlayerDamage: 'False',
        bEnableInvaderEnemy: 'True',
      },
      {
        Difficulty: 'None', DayTimeSpeedRate: '1.200000', NightTimeSpeedRate: '0.800000',
        ExpRate: '0.800000', PalCaptureRate: '0.700000', PalSpawnNumRate: '1.000000',
        PalDamageRateAttack: '1.200000', PalDamageRateDefense: '1.500000',
        PlayerDamageRateAttack: '0.700000', PlayerDamageRateDefense: '1.500000',
        PlayerStomachDecreaceRate: '1.500000', PlayerStaminaDecreaceRate: '1.300000',
        PalStomachDecreaceRate: '1.300000', PalStaminaDecreaceRate: '1.300000',
        CollectionDropRate: '0.800000', EnemyDropItemRate: '0.800000',
        ItemWeightRate: '1.300000', PalEggDefaultHatchingTime: '2.000000',
        WorkSpeedRate: '0.800000', DeathPenalty: 'All',
        BaseCampMaxNumInGuild: '3', BaseCampWorkerMaxNum: '20',
        bEnableFriendlyFire: 'True', bEnablePlayerToPlayerDamage: 'False',
        bEnableInvaderEnemy: 'True',
      },
    ]

    expect(maps).toEqual(expected)
  })
})
