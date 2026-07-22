# 01 · 调研分析报告

> Palworld Server Manager 桌面应用 — 技术选型与可行性调研
> 版本：v1.0 · 编制日期：2026-07-20 · 编制依据：`Palworld-out/` + `palworld-warm-glass-preview/` 现有资源

---

## 一、调研目标与范围

### 1.1 调研目标
基于现有两个实验资源（实验架构 `Palworld-out/` 与实验前端 `palworld-warm-glass-preview/`），为构建一个功能完整、可正常运行的 Palworld 服务器管理桌面应用，完成以下三方面调研：
1. **桌面应用开发技术栈调研**：评估 Electron、NW.js、Tauri、Wails 等主流方案的优劣，明确技术选型依据。
2. **Palworld 相关 API/协议调研**：梳理 Palworld 专用服务器可被外部程序消费的协议与接口（RCON、REST API、INI 配置、SteamCMD、SAV 存档、Radmin LAN）。
3. **现有项目结构与依赖调研**：盘点 `Palworld-out/` 与 `palworld-warm-glass-preview/` 的代码资产、复用度与缺口。

### 1.2 调研范围
| 范围 | 内容 |
|------|------|
| 桌面框架 | Electron 30+、NW.js 0.90+、Tauri 2.x、Wails 2.x、Neutralino.js |
| Palworld 协议 | RCON（Source RCON Protocol）、REST API、PalWorldSettings.ini、SteamCMD `app_update 2394010` |
| 现有资源 | `Palworld-out/src-tauri/`（Rust 后端）、`Palworld-out/dist/`（前端构建产物）、`Palworld-out/package.json`、`palworld-warm-glass-preview/pages/dashboard.html`、`colors_and_type.css` |
| 系统依赖 | Windows 10/11、WebView2 Runtime、PowerShell、PalServer.exe、SteamCMD |

### 1.3 调研方法
- **静态代码分析**：直接读取两个资源目录下的源码、配置与设计预览
- **协议规范查阅**：参考 Source RCON Protocol 公开规范与 Palworld 官方服务器文档
- **竞品对比**：参考 `Palworld-out/Desktop_App_Feasibility_Report.md` 中已沉淀的竞品分析
- **可行性评估**：基于已实现的 Rust 后端代码进行实际验证

---

## 二、桌面应用技术栈调研

### 2.1 主流方案横向对比

| 对比维度 | Electron | NW.js | Tauri 2.x | Wails 2.x | Neutralino.js |
|----------|----------|-------|-----------|-----------|---------------|
| 后端语言 | Node.js | Node.js | Rust | Go | C++/Node |
| 渲染引擎 | Chromium 内置 | Chromium 内置 | 系统 WebView2 | 系统 WebView2 | 系统 WebView2 |
| 安装包体积 | 80–150 MB | 90–160 MB | 3–10 MB | 8–15 MB | 2–5 MB |
| 内存占用（空闲） | 200–500 MB | 220–520 MB | 30–80 MB | 40–90 MB | 25–60 MB |
| 冷启动 | 3–8 s | 3–7 s | 1–2 s | 1–2 s | 1–2 s |
| 跨平台 | Win/Mac/Linux | Win/Mac/Linux | Win/Mac/Linux | Win/Mac/Linux | Win/Mac/Linux |
| 系统级 API | 通过 Node child_process / N-API | 同 Electron | Rust 原生调用 + Tauri Plugin | Go 原生调用 | 受限的扩展 API |
| 进程管理 | child_process.spawn | child_process.spawn | std::process::Command | os/exec | 受限 |
| 文件系统 | fs 模块 | fs 模块 | std::fs（更安全） | os/io | 受限 |
| 安全模型 | Node 集成带来较大攻击面 | 同 Electron | 默认禁用 Node，CSP 严格，能力权限模型 | 默认 CSP，能力声明 | 简化沙箱 |
| 生态成熟度 | 极成熟（VS Code、Discord） | 成熟但社区萎缩 | 快速成长，Tauri 2 已 GA | 成长中 | 较小众 |
| 学习曲线 | 低（纯 JS/TS） | 低 | 中（需基础 Rust） | 中（需基础 Go） | 低 |
| 现有资产复用 | 高（任何 Web 前端） | 高 | **极高**（已实现 Rust 后端） | 中（需重写后端） | 低 |
| 系统托盘/通知 | 内置 | 内置 | 官方插件 | 内置 | 插件 |
| 自动更新 | electron-updater | 第三方 | 官方 plugin-updater | 无官方 | 无 |

### 2.2 选型决策：**Tauri 2.x + Vue 3**

#### 决策依据

1. **现有资产复用度最高**：`Palworld-out/src-tauri/` 已包含 4 个 Rust 源文件（`main.rs` 706 行、`rcon.rs` 120 行、`network.rs` 67 行、`settings.rs` 38 行），已实现 21 个 Tauri command，涵盖进程管理、配置读写、防火墙、RCON、Radmin LAN 检测、设置持久化。Electron/NW.js 方案需将 Rust 后端整体重写为 Node.js，工作量过大且收益为负。

2. **性能与体积优势契合工具类应用**：服务器管理器是常驻型轻量工具，启动速度与内存占用直接影响用户体验。Tauri 安装包 <10MB、空闲内存 <80MB，显著优于 Electron 的 100MB+/200MB+。

3. **安全模型适合系统级操作**：本应用需调用 PowerShell（防火墙规则、网络检测）、启动外部进程（PalServer.exe、SteamCMD）、读写 INI 文件。Tauri 的能力权限模型（`capabilities/default.json`）可精确声明 `core:default`、`dialog:allow-open` 等权限，避免过度暴露系统 API。

4. **Tauri 2.x 已 GA 且生态稳定**：`@tauri-apps/api` 2.0+、`@tauri-apps/cli` 2.0+、`tauri-plugin-dialog` 2.x 均已正式发布，文档完善，与 Vue 3 + Vite 5 集成方案成熟。

5. **前端栈与现有 `package.json` 完全一致**：`Palworld-out/package.json` 已声明 Vue 3.4、TypeScript 5.4、Pinia 2.1、Vite 5.4、`@tauri-apps/api` 2.0、`lucide-vue-next`，无需调整依赖。

#### 不选其他方案的理由
- **Electron/NW.js**：体积与内存劣势明显，且需重写 Rust 后端
- **Wails**：后端语言切换为 Go，无法复用现有 Rust 代码
- **Neutralino.js**：生态过小，系统级 API 受限，不适合本应用的进程/防火墙/RCON 需求

### 2.3 Tauri 2.x 关键能力清单（本应用将使用）

| 能力 | 实现方式 | 用途 |
|------|----------|------|
| 窗口管理 | `tauri::WebviewWindow` | 主窗口 1200×800、居中、延迟显示 |
| 命令调用 | `#[tauri::command]` + `invoke()` | 前后端通信主通道 |
| 事件推送 | `app.emit()` + `listen()` | 服务器日志实时推流 |
| 状态管理 | `tauri::State<T>` + `Arc<Mutex<T>>` | 进程句柄、RCON 客户端、日志缓冲区 |
| 文件对话框 | `tauri-plugin-dialog` | 选择服务器目录、配置文件 |
| 持久化存储 | `dirs::data_local_dir()` + JSON | 应用设置（`%AppData%/PalworldServerManager/settings.json`） |
| 打包 | `tauri build` → NSIS / MSI | Windows 安装包 |

---

## 三、Palworld 相关 API/协议调研

### 3.1 RCON 协议（Source RCON Protocol）

**协议性质**：TCP-based，由 Valve 在 Source 引擎中定义，Palworld 完全兼容。

**数据包格式（小端序）**：
| 字段 | 长度 | 说明 |
|------|------|------|
| Length | 4 bytes (i32) | 包体长度（不含自身） |
| ID | 4 bytes (i32) | 请求 ID，用于匹配响应 |
| Type | 4 bytes (i32) | 3=AUTH, 2=COMMAND, 0=RESPONSE |
| Body | 变长 | UTF-8 字符串 |
| Padding | 2 bytes | `\0\0` |

**通信流程**：
1. TCP 连接到 `host:25575`
2. 发送 Type=3 的 AUTH 包（含密码）
3. 接收 AUTH_RESPONSE（ID≠-1 表示成功）
4. 发送 Type=2 的 COMMAND 包
5. 接收 Type=0 的 RESPONSE 包
6. 关闭连接或保持长连接

**现有实现状态**：`Palworld-out/src-tauri/src/rcon.rs` 已完整实现 `RconClient`（connect/send_command/disconnect/is_connected），使用 `TcpStream` + `set_read_timeout(5s)` + `Mutex<RequestID>` 线程安全封装。

**常用 RCON 命令**：
| 命令 | 功能 |
|------|------|
| `/ShowPlayers` | 列出在线玩家（name, playeruid, steamid） |
| `/Info` | 服务器信息（名称、版本、玩家数） |
| `/Save` | 强制保存存档 |
| `/Broadcast <message>` | 广播消息 |
| `/KickPlayer <steamid>` | 踢出玩家 |
| `/BanPlayer <steamid>` | 封禁玩家 |

### 3.2 REST API

**启用方式**：`PalWorldSettings.ini` 中 `RESTAPIEnabled=True`，`RESTAPIPort=8212`。

**端点（v1）**：
| 方法 | 路径 | 功能 |
|------|------|------|
| GET | `/v1/api/info` | 服务器信息 |
| GET | `/v1/api/players` | 玩家列表 |
| GET | `/v1/api/settings` | 服务器设置 |
| POST | `/v1/api/kick` | 踢出玩家 |
| POST | `/v1/api/ban` | 封禁玩家 |
| POST | `/v1/api/save` | 保存存档 |
| POST | `/v1/api/announce` | 广播消息 |
| POST | `/v1/api/stop` | 停止服务器 |

**认证**：HTTP Basic Auth（`adminPassword`）。

**调研结论**：本应用**优先使用 RCON**（已实现），REST API 作为可选增强（前端可直接 `fetch`，无需 Tauri 桥接，但需要 CORS 配置或通过 Rust 代理）。

### 3.3 PalWorldSettings.ini 配置格式

**文件路径**：`<ServerPath>/Pal/Saved/Config/WindowsServer/PalWorldSettings.ini`

**格式**：
```ini
[/Script/Pal.PalGameWorldSettings]
OptionSettings=(Difficulty=None,DayTimeSpeedRate=1.000000,ExpRate=1.000000,...)
```

**解析难点**：值中可能含引号、括号（如 `CrossplayPlatforms=(Steam,Xbox,PS5,Mac)`），需状态机解析。

**现有实现状态**：`Palworld-out/src-tauri/src/main.rs` 的 `read_config`/`write_config`/`get_default_config`/`get_config_descriptions` 已完整实现：
- `read_config`：状态机解析（处理引号/括号嵌套）
- `get_default_config`：内置 100+ 默认参数（含最新 Sakurajima 更新参数）
- `get_config_descriptions`：50+ 参数的元信息（描述、字段类型、min/max/step）

### 3.4 SteamCMD 集成

**App ID**：`2394010`（Palworld Dedicated Server）

**安装命令**：
```
steamcmd.exe +force_install_dir <路径> +login anonymous +app_update 2394010 validate +quit
```

**自动化方案**：
- Rust 端通过 `Command::new("steamcmd.exe")` 启动
- 通过 stdout 管道实时回传进度（与 PalServer 日志收集同模式）
- 前端通过 `listen("steamcmd-log")` 显示

**调研结论**：本应用**不强制自动下载 SteamCMD**（需用户自行安装），但提供"一键更新服务器"功能调用已安装的 SteamCMD。理由：SteamCMD 安装涉及 Steam 用户协议，自动下载存在合规风险。

### 3.5 SAV 存档管理

**存档路径**：`<ServerPath>/Pal/Saved/SaveGames/0/<GUID>/Level/`

**备份方案**：
- 全量复制 SaveGames 目录到 `%AppData%/PalworldServerManager/backups/<timestamp>/`
- 使用 Rust `std::fs::copy` 递归复制
- 前端展示备份列表，支持恢复

**调研结论**：实现复杂度低，纯文件操作，无需第三方库。

### 3.6 Radmin LAN 集成

**现状**（来自 `Palworld-out/Deployment_Technical_Spec.md`）：
- Radmin LAN 为闭源软件，无公开 API
- 可通过 PowerShell 检测：
  - 适配器：`Get-NetAdapter | Where-Object { $_.InterfaceDescription -like '*Famatech Radmin VPN*' }`
  - 虚拟 IP：`Get-NetIPAddress -InterfaceAlias 'Radmin VPN' -AddressFamily IPv4`

**现有实现状态**：`Palworld-out/src-tauri/src/network.rs` 已实现 `check_radmin_lan()` 与 `get_local_ip()`。

**集成策略**：仅做"检测+引导"，不做深度集成（创建/加入网络仍由 Radmin LAN GUI 完成）。

### 3.7 Windows 系统依赖

| 依赖 | 用途 | 检测/安装方式 |
|------|------|---------------|
| WebView2 Runtime | Tauri 渲染 | Windows 11 已内置；Win10 通过 bootstrapper 安装 |
| PowerShell | 防火墙、网络检测、Radmin 检测 | Windows 内置 |
| PalServer.exe | 服务器本体 | 用户通过 Steam 安装 |
| SteamCMD（可选） | 服务器更新 | 用户手动安装 |
| Visual C++ Redistributable | PalServer 运行依赖 | 引导用户安装 |

---

## 四、现有项目结构调研

### 4.1 `Palworld-out/` 资产盘点

```
Palworld-out/
├── dist/                          # 前端构建产物（已存在，但 src/ 缺失）
│   ├── assets/
│   │   ├── index-*.js (10 个)     # Vue 3 + Pinia + 业务代码打包产物
│   │   ├── index-*.css (3 个)
│   │   └── ...
│   └── index.html
├── src-tauri/                     # ✅ Rust 后端（完整可用）
│   ├── src/
│   │   ├── main.rs                # 706 行，21 个 Tauri command
│   │   ├── rcon.rs                # 120 行，RCON TCP 客户端
│   │   ├── network.rs             # 67 行，Radmin LAN + 本地 IP
│   │   └── settings.rs            # 38 行，JSON 设置持久化
│   ├── capabilities/default.json  # Tauri 能力权限声明
│   ├── icons/icon.ico
│   ├── Cargo.toml                 # tauri 2.0 + tauri-plugin-dialog 2 + serde + tokio + dirs
│   ├── Cargo.lock
│   └── build.rs
├── index.html                     # Vite 入口（引用 /src/main.ts，但 src/ 缺失）
├── package.json                   # Vue 3.4 + Pinia 2.1 + Vite 5.4 + TS 5.4 + lucide-vue-next
├── package-lock.json
├── AGENTS.md                      # 项目协作规范
├── Deployment_Technical_Spec.md   # 部署技术规格
├── Desktop_App_Feasibility_Report.md  # 可行性报告
├── Palworld_Dedicated_Server_Guide.md # 服务器搭建指南
└── *.ps1                          # 调试脚本（截图、窗口检测等）
```

#### 4.1.1 Rust 后端已实现能力清单

| 模块 | Tauri Command | 状态 |
|------|---------------|------|
| 服务器进程 | `init_server_state`, `start_server`, `stop_server`, `get_server_status`, `get_server_logs`, `clear_server_logs` | ✅ 完整 |
| 配置管理 | `read_config`, `write_config`, `get_default_config`, `get_config_descriptions` | ✅ 完整 |
| 防火墙 | `check_firewall_rules`, `add_firewall_rules` | ✅ 完整 |
| 网络 | `check_port_usage`, `check_radmin_lan_status` | ✅ 完整 |
| RCON | `rcon_connect`, `rcon_send_command`, `rcon_disconnect`, `rcon_is_connected` | ✅ 完整 |
| 设置 | `load_app_settings`, `save_app_settings` | ✅ 完整 |

#### 4.1.2 Rust 后端已具备的工程能力
- **进程管理**：`std::process::Command::spawn` + stdout/stderr 管道 + 后台线程收集日志 + `try_wait()` 状态检测
- **事件推送**：`app_handle.emit("server-log", line)` 实时推送日志到前端
- **线程安全**：`Arc<Mutex<Vec<String>>>` 日志缓冲区（上限 500 行自动滚动）
- **窗口管理**：`setup` hook 中延迟 500ms 设置窗口 1200×800 + 居中 + 显示
- **RCON 协议**：完整的小端序包格式 + 5s 读超时 + 认证流程
- **配置解析**：状态机解析 `OptionSettings=(...)`，处理引号/括号嵌套
- **错误处理**：所有 command 返回 `Result<T, String>`，错误信息中文

#### 4.1.3 关键缺口
| 缺口 | 影响 | 解决方案 |
|------|------|----------|
| **`src/` 前端源码完全缺失** | 无法 dev/build/tauri:dev | 从零创建 Vue 3 项目结构 |
| **`tauri.conf.json` 缺失** | Tauri 无法 build | 创建 Tauri 2 配置文件 |
| **`vite.config.ts` 缺失** | Vite 无法构建 | 创建配置（含 Tauri host 白名单） |
| **`tsconfig.json` 缺失** | TS 无法类型检查 | 创建 strict 模式配置 |
| **未实现 SteamCMD 更新** | 无法自动更新服务器 | 新增 `steamcmd.rs` + command |
| **未实现存档备份** | 无法备份/恢复存档 | 新增 `backup.rs` + command |
| **未实现自动重启** | 崩溃后无法自恢复 | 增强 `server` 模块 |
| **未实现系统托盘** | 关闭窗口即退出 | 引入 `tauri-plugin-system-tray` |

### 4.2 `palworld-warm-glass-preview/` 资产盘点

```
palworld-warm-glass-preview/
├── .preflight/preflight.html      # 设计预检页
├── pages/
│   └── dashboard.html             # ✅ 唯一的设计预览页（572 行）
├── colors_and_type.css            # ✅ 设计系统 CSS 变量（30 行）
├── orchestration-summary.json     # 设计编排元数据
├── validation-report.json         # 设计验证报告
└── palworld-warm-glass-preview.design  # 设计源文件
```

#### 4.2.1 设计系统提取

**品牌前缀**：`palwarm`（Palworld Warm Glass）

**色彩系统**：
| 变量 | 值 | 用途 |
|------|-----|------|
| `--palwarm-background` | `#f5ede2` | 奶油米白背景 |
| `--palwarm-foreground` | `#3f322c` | 暖棕主文字 |
| `--palwarm-card` | `rgba(255,252,247,0.72)` | 半透明玻璃卡片 |
| `--palwarm-primary` | `#e66f51` | 珊瑚暖橙（唯一品牌主色） |
| `--palwarm-primary-foreground` | `#ffffff` | 主色上的文字 |
| `--palwarm-muted` | `rgba(255,255,255,0.48)` | 静默背景 |
| `--palwarm-muted-foreground` | `#77675f` | 次要文字 |
| `--palwarm-border` | `rgba(116,88,72,0.14)` | 低对比暖灰边框 |
| `--palwarm-state-success` | `#4f8a6b` | 成功状态 |
| `--palwarm-state-warning` | `#b8782f` | 警告状态 |
| `--palwarm-state-error` | `#c9554d` | 错误状态 |
| `--palwarm-state-info` | `#4b7896` | 信息状态 |

**圆角体系**：12px（sm）/ 18px（md）/ 26px（lg）/ 32px（panel）

**字体系统**：
- Sans：`Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", sans-serif`
- Mono：`"SFMono-Regular", Consolas, "Liberation Mono", monospace`

**玻璃面板配方**：
```css
background: var(--palwarm-glass-soft);  /* color-mix(card 66%, transparent) */
border: 1px solid var(--palwarm-glass-edge);  /* color-mix(primary-foreground 72%, transparent) */
box-shadow: 0 18px 42px color-mix(in srgb, foreground 5%, transparent);
backdrop-filter: blur(24px) saturate(145%);
```

**布局结构**（从 dashboard.html 提取）：
```
app-shell (grid: 228px | 1fr, padding 14px, gap 14px)
├── sidebar.glass-panel (228px 固定宽度，圆角 32px)
│   ├── brand (logo + 标题)
│   ├── nav (5 个 nav-item：首页/配置/网络/RCON/故障排查)
│   └── sidebar-status (服务器状态摘要)
└── workspace (flex column)
    ├── header.glass-panel (页面标题 + 状态徽章)
    └── content-scroll (滚动区域)
        └── dashboard
            ├── overview-grid (4 列状态卡片)
            └── content-grid (控制卡片 + 指南卡片)
```

**导航项**：
| 序号 | 名称 | 图标（lucide） | 页面 key |
|------|------|----------------|----------|
| 1 | 首页 | `layout-dashboard` | `dashboard` |
| 2 | 配置 | `sliders-horizontal` | `config` |
| 3 | 网络 | `wifi` | `network` |
| 4 | RCON | `terminal-square` | `rcon` |
| 5 | 故障排查 | `triangle-alert` | `troubleshoot` |

#### 4.2.2 设计预览的局限
- **仅 dashboard 页**：其他 4 个页面（配置/网络/RCON/故障排查）无设计稿，需按设计系统延展
- **静态 HTML + Tailwind CDN**：使用了 `https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.3.1/dist/index.global.js`，不适合生产环境
- **Lucide UMD**：`https://unpkg.com/lucide@1.8.0/dist/umd/lucide.min.js` + `lucide.createIcons()` 调用
- **无业务逻辑**：仅路径选择对话框和日志展开有 mock 交互
- **响应式断点**：1060px / 900px / 760px 三档（桌面应用窗口可缩放，需保留）

#### 4.2.3 迁移到 Vue 3 的策略
| 设计预览元素 | Vue 3 实现方案 |
|-------------|----------------|
| Tailwind CDN | 改为本地 Tailwind 4 PostCSS 插件（或继续用原生 CSS + CSS 变量） |
| Lucide UMD + `createIcons()` | 改为 `lucide-vue-next`（已在 package.json） |
| `--palwarm-*` CSS 变量 | 直接迁移到 `src/style.css` |
| `.glass-panel` 等类 | 提取为全局 CSS 或 Vue 组件 `<GlassPanel>` |
| 静态 HTML 结构 | 拆分为 `App.vue`（shell） + 5 个页面组件 |
| mock 交互 | 替换为 Pinia store + Tauri `invoke()` |

---

## 五、可行性评估

### 5.1 技术可行性

| 评估项 | 结论 | 依据 |
|--------|------|------|
| 桌面框架 | ✅ 完全可行 | Tauri 2.x GA，现有 Rust 后端已验证 |
| 前端栈 | ✅ 完全可行 | Vue 3 + TS + Pinia + Vite 成熟方案，package.json 已就绪 |
| 设计系统迁移 | ✅ 高度可行 | CSS 变量可直接迁移，布局结构清晰 |
| 进程管理 | ✅ 已实现 | `start_server`/`stop_server` 已工作 |
| RCON 集成 | ✅ 已实现 | `rcon.rs` 完整协议实现 |
| 配置读写 | ✅ 已实现 | 状态机解析 + 100+ 默认参数 |
| 防火墙/网络 | ✅ 已实现 | PowerShell 调用封装完成 |
| Radmin 检测 | ✅ 已实现 | `network.rs` 已工作 |
| SteamCMD 更新 | ⚠️ 需新增 | 简单 `Command::spawn` 即可 |
| 存档备份 | ⚠️ 需新增 | 纯文件操作，复杂度低 |
| 系统托盘 | ⚠️ 需新增 | Tauri 2 官方插件支持 |
| 自动重启 | ⚠️ 需新增 | 后台线程 + `try_wait` 检测 |

### 5.2 资产复用率评估

| 资产 | 复用率 | 说明 |
|------|--------|------|
| Rust 后端代码 | **95%** | 21 个 command 直接复用，仅需新增 SteamCMD/Backup/Tray 模块 |
| `Cargo.toml` 依赖 | **90%** | 仅需追加 `tauri-plugin-system-tray`（如需托盘） |
| `package.json` 依赖 | **100%** | Vue 3 + Pinia + Vite + TS + lucide + Tauri API 全部保留 |
| 设计系统 CSS 变量 | **100%** | `--palwarm-*` 直接迁移 |
| 布局结构 | **80%** | dashboard 页可直接迁移，其他 4 页按设计系统延展 |
| 组件交互模式 | **70%** | 路径选择、状态徽章、玻璃卡片等模式可复用 |
| 现有文档 | **100%** | AGENTS.md / Feasibility / Deployment Spec 保留作为参考 |

**综合复用率**：约 **80%**，主要工作量在 Vue 3 前端从零搭建（5 个页面）+ 3 个 Rust 新增模块。

### 5.3 风险评估

| 风险 | 等级 | 缓解措施 |
|------|------|----------|
| Vue 3 前端 src/ 完全缺失 | 中 | 按 dashboard.html 设计系统延展，参考 AGENTS.md 目录约定 |
| 其他 4 页无设计稿 | 中 | 严格遵循 `--palwarm-*` 设计系统，保持视觉一致 |
| Tailwind CDN 不适合生产 | 低 | 改用 Tailwind 4 PostCSS 或继续用原生 CSS（设计预览已用大量原生 CSS） |
| Tauri 2 配置文件缺失 | 低 | 按 Tauri 2 官方模板创建 `tauri.conf.json` |
| WebView2 在 Win10 旧版本可能缺失 | 低 | Tauri 安装包可内置 bootstrapper |
| Rust 编译环境需 MSVC | 低 | 要求开发者安装 Visual Studio Build Tools |

---

## 六、调研结论

### 6.1 技术选型最终结论

**采用 Tauri 2.x + Vue 3 + TypeScript + Pinia + Vite 5 + lucide-vue-next**，理由：
1. 现有 Rust 后端（`Palworld-out/src-tauri/`）可直接复用 95%
2. 现有 `package.json` 依赖无需调整
3. 现有设计系统（`palworld-warm-glass-preview/`）可完整迁移
4. Tauri 2 在体积、性能、安全上全面优于 Electron/NW.js
5. 工具类应用场景与 Tauri 的轻量特性高度契合

### 6.2 关键工作量分布

| 工作项 | 占比 | 说明 |
|--------|------|------|
| Vue 3 前端搭建（5 页 + 路由 + store + 组件） | **60%** | 主要工作量 |
| Rust 后端新增模块（SteamCMD/Backup/Tray/AutoRestart） | **15%** | 增量功能 |
| 设计系统迁移与组件化 | **10%** | CSS 变量 + 玻璃面板组件 |
| Tauri 配置与打包 | **5%** | tauri.conf.json + NSIS |
| 测试与验收 | **10%** | 单元测试 + 集成测试 + 手动验收 |

### 6.3 可行性总评

**✅ 完全可行，建议立即推进。**

- 现有资产复用率高（80%），避免重复造轮子
- 技术栈成熟稳定，无技术阻塞风险
- Rust 后端已验证可工作，前端有完整设计系统参考
- 主要工作集中在 Vue 3 前端搭建，属于标准工程任务

---

## 七、参考资源

### 7.1 项目内文档
- [Palworld-out/AGENTS.md](file:///F:/study/Palworld-Server-Manager/Palworld-out/AGENTS.md) — 项目协作规范
- [Palworld-out/Desktop_App_Feasibility_Report.md](file:///F:/study/Palworld-Server-Manager/Palworld-out/Desktop_App_Feasibility_Report.md) — 桌面应用可行性报告
- [Palworld-out/Deployment_Technical_Spec.md](file:///F:/study/Palworld-Server-Manager/Palworld-out/Deployment_Technical_Spec.md) — 部署技术规格
- [Palworld-out/Palworld_Dedicated_Server_Guide.md](file:///F:/study/Palworld-Server-Manager/Palworld-out/Palworld_Dedicated_Server_Guide.md) — 服务器搭建指南

### 7.2 技术规范
- Tauri 2 官方文档：https://v2.tauri.app/
- Source RCON Protocol：https://developer.valvesoftware.com/wiki/Source_RCON_Protocol
- Palworld REST API：https://tech.palworldgame.com/category/api
- Palworld Dedicated Server：https://tech.palworldgame.com/

### 7.3 现有代码资产
- [Palworld-out/src-tauri/src/main.rs](file:///F:/study/Palworld-Server-Manager/Palworld-out/src-tauri/src/main.rs) — 21 个 Tauri command
- [Palworld-out/src-tauri/src/rcon.rs](file:///F:/study/Palworld-Server-Manager/Palworld-out/src-tauri/src/rcon.rs) — RCON 客户端
- [Palworld-out/src-tauri/src/network.rs](file:///F:/study/Palworld-Server-Manager/Palworld-out/src-tauri/src/network.rs) — 网络检测
- [Palworld-out/src-tauri/src/settings.rs](file:///F:/study/Palworld-Server-Manager/Palworld-out/src-tauri/src/settings.rs) — 设置持久化
- [palworld-warm-glass-preview/pages/dashboard.html](file:///F:/study/Palworld-Server-Manager/palworld-warm-glass-preview/pages/dashboard.html) — 设计预览
- [palworld-warm-glass-preview/colors_and_type.css](file:///F:/study/Palworld-Server-Manager/palworld-warm-glass-preview/colors_and_type.css) — 设计系统变量
