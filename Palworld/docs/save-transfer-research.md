# Palworld 「预置存档 / 角色数据转移」调研与可行方案

> 调研模式产出 · 产品经理：许清楚（software-product-manager）
> 阅读依据：`docs/finale-prd.md`、`docs/finale-design.md`、`reference-projects/` 下 3 个同行项目、主项目 `src-tauri/src/` 现状，以及社区实践（PalworldSaveTools / PalworldCharacterTransfer / 多家主机商迁移指南）。
> 范围：只做调研与方案设计，不含实现代码。

---

## 一、同行项目对比

| 项目 | 技术栈 | 是否支持「存档备份」 | 是否支持「角色跨服转移」 | 实现方式 | 关键结论 |
|---|---|---|---|---|---|
| **zaigie/palworld-server-tool**（`zaigie-palworld-server-tool-vpn`） | Go + 自研 Python `sav_cli` | ✅ 有（本地存档复制 + bbolt 元数据） | ❌ 无 | **文件级整目录复制**：`internal/source/local.go::CopyFromLocal` 找 `Level.sav` → glob 所有 `*.sav` → 复制 + 递归 `Players/` 到临时目录；`sav_cli` 仅**解析 .sav 用于展示**玩家/公会（只读） | 备份=纯文件拷贝，不解析/不改写 sav 内容；无角色迁移能力 |
| **amantu-qbit/palworld-server-manager**（`amantu-qbit-tauri-manager-vpn`） | Rust + Tauri（与本项目经理架构最近） | ⚠️ 深度解析+编辑 | ⚠️ 具备能力但未见显式"跨服迁移"命令 | **Rust 解析 GVAS**：`bridge/src/save/` 解压 zlib → `parse_gvas` → 解码 `CharacterSaveParameterMap` 为 players/pals；`edit/` 模块在解压字节缓冲上做**定位→修补→重打包**（保留未改动字节），编辑前备份原文件、原子替换、**服务器运行时拒绝写入** | 具备"解析+改写角色/公会"的完整内嵌能力，是做角色转移最理想的底座 |
| **PalworldSaveTools**（`PalworldSaveTools-main`） | Python（PySide6 GUI）+ `palworld_save_tools` 解析库 | ✅ 有 | ✅✅ **唯一明确实现** | `backup_manager.py`：`export_player_backup`/`import_player_backup` 把某玩家的 Level.sav 中 `CharacterSaveParameterMap` 条目 + `Players/<uid>.sav` 打包成 `.pst7`，再导入目标世界（插入/替换目标 Level.sav 条目 + 写出目标 `Players/<uid>.sav`）；另有基地/建筑迁移 `export_base_backup` | 是"角色数据转移"的现成范式。注意：**其 import 不处理公会归属（GroupSaveDataMap），也不改 SteamID**——印证了本文的"坑" |

**补充社区范式（web 调研）：** `PalworldCharacterTransfer`（Python GUI）是"角色跨服转移"的另一现成实现：需 4 个文件（源/目标各 `Players/<id>.sav` + `Level.sav`），**目标世界须先建好同名角色再覆盖**，并提供 "Keep Old Guild ID" 选项处理公会。多家主机商（XGamingServer / DoomHosting）的迁移指南也印证：整包世界复制最稳，角色迁移最坑。

---

## 二、Palworld 存档结构要点

**目录约定（主项目复用 `settings.server_path`）：**
`{server_path}/Pal/Saved/SaveGames/<WorldName>/`

| 文件 / 目录 | 内容 | 决定什么 |
|---|---|---|
| `Level.sav` | 世界主体：地形、建筑、对象、野生帕鲁、共享数据 | **世界** |
| `LevelMeta.sav` | 世界元数据（名称、创世设置） | 世界名/设置 |
| `WorldOption.sav` | 世界设置快照，**会覆盖** `PalWorldSettings.ini` | ⚠️ 迁移时通常要删，否则服设被旧世界覆盖 |
| `LocalData.sav` | 服务器本地状态 | 局部状态 |
| `Players/<id>.sav` | 每个玩家一个角色存档：等级、背包、帕鲁箱、位置、科技点 | **角色**（id 一般为 SteamID64 17 位或 UUID） |
| `Level.sav` 内 `CharacterSaveParameterMap` | 角色身份条目（key=PlayerUId+InstanceId，含 NickName/Level/IsPlayer） | 角色在世界中的引用 |
| `Level.sav` 内 `GroupSaveDataMap` | 公会（成员含 player_uid） | **公会归属** |

**跨服 / 单机→联机迁移的关键约束（"坑"）：**

1. **SteamID 绑定**：`Players/<id>.sav` 复制到新服后，游戏**无法自动把文件和新服玩家的 Steam ID 重新关联**（"ID 绑定丢失"）。主流做法：玩家先在新服建好角色（生成一个全新 `<id>.sav`），再用旧数据**覆盖**该 .sav 并同步覆盖 Level.sav 中对应 `CharacterSaveParameterMap` 条目；否则玩家登录被强制新建角色、旧数据"消失"。
2. **公会归属在 Level.sav，不在 Players/**：角色所属公会的引用（group_id）在 `CharacterSaveParameterMap`/`GroupSaveDataMap`，**只在 `Players/<id>.sav` 里复制而不动 Level.sav 公会条目 → 角色迁过去后不再属于原公会（或引用悬空）**。要保留公会需同时处理 `GroupSaveDataMap`（目标服须存在该公会，或一并迁移公会）。
3. **帕鲁/建筑分离**：角色个人帕鲁（队伍+帕鲁箱）在 `Players/<id>.sav` 内；但**基地/建筑/箱中帕鲁在世界（Level.sav + MapObjectSaveData）里**，单独迁角色带不走基地。
4. **服务器须完全停止**：运行中替换文件会被游戏自动保存覆盖。
5. **跨平台（Win↔Linux）可能丢角色数据** → 同 OS 优先；**Xbox/Game Pass** 存档是加密 `.wgs`，无法迁到 Steam 服。
6. **WorldName 须匹配** `GameUserSettings.ini` 的 `DedicatedServerName` 文件夹名，否则加载成空世界。

---

## 三、术语澄清：老板说的两个词可能指什么

> ⚠️ **重要命名警示**：主项目现有 `src-tauri/src/presets.rs` 里的 "preset" 是**服务器配置预设**（default / pve-friendly / pvp-competitive / speedrun 几套 ini 参数），**不是存档**。"预置存档"一词极易与之混淆，需单独对齐命名。

老板说的"预置存档转移、角色数据转移"至少可能有 3 种解读：

- **(a) 预置存档 = 整套世界存档预制分发**：把 `Level.sav + LevelMeta.sav + Players/ + ...` 一整包做成"模板/种子服"分发给别人或自己快速开服。本质=整目录文件拷贝（zaigie `CopyFromLocal` / 主机商上传）。**风险最低、实现最简单。**
- **(b) 角色数据转移 = 单个玩家从 A 服迁到 B 服**：把某玩家 `Players/<id>.sav` + Level.sav 中其角色条目迁移到目标世界（PalworldSaveTools `import_player_backup` / PalworldCharacterTransfer）。涉及 SteamID 绑定 + 公会处理，是"坑"最集中的场景。
- **(c) 单机(合作)存档 → 联机服**：本地 co-op 存档（`%LOCALAPPDATA%/Pal/Saved/SaveGames/<SteamID>/...`）搬到专用服。主机角色文件是 `000000...001.sav`，需 host-save-fix 工具重新绑定到新服身份；非主机角色可正常迁移。这是 (a)/(b) 之外的特殊一类。

**推荐解读**：最可能是 **(a) 整包世界预置 + (b) 单人角色跨服**。建议先确认是否含 (c)。MVP 优先做 (a)（纯文件拷贝，安全），(b) 做"导出/导入角色包"但**首版明确不保证公会保留**（UI 提示风险）。

---

## 四、本项目 MVP 建议方案

主项目现状核查：`finale-prd.md` 称"后端已有 `config.listBackups` 骨架"——经核对 `config.rs`，该命令实为 **PalWorldSettings.ini 配置备份**（`list_config_backups` / `restore_config_backup`，目录 `config-backups/`），**并非世界存档备份**。`presets.rs` 是配置预设。→ **世界存档的备份/转移目前一行代码都没有，需从零做。** 路径可复用 `settings.server_path` 拼出 `Pal/Saved/SaveGames/<WorldName>/`。

### P0 — 存档备份 / 恢复（= 预置存档的生成与还原，纯文件层）
- 备份：`copy_dir` 整个世界文件夹到 `%AppData%/PalworldServerManager/world-backups/<时间戳>/`（比 zaigie 的"glob + 递归 Players"更稳，直接整目录拷贝）。
- 恢复：先停服 → 用备份整体覆盖目标世界目录（写前对现有目录做时间戳备份，原子替换）；注意排除/提示 `WorldOption.sav`。
- 校验：复制后对比文件数/大小。
- 风险低，纯 Rust `std::fs`，**无需解析 sav**。

### P1 — 角色导出 / 导入（角色数据转移）
- 导出：给定源世界 + 玩家 uid → 收集 `Players/<uid>.sav` + 源 Level.sav 中该玩家 `CharacterSaveParameterMap` 条目（起步可直接"复制两个文件 + 记录条目"，进阶再做 `.pst7` 风格的自定义容器）。
- 导入到目标服：
  1. 前置校验：目标服已存在该玩家（已建角色、有 `Players/<uid>.sav`）；否则提示"请先在目标服用同一账号建一个角色"。
  2. 停服 → 覆盖目标 `Players/<uid>.sav` + 替换/插入目标 Level.sav 的 `CharacterSaveParameterMap` 条目。
  3. **首版不处理公会**：UI 明确告知"公会归属可能丢失"，留 P2 做 `GroupSaveDataMap`。
  4. 写前备份、原子替换、重开后让玩家登录验证。
- UID 映射：首版假设源/目标同 SteamID（文件同名），**不做 ID 改写**；跨身份改写（host-save-fix 那种）列为 P2。

### 本轮不做
基地/建筑迁移、公会整包迁移、Xbox 迁移、SteamID 改写、实时热迁移（必须停服）。

---

## 五、风险清单

| ID | 风险 | 影响 | 缓解 |
|---|---|---|---|
| R1 | SteamID 绑定失败 → 玩家登录被强制新建角色 | 旧数据覆盖错对象/看似丢失 | 导入前校验目标 `Players/<uid>.sav` 存在且 uid 一致；引导"先建角色再导入" |
| R2 | 只迁 Players 导致角色脱离原公会 | 公会归属丢失 | 首版 UI 明示；P2 处理 `GroupSaveDataMap` |
| R3 | 服务器运行中替换被自动保存覆盖 | 迁移无效/回滚 | 导入/恢复前强制停服（复用现有启停逻辑），失败回滚 |
| R4 | 跨平台（Win↔Linux）丢数据 | 角色损坏 | MVP 仅支持本机同 OS（老板为 Windows 专用服），提示勿跨系统搬 |
| R5 | sav 格式随游戏版本变化 | 解析/改写失败 | 基于社区解析库/参考结构；标注工具版本需随 Palworld 更新；所有编辑前先整目录备份 |
| R6 | 覆盖错误世界/错误玩家 | 数据事故 | 导入前二次确认 + 显示源/目标世界名与玩家名 |
| R7 | WorldName 不匹配 `DedicatedServerName` | 加载空世界 | 提示用户核对文件夹名 |

---

## 六、待老板确认问题（需拍板）

- **Q1** 两个词到底指什么？是否包含 (c) 单机→联机？**推荐 (a)+(b)。**
- **Q2** "预置存档"的"预设"是"整包世界模板"，还是像现有 `presets.rs` 那样的**服务器配置预设**？（二者完全不同，需对齐命名避免混淆。）
- **Q3** 角色转移是否要求**保留公会**？保留则工作量显著上升（需处理 `GroupSaveDataMap`）。
- **Q4** 角色转移是否要支持"跨 Steam 账号改写身份"（host-save-fix 场景）？还是仅同账号同 SteamID？
- **Q5** 备份/角色包存放位置与是否加密/压缩？（参考 `.pst7` 用 zstd+brotli。）
- **Q6** 是否需要"备份自动滚动"（`bIsUseBackupSaveData` / 定时备份），还是纯手动？
- **Q7** 目标服来源：仅本机另一个世界目录，还是也支持从外部文件/别处导入？
