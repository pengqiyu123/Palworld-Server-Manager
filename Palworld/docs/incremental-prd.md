# 帕鲁服务器管理器 · 增量 PRD（mock → 成品 app）

> 本轮目标：把 app 从「视觉还原 mock 前端」推进到「成品 app（本地真数据 + 异地联机引导）」。
> 定位：**Tauri 2 + Vue3 桌面单机管理器**，面向自建帕鲁专用服的服主，图形化完成开服 / 改配置 / 管玩家 / 引导朋友联机。
> 创建：2026-07-22 ｜ 产品经理 许清楚 ｜ 主理人 齐活林

---

## 一、产品目标

**一句话定位**：不用记命令行，一个桌面 app 就能开关帕鲁服、图形化改配置、看实时状态、管在线玩家、引导朋友异地联机。

**本轮范围边界（v1 成品）**：
- ✅ 翻 `VITE_MOCK=false`，S1–S4 全部接真实数据源（REST API 首选 + RCON 备份 + Rust 后端命令）。
- ✅ 新增「玩家管理」屏 + 全局 60s 轮询引擎 + Connect/首跑首屏。
- ✅ 本地真能用：老板本机已实测跑通首次开服（三端口全开 + REST/RCON 双通），app 直接对接这套真数据。
- ✅ 异地联机引导：app 做到「检测 + 引导 + 给同学连法说明」，真跨网建网由老板在 Radmin VPN 完成。
- ❌ 不做：公会 / 白名单 / 世界地图 / Trends 时序图 / 存档解析（见第六节）。

---

## 二、用户故事

1. **首次开服向导**：作为服主，我第一次打开 app，希望它自动找到 PalServer.exe、检测端口、引导我改配置并一键开服，而不是去查命令行和注册表。
2. **看服务器状态**：作为服主，服开起来后我希望一眼看到服名、版本、FPS、在线人数、游戏内天数、运行时长，不用盯控制台。
3. **管玩家**：作为服主，朋友进服后我希望看到谁在线（昵称/等级/ping），能一键踢捣乱的、封恶意的、全服广播通知，不用进游戏敲管理员指令。
4. **改配置**：作为服主，我希望图形化改 PalWorldSettings.ini（人数/经验/PVP/密码等），app 提醒我「停服再改」并帮我优雅关服→改→重启。
5. **异地联机引导**：作为服主，我希望 app 检测 Radmin 是否就绪 + 防火墙是否放行，然后给我一段能直接发给同学的「连法说明」（虚拟 IP:8211），同学照做就能进服。

---

## 三、功能池（P0/P1/P2）

> 字段说明：**优先级** P0 必做 / P1 应做 / P2 后做 ｜ **数据源** REST API / RCON / Rust 后端命令 / 纯前端 ｜ **来源** 抄自 zaigie / 抄自 amantu / 原创 ｜ **落地屏** S1 概览 / S2 配置 / S3 网络 / S4 RCON / S5 玩家管理(新) / 全局

### P0 · 必做（成品 app 本地真能用）

| # | 功能 | 优先级 | 数据源 | 来源 | 落地屏 | 描述 |
|---|---|---|---|---|---|---|
| 1 | 翻 VITE_MOCK=false + REST client | P0 | REST API + Rust | 原创 | 全局 | api.ts 新增 REST 调用层（Basic Auth `admin`+AdminPassword），各 store 从 mock 切真数据；建议走 Rust `rest_proxy` 代理避免明文密码暴露前端 |
| 2 | Connect/首跑首屏 | P0 | Rust steam_detect + server | 抄 amantu | S1 | 未探测到 PalServer.exe 或未起服时，S1 显示首跑向导（定位服务器→检查端口→启动）；已起服则进入仪表盘 |
| 3 | 概览仪表盘 | P0 | REST API(`/info`+`/metrics`) | 抄 zaigie+amantu | S1 | 显示 服名/版本/世界GUID(`info`) + FPS/平均FPS/在线人数/最大人数/天数/运行时长(`metrics`) + 进程状态(server.getStatus) |
| 4 | 服务器启停 | P0 | Rust server | 原创 | S1/S4 | 一键开服(api.server.start，PalServer.exe)；停服走 REST `/shutdown`(优雅，带秒数+消息)兜底 force kill |
| 5 | 60s 轮询引擎 | P0 | REST API | 抄 zaigie | 全局 | 周期拉 `/info`+`/metrics`+`/players` 写入 store，UI 自动刷新；服停时自动停止轮询，避免无意义报错 |
| 6 | 玩家管理-在线列表 | P0 | REST API(`/players`) | 抄 zaigie+amantu | S5(新) | 表格：昵称/SteamID(userId)/等级/ping/坐标/建筑数；随轮询自动刷新 |
| 7 | 玩家管理-踢人/封人 | P0 | REST API(`/kick` `/ban`) | 抄 zaigie | S5 | 点玩家行→踢/封（body `{userid}`）；二次确认弹窗防误操作 |
| 8 | 玩家管理-广播 | P0 | REST API(`/announce`) | 抄 zaigie | S5 | 输入框 + 发送（body `{message}`），全服玩家收到游戏内公告 |
| 9 | 配置编辑-真实读写 | P0 | Rust config | 原创改进 | S2 | 接 api.config.read/write，落点=`Saved\Config\WindowsServer\PalWorldSettings.ini`；暴露 ServerName/AdminPassword/RCONEnabled/RCONPort/RESTAPIEnabled/RESTAPIPort/PublicPort/ServerPlayerMaxNum + 玩法项(经验/PVP/采集等) |
| 10 | RCON 终端-真实连接 | P0 | RCON | 原创改进 | S4 | 接 api.rcon.connect/send（**需修 rcon.rs 多包 bug**）；连 127.0.0.1:25575 + AdminPassword；快捷按钮 Info/ShowPlayers/Broadcast/SaveWorld/Shutdown |
| 11 | 网络状态-真实检测 | P0 | Rust network+firewall | 原创改进 | S3 | 接 api.network.checkRadminLan + api.firewall.check；显示 Radmin installed/virtual_ip/adapter_status + 本地IP + UDP8211/TCP25575/TCP8212 三端口放行态 |
| 12 | 防火墙一键放行 | P0 | Rust firewall | 原创 | S3 | api.firewall.addRules 一键放行 UDP 8211（+可选 TCP 25575 仅内网）；放行后刷新状态徽标 |
| 13 | 异地联机引导 | P0 | Rust network + 纯前端 | 抄同行+实践 | S3 | 详见第四节专项设计：检测 Radmin/防火墙→给同学连法说明卡（见第四节） |
| 14 | 优雅关服 | P0 | REST API(`/shutdown`) | 抄 zaigie | S1/S4/S5 | POST `/shutdown` body `{waittime, message}` → 倒计时存档 → 退出；比 force kill 安全，避免丢档 |

### P1 · 应做（体验完善）

| # | 功能 | 优先级 | 数据源 | 来源 | 落地屏 | 描述 |
|---|---|---|---|---|---|---|
| 15 | 实时日志激活 | P1 | Rust server(stdout) | 原创改进 | logs屏 | 读 server.rs stdout 管道（**需改 start_server 直接 spawn `PalServer-Win64-Shipping-Cmd.exe` 捕获 [LOG] 行**，不 spawn 包装器）；已有 server-log 事件 + LogPanel 组件 |
| 16 | 服务器状态全局徽标 | P1 | REST API(轮询) | 抄 zaigie | 侧栏/标题栏 | 侧栏状态卡 + 标题栏显示 服名 + 在线人数 + FPS 小徽标 |
| 17 | 配置预设套用 | P1 | Rust presets | 原创改进 | S2 | api.presets.list/apply（后端已有）；一键套用「休闲/硬核/原版」预设，合并到当前 config |
| 18 | 配置备份恢复 | P1 | Rust config | 原创改进 | backup屏 | api.config.listBackups/restoreBackup（后端已有）；写配置前自动备份一份，支持回滚 |
| 19 | RCON 作为 REST 备份 | P1 | RCON | 原创 | S4/S5 | REST 不可用时降级走 RCON `ShowPlayers`(CSV)/`KickPlayer`/`BanPlayer`；两者数据归一 |
| 20 | 停服警告弹窗 | P1 | 纯前端 + Rust server | 原创 | S2 | 运行中改配置，弹「需停服生效，是否 停服→改→重启」向导；防止改了被运行时覆盖（first-launch 实测坑） |
| 21 | 同学连法说明卡 | P1 | Rust network + 纯前端 | 原创 | S3 | 可复制文案「在游戏里填 `<Radmin虚拟IP>:8211` 直连」+ 一键复制按钮；附 Radmin 装包/加网图文步骤 |
| 22 | 玩家离线/历史记录 | P1 | REST API(`/players`) | 抄 zaigie | S5 | REST `/players` 会返回近期曾上线玩家（含离线）；本地记录最后在线时间 |

### P2 · 后做（明确本轮不做）

| # | 功能 | 优先级 | 来源 | 不做原因 |
|---|---|---|---|---|
| 23 | 公会管理 | P2 | 抄 zaigie | 需存档解析，v1 不做 |
| 24 | 白名单 | P2 | 抄 zaigie | 需存档解析，v1 不做 |
| 25 | 世界地图（玩家位置可视化） | P2 | 抄 zaigie+amantu | 需坐标→地图映射，锦上添花 |
| 26 | Trends 时序图/Sparkline | P2 | 抄 amantu | 需持久化时序数据 + 图表库，v1 不做 |
| 27 | 角色/仓库存档解析 | P2 | 抄 zaigie+amantu(Bridge Tier-2) | 需 sav_cli 解析 Level.sav，复杂度高 |
| 28 | RawSave 编辑 | P2 | 抄 amantu | 高风险，v1 不做 |
| 29 | i18n 多语言 | P2 | 抄 zaigie | 本轮中文优先 |
| 30 | 暗色模式 | P2 | 抄 zaigie | 视觉轮已定调，本轮不改 |
| 31 | JWT 登录态 | P2 | 抄 zaigie | 咱是本地桌面 app，无需 Web 登录态 |

---

## 四、异地联机引导设计（S3 网络页）

> 设计原则：**app 做到「检测 + 引导 + 给连法」，真跨网建网由老板在 Radmin VPN 完成**。app 不替你建网，只检测就绪态 + 生成可发给同学的说明。

### S3 页面结构（三块）

```
┌─────────────────────────────────────────────────┐
│ ① 端口放行态（三张 PortCard）                     │
│   UDP 8211（游戏）  TCP 25575（RCON）  TCP 8212（REST）│
│   每卡显示：协议/方向/当前放行态  [一键放行] 按钮     │
├─────────────────────────────────────────────────┤
│ ② Radmin VPN 状态卡                              │
│   已装?  虚拟 IP: 10.x.x.x  适配器状态: Up/Down    │
│   未装→提示去 radmin-vpn.com 下载                 │
├─────────────────────────────────────────────────┤
│ ③ 异地联机引导（4 步可勾选进度卡）                  │
│   Step A 放行防火墙 UDP 8211  [检测][一键放行]     │
│   Step B Radmin 已装并连入虚拟局域网 [检测]         │
│   Step C 把你的虚拟 IP:8211 发给同学 [一键复制]     │
│   Step D 同学装 Radmin+加网+游戏内填 IP:8211        │
└─────────────────────────────────────────────────┘
```

### 引导步骤明细

- **Step A · 放行防火墙 UDP 8211**：app 调 `api.firewall.check` 检测放行态；未放行→「一键放行」按钮调 `api.firewall.addRules`。文案强调：游戏走 UDP 8211，Windows 防火墙默认拦入站，不放行同学连不上。
- **Step B · Radmin VPN 就绪**：app 调 `api.network.checkRadminLan` 检测 `installed` + `adapter_status`；显示当前虚拟 IP。未装→引导下载链接；已装未连→提示「在 Radmin 客户端里创建/加入一个网络」。
- **Step C · 发连法给同学**：生成可复制文案，例如：
  > 朋友连我帕鲁服：①装 Radmin VPN（radmin-vpn.com）②我拉你进我的虚拟网络 ③进游戏→多人→专用服务器→填 `10.x.x.x:8211` 直连。不用公网映射，不用端口转发。
  「一键复制」按钮复制到剪贴板（用 `tauri_plugin_clipboard_manager`，已装）。
- **Step D · 同学端操作**：纯说明卡，图文步骤（装 Radmin→加网→游戏内填 IP:8211）。

### 关键约束（文案必须传达）

1. **Radmin 虚拟局域网免公网映射**：绕开家用宽带 CGNAT，同学直连你的虚拟 IP，不用折腾路由器端口转发。
2. **RCON 25575 切勿对公网开放**：仅本地/内网管理用；Radmin 网内也建议只对本机开放。
3. **ServerPassword 在 Radmin 局域网可空**：信任网内成员则不设服密码；公网联机才必须设。
4. **服/客版本必须一致**：first-launch 实测客户端比服务器旧 1.7 天→连接超时；引导卡提示「若同学连不上，先确认游戏和专用服都更新到最新」。

---

## 五、待确认问题（给老板拍板）

1. **REST API 调用层放前端 fetch 还是走 Rust 代理？**
   - 建议：走 Rust `rest_proxy` 命令代理。理由：①Basic Auth 明文密码不暴露在前端 JS；②绕开浏览器 CORS；③Rust 侧可统一缓存/重试。代价：多写一个 Rust 命令。
   - 若直接前端 fetch：省一个后端命令，但 AdminPassword 明文出现在前端，且需处理 CORS。
   - **请老板定：要安全（Rust 代理）还是要快（前端直连）？**

2. **start_server 是否改为直接 spawn `PalServer-Win64-Shipping-Cmd.exe`（Cmd 版）以捕获 stdout 日志？**
   - first-launch-status 实测：spawn `PalServer.exe` 包装器会开新控制台给 shipping exe，[LOG] 行不进 bash stdout，app 拿不到日志。
   - 改法：直接 spawn Cmd 版 + 捕获 stdout 管道，解析 `[LOG] ... connected/joined/left` 得玩家进出事件。
   - 但若 REST 60s 轮询已足够拿玩家列表，stdout 日志可降为 P1（只做日志展示，不靠它拿玩家）。
   - **请老板定：日志功能 P0 还是 P1？是否本轮就改 spawn Cmd 版？**

3. **本轮是否只支持 Radmin 虚拟局域网联机，还是也要支持公网联机（端口映射/UPnP）？**
   - first-launch-status 默认 ServerPassword 空（Radmin 局域网场景）。
   - 若只做 Radmin：S3 引导聚焦虚拟局域网，ServerPassword 默认空。
   - 若也要公网：需额外做端口映射检测/UPnP 自动转发，ServerPassword 必填校验。
   - **请老板定：本轮联机只做 Radmin，还是 Radmin + 公网都做？**

4. **RCON 多包 bug 修复优先级是 P0 还是 P1？**
   - 实测：`rcon.rs` 的 `send_command` 只读 1 包、不处理空/多包响应（Info 回空 body 会超时）。
   - 若 REST API 作为首选数据源（已实测通），RCON 可降为 P1 备份通道，bug 可缓修。
   - 若要 RCON 终端(S4)真能用（Info/ShowPlayers），bug 必须 P0 修。
   - **请老板定：S4 RCON 终端 P0 真用 还是 P1 备份？**

5. **玩家「封人」后的封禁名单存哪？**
   - REST `/ban` 无列表查询接口；解封靠 `/unban` 需 userId。
   - 选项：①app 本地维护封禁名单（settings.json）②不做封禁名单管理，只做封/解封动作。
   - **请老板定：要不要本地封禁名单？**

## 五·补 决策记录（老板 2026-07-21 23:56 拍板，全按主理人建议）

1. **REST client 走 Rust 代理**（非前端直连）。AdminPassword 不进前端 JS，绕开 CORS，与 `rcon.rs` 模式一致。
2. **start_server 改 spawn Cmd 版捕获 stdout 日志 = P1**（非 P0）。REST 60s 轮询已覆盖玩家/指标核心状态，控制台事件缓做。
3. **本轮联机只做 Radmin**（不做公网端口映射/UPnP）。ServerPassword 默认空。
4. **RCON 多包 bug 修复 = P1**（非 P0）。REST 已覆盖所有 P0 管理动作；S4 RCON 终端降为 P1，rcon.rs 修复跟着 P1。**P0 不依赖 rcon.rs**。
5. **封禁名单只做动作不做名单**（不本地维护）。app 只调 `/ban` `/unban`，ban list 交给服务器(BanListURL)。

**本轮 P0 范围（锁定）**：Rust 代理 REST + Connect 首屏(双模式) + 概览仪表盘(info/metrics) + 服务器启停 + 60s 轮询 + 玩家管理(在线表/踢人/封人/广播) + 配置真实读写 + 网络真实检测 + 防火墙一键放行 UDP 8211 + 异地联机引导(Radmin) + 优雅关服。RCON 终端/控制台日志/公网联机/本地封禁名单 = P1+。

---

## 六、范围外明确（本轮不做）

- **公会管理 / 白名单 / 世界地图 / Trends 时序图 / 角色仓库存档解析 / RawSave 编辑**：均为 P2，需存档解析或图表库，本轮不做（见功能池 P2）。
- **公网联机（端口映射 / UPnP 自动转发）**：本轮只做 Radmin 虚拟局域网联机引导。
- **多服管理**：单服，一个 app 管一个 PalServer 实例。
- **PalServer 自动更新 / SteamCMD 集成**：手动更新，app 不管。
- **跨平台**：仅 Windows（PalServer 仅 Win）。
- **玩家位置地图可视化 / 帕鲁图鉴 / 建筑统计**：锦上添花，后做。
- **i18n / 暗色模式 / JWT 登录态**：本轮中文 + 现有视觉调性 + 本地 app 免登录。

---

## 附：数据源与后端缺口对照

| 数据源 | 状态 | 备注 |
|---|---|---|
| REST API(8212) | ✅ 已实测三接口通 | 首选数据源，Basic Auth admin+AdminPassword |
| RCON(25575) | ✅ auth+命令通，⚠️ send_command 有多包 bug | 备份数据源，需修 |
| Rust steam_detect | ✅ 实测命中 E:\ | 首跑定位 |
| Rust server | ✅ start/stop/status/logs 实现 | ⚠️ start_server spawn 包装器拿不到 stdout 日志（见待确认 2） |
| Rust config | ✅ read/write/default/descriptions/backups | 落点须确认 = Saved\Config 那份 |
| Rust firewall | ✅ check/addRules | 一键放行 UDP 8211 |
| Rust network | ✅ checkRadminLan/checkPortUsage | Radmin 检测 + 端口占用 |
| Rust presets/settings | ✅ 已实现 | 预设套用 + app 设置持久化 |
| **缺口：REST client 层** | ❌ 待新增 | Rust `rest_proxy` 或前端 fetch（见待确认 1） |

---

*本 PRD 基于 `docs/first-launch-status.md` 实测留痕 + `reference-projects/` 同行调研结论撰写。所有数据源、端口、密码均为老板本机真实验证值。*
