# 02 · 需求规格说明书

> Palworld Server Manager 桌面应用 — 需求规格说明书（SRS）
> 版本：v1.0 · 编制日期：2026-07-20

---

## 一、项目概述

### 1.1 项目定位
Palworld Server Manager 是一款面向 Windows 平台的幻兽帕鲁（Palworld）专用服务器**一站式 GUI 管理桌面应用**，目标用户为本地开服的玩家与小型社区服主。基于 Tauri 2 + Vue 3 构建，覆盖从搭建、配置、启动、监控到运维的完整生命周期。

### 1.2 项目目标
| 目标维度 | 描述 |
|----------|------|
| 功能目标 | 5 大功能模块（首页/配置/网络/RCON/故障排查），覆盖服务器管理全流程 |
| 体验目标 | 安装包 <15MB，冷启动 <3s，空闲内存 <100MB |
| 工程目标 | 复用现有 Rust 后端 95%，前端从设计系统延展 |
| 部署目标 | 输出 NSIS 安装包，目标部署目录 `F:\study\Palworld-Server-Manager\Palworld` |

### 1.3 用户角色
| 角色 | 描述 | 核心诉求 |
|------|------|----------|
| 本地开服玩家 | 在自己电脑上开服给好友玩 | 一键搭建、零命令行、可视化配置 |
| 小型社区服主 | 管理 10–32 人小服 | 远程管理（RCON）、玩家管理、定时备份 |
| 自托管技术爱好者 | 关注参数调优与性能 | 全参数可视化、性能监控、自动重启 |

---

## 二、功能需求

### 2.1 功能模块总览

```
Palworld Server Manager
├── M1 首页（Dashboard）        — P0
├── M2 配置（Config）           — P0
├── M3 网络（Network）          — P0
├── M4 RCON 控制台（Rcon）      — P1
├── M5 故障排查（Troubleshoot） — P1
├── M6 存档备份（Backup）       — P2
├── M7 SteamCMD 更新            — P2
├── M8 系统托盘与常驻           — P2
└── M9 自动重启                 — P2
```

### 2.2 M1 · 首页（Dashboard）— P0

#### 2.2.1 功能描述
服务器状态总览与一键启停控制入口。

#### 2.2.2 功能项列表
| 编号 | 功能项 | 描述 | 优先级 | 状态 |
|------|--------|------|--------|------|
| M1-F1 | 状态概览卡片 | 4 列卡片：服务器状态/服务器路径/防火墙状态/连接信息 | P0 | ✅ 已实现 |
| M1-F2 | 服务器控制 | 启动/停止按钮 + 路径选择对话框 | P0 | ✅ 已实现 |
| M1-F3 | 实时日志面板 | 可折叠的日志流（最多 500 行，自动滚动） | P0 | ✅ 已实现 |
| M1-F4 | 快速搭建指南 | 4 步引导卡片（下载/路径/配置/启动） | P1 | ✅ 已实现 |
| M1-F5 | 状态自动刷新 | 5 秒轮询 `get_server_status` | P0 | ✅ 已实现 |
| M1-F6 | 日志实时推送 | 监听 `server-log` 事件追加显示 | P0 | ✅ 已实现 |
| M1-F7 | 一键复制连接地址 | 复制 `IP:8211` 到剪贴板 | P1 | ✅ 已实现 |

#### 2.2.3 交互流程
```
用户打开应用
  ├── 应用初始化 → 调用 init_server_state + load_app_settings
  ├── 显示上次保存的服务器路径
  ├── 用户点击"选择路径" → 弹出 Tauri 文件对话框 → 选择目录 → 保存到 settings
  ├── 用户点击"启动服务器"
  │   ├── 调用 start_server(path)
  │   ├── 按钮置灰，状态变为"启动中"
  │   ├── 监听 server-log 事件，追加到日志面板
  │   └── 启动成功后状态卡片更新为"运行中" + 显示 PID
  ├── 用户点击"停止服务器"
  │   ├── 调用 stop_server()
  │   └── 状态卡片更新为"已停止"
  └── 用户展开/折叠日志面板
```

### 2.3 M2 · 配置（Config）— P0

#### 2.3.1 功能描述
可视化编辑 `PalWorldSettings.ini` 的 100+ 参数。

#### 2.3.2 功能项列表
| 编号 | 功能项 | 描述 | 优先级 | 状态 |
|------|--------|------|--------|------|
| M2-F1 | 参数分组导航 | 按类别分组：基础/世界规则/PvP/玩家/帕鲁/建筑/服务器/网络/RCON | P0 | ✅ 已实现 |
| M2-F2 | 字段类型渲染 | range（滑块）/toggle（开关）/select（下拉）/text（输入）/number（数字） | P0 | ✅ 已实现 |
| M2-F3 | 参数描述展示 | 鼠标悬停显示中文描述 + min/max/step 提示 | P0 | ✅ 已实现 |
| M2-F4 | 加载现有配置 | 调用 `read_config(path)` 读取并填充表单 | P0 | ✅ 已实现 |
| M2-F5 | 恢复默认值 | 调用 `get_default_config` 一键重置 | P0 | ✅ 已实现 |
| M2-F6 | 保存配置 | 调用 `write_config(path, config)` 写入 INI | P0 | ✅ 已实现 |
| M2-F7 | 配置路径自动定位 | 基于 server_path 拼接 `Pal/Saved/Config/WindowsServer/PalWorldSettings.ini` | P0 | ✅ 已实现 |
| M2-F8 | 预设方案 | 快速套用预设：默认/PvE 友好/PvP 竞技/速通（`list_presets` / `apply_preset`） | P1 | ✅ 已实现 |
| M2-F9 | 修改前对比 | 高亮未保存的修改项（`DiffIndicator.vue`） | P1 | ✅ 已实现 |
| M2-F10 | 配置备份 | 保存前自动备份上一版到 `%AppData%/.../config-backups/`（`write_config` 自动备份 + `list_config_backups` / `restore_config_backup`） | P1 | ✅ 已实现 |

#### 2.3.3 交互流程
```
用户切换到配置页
  ├── 自动定位配置文件路径 → 调用 read_config
  ├── 加载失败 → 提示"配置文件不存在，是否使用默认值创建"
  ├── 加载成功 → 按分组渲染表单
  ├── 用户编辑参数 → 实时标记为"已修改"
  ├── 用户点击"恢复默认" → 二次确认 → 调用 get_default_config
  ├── 用户点击"保存"
  │   ├── 自动备份当前配置（若开启）
  │   ├── 调用 write_config
  │   └── 成功提示 + 清除"已修改"标记
  └── 用户切换到其他页 → 若有未保存修改 → 二次确认
```

### 2.4 M3 · 网络（Network）— P0

#### 2.4.1 功能描述
防火墙规则配置、端口占用检测、Radmin LAN 检测、连接地址展示。

#### 2.4.2 功能项列表
| 编号 | 功能项 | 描述 | 优先级 | 状态 |
|------|--------|------|--------|------|
| M3-F1 | 防火墙状态检测 | 调用 `check_firewall_rules` 显示 8211/27015/25575 状态 | P0 | ✅ 已实现 |
| M3-F2 | 一键放行端口 | 调用 `add_firewall_rules` 添加入站规则 | P0 | ✅ 已实现 |
| M3-F3 | 端口占用检测 | 调用 `check_port_usage(port)` 显示占用进程 | P0 | ✅ 已实现 |
| M3-F4 | Radmin LAN 检测 | 调用 `check_radmin_lan_status` 显示安装状态/虚拟 IP | P0 | ✅ 已实现 |
| M3-F5 | 连接地址展示 | 显示本机 IP/Radmin IP/公网 IP（手动输入）三档 | P0 | ✅ 已实现 |
| M3-F6 | 一键复制地址 | 复制 `IP:8211` 到剪贴板（`@tauri-apps/plugin-clipboard-manager`） | P1 | ✅ 已实现 |
| M3-F7 | Radmin 安装引导 | 未安装时显示下载链接 + 安装步骤 | P1 | ✅ 已实现 |
| M3-F8 | 联机向导 | 4 步引导：下载 Radmin → 创建/加入网络 → 重启 Radmin → 复制地址 | P1 | ✅ 已实现 |

### 2.5 M4 · RCON 控制台（Rcon）— P1

#### 2.5.1 功能描述
内置 RCON 客户端，支持玩家管理、命令执行、服务器信息查看。

#### 2.5.2 功能项列表
| 编号 | 功能项 | 描述 | 优先级 | 状态 |
|------|--------|------|--------|------|
| M4-F1 | 连接配置 | 主机/端口/密码表单（默认 127.0.0.1:25575） | P1 | ✅ 已实现 |
| M4-F2 | 连接状态指示 | 显示已连接/未连接 + 重连按钮 | P1 | ✅ 已实现 |
| M4-F3 | 快捷命令按钮 | `/Save` `/Info` `/ShowPlayers` `/Broadcast` | P1 | ✅ 已实现 |
| M4-F4 | 玩家列表表格 | 解析 `/ShowPlayers` 输出 → 表格展示 name/steamid（`PlayerTable.vue`） | P1 | ✅ 已实现 |
| M4-F5 | 玩家操作 | 踢出/封禁（调用 `/KickPlayer` `/BanPlayer`，`rconStore.kickPlayer/banPlayer`） | P1 | ✅ 已实现 |
| M4-F6 | 自定义命令输入 | 文本框 + 回车发送 + 历史记录（↑↓ 翻页，`commandHistory` 持久化到 localStorage） | P1 | ✅ 已实现 |
| M4-F7 | 响应展示 | 滚动文本区域显示命令输出 | P1 | ✅ 已实现 |
| M4-F8 | 广播消息 | 输入框 + 发送按钮（`/Broadcast <msg>`） | P2 | 未实现 |
| M4-F9 | 配置持久化 | RCON 主机/端口/密码保存到 AppSettings（连接成功后自动保存） | P1 | ✅ 已实现 |

#### 2.5.3 交互流程
```
用户切换到 RCON 页
  ├── 从 settings 加载上次 RCON 配置
  ├── 用户点击"连接"
  │   ├── 调用 rcon_connect(host, port, password)
  │   ├── 成功 → 状态指示变绿 + 自动调用 /ShowPlayers
  │   └── 失败 → 错误提示（密码错误/连接超时）
  ├── 用户点击快捷命令
  │   ├── 调用 rcon_send_command
  │   └── 响应追加到输出区
  ├── 用户输入自定义命令 + 回车
  │   ├── 调用 rcon_send_command
  │   ├── 命令存入历史
  │   └── 响应追加到输出区
  ├── 用户在玩家表格点击"踢出"
  │   ├── 二次确认 → 调用 rcon_send_command("/KickPlayer <steamid>")
  │   └── 刷新玩家列表
  └── 用户离开页面 → 保持连接（不主动断开）
```

### 2.6 M5 · 故障排查（Troubleshoot）— P1

#### 2.6.1 功能描述
常见故障的诊断与修复指引。

#### 2.6.2 功能项列表
| 编号 | 功能项 | 描述 | 优先级 | 状态 |
|------|--------|------|--------|------|
| M5-F1 | 故障分类 | 6 类：启动失败/连接不上/卡顿/崩溃/RCON 失败/存档损坏 | P1 | ✅ 已实现 |
| M5-F2 | 自动诊断 | 一键诊断：端口/防火墙/进程/路径/Radmin（并行 5 项检测，`DiagnosticReport.vue`） | P1 | ✅ 已实现 |
| M5-F3 | 解决方案卡片 | 每个故障对应可执行的修复步骤 | P1 | ✅ 已实现 |
| M5-F4 | 日志导出 | 导出最近 500 行服务器日志到文件（`export_server_logs` command + `dialog.save()`） | P1 | ✅ 已实现 |
| M5-F5 | VC++/DirectX 检测 | 检测运行依赖是否安装 + 下载链接 | P2 | 未实现 |
| M5-F6 | 配置校验 | 调用 `read_config` 检查参数是否合法 | P2 | 未实现 |

### 2.7 M6 · 存档备份（Backup）— P2

| 编号 | 功能项 | 描述 | 优先级 |
|------|--------|------|--------|
| M6-F1 | 一键备份 | 复制 SaveGames 到 `%AppData%/.../backups/<timestamp>/` | P2 |
| M6-F2 | 备份列表 | 时间戳 + 大小 + 恢复按钮 + 删除按钮 | P2 |
| M6-F3 | 恢复备份 | 二次确认 → 覆盖当前存档 | P2 |
| M6-F4 | 自动备份 | 启用/禁用 + 间隔（30min/1h/6h） | P2 |
| M6-F5 | 备份保留策略 | 最多保留 N 份（默认 20） | P2 |

### 2.8 M7 · SteamCMD 更新 — P2

| 编号 | 功能项 | 描述 | 优先级 |
|------|--------|------|--------|
| M7-F1 | SteamCMD 路径配置 | 设置 steamcmd.exe 路径 | P2 |
| M7-F2 | 一键更新服务器 | 调用 `steamcmd +app_update 2394010 validate` | P2 |
| M7-F3 | 更新日志实时显示 | 监听 `steamcmd-log` 事件 | P2 |
| M7-F4 | 更新前自动停止 | 服务器运行中则先停止 | P2 |

### 2.9 M8 · 系统托盘与常驻 — P2

| 编号 | 功能项 | 描述 | 优先级 |
|------|--------|------|--------|
| M8-F1 | 关闭最小化到托盘 | 关闭按钮 → 最小化到系统托盘 | P2 |
| M8-F2 | 托盘菜单 | 显示/退出/启动服务器/停止服务器 | P2 |
| M8-F3 | 开机自启 | 选项：开机自动启动应用 | P2 |
| M8-F4 | 服务器开机自启 | 选项：应用启动后自动启动服务器 | P2 |

### 2.10 M9 · 自动重启 — P2

| 编号 | 功能项 | 描述 | 优先级 |
|------|--------|------|--------|
| M9-F1 | 崩溃自动重启 | 检测进程退出 → 自动重启（最多 3 次/小时） | P2 |
| M9-F2 | 定时重启 | 间隔（6h/12h/24h）+ 启用开关 | P2 |
| M9-F3 | 重启通知 | 重启前通过 RCON `/Broadcast` 通知玩家 | P2 |

---

## 三、非功能需求

### 3.1 性能需求

| 编号 | 指标 | 目标值 | 验收方式 |
|------|------|--------|----------|
| NFR-P1 | 安装包体积 | <15 MB | 检查 NSIS 安装包大小 |
| NFR-P2 | 冷启动时间 | <3 s（从双击到主窗口可见） | 秒表测量 3 次取平均 |
| NFR-P3 | 空闲内存占用 | <100 MB | 任务管理器观察 |
| NFR-P4 | 服务器运行时内存 | <150 MB | 任务管理器观察 |
| NFR-P5 | 日志推送延迟 | <200 ms（从 stdout 到前端显示） | 时间戳对比 |
| NFR-P6 | RCON 命令响应 | <1 s（含网络往返） | 实际发送命令测量 |
| NFR-P7 | 配置保存耗时 | <500 ms | 实际保存测量 |
| NFR-P8 | 状态轮询频率 | 5 s/次（可配置） | 代码审查 |

### 3.2 安全需求

| 编号 | 需求 | 实现方式 |
|------|------|----------|
| NFR-S1 | RCON 密码本地加密存储 | 使用 Windows DPAPI 或 Tauri secure storage |
| NFR-S2 | 管理员密码不明文显示 | 输入框 type=password |
| NFR-S3 | PowerShell 命令注入防护 | 所有用户输入经 shell-escape 处理 |
| NFR-S4 | 文件路径校验 | 限制只能选择目录，禁止直接输入 |
| NFR-S5 | Tauri CSP 严格策略 | `default-src 'self'`，禁止远程脚本 |
| NFR-S6 | 不上传任何数据 | 应用纯本地，无任何网络上报 |

### 3.3 兼容性需求

| 编号 | 需求 | 说明 |
|------|------|------|
| NFR-C1 | Windows 10 1809+ | 最低系统版本 |
| NFR-C2 | Windows 11 全版本 | 完全支持 |
| NFR-C3 | WebView2 Runtime | Win11 内置；Win10 通过 bootstrapper 安装 |
| NFR-C4 | Palworld 服务器版本 | 兼容当前最新版（含 Sakurajima 更新参数） |
| NFR-C5 | Radmin LAN（可选） | 已安装时自动检测，未安装不影响其他功能 |
| NFR-C6 | SteamCMD（可选） | 已安装时支持一键更新，未安装不影响其他功能 |
| NFR-C7 | 屏幕分辨率 | 最低 1280×720，推荐 1920×1080 |

### 3.4 可靠性需求

| 编号 | 需求 | 实现方式 |
|------|------|----------|
| NFR-R1 | 服务器进程异常退出检测 | `try_wait()` 轮询 + 事件通知前端 |
| NFR-R2 | RCON 连接断开自动重连 | 检测 `is_connected()` 失败时提示重连 |
| NFR-R3 | 设置文件损坏容错 | JSON 解析失败时回退默认值 |
| NFR-R4 | 配置文件备份 | 保存前自动备份上一版 |
| NFR-R5 | 日志缓冲区上限 | 500 行自动滚动，防止内存泄漏 |
| NFR-R6 | 进程句柄释放 | 应用退出前确保子进程已 kill |

### 3.5 可维护性需求

| 编号 | 需求 | 实现方式 |
|------|------|----------|
| NFR-M1 | 代码规范 | 遵循 AGENTS.md 中的 TypeScript/Rust/CSS 规范 |
| NFR-M2 | 类型严格 | TypeScript strict 模式 + Rust 无 unsafe |
| NFR-M3 | 模块解耦 | 前端按页面+store 分模块，后端按功能分文件 |
| NFR-M4 | 错误信息中文化 | 所有 Tauri command 错误返回中文 |
| NFR-M5 | 构建可复现 | `npm run build` + `npm run tauri:build` 一键打包 |

### 3.6 国际化需求
- **NFR-I1**：当前版本仅支持简体中文，但代码结构预留 i18n 接入点（`t('key')` 模式）
- **NFR-I2**：所有用户可见文案集中在组件中，不散落在工具函数

---

## 四、用户体验需求

### 4.1 视觉设计需求

#### 4.1.1 设计系统（源自 `palworld-warm-glass-preview`）
| 元素 | 规格 |
|------|------|
| 主色 | `#e66f51`（珊瑚暖橙） |
| 背景 | `#f5ede2`（奶油米白）+ 三层径向渐变光晕 |
| 卡片 | `rgba(255,252,247,0.72)` 半透明 + `backdrop-filter: blur(24px) saturate(145%)` |
| 边框 | `rgba(116,88,72,0.14)` 低对比暖灰 |
| 圆角 | 12px（sm）/ 18px（md）/ 26px（lg）/ 32px（panel） |
| 字体 | Inter + Microsoft YaHei（中文优先） |
| 状态色 | success `#4f8a6b` / warning `#b8782f` / error `#c9554d` / info `#4b7896` |

#### 4.1.2 布局需求
- **NFR-U1**：左侧 228px 固定侧栏（导航 + 服务器状态摘要）
- **NFR-U2**：主工作区顶部独立头部（页面标题 + 状态徽章）
- **NFR-U3**：内容区可滚动，侧栏与头部不滚动
- **NFR-U4**：窗口最小尺寸 1200×800（推荐），最小可缩放至 900×720
- **NFR-U5**：响应式断点 1060px / 900px / 760px（侧栏折叠为图标）

### 4.2 交互需求

| 编号 | 需求 | 实现方式 |
|------|------|----------|
| NFR-U6 | 所有按钮支持键盘焦点 | `:focus-visible` 焦点环 |
| NFR-U7 | 状态变化无障碍通知 | `aria-live="polite"` 区域 |
| NFR-U8 | 导航标记当前页 | `aria-current="page"` |
| NFR-U9 | 危险操作二次确认 | 停止服务器/恢复备份/重置配置 |
| NFR-U10 | 长操作显示 loading | 启动服务器/保存配置/更新服务器 |
| NFR-U11 | 错误提示友好 | 中文 + 具体原因 + 建议操作 |
| NFR-U12 | 支持 `prefers-reduced-motion` | 减少动画 |
| NFR-U13 | 表单回车提交 | RCON 命令输入、广播消息 |
| NFR-U14 | 表格空状态 | 玩家列表无数据时显示空状态插图 |
| NFR-U15 | 快捷键支持 | Ctrl+S 保存配置（P2） |

### 4.3 反馈需求
| 场景 | 反馈方式 |
|------|----------|
| 按钮点击 | 微动效（translateY -1px） + 颜色变化 |
| 操作成功 | 顶部 Toast 提示（3s 自动消失） |
| 操作失败 | 顶部 Toast 错误提示（红色，需手动关闭） |
| 长操作 | 按钮置灰 + 旋转 loading 图标 |
| 状态变化 | 状态徽章颜色变化 + dot 脉冲 |
| 日志新增 | 自动滚动到底部 + 新行高亮 1s |

---

## 五、功能模块划分

### 5.1 前端模块划分

```
src/
├── main.ts                      # 应用入口
├── App.vue                      # 根组件（AppShell 布局）
├── style.css                    # 全局样式（--palwarm-* 变量 + 玻璃面板）
├── router/
│   └── index.ts                 # Vue Router（5 个路由）
├── stores/
│   ├── server.ts                # 服务器状态（进程/日志/路径）
│   ├── config.ts                # 配置状态（参数/修改标记）
│   ├── network.ts               # 网络状态（防火墙/端口/Radmin）
│   ├── rcon.ts                  # RCON 状态（连接/玩家/输出）
│   └── settings.ts              # 应用设置（持久化）
├── api/
│   └── tauri.ts                 # Tauri invoke 封装（统一错误处理）
├── components/
│   ├── layout/
│   │   ├── AppShell.vue         # 应用外壳（侧栏 + 工作区）
│   │   ├── Sidebar.vue          # 侧栏（导航 + 状态摘要）
│   │   └── HeaderBar.vue        # 页面头部
│   ├── ui/
│   │   ├── GlassPanel.vue       # 玻璃面板容器
│   │   ├── StateBadge.vue       # 状态徽章
│   │   ├── BaseButton.vue       # 按钮组件
│   │   ├── Toast.vue            # 全局 Toast
│   │   └── ConfirmDialog.vue    # 确认对话框
│   └── server/
│       ├── LogPanel.vue         # 日志面板
│       └── PathDialog.vue       # 路径选择对话框
├── views/
│   ├── DashboardView.vue        # 首页
│   ├── ConfigView.vue           # 配置
│   ├── NetworkView.vue          # 网络
│   ├── RconView.vue             # RCON
│   └── TroubleshootView.vue     # 故障排查
└── types/
    └── tauri.ts                 # Tauri 命令类型声明
```

### 5.2 后端模块划分

```
src-tauri/src/
├── main.rs                      # 入口 + 命令注册
├── server.rs                    # 服务器进程管理（从 main.rs 拆出）
├── config.rs                    # 配置读写（从 main.rs 拆出）
├── firewall.rs                  # 防火墙（从 main.rs 拆出）
├── network.rs                   # 网络检测（已存在）
├── rcon.rs                      # RCON 客户端（已存在）
├── settings.rs                  # 设置持久化（已存在）
├── steamcmd.rs                  # 【新增】SteamCMD 更新
├── backup.rs                    # 【新增】存档备份
└── tray.rs                      # 【新增】系统托盘
```

---

## 六、交互流程设计

### 6.1 全局流程

#### 6.1.1 应用启动流程
```
用户双击应用图标
  ├── Tauri 启动 → 创建主窗口（1200×800，隐藏）
  ├── 延迟 500ms → 设置尺寸 + 居中 + 显示 + 聚焦
  ├── Vue 应用挂载 → 路由初始化 → 默认跳转 /dashboard
  ├── 各 store 初始化
  │   ├── settings store → load_app_settings()
  │   ├── server store → init_server_state()
  │   └── network store → check_firewall_rules() + check_radmin_lan_status()
  └── Dashboard 页渲染 → 显示上次路径 + 服务器状态
```

#### 6.1.2 全局错误处理流程
```
Tauri command 返回 Err
  ├── invoke().catch(error)
  ├── 提取 error 字符串
  ├── 调用 toast.error(message)
  └── Toast 显示 3-5s（错误需手动关闭）
```

### 6.2 跨模块流程

#### 6.2.1 首次使用流程
```
用户首次打开应用
  ├── 检测 settings.server_path 为空
  ├── Dashboard 显示"未设置路径"状态
  ├── 引导用户点击"选择路径"
  │   ├── 弹出 Tauri 文件对话框
  │   ├── 用户选择包含 PalServer.exe 的目录
  │   └── 保存到 settings.server_path
  ├── 自动定位配置文件路径
  ├── 引导用户配置防火墙（一键放行）
  ├── 引导用户切换到"配置"页设置服务器参数
  └── 返回首页启动服务器
```

#### 6.2.2 服务器运行时流程
```
服务器启动成功
  ├── Dashboard 状态卡片更新（运行中 + PID）
  ├── 日志面板自动展开 + 实时追加
  ├── 后台线程收集 stdout/stderr
  ├── 5s 轮询 get_server_status 检测存活
  ├── 用户切换到 RCON 页 → 可连接（需配置 AdminPassword）
  ├── 用户切换到配置页 → 修改后保存需重启服务器才生效（提示）
  └── 用户关闭应用 → 弹窗询问"是否停止服务器"
```

### 6.3 关键交互细节

#### 6.3.1 路径选择对话框
- 触发：Dashboard 的"选择路径"按钮 + Config 页的"更换路径"按钮
- 实现：调用 Tauri `dialog.open({ directory: true })`
- 校验：选中目录后检查 `PalServer.exe` 是否存在
- 失败提示：`所选目录未找到 PalServer.exe，请确认是否为服务器安装目录`
- 成功：保存到 settings + 更新 server store

#### 6.3.2 日志面板
- 默认折叠（仅显示按钮"显示日志"）
- 展开后高度 200px，可滚动
- 自动滚动到底部（除非用户手动上滚）
- 新行高亮 1s（CSS 动画）
- 最多保留 500 行（前端裁剪）
- 清空按钮（调用 `clear_server_logs`）

#### 6.3.3 配置表单
- 按分组折叠（默认展开"基础"组）
- range 类型显示当前值 + 滑块
- toggle 类型显示开关
- select 类型显示下拉（如 Difficulty: None/Normal/Difficult）
- 修改项标记小圆点
- 底部固定栏：保存 / 恢复默认 / 取消修改

---

## 七、需求优先级排序

### 7.1 优先级定义
- **P0（必须）**：MVP 必备，无此功能应用无法使用
- **P1（应该）**：核心体验，发布时应有
- **P2（可选）**：增强功能，后续迭代

### 7.2 P0 需求清单（MVP）

| 模块 | 功能项 | 验收标准 |
|------|--------|----------|
| M1 首页 | M1-F1 状态概览卡片 | 4 张卡片正确显示状态 |
| M1 首页 | M1-F2 服务器控制 | 启动/停止按钮工作正常 |
| M1 首页 | M1-F3 实时日志面板 | 日志实时推送 + 自动滚动 |
| M1 首页 | M1-F5 状态自动刷新 | 5s 轮询，状态准确 |
| M1 首页 | M1-F6 日志实时推送 | server-log 事件正确监听 |
| M2 配置 | M2-F1 参数分组导航 | 9 个分组正确渲染 |
| M2 配置 | M2-F2 字段类型渲染 | 5 种类型正确渲染 |
| M2 配置 | M2-F3 参数描述展示 | 悬停显示中文描述 |
| M2 配置 | M2-F4 加载现有配置 | 正确解析 INI |
| M2 配置 | M2-F5 恢复默认值 | 一键重置 |
| M2 配置 | M2-F6 保存配置 | 正确写入 INI |
| M2 配置 | M2-F7 配置路径自动定位 | 基于 server_path 拼接 |
| M3 网络 | M3-F1 防火墙状态检测 | 3 个端口状态正确 |
| M3 网络 | M3-F2 一键放行端口 | 规则添加成功 |
| M3 网络 | M3-F3 端口占用检测 | 显示占用进程 |
| M3 网络 | M3-F4 Radmin LAN 检测 | 安装状态 + 虚拟 IP |
| M3 网络 | M3-F5 连接地址展示 | 三档地址显示 |
| 全局 | 应用外壳 | 侧栏 + 头部 + 路由 |
| 全局 | 设计系统迁移 | --palwarm-* 全部应用 |
| 全局 | 设置持久化 | 路径/RCON 配置保存 |

### 7.3 P1 需求清单

| 模块 | 功能项 |
|------|--------|
| M1 首页 | M1-F4 快速搭建指南, M1-F7 一键复制连接地址 |
| M2 配置 | M2-F8 预设方案, M2-F9 修改前对比, M2-F10 配置备份 |
| M3 网络 | M3-F6 一键复制地址, M3-F7 Radmin 安装引导, M3-F8 联机向导 |
| M4 RCON | M4-F1~F9 全部 |
| M5 故障排查 | M5-F1~F4 |

### 7.4 P2 需求清单

| 模块 | 功能项 |
|------|--------|
| M5 故障排查 | M5-F5 VC++/DirectX 检测, M5-F6 配置校验 |
| M6 存档备份 | M6-F1~F5 |
| M7 SteamCMD | M7-F1~F4 |
| M8 系统托盘 | M8-F1~F4 |
| M9 自动重启 | M9-F1~F3 |

### 7.5 优先级与发布版本映射

| 版本 | 包含优先级 | 目标 |
|------|-----------|------|
| v1.0.0（MVP） | P0 | 验证核心流程：搭建 + 配置 + 启停 + 网络 |
| v1.1.0 | P0 + P1 | 完整功能：含 RCON + 故障排查 + 预设方案 |
| v1.2.0 | + P2 | 增强功能：备份 + SteamCMD + 托盘 + 自动重启 |

---

## 八、验收标准

### 8.1 功能验收
- 所有 P0 功能项 100% 通过验收
- 所有 P1 功能项 100% 通过验收（v1.1.0）
- P2 功能项按实际实现情况验收

### 8.2 非功能验收
| 验收项 | 标准 |
|--------|------|
| 安装包体积 | <15 MB |
| 冷启动 | <3 s |
| 空闲内存 | <100 MB |
| 类型检查 | `vue-tsc --noEmit` 无错误 |
| Rust 编译 | `cargo check` 无错误 |
| 生产构建 | `npm run tauri:build` 成功输出 NSIS 安装包 |
| 实机运行 | 在 Windows 10/11 上安装运行无崩溃 |

### 8.3 用户体验验收
- 5 大页面视觉一致（遵循 --palwarm-* 设计系统）
- 所有按钮可键盘操作
- 错误提示均为中文
- 危险操作均有二次确认

---

## 九、本次优化新增功能（2026-07）

> 本节用于补充记录本次优化（change-id: `optimize-palworld-design`）实际落地的功能项，与正文需求 ID 对照。

### 9.1 已实现功能与需求 ID 对照

| 功能 | 实现 | 对应需求 ID | 备注 |
|------|------|------------|------|
| 配置预设 | `presets.rs` + `PresetSelector.vue` + 4 套预设 JSON | M2-F8 | 包含 default / pve-friendly / pvp-competitive / speedrun |
| 修改前对比 | `DiffIndicator.vue` | M2-F9 | 在参数行左侧显示橙色竖线 |
| 配置备份 | `write_config` 自动备份 + `list_config_backups` | M2-F10 | 备份目录 `%AppData%/PalworldServerManager/config-backups/`，最多保留 20 份 |
| 配置恢复 | `restore_config_backup` command + ConfigView 恢复备份按钮 | （本次新增） | 不在原需求 ID 列表中，作为 M2-F10 的延伸功能 |
| 联机向导 | `NetworkView.vue` 4 步引导卡片 | M3-F8 | 下载 Radmin → 创建/加入网络 → 重启 Radmin → 复制地址 |
| RCON 玩家列表 | `PlayerTable.vue` + `rconStore.players/refreshPlayers` | M4-F4 | 解析 `/ShowPlayers` 输出，按最后一个逗号分隔 name 和 steamid |
| RCON 踢出玩家 | `rconStore.kickPlayer(steamid)` | M4-F5 | 二次确认 + Toast + 刷新玩家列表 |
| RCON 封禁玩家 | `rconStore.banPlayer(steamid)` | M4-F5 | 二次确认 + Toast + 刷新玩家列表 |
| RCON 命令历史 | `commandHistory` ref + 持久化到 localStorage | M4-F6 | ↑↓ 翻页，最多 50 条，key=`rcon-command-history` |
| RCON 配置持久化 | `settings` store + 连接成功后自动保存 | M4-F9 | host/port/password 保存到 AppSettings |
| 故障一键诊断 | `TroubleshootView.vue` + `DiagnosticReport.vue` + 并行 5 项检测 | M5-F2 | OK/WARN/ERROR 三态卡片，可展开查看修复建议 |
| 日志导出 | `export_server_logs` command + `dialog.save()` | M5-F4 | 导出最近 500 行服务器日志 |

### 9.2 Rust 后端新增命令

| 命令 | 模块 | 用途 |
|------|------|------|
| `list_presets` | `presets.rs` | 列出 4 套预设的元信息（name / description / key_params） |
| `apply_preset` | `presets.rs` | 将指定预设合并到调用方传入的 config HashMap（缺失不覆盖） |
| `list_config_backups` | `config.rs` | 列出 `%AppData%/PalworldServerManager/config-backups/` 下的备份文件 |
| `restore_config_backup` | `config.rs` | 从指定备份恢复配置文件 |
| `export_server_logs` | `server.rs` | 将 `state.logs` 写入用户选择的路径 |

### 9.3 前端新增组件

| 组件 | 路径 | 用途 |
|------|------|------|
| `PresetSelector.vue` | `components/ui/` | 预设下拉选择 + 描述展示 |
| `DiffIndicator.vue` | `components/ui/` | 橙色竖线 + tooltip 显示原值 |
| `DiagnosticReport.vue` | `components/ui/` | 可展开的诊断报告卡片 |
| `PlayerTable.vue` | `components/server/` | 玩家表格 + 空状态 |
| `Toast.vue`（重构） | `components/ui/` | 全局 Toast 队列（`useToastStore`） |

### 9.4 运维脚本（位于 `Palworld/scripts/`）

| 脚本 | 用途 |
|------|------|
| `verify-window.ps1` | 窗口尺寸验证 + 自动修复 + 截图（继承 v2 Task 7） |
| `switch-route.ps1` | 文件触发机制路由切换 + 截图（继承 v2 Task 8） |
| `check-webview2.ps1` | WebView2 运行时注册表检查（本次新增） |
| `health-check.ps1` | 综合诊断：WebView2 + 端口 5222 + 防火墙 + 窗口 + 截图（本次新增） |
