# 幻兽帕鲁角色存档迁移实现机制三方对比研究——自研、同行与官方说明

> **历史研究，结论已修订（2026-07-27）**：后续真档与游戏实机证明，帕鲁 CSPM key 必须保持稳定，不能全局交换或删除；Fix Host 与跨世界 Character Transfer 也必须分开建模。请优先阅读 [`palworld-migration-success-record-2026-07-27.md`](./palworld-migration-success-record-2026-07-27.md)。

**日期**：2026-07-25
**执行模式**：完整

---

## 目录

- [引言](#引言)
- [1. 自研角色迁移实现机制：U01 字节级交换与 v2 三阶段结构化重绑](#1-自研角色迁移实现机制u01-字节级交换与-v2-三阶段结构化重绑)
- [2. 同行 reference-projects 的存档解析与角色迁移实现能力](#2-同行-reference-projects-的存档解析与角色迁移实现能力)
- [3. 幻兽帕鲁官方说明与合规边界核验](#3-幻兽帕鲁官方说明与合规边界核验)
- [4. 三方角色迁移实现机制差异对比与合规风险研判](#4-三方角色迁移实现机制差异对比与合规风险研判)
- [结论](#结论)
- [参考文献](#参考文献)

---

## 引言

幻兽帕鲁（Palworld）的玩家存档建立在社区逆向的 GVAS 二进制格式之上（魔数 0x53415647），其官方从未发布任何面向"玩家角色 UID 重绑/迁移"的格式规格或工具，角色存档迁移长期由社区工具链驱动（[cheahjs/palworld-save-tools](https://github.com/cheahjs/palworld-save-tools)；[scottanderson/gvas](https://github.com/scottanderson/gvas)）。对于需要把某一玩家角色顶替或迁移到另一存档的运营场景，自研项目、同行开源工具与官方公开立场三者之间，在"能不能做、怎么做、合不合规"上形成了显著分野，这正是本报告的研究起点。

本报告围绕用户的三核心问题展开——（a）自研项目如何实现角色迁移；（b）同行 reference-projects 具备怎样的迁移能力；（c）官方是否有任何说明与合规边界——对三方实现机制进行系统性对比。研究覆盖自研代码（fix_host.rs、sav_io.rs、save_edit.rs 等）、同行三仓库（PalworldSaveTools、zaigie、amantu）与官方公开资料（Steam 新闻、Games Fuze、GameSpot 等）三类来源，逐章拆解解析策略、UID 重绑方式、公会处理与迁移范围四个维度。

本研究的关键发现可预告如下：第一，自研以 U01 字节级 3-pass 交换与 v2 结构化只改 Guid 两条互补路线覆盖主场景，并以停服守卫+快照+回滚兜底，但遗漏了 CSPM 记录级去重（[PalworldSaveTools fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py) 已实现）；第二，官方仅提供帕鲁转移（[Steam 官方新闻](https://store.steampowered.com/news/app/1623730/view/596264277969469557)），把角色 UID 重绑完全留给社区；第三，所谓"官方 PalMigrationTool"仅见于单一第三方主张（[supercraft.host](https://supercraft.host/wiki/palworld/official-character-migration-v2/)），证据薄弱，须标注〔待官方证实〕。

---

## 1. 自研角色迁移实现机制：U01 字节级交换与 v2 三阶段结构化重绑

### 论点
本项目（Palworld-Server-Manager）针对"玩家角色 UID 迁移/顶替"实现了两条互补路线：U01 字节级 3-pass 交换（fix_host_save_in_dir）用于单机/联机 host 角色向专用服的整对顶替；v2 三阶段结构化重绑（fix_host_save_multi + patch_guild_group_name）用于多玩家批量、按字段精确重绑。两者均由 StopServerAssertion 停服守卫 + 整包快照 + 失败回滚兜底。唯一功能缺口是 CSPM 记录级去重尚未实现（同行 PalworldSaveTools 已实现，见第 2 章）。

### 论据
1. **U01 字节级 3-pass 交换（fix_host_save_in_dir，[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs)）。** 核心是 sav_io.rs 的 swap_guids（[sav_io.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/sav_io.rs)）：先取临时 UID `temp = *old` 并将 `temp[15] ^= 0xFF`（防双 GUID 污染），再依次 `replace_all(old, &temp)`、`replace_all(new, old)`、`replace_all(&temp, new)`，并断言 `temp ≠ new` 且 `temp` 不在缓冲区内。该算法完全绕过 GVAS 解析器，直接在字节层把 old↔new 互换，从而免疫"gvas crate 对嵌套 RawData 覆盖不全（R-GVAS-1，见模块注释）"导致的字段错位。随后 fix_host_save_in_dir 执行文件名交换（`Players/<uid>.sav` 的 32-hex stem 重命名）与 `_dps` 容器ID 修补（patch_dps，[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs) L128 设 OwnerPlayerUId + SlotId.ContainerId.ID；patch_dps_file L189 为 R-DPS-1 降级写回）。
2. **v2 三阶段结构化重绑。** Phase A 为前置快照/停服（并入安全闸门，见下文）。Phase B（fix_host_save_multi，[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs) L418）以 replace_guid_structured（[sav_io.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/sav_io.rs) L208）**只改 Guid 类型属性**，做单向 1:1 顶替——规避了 replace_guid_bytes（L184）盲搜 16 字节触发 337GB 分配崩溃的风险（R-GVAS-1 的对称问题）。Phase C（patch_guild_group_name，[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs) L512）用 replace_ascii_32hex_in_place（L545）处理 group_name——公会数据中 group_name 是 32-hex ASCII 的 FString 而非 Guid struct，必须字符串级单独替换（P0-3 陷阱）。
3. **安全闸门（run_fix_host_with_guard / migrate_world_v2）。** 停服守卫 StopServerAssertion 枚举（[save_edit.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit.rs) L158）要求迁移前服务器必须停止；run_fix_host_with_guard（L184）先整包快照再操作、失败回滚；migrate_world_v2（L520）确保 ensure_server_stopped，run_migration_v2_with_guard（L295）仅在 `_migration_backups/current/0/` 不存在时备一次（L310–324），rollback_migration_v2_impl（L345）负责回滚。
4. **缺口对比（与同行基准）。** 数据显示，自研 v2 重绑后未删除 new_uid 原条目（CSPM 记录级去重未做），而同行 PalworldSaveTools 在全结构化重序列化中执行了去重动作（[fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py)）。GVAS 无官方规格，其结构完全由社区逆向（[cheahjs/palworld-save-tools](https://github.com/cheahjs/palworld-save-tools)），这正解释了两条自研路线都必须自行处理 UID 异构载体的根因。

### U01 vs v2-B/C 对比表
| 维度 | U01（fix_host_save_in_dir） | v2 Phase B（fix_host_save_multi） | v2 Phase C（patch_guild_group_name） |
|------|------------------------------|-----------------------------------|--------------------------------------|
| 解析策略 | 字节级 3-pass 交换（绕过解析器） | 结构化只改 Guid（replace_guid_structured） | 字符串级 32-hex 替换（replace_ascii_32hex_in_place） |
| UID 重绑方式 | 对称交换 old↔new + 文件名交换 | 单向 1:1 顶替 | 公会 group_name 字符串替换 |
| 容器·公会处理 | patch_dps 设容器ID | （角色 UID 重绑） | 公会名重绑 |
| 适用场景 | host 单对顶替（单机→专用服） | 多玩家批量顶替 | 公会归属重绑 |
| 崩溃风险 | 无（不依赖解析器） | 规避盲搜 16 字节崩溃 | 无 |

### 分析
U01 与 v2 的取舍根因一致：GVAS 无官方规格、社区解析器覆盖不全（R-GVAS-1）。U01 用"字节级"彻底绕开解析器，换来对任意版本存档的鲁棒性，但只能做整对对称交换；v2 用"结构化只改 Guid"换取字段级精度与批量能力，但依赖解析器正确——因此必须把 group_name 这种非 Guid 的 32-hex FString 单独处理。安全闸门则确保两条路线都只在停服离线、可回滚的前提下运行。

### 小结
自研以 U01（字节级整对顶替）+ v2（结构化批量重绑）覆盖角色 UID 迁移的两类主场景，并以 StopServerAssertion + 快照 + 回滚保障安全。**唯一缺口**：v2 重绑后未删除 new_uid 原条目（CSPM 记录级去重），需上真档前手工前置；该能力同行 PalworldSaveTools 已实现（第 2 章）。

#### 本章使用来源清单
1. [fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs) — 自研 U01/v2 核心。
2. [sav_io.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/sav_io.rs) — swap_guids 3-pass、replace_guid_structured、replace_guid_bytes。
3. [save_edit.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit.rs) — run_fix_host_with_guard、StopServerAssertion、migrate_world_v2、run_migration_v2_with_guard、rollback。
4. [models.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/models.rs) — FixHostRequest / UidMapping / ThreePhaseMigrationRequest / MigrateRequest。
5. [world_copy.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/world_copy.rs) — parse_level_gvas / guid_std / read_players_from_level。
6. [PalworldSaveTools fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py) — 同行基准，佐证 CSPM 去重动作。
7. [cheahjs/palworld-save-tools (GitHub)](https://github.com/cheahjs/palworld-save-tools) — 社区逆向 GVAS，佐证无官方规格。

---

## 2. 同行 reference-projects 的存档解析与角色迁移实现能力

### 论点
对 reference-projects 三仓库的审计显示，角色迁移能力呈现清晰的三级分化：① PalworldSaveTools 具备完整迁移能力；② zaigie 仅做只读解析导出；③ amantu 纯 REST 管理。结论：只有 PalworldSaveTools 能实际迁移角色，它同时是本项目的校准基准。

### 论据
1. **Tier 1 — PalworldSaveTools（fix_host_save.py，[fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py)）。** combined_task（L503）/ fix_save（L178）采用**全结构化 JSON 重序列化**：sav_to_json 解析 → 编辑 → json_to_sav 写回。它对称交换 PlayerUId 与 IndividualId.PlayerUId（L202–205），按 InstanceId 匹配改写 CSPM（L216–221），并对公会做结构化修复（L222–240）、deep_swap（L241–258）、copy_dps_file（L287）、文件名交换（L276–280），**且含 CSPM 记录级去重动作**。character_transfer.py 的 transfer_all_characters（L811）进一步实现**跨世界复制式转移**（fast_deepcopy L845、等级<2 跳过 L841–843、transfer_guild 保留旧公会 L860–862）。
2. **Tier 2 — zaigie（sav_cli.py，[sav_cli.py](file:///F:/study/Palworld-Server-Manager/reference-projects/zaigie-palworld-server-tool-vpn/sav_cli/sav_cli.py)）。** 仅提供 convert_sav / structure_player / structure_guild，把存档导出为 {players, guilds} JSON，**只读、无写入、无迁移**。
3. **Tier 3 — amantu（commands.rs，[commands.rs](file:///F:/study/Palworld-Server-Manager/reference-projects/amantu-qbit-tauri-manager-vpn/src-tauri/src/commands.rs)）。** save_world（L104）/ save_connection（L128）/ save_bridge（L154）只是 REST 管理面板的持久化接口，**纯管理、无存档编辑、无迁移**。

### 三级能力分化表
| 层级 | 项目 | 解析策略 | 迁移能力 | 去重·公会 |
|------|------|----------|----------|-----------|
| Tier 1 | PalworldSaveTools | 全结构化 JSON 重序列化 | 有（对称交换 + 跨世界复制） | 有 CSPM 去重 + 结构化公会修复 |
| Tier 2 | zaigie | 只读解析导出 | 无 | 无 |
| Tier 3 | amantu | 无（纯 REST 管理） | 无 | 无 |

### 分析
三级分化印证了第 1 章的判断：角色迁移的难点不在"读"，而在"安全重写 GVAS"。zaigie 证明了社区能读，amantu 证明了能管，但只有 PalworldSaveTools 解决了"重写 + 去重 + 公会一致性"的完整链路。其"全结构化重序列化"路线与自研 v2 的"结构化只改 Guid"路线殊途同归，差异在于 PalworldSaveTools 重序列化整份存档因而能顺带做 CSPM 去重，而自研 v2 只改 Guid 类型属性故遗漏去重（[sav_io.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/sav_io.rs) 的 replace_guid_structured 不含去重逻辑）。这为第 4 章三方对比埋下关键分化点。

### 小结
同行三仓库中仅 PalworldSaveTools 具备真实角色迁移能力，且以"全结构化重序列化 + 去重 + 公会修复"领先自研 v2（自研缺去重）；zaigie 只读、amantu 纯管理，均不构成迁移方案。本项目应以 PalworldSaveTools 为校准基准补齐 CSPM 去重。

#### 本章使用来源清单
1. [PalworldSaveTools fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py) — combined_task / fix_save / 对称交换 / CSPM 改写 / 公会修复 / 去重。
2. [PalworldSaveTools character_transfer.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/character_transfer.py) — transfer_all_characters 跨世界复制式转移。
3. [zaigie sav_cli.py](file:///F:/study/Palworld-Server-Manager/reference-projects/zaigie-palworld-server-tool-vpn/sav_cli/sav_cli.py) — 只读解析导出 players/guilds。
4. [amantu commands.rs](file:///F:/study/Palworld-Server-Manager/reference-projects/amantu-qbit-tauri-manager-vpn/src-tauri/src/commands.rs) — 纯 REST 管理接口。
5. [fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs) — 自研 v2 缺去重对比锚点。
6. [sav_io.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/sav_io.rs) — replace_guid_structured 不含去重。
7. [cheahjs/palworld-save-tools (GitHub)](https://github.com/cheahjs/palworld-save-tools) — 社区解析库，佐证 GVAS 结构与去重实践。

---

## 3. 幻兽帕鲁官方说明与合规边界核验

### 论点
幻兽帕鲁的官方（Pocketpair）在角色存档迁移这一议题上，只给出了"帕鲁（Pal）跨世界/跨存档转移"的官方能力，从未发布任何面向"玩家角色 UID 重绑/迁移"的官方工具、格式规格或操作指引。社区与第三方工具（含本项目 save_edit、PalworldSaveTools 等）之所以大量存在，正是因为官方在这块留有空白。所谓"官方 PalMigrationTool"目前仅见于单一第三方托管站点，证据强度弱，须标注为〔待官方证实〕。

### 论据
1. **官方路线图确有"World Transfers for Pals"，但对象是帕鲁而非玩家角色。** Pocketpair 在 Steam 官方新闻（一手来源）的一周年路线图中明确列出 "World Transfers for Pals"（[Steam 官方新闻](https://store.steampowered.com/news/app/1623730/view/596264277969469557)）。
2. **1.0 版本落地的对应能力是"Global Palbox（全球帕鲁箱）"，仍只转移帕鲁。** 据 1.0 指南，玩家可在不同存档间建造 Global Palbox 以转移帕鲁"快照"，且明确不转移玩家角色本身（[Games Fuze 指南](https://gamesfuze.com/guides/how-to-transfer-pals-to-a-new-save-with-the-global-palbox-in-palworld-1-0/)）。
3. **"官方 PalMigrationTool"缺一手佐证，属〔待官方证实〕。** 第三方站点 supercraft.host 声称 Pocketpair 自 2025 年 12 月更新起以原生 `PalMigrationTool`（位于服务器文件 `/Tools/Migration/`，支持 `--export/--import`）取代第三方 Python 脚本（[supercraft.host](https://supercraft.host/wiki/palworld/official-character-migration-v2/)）。但该文未链接任何 Pocketpair 官方公告、Steam 新闻或更新日志；其余检索结果均为社区/第三方方案，无一处为官方一手来源。因此该"官方工具"论断目前证据强度弱，须标注〔待官方证实〕。
4. **GVAS 存档格式无官方规格。** Palworld 存档采用 Unreal Engine 的 GVAS 容器（魔数 `0x53415647`，由社区逆向项目 cheahjs/palworld-save-tools 与 scottanderson/gvas 记录）并外包 PlZ/PLM 压缩，但 Pocketpair 与 Epic 均未发布任何官方字段规格。其结构完全由社区逆向实现（[cheahjs/palworld-save-tools](https://github.com/cheahjs/palworld-save-tools)、[scottanderson/gvas](https://github.com/scottanderson/gvas)）。
5. **官方反作弊立场明确，但针对的是"作弊/欺诈"，未专门禁止离线存档编辑。** Pocketpair 在 Steam 公告中称 "as a company, we do not tolerate any fraudulent activity or cheating"，并推出 "external anti-cheat solution"——官方服务器强制启用，社区服/单人/联机可选（[GameSpot](https://www.gamespot.com/articles/palworld-will-recieve-intensified-anti-cheat-measures/1100-6521160/)、[Eurogamer](https://www.eurogamer.net/palworld-studio-admits-its-cant-keep-up-with-cheaters-says-anti-cheat-solution-on-the-way)）。检索未发现官方 EULA 中有明确禁止"离线修改本地存档文件"的条款。

### 分析
综合证据强度：① "World Transfers for Pals / Global Palbox"为帕鲁转移——证据强；② 官方反作弊禁令——证据强；③ "官方不提供角色 UID 迁移工具"——证据中强（无一手证据 + 整个生态均为社区驱动的反向印证）；④ "PalMigrationTool 官方工具"——证据弱，必须〔待官方证实〕；⑤ "GVAS 无官方规格"——证据强。关键边界：官方为帕鲁提供了"转移"语义的原生能力，却把"玩家角色 UID 重绑"留给了社区。

### 小结
幻兽帕鲁官方对"角色 UID 迁移"未作正面说明：既无官方工具（除〔待官方证实〕的 PalMigrationTool 外），也无官方格式规格，更无专门的操作指引。官方的公开立场仅覆盖两点——帕鲁的跨存档转移（Global Palbox）与反作弊禁令。角色迁移的全部能力与风险边界，均由社区逆向 GVAS 与自建工具承担。

#### 关键发现
- 发现1：官方路线图 "World Transfers for Pals" 与 1.0 的 Global Palbox 均只转移帕鲁，不涉及玩家角色 UID。
- 发现2：所谓官方 PalMigrationTool 仅见于 supercraft.host 单一第三方站点，无一手佐证，标注〔待官方证实〕。
- 发现3：GVAS 格式无任何官方规格，完全由社区逆向。
- 发现4：官方反作弊强制于官方服务器、可选于社区/单人，未明确禁止离线存档编辑。

#### 数据摘要
| 指标 | 数据 | 来源 |
|------|------|------|
| 官方转移能力 | World Transfers for Pals（帕鲁，非角色） | [Steam 官方新闻](https://store.steampowered.com/news/app/1623730/view/596264277969469557) |
| 1.0 对应功能 | Global Palbox 跨存档帕鲁快照转移 | [Games Fuze](https://gamesfuze.com/guides/how-to-transfer-pals-to-a-new-save-with-the-global-palbox-in-palworld-1-0/) |
| PalMigrationTool 证据 | 仅 1 个第三方站点，无官方链接 | [supercraft.host](https://supercraft.host/wiki/palworld/official-character-migration-v2/)〔待官方证实〕 |
| GVAS 官方规格 | 不存在（仅社区逆向） | [cheahjs/palworld-save-tools](https://github.com/cheahjs/palworld-save-tools) |
| 反作弊策略 | 官方服强制 / 社区·单人·联机可选 | [GameSpot](https://www.gamespot.com/articles/palworld-will-recieve-intensified-anti-cheat-measures/1100-6521160/) |

#### 章内全部引用来源清单
| # | 标题 | URL | 类型 |
|---|------|-----|------|
| 1 | Steam 官方新闻：Palworld 一周年路线图（含 "World Transfers for Pals"） | https://store.steampowered.com/news/app/1623730/view/596264277969469557 | 官方一手来源 |
| 2 | Games Fuze：How to Transfer Pals to a New Save With the Global Palbox in Palworld 1.0 | https://gamesfuze.com/guides/how-to-transfer-pals-to-a-new-save-with-the-global-palbox-in-palworld-1-0/ | 第三方指南 |
| 3 | supercraft.host：Official Character Migration & RCON v2（声称 PalMigrationTool） | https://supercraft.host/wiki/palworld/official-character-migration-v2/ | 第三方站点（〔待官方证实〕） |
| 4 | GameSpot：Palworld Will Receive Intensified Anti-Cheat Measures | https://www.gamespot.com/articles/palworld-will-recieve-intensified-anti-cheat-measures/1100-6521160/ | 二手媒体 |
| 5 | Eurogamer：Palworld studio admits it can't "keep up" with cheaters | https://www.eurogamer.net/palworld-studio-admits-its-cant-keep-up-with-cheaters-says-anti-cheat-solution-on-the-way | 二手媒体 |
| 6 | cheahjs/palworld-save-tools (GitHub) | https://github.com/cheahjs/palworld-save-tools | 社区开源项目 |
| 7 | scottanderson/gvas (GitHub) | https://github.com/scottanderson/gvas | 社区开源项目 |

---

## 4. 三方角色迁移实现机制差异对比与合规风险研判

### 论点
综合前三章，自研、同行、官方在"玩家角色 UID 迁移"上呈现三种截然不同的立场。本章以一张五实现 × 四维对比总表收束前三章，并基于第 3 章官方核验给出合规风险研判与可执行的运营建议，正面回应用户原问题。

### 论据（含对比表）
| 维度 | 自研 U01 | 自研 v2 | PalworldSaveTools | zaigie | amantu |
|------|---------|--------|-------------------|--------|--------|
| 解析策略 | 字节级交换：swap_guids 3-pass + 文件名交换 + _dps 容器ID 修补（[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs)、[sav_io.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/sav_io.rs)）| 结构化只改 Guid 类型属性（replace_guid_structured）+ group_name 32-hex 字符串级替换（[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs)）| 全结构化 JSON 重序列化（sav_to_json→编辑→json_to_sav）（[fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py)）| 只读：convert_sav/structure_player/structure_guild 导出 {players,guilds} JSON（[sav_cli.py](file:///F:/study/Palworld-Server-Manager/reference-projects/zaigie-palworld-server-tool-vpn/sav_cli/sav_cli.py)）| 无（纯管理 save_world/save_connection/save_bridge）（[commands.rs](file:///F:/study/Palworld-Server-Manager/reference-projects/amantu-qbit-tauri-manager-vpn/src-tauri/src/commands.rs)）|
| UID 重绑方式 | 3-pass 对称交换 + 文件名交换（[sav_io.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/sav_io.rs) swap_guids）| 单向 1:1 顶替（fix_host_save_multi Phase B）（[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs)）| 对称交换 PlayerUId/IndividualId.PlayerUId + 按 InstanceId 匹配改写 CSPM（[fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py)）| 无 | 无 |
| 公会处理 | patch_dps 设 OwnerPlayerUId + SlotId.ContainerId.ID（[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs)）| patch_guild_group_name 用 group_name 32-hex 字符串替换（[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs)）| RawData 结构化修复 + 记录级去重（[fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py)）| 无 | 无 |
| 迁移范围 | 单机↔专用服 单对（host 顶替）| 多玩家批量（三阶段迁移）| 跨世界整批 + 保留旧公会（[character_transfer.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/character_transfer.py)）| 无 | 无 |

#### 关键机制分歧解读
1. **三条路线取舍的根因——解析器覆盖不全 + 盲搜崩溃。** U01 完全绕过解析器，以字节级 3-pass 交换规避 gvas crate 对嵌套 RawData 覆盖不全（R-GVAS-1）的问题；v2 改走结构化只改 Guid，规避 `replace_guid_bytes` 盲搜 16 字节触发 337GB 分配崩溃的风险；PalworldSaveTools 依赖全结构化重序列化，最完整但前提是 schema 准确。
2. **group_name 是 FString（32-hex ASCII）必须单独字符串级替换。** 公会数据中 group_name 以 32-hex ASCII 字符串存储而非 Guid struct，自研以 `replace_ascii_32hex_in_place`（P0-3）处理，PalworldSaveTools 同样单独处理。
3. **CSPM 记录级去重缺口（自研 v2 待补）。** 自研 v2 三段重绑后未删除 new_uid 原条目，需上真档前手工前置；而 PalworldSaveTools 与 palworld-server-toolkit 均在重绑时执行 CSPM 去重动作（[fix_host_save.py](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py)、[palworld-server-toolkit](https://github.com/magicbear/palworld-server-toolkit/)）。

### 分析（合规风险研判）
基于第 3 章官方核验：Pocketpair 仅以一手 Steam 新闻确认 "World Transfers for Pals" 与 1.0 Global Palbox（仅帕鲁转移），并以官方声明推出 "external anti-cheat solution"（官方服强制、社区/单人/联机可选）。角色 UID 迁移无官方工具、无规格、无指引，全靠社区逆向 GVAS。据此研判：
- **离线/停服编辑风险自担**：本地存档修改本身未被官方 EULA 明确禁止，但正确性由操作者负责；务必停服并先备副本。
- **带入官方反作弊服属高风险**：将迁移结果上传至强制反作弊的官方服务器，可能触发作弊判定与封号。
- **运行时内存修改器（trainer/Cheat Engine）红线**：直接违反官方 "do not tolerate cheating" 声明，可能永久封号，与本报告的离线编辑路线不可混为一谈。
- **PalMigrationTool 〔待官方证实〕**：仅 supercraft.host 单一第三方站点主张，无一手佐证，勿当作已发布能力纳入决策。

### 小结（收束结论：正面回答用户三问）
- **（a）自研怎么实现**：U01 以 swap_guids 字节级 3-pass 交换 + 文件名交换 + _dps 容器ID 修补，解决 host 单对顶替；v2 以 replace_guid_structured 结构化只改 Guid + group_name 字符串级替换，实现多玩家批量三阶段重绑；二者均由 StopServerAssertion 停服守卫 + 整份快照备份 + 失败回滚兜底。唯一缺口是 CSPM 记录级去重。
- **（b）同行怎么实现**：PalworldSaveTools 以全结构化重序列化 + 对称交换 + 去重为校准基准；zaigie 仅只读导出，amantu 纯管理，二者均无迁移能力。
- **（c）官方有无说明**：官方只管帕鲁转移（World Transfers for Pals / Global Palbox）与反作弊，对角色 UID 迁移从未提供工具/规格/指引；PalMigrationTool 仍属〔待官方证实〕。

**给运营者的实操建议**：
1. 一律停服离线编辑，并先复制整份存档副本验证。
2. 切勿将迁移结果带入强制反作弊的官方服务器；社区服/单机可放心使用。
3. 补齐 CSPM 记录级去重（借鉴 PalworldSaveTools），避免重绑后 new_uid 残留旧条目导致上真档异常。

#### 本章新增来源
1. [magicbear/palworld-server-toolkit (GitHub)](https://github.com/magicbear/palworld-server-toolkit/) — 社区工具，提供 CopyPlayer/MigratePlayer 并在重绑时执行 CSPM 去重动作。

---

## 结论

综合四章研究，本报告正面回应了用户三问。其一，自研项目以 U01 字节级 3-pass 交换（swap_guids 防双 GUID 污染 + 文件名交换 + _dps 容器ID 修补）与 v2 三阶段结构化重绑（replace_guid_structured 只改 Guid + patch_guild_group_name 处理 32-hex FString）两条互补路线实现角色顶替/重绑，并以 StopServerAssertion 停服守卫、整包快照与失败回滚兜底（[fix_host.rs](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs)）；其唯一功能缺口是迁移后未做 CSPM 记录级去重。其二，同行参考项目中仅 PalworldSaveTools 具备真实迁移能力（全结构化 JSON 重序列化 + 对称交换 PlayerUId + 按 InstanceId 改写 CSPM + 去重），zaigie 仅只读导出、amantu 纯 REST 管理，均无迁移能力。其三，官方（Pocketpair）仅提供帕鲁跨世界/跨存档转移（[Steam 官方新闻](https://store.steampowered.com/news/app/1623730/view/596264277969469557)），从未发布面向角色 UID 重绑的工具、规格或指引；所谓"官方 PalMigrationTool"仅见于单一第三方站点（[supercraft.host](https://supercraft.host/wiki/palworld/official-character-migration-v2/)），缺一手佐证，须标注〔待官方证实〕。

三条跨章关键发现收敛如下：第一，GVAS 无任何官方规格，结构完全由社区逆向（魔数 0x53415647，[scottanderson/gvas](https://github.com/scottanderson/gvas)），这是三方实现不确定性的共同根源；第二，group_name 的 32-hex FString 陷阱与盲搜 16 字节触发 337GB 分配崩溃，揭示了字节级盲搜与结构化解析两条路线的安全边界差异；第三，CSPM 去重缺口使自研 v2 在功能完整性上落后于 PalworldSaveTools，而 magicbear/palworld-server-toolkit 等同行的去重实践印证了该缺口的普遍性。

对应上述研判，提出三项实操建议：1）坚持停服离线编辑并先备完整副本（[GameSpot](https://www.gamespot.com/articles/palworld-will-recieve-intensified-anti-cheat-measures/1100-6521160/)）；2）勿将改写后存档带入强制反作弊的官方服；3）优先补齐 CSPM 记录级去重，以 PalworldSaveTools 为校准基准。未来应持续跟踪官方是否就角色迁移发布正式工具或规格，以收敛当前由社区单方面承担的实现与合规风险。

---

## 参考文献

- cheahjs, palworld-save-tools（GitHub 仓库）, n.d. [链接](https://github.com/cheahjs/palworld-save-tools)
- Eurogamer, Palworld studio admits it can't keep up with cheaters, says anti-cheat solution on the way, 2024 [链接](https://www.eurogamer.net/palworld-studio-admits-its-cant-keep-up-with-cheaters-says-anti-cheat-solution-on-the-way)
- GameSpot, Palworld will receive intensified anti-cheat measures, 2024 [链接](https://www.gamespot.com/articles/palworld-will-recieve-intensified-anti-cheat-measures/1100-6521160/)
- Games Fuze, How to transfer Pals to a new save with the Global Palbox in Palworld 1.0, 2024 [链接](https://gamesfuze.com/guides/how-to-transfer-pals-to-a-new-save-with-the-global-palbox-in-palworld-1-0/)
- magicbear, palworld-server-toolkit（GitHub 仓库）, n.d. [链接](https://github.com/magicbear/palworld-server-toolkit/)
- scottanderson, gvas（GitHub 仓库）, n.d. [链接](https://github.com/scottanderson/gvas)
- Steam（Pocketpair）, World Transfers for Pals（官方新闻）, 2024 [链接](https://store.steampowered.com/news/app/1623730/view/596264277969469557)
- supercraft.host, Official character migration v2（第三方 Wiki）, n.d. [链接](https://supercraft.host/wiki/palworld/official-character-migration-v2/)
- Palworld-Server-Manager（项目内代码·自研）, fix_host.rs, n.d. [代码](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/fix_host.rs)
- Palworld-Server-Manager（项目内代码·自研）, sav_io.rs, n.d. [代码](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/sav_io.rs)
- Palworld-Server-Manager（项目内代码·自研）, save_edit.rs, n.d. [代码](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit.rs)
- Palworld-Server-Manager（项目内代码·自研）, models.rs, n.d. [代码](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/models.rs)
- Palworld-Server-Manager（项目内代码·自研）, world_copy.rs, n.d. [代码](file:///F:/study/Palworld-Server-Manager/Palworld/src-tauri/src/save_edit/world_copy.rs)
- Palworld-Server-Manager（项目内代码·同行 PalworldSaveTools）, fix_host_save.py, n.d. [代码](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/fix_host_save.py)
- Palworld-Server-Manager（项目内代码·同行 PalworldSaveTools）, character_transfer.py, n.d. [代码](file:///F:/study/Palworld-Server-Manager/reference-projects/PalworldSaveTools-main/palworld_toolsets/character_transfer.py)
- Palworld-Server-Manager（项目内代码·同行 zaigie）, sav_cli.py, n.d. [代码](file:///F:/study/Palworld-Server-Manager/reference-projects/zaigie-palworld-server-tool-vpn/sav_cli/sav_cli.py)
- Palworld-Server-Manager（项目内代码·同行 amantu）, commands.rs, n.d. [代码](file:///F:/study/Palworld-Server-Manager/reference-projects/amantu-qbit-tauri-manager-vpn/src-tauri/src/commands.rs)

---

## 待完善事项

1. 自研 v2 CSPM 记录级去重缺口：属已知功能待补，已在第 4 章结论与建议中点明；上真档前需手工前置该去重动作。
2. PalMigrationTool〔待官方证实〕：仅 supercraft.host 单一第三方主张，缺官方一手佐证，决策时勿当作已发布能力。
3. 第 1 章「三阶段」命名精度：Phase A 实为前置快照/停服（已并入安全闸门论述），读者对阶段计数或存疑，建议后续措辞优化（非阻塞）。
4. 第 2 章可选增强（非必须）：论据正文 L811 引用可补超链接、社区来源可在分析段落地、可选补回 GVAS 底层格式深度（Anderson DeepWiki / gewenwu916 DEV）。
5. 第 3 章可选增强（非必须）：重生成稿以「章内引用来源清单」替代「数据摘要表」，如需统一体例可补回。

---

> 本报告由 AI 深度研究团队生成，重要决策请经专业人员核验。所有引用来源请用户在重要场景下二次核验时效性与真实性。
