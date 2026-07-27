# 联机版本统一检测报告（从联机补丁入手）

> 检测时间：2026-07-22 21:xx
> 检测手法：绕过 300MB 游戏本体 exe，直接从 **online-fix 联机补丁包 `pal123`** 入手（文件少、可读、含版本/身份锚点），并比对服务端 `PalServer` 真实日志。
> 前提（老板已确认）：你与同学的 **Palworld 游戏本体版本一致**，故本体 exe 不再比对。
> 适用范围：仅联机前的版本/身份一致性排查，非软件交付物。

---

## 1. 检测对象与角色定位

| 路径 | 角色 | 性质 |
|------|------|------|
| `E:\SteamLibrary\steamapps\common\pal123` | **联机补丁包（online-fix）** | 伪造 Steam 身份让非正版/特定客户端能进联机。含 `SteamFix64.dll` + `EpicFix64.dll` + `steam_api64.dll` + 两个 `.ini` |
| `E:\SteamLibrary\steamapps\common\Palworld` | 游戏客户端本体（正版） | 你已确认与同学版本一致 |
| `E:\SteamLibrary\steamapps\common\PalServer` | 专用服务端（正版） | 对外提供 8211 游戏端口；日志在 `Pal/Binaries/Win64/logs/` |

> ⚠️ `pal123` **不是游戏本体**（仅 2.3MB，只有 `Engine/Steamworks/Steamv153/Win64` 的 3 个 dll + `Pal/Binaries/Win64` 的 2 个 ini），它只是一套联机补丁。

---

## 2. 联机补丁 `pal123` 完整文件清单与时间线

```
pal123\
├─ Engine\Binaries\ThirdParty\Steamworks\Steamv153\Win64\
│   ├─ EpicFix64.dll       640,512 B   2026-03-24 23:50
│   ├─ steam_api64.dll     288,768 B   2026-03-24 23:59   ← 补丁版(伪造)
│   └─ SteamFix64.dll    1,431,552 B   2025-09-10 22:38   ← online-fix 主程序(极旧)
└─ Pal\Binaries\Win64\
    ├─ SteamFix.ini           287 B   2026-07-10 14:07   ← 核心配置
    └─ EpicFix.ini            18 B   2026-01-06           ← EOS 配置
```

### 2.1 `SteamFix.ini`（补丁核心配置，完整摘录）
```ini
[Main]
RealAppId=1623730        ; Palworld 客户端真实 appid
FakeAppId=480            ; 对外伪装成 Valve SpaceWar 演示(480)，绕过版本/鉴权检查
Language=schinese
BuildId=0                ; 不锁定具体构建版本 → 跨版本联机的关键
[Misc]
Overlay=true
UnlockAllDLC=false
ShowOnlyPiratedServers=false
[Interfaces]
Apps=True / User=True / Stats=True / Storage=True / Utils=True
Workshop=False / Inventory=True / Friends=True
[FreeTP]
Id=5498
```

### 2.2 `EpicFix.ini`
```ini
[EOS]
NoAuth=true               ; 关闭 Epic 在线鉴权，让 Epic 版联机放行
```

---

## 3. 服务端 `PalServer` 关键证据

### 3.1 联机身份
- `Pal/Binaries/Win64/steam_appid.txt` 内容 = **`1623730`**
  - ⚠️ 官方正版 PalServer 的 appid 应为 **2394010**；此处写成 **1623730（客户端 appid）**，说明该服务端被配置成"以客户端身份加入联机网络"。

### 3.2 `steam_api64.dll` 时间线对比
| 位置 | 时间 | 说明 |
|------|------|------|
| 客户端本体(Palworld) | **2026-07-21 22:20** | 你最新更新 |
| 服务端本体(PalServer) | **2026-07-20 17:29** | 比客户端早 **1 天** |
| 补丁版(pal123) | **2026-03-24** | 伪造版，仅客户端侧 |

### 3.3 服务端真实启动日志（已发生 5 次）
`Pal/Binaries/Win64/logs/connection_log_8211.txt`：
```
[2026-07-21 21:48:04] Client version: no bootstrapper found
[2026-07-21 21:48:04] IPv6 HTTP connectivity test ... TIMEOUT
[2026-07-21 21:48:04] IPv6 UDP connectivity test ... FAILED, no addresses resolved
[2026-07-21 23:09:22] ... (同上)
[2026-07-21 23:20:26] ... (同上)
[2026-07-21 23:42:35] ... (同上)
[2026-07-22 20:43:33] ... TIMEOUT   ← 注意这次带 (127.0.0.1:7897) 代理
```

`Pal/Binaries/Win64/logs/appinfo_log.txt`：
```
failed to open file: .../appcache/appinfo.vdf   ← 每次都读不到 Steam appinfo 缓存
```

**解读**：
- 服务端在 7/21 晚到 7/22 晚确实反复启动过 5 次，但 **connection_log 里没有任何客户端连入记录** → 你同学那边根本没连进来。
- `Client version: no bootstrapper found` + `appinfo.vdf` 缺失 = 服务端 Steam 网络初始化时探活失败（非致命，但说明服务端未处于正常 Steam 在线态）。
- IPv6 探测失败 = 服务端机器无 IPv6 地址（常见于仅 IPv4 + Radmin VPN 环境）。
- **7/22 20:43 那次带 `127.0.0.1:7897`** = 服务端进程继承了系统 `http_proxy` 环境变量，**代理污染**会干扰对外 Steam 验证/握手。

---

## 4. 需要统一的清单（按优先级）

### 🔴 P0 致命（直接导致连不进来）
| # | 不统一点 | 证据 | 统一动作 |
|---|----------|------|----------|
| **U1** | **联机网络身份不一致**：客户端 `FakeAppId=480` ↔ 服务端 `steam_appid.txt=1623730` | 见 §2.1 / §3.1 | 两端必须进入**同一 Steam 联机网络**。要么服务端也走 online-fix 伪装（同 FakeAppId 体系），要么放弃 FakeAppId 改 Radmin VPN 局域网直连 + 服务端独立鉴权。**这是 connection_log 零连入的根因** |
| **U2** | **服务端缺联机补丁**：`pal123` 的 `SteamFix64.dll`/`EpicFix64.dll` 未注入 PalServer | 服务端 `Steamworks/Steamv153/Win64/` 下无补丁 dll（§3.2 对比） | 若拓扑要求服务端也进 online-fix 网络，需把补丁 dll + ini 一并拷入 PalServer 对应目录 |

### 🟡 P1 警告（可能造成握手/版本失败）
| # | 不统一点 | 证据 | 统一动作 |
|---|----------|------|----------|
| **U3** | **SteamFix64.dll 版本极旧**（2025-09-10，早于游戏本体 10 个月） | §2 时间线 | online-fix 主程序若不支持 7/21 游戏新版的握手协议，补丁会失效。建议去 online-fix 源站核对是否有针对当前游戏版本的更新补丁 |
| **U4** | **服务端 `steam_api64.dll` 比客户端早 1 天**（7/20 vs 7/21） | §3.2 | 建议将服务端更新到与客户端同一天版本（Steam 验证 PalServer 时版本差可能触发 mismatch） |
| **U5** | **服务端启动代理污染**（7/22 带 `127.0.0.1:7897`） | §3.3 | 启动 PalServer 前执行 `set http_proxy=` / `set https_proxy=` 清空代理，避免对外验证走代理失败 |

### 🟢 已一致 / 无需动
| # | 项目 | 状态 |
|---|------|------|
| U6 | `BuildId=0`（不锁版本） | ✅ 两端均保持 0，勿改具体数字 |
| U7 | `Language=schinese` | ✅ 一致 |
| U8 | 客户端游戏本体版本（你 vs 同学） | ✅ 老板已确认一致 |
| U9 | IPv6 自检失败 | ⚠️ 非致命，Radmin VPN 走 IPv4 直连可忽略 |

---

## 5. 待老板拍板的拓扑决策（决定 U1/U2 怎么修）

当前证据指向 **两种可能拓扑**，修法完全不同，需老板确认实际搭法：

- **拓扑 A（online-fix 伪造网络）**：你与同学都靠 `FakeAppId=480` 进同一伪造 Steam 网络，服务端也必须打同款补丁并以相同身份加入。→ 修法：U1 让服务端 `steam_appid.txt` 与客户端 FakeAppId 对齐；U2 把补丁 dll+ini 注入 PalServer。
- **拓扑 B（Radmin VPN 局域网直连）**：Radmin VPN 组虚拟局域网，同学直接 IP 连你机器 8211，不走 Steam 公网大厅。→ 修法：客户端**不必**伪装 480，服务端用正版 2394010 独立鉴权，U1/U2 反而不该做（伪装反而坏事）。

> 鉴于 `steam_appid.txt=1623730`（客户端 appid 而非 2394010 服务端 appid），当前服务端配置**偏向拓扑 A 的"伪装"思路**，但又**没注入补丁 dll**——处于"半伪装"的断裂态，所以连不进来。

---

## 6. 下一步建议（老板联机前动作）

1. **先确认拓扑 A 还是 B**（§5），再决定 U1/U2 修法——这是头号 blocker。
2. 若走拓扑 A：核对 online-fix 源站是否有针对 7/21 游戏版本的 **SteamFix64.dll 更新**（U3），并注入 PalServer。
3. 启动 PalServer 前 **清空 http_proxy/https_proxy**（U5）。
4. 重新启动服务端后，盯 `logs/connection_log_8211.txt`：若出现你同学 IP 的连入记录 + `/players` 出现 2 条真实行 = 联机成功（对应 `finale-prd.md` 的 D1 验收）。
5. 本检测报告与 `finale-prd.md` / `finale-design.md` 配套，待联机成功后归档进 `finale-status.md`。

---

## 附：原始证据索引
- 补丁配置：`pal123/Pal/Binaries/Win64/SteamFix.ini`、`EpicFix.ini`
- 补丁 dll：`pal123/Engine/Binaries/ThirdParty/Steamworks/Steamv153/Win64/*.dll`
- 服务端身份：`PalServer/Pal/Binaries/Win64/steam_appid.txt`
- 服务端日志：`PalServer/Pal/Binaries/Win64/logs/connection_log_8211.txt`、`appinfo_log.txt`
