import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(__dirname, '..')
const main = readFileSync(resolve(root, 'src/main.ts'), 'utf-8')

describe('真实运行模式', () => {
  it('只有显式 VITE_MOCK=true 才能注入 RCON 样例日志', () => {
    expect(main).toContain("const MOCK = env?.VITE_MOCK === 'true'")
    const mockBranchStart = main.indexOf('if (MOCK)')
    const realBranchStart = main.indexOf('await Promise.all', mockBranchStart)
    const mockBranch = main.slice(mockBranchStart, realBranchStart)

    expect(mockBranch).toContain('rconStore.seedMock()')
  })
})
