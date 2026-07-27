# 角色、公会与备份拆分计划 · UX 架构评审

> 评审人：ArchitectUX · 日期：2026-07-27
> 评审对象：《角色、公会与备份拆分实施计划》（老板 2026-07-27 下发）
> 评审方法：完整阅读 `src-tauri/src/save_edit*` 后端 + `src/views/*` 前端 + `docs/*` 9 份设计文档，交叉验证计划与代码现状的差距

---

## 一、评审结论

计划的**产品方向正确**——取消"接管单机主机"组合按钮、拆为三个独立操作、引入工作流清单持久化、备份三层化，这些都是对的。但计划在**技术语义**和**历史决策对齐**两个层面有盲点，直接按计划落地会踩三个坑：

1. **语义低估**：计划用"拆分"措辞，但 `transfer.rs` 现状是"字节级整替换 + 随机 GUID"，`fix_host.rs` 现状是"双向交换"——计划要的"保留目标 UID + 字段级合并"和"单向绑定"是**重写**，不是拆分。工作量被措辞掩盖。
2. **历史冲突**：计划"3 个独立操作"与 `save-migration-research-validation-2026-07.md`"首选整世界迁移，不是拆成三个互不相关的导入操作"**直接冲突**，必须显式消解。
3. **关键遗漏**：计划没提"CSPM 记录级去重"（重绑后目标 Level.sav 会同时存在两条同 UID 记录→冲突），这是 working memory 已记录的实机迁移前置条件。

下面分七层给出优化。

---

## 二、关键发现（三层真相）

### 2.1 后端真相（`src-tauri/src/save_edit.rs` + `save_edit/`）

| 计划项 | 现状 | 差距 |
|---|---|---|
| `run_phase_a/b/c` 独立 command | 是 `migrate_world_v2` 的字段开关（`save_edit.rs:248-277`），A 与 B+C 互斥，B 与 C 必须同请求 | 小改（拆字段即可） |
| `transfer_full_character` 独立接口 | `transfer_character` 存在，但 `TransferSubset` 字段定义后**完全未消费**（`models.rs:68`）；角色转移是 `Players/<old>.sav` 字节级整替换 + `gen_unique_guid` 随机新 GUID（`transfer.rs:177`） | **大改（重写）** |
| 角色转移保留目标 UID/InstanceId/GroupId | 全部用随机新 GUID 覆盖（`transfer.rs:177`） | **大改** |
| `bind_original_host_guild` 独立接口 | 不存在；公会逻辑耦合在 `fix_host::fix_host_save_multi` 内（`fix_host.rs:720`） | 中改（剥离） |
| 公会重绑覆盖 `GroupProperty`/`group_name`/`builder`/`palbox` | `fix_host.rs` 已覆盖 admin/members/handles/last_modifier/marker_owner + `_dps`；**未覆盖** GroupProperty/group_name/builder/palbox | 中改（补全） |
| 备份 full/rollback/operations 三层 | 仅 `_backups/<world>/<id>` + `_migration_backups/<id>/0/` 两层 | 中改 |
| `MigrationWorkflow` 清单 | 不存在 | 大改（新增 schema + 持久化） |
| 备份位置静默回退 | `settings.backup_root` 空则回退 `_backups`，无 UI 警告（`save_edit.rs:49-53`） | 小改 |
| 服务器运行时拒绝写 | `ensure_server_stopped` 已真实进程检测（child + pid 双路） | 无差距 |
| 应用重启后可恢复状态 | 无任何持久化工作流状态 | 大改 |
| 写入失败逐字节恢复 | 仅目录级 `remove_dir_recursive + copy_dir_recursive` 回滚 | 中改（需 staging+rename） |

**关键约束**：`migrate_singleplayer_to_server_v2_impl`（`save_edit.rs:248-256`）强制 A 与 B+C 互斥、B 与 C 必须同请求。**这正是新计划要打破的约束**——把 B（角色）和 C（公会）拆成独立操作。

### 2.2 前端真相（`src/views/SaveMigrationView.vue` 等）

11 条 UX 断点已定位，最关键的 5 条：

1. **PlayerPicker 复选框 vs V2 单选约束冲突**：`PlayerPicker.vue` 是 `<input type="checkbox">` 多选，但 `canV2`（`SaveMigrationView.vue:270-276`）要求 `length===1`，用户勾两个后按钮 disabled 且**无原因提示**。
2. **进度黑箱**：单一 `busy` ref + "处理中…"，V2 三阶段迁移只显示 `phase_b_changed`/`phase_c_changed` 数字（`SaveMigrationView.vue:479-481`），无"准备备份→写入存档→检查结果"任何可视化。
3. **lastV2BackupId 刷新即丢**：存组件 `ref('')`（`SaveMigrationView.vue:237`），切路由/刷新就丢失回滚能力。
4. **术语混乱**：section 标题"② 角色与公会绑定（修复主机存档）"同时用两个名词；按钮"绑定角色与公会"；ConfirmDialog 又用"绑定到服务器账号"——用户无法建立概念映射。
5. **页面职责过载**：`SaveMigrationView` 同时承载"整包迁移 + 角色绑定 + 科技编辑 + 属性编辑"四类任务，③ 节"数据修改"与迁移流程无强关联。

### 2.3 文档对齐真相

- **C1 冲突（最关键）**：`save-migration-research-validation-2026-07.md` 明确"首选整世界迁移，不是拆成三个互不相关的导入操作"。新计划"3 个独立操作"措辞与之冲突。
- "接管单机主机"、`run_phase_a/b/c`、`migrate_world_v2` 在 9 份文档里**完全无记录**——这些是代码层演进，文档没跟上。
- full/rollback/operations 三层、三按钮弹窗、指纹去重、最近 3 份、删除保护——**全部首次提出**，历史只有单层 `_backups`。
- `MigrationWorkflow` 清单首次提出，且与 finale 系列"不持久化实时状态"原则冲突。

---

## 三、必须先消解的战略冲突

### 3.1 C1：拆分 vs 整世界迁移

这不是真冲突，是**措辞冲突**。深挖两层文档的"拆分"对象不同：

- 验证文档反对的是"把**世界迁移本身**拆成多个互不相关的导入步骤"——指单机→服务器的主路径应一键完成。
- 新计划做的是"把**后置的角色/公会绑定操作**从世界迁移里解耦"——世界迁移仍是主路径，只是绑定可独立执行。

**优化建议**：在新计划开篇显式声明：

> 本计划保留"整世界迁移"为单机→服务器的主路径（与 `save-migration-research-validation-2026-07.md` 一致）。拆分的对象是**后置的角色身份与公会归属操作**——现状把它们耦合在 `migrate_world_v2` 的 B/C 阶段（必须同请求执行），拆分后可独立执行、独立回滚、独立验证。

### 3.2 语义陷阱：交换 vs 转移 vs 绑定

这是计划最大的技术盲点。三个动词对应三套完全不同的实现：

| 动词 | 现状实现 | 计划要求 | 性质 |
|---|---|---|---|
| **交换**（fix_host 现状） | 双向对称交换 old↔new，两端都保留 | — | 已实现 |
| **转移**（transfer 计划） | 字节级整替换 + 随机 GUID | 保留目标 UID + 字段级合并源角色数据 | **重写** |
| **绑定**（guild 计划） | 接受任意 (old,new) 映射 | 只接受原主角色 + 原公会，前置等价校验 | **重写** |

**优化建议**：把计划里的"拆分"改为"重写 + 解耦"。`transfer_full_character` 和 `bind_original_host_guild` 不是从现有代码拆出来，是按新契约重写。`fix_host_save_multi` 的双向交换逻辑可保留为内部工具函数，但三个独立 command 的语义是新的。

### 3.3 关键遗漏：CSPM 记录级去重

working memory 已记录（2026-07-26）：

> 恢复实机迁移前必须补 CSPM 记录级去重：重绑 old→new 后目标 Level.sav 会同时存在"重绑后的旧角色"与"new_uid 原有角色"两条同 UID 记录→冲突，须先删 new_uid 原有 `CharacterSaveParameterMap` 条目。

计划全文未提此点。**必须在 `transfer_full_character` 的契约里增加前置步骤**：转移前先删除目标 UID 在目标 Level.sav 的 CSPM 原有条目，再写入源角色数据。否则转移后游戏内会出现角色身份冲突。

---

## 四、七个维度的优化建议

### 4.1 信息架构

**计划已对**：迁移页分段任务切换器、世界存档页加"备份与回滚"页签、属性/等级/科技移回玩家管理。

**补强三点**：

1. **分段切换器 + 步骤指示器**：纯 tab 切换会丢失时序依赖（世界迁移 → 创建角色 → 角色转移 → 公会恢复）。在 tab 上方加步骤指示器（见评审图 1），"恢复原公会"tab 在未完成角色转移前 `disabled` + tooltip 说明"请先完成角色转移"。
2. **"恢复原公会"空状态**：如果 workflow 清单不存在（用户直接跳到这个 tab），显示"请先完成世界迁移和角色转移"的**空状态引导**，而不是 disabled——让用户知道下一步该做什么。
3. **删除 `/backup` 占位页**：现状 `/backup` 路由指向 `PlaceholderView`（`router/index.ts:58-71`），计划把备份合并进 `/saves` 的页签后，这个占位页应删除，避免侧边栏出现两个备份入口。

### 4.2 交互流程

**计划已对**：双单选角色列表、"继续转移角色"串联入口、只读摘要。

**补强四点**：

1. **角色列表单选改造**：`PlayerPicker` 现状是复选框，必须改为单选模式。两个列表中间的箭头下方放一致性提示"等级/背包/帕鲁/科技转移 · 公会关系不变"，而不是页面顶部——用户视线在两个列表之间时正好看到。
2. **"继续转移角色"预填来源**：计划说世界迁移成功后"继续转移角色"仅切换并预填表单。但源世界已迁移走，"源角色"信息从哪来？**必须从 workflow 清单的 `original_host_character` 字段读取**——这要求 `migrate_world` 执行时就记录原单机主角色身份（PlayerUId + InstanceId + NickName）。预填时自动选中源角色，用户只需在目标服务器角色列表里单选。
3. **"等待游戏验证"是前端引导**：计划把"等待游戏验证"列为 5 阶段进度之一。但后端无法真正感知游戏内验证（除非轮询 RCON，侵入性大）。**优化**：前 4 阶段（检查服务器→准备备份→写入存档→检查结果）是后端真实进度，第 5 阶段"等待游戏验证"是**前端引导**——显示"请启动游戏进服验证背包/帕鲁/公会/据点"，并提供"验证成功→关闭工作流"和"验证失败→一键回滚"两个按钮。
4. **备份列表"来源"字段**：计划要"世界名/时间/大小/角色数/版本/来源"。"来源"是关键字段——让用户理解"这个备份怎么来的"（手动备份/世界迁移/角色转移/公会恢复）。`WorldBackupInfo` 类型（`types/tauri.ts:309-315`）现状只有 4 字段，必须扩展。

### 4.3 状态机

**计划已对**：应用重启后可恢复状态但不自动继续。

**补强**：状态机必须明确六个主状态 + 两个分支状态（见评审图 2）：

```
created → world_migrated → character_transferred → guild_bound → verified → closed
                                    ↓
                              failed → rolled_back → closed
```

- `created`：创建工作流 + 创建完整备份（full/）
- `world_migrated`：世界迁移完成，**记录原单机主角色 + 原公会**（供后续转移/绑定用）
- `character_transferred`：角色转移完成，保存回滚包（rollback/）
- `guild_bound`：公会恢复完成，保存回滚包
- `verified`：用户在游戏内验证完成（前端触发）
- `closed`：工作流关闭，完整备份可删
- `failed`：任一步失败，已记录错误
- `rolled_back`：已自动或手动回滚，可关闭

**关键**：`world_migrated` 状态必须记录 `original_host_character`（PlayerUId + InstanceId + NickName）和 `original_guild`（GroupUId + 名称），这是 `bind_original_host_guild` "只接受原单机主角色"的语义来源。

### 4.4 契约边界

**计划已对**：三个独立接口、目标身份保留、公会等价校验。

**补强三点**：

1. **三个接口必须接受 `workflow_id`**：不是独立参数。`transfer_full_character(workflow_id, source_char_id, target_char_id)` 和 `bind_original_host_guild(workflow_id)` 都从 workflow 清单读取上下文。这样才能保证"公会恢复只接受原单机主角色"——原主角色信息存在清单里，不接受前端传入。
2. **CSPM 记录级去重前置**（见 §3.3）：`transfer_full_character` 执行前，先删除目标 UID 在目标 Level.sav 的 CSPM 原有条目，再写入源角色数据。这是计划遗漏的 P0 前置步骤。
3. **等价校验是前置断言**：计划说"目标公会数据前后必须通过等价校验"。这个校验应该是 `bind_original_host_guild` 的**前置断言**——执行前读目标公会当前状态，与 workflow 清单记录的"原公会"做等价比对，不一致就拒绝（公会冲突）。而不是事后校验。

### 4.5 备份策略

**计划已对**：三层结构、指纹去重、最近 3 份、删除保护、活动工作流引用禁止删除。

**补强三点**：

1. **完整备份 + 每步回滚包的双层回滚**：现状"每次操作新快照"其实更安全（不回滚到陈旧状态，`save_edit.rs:314` 注释明示）。计划"共用一份完整备份"节省空间。**优化**：两者都保留——完整备份（迁移前基线）+ 每步回滚包（仅修改文件）。UI 要区分两个回滚动作："回到迁移前"（用完整备份）和"撤销单步操作"（用回滚包）。
2. **指纹去重优化**：现状 `find_matching_backup`（`save_edit.rs:68-84`）是逐文件逐字节比较整目录，开销大。**优化**：用文件级 `(path, size, hash)` 三元组做指纹，hash 用 xxHash 或 blake3（比逐字节快一个数量级）。
3. **"最近 3 份"规则细化**：计划说"每个世界只保留最近 3 个已完成回滚包"。应该是"**每个工作流最多 3 个回滚包**"——一个工作流最多 3 步操作（角色转移 + 公会恢复 + 可能的重试），每步一个回滚包，正好 3 个。完整备份不在此限（永久保留）。

### 4.6 错误恢复

**计划已对**：暂存、回读验证、替换日志、失败自动恢复、服务器运行时拒绝。

**补强两点**：

1. **staging + rename 原子写**：计划要"任一文件写入失败后逐字节恢复"。现状是目录级 `remove_dir_recursive + copy_dir_recursive`（`save_edit.rs:221-223`），不是逐字节。要满足这个测试，必须引入临时文件 + rename 原子写：写 `.sav.tmp` → 回读验证 → `rename(.sav.tmp, .sav)`。rename 在同一文件系统是原子的，失败时 `.sav` 仍是旧文件。
2. **半写文件检测**：故障注入测试要覆盖"写入 Level.sav 中途进程被杀"场景。回滚后要验证所有 `.sav` 仍可解压（`SavFile::load` 不报错），现状测试 `midwrite_failure_rolls_back_to_prechange`（`save_edit.rs:896-939`）已覆盖此点，但仅注入 rename 失败，未注入写入中途 kill。补充测试。

### 4.7 测试金字塔

**计划已对**：测试先行、负向测试、故障注入、Vitest、Playwright、F:\1 副本、PalworldSaveTools 对照。

**补强三点**：

1. **CSPM 去重测试**：覆盖 §3.3 场景——目标 Level.sav 已有 new_uid 的 CSPM 条目，执行 `transfer_full_character` 后，new_uid 应只有一条 CSPM 条目（源角色数据），不存在重复。
2. **公会等价校验测试**：`bind_original_host_guild` 前置断言——目标公会当前状态与 workflow 清单记录的"原公会"不一致时，必须拒绝执行。构造"目标公会已被其他人加入"场景，断言拒绝。
3. **工作流重启恢复测试**：杀进程后重启，读取 `operations/<operation-id>.json`，UI 应显示"工作流进行中：角色转移已完成，待恢复公会"，且**不自动继续执行**。用户点击"继续"才执行下一步。

---

## 五、实施顺序建议

计划说"测试先行"，但有几个依赖顺序必须明确：

```
第 0 步：MigrationWorkflow 清单 schema + 持久化
         （三个独立接口都依赖 workflow_id，没有清单就无法串联）
         ↓
第 1 步：完整备份层（full/）+ 指纹去重优化
         （工作流开始前要创建完整备份）
         ↓
第 2 步：migrate_world 独立接口
         （现状 migrate_world_to_server 改签名，接受 workflow_id，最易）
         ↓
第 3 步：transfer_full_character 重写
         （最难：字段级合并 + 保留目标 UID + CSPM 去重，工作量最大）
         ↓
第 4 步：bind_original_host_guild 从 fix_host 剥离
         （补全 GroupProperty/group_name/builder/palbox + 前置等价校验）
         ↓
第 5 步：前端分段切换器 + 步骤指示器
         （契约稳定后再做正式 UI，与计划"后端契约和失败测试稳定后再做 UI"一致）
```

**关键**：第 0 步清单 schema 必须最先做，且要定义清楚 `original_host_character` 和 `original_guild` 的字段——它们是第 3、4 步的语义基础。

---

## 六、风险清单

| 风险 | 等级 | 说明 | 缓解 |
|---|---|---|---|
| `transfer_full_character` 字段级合并工作量被低估 | **高** | 现状是字节级整替换，要改为解析 PlayerSaveData 后按字段合并（等级/属性/科技/背包/装备/帕鲁/容器/DPS），每个字段都要定位 + 合并 + 回写 | 拆为子任务逐字段实现，先等级/属性（易），再背包/装备（中），最后帕鲁/容器/DPS（难） |
| CSPM 记录级去重遗漏 | **高** | 计划未提，working memory 已记录为实机迁移前置条件 | 在契约里显式列为 P0 前置步骤 + 测试覆盖 |
| C1 文档冲突未消解 | 中 | 新计划与验证文档措辞冲突，落地后可能被质疑"违背历史调研" | 在计划开篇显式声明拆分对象（见 §3.1） |
| "等待游戏验证"被理解为后端能力 | 中 | 后端无法感知游戏内验证，若实现成轮询 RCON 侵入性大 | 明确为前端引导 + 用户手动确认（见 §4.2.3） |
| 完整备份共用 vs 每步快照的 trade-off | 中 | 共用节省空间但回滚会回到迁移前（丢失中间状态）；每步快照更安全但占空间 | 两者都保留，UI 区分两个回滚动作（见 §4.5.1） |
| `fix_host` 双向交换逻辑被废弃但未清理 | 低 | 拆分后 `fix_host_save_multi` 可能成为死代码 | 重写完成后删除 + 清理前端 `fixHostSave` invoke 封装（`api/tauri.ts:180`） |

---

## 七、给老板的一句话

计划方向对，但"拆分"措辞掩盖了两个重写（角色转移 + 公会绑定）和一个遗漏（CSPM 去重）。建议按五步顺序落地：先做 workflow 清单 schema → 完整备份层 → migrate_world → transfer_full_character（最难）→ bind_original_host_guild → 前端。其中 `transfer_full_character` 的字段级合并是最大风险，建议拆为子任务逐字段实现。

---

**ArchitectUX 评审完成** · 2026-07-27 · 基于 save_edit.rs / save_edit/ / 9 份 docs 完整阅读
