import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('系统日志入口', () => {
  it('侧边栏只提供系统日志入口，不再展示故障排查', async () => {
    const sidebar = await readFile(
      resolve(process.cwd(), 'src/components/layout/Sidebar.vue'),
      'utf8',
    )

    expect(sidebar).toContain("path: '/system-logs'")
    expect(sidebar).toContain("label: '系统日志'")
    expect(sidebar).not.toContain("label: '故障排查'")
    expect(sidebar).not.toContain('to="/settings"')
  })

  it('旧地址统一跳转到系统日志页', async () => {
    const router = await readFile(
      resolve(process.cwd(), 'src/router/index.ts'),
      'utf8',
    )

    expect(router).toContain("path: '/settings'")
    expect(router).toContain("path: '/troubleshoot'")
    expect(router).toContain("redirect: '/system-logs'")
    expect(router).toContain("meta: { title: '系统日志' }")
  })

  it('页面只显示系统日志操作，不包含诊断和排查指南', async () => {
    const view = await readFile(
      resolve(process.cwd(), 'src/views/SystemLogsView.vue'),
      'utf8',
    )

    expect(view).toContain('系统日志')
    expect(view).toContain('复制错误信息')
    expect(view).toContain('导出日志')
    expect(view).toContain('清空日志')
    expect(view).not.toContain('一键诊断')
    expect(view).not.toContain('常见问题')
    expect(view).not.toContain('排查步骤')
  })
})
