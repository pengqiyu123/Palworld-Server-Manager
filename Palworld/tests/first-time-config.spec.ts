import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('first-time configuration flow', () => {
  it('returns to the overview after first-time configuration is saved', async () => {
    const source = await readFile(resolve(process.cwd(), 'src/views/ConfigView.vue'), 'utf8')

    expect(source).toContain("route.query.firstTime === 'true'")
    expect(source).toContain("router.push('/overview')")
  })
})
