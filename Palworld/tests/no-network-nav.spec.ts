import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * 导航栏-网络（TDD · 绿）：删除独立的「网络」页。
 * 联机就绪检测（Radmin 5 档 L0-L4）已常驻 OverviewView，
 * 不需要再保留一个独立网络页，避免与概览功能重复。
 * 采用源码字符串断言（与 overiew-architecture.spec.ts 一致），
 * 不依赖 router 运行时，稳定可靠。
 */
describe('导航栏-网络', () => {
  it('Sidebar 不应渲染独立的「网络」导航项', async () => {
    const sidebar = await readFile(
      resolve(process.cwd(), 'src/components/layout/Sidebar.vue'),
      'utf8',
    )
    expect(sidebar).not.toContain("path: '/network'")
    expect(sidebar).not.toContain("label: '网络'")
  })

  it('Router 不应注册 /network 路由', async () => {
    const router = await readFile(
      resolve(process.cwd(), 'src/router/index.ts'),
      'utf8',
    )
    expect(router).not.toContain("path: '/network'")
    expect(router).not.toContain("name: 'network'")
  })
})
