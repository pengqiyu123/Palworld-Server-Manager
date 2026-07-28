# 帕鲁服务器管理器 · 增量 PRD（UI3 修订：路径普适性 / 管理员复制指令 / 概览重构 / 配置ⓘ简介 / 本地存档路径 / 数据迁移对标同行）

> 本轮目标：基于老板实机验收后提出的 6 点新修订反馈，做 UI3 增量修订。
> 定位：Tauri2 + Vue3 桌面单机管理器，面向自建帕鲁专用服的服主。
> 关联：承接 UI2（`docs/prd-ui2-increment.md` + `docs/design-ui2.md`，整体 IA 已定，本轮**不重写**整体信息架构，仅聚焦 6 点变更）。
> 创建：2026-07-XX ｜ 产品经理 许清楚 ｜ 主理人 齐活林

---

## 一、产品目标

本轮聚焦老板在 UI2 验收后提出的 6 点修订反馈，达成四件核心事：①修掉配置/存档相关功能里**机器专属绝对路径写死**导致的"只能在你电脑上跑"的普适性隐患（审计并改用动态探测 + 用户设置推导）；②把管理员密码后的「复制」按钮从复制纯密码改为复制 **`/AdminPassword <密码>`** 指令，使服主能直接粘进游戏聊天框回车获权；③**重构概览页**——删除顶部独立状态/启停卡，把所有内容压成下方单一流程（第 1 步自动获取服务器/Radmin/游戏三应用路径、失败给手动兜底，第 2 步三个启动按钮）；④补齐体验细节——配置项 ⓘ 悬停简介、本地存档同时扫 AppData 与 Steam 并支持手动兜底与点击展开信息、数据迁移页**对标同行**做"左原数据 / 右新数据 / 中间箭头方向"的角色罗列转移 UI。

---

## 二、改动基线（已拍板的三项决策，PRD 必须照此写需求）

| 决策 | 内容 |
|---|---|
| ① 本地存档路径（point 5） | **两者都扫合并**——同时扫 `C:\Users\<user>\AppData\Local\Pal\Saved\SaveGames` 与 UI2 已实现的 Steam 库路径（`steamapps/common/Palworld/Pal/Saved/SaveGames`），合并到「本地单机存档」区块；匹配不到时提供**手动选择目录**兜底。点击某存档 → 显示该存档可获取到的信息（世界名/GUID/玩家数/修改时间/Level.sav 解析概要）。 |
| ② 管理员复制指令（point 2） | 密码后的「复制」按钮点击后复制 **`/AdminPassword <密码>`**（带斜杠前缀），可直接粘到游戏聊天框回车获管理员权限（替代当前纯密码复制）。 |
| ③ 概览重构（point 3） | **去掉顶部独立的状态/启停卡**，所有内容融入下方单一流程：①第 1 步 = 自动获取服务器/Radmin/游戏三个应用路径（自动探测，失败则在按钮旁/下出现三个手动选目录入口，选完自动检测并锁定目录）②第 2 步 = 三个启动按钮（启动服务器 / 启动 Radmin / 启动游戏，复用既有 handler）。无"检查网络端口"步骤（UI2 已删）。 |

---

## 三、竞品调研摘要（A 节 · point 6 明确要求"对标同行"）

> 联网调研时间：2026-07-23。老板原话："数据迁移同行已经遥遥领先，似乎还有官方设计师的参与，非常可靠"——本摘要优先借鉴其信息架构与可视化，不闭门造车。下列 5 个交互范式作为 point 6「左原数据/右新数据/箭头方向」设计的参考基线。

### 调研到的成熟同行项目

| # | 项目 | 链接 | 形态 |
|---|---|---|---|
| P1 | **PalworldSaveTools**（FreddyFunk / deafdudecomputers 活跃维护） | https://github.com/FreddyFunk/PalworldSaveTools | 桌面 GUI（Nuitka 跨平台），最成熟、官方感最强 |
| P2 | **PalworldCharacterTransfer**（jinu0608） | https://github.com/jinu0608/PalworldCharacterTransfer | Python + 打包 exe，专注角色转移 GUI |
| P3 | **Palworld Cross-Save Player Transfer**（tcno.co） | https://hub.tcno.co/games/palworld/player-transfer/ | 浏览器端（文件本地处理不上传） |
| P4 | **fix_host_save.py**（CSDN 教程 / 社区工具） | https://blog.csdn.net/gitblog_00347/article/details/151441330 | 命令行 + GUI，Host↔专用服 GUID 互换 |
| P5 | **Host Havoc Palworld Settings Generator**（配置项参照，point 4 也用） | https://hosthavoc.com/en-GB/wiki/game-servers/palworld/server-settings-reference | 网页可视化配置生成器 |

### 可借鉴的 3–5 个交互范式

**范式 1 · 双栏 + 方向箭头（最贴合老板诉求，来自 P3 tcno.co）**
- 左栏 = **Source world (Old Save)**，拖入/选择完整世界文件夹后列出该世界玩家；右栏 = **Destination world (New Save)**，同样列出玩家；两栏**中间用 `→` 箭头**标记迁移方向。
- 用户勾选要移动的角色 → "Review" 预览 → "Migrate" 执行 → 下载补丁包解压回目标世界。
- **借鉴点**：左原数据 / 右新数据 / 中向箭头的三段式布局，以及"先预览再执行"的安全感。

**范式 2 · 富信息角色卡 + Tab 分栏（来自 P1 PalworldSaveTools）**
- Players / Guilds / Bases 三 Tab；每个**玩家卡显示 name、level、pal count、guild、last online**，可编辑/删除/右键菜单；Character Transfer 工具选源+目标存档，支持**单个或全体**转移，保留角色/帕鲁/背包/科技。
- 每个控件悬停有详细 tooltip 帮助系统（"hover over any button, field, or control to reveal detailed tooltips"）。
- **借鉴点**：角色卡片信息密度（名字/等级/公会/帕鲁数/最后在线），以及"每个控件都有解释"的帮助体系——与本 PRD point 4 ⓘ 一致。

**范式 3 · 源/目标存档 + 玩家文件识别 + 公会 ID 保留（来自 P2 PalworldCharacterTransfer）**
- GUI 选源/目标 player `.sav` + 源/目标 `level.sav`；提供 **"Keep Old Guild ID After Transfer"** 勾选框；明确教用户"如何识别玩家存档文件""先在新世界建同名角色"。
- **借鉴点**：转移前的前置引导（先建角色）+ 公会归属保留选项，应放进本页"注意事项"。

**范式 4 · 下拉源/目标 + 一键执行（来自 P4 fix_host_save.py）**
- 下拉选「新角色（覆盖）/ 旧角色（保留）」+ "Guild fix" 勾选 + "Run Command"。
- **借鉴点**：最简 source→target 映射范式，作为本页 Fix Host Save 区块的既有形态可保留。

**范式 5 · 可视化配置生成器逐项解释（来自 P5 Host Havoc，point 4 也用）**
- 119 项设置，每项**内联解释 + 默认值 + 可导入导出**。
- **借鉴点**：佐证"每项配置给人类可读简介"是行业通行做法（见 B 节）。

### 本 PRD 采用的 point 6 设计基线（综合范式 1+2+3）
> 详见第六节「③ 数据迁移页」：左侧源世界角色卡列表（名字/等级/公会/帕鲁数/最后在线）、右侧目标世界角色卡列表、中间 `→` 方向箭头；支持勾选多个角色、保留公会选项；下方保留既有 Fix Host Save / 整包迁移 / 科技属性编辑区块。

---

## 四、选项简介数据源（B 节 · point 4）

> 结论：**优先取自代码元数据**（项目已有现成数据，无需从零写），不足项用 web 标准文档补齐。

### 数据源判定
- **代码元数据（主来源）**：`src-tauri/src/config.rs::get_config_descriptions()`（行 584–637）已为 **~50 个** PalWorldSettings 选项定义了中文 `description` 字段（如"游戏难度""白天时间速率""帕鲁捕获率""死亡惩罚: None/Item/ItemAndEquipment/All"等），结构为 `ConfigValue { name, value, description, field_type, min, max, step }`，并通过 `src/api/tauri.ts:78` 的 `api.config.getDescriptions()` 暴露给前端。**当前 `ConfigView.vue` 尚未引用该数据**——R4 仅是前端把 `description` 接到每个配置项的 ⓘ 悬停上，无需后端改动。
- **web 标准文档（补充源）**：若需覆盖代码未列的选项（如 `BaseCampMaxNumInGuild`、`RandomizerType`、`CrossplayPlatforms`、`bIsUseBackupSaveData`、`bEnableVoiceChat` 等），用下列标准文档补齐：
  - Palworld Wiki · PalWorldSettings.ini：https://palworld.wiki.gg/wiki/PalWorldSettings.ini
  - Host Havoc · Server Settings Reference：https://hosthavoc.com/en-GB/wiki/game-servers/palworld/server-settings-reference
  - LOW.MS · Server configuration for Palworld：https://low.ms/de/knowledgebase/server-configuration-for-palworld

### 将覆盖的核心选项示例（10 个，均取自代码元数据）

| 选项名 | 当前 description（来自代码） | 来源 | ⓘ 建议增强 |
|---|---|---|---|
| `Difficulty` | 游戏难度 | 代码 | — |
| `DayTimeSpeedRate` | 白天时间速率 | 代码 | 注明 >1 白天更快 |
| `ExpRate` | 经验值倍率 | 代码 | 注明 2=双倍 |
| `PalCaptureRate` | 帕鲁捕获率 | 代码 | 注明 >1 更易抓 |
| `DeathPenalty` | 死亡惩罚: None/Item/ItemAndEquipment/All | 代码 | — |
| `ServerPlayerMaxNum` | 服务器最大玩家数 | 代码 | 注明上限 32 |
| `AdminPassword` | 管理员密码 | 代码 | ⓘ 追加"复制按钮生成 `/AdminPassword <密码>` 可直接在游戏聊天框回车认证"（呼应 point 2） |
| `bEnableInvaderEnemy` | 启用入侵敌人（袭击事件） | 代码 | — |
| `AutoSaveSpan` | 自动保存间隔（秒） | 代码 | 注明默认 30 |
| `WorkSpeedRate` | 工作速度倍率 | 代码 | — |

---

## 五、用户故事（按 6 点各 1 条）

1. **路径普适性（point 1）**：作为服主，我希望涉及文件夹路径的功能（服务器配置、PalWorldSettings.ini、存档扫描等）都能在我和朋友的任意电脑上自动定位，以便不在"只能在你机器上跑"的环境里翻车。
2. **管理员复制指令（point 2）**：作为服主，我希望点管理员密码后的「复制」按钮直接复制 `/AdminPassword <密码>` 指令，以便粘进游戏聊天框回车就能拿到管理员权限，而不是还手敲斜杠。
3. **概览重构（point 3）**：作为服主，我希望概览页去掉顶部冗余的状态/启停卡、改成单一流程——先自动拿到服务器/Radmin/游戏三个路径（失败可手动选），再点三个启动按钮开服联机，以便界面干净、一步到位。
4. **配置项 ⓘ 简介（point 4）**：作为服主，我希望每个配置项旁边有 ⓘ，悬停就能看懂这项是干嘛的、默认多少，以便不查 wiki 也能放心改配置。
5. **本地存档路径（point 5）**：作为服主，我希望本地存档页同时列出 AppData 单机档与 Steam 单机档、匹配不到能手动选目录，且点开某个存档能看到它的世界名/玩家数/修改时间等信息，以便确认这就是我要迁的档。
6. **数据迁移对标同行（point 6）**：作为服主，我希望迁移页像成熟同行那样左边列原世界角色、右边列目标世界角色、中间用箭头标方向，角色卡清楚显示名字/等级/公会等，以便直观地挑角色、放心转移。

---

## 六、需求池（P0/P1/P2，ID R1~R6）

> 优先级：P0 必做 / P1 应做 / P2 待定可选 ｜ 落地屏：S1 概览 / S2 配置 / S7 本地存档 / S8 数据迁移
> 编号对应老板 6 点：R1→①普适性 / R2→②复制指令 / R3→③概览重构 / R4→④ⓘ简介 / R5→⑤本地存档路径 / R6→⑥数据迁移对标

### P0 · 必做

| ID | 标题 | 验收口径 AC |
|---|---|---|
| **R1** | 路径普适性审计（禁止机器专属绝对路径写死） | ①审计并在文档列出所有写死绝对路径处：`src-tauri/src/save_transfer.rs:63-70` 的 `STEAM_LIBRARY_ROOTS`（含 `E:\SteamLibrary`、`D:\SteamLibrary` 等，注释写"老板默认 E:\SteamLibrary"）、`src-tauri/src/save_edit/path_util.rs:16-21` 的同名常量、`src/views/OverviewView.vue:351` 手动选目录 fallback 默认值 `'D:\Steam\steamapps\common\Palworld\PalServer'`；②**Steam 库根改为由 `steam_detect.rs::parse_library_roots` 动态读取 Steam `libraryfolders.vdf`/注册表得出**，移除"仅 E/D/C 盘"写死兜底（或仅作最后兜底且不在主探测路径）；③所有 PalWorldSettings.ini / SaveGames / 备份路径均由 `server_path` 或 `dirs::data_dir()`/`dirs::data_local_dir()` 推导，**禁止写死机器专属绝对路径**；④确认 `config.rs` 备份目录（`dirs::data_dir()`）与配置路径（由 `server_path` 拼接）已合规；⑤`#[cfg(test)]` 内写死本机路径（如 `save_transfer.rs:652`、`steam_detect.rs:184-213`）须为 `cfg(test)` 且 skip 优雅，不得影响 shipped 行为；⑥验证：在"非 E 盘、非 D 盘装 Steam"的机器上，存档双检测与 Steam 库扫描仍可用；`cargo test` 不依赖本机真实路径。 |
| **R2** | 管理员复制指令改为 `/AdminPassword <密码>` | ①定位 `src/views/ConfigView.vue:46` 的「复制」按钮（`onCopyPw`，行 171–199）；②点击后写入剪贴板的内容由纯密码 `pw` 改为 `/AdminPassword ` + **去引号后**的密码（形如 `/AdminPassword mypassword`）；③空密码时禁用复制或提示先填密码；④复制成功提示改为"管理员指令已复制，粘贴到游戏聊天框回车即可获管理员权限"；⑤验证：点击复制，剪贴板 = `/AdminPassword <密码>`；粘贴进游戏聊天框回车即获管理员权限。 |
| **R3** | 概览重构（去顶部卡 + 第1步取三路径含手动兜底 + 第2步三启动按钮） | ①**移除** `OverviewView.vue` 顶部独立状态/启停卡（`.lz-status` 区：状态徽标 + 启/停按钮），不再有常驻启停/状态徽标；②下方改为**单一「启动流程」**：**第 1 步 = 自动获取三个应用路径**（服务器 `server_path` / Radmin VPN 路径 / 游戏路径），进入页面即自动探测，三路径各自显示"已定位/待定位"态；③第 1 步任一路径探测失败时，在该路径项**旁/下出现「手动选择目录」入口**，用户选目录后校验并"锁定"该路径（写回设置）；④**第 2 步 = 三个启动按钮**（启动服务器 / 启动 Radmin VPN / 启动游戏），复用既有 `onStart` / `onLaunchRadmin` / `onLaunchGame` handler；⑤服务器"正在运行"的态以第 2 步「启动服务器」卡片的运行态 + 便捷关服入口表达（不回到顶部常驻卡）；⑥验证：概览页无独立顶部启停卡；三应用路径可自动探测或手动锁定；三启动按钮均能触发对应 handler。 |
| **R5** | 本地存档路径修正（AppData + Steam 合并 + 手动选目录 + 点击显信息） | ①后端 `discover_local_worlds`（或新增合并命令）**同时扫** `C:\Users\<user>\AppData\Local\Pal\Saved\SaveGames`（`dirs::data_local_dir()/Pal/Saved/SaveGames`，可移植）与 UI2 已实现的 Steam 库路径（`steamapps/common/Palworld/Pal/Saved/SaveGames`），合并返回，并为每个 world 标注来源（`appdata` / `steam`）；②前端「本地单机存档（Steam）」区块**更名为/扩展为「本地单机存档」**，合并展示两类结果（来源以标签区分，去重）；③匹配不到时区块内显示**「手动选择目录」入口**（调 `open` 对话框，选中目录回灌为额外扫描根，重扫）；④**点击某 world 卡 → 展开信息面板**，显示世界名 / GUID / 玩家数 / 修改时间 / `Level.sav` 解析得到的概要（如世界设置摘要、玩家列表等可获取字段）；⑤验证：AppData 下有单机档时出现在区块；AppData 与 Steam 合并无重复；手动选目录兜底可用；点击存档展开信息面板。 |

### P1 · 应做

| ID | 标题 | 验收口径 AC |
|---|---|---|
| **R4** | 配置项 ⓘ 简介（悬停显示 description） | ①`ConfigView` 进入时调用 `api.config.getDescriptions()` 获取 `ConfigValue[]`，按 `name` 建索引；②每个配置项行（含常规选项与 AdminPassword 专属区）右侧加 ⓘ 图标，悬停/聚焦显示该项的 `description`（复用既有 `InfoTip.vue` / `Tooltip.vue` 组件）；③`AdminPassword` 的 ⓘ 追加"复制按钮生成 `/AdminPassword <密码>` 可直接在游戏聊天框回车认证"（呼应 R2）；④`description` 缺失的选项（代码未列）用 B 节 web 标准文档补齐兜底文案；⑤验证：悬停任意配置项 ⓘ 显示人类可读简介；不依赖联网也能显示代码内 ~50 项。 |
| **R6** | 数据迁移角色罗列 + 左右对照 + 箭头方向（对标同行） | ①重构 `SaveMigrationView`「③ 跨服角色转移」区块：由当前两个 `<select>` 下拉改为**左（源世界角色卡列表）/ 右（目标世界角色卡列表）/ 中（`→` 方向箭头）**三段式布局；②角色卡显示**名字 / 等级 / 公会 / 帕鲁数 / 最后在线**（数据来自既有 `PlayerPicker` 的 `PlayerEntry`：`nickname`/`level`/`guild_id`/`pal_count`/`last_online`，复用 `PlayerPicker` 或同数据结构）；③支持勾选多个角色参与转移；保留"公会 ID 保留/转移子集"选项（复用既有 `TransferSubsetSelector`）；④加一句前置引导"请先在目标世界用同一账号建好角色"；⑤下方既有 Fix Host Save / 整包迁移 / 科技属性编辑区块保持不变；⑥验证：源/目标两栏列出各自角色卡并带名字等级等信息；中间箭头标方向；勾选角色后执行转移成功。 |

### P2 · 待定 / 可选（默认本轮可不做）

| ID | 标题 | 说明 |
|---|---|---|
| R7 | Oodle 压缩存档提示醒目化 | 检测到 Oodle 压缩存档时 UI 红字/徽标醒目提示并引导解压。**待定/可选**：是否本轮做、是否在 UI 预检测红字引导，见第七节待确认 1。 |
| R8 | 迁移前预览面板 | 在执行角色转移前，仿 tcno.co 提供"Review"预览（列出将被移动的角色及其目标落点）。**可选**：若 R6 时间允许可加，否则留 P2。 |

---

## 七、UI 设计稿描述（文字稿）

> 仅聚焦 6 点变更区域，不重写其他 IA。沿用既有设计令牌（`--r-card` / `--glass-bg` / `--primary` 等）。

### 7.1 概览页（S1）新单流程（R3）

```
┌─────────────────────────────────────────────────────────┐
│ 页面标题：服务器概览                                       │
├─────────────────────────────────────────────────────────┤
│ 启动流程（单一流程，无顶部独立启停卡）                      │
│                                                           │
│ ┌─ 第 1 步：自动获取三个应用路径 ─────────────────────┐  │
│ │ [① 服务器]  已定位 ✓  C:\...\PalServer            [手动选目录] │
│ │ [② Radmin]  已定位 ✓  C:\Program Files\Radmin VPN [手动选目录] │
│ │ [③ 游戏]    已定位 ✓  steam://rungameid/1623730   [手动选目录] │
│ │  · 进入页面即自动探测；任一项失败→该项[手动选目录]高亮，      │
│ │    选目录后校验并"锁定"（写回设置，显示已定位）              │
│ └────────────────────────────────────────────────────┘  │
│ ┌─ 第 2 步：三个启动按钮 ─────────────────────────────┐  │
│ │ [ 启动服务器 ]   [ 启动 Radmin VPN ]   [ 启动游戏 ]     │
│ │  · 复用 onStart / onLaunchRadmin / onLaunchGame         │
│ │  · 「启动服务器」运行中时卡片显运行态 + 优雅/强制关服入口    │
│ └────────────────────────────────────────────────────┘  │
│                                                           │
│ （仪表盘模式下的 FPS/在线/指标卡片保留为只读信息区，       │
│   不归入"被删的顶部启停卡"）                               │
└─────────────────────────────────────────────────────────┘
```

- **删除**：顶部 `.lz-status`（状态徽标 + 启/停按钮）常驻卡。
- **第 1 步**：三应用路径自动探测 + 手动兜底锁定；路径来源——服务器用既有 `steam.detect`、Radmin 用注册表/已知路径探测、游戏用 `steam://rungameid/1623730` 探测可用性。
- **第 2 步**：三启动按钮复用既有 handler；运行状态以第 2 步卡片态表达（不再有独立顶部卡）。
- **命名同步**：无新增路由/导航改名（概览页名沿用），仅视图内部结构调整。

### 7.2 本地存档页（S7）双来源 + 点击展开（R5）

```
┌─────────────────────────────────────────────────────────┐
│ 本地存档（标题沿用 UI2 更名）                               │
├─────────────────────────────────────────────────────────┤
│ 区块：本地单机存档（合并 AppData + Steam）                  │
│  ┌─ world 卡 A（来源:AppData）────────────┐               │
│  │ 💾 世界名        2 名角色 · 128 MB       │               │
│  │ 来源: C:\Users\xxx\AppData\Local\Pal... │               │
│  │ [迁移到服务器]  （点击卡片其余区域→展开信息）│            │
│  └─────────────────────────────────────────┘             │
│  ┌─ world 卡 B（来源:Steam）─────────────┐                │
│  │ 💾 世界名        1 名角色 · 64 MB        │               │
│  │ 来源: D:\Steam\steamapps\common\...     │               │
│  └─────────────────────────────────────────┘             │
│  ┌─ 点击 world 卡展开的信息面板 ──────────┐                │
│  │ 世界名 / GUID / 玩家数 / 修改时间         │               │
│  │ Level.sav 解析概要：世界设置、玩家列表…   │               │
│  └─────────────────────────────────────────┘             │
│  （若两类都未匹配到）→ 显示「手动选择目录」入口              │
├─────────────────────────────────────────────────────────┤
│ 区块：服务器存档（专用服）  —— UI2 既有结构，不变            │
└─────────────────────────────────────────────────────────┘
```

- **合并**：后端同时扫 AppData Local Pal 与 Steam 库，前端「本地单机存档」区块合并展示，来源以标签区分。
- **手动兜底**：匹配不到时区块内出现「手动选择目录」按钮（调 `open` 对话框），选中目录作为额外扫描根重扫。
- **点击展开**：world 卡点击展开信息面板（世界名/GUID/玩家数/修改时间/Level.sav 概要）。
- **命名同步**：区块标题「本地单机存档（Steam）」→「本地单机存档」（去掉 Steam 限定词，因现含 AppData）；UI2 已完成的 Sidebar/router/View 三处「本服存档→本地存档」更名保持不变、无需再改。

### 7.3 数据迁移页（S8）左原/右新 + 箭头（R6，对标同行）

```
宽屏（≥约 720px）：
┌──────────────────────┐      ┌──────────────────────┐
│ 源世界（原数据）        │  ➜   │ 目标世界（新数据）      │
│ [下拉选源世界]          │      │ [下拉选目标世界]        │
│ ┌─ 角色卡 ──────────┐ │      │ ┌─ 角色卡 ──────────┐ │
│ │ ☐ 名字 Lv12        │ │      │ │ ☐ 名字 Lv30        │ │
│ │ 公会: xxx · 帕鲁8   │ │      │ │ 公会: yyy · 帕鲁5   │ │
│ │ 最后在线: 2天前     │ │      │ │ 最后在线: 1小时前   │ │
│ └──────────────────┘ │      │ └──────────────────┘ │
│ ┌─ 角色卡 ──────────┐ │      │ ┌─ 角色卡 ──────────┐ │
│ │ ☐ 名字 Lv8  ...    │ │      │ │ （目标世界角色列表） │ │
│ └──────────────────┘ │      │ └──────────────────┘ │
└──────────────────────┘      └──────────────────────┘
   前置引导：请先在目标世界用同一账号建好角色
   [保留公会 ID] 勾选 · 转移子集(角色/公会/科技/背包/帕鲁)
   [执行角色转移]
窄屏（<720px，自动折行）：源栏 → 箭头(↓) → 目标栏 纵向排列
```

- **左原/右新/中箭头**：源世界角色卡在左、目标世界角色卡在右、中间 `➜` 方向箭头标迁移方向（借鉴 tcno.co 范式 1）。
- **角色卡信息**：名字 / 等级 / 公会 / 帕鲁数 / 最后在线（复用 `PlayerPicker` 的 `PlayerEntry` 数据，借鉴 PalworldSaveTools 范式 2）。
- **多选 + 保留公会**：勾选多个角色，保留"公会 ID 保留"与转移子集（复用 `TransferSubsetSelector`，借鉴 PalworldCharacterTransfer 范式 3）。
- **下方既有区块**（Fix Host Save / 整包迁移 / 科技属性编辑）保持 UI2 结构不变。
- **命名同步**：无新增路由/导航改名（迁移页名「数据迁移」沿用）。

---

## 八、待确认问题（本轮仍存疑、未拍板）

1. **Oodle 压缩存档（R7）是否本轮 UI 红字引导？** PRD 内为 P2 待定项。是否升级为 P1 并红字引导，待老板定；默认保持 P2 待定/可选。
2. **概览第 2 步「关服」入口形态（R3）**：老板要求删顶部启停卡，但服务器运行中仍需能关服。建议「启动服务器」卡片在运行态下变为"运行中 · [优雅关服][强制停止]"（复用既有 `onGracefulShutdown`/`onForceStop`）。是否接受此形态、还是另设常驻小控件？待老板定。
3. **point 5 点击存档显示的信息深度（R5）**：列出"世界名/GUID/玩家数/修改时间/Level.sav 解析概要"是否足够？是否需要更深（如公会数、总游玩时长）？默认按 PRD 列项，深度受 `Level.sav` 解析能力限制，待老板定边界。
4. **R6 是否要"迁移前预览"（R8）**：tcno.co 有 Review 预览步骤。本轮 R6 是否需加预览面板，还是直接执行（执行前已有二次确认弹窗）？默认不加、留 P2，待老板定。
5. **概览第 1 步"游戏路径"探测方式（R3）**：游戏用 `steam://rungameid/1623730` 探测可用性即可，还是需解析出本地 Steam 客户端 exe 真实路径？建议仅探测可用性（无需绝对路径），待老板定。

---

## 九、范围外明确（本轮不做）

- 重写上一轮已定的整体 IA（UI2 的 T1–T8 导航序与九页结构不变，仅 S1/S2/S7/S8 局部变更）。
- Fix Host Save 手动 GUID 简化 UI（沿用 UI2 既有整包迁移 / Fix Host Save 能力，UI 不暴露简化版）。
- 公网联机端口映射 / UPnP（沿用 UI2 Radmin 方案）。
- 存档深度解析（公会/仓库存档全量，沿用 UI2 P2）。
- 多服管理 / 跨平台（仅 Windows 单服）。

---

*本增量 PRD 基于老板实机验收 6 点反馈 + 已拍板三项决策撰写；竞品调研（A 节）于 2026-07-23 联网完成，来源项目见第三节；选项简介数据源（B 节）优先取自 `config.rs::get_config_descriptions()` 代码元数据，缺口用 palworld.wiki.gg / Host Havoc / low.ms 标准文档补齐。路径普适性（R1）审计结论基于实际代码 grep：写死路径集中于 `save_transfer.rs`/`path_util.rs` 的 `STEAM_LIBRARY_ROOTS` 与 `OverviewView.vue:351` 的手动选目录 fallback，配置文件路径（`config.rs`）已合规使用 `dirs::*` 与 `server_path` 推导。*
