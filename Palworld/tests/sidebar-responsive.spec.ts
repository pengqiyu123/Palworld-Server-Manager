import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(__dirname, '..')
const sidebar = readFileSync(resolve(root, 'src/components/layout/Sidebar.vue'), 'utf-8')
const styles = readFileSync(resolve(root, 'src/style.css'), 'utf-8')

describe('窄窗侧栏', () => {
  it('只隐藏导航标签并保留 AppIcon 图标容器', () => {
    expect(sidebar).toContain('class="nav-item-label"')
    expect(styles).toContain('.sidebar .nav-item-label')
    expect(styles).not.toContain('.sidebar .nav-item span,')
  })
})
