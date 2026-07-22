# AGENTS.md

## 项目简介

Palworld Server Manager 是一款面向 Windows 平台的幻兽帕鲁（Palworld）专用服务器**一站式 GUI 管理桌面应用**，目标用户为本地开服的玩家与小型社区服主。基于 Tauri 2 + Vue 3 构建，覆盖从搭建、配置、启动、监控到运维的完整生命周期。

本目录（`Palworld/`）是应用的最终部署目录，包含前端源码、Rust 后端源码、运维脚本以及截图输出目录。

## 技术栈

- **前端**：Vue 3（Composition API + `<script setup lang="ts">`）、TypeScript（strict 模式）、Pinia、Vue Router、Vite、`@tauri-apps/api`、`@tauri-apps/plugin-dialog`、`@tauri-apps/plugin-clipboard-manager`、`lucide-vue-next`
- **桌面端**：Tauri 2、Rust（edition 2021）
- **包管理器**：npm

## 常用命令

在 `Palworld/` 目录执行：

- `npm run dev`：启动前端开发服务（Vite Dev Server，端口 5173）
- `npm run build`：执行类型检查（`vue-tsc`）并构建前端到 `dist/`
- `npm run preview`：预览前端构建结果
- `npm run lint`：仅执行 TypeScript 类型检查（`vue-tsc --noEmit`，不产出文件）
- `npm run verify`：等价于 `npm run lint && npm run build`，类型检查 + 生产构建
- `npm run tauri:dev`：启动桌面应用开发模式（自动启动前端 dev server + Rust 编译 + 弹出窗口）
- `npm run tauri:build`：构建桌面应用（前端构建 + Rust release 编译 + NSIS 安装包）

在 `Palworld/src-tauri/` 目录执行：

- `cargo check`：Rust 类型检查（CI 入口，零错误零警告）
- `cargo build --release`：Rust release 编译
- `cargo run`：直接运行 Rust 二进制（不启动前端 dev server，仅用于调试后端）

## 代码规范

### 通用
- 2 空格缩进、单引号，语句末尾不强制分号
- 文件命名：组件文件使用 PascalCase，变量和函数使用 camelCase
- 修改后至少运行 `npm run build` 或 `npm run verify`，确保类型检查与生产构建通过

### 前端（Vue 3 + TypeScript）
- Vue 组件统一使用 `<script setup lang="ts">` 与 Composition API
- TypeScript 开启 strict 模式，禁止未使用的变量和参数（`noUnusedLocals` / `noUnusedParameters`）
- 通用视觉样式集中维护在 `src/style.css`（`--palwarm-*` CSS 变量 + 玻璃面板 `.glass-panel` 类）
- 业务状态保留在 Pinia store 中，不在组件内直接 `invoke()`
- `invoke()` 调用统一走 `src/api/tauri.ts` 封装层（统一错误处理 + 类型对齐）
- 新增图标按需引入 `lucide-vue-next`

### Rust 后端
- 所有 Tauri command 返回 `Result<T, String>`，错误信息使用中文
- 不使用 `unsafe` 代码
- 共享状态通过 `tauri::State<Arc<T>>` 注入，内部用 `Mutex` 保护
- 涉及用户输入的 PowerShell 调用必须用 `shell_escape::escape(...)` 包裹
- 后台线程（日志收集等）使用 `std::thread::spawn`，错误吞掉但 `eprintln!` 记录

## 运维脚本说明

所有运维脚本位于 `Palworld/scripts/`，使用 PowerShell 编写。脚本注释使用英文（避免编码问题），输出可被程序化解析。

| 脚本 | 用途 | 输出 | 退出码 |
|------|------|------|--------|
| `verify-window.ps1` | 窗口尺寸验证 + 自动修复 + 截图 | JSON：`{status, hwnd, width, height, screenshot}` | 0 = OK/FIXED，1 = FAILED/NOT_FOUND |
| `switch-route.ps1` | 文件触发机制路由切换 + 截图（E2E 验收用） | 控制台日志 + 截图文件 | 0 = 成功，1 = 失败 |
| `check-webview2.ps1` | WebView2 运行时注册表检查（HKLM/HKCU EdgeUpdate） | 控制台日志 | 0 = 已安装，1 = 未找到 |
| `health-check.ps1` | 综合诊断：WebView2 + 端口 5222 + 防火墙 + 窗口 + 截图 | JSON 报告 + 截图文件 | 0 = OK，1 = WARN，2 = ERROR |

### 脚本调用示例

```powershell
# 窗口尺寸验证（应用启动后执行）
powershell -ExecutionPolicy Bypass -File scripts/verify-window.ps1

# 路由切换截图（需应用已启动）
powershell -ExecutionPolicy Bypass -File scripts/switch-route.ps1 -RouteName config

# WebView2 运行时检查
powershell -ExecutionPolicy Bypass -File scripts/check-webview2.ps1

# 综合健康诊断（应用启动后执行，输出 JSON 报告）
powershell -ExecutionPolicy Bypass -File scripts/health-check.ps1
```

### 截图目录

所有脚本生成的截图默认保存到 `Palworld/screenshots/`，命名规则：
- `window-YYYYMMDD-HHmmss.png`：窗口尺寸验证截图
- `route-<name>-YYYYMMDD-HHmmss.png`：路由切换截图
- `health-YYYYMMDD-HHmmss.png`：健康诊断截图

## 模块结构

### Rust 后端（`src-tauri/src/`）

| 文件 | 职责 |
|------|------|
| `main.rs` | Tauri Builder 配置 + 命令注册 + State 注入（≤ 80 行） |
| `server.rs` | 服务器进程启动/停止/状态/日志收集（上限 500 行 + 进程退出事件 emit） |
| `config.rs` | INI 解析/默认值/描述/写入前自动备份 + `list_config_backups` / `restore_config_backup` |
| `presets.rs` | 4 套预设（default / pve-friendly / pvp-competitive / speedrun）的 `list_presets` / `apply_preset` |
| `firewall.rs` | PowerShell 防火墙规则（`shell-escape` 防注入） |
| `network.rs` | Radmin LAN + 本地 IP 检测（`shell-escape` 防注入） |
| `rcon.rs` | TCP RCON 协议客户端 |
| `settings.rs` | 应用设置 JSON 持久化（`%AppData%/PalworldServerManager/settings.json`） |
| `route_switch.rs` | 文件触发机制路由切换（E2E 验收用，非生产路径） |
| `window_fix.rs` | Rust 内 Win32 强制修复窗口尺寸（三重降级方案 Layer 2） |

### 预设资源（`src-tauri/presets/`）

| 文件 | 预设名 | 描述 |
|------|--------|------|
| `default.json` | 默认预设 | 10 个核心参数的默认值 |
| `pve-friendly.json` | PvE 友好 | Difficulty=None / PlayerDamageRate=0.5 / PalSpawnNumRate=1.5 |
| `pvp-competitive.json` | PvP 竞技 | bEnablePlayerToPlayerDamage=true / DeathPenalty=All |
| `speedrun.json` | 速通 | PalCaptureRate=2.0 / PalSpawnNumRate=2.0 / ExpRate=2.0 |

### 前端（`src/`）

| 目录 | 职责 |
|------|------|
| `src/main.ts` | 应用入口（按序初始化 settings → server（含 listener）→ network → config → rcon） |
| `src/App.vue` | 根组件（AppShell 布局 + Toast 容器） |
| `src/style.css` | 全局样式 + `--palwarm-*` CSS 变量 + `.glass-panel` 类 |
| `src/router/index.ts` | Vue Router（5 个路由：dashboard / config / network / rcon / troubleshoot） |
| `src/stores/` | Pinia stores（server / config / network / rcon / settings / toast） |
| `src/api/tauri.ts` | Tauri `invoke` 封装（统一错误处理 + 类型对齐） |
| `src/types/tauri.ts` | Tauri 命令的 TypeScript 类型声明 |
| `src/components/layout/` | AppShell / Sidebar / HeaderBar |
| `src/components/ui/` | GlassPanel / StateBadge / BaseButton / Toast / ConfirmDialog / PresetSelector / DiffIndicator / DiagnosticReport |
| `src/components/server/` | LogPanel / PathDialog / PlayerTable |
| `src/views/` | DashboardView / ConfigView / NetworkView / RconView / TroubleshootView |

### Tauri 能力权限（`src-tauri/capabilities/default.json`）

当前已声明的权限：
- `core:default` / `core:window:default` / `core:webview:default` / `core:event:default`
- `dialog:default` / `dialog:allow-open` / `dialog:allow-save`
- `clipboard-manager:allow-write-text`

## 设计系统

应用视觉系统源自 `palworld-warm-glass-preview` 设计稿，关键 CSS 变量定义在 `src/style.css`：

- 主色 `--palwarm-primary`: `#e66f51`（珊瑚暖橙）
- 背景 `--palwarm-background`: `#f5ede2`（奶油米白）+ 三层径向渐变光晕
- 卡片 `--palwarm-card`: `rgba(255,252,247,0.72)` 半透明
- 玻璃面板：`backdrop-filter: blur(24px) saturate(145%)`，边框 `rgba(116,88,72,0.14)`
- 圆角：12px（sm）/ 18px（md）/ 26px（lg）/ 32px（panel）
- 字体：Inter + Microsoft YaHei（中文优先）
- 状态色：success `#4f8a6b` / warning `#b8782f` / error `#c9554d` / info `#4b7896`

## 数据存储位置

应用运行时数据保存在 `%AppData%/PalworldServerManager/`：

| 路径 | 用途 |
|------|------|
| `settings.json` | 应用设置（serverPath / rconHost / rconPort / rconPassword 等） |
| `config-backups/PalWorldSettings-YYYYMMDD-HHmmss.ini` | 配置文件自动备份（最多保留 20 份） |

## 本次优化总结（2026-07 · change-id: `optimize-palworld-design`）

详细功能对照见 `docs/02-Requirements-Specification.md` §9。本次优化完成的核心工作：

1. **Rust 后端 P1 功能补齐**：新增 `presets.rs` 模块 + 4 套预设；`config.rs` 增加写入前自动备份 + `list_config_backups` / `restore_config_backup`；`server.rs` 日志上限 500 + 进程退出事件 emit + `export_server_logs`；`firewall.rs` / `network.rs` 增加 `shell-escape` 防护
2. **前端 P1 功能补齐**：全局 Toast 队列（`useToastStore`）；RCON 玩家列表 + 命令历史 + 踢出/封禁；配置预设选择器 + 修改前对比指示器 + 恢复备份；网络页联机向导 4 步 + 复制地址；故障页一键诊断 + 导出日志
3. **UI 组件补齐**：新增 `PresetSelector` / `DiffIndicator` / `DiagnosticReport` / `PlayerTable`；重构 `Toast.vue` 为全局队列
4. **视觉修正**：`GlassPanel.vue` backdrop-filter 调整为 `blur(24px) saturate(145%)`；`StateBadge.vue` 添加 `@keyframes pulse` 动画；`Sidebar.vue` 当前页添加左侧 3px 橙色竖线
5. **运维脚本整合**：复制 `check-webview2.ps1`；新建 `health-check.ps1` 综合诊断脚本（JSON 报告 + 退出码 0/1/2）
6. **权限扩展**：`capabilities/default.json` 新增 `dialog:allow-save` / `clipboard-manager:allow-write-text`
7. **构建脚本**：`package.json` 新增 `lint` 与 `verify` 命令

## 参考资料

- 需求规格：`docs/02-Requirements-Specification.md`
- 技术架构：`docs/03-Technical-Architecture.md`
- 研究分析：`docs/01-Research-Analysis-Report.md`
- 任务清单：`.trae/specs/optimize-palworld-design/tasks.md`
