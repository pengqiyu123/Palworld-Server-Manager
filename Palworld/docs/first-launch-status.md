# 帕鲁专用服 · 首次开服实践状态

> 目的：把"在老板机器上真把帕鲁服开起来"这件事，按步骤**记录 → 检测 → 留状态 → 一步步完善 app 引导能力**。
> 创建：2026-07-21 ｜主理人 齐活林（齐活林）

---

## 一、当前环境检测（2026-07-21 实测，read-only）

| 项 | 检测路径 | 状态 |
|---|---|---|
| PalServer.exe | `E:\SteamLibrary\steamapps\common\PalServer\PalServer.exe` | ✅ 存在（182KB，Jul20 安装） |
| 默认配置模板 | `...\PalServer\DefaultPalWorldSettings.ini` | ✅ 存在（3781B） |
| 首次启动生成的配置 | `...\PalServer\Pal\Saved\Config\WindowsServer\PalWorldSettings.ini` | ✅ **已于 21:48 首次启动生成（但为空，仅 2 字节）** |
| Radmin VPN | （老板确认已装） | ✅ 已装（虚拟局域网，朋友直连用） |
| 防火墙 UDP 8211 | 服务器已监听 `UDP 0.0.0.0:8211`（PID 26768） | ⏳ 入站规则待放行（异地同学连之前） |

**结论**：服务端已安装但**从未启动过**。`PalWorldSettings.ini`（你真正要改的那份）还没生成——这正是首次开服最大的坑：必须先跑一次让它生成，且要在**停服状态**下改 `Saved\Config` 那份。

`app` 后端的 `steam_detect` 算法（读注册表 + 解析 `libraryfolders.vdf`）理论上能自动命中你这个 `E:\` 路径；当前前端是 `VITE_MOCK=true`，故 UI 还不会真调它。

---

## 二、如何建立（首次开服 5 步）

**0. 前置（已具备）**：PalServer.exe 已装（✅ E:\）、Radmin VPN 已装（✅）。

**1. 首次启动生成配置（关键）**
- 双击 `PalServer.exe`（或 `PalServer.exe -USEALLOWCOMMANDLine`）。
- 它**第一次生成** `Pal\Saved\Config\WindowsServer\PalWorldSettings.ini`，随后可能因默认空配置而退出/或起服。
- ⚠️ 此时**不要改配置**——运行中点改会被关机覆盖。

**2. 停服 + 改"对的那份" ini**
- 完全关闭 PalServer 进程。
- 打开 `Pal\Saved\Config\WindowsServer\PalWorldSettings.ini`（**不是**根目录的 `Default` 那份！）。
- 设关键项：
  - `ServerName=你的服名`
  - `ServerPassword=`（公网联机才需；Radmin 局域网可空）
  - `AdminPassword=强随机串`（RCON 用，**不是 ServerPassword**！）
  - `RCONEnabled=True`
  - `RCONPort=25575`
  - `ServerPlayerMaxNum=...`
  - `PublicPort=8211`（玩家连的 UDP 端口）
- 保存。

**3. 防火墙放行 UDP 8211**
- 玩家连服走 UDP 8211，Windows 防火墙需放行（入站）。
- RCON 25575 **切勿**对公网开放（仅本地/内网管理）。

**4. 启动服务端**
- 再跑 `PalServer.exe`，这次带你的配置起服。
- 本机进游戏 → 多人(专用) → 填 `127.0.0.1:8211` 直连验证。

**5. 朋友通过 Radmin 联机**
- 朋友装 Radmin VPN 并加入你的虚拟局域网。
- 游戏内填你的 **Radmin 虚拟 IP:8211**（非公网 IP，免端口映射，绕开家用宽带 CGNAT）。

---

## 三、状态看板

| 步骤 | 内容 | 状态 |
|---|---|---|
| 0 | 前置（exe / Radmin 已装） | ✅ 完成 |
| 1 | 首次启动生成 ini | ✅ **完成（21:48，服已监听 8211，生成的 ini 为空需填）** |
| 2 | 停服改 `Saved\Config` ini | ✅ **完成（23:19，停服→填配置→重启验证中）** |
| 3 | 防火墙 UDP 8211 放行 | ⏳ 待办 |
| 4 | 本机起服 + `127.0.0.1` 验证 | ✅ **完成（23:1x，更新客户端版本后连入，世界存档已生成）** |
| 5 | Radmin 朋友联机 | ⏳ 待办 |

---

## 四、变化记录（append-only）

- 2026-07-21 21:47 启动前快照：exe / Default ini 在，`Pal\Saved` 目录**不存在** → 判定尚未首次启动。Radmin 已装。
- 2026-07-21 21:48 **首次启动成功**（命令 `PalServer.exe -useperfthreads -NoAsyncLoadingThread -UseMultithreadForDS`，后台 task IgSD46）。实测结果：
  - **进程**：`PalServer-Win64-Shipping-Cmd.exe`（PID 26768，占用 ~1GB 内存）持续运行 = 服真起来了。
  - **端口**：已监听 `UDP 0.0.0.0:8211`（PID 26768）→ 用默认设置起服成功。
  - **新生成目录**：`Pal\Saved\`（含 `Config\WindowsServer\`（43 个 .ini）、`Config\CrashReportClient\`、`ImGui`、`Logs`）。
  - **🔥 关键坑坐实**：新生成的 `Config\WindowsServer\PalWorldSettings.ini` **只有 2 字节（一个空行 `^M`）= 完全空**！要改配置必须先把根目录 `DefaultPalWorldSettings.ini`（3781B）里 `[/Script/Pal.PalGameWorldSettings]` 那段完整内容复制进这份空 ini，才能改 ServerName/AdminPassword/RCON 等——直接改会没有任何键可改。
  - **注意**：`Logs` 目录当前为空（专用服日志可能延迟落盘或输出到控制台窗口）。
- **下一步**：停服（关掉 PID 26768）→ 复制 Default 内容进空 `PalWorldSettings.ini` → 设 AdminPassword/RCONEnabled=True/RCONPort=25575/ServerName 等 → 放行防火墙 UDP 8211 → 重启起服 → `127.0.0.1:8211` 自测。
- 2026-07-21 21:59 主理人复核：进程仍在（PID 26768，`PalServer-Win64-Shipping` ~1GB）、`UDP 0.0.0.0:8211` 仍监听；`Pal\Saved\Config\WindowsServer\` 下共 43 个 .ini 已生成（多为引擎默认空文件，仅 `Engine.ini`/`GameUserSettings.ini` 有内容），`PalWorldSettings.ini` 仍为 2 字节空文件 = 服以**全默认设置**运行中（无服名/无管理员密码/RCON 关）。
- 2026-07-21 22:0x 老板决定：**不停服，先本机自测能否进服**（选"先本机自测能进"）。服务器保持运行（PID 26768 / UDP 8211 监听 / 空配置=默认设置）。下一步由老板开游戏连 `127.0.0.1:8211` 验证；自测通过后回头做第 2 步（停服填配置）。
- 2026-07-21 22:15 本机自测**连接超时**（老板报"一直显示连接超时"）。主理人排查：
  - 服务端**健康**：PID 26768 仍在、UDP 0.0.0.0:8211 仍监听、内存 798MB（未崩）。
  - **无任何连接日志**：全树零 `.log`/零 crash 文件（含 `Saved/Logs` 空）——专用服默认不写连接日志，客户端超时在服务端不可见。
  - **🔥 头号嫌疑=版本不一致**：`appmanifest_2394010`(专用服) LastUpdated=1784540201/Jul20 17:36，而 `appmanifest_1623730`(游戏客户端) LastUpdated=1784390415/≈Jul18 23:50 → **客户端比服务器旧约 1.7 天**。帕鲁要求服/客同版本，旧客户端被拒→表现成超时。（诚实限定：两 appid 不同，Steam buildid 不可直接比；但更新时间差 1.7 天是真实信号。）
  - 防火墙：三配置全启用、无放行 PalServer/UDP8211 规则 → **不影响本机回环(127.0.0.1)连接**，但同学经 Radmin 来连时必卡（第 3 步待办）。
  - **待老板动作**：Steam 库里把「幻兽帕鲁」和「幻兽帕鲁 专用服务器」都更新到最新（重点更新旧的客户端），重启游戏后重试 127.0.0.1:8211。
- 2026-07-21 23:1x **本机自测成功**（老板更新客户端后连入 `127.0.0.1:8211`）。前后对比 diff：
  - **版本对齐**：客户端 `appmanifest_1623730` `LastUpdated` 从 `1784390415`(≈Jul18) → `1784646548`(≈Jul21 23:0x)，Steam 把客户端更新到当前 → 超时消失、能进。专用服 `2394010` `LastUpdated` 仍 `1784540201`(Jul20) 未变。
  - **服务器被重启**：PID 26768 → **46180**（`PalServer-Win64-Shipping`，新实例，Steam 更新时顺带重启）；内存 798MB → **1.19GB**（玩家连入→世界+存档加载）；`UDP 0.0.0.0:8211` 仍监听（PID 46180）。
  - **🔥 新生成世界存档**（首次启动时不存在）：`Pal\Saved\SaveGames\0\1A91A61548C7B6FD7B58B2B70710F7EE\` 下出现 `Level.sav`（世界存档）+ `LevelMeta.sav`（世界元数据）+ `backup\world\`（备份目录）。`1A91A615...` = 本局世界 GUID，`0` = 存档槽位。= 真实会话已落盘。
  - **Logs 仍空**：`Pal\Saved\Logs\` 跑了 1.5h 仍 0 文件 → **坐实帕鲁专用服默认不写磁盘日志**（只吐控制台），app 的"日志"功能后续要靠读控制台/重定向或 RCON，不能指望读 Logs 目录。
  - **PalWorldSettings.ini 仍 2 字节空**：你用的是**全默认配置**进来的（无服名/无 AdminPassword/RCON 关）。第 2 步待做：停服→复制 `DefaultPalWorldSettings.ini` 内容进空 ini→设 ServerName/AdminPassword/RCONEnabled=True→重启。
  - **CrashReportClient** 多了 2 个 `UECC-Windows-<hash>` 子目录（引擎崩溃上报客户端配置，正常）。
  - **结论**：本地链路完全跑通（服能起→能进→能存档）。下一步=第 2 步填配置（解锁 RCON/管理员），再第 3 步防火墙+第 5 步 Radmin 让同学连。
- 2026-07-21 23:15 **老板贴来服务器控制台输出**（=此前找不到的"连接记录"，它只吐控制台、不落盘）。关键信息：
  - **服务端游戏版本 = `v1.0.1.100619`**（AppID 2394010，breakpad minidump）。这是真正的游戏版本号，比 Steam buildid 更准——以后查版本不一致直接比这个。
  - `Running Palworld dedicated server on :8211` ✓
  - `[23:10:02] [LOG] 煜 127.0.0.1 connected the server. (User id: steam_76561199381352956)` —— 玩家"煜"（老板）从本机连入。
  - `[23:14:28] [LOG] 煜 joined the server. (User id: ..., Player id: 4E239D4F000000000000000000000000)` —— join 完成。**connect→join 间隔 ~4 分钟**（世界加载/建角色）。
  - **对 app 的重大启示**：控制台 stdout **就是**日志源——Rust 后端 `server.rs` 的 `start_server()` 用 `Command::spawn` 拉起 PalServer.exe 时已捕获子进程 stdout/stderr 管道 → **app 的"日志"功能应读这个管道**（不是去读空的 `Saved\Logs\` 目录）。此前 S4/日志功能以为能读 Logs 目录是错的，已纠正。
  - **SteamID `steam_76561199381352956` + PlayerID `4E239D4F...`** 正是 RCON 管理员指令（`KickPlayer`/`BanPlayer`）需要的标识 → app"玩家管理"页就用这俩 ID。
- 2026-07-21 23:19 **第 2 步：停服→填配置→重启** 执行：
  - **停服**：force kill 包装器 `PalServer.exe`(PID 43712) + 实体 `PalServer-Win64-Shipping`(PID 46180)，2s 后复查无进程、8211 释放（RCON 未开无法优雅关服，但玩家 23:16:39 已离线+存档已落盘，force kill 风险低）。
  - **填配置**（脚本 `docs/write-config.py`）：读 `DefaultPalWorldSettings.ini` 模板→去注释→精确替换 3 字段→写进 `Pal\Saved\Config\WindowsServer\PalWorldSettings.ini`（2B→3565B）。核验通过：`ServerName="煜的帕鲁世界"`(中文未乱码) / `AdminPassword="otGwh1JjCiEHx2Hx"`(16位随机) / `RCONEnabled=True` / `RCONPort=25575` / `ServerPassword=""`(Radmin 局域网空) / `PublicPort=8211` / `ServerPlayerMaxNum=32`。其余字段保持模板默认。
  - **管理员密码（明文，仅此一次）**：`otGwh1JjCiEHx2Hx` —— RCON 登录 + 游戏内管理员指令都用它，**不是 ServerPassword**。
  - **重启**：后台 `PalServer.exe -useperfthreads -NoAsyncLoadingThread -UseMultithreadForDS`（task 0YLv2W），待验证 8211 + **25575(RCON)** 双端口监听。
  - **关于"自动获取状态信息"**（老板问）：现在我个人**不能**直接读 PalServer 控制台窗口（[LOG] 行只在那个窗口、不进 bash stdout，因为 `PalServer.exe` 包装器会开新控制台给 `PalServer-Win64-Shipping-Cmd.exe`）。**但 app 能自动获取**——关键：app 的 Rust `server.rs` 应**直接 spawn `PalServer-Win64-Shipping-Cmd.exe`（Cmd 版）并捕获其 stdout 管道**（不要 spawn `PalServer.exe` 包装器，否则日志进新控制台拿不到），逐行解析 `[LOG] ... connected/joined/left` 即得玩家进出事件；再加 RCON `ShowPlayers` 可按需拉在线列表。本次 `server_stdout.log` 大概率只捕到启动 echo、捕不到 [LOG]（印证包装器问题）。
  - **重启验证通过**：新进程 `PalServer-Win64-Shipping`(PID 52964，932MB)；`UDP 0.0.0.0:8211` + **`TCP 0.0.0.0:25575 LISTENING`** 双端口监听 → **RCONEnabled=True 确被读取、RCON 服务已起来**。配置生效铁证。
  - **stdout 捕获验证**：`server_stdout.log` 为空 → 印证 `PalServer.exe` 包装器开新控制台给 shipping exe、[LOG] 不进 bash stdout。**app 设计结论**：Rust `server.rs` 要直接 spawn `PalServer-Win64-Shipping-Cmd.exe`(Cmd 版) 并捕获 stdout 才能拿到 [LOG] 行，不能 spawn `PalServer.exe` 包装器。
- 2026-07-21 23:36 **RCON 端到端验证通过**（老板贴服务器控制台 + 主理人 raw dump 双证）：
  - **服务器控制台日志**（老板贴）显式记录每次 RCON 命令被执行：`[23:33:01] RCON executed the command. Info` / `[23:36:16] RCON executed the command. ShowPlayers` → **auth 通过、命令送达、服务器执行**，全链路通。
  - **主理人 raw 字节 dump**（`docs/rcon-raw.py`）拿到 `ShowPlayers` 真实响应：`name,playeruid,steamid\n`（CSV 表头；因老板 23:33:14 已离线故无玩家行；在线时会出 `昵称,playeruid,steamid` 行）。= **app"玩家管理"页的数据源已跑通**。
  - **此前 Info "超时"是测试脚本 bug**（Info 回空 body，脚本跳过空包继续等→超时），非服务器/RCON 问题——控制台证明 Info 每次都被执行。咱项目 `rcon.rs` 的 `send_command` 只读 1 包、不处理空/多包响应，**后续要改**（参考 gorcon 库处理多包+空包）。
  - **协议 bug 复盘**：手写 Python 客户端首版只发 1 个 null（Valve RCON 要 2 个），咱 `rcon.rs` 第 87-88 行 `push(0);push(0)` 是对的。教训：RCON 用成熟库（Go=gorcon/rcon、Rust=rcon crate），别手写。
  - **老板控制台另含**：`[23:24:14] 煜 executed the command. AdminPassword otGwh1JjCiEHx2Hx`（游戏内管理员认证留痕）、`[23:24:00] connected` / `[23:24:07] joined`（第二次进服 connect→join 仅 7 秒，远快于首次 4 分钟，因世界已生成）。
- **参考项目 zaigie/palworld-server-tool 调研结论**（`reference-projects/zaigie-palworld-server-tool-vpn/`）：
  - **RCON**：用成熟库 `github.com/gorcon/rcon`（`rcon.Dial(addr,pass,timeout)`），非手写。支持 `UseBase64`（帕鲁 RCON 非 ASCII 用 base64 编解码）。
  - **🔥 REST API（更优路径）**：帕鲁自带 HTTP REST API（端口 8212，`RESTAPIEnabled`），Basic Auth，返回结构化 JSON：`GET /v1/api/info`(服名/版本) `/v1/api/metrics`(FPS/玩家数/在线时长/天数) `/v1/api/players`(在线玩家完整列表：name/userId/steamId/ip/ping/坐标/等级/建筑数)；`POST /v1/api/kick` `/ban` `/unban` `/announce`(广播) `/shutdown`(优雅关服,带秒数+消息) `/stop`。**比 RCON 的 CSV 文本更结构化、更适合 app 展示**。当前咱配置 `RESTAPIEnabled=False`→要开需改配置+重启。
  - **存档解析**：`sav_cli/` + `internal/tool/save.go` 直接解析 `Level.sav` 拿离线玩家/帕鲁/公会数据（RCON/REST 给不了的）。
  - **定时轮询**：`internal/task/rcon_task.go` 周期性 `ShowPlayers` 存 DB，UI 始终有新鲜数据——**app 不必解析控制台事件，轮询 REST/RCON 即可**。
- **同行设计调研结论**（2026-07-21 23:38，`reference-projects/` 两个项目）：
  - **zaigie/palworld-server-tool**（Go+Vue web）：4 主视图(概览/玩家/公会/地图) + 6 弹窗(RCON抽屉/广播/关服/备份/白名单/配置)；**60s 轮询** `getPlayerList+getServerMetrics`；JWT 登录态；header 显示服名+FPS+在线人数；i18n(zh/en/ja)；暗色模式。数据源=REST API(主) + RCON + sav 解析。
  - **amantu Tauri manager**（Tauri+React，同栈最近）：**分组侧栏** = Overview(Dashboard/Players/WorldMap/Trends) + Control(Console/Settings/BanManager) + About(Support) + **Server+(条件显示: ServerControl/Characters/Storage/Guilds/RawSave)**；**Connect 首屏**（未连接先显示连接页）；**Bridge 分层**=基础功能走 REST/RCON，高级(存档解析:角色/仓库/公会/RawSave)走一个"Tier-2 bridge"检测到才解锁；Trends 用 Sparkline/TimeSeriesChart 做监控时序图；Sidebar 可折叠(persist localStorage)。
  - **该抄的 5 条**：① 分组侧栏(Overview/Control 分组)比咱当前平铺清晰；② **Connect/首跑首屏**先连接再进主界面(对咱 S1 向导)；③ **轮询 REST/RCON**(60s)拿玩家+指标，别 parse 控制台事件；④ **REST API 为首选数据源**(结构化 JSON，两同行都这么用)；⑤ Bridge 分层=基础先做、存档解析(角色/仓库/公会)作为高级后做。
  - **可跳过/后做**：公会管理、白名单、世界地图、Trends 时序图、角色/仓库存档解析——都是高级/锦上添花，咱 v1 不必。
  - **咱 app 对照**：当前 S1-S4 四屏 + 占位屏，信息架构偏平铺；可借鉴 amantu 的分组+Connect 首屏；数据层翻 VITE_MOCK=false 后接 REST(开 RESTAPIEnabled) + RCON(已验证)。
- 2026-07-21 23:41 **REST API 开通并实测通过**（老板指令"开"）：
  - 停服(PID 46308+52964) → Python 改 `RESTAPIEnabled=False→True`（RESTAPIPort=8212 保持）→ 重启(PID 47072) → **三端口全开**：`UDP 8211`(游戏) + `TCP 25575`(RCON) + `TCP 8212`(REST)。
  - **curl 实测三接口**（Basic Auth `admin` + AdminPassword）全部返回干净 JSON：
    - `GET /v1/api/info` → `{"version":"v1.0.1.100619","servername":"煜的帕鲁世界","description":"","worldguid":"1A91A61548C7B6FD7B58B2B70710F7EE"}`（服名中文正常，worldguid 与 SaveGames 目录 GUID 一致）。
    - `GET /v1/api/metrics` → `{"currentplayernum":0,"serverfps":59,"serverfpsaverage":59.97,"serverframetime":16.79,"days":2,"maxplayernum":32,"basecampnum":0,"uptime":40}`。
    - `GET /v1/api/players` → `{"players":[]}`（老板已离线故空；在线时返回 name/userId/steamId/ip/ping/坐标/等级/建筑数 完整行）。
  - **结论**：app 数据源问题彻底解决——REST API 给结构化 JSON（比 RCON 的 CSV 文本好用），`/info` 填概览服名+版本、`/metrics` 填 FPS+在线人数+时长+天数、`/players` 填玩家表、`POST /kick /ban /unban /announce /shutdown /stop` 做管理动作。**这是 app 翻 VITE_MOCK=false 后的首选数据源**。
  - **REST API 认证**：HTTP Basic Auth，username=`admin`，password=`AdminPassword`(=otGwh1JjCiEHx2Hx)。RCON 仍并行可用(25575)。
  - **🔥 活体验证**（23:45 老板进服）：`GET /v1/api/players` 返回老板真实玩家行 `{"name":"煜","playerId":"4E239D4F...","userId":"steam_76561199381352956","iP":"127.0.0.1","ping":24.39,"location_x":-357712.84,"location_y":268755.63,"level":1}`；`/metrics` 的 `currentplayernum` 同步从 0→1。= **app"玩家管理"页数据源活体验证通过**（昵称/SteamID/坐标/等级/ping 全到位，踢人封人用 userId 调 POST /kick /ban）。
- （后续每次实践进展在此追加）

---

## 五、app 如何"一步步完善"来引导这件事

当前 app 前端是 **mock**（`VITE_MOCK=true`），后端命令是**真实实现**（steam_detect / server / config / rcon / firewall / network）。完善路线：

1. **翻 `VITE_MOCK=false`**：把 UI 接到真实后端（自动命中 E:\ 的 PalServer.exe）。
2. **S1 做成"首次开服向导"**：把上面 5 步变可勾选进度 + 每步人话解释 + 第 2 步"改错文件"的坑提示。
3. **S2 配置**：UI 编辑的就是 `Saved\Config` 那份 ini（确认 `config.write` 落点正确），暴露 `AdminPassword/RCONEnabled/ServerName…` 并显示"停服再改"警告。
4. **S3 网络**：显示 Radmin 已连状态 + 本地 IP + 8211 UDP 放行态，文案"朋友用 Radmin 虚拟IP:8211 直连，不用公网映射"。
5. **S4 RCON**：连 `127.0.0.1:25575`，注明"先在第 2 步开 RCON + 设密码"。
6. **后端缺口要补**：`start_server()` 现只硬编码 3 个性能参数，没把 UI 的 port/RCON/密码翻成启动参数；确认 `config.write` 落点是 `Saved\Config` 那份。
