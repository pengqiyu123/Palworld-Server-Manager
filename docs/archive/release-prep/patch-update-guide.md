# 联机补丁更新 + 直连诊断指引

> 生成时间：2026-07-22 21:0x
> 背景：同学经 Radmin VPN 直连进不来，老板疑"补丁太旧"。
> 核心结论：**Radmin VPN 局域网直连模式下，online-fix 补丁新旧不是进不来的主因**，先做 §1 四要素自查，再决定是否更补丁。

---

## ⚠️ 我能 / 不能做什么（边界）
- ❌ **我无法替你联网下载补丁**：online-fix 补丁只能从其源站手动下；当前环境代理受限，且第三方包来源安全须你自己把关。
- ✅ **你能下好包丢进 `pal123` 覆盖后叫我**：我用同一套方法核对 dll 时间线 + 读服务端 `connection_log_8211.txt` 确认连入。
- ✅ **不更补丁也能做的诊断**（§1）立即可执行。

---

## 1. 立即可做：Radmin VPN 直连四要素自查（不依赖补丁）

> 直连 = 同学游戏直接 UDP 连你机器 `Radmin虚拟IP:8211`，绕过 Steam 公网大厅。

| # | 要素 | 怎么查 | 不通的现象 |
|---|------|--------|--------------|
| D1 | **两边 Radmin VPN 虚拟 IP 互通** | 你机器上 `ping <同学RadminIP>`，同学也 `ping <你RadminIP>` | 一方 ping 不通 → 虚拟网没组上 / 一方未连 Radmin |
| D2 | **服务端真在监听 8211** | 你机器 `netstat -ano | findstr 8211` 看有无 `0.0.0.0:8211` 或 `25.x.x.x:8211` LISTENING | 无监听 → 服务端没起 / 崩了（看 `logs/`） |
| D3 | **Windows 防火墙放行 8211 UDP** | 防火墙高级设置 → 入站规则放行 `PalServer-Win64-Shipping-Cmd.exe` 或 UDP 8211 | 监听在但同学连不上 → 多半被防火墙挡 |
| D4 | **同学游戏用"直接连接"填你的 Radmin 虚拟 IP:8211** | 游戏内多人→直接连接，填 `你RadminIP:8211` | 填错 IP（填了公网 IP / 127.0.0.1）→ 连不到你服务端 |

> 若 D1–D4 全过仍进不来，才回头怀疑联机身份层（见 `version-unify-check.md` 的 U1）。

---

## 2. 启动服务端前必做：清空代理（U5）

上次 `connection_log_8211.txt` 的 7/22 那次启动，**服务端进程继承了 `http_proxy=127.0.0.1:7897`**，会干扰对外验证/握手。

启动 `PalServer-Win64-Shipping-Cmd.exe` 前，在它那个命令行里先执行：
```bat
set http_proxy=
set https_proxy=
set HTTP_PROXY=
set HTTPS_PROXY=
```
再拉起服务端。

---

## 3. 若确认需更新补丁（online-fix 大厅模式才需要）

> 仅当你实际是"online-fix 大厅联机"（非纯 Radmin 直连）且怀疑补丁不支持 7/21 游戏新版时。

手工步骤（你来做）：
1. 去你常用的 online-fix 源站，找 **匹配 2026-07-21 之后游戏版本** 的 Palworld 联机补丁包。
2. 覆盖 `E:\SteamLibrary\steamapps\common\pal123\` 下的：
   - `Engine\Binaries\ThirdParty\Steamworks\Steamv153\Win64\SteamFix64.dll`（主程序，当前 2025-09-10 极旧）
   - 同目录 `steam_api64.dll`、`EpicFix64.dll`
   - `Pal\Binaries\Win64\SteamFix.ini`、`EpicFix.ini`
3. **保留 `SteamFix.ini` 的 `RealAppId=1623730` / `FakeAppId=480` / `BuildId=0` / `Language=schinese`** 配置（除非新包明确要改）。
4. 覆盖后**叫我**，我比对 dll 时间线 + 读 `steam_appid.txt` 与 `connection_log` 确认。

> 注：当前补丁 `SteamFix64.dll` 是 **2025-09-10**，比游戏本体早 10 个月——若你确走 online-fix 大厅，这确实可能不支持 7/21 新版握手，值得更。但**纯 Radmin 直连不需要它**。

---

## 4. 更新/修复后验收（联机成功标志）

盯两个证据，对应 `finale-prd.md` 的 D1：
1. `PalServer\Pal\Binaries\Win64\logs\connection_log_8211.txt` 出现**你同学 IP/虚拟IP 的连入记录**（不再只有 `no bootstrapper found`）。
2. 服务端 `/players` 命令返回 **≥2 条真实行**（你 + 同学）。

满足即联机成功，可归档进 `finale-status.md`。

---

## 附：与既有文档的关系
- `version-unify-check.md` → 本次检测证据（U1 身份不一致 / U5 代理污染 根因）
- 本指引 → 不更补丁也能做的诊断 + 更补丁的手工步骤
- `finale-prd.md` / `finale-design.md` → 软件侧收官（待你拍板拓扑后开工）
