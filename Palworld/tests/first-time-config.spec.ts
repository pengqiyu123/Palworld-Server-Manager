import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('first-time configuration flow', () => {
  it('returns to the overview after first-time configuration is saved', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/ConfigView.vue'), 'utf8')

    expect(source).toContain("route.query.firstTime === 'true'")
    expect(source).toContain("router.push('/overview')")
  })

  it('新手联机只要求 REST 管理能力，不再要求即将弃用的 RCON', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/stores/onboarding.ts'), 'utf8')

    expect(source).toContain("cfg['RESTAPIEnabled']")
    expect(source).not.toContain("cfg['RCONEnabled']")
    expect(source).not.toContain('port_25575_open &&')
    expect(source).not.toContain('port_8212_open')
  })
})
