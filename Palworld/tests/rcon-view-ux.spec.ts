import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(__dirname, '..')
const view = readFileSync(resolve(root, 'src/views/RconView.vue'), 'utf-8')
const sidebar = readFileSync(resolve(root, 'src/components/layout/Sidebar.vue'), 'utf-8')
const router = readFileSync(resolve(root, 'src/router/index.ts'), 'utf-8')

describe('REST 服务器管理操作台', () => {
  it('不再把已弃用的 RCON 连接冒充为核心管理能力', () => {
    expect(view).toContain('服务器控制台')
    expect(view).toContain('REST 管理')
    expect(view).toContain('连接管理接口')
    expect(view).not.toContain('RCON 远程控制台')
    expect(view).not.toContain('连接 RCON')
    expect(sidebar).toContain("label: '服务器控制台'")
    expect(sidebar).not.toContain("label: 'RCON 控制台'")
    expect(router).toContain("meta: { title: '服务器控制台' }")
  })

  it('将常用安全操作呈现为带用途说明的可折叠横向快捷栏', () => {
    expect(view).toContain('快捷操作')
    expect(view).toContain('收起快捷操作')
    expect(view).toContain('shortcutsCollapsed')
    expect(view).toContain('查看服务器信息')
    expect(view).toContain('查看在线玩家')
    expect(view).toContain('保存世界')
    expect(view).toContain('发送公告')
  })

  it('所有快捷操作只预填命令，危险关服仍须先确认', () => {
    expect(view).toContain("prefill('Info')")
    expect(view).toContain("prefill('ShowPlayers')")
    expect(view).toContain("prefill('Save')")
    expect(view).not.toContain("runQuick('Info'")
    expect(view).not.toContain("runQuick('ShowPlayers'")
    expect(view).not.toContain("runQuick('Save'")
    expect(view).toContain('确认关服')
    expect(view).toContain('shutdownConfirmOpen')
    expect(view).toContain("prefill('Shutdown 60 '")
    expect(view).not.toContain("quick('Shutdown')")
  })

  it('保留原始命令和历史记录入口，并提供命令处理中状态', () => {
    expect(view).toContain('管理命令')
    expect(view).toContain('↑↓ 翻历史')
    expect(view).toContain('commandPending')
    expect(view).toContain('点击后填入下方命令框，再由你确认发送')
  })

  it('快捷栏固定显示在控制台上方，并支持横向滚动', () => {
    expect(view).toContain('快捷操作已收起')
    expect(view).toContain('.shortcut-list')
    expect(view).toContain('overflow-x: auto')
    expect(view).not.toContain('grid-template-columns: 252px')
  })

  it('命令发送失败后会结束执行状态，不会永久禁用输入框', () => {
    const start = view.indexOf('async function runCommand')
    const end = view.indexOf('async function onSend', start)
    const commandFunction = view.slice(start, end)

    expect(commandFunction).toContain('try {')
    expect(commandFunction).toContain('finally')
    expect(commandFunction).toContain('commandPending.value = false')
  })

  it('控制台只显示后端确认的连接端点，不硬编码默认端口', () => {
    expect(view).toContain('managementEndpoint')
    expect(view).not.toContain('命令记录 · 127.0.0.1:25575')
    expect(view).not.toContain('RCON 已连接（127.0.0.1:25575）')
  })
})
