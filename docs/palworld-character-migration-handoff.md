# 幻兽帕鲁角色迁移（三方对比）工程落实交接单

> **已废止（2026-07-27）**：本文把真实 CSPM 误判为需要“去重”，且混淆了 Fix Host 与通用角色转移。实机成功结果、正确算法和下一版决策以 [`palworld-migration-success-record-2026-07-27.md`](./palworld-migration-success-record-2026-07-27.md) 为准；不得继续按本文 P0 去重规格实现。

> 面向对象：编程 AI「Codex」
> 用途：本文件为**自包含、可直接落地实现**的工程规格与交接说明。研究阶段已结束，本文件不重复调研，只交付可执行规格。
> 项目根路径：`F:/study/Palworld-Server-Manager/Palworld/`（Tauri2 + Vue3 + TS 桌面应用，Rust 后端）
> 同行参考路径：`F:/study/Palworld-Server-Manager/reference-projects`

---

## 1. 项目当前状态总结

- 三方对比深度研究**已完成**：自研方案（U01 / v2）、同行基准（`PalworldSaveTools` 等）、官方立场均已梳理并确认结论。
- 自研 v2 方案已具备较完整重绑链（`fix_host_save_multi` + 公会名修复 + `StopServerAssertion` 安全闸门 + 快照 + 回滚），但**唯一已知核心缺口是重绑后未做 CSPM 记录级去重**。
- 迁移后 `Level.sav` 会同时存在「重绑后的旧角色」与「`new_uid` 原有角色」两条同 UID 记录，导致冲突——这是恢复实机迁移前**必须补齐**的一刀。
- 已有的解析/探针能力（`read_players_from_level` 等）与安全闸门（停服断言、快照、回滚）可直接复用，无需手搓底层。
- **待 Codex 落实**：P0 补 CSPM 去重、P1 真档副本验证、P2 本机停服+备份+`npm run tauri:dev` 执行 B+C 阶段验收。

---

## 2. 三方对比结论速览

| 来源 | 做法 | 关键发现 | 缺口 / 风险 |
|------|------|----------|-------------|
| **自研**（U01 方案） | `swap_guids` 字节级 3-pass 交换（old→TEMP[尾字节 XOR 0xFF]→new→old）+ 文件名交换 + `_dps` 容器 ID 修补（`patch_dps`） | 字节级对称交换，概念简单、可溯源 | 依赖字节扫描，对 FString(32-hex) 公会名无效；U01 路线已被 v2 取代 |
| **自研**（v2 方案） | `fix_host_save_multi`（`replace_guid_structured` 只改 Guid 结构化重绑）+ `patch_guild_group_name`（`replace_ascii_32hex_in_place` 显式替换 32-hex FString 公会名）+ `StopServerAssertion` 安全闸门 + 快照 + 回滚 | 结构化重绑 + 安全闸门齐备，是实机迁移的技术基线 | **缺口：重绑后未删除 `new_uid` 原有 CSPM 条目（缺记录级去重）**，会产生同 UID 双记录冲突 |
| **同行**（cheahjs/deafdudecomputers `PalworldSaveTools`，基准） | 全结构化 JSON 重序列化 + **CSPM 去重** + 对称交换 + 公会修复(deep_swap) + copy_dps + 文件名交换 | 重绑后做 CSPM 去重——**正是自研缺的那一刀** | 全重序列化路线与自研结构化路线不同，仅去重思路可借鉴 |
| **同行**（zaigie） | 只读分析 | 仅分析，无可执行迁移 | 不适用 |
| **同行**（amantu） | 纯 REST（save_world / save_connection / save_bridge） | 接口化存档读写 | 无角色重绑实现 |
| **官方** | 仅提供**帕鲁**转移（World Transfers for Pals / 1.0 Global Palbox）+ 反作弊声明 | **角色迁移无官方工具** | 网传 `PalMigrationTool` 仅 `supercraft.host` 单一第三方主张 → 标〔待官方证实〕，**不可作为实现依据** |

---

## 3. 待 Codex 解决的核心工程问题

> 优先级定义：P0 = 阻断实机迁移的硬缺口；P1 = 正确性/安全性验证；P2 = 流程与验收执行。

### P0 — 补 `Level.sav` 的 CSPM 记录级去重（阻塞级）
- **问题**：`fix_host_save_multi`（见 `fix_host.rs:418`）完成 old→new 重绑后，未在目标 `Level.sav` 的 `CharacterSaveParameterMap`(CSPM) 中删除 `new_uid` 原有条目。
- **后果**：重绑后的旧角色与 `new_uid` 原有角色两条同 UID 记录共存 → 加载冲突 / 数据漂移。
- **目标**：参考 `PalworldSaveTools/fix_host_save.py` 的 CSPM 去重逻辑，在重绑链末端补「删除 `new_uid` 原有 CSPM 条目」一步。详见第 4 节规格。

### P1 — 真档副本验证（正确性与回归）
- 用**真实玩家存档副本**（非生产档、非沙箱生成档）跑通重绑 + 去重全链。
- 用 `read_players_from_level`（见 `world_copy::f5_world_summary_by_path_impl` 调用链）作为只读探针，断言：重绑后 CSPM 中 old 角色已迁移、new 原有条目已被移除、无同 UID 重复。
- 配合 `cargo test` 单测覆盖 `replace_guid_structured` / `replace_ascii_32hex_in_place` / 去重函数。

### P2 — 本机停服 + 备份 + `npm run tauri:dev` 执行 B+C 阶段验收（流程级）
- 实机 UI 验收**必须由人在本机**执行（增量编译 + 有显示器）。
- 沙箱内**禁止**为「看 UI」启动 `tauri:dev`（headless + exFAT + cargo 坏锁，全量编译慢且弹不出窗口）。
- 流程：副本 → 停服 → 备份 → 执行迁移 → 本机 `npm run tauri:dev` 跑 B+C 阶段验收。

---

## 4. 详细实现规格

> 通用铁律（见第 3 节 / 第 5 节）：所有迁移逻辑**必须跑在副本上**；严禁手搓 `SavFile::load`+`parse` 裸打印，先 grep 现成解析函数（如 `read_players_from_level`）复用。

### 4.1 P0：CSPM 记录级去重（可照抄规格）

**目标文件 / 函数**
- 主逻辑落点：`fix_host.rs` 的 `fix_host_save_multi`（L418）。在「完成 old→new 结构化重绑（`replace_guid_structured`）与公会名修复（`patch_guild_group_name`）」之后、写回存档之前，新增去重步骤。
- 去重实现建议新增函数：例如在 `fix_host.rs` 内新增 `dedup_csmp_by_uid`，或复用 `sav_io.rs` 的 CSPM 读写能力。
- 复用探针：`world_copy::f5_world_summary_by_path_impl` → `read_players_from_level`（枚举 `Level.sav` 的 CSPM，作为只读探针，判定条目归属）。

**算法描述**
1. 重绑完成：old 角色记录已被改写为 `new_uid`（即其 `PlayerUId` / 关联 UID 字段指向 new）。
2. 此时 `Level.sav` 的 `CharacterSaveParameterMap` 中可能存在两条逻辑上指向 `new_uid` 的记录：
   - 记录 A：由 old 重绑而来（**保留**）。
   - 记录 B：`new_uid` 在目标存档中**原有**的角色（**删除**）。
3. 去重：以 `new_uid` 为键，遍历 CSPM，删除「属于 `new_uid` 原有、且非本次重绑来源」的条目；保留重绑后的 old 条目。
   - 判定依据参考 `PalworldSaveTools/fix_host_save.py` 的 CSPM 去重（对称交换后移除被覆盖方原记录）：通过记录的 UID 字段与「本次迁移的 (old_uid, new_uid) 映射」比对——凡 `uid == new_uid` 且来源标记非 old 重绑者，删除。
4. 写回：删除后重新序列化 CSPM，确保 `Level.sav` 内 `new_uid` 仅对应**一条**角色记录。

**插入点（精确）**
- 在 `fix_host.rs:418` `fix_host_save_multi` 内部、`replace_guid_structured`（来自 `sav_io.rs:208`）与 `patch_guild_group_name`（`fix_host.rs:512`，内部 `replace_ascii_32hex_in_place` 在 `fix_host.rs:545`）执行完毕、**存档落盘之前**插入去重调用。

**伪代码 / 步骤**
```rust
// 在 fix_host_save_multi 内，重绑 + 公会名修复之后：
let migration = (old_uid, new_uid);
// 1) 读取目标 Level.sav 的 CSPM（复用 read_players_from_level 枚举，或已有解析器）
let mut csmp = read_character_save_parameter_map(&target_level_sav)?;
// 2) 去重：删除 new_uid 原有条目，保留 old 重绑而来的条目
csmp.retain(|entry| {
    let uid = entry.player_uid();           // 取该记录的 UID 字段
    if uid == new_uid {
        // 仅保留「本次由 old 重绑而来」的那条；原有 new_uid 条目返回 false 被删除
        entry.migration_source() == Some(old_uid)
    } else {
        true
    }
});
// 3) 写回 CSPM 到 Level.sav（走既有序列化路径）
write_character_save_parameter_map(&mut target_level_sav, csmp)?;
// 4) 继续既有落盘 / 快照流程
```
> 注：上述 `player_uid()` / `migration_source()` / `read_character_save_parameter_map` / `write_character_save_parameter_map` 为占位签名，**Codex 须对照项目现有 CSPM 数据结构与 `read_players_from_level` 的真实字段名落地**，不要伪造字段。

**依赖**
- `read_players_from_level`（只读探针，确认 `Level.sav` 的 CSPM 枚举方式）。
- `replace_guid_structured`（`sav_io.rs:208`）已完成 old→new 重绑。
- `patch_guild_group_name`（`fix_host.rs:512`）已完成公会名修复。
- 同行参考：`PalworldSaveTools/fix_host_save.py` 的 CSPM 去重段（对称交换后移除被覆盖方）。

---

### 4.2 P1：真档副本验证规格

**目标文件 / 函数**
- 测试用例落点：在现有 `cargo test` 体系内新增集成测试，或扩展 `save_edit.rs` 的 `migrate_world_v2`(L520) / `run_migration_v2_with_guard`(L295) 相关测试。
- 验证探针：复用 `read_players_from_level`（经 `f5_world_summary_by_path_impl`）枚举目标副本 CSPM。

**算法 / 步骤**
1. 取一份**真实玩家存档副本**（拷贝自生产档，原档不动）。
2. 在副本上执行 `fix_host_save_multi` 全链（含新增去重）。
3. 用 `read_players_from_level` 读取副本 `Level.sav` 的 CSPM，断言：
   - old 角色已存在且 UID 指向 new；
   - `new_uid` 原有条目已被移除；
   - 无两条同 UID 记录（count(`uid == new_uid`) == 1）。
4. `cargo test` + `vue-tsc --noEmit` 均通过。

**依赖**：P0 去重实现；`read_players_from_level`；真实副本存档。

---

### 4.3 P2：本机停服 + 备份 + `npm run tauri:dev` 验收规格

**目标文件 / 函数**
- 流程编排：`save_edit.rs` 的 `StopServerAssertion`(L158) / `run_fix_host_with_guard`(L184) / `run_migration_v2_with_guard`(L295，备份段 L310–324) / `rollback_migration_v2_impl`(L345) / `migrate_world_v2`(L520)。

**步骤**
1. 副本：将生产存档复制为迁移工作副本，原档零改动。
2. 停服：`StopServerAssertion`（L158）确保服务器已停。
3. 备份：`run_migration_v2_with_guard`（L295）的备份段（L310–324）生成快照。
4. 执行：跑 B+C 阶段（重绑 + 去重 + 公会修复）。
5. 本机验收：由人在**本机**执行 `npm run tauri:dev`（增量编译 + 显示器），走 UI 流程验收；沙箱**禁止** `tauri:dev`。
6. 异常回滚：`rollback_migration_v2_impl`（L345）在失败 / 无法解析块时回滚到快照。

**依赖**：`StopServerAssertion`、快照、回滚均已具备；本机有显示器与 cargo 增量环境。

---

## 5. 安全闸门与回滚

### 5.1 已有能力（Codex 直接复用，勿重复造）
- **`StopServerAssertion`**（`save_edit.rs:158`）：迁移前停服安全闸门。
- **快照 / 备份**：`run_migration_v2_with_guard`（`save_edit.rs:295`）内备份段 L310–324。
- **回滚**：`rollback_migration_v2_impl`（`save_edit.rs:345`）。
- **重绑与修复原语**：`replace_guid_structured`（`sav_io.rs:208`）、`patch_guild_group_name`（`fix_host.rs:512`）、`replace_ascii_32hex_in_place`（`fix_host.rs:545`）、`patch_dps`（`fix_host.rs:128`）/ `patch_dps_file`（`fix_host.rs:189`）、`swap_guids`（3-pass，U01 用）。

### 5.2 Codex 必须遵守的铁律
1. **绝不在生产 / 线上档直接迁移**：全程在**副本**上跑，原档不动。
2. 迁移前**必须停服**（走 `StopServerAssertion`）+ **必须备份**（走 L310–324 快照）。
3. **禁止在沙箱里为「看 UI」启动 `tauri:dev`**（沙箱 headless + exFAT + cargo 坏锁，全量编译既慢又弹不出窗口）。逻辑验证用 `cargo test` + `vue-tsc --noEmit`；**实机 UI 验收由人在本机**用 `npm run tauri:dev` 执行。
4. 遇**无法解析的块立即中止**（绝产半重绑档），防止格式漂移；触发 `rollback_migration_v2_impl` 回滚。
5. 遵循 PST 官方 recipe：**新角色须先在目标存档创建**，再执行重绑。
6. 角色身份真源铁律：`Level.sav` 的 `CharacterSaveParameterMap`(CSPM) 为角色身份真源；`Players/<uid>.sav` 仅 SaveData 元信息、无昵称——解析/判定一律以 CSPM 为准。
7. `group_name` 是 FString(32-hex ASCII) **非 16 字节 GUID**，字节交换扫不到，须显式替换（`replace_ascii_32hex_in_place`）；self-guild 的 `group_name` 即 = 玩家 UID 的 FString。

---

## 6. 验收标准

### 6.1 单元 / 集成测试建议
- **单元**：
  - `replace_guid_structured`（`sav_io.rs:208`）：给定 old/new UID，断言仅 Guid 字段被重绑、其他字节不变。
  - `replace_ascii_32hex_in_place`（`fix_host.rs:545`）：给定 32-hex FString，断言公会名被精确替换。
  - **新增去重函数**：构造含 old 重绑条目 + new 原有条目的 CSPM fixture，断言去重后 `new_uid` 仅 1 条且为 old 来源。
- **集成**（P1）：
  - 真实存档副本上跑 `fix_host_save_multi` 全链，用 `read_players_from_level` 断言：old 已迁移、new 原有条目已删、无同 UID 重复。
  - `cargo test` 全绿；`vue-tsc --noEmit` 无类型错误。

### 6.2 手动验证步骤（P2，人在本机）
1. 取生产存档 → **复制**为工作副本（原档零改动）。
2. 本机 `npm run tauri:dev` 启动 UI。
3. 在 UI 触发迁移流程：系统先走 `StopServerAssertion` 停服 → 走 L310–324 备份快照。
4. 执行 B+C 阶段（重绑 + 去重 + 公会修复）。
5. 验证：进入游戏 / 读取存档，确认角色身份正确、无重复角色、公会名正确、无加载报错。
6. 异常时确认 `rollback_migration_v2_impl` 已回滚至快照，原档无损。

---

## 7. 给 Codex 的注意点 / 交接清单

> 以下条目 actionable，建议逐条勾选后再开工。

- [ ] **先 grep 现成解析函数**：确认 `world_copy::f5_world_summary_by_path_impl` → `read_players_from_level` 真实签名与字段，用作 CSPM 只读探针；**严禁手搓 `SavFile::load`+`parse` 裸打印**。
- [ ] **P0 第一优先**：在 `fix_host.rs:418` `fix_host_save_multi` 内、重绑 + 公会名修复之后插入 CSPM 去重；参考 `PalworldSaveTools/fix_host_save.py` 的去重段。
- [ ] 去重判定以 `new_uid` 为键，删除 `new_uid` 原有条目、保留 old 重绑条目，确保 CSPM 内 `new_uid` 仅 1 条。
- [ ] **不要重建**已具备能力：`StopServerAssertion`(L158)、快照(L310–324)、回滚(L345)、`replace_guid_structured`(L208)、`replace_ascii_32hex_in_place`(L545) 直接复用。
- [ ] 全链路**只跑副本**，原档不动；迁移前停服 + 备份。
- [ ] 逻辑验证用 `cargo test` + `vue-tsc --noEmit`；**绝不在沙箱跑 `tauri:dev` 看 UI**。
- [ ] 实机 UI 验收交人：本机 `npm run tauri:dev`（增量 + 显示器）。
- [ ] 遇无法解析块 → 中止 + 回滚，绝不产出半重绑档。
- [ ] 遵守角色身份真源铁律：CSPM 为角色真源，`Players/<uid>.sav` 无昵称；`group_name` 是 32-hex FString 须显式替换。
- [ ] 遵循 PST 官方 recipe：新角色须先在目标存档创建。
- [ ] 待核对补齐（恢复实机前）：自研 fix_host 仅确认覆盖 ①②③④⑤⑥；⑦⑧⑨⑩⑪③（公会 / 建筑 builder ID / 箱 / 告示牌 / 死亡袋 / 门锁 / 地图解锁 / 技术 / Paldeck / 任务等约 200 处重绑清单）疑似缺口，须逐项核对补齐。
- [ ] 网传 `PalMigrationTool` 标〔待官方证实〕，**不作为实现依据**。

---

> 本报告由 AI 深度研究团队生成，重要实现决策请经人工核验。
