# Palworld 存档管理类竞品功能版图（调研报告）

> **二次核验勘误（2026-07-24）**：请与 [`save-migration-research-validation-2026-07.md`](save-migration-research-validation-2026-07.md) 一起阅读。当前本地 `PalworldSaveTools` 的 `character_transfer.py` 定义了 `guild` 转移步骤和 `transfer_guild()`，故本文任何“跨世界刻意不含公会”的绝对表述都不能作为当前源码结论；跨世界公会的精确语义仍需真实样本验收。

> 调研角色：产品经理（Alice）· 研究模式（只调研、只产出文档，不改任何代码）
> 信息来源：本地参考项目 `reference-projects/PalworldSaveTools-main`（已逐文件读取 README/AGENTS/changelogs、9 个 toolset、7 个 manager、UI tabs/dialogs/map_view/editor）+ 联网检索（palworld-save-pal、palworld-server-toolkit、Palworld-Pal-Editor、PalworldCharacterTransfer、各类 web/PS 服务器管理器）+ 本项目已有 `docs/f5-local-to-server-design.md`、`docs/f5-architecture.md`
> 用途：后续架构师编写数据库结构文档的输入之一

---

## 0. 调研方法说明

- **本地参照（最主要）**：直接读取 `PalworldSaveTools`（PST，deafdudecomputers）源码，逐项确认 toolset / manager 的真实能力，而非只看 README 宣传。
- **联网交叉验证**：检索社区其他成熟工具，确认能力边界与形态差异。
- **对比基线**：以本项目 `docs/f5-*` 规划文档为"我们已有/已规划"的基准，避免编造。
- **置信度**：凡能力来自实际读到的源码/文档标注 ✓；仅 README 提及、未读到实现标注 △ 或"未确认"；未提及标注 ✗。

---

## 一、主流工具一览

| # | 工具名 | 形态 | 开源 / 许可证 | 平台覆盖（Steam 单机/联机 · 专用服 · Xbox/GamePass） | 技术栈 |
|---|---|---|---|---|---|
| 1 | **PalworldSaveTools (PST)** | 桌面 GUI（Win/Linux/macOS 三端预编译二进制） | 开源 · MIT | Steam 单机/联机 ✓、专用服 ✓、GamePass ✓（双向转换） | Python + PySide6 + `palsav` 解析引擎 |
| 2 | **palworld-save-pal (PSP)** | Web + 桌面（Tauri/setup.py 打包）+ Docker | 开源 · MIT | Steam ✓、GamePass ✓（solo/coop/dedicated）；新增"原生服务器管理（steamcmd）" | SvelteKit5 + Python 后端 |
| 3 | **palworld-server-toolkit** | CLI（Python）+ 简易 GUI | 开源（pip 安装） | Steam ✓（专用服为主）；GamePass 未确认 | Python（magicbear 高速解析内核） |
| 4 | **Palworld-Pal-Editor** | GUI + WebUI + CLI + Docker | 开源 | **仅 Steam**；GamePass 建议用 XGP 转换工具 | Python |
| 5 | **Palworld Server Tool（web，zaigie 系）** | Web（REST + RCON） | 开源 | 专用服 ✓（Docker/k8s 同步存档）；不对单机做编辑 | Go 后端 + Web 前端 |
| 6 | **Palworld Server Manager（本项目）** | 桌面 GUI（Tauri2 + Vue3） | 主项目 | Steam 单机/联机 ✓、专用服 ✓；GamePass 明确 MVP 排除 | Rust + Vue3 + Tauri2 |

> 说明：另有 `PalworldCharacterTransfer`（Python 脚本+exe，专注跨服角色转移、带 "Keep Old Guild ID" 选项）、`PalServerTools`（.NET Web 运维，可读写 `WorldOption.sav`）、`PalworldServerTools`（PowerShell 运维脚本）等，定位偏"服务器运维/单点能力"，下文并入对比。

---

## 二、功能能力矩阵

图例：✓ 已支持 ｜ △ 部分支持/规划中/仅间接 ｜ ✗ 不支持 ｜ 未确认 ｜ — 不适用

| 功能行 | PST（本地参照） | palworld-save-pal | palworld-server-toolkit | Palworld-Pal-Editor | Palworld Server Tool(web) | 本项目（Palworld Server Manager） |
|---|---|---|---|---|---|---|
| **整包世界备份/恢复（含指定外部目录）** | △ 每次操作自动备份（含 Level/Players/WorldOption/LocalData），存 AppData `Backups/`；非用户任意外部目录 | 未确认（Server Manager 为新增） | ✗ | ✗ | ✓ 定时自动备份（interval+保留天数），可对接 Docker/k8s，路径可配 | ✓ F4 `backup_world` 支持 `dest` 外部目录 + `restore_world` + `_backups/<world>/<ts>/` |
| **角色存档导出/导入（保持 SteamID）** | ✓ `export/import_player_backup`（.pst7，含 cspm + 玩家 .sav） | 未确认（以"跨世界转移"实现，非单文件导出） | △ 经 `CopyPlayer`/`MigratePlayer` 跨世界搬移 | ✗ | ✗ | ✓ F4 `export/import_character`（纯文件拷贝，SteamID 不变） |
| **角色跨世界/跨服转移（含 Fix Host Save / UID 互换）** | ✓ `character_transfer` + `fix_host_save`（UID 互换）；⚠ 跨服**刻意不含公会** | ✓ Player Transfer（character/inventory/pals/technology/appearance，覆盖/新建）+ Player UID Swap | ✓ `CopyPlayer`/`MigratePlayer` 跨世界 | ✗ | ✗ | ✓ P0 `fix_host_save`（UID 互换）+ P1 `transfer_character`（子集）；同世界保留公会、跨世界合并 P2 |
| **科技点编辑、玩家基础属性编辑（改名/等级/Max All）** | ✓ 科技点/Ancient 科技点/改名/等级/Max All Stats/遗物属性（player_manager 已确认） | ✓ 科技点/Ancient 科技点/改名/等级/属性/治疗 | △ 编辑玩家参数/物品/金钱（科技点未明确） | ✗（明确为未来 TODO） | △ 仅展示（RCON/解析），不深度改写 | ✓ P1 `edit_tech` + `edit_player_attr`（改名/等级/Max All） |
| **公会（Guild）数据查看/转移/合并** | ✓ 查看 + 同存档内转移（换领袖/改名/设等级/解锁实验室/跨公会移人）；跨服合并 ✗ | ✓ 查看 + 改名/金库/实验室/删除；跨服合并未确认 | ✓ 移动玩家公会、删空公会 | ✗ | △ 展示公会/玩家（解析层） | △ 同世界保留=P0；跨世界合并=P2（仅接口/提示）；管理 UI=P2 |
| **据点（Base/Camp）数据查看/转移** | ✓ 查看 + 蓝图导出/导入（.pstbase/.json）+ 克隆到其他公会（偏移）+ 位移/微调/调半径/删除 | △ 查看 + 编辑据点帕鲁/容器；导出导入未确认 | △ 删除营地；查看/转移未明确 | ✗ | ✗ | △ P2（基地导出/导入/克隆，f5 列为 P2，未实现） |
| **背包/物品（Inventory/Item）编辑** | ✓ 玩家背包（物品/关键物品/装备/数量/增删）、基地容器、跨公会物品操作、修复物品、解锁全快速旅行 | ✓ 玩家背包编辑 + 预设/装备方案；基地容器 | ✓ 编辑物品 | △ 帕鲁入包（玩家背包非重点） | ✗ | △ P2（背包深度编辑，f5 列为 P2） |
| **地图/建筑（Map/World object）编辑与恢复** | ✓ 交互式地图（标记/排除区/校准）、Restore Map（清迷雾/解锁全地图）、据点位移、删非据点对象、修复建筑 | ✓ 地图集成 | ✗ | ✗ | ✗ | △ P2（地图迷雾解锁）；基地修改 P2 |
| **世界参数（WorldOption）编辑** | △ 仅 raw `modify_save` 可改；无专用编辑器；迁移时备份 WorldOption.sav | 未确认 | 未确认 | ✗ | △ 可读取 WorldOption.sav（运维展示） | △ 迁移时处理（提示删 `WorldOption.sav`）；专用编辑未规划 |
| **跨平台存档导入导出（Xbox/GamePass ↔ Steam）** | ✓ 双向（game_pass_save_fix + xgp_save_extract + convert）；加密 .wgs 提取 | ✓ GamePass & Steam 全支持 | 未确认（疑仅 Steam） | ✗ 仅 Steam（建议用 XGP 工具转换） | ✗ | ✗ MVP 明确排除（R10），UI 检测 .wgs 提示 |
| **存档诊断 / 修复非法数据** | ✓ `save_diagnostic`（孤儿玩家/公会异常/损坏条目）、修复非法帕鲁/玩家、修复物品/建筑、清非法物品/被动、修负时间戳 | △ Data Explorer/Debug（只读）；修复非法未确认 | ✓ 描述含 "repair sav file" | ✗ | △ 解析展示层 | △ 仅 round-trip 校验 + 版本门控（R1/R8）；诊断/修非法未规划 |

**关键洞察：**
- PST 与 PSP 是**功能最全、形态最接近"成熟产品"**的两款——几乎覆盖存档可改写的全部维度，且都支持 GamePass↔Steam。
- 帕鲁深度编辑是 **Palworld-Pal-Editor** 的强项（IV/魂/技能/被动/工作适应/克隆/导入导出/修非法），但**完全不做玩家/公会/世界**编辑，且 Steam-only。
- **各类 "Server Tool / Server Manager"**（Go web、.NET、PowerShell）本质是**服务器运维**（RCON、自动备份、重启、配置、在线玩家、k8s/Docker 同步），**不是存档编辑器**——它们读 .sav 只为展示，深度改写很弱。
- 本项目定位是"服务器管理器"，与第 5 类运维工具同赛道，但在**存档解析改写（F5）**上正补强，对标 PST/PSP 的编辑能力。

---

## 三、关键 UX 范式（成熟工具"选世界 → 选角色/公会 → 执行操作"流程）

基于 PST 实际代码与 changelogs 提炼（PSP/Pal-Editor 路线一致）：

1. **分层选择**：先"世界级"（多存档发现，扁平 `<World>/` 与 GUID 嵌套 `<World>/<GUID>/` 双兼容），再"玩家级多选"（名称/等级/公会/帕鲁数/最近在线），最后"操作子集勾选"（character/guild/tech/inventory/pals/appearance 等，对应 PST `_TRANSFER_STEPS`）。
2. **右键上下文菜单 + 双击快改**：左键选中、右键几乎万物可操作（`ScrollableContextMenu`）、双击快速编辑/删除；树形/列表控件统一。
3. **多选批量工具条**：Ctrl/Shift 多选后出现内联批量条（Max/Heal/Rename/Delete），选区跨分页保持（changelogs 2.1.7）。
4. **Tab 化功能分区**：Map / Tools / Players / Guilds / Bases / Player Inventory / Base Inventory / Pal Editor / Exclusions，每 Tab 内置帮助 + 悬浮 tooltip 详尽说明。
5. **工具卡片化**：Tools Tab 用可点击卡片承载独立工具（转换、GamePass→Steam、SteamID 换算、Restore Map、Slot Injector、Modify Save、Character Transfer、Fix Host Save）。
6. **停服/前置断言与二次确认**：Fix Host Save 明确要求"先停服、双方 ≥Lv2"（2.1.8 已放宽等级）；批量 Max All Pals 用"双重确认"对话框。
7. **防呆保护**：
   - 每次保存前**自动备份**；"Load from Backup" 列出时间戳/世界名/玩家数供回滚；
   - **Stale save detection**：若磁盘上 `Level.sav` 自加载后被改动（如游戏/服务器又自动存过），覆盖前告警；
   - **Unsaved changes warning**：退出前有未保存改动则提示保存/放弃/取消；
   - **Exclusion 排除列表**：清理前先把要保护的玩家/公会/据点加入排除名单（持久化）。
8. **拖拽加载**：任意 Tab 拖入 `.sav` 即加载（overlay 提示）。
9. **错误表现**：深色 overlay 替代零散弹窗；loading 覆盖层随操作窗口。

> 对本项目的直接启示：F5 已规划 L1–L4 分层选择 + 停服断言 + 改写前调 F4 备份 + 失败自动回滚，与成熟范式一致；建议补强"Stale save 检测、退出未保存提醒、清理前排除名单、操作子集勾选卡片"等 UX 细节。

---

## 四、本地（单机/联机）vs 专用服 差异处理

成熟工具如何处理两者不同：

| 维度 | 单机/联机（Host/Co-op） | 专用服（Dedicated） | 工具处理方式 |
|---|---|---|---|
| 路径 | `%localappdata%\Pal\Saved\SaveGames\<YOURID>\<RANDOMID>\` | `steamapps\common\Palworld\Pal\Saved\SaveGames\0\<RANDOMSERVERID>\` | `get_steam_save_path()` 解析 Win/macOS/Linux/Proton；自动扫描 `SaveGames` 下含 `Level.sav` 的目录（扁平/GUID 嵌套兼容） |
| 外层结构 | 无 `0/` 层 | 多一层 `SaveGames/0/` | 迁移时显式处理 `0/` 外层 |
| 主机角色 | 主机固定 `0001.sav`（UID `00000001...001`） | 随机 GUID `.sav` | Fix Host Save = 旧主机角色 ↔ 新角色 **UID 互换**（改写 Players + Level 的 Character/Group 引用 + `_dps.sav` + 深度 `OwnerPlayerUId` 等） |
| WorldOption | 一般无 | `WorldOption.sav` 会**覆盖** `PalWorldSettings.ini`；`DedicatedServerName` 须匹配文件夹名 | 迁移到专用服时**提示删除 `WorldOption.sav`**、校验 `DedicatedServerName` 一致（R5） |
| GamePass | 加密 `.wgs`（Packages 目录，需 `xgp-save-extract` 提取） | — | 独立提取/转换管线（`xgp_save_extract.py` + `game_pass_save_fix.py`），含云同步未完成告警 |
| 备份范围 | Level/Players | Level/Players + `WorldOption.sav` + `LocalData.sav`（changelogs 2.1.3） | 备份动作一并保护这些文件，跳过存档内的 `Backups/` 子目录 |

**结论**：差异核心在"路径解析、主机 vs 随机 UID、外层 `0/`、WorldOption 覆盖、GamePass 加密"五点；PST 用统一 `get_steam_save_path` + Fix Host Save + 迁移提示覆盖全部。本项目 f5 设计已对齐（复用 `find_world_data_dir`、fix_host.rs、R5 提示）。

---

## 五、对我们项目（Palworld Server Manager）的启示

### 5.1 我们已经能做 / 已规划（基于 f5-architecture.md / f5-local-to-server-design.md）

- ✅ **整包世界备份/恢复**（F4 `backup_world`/`restore_world`，支持外部 `dest` 目录）——强于 PST 的固定备份位。
- ✅ **多世界发现 + 多玩家选择 UI**（L1–L3，F4 `discover_worlds` + PlayerPicker）。
- ✅ **整包世界迁移**（文件级拷贝 + WorldOption/DedicatedServerName 提示，P0）。
- ✅ **Fix Host Save / UID 互换**（P0 灵魂步骤，停服断言 + 版本门控 + round-trip 校验 + 失败回滚）。
- ✅ **跨服角色转移**（P1，按子集 character/tech/inventory/pals）。
- ✅ **科技点编辑 + 玩家基础属性**（P1，`edit_tech`/`edit_player_attr`，老板点名项）。
- ✅ **改写前强制备份 + 失败自动回滚契约**（复用 F4）、路径白名单（防穿越）、停服前置。

### 5.2 竞品有但我们还没做（按优先级建议）

| 优先级 | 能力缺口 | 竞品参照 | 备注 |
|---|---|---|---|
| **P1（高 ROI）** | **帕鲁（Pal）深度编辑**：等级/魂/IV/技能/被动/工作适应/幸运/BOSS 旗标、克隆、导入导出、修复非法帕鲁 | PST、Pal-Editor、PSP | 玩家呼声最高；需骨架生成 + GUID 防碰撞（R9），复杂度高 |
| **P1** | **背包/物品深度编辑**：数量增删、装备槽、基地容器、解锁全快速旅行 | PST、PSP | 需按 `ItemContainerSaveData` 容器 ID 定位 |
| **P2** | **公会管理 UI**：改名/换领袖/设等级/解锁实验室/跨公会移人/跨世界合并 | PST、PSP、toolkit | 跨世界合并 PST 也刻意不做，但"同存档管理"应补 UI |
| **P2** | **据点（Base）导出/导入/克隆/位移/调半径** | PST | `MapObjectSaveData` 改写，复杂度高 |
| **P2** | **地图迷雾解锁 / 交互式地图查看器** | PST | `LocalData.sav` 改写 |
| **P2** | **Slot Injector（扩大帕鲁箱槽位）** | PST | 兼容性敏感 |
| **P2** | **存档诊断 / 修复非法数据**：孤儿玩家、损坏条目、负时间戳、非法物品/被动 | PST `save_diagnostic` + Fix Illegal | 我们仅有 round-trip 校验，缺"诊断+修复"产品化 |
| **P3** | **WorldOption.sav 可视化编辑** | PalServerTools、web Server Tool 可读写 | 运维向，PST 也仅 raw 改；建议 P3 |
| **P3（MVP 排除，未来）** | **GamePass(Xbox) ↔ Steam 跨平台转换** | PST、PSP 全支持 | 加密 `.wgs` 门槛高，R10 已排除；可作为差异化亮点远期规划 |
| **运维增强** | **自动定时备份调度、在线玩家管理、RCON 指令、多服/多开管理** | web/k8s Server Tool、PalServerTools、.NET PalServerTools | 我们已是"服务器管理器"，可借鉴其运维面（备份保留天数、白名单踢人、重启、配置页） |

### 5.3 形态/架构启示

- **不要重蹈"纯运维"覆辙**：第 5 类 web 工具只读 .sav 做展示，深度改写弱；本项目既有运维（F1–F4）又有解析改写（F5），应把"服务器运维 + 存档深度编辑"做成一体，形成相对纯运维工具的差异化。
- **GamePass 支持是显著差异化**：PST/PSP 均支持，本项目 MVP 排除；若未来做，可直接复用 PST 的 `xgp_save_extract` + `game_pass_save_fix` 思路（加密 .wgs → 提取 → 转换）。
- **帕鲁编辑是最大单点能力缺口**：竞品中 PST/Pal-Editor/PSP 都把它做深；建议 P1 立项，与科技点/玩家属性共用 `save_edit.rs` 解析底座。

---

## 附：信息置信度与未确认项

- **高置信（已读源码/文档）**：PST 全部功能（9 toolset + 7 manager 方法签名 + changelogs 逐条）、本项目 f5 规划（已读两份设计文档）、Palworld-Pal-Editor 的"仅帕鲁 / 不编辑玩家 / Steam-only"（README 明确）。
- **中置信（联网 README/发布说明）**：PSP 的玩家/公会/据点/GamePass 能力、palworld-server-toolkit 的转移/编辑能力、各 web 运维工具的备份/RCON 能力。
- **未确认（避免断言）**：PSP 是否支持"单玩家存档导出为独立文件"、PSP 跨服公会合并、palworld-server-toolkit 的 GamePass 支持、各工具"用户任意外部目录备份"细节——均已标注"未确认"。
- ⚠ 所有结论均基于本次实际读取的本地文件与联网所得，未凭空编造功能。
