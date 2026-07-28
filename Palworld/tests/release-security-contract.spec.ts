import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(__dirname, '..')
const read = (path: string) => readFileSync(resolve(root, path), 'utf-8')

describe('正式发布安全契约', () => {
  it('生产 CSP 不包含开发服务器或动态代码执行权限', () => {
    const config = JSON.parse(read('src-tauri/tauri.conf.json'))
    const csp = String(config.app.security.csp)
    const devCsp = String(config.app.security.devCsp)

    expect(csp).not.toContain('http://localhost:5222')
    expect(csp).not.toContain('ws://localhost:5222')
    expect(csp).not.toContain("'unsafe-eval'")
    expect(csp).not.toContain("'unsafe-inline'")
    expect(devCsp).toContain('http://localhost:5222')
    expect(devCsp).toContain('ws://localhost:5222')
  })

  it('全局提示只渲染文本，不解析后端返回的 HTML', () => {
    const tooltip = read('src/components/ui/Tooltip.vue')
    const infoTip = read('src/components/ui/InfoTip.vue')
    const uiStore = read('src/stores/ui.ts')

    expect(tooltip).not.toContain('v-html')
    expect(tooltip).toContain('uiStore.tooltip.text')
    expect(infoTip).toContain('text: string')
    expect(infoTip).not.toContain('html: string')
    expect(uiStore).toContain('text: string')
    expect(uiStore).not.toContain('html: string')
  })

  it('正式构建不包含未使用的剪贴板插件和已弃用的 RCON 客户端', () => {
    const cargo = read('src-tauri/Cargo.toml')
    const main = read('src-tauri/src/main.rs')
    const packageJson = read('package.json')
    const capabilities = read('src-tauri/capabilities/default.json')

    expect(cargo).not.toContain('tauri-plugin-clipboard-manager')
    expect(cargo).not.toMatch(/^rcon\s*=/m)
    expect(main).not.toContain('tauri_plugin_clipboard_manager::init')
    expect(main).not.toContain('mod rcon;')
    expect(packageJson).not.toContain('@tauri-apps/plugin-clipboard-manager')
    expect(capabilities).not.toContain('clipboard-manager:')
  })

  it('自动配置防火墙时不公开本机管理端口', () => {
    const firewall = read('src-tauri/src/firewall.rs')
    const addRules = firewall.slice(firewall.indexOf('pub async fn add_firewall_rules'))

    expect(addRules).not.toContain('(25575, "TCP"')
    expect(addRules).not.toContain('(8212, "TCP"')
  })

  it('ES2020 代码不使用 Array.at 且复制操作等待真实结果', () => {
    const rconStore = read('src/stores/rcon.ts')
    const configView = read('src/views/ConfigView.vue')

    expect(rconStore).not.toContain('.at(')
    expect(configView).toContain('await navigator.clipboard.writeText(cmd)')
    expect(configView).toContain("if (!document.execCommand('copy'))")
  })

  it('npm、Cargo、Tauri 与界面显示使用同一个发布版本', () => {
    const packageVersion = JSON.parse(read('package.json')).version as string
    const cargo = read('src-tauri/Cargo.toml')
    const tauri = JSON.parse(read('src-tauri/tauri.conf.json'))
    const sidebar = read('src/components/layout/Sidebar.vue')
    const cargoVersion = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1]

    expect(cargoVersion).toBe(packageVersion)
    expect(tauri.version).toBe(packageVersion)
    expect(sidebar).toContain(`v${packageVersion}`)
  })

  it('卸载把备份留在安装目录外，重装后再自动恢复', () => {
    const hooks = read('src-tauri/nsis/installer-hooks.nsh')
    const postInstall = hooks.slice(
      hooks.indexOf('!macro NSIS_HOOK_POSTINSTALL'),
      hooks.indexOf('!macro NSIS_HOOK_PREUNINSTALL'),
    )
    const postUninstall = hooks.slice(hooks.indexOf('!macro NSIS_HOOK_POSTUNINSTALL'))

    expect(postInstall).toContain('Rename "$PalworldBackupsPreserved" "$INSTDIR\\backups"')
    expect(postUninstall).not.toContain('CreateDirectory "$INSTDIR"')
    expect(postUninstall).not.toContain('Rename "$PalworldBackupsPreserved" "$INSTDIR\\backups"')
  })

  it('随第三方游戏数据保留完整的上游 MIT 授权条款', () => {
    const notice = read('src-tauri/resources/palworld-save-tools/NOTICE.md')
    const thirdParty = read('THIRD_PARTY_NOTICES.md')

    expect(notice).toContain('MIT License')
    expect(notice).toContain('Copyright (c) 2026 Pylar')
    expect(notice).toContain('Permission is hereby granted, free of charge')
    expect(notice).toContain('THE SOFTWARE IS PROVIDED "AS IS"')
    expect(thirdParty).toContain('PalworldSaveTools')
    expect(thirdParty).toContain('GPL-3.0-or-later')
  })
})
