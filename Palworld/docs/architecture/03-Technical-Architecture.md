# 03 · 技术架构文档

> Palworld Server Manager 桌面应用 — 技术架构设计
> 版本：v1.0 · 编制日期：2026-07-20

---

## 一、系统总览

### 1.1 架构风格
**Tauri 2 桌面应用 + Vue 3 SPA + Rust 命令式后端**的经典三层架构：
- **渲染层（WebView2）**：Vue 3 SPA，负责 UI 渲染与用户交互
- **桥接层（Tauri IPC）**：`invoke()` / `emit()` / `listen()` 三种通信通道
- **系统层（Rust）**：Tauri Commands 调用 Windows API / 进程 / 文件系统

### 1.2 架构图

```
┌────────────────────────────────────────────────────────────────────┐
│                    Palworld Server Manager (Tauri 2 App)            │
├────────────────────────────────────────────────────────────────────┤
│  渲染层 (Vue 3 + WebView2)                                          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Views (5 个页面)                                            │  │
│  │  ├── DashboardView   ├── ConfigView    ├── NetworkView      │  │
│  │  ├── RconView        └── TroubleshootView                    │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │  Stores (Pinia)      │  Components      │  Router            │  │
│  │  ├── server.ts       │  ├── AppShell    │  └── index.ts      │  │
│  │  ├── config.ts       │  ├── GlassPanel  │                    │  │
│  │  ├── network.ts      │  ├── StateBadge  │                    │  │
│  │  ├── rcon.ts         │  └── BaseButton  │                    │  │
│  │  └── settings.ts     │                  │                    │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │  API Layer: src/api/tauri.ts (invoke 封装 + 类型声明)        │  │
│  └──────────────────────────────────────────────────────────────┘  │
├────────────────────────────────────────────────────────────────────┤
│  桥接层 (Tauri IPC)                                                 │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐       │
│  │  invoke()      │  │  emit()        │  │  listen()      │       │
│  │  请求-响应     │  │  Rust→Vue 推送 │  │  Vue 订阅事件  │       │
│  └────────────────┘  └────────────────┘  └────────────────┘       │
├────────────────────────────────────────────────────────────────────┤
│  系统层 (Rust + Tauri 2)                                            │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Tauri Commands (21 + 新增)                                  │  │
│  │  ├── server.rs    (进程管理 + 日志收集)                      │  │
│  │  ├── config.rs    (INI 解析 + 默认配置)                      │  │
│  │  ├── firewall.rs  (PowerShell 防火墙)                        │  │
│  │  ├── network.rs   (Radmin + 本地 IP)                         │  │
│  │  ├── rcon.rs      (RCON TCP 客户端)                          │  │
│  │  ├── settings.rs  (JSON 持久化)                              │  │
│  │  ├── steamcmd.rs  (SteamCMD 更新) [新增]                     │  │
│  │  ├── backup.rs    (存档备份) [新增]                          │  │
│  │  └── tray.rs      (系统托盘) [新增]                          │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │  State Management (Arc<Mutex<T>>)                            │  │
│  │  ├── ServerState (process + logs + server_path)              │  │
│  │  └── RconState   (TcpStream + request_id)                    │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │  OS Layer: std::process / std::fs / std::net / PowerShell    │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
        │                          │                          │
        ▼                          ▼                          ▼
   PalServer.exe            SteamCMD.exe              Windows NetSecurity
   (子进程)                 (子进程)                  (PowerShell 调用)
```

---

## 二、分层设计

### 2.1 渲染层（Vue 3 SPA）

#### 2.1.1 职责
- UI 渲染与用户交互
- 调用 Tauri Commands 获取/修改数据
- 监听 Tauri Events 更新 UI
- 状态管理（Pinia）

#### 2.1.2 技术栈
| 技术 | 版本 | 用途 |
|------|------|------|
| Vue 3 | 3.4+ | 渲染框架（Composition API + `<script setup>`） |
| TypeScript | 5.4+ | 类型安全（strict 模式） |
| Pinia | 2.1+ | 状态管理 |
| Vue Router | 4.x | 路由 |
| Vite | 5.4+ | 构建工具 |
| lucide-vue-next | 1.0+ | 图标库 |
| @tauri-apps/api | 2.0+ | Tauri 前端 SDK |

#### 2.1.3 路由设计
```typescript
// src/router/index.ts
const routes = [
  { path: '/', redirect: '/dashboard' },
  { path: '/dashboard', name: 'dashboard', component: DashboardView, meta: { title: '首页' } },
  { path: '/config', name: 'config', component: ConfigView, meta: { title: '配置' } },
  { path: '/network', name: 'network', component: NetworkView, meta: { title: '网络' } },
  { path: '/rcon', name: 'rcon', component: RconView, meta: { title: 'RCON' } },
  { path: '/troubleshoot', name: 'troubleshoot', component: TroubleshootView, meta: { title: '故障排查' } },
]
```

#### 2.1.4 状态管理设计

**server.ts store**:
```typescript
// src/stores/server.ts
export const useServerStore = defineStore('server', () => {
  const status = ref<ServerStatus>({ running: false, pid: null, serverPath: '', logCount: 0 })
  const logs = ref<string[]>([])
  const isLoading = ref(false)

  async function init() { /* 调用 init_server_state */ }
  async function start(path: string) { /* 调用 start_server */ }
  async function stop() { /* 调用 stop_server */ }
  async function refreshStatus() { /* 调用 get_server_status */ }
  function setupLogListener() { /* listen('server-log') */ }

  return { status, logs, isLoading, init, start, stop, refreshStatus, setupLogListener }
})
```

**config.ts store**:
```typescript
export const useConfigStore = defineStore('config', () => {
  const configPath = ref('')
  const config = ref<Record<string, string>>({})
  const descriptions = ref<ConfigValue[]>([])
  const dirty = ref<Set<string>>(new Set())

  async function load(path: string) { /* 调用 read_config */ }
  async function save() { /* 调用 write_config */ }
  async function resetToDefault() { /* 调用 get_default_config */ }
  function update(key: string, value: string) { /* 标记 dirty */ }

  return { configPath, config, descriptions, dirty, load, save, resetToDefault, update }
})
```

### 2.2 桥接层（Tauri IPC）

#### 2.2.1 三种通信通道

| 通道 | 方向 | 用途 | 示例 |
|------|------|------|------|
| `invoke()` | Vue → Rust | 请求-响应式调用 | `invoke('start_server', { path })` |
| `emit()` | Rust → Vue | 服务端推送事件 | `app.emit("server-log", line)` |
| `listen()` | Vue 订阅 | 监听 Rust 推送 | `listen('server-log', cb)` |

#### 2.2.2 事件清单

| 事件名 | 载荷 | 触发时机 | 前端处理 |
|--------|------|----------|----------|
| `server-log` | `string` | 服务器输出一行 stdout/stderr | 追加到日志面板 |
| `server-status-change` | `ServerStatus` | 进程退出（未来扩展） | 更新状态卡片 |
| `steamcmd-log` | `string` | SteamCMD 输出一行 | 追加到更新日志（P2） |

#### 2.2.3 API 封装层

```typescript
// src/api/tauri.ts
import { invoke } from '@tauri-apps/api/core'

export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (error) {
    // Tauri 错误统一为 string
    const message = typeof error === 'string' ? error : String(error)
    throw new Error(message)
  }
}

// 类型化封装
export const api = {
  server: {
    init: () => tauriInvoke<ServerStatus>('init_server_state'),
    start: (path: string) => tauriInvoke<ServerStatus>('start_server', { path }),
    stop: () => tauriInvoke<ServerStatus>('stop_server'),
    getStatus: () => tauriInvoke<ServerStatus>('get_server_status'),
    getLogs: () => tauriInvoke<string[]>('get_server_logs'),
    clearLogs: () => tauriInvoke<void>('clear_server_logs'),
  },
  config: {
    read: (path: string) => tauriInvoke<Record<string, string>>('read_config', { path }),
    write: (path: string, config: Record<string, string>) => tauriInvoke<string>('write_config', { path, config }),
    getDefault: () => tauriInvoke<Record<string, string>>('get_default_config'),
    getDescriptions: () => tauriInvoke<ConfigValue[]>('get_config_descriptions'),
  },
  firewall: {
    check: () => tauriInvoke<FirewallStatus>('check_firewall_rules'),
    add: () => tauriInvoke<string>('add_firewall_rules'),
  },
  network: {
    checkPort: (port: number) => tauriInvoke<string | null>('check_port_usage', { port }),
    checkRadmin: () => tauriInvoke<RadminLanStatus>('check_radmin_lan_status'),
  },
  rcon: {
    connect: (host: string, port: number, password: string) => tauriInvoke<string>('rcon_connect', { host, port, password }),
    send: (command: string) => tauriInvoke<string>('rcon_send_command', { command }),
    disconnect: () => tauriInvoke<void>('rcon_disconnect'),
    isConnected: () => tauriInvoke<boolean>('rcon_is_connected'),
  },
  settings: {
    load: () => tauriInvoke<AppSettings>('load_app_settings'),
    save: (settings: AppSettings) => tauriInvoke<void>('save_app_settings', { settings }),
  },
}
```

### 2.3 系统层（Rust + Tauri 2）

#### 2.3.1 模块划分

> M1–M5 全部模块已实现 ✅；P2 模块（M6/M7/M8）保留为 🆕 待后续迭代。

| 模块 | 文件 | 职责 | 状态 |
|------|------|------|------|
| 入口 | `main.rs` | Tauri Builder 配置 + 命令注册 + State 注入 | ✅ 已实现（M1） |
| 服务器进程 | `server.rs` | 启动/停止/状态/日志收集（上限 500 + 进程退出事件） | ✅ 已实现（M1） |
| 配置管理 | `config.rs` | INI 解析/默认值/描述/写入前自动备份/列备份/恢复备份 | ✅ 已实现（M2） |
| 防火墙 | `firewall.rs` | PowerShell 防火墙规则（shell-escape 防注入） | ✅ 已实现（M3） |
| 网络检测 | `network.rs` | Radmin LAN + 本地 IP（shell-escape 防注入） | ✅ 已实现（M3） |
| RCON | `rcon.rs` | TCP RCON 协议客户端 | ✅ 已实现（M4） |
| 设置持久化 | `settings.rs` | JSON 读写 | ✅ 已实现（全局） |
| 配置预设 | `presets.rs` | 4 套预设（default / pve-friendly / pvp-competitive / speedrun）的列出与套用 | ✅ 已实现（M2） |
| 路由切换 | `route_switch.rs` | 通过文件触发机制 + `WebviewWindow::eval()` 调用 `window.__router.push()` 实现 E2E 路由切换 | ✅ 已实现（E2E 验收用，非生产路径） |
| SteamCMD | `steamcmd.rs` | 服务器更新 | 🆕 新增（P2） |
| 存档备份 | `backup.rs` | 备份/恢复 | 🆕 新增（P2） |
| 系统托盘 | `tray.rs` | 托盘菜单 | 🆕 新增（P2） |

#### 2.3.2 共享状态设计

```rust
// 主进程共享状态（通过 tauri::State 注入）
struct ServerState {
    process: Mutex<Option<std::process::Child>>,    // 子进程句柄
    server_path: Mutex<String>,                      // 服务器路径
    logs: Arc<Mutex<Vec<String>>>,                   // 日志缓冲区（上限 500）
}

pub struct RconState {
    pub client: Mutex<RconClient>,                   // RCON TCP 客户端
}
```

#### 2.3.3 命令注册

```rust
.invoke_handler(tauri::generate_handler![
    // 服务器
    init_server_state, start_server, stop_server,
    get_server_status, get_server_logs, clear_server_logs,
    export_server_logs,                       // 【新增 P1】导出最近 500 行日志
    // 配置
    read_config, write_config, get_default_config, get_config_descriptions,
    list_config_backups,                      // 【新增 P1】列出配置备份
    restore_config_backup,                    // 【新增 P1】从备份恢复配置
    // 配置预设
    list_presets,                              // 【新增 P1】列出 4 套预设元信息
    apply_preset,                             // 【新增 P1】套用指定预设到 config
    // 防火墙
    check_firewall_rules, add_firewall_rules,
    // 网络
    check_port_usage, check_radmin_lan_status,
    // RCON
    rcon_connect, rcon_send_command, rcon_disconnect, rcon_is_connected,
    // 设置
    load_app_settings, save_app_settings,
    // 新增（P2，尚未实现）
    // update_server, backup_save, restore_save, list_backups,
])
```

> 命令清单合计 21 项（含 5 个本次优化新增的 P1 命令）。

---

## 三、模块间接口定义

### 3.1 类型定义（TypeScript 端）

```typescript
// src/types/tauri.ts

export interface ServerStatus {
  running: boolean
  pid: number | null
  serverPath: string
  logCount: number
}

export interface FirewallStatus {
  port8211Open: boolean
  port27015Open: boolean
  port25575Open: boolean
}

export interface ConfigValue {
  name: string
  value: string
  description: string
  fieldType: 'range' | 'toggle' | 'select' | 'text' | 'number'
  min: number | null
  max: number | null
  step: number | null
}

export interface RadminLanStatus {
  installed: boolean
  virtualIp: string
  adapterStatus: string
}

export interface AppSettings {
  serverPath: string
  configPath: string
  rconHost: string
  rconPort: number
  rconPassword: string
}
```

### 3.2 命令接口契约

#### 3.2.1 服务器进程管理

| 命令 | 入参 | 返回 | 错误场景 |
|------|------|------|----------|
| `init_server_state` | 无 | `ServerStatus` | - |
| `start_server` | `{ path: string }` | `ServerStatus` | 服务器已在运行 / PalServer.exe 不存在 / spawn 失败 |
| `stop_server` | 无 | `ServerStatus` | 服务器未运行 |
| `get_server_status` | 无 | `ServerStatus` | - |
| `get_server_logs` | 无 | `string[]` | - |
| `clear_server_logs` | 无 | `void` | - |

#### 3.2.2 配置管理

| 命令 | 入参 | 返回 | 错误场景 |
|------|------|------|----------|
| `read_config` | `{ path: string }` | `Record<string, string>` | 文件不存在 / 解析失败 |
| `write_config` | `{ path: string, config: Record<string, string> }` | `string`（成功消息） | 写入失败 |
| `get_default_config` | 无 | `Record<string, string>` | - |
| `get_config_descriptions` | 无 | `ConfigValue[]` | - |

#### 3.2.3 防火墙

| 命令 | 入参 | 返回 | 错误场景 |
|------|------|------|----------|
| `check_firewall_rules` | 无 | `FirewallStatus` | PowerShell 调用失败 |
| `add_firewall_rules` | 无 | `string`（成功消息） | 规则添加失败（非"已存在"） |

#### 3.2.4 网络

| 命令 | 入参 | 返回 | 错误场景 |
|------|------|------|----------|
| `check_port_usage` | `{ port: number }` | `string \| null`（占用信息） | netstat 调用失败 |
| `check_radmin_lan_status` | 无 | `RadminLanStatus` | PowerShell 调用失败 |

#### 3.2.5 RCON

| 命令 | 入参 | 返回 | 错误场景 |
|------|------|------|----------|
| `rcon_connect` | `{ host: string, port: number, password: string }` | `string`（成功消息） | 连接失败 / 认证失败 |
| `rcon_send_command` | `{ command: string }` | `string`（响应） | 未连接 / 发送失败 / 接收超时 |
| `rcon_disconnect` | 无 | `void` | - |
| `rcon_is_connected` | 无 | `boolean` | - |

#### 3.2.6 设置

| 命令 | 入参 | 返回 | 错误场景 |
|------|------|------|----------|
| `load_app_settings` | 无 | `AppSettings` | 文件损坏时返回默认值 |
| `save_app_settings` | `{ settings: AppSettings }` | `void` | 写入失败 |

### 3.3 新增命令接口（P2）

#### 3.3.1 SteamCMD
```rust
#[command]
async fn update_server(steamcmd_path: String, server_path: String, app_handle: AppHandle) -> Result<String, String>
// 通过 app_handle.emit("steamcmd-log", line) 推送进度
```

#### 3.3.2 存档备份
```rust
#[derive(Serialize)]
struct BackupInfo { name: String, timestamp: String, size_bytes: u64 }

#[command]
async fn backup_save(server_path: String) -> Result<String, String>  // 返回备份路径

#[command]
async fn list_backups() -> Result<Vec<BackupInfo>, String>

#[command]
async fn restore_backup(backup_name: String, server_path: String) -> Result<(), String>

#[command]
async fn delete_backup(backup_name: String) -> Result<(), String>
```

---

## 四、数据流设计

### 4.1 启动服务器数据流

```
[用户点击启动按钮]
    │
    ▼
[DashboardView.vue] 调用 serverStore.start(path)
    │
    ▼
[stores/server.ts] 调用 api.server.start(path)
    │
    ▼
[api/tauri.ts] invoke('start_server', { path })
    │
    ▼  (Tauri IPC 序列化)
[Rust main.rs] start_server(state, app_handle, path)
    │
    ├──> 校验 exe_path 存在
    ├──> Command::new(exe_path).spawn() → 获取 child
    ├──> 启动 stdout 收集线程
    │       └──> 循环读取 line → logs.push(line) → app.emit("server-log", line)
    ├──> 启动 stderr 收集线程（同上，前缀 [ERR]）
    ├──> *process = Some(child)
    └──> 返回 ServerStatus { running: true, pid, ... }
    │
    ▼  (Tauri IPC 反序列化)
[api/tauri.ts] 返回 ServerStatus
    │
    ▼
[stores/server.ts] status.value = result, isLoading.value = false
    │
    ▼
[DashboardView.vue] 响应式更新 UI（状态卡片变绿 + PID 显示）

并行：
[main.ts setupLogListener] listen('server-log', (event) => {
    serverStore.logs.push(event.payload)
})
    │
    ▼
[LogPanel.vue] 响应式追加日志行 + 自动滚动
```

### 4.2 配置保存数据流

```
[用户点击保存按钮]
    │
    ▼
[ConfigView.vue] 调用 configStore.save()
    │
    ▼
[stores/config.ts]
    ├──> 组装 config: Record<string, string>（从表单状态）
    └──> 调用 api.config.write(configPath, config)
    │
    ▼
[api/tauri.ts] invoke('write_config', { path, config })
    │
    ▼
[Rust main.rs] write_config(path, config)
    │
    ├──> 排序 options
    ├──> 格式化为 "Key=Value,Key=Value"
    ├──> 包装为 "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(...)\n"
    └──> std::fs::write(path, content)
    │
    ▼
[ConfigView.vue] Toast 显示"配置文件已保存" + dirty.clear()
```

### 4.3 RCON 命令执行数据流

```
[用户输入命令 + 回车]
    │
    ▼
[RconView.vue] 调用 rconStore.sendCommand(cmd)
    │
    ▼
[stores/rcon.ts] 调用 api.rcon.send(cmd)
    │
    ▼
[api/tauri.ts] invoke('rcon_send_command', { command: cmd })
    │
    ▼
[Rust rcon.rs] RconClient.send_command(cmd)
    │
    ├──> send_packet(2, cmd)
    │       └──> 构造 packet: Length + ID + Type + Body + Padding
    │       └──> stream.write_all(packet)
    ├──> receive_packet()
    │       └──> 读取 length (4B) → 读取 body (length B) → 解析 ID + body
    └──> 返回 body 字符串
    │
    ▼
[RconView.vue] 响应追加到输出区 + 命令存入历史
```

---

## 五、核心技术难点与解决方案

### 5.1 难点一：INI 配置文件解析（已解决）

**问题**：`PalWorldSettings.ini` 的 `OptionSettings=(...)` 格式不是标准 INI，值中包含引号、括号嵌套（如 `CrossplayPlatforms=(Steam,Xbox,PS5,Mac)`），简单按逗号分割会出错。

**解决方案**（已实现于 `main.rs` 的 `read_config`）：
- 状态机解析：`in_quotes`（引号内）/ `in_parens`（括号嵌套层级）/ `parsing_key`（当前在解析 key 还是 value）
- 遇到 `,` 且 `!in_quotes && in_parens == 0` 时才切分键值对
- 遇到 `=` 且 `!in_quotes && parsing_key` 时切换到 value 解析

**风险**：当前实现假设 `OptionSettings=(...)` 只有一个，若未来 Palworld 改为多行格式需调整。

### 5.2 难点二：服务器进程日志实时推流（已解决）

**问题**：`std::process::Command::spawn` 返回的 child 的 stdout 是阻塞读取，会卡住主线程。

**解决方案**（已实现于 `main.rs` 的 `start_server`）：
- 启动独立 `std::thread::spawn` 线程读取 stdout
- 使用 `BufReader::lines()` 逐行读取
- 通过 `app_handle.emit("server-log", line)` 推送到前端
- 日志缓冲区 `Arc<Mutex<Vec<String>>>` 上限 500 行，超出移除最早行
- 前端 `listen('server-log', cb)` 接收并追加

**优化点**：
- 前端 LogPanel 应限制显示行数（如 200 行），避免 DOM 性能问题
- 自动滚动到底部（除非用户手动上滚，需记录滚动位置）

### 5.3 难点三：RCON 协议正确性（已解决）

**问题**：Source RCON Protocol 对包长度、ID 匹配、认证响应有严格要求。

**解决方案**（已实现于 `rcon.rs`）：
- 包长度校验：`length < 10 || length > 4096` 视为无效
- 5 秒读超时：`set_read_timeout(Some(Duration::from_secs(5)))`
- 认证流程：发送 Type=3 AUTH 包 → 接收响应 → 判断 body 是否含 "auth"（兼容 Palworld 实现）
- 线程安全：`Mutex<RconClient>` 保护 `TcpStream`

**已知限制**：
- 当前实现未处理多包响应（响应超过 4096 字节会被截断）—— 对 Palworld 命令足够
- 未实现 RCON 连接保活（长连接空闲可能被服务器断开）—— 通过 `is_connected()` 检测 + 用户手动重连

### 5.4 难点四：PowerShell 命令注入防护

**问题**：防火墙规则、网络检测使用 `Command::new("powershell").args([...])`，若参数包含用户输入需防注入。

**解决方案**：
- 当前所有 PowerShell 调用参数均为硬编码（端口号、协议），无用户输入直接拼接
- 未来扩展时（如自定义端口），使用 `format!` 时确保参数为数字类型
- **禁止**使用 `cmd /c` 拼接字符串，统一用 `Command::new().args()`

### 5.5 难点五：Tailwind CDN 迁移

**问题**：设计预览使用 `<script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.3.1">`，不适合生产环境（运行时编译、CSP 限制、性能差）。

**解决方案**：**不引入 Tailwind**，改用原生 CSS + CSS 变量。理由：
- 设计预览中 90% 的样式是原生 CSS（`--palwarm-*` 变量 + 直接属性）
- Tailwind 仅用于少量工具类（`min-h-screen`、`flex`、`gap-*`），可手动改写
- 避免引入 Tailwind 构建链的复杂度
- Tauri CSP 可保持严格（`default-src 'self'`）

**迁移规则**：
| Tailwind 类 | 替换为 |
|------------|--------|
| `min-h-screen` | `min-height: 100vh` |
| `flex` / `grid` | `display: flex/grid` |
| `gap-14` | `gap: 14px` |
| `p-4` | `padding: 16px` |
| `overflow-hidden` | `overflow: hidden` |
| `font-sans` | `font-family: var(--palwarm-font-sans)` |

### 5.6 难点六：设计系统从静态 HTML 迁移到 Vue 组件

**问题**：`dashboard.html` 是 572 行的静态 HTML，需拆分为 Vue 组件且保持视觉一致。

**解决方案**：
1. **CSS 变量直接迁移**：`colors_and_type.css` 的 `--palwarm-*` 变量复制到 `src/style.css`
2. **玻璃面板提取为组件**：`<GlassPanel>` 接收 slot 内容，应用 `.glass-panel` 类
3. **布局拆分**：
   - `App.vue` → `.app-shell` 容器
   - `<Sidebar>` → `.sidebar.glass-panel`（导航 + 状态摘要）
   - `<HeaderBar>` → `.header.glass-panel`（页面标题 + 状态徽章）
   - 各 View 组件 → `.content-scroll` 内容区
4. **图标迁移**：`lucide-vue-next` 替代 `<i data-lucide="xxx">`
   - `<i data-lucide="layout-dashboard">` → `<LayoutDashboard />`
5. **状态绑定**：mock 文本替换为 Pinia store 响应式数据

### 5.7 难点七：Tauri 2 配置文件创建

**问题**：`Palworld-out/` 缺失 `tauri.conf.json`，无法 `tauri build`。

**解决方案**：创建标准 Tauri 2 配置：
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Palworld Server Manager",
  "version": "1.0.0",
  "identifier": "com.palworld.server-manager",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "Palworld Server Manager",
        "width": 1200,
        "height": 800,
        "minWidth": 900,
        "minHeight": 720,
        "resizable": true,
        "visible": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": ["icons/icon.ico"]
  }
}
```

### 5.8 难点八：窗口尺寸设置时序（已解决 · 三重降级方案）

**问题**：Tauri 2.11.5 的 `setup` hook 中 `WebviewWindowBuilder::inner_size` / `set_size` 在部分 Windows 环境下不生效，窗口实际渲染为 14×14 像素，用户看不到应用内容。

#### 真实根因（2026-07 重新诊断）

经实测确认：
1. Tauri 自身的 `inner_size(1200.0, 800.0)` 与 `set_size(LogicalSize::new(...))` 在当前 Windows 环境下均不生效
2. `Process.MainWindowHandle` 在窗口初始化早期返回的 HWND 可能指向 WebView2 内部的隐藏 helper window（14×14 + 空标题），而非真正的应用主窗口
3. Win32 `MoveWindow` API 可强制修正窗口尺寸（已在 `Palworld-out/check-window.ps1` 中验证有效）

#### 三重降级方案

不依赖单一 API，按顺序执行三层修复，任一层成功即停止：

**第一层（Tauri 标准 API）**：`main.rs` setup hook 中
- `WebviewWindowBuilder::new(...).inner_size(1200.0, 800.0).visible(false).build()`
- 立即调用 `window.set_size(LogicalSize::new(1200.0, 800.0))` + `window.show()` + `window.set_focus()`
- 适用于大多数环境，但对当前用户的 Windows 配置无效

**第二层（Rust 内 Win32 强制修复）**：`window_fix.rs` 模块
- `std::thread::spawn` 延迟 800ms 后通过 `window.hwnd()` 获取 HWND
- 调用 `windows::Win32::UI::WindowsAndMessaging::MoveWindow(hwnd, x, y, 1200, 800, true)`
- 配合 `ShowWindow(SW_RESTORE)` + `SetForegroundWindow`
- 等待 500ms 后 `GetWindowRect` 验证尺寸
- **当前环境主要依赖此层**

**第三层（PowerShell 外部修复 + 验收脚本）**：`scripts/verify-window.ps1`
- 按"Palworld Server Manager"标题 `EnumWindows` 枚举顶级窗口（不依赖 `Process.MainWindowHandle`）
- 取尺寸最大的匹配窗口作为真正的应用主窗口
- 检测尺寸异常时自动 `MoveWindow` 修复
- 调用 `Graphics.CopyFromScreen` 截图保存为证据
- 输出 JSON 报告：`{"status":"OK|FIXED|FAILED","hwnd":"0x...","width":N,"height":N,"screenshot":"..."}`

#### 验收脚本使用方式

```powershell
# 启动应用后执行
powershell -ExecutionPolicy Bypass -File scripts/verify-window.ps1

# 端到端测试（自动启动 + 验证 + 终止）
powershell -ExecutionPolicy Bypass -File scripts/test-window-flow.ps1
```

退出码：`0` = OK/FIXED，`1` = FAILED/NOT_FOUND。AI 可程序化解析退出码判断验证结果。

---

## 六、目录结构设计（最终部署目标）

```
F:\study\Palworld-Server-Manager\Palworld\          # 最终部署目录
├── src/                                             # 前端源码
│   ├── main.ts                                      # 入口
│   ├── App.vue                                      # 根组件
│   ├── style.css                                    # 全局样式 + --palwarm-* 变量
│   ├── router/
│   │   └── index.ts
│   ├── stores/
│   │   ├── server.ts
│   │   ├── config.ts
│   │   ├── network.ts
│   │   ├── rcon.ts
│   │   └── settings.ts
│   ├── api/
│   │   └── tauri.ts                                 # invoke 封装 + 类型
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppShell.vue
│   │   │   ├── Sidebar.vue
│   │   │   └── HeaderBar.vue
│   │   ├── ui/
│   │   │   ├── GlassPanel.vue
│   │   │   ├── StateBadge.vue
│   │   │   ├── BaseButton.vue
│   │   │   ├── Toast.vue
│   │   │   └── ConfirmDialog.vue
│   │   └── server/
│   │       ├── LogPanel.vue
│   │       └── PathDialog.vue
│   ├── views/
│   │   ├── DashboardView.vue
│   │   ├── ConfigView.vue
│   │   ├── NetworkView.vue
│   │   ├── RconView.vue
│   │   └── TroubleshootView.vue
│   └── types/
│       └── tauri.ts
├── src-tauri/                                       # Rust 后端
│   ├── src/
│   │   ├── main.rs                                  # 入口 + 命令注册
│   │   ├── server.rs                                # 【拆出】进程管理
│   │   ├── config.rs                                # 【拆出】配置读写
│   │   ├── firewall.rs                              # 【拆出】防火墙
│   │   ├── network.rs                               # 网络检测
│   │   ├── rcon.rs                                  # RCON 客户端
│   │   ├── settings.rs                              # 设置持久化
│   │   ├── steamcmd.rs                              # 【新增】SteamCMD
│   │   ├── backup.rs                                # 【新增】备份
│   │   └── tray.rs                                  # 【新增】托盘
│   ├── capabilities/
│   │   └── default.json
│   ├── icons/
│   │   └── icon.ico
│   ├── gen/                                         # Tauri 生成的 schema
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── build.rs
│   └── tauri.conf.json                              # 【新增】Tauri 配置
├── public/
│   └── favicon.ico
├── index.html                                       # Vite 入口
├── package.json
├── package-lock.json
├── tsconfig.json                                    # 【新增】TS 配置
├── tsconfig.node.json                               # 【新增】Node 端 TS 配置
├── vite.config.ts                                   # 【新增】Vite 配置
├── AGENTS.md                                        # 项目协作规范
├── README.md                                        # 【可选】使用说明
└── docs/                                            # 设计与实施文档
    ├── 01-Research-Analysis-Report.md
    ├── 02-Requirements-Specification.md
    ├── 03-Technical-Architecture.md
    └── 04-Implementation-Guide.md
```

---

## 七、主题与设计系统架构

### 7.1 CSS 变量层级

```css
/* src/style.css */

/* 第一层：基础调色板（来自 colors_and_type.css） */
:root {
  --palwarm-background: #f5ede2;
  --palwarm-foreground: #3f322c;
  --palwarm-card: rgba(255, 252, 247, 0.72);
  --palwarm-primary: #e66f51;
  --palwarm-primary-foreground: #ffffff;
  --palwarm-muted: rgba(255, 255, 255, 0.48);
  --palwarm-muted-foreground: #77675f;
  --palwarm-border: rgba(116, 88, 72, 0.14);
  --palwarm-input: rgba(255, 255, 255, 0.62);
  --palwarm-ring: rgba(230, 111, 81, 0.28);
  --palwarm-radius-sm: 12px;
  --palwarm-radius-md: 18px;
  --palwarm-radius-lg: 26px;
  --palwarm-state-success: #4f8a6b;
  --palwarm-state-warning: #b8782f;
  --palwarm-state-error: #c9554d;
  --palwarm-state-info: #4b7896;
  --palwarm-font-sans: Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", sans-serif;
  --palwarm-font-mono: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
}

/* 第二层：派生变量（来自 dashboard.html） */
:root {
  --palwarm-glass-edge: color-mix(in srgb, var(--palwarm-primary-foreground) 72%, transparent);
  --palwarm-soft-edge: var(--palwarm-border);
  --palwarm-glass-strong: color-mix(in srgb, var(--palwarm-card) 88%, transparent);
  --palwarm-glass-soft: color-mix(in srgb, var(--palwarm-card) 66%, transparent);
  --palwarm-primary-soft: color-mix(in srgb, var(--palwarm-primary) 12%, transparent);
  --palwarm-static-shadow: 0 18px 42px color-mix(in srgb, var(--palwarm-foreground) 5%, transparent);
  --palwarm-static-shadow-sm: 0 8px 22px color-mix(in srgb, var(--palwarm-foreground) 4%, transparent);
}

/* 第三层：全局基础样式 */
html, body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: var(--palwarm-font-sans);
  color: var(--palwarm-foreground);
  background:
    radial-gradient(circle at 7% 8%, color-mix(in srgb, var(--palwarm-primary) 18%, transparent), transparent 28rem),
    radial-gradient(circle at 90% 2%, color-mix(in srgb, var(--palwarm-primary) 10%, transparent), transparent 27rem),
    radial-gradient(circle at 63% 100%, color-mix(in srgb, var(--palwarm-state-warning) 10%, transparent), transparent 30rem),
    linear-gradient(145deg, color-mix(in srgb, var(--palwarm-background) 88%, white), var(--palwarm-background));
}

/* 玻璃面板通用类 */
.glass-panel {
  background: var(--palwarm-glass-soft);
  border: 1px solid var(--palwarm-glass-edge);
  box-shadow: var(--palwarm-static-shadow);
  backdrop-filter: blur(24px) saturate(145%);
  -webkit-backdrop-filter: blur(24px) saturate(145%);
}
```

### 7.2 组件视觉契约

| 组件 | 视觉规格 |
|------|----------|
| `GlassPanel` | `.glass-panel` 类，圆角 32px（panel 级） |
| `StateBadge` | 胶囊形（999px），10px 字号，状态色背景 9% 透明度 |
| `BaseButton` | 高度 40px，圆角 13px，主按钮渐变背景，次按钮 muted 背景 |
| `Card`（卡片） | `.glass-panel` + 圆角 26px，padding 18-20px |
| `Navitem` | 高度 46px，圆角 15px，active 态 primary-soft 背景 |
| `StatusDot` | 9×9px 圆点，2px 白边，3px 状态色光晕 |

---

## 八、错误处理与日志策略

### 8.1 前端错误处理
- **Tauri invoke 错误**：统一在 `api/tauri.ts` 捕获，转 `Error` 抛出
- **Store 错误**：捕获后调用 `toast.error(message)`
- **全局未捕获错误**：`app.config.errorHandler` 兜底 + console.error

### 8.2 后端错误处理
- **Tauri command 错误**：所有 command 返回 `Result<T, String>`，错误信息中文
- **Rust panic**：通过 `catch_unwind` 在关键路径捕获（可选）
- **后台线程错误**：日志收集线程的错误吞掉但记录到 stderr

### 8.3 日志策略
| 层级 | 日志方式 | 用途 |
|------|----------|------|
| 前端 | `console.log/warn/error` | 开发调试 |
| Rust | `eprintln!` | 开发调试 |
| 服务器 | stdout/stderr → 前端日志面板 | 用户可见 |
| 应用设置 | `%AppData%/PalworldServerManager/settings.json` | 持久化 |

---

## 九、性能优化策略

### 9.1 前端优化
- **路由懒加载**：5 个 View 组件 `() => import('...')`
- **日志虚拟滚动**：超过 200 行时启用虚拟列表（P2）
- **防抖轮询**：`get_server_status` 5s 间隔，页面不可见时暂停
- **响应式优化**：logs 使用 `shallowRef` 避免深度追踪

### 9.2 后端优化
- **日志缓冲区上限**：500 行自动滚动，防止内存膨胀
- **进程句柄及时释放**：`stop_server` 后 `child.wait()` 确保回收
- **RCON 连接复用**：保持长连接，避免反复 TCP 握手

### 9.3 构建优化
- **Vite 代码分割**：vendor + per-route chunk
- **Tree shaking**：lucide-vue-next 按需引入图标
- **Rust release 优化**：`[profile.release] lto = true, codegen-units = 1`

---

## 十、安全架构

### 10.1 Tauri 能力权限

```json
// src-tauri/capabilities/default.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for Palworld Server Manager",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:default",
    "core:webview:default",
    "core:event:default",
    "dialog:default",
    "dialog:allow-open"
  ]
}
```

### 10.2 CSP 策略
```
default-src 'self';
img-src 'self' data:;
style-src 'self' 'unsafe-inline';
script-src 'self';
connect-src 'self' ipc: http://ipc.localhost;
```

### 10.3 数据安全
- RCON 密码存储在 `%AppData%/PalworldServerManager/settings.json`（明文，未来可用 DPAPI 加密）
- 不上传任何用户数据到远程
- PowerShell 调用无用户输入直接拼接
