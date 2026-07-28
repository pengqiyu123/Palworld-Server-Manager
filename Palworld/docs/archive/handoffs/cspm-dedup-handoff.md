# Palworld 角色迁移 — CSPM 记录级去重 交接文档（2026-07-26）

> 适用对象：接手的另一个 AI / 工程师。本文档自包含，无需看历史对话即可开工。
> 项目：`F:\study\Palworld-Server-Manager\Palworld`（Tauri2 + Rust `src-tauri`，前端 Vue3）。
> 当前状态：**解析层已修；去重逻辑有致命 bug 未落地，整项任务未完成。**

---

## 1. 任务来龙去脉（为什么做这个）

- 前期交付了深度调研报告 `F:\study\Palworld-Server-Manager\palworld-character-migration-report.md`，核心结论之一是：**`fix_host_save_multi` 缺少 CSPM（CharacterSaveParameterMap）记录级去重**。
- 场景：把本地单机角色 `000…0001`（煜，lv48）顶替到服务器真实身份 `4E239D4F…0000`（煜2，空 lv2）。顶替后 `Level.sav` 里会同时存在「重绑后的煜」和「煜2 原有空角色」两条同 `PlayerUId` 记录 → 冲突。正确行为 = 删掉煜2 原有记录，只留煜。
- 同行共识（PalworldSaveTools 等）也要求重绑后做记录级去重。这是报告头号 follow-up。

## 2. 已验证完成的部分（可信）

通过读源码 + grep 独立确认，**不是**听信工程师自报：

| 项 | 文件:行 | 状态 |
|---|---|---|
| `SavFile::parse` 改为 `read_with_hints` + `palworld_hints()`（40 条，取自 PalworldSaveTools `paltypes.py`） | `sav_io.rs:113-116, 263` | ✅ 已落地，真机 `Level.sav` 的 `MissingHint` 解析问题已解决 |
| `real_save_integration` 测试模块（对真实 `E:\1A91A615\Level.sav` 副本只读/跑 Phase B，零写原档） | `sav_io.rs:1700-1775`（含 `practice_migration_on_live_copy` 等） | ✅ 已落地 |
| Phase B 三阶段骨架（预扫描 / 重绑 / 去重）写在 `fix_host_save_multi` 内 | `fix_host.rs:490-581` | ✅ 骨架在，但去重实际不生效（见 §3） |
| `element_player_uid` / `element_instance_id` 助手 | `fix_host.rs:414, 437` | ✅ 已落地，对 `StructPropertyValue::CustomStruct` 取 `key.PlayerUId/InstanceId` 正确 |

## 3. 🔴 阻断性 Bug（核心未交付）

**工程师自报"已完成 Map+Array 兼容修复"，但独立核验证明该修复并未落地。**

- `fix_host.rs:465` `level_char_map_structs` 仍调用 `sav_io::as_struct_array(cmp)`
- `fix_host.rs:474` `level_char_map_structs_mut` 仍调用 `sav_io::as_struct_array_mut(cmp)`
- `sav_io.rs:946-959` 的 `as_struct_array` / `as_struct_array_mut` **只匹配 `ArrayProperty::Structs`**（即合成测试夹具）。
- **真实存档的 CSPM 是 `MapProperty`**（不是 Array），证据：
  - `sav_io.rs:283-288` 的 40 条 hint 明确写出 `CharacterSaveParameterMap.MapProperty.Key.StructProperty` / `.Value.StructProperty`；
  - gvas crate 源码 `C:\Users\pengq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\gvas-0.11.0\src\properties\map_property.rs:62-73` 确认 CSPM 走 `MapProperty::Properties { value: HashableIndexMap<Property, Property>, .. }`。
- **后果**：真机 `Level.sav` 上 `as_struct_array` 返回 `None` → `level_char_map_structs` 返回 `None` →
  - Phase 1 预扫描收集不到 `new_uid` 原有 InstanceId 集合；
  - Phase 3 `retain` 删不掉任何条目；
  - **去重在真机上静默 no-op**（三阶段全跳过），煜2 不会被删 → 任务实质失败。合成 Array 夹具测试仍绿（掩盖了问题）。

## 4. 🔴 连带：真机测试计数器也写错（必须一起改）

`sav_io.rs:1733` `cspm_player_uid_eq_count`（真机测试用它断言"煜保留、煜2 删除" → `assert_eq!(count, 1)`，`sav_io.rs:1725-1728`）内部也用 `as_struct_array(cmp)`（`sav_io.rs:1750`）。
- 即便 §3 的生产代码修好，这个计数器在真机 Map 上仍返回 `0` → 断言 `count==1` 会 **FAIL**。
- 结论：真机去重测试目前**不可能通过**，除非计数器也改成 Map 感知。

## 5. ✅ 精确修复方案（照做即可，已读完 gvas 类型）

### 5.1 `sav_io.rs` 新增两个公开助手
```rust
// 取 MapProperty::Properties 的底层 IndexMap（不可变 / 可变）
pub fn as_props_map(p: &Property) -> Option<&HashableIndexMap<Property, Property>> {
    match p {
        Property::MapProperty(MapProperty::Properties { value, .. }) => Some(value),
        _ => None,
    }
}
pub fn as_props_map_mut(p: &mut Property) -> Option<&mut HashableIndexMap<Property, Property>> {
    match p {
        Property::MapProperty(MapProperty::Properties { value, .. }) => Some(value),
        _ => None,
    }
}
```
- `HashableIndexMap` / `MapProperty` 在 `sav_io.rs` 已可用（见 `sav_io.rs:32` `use ...map_property::MapProperty` 及 L584 `HashableIndexMap` 用法）。
- 内层容器是 `value.0`，类型为 `indexmap::IndexMap<Property, Property>`，支持 `.iter()` / `.retain(|k,v|...)` / `.get_index()` 等。

### 5.2 `fix_host.rs` 用统一访问器替换 Array-only 的 `level_char_map_structs` / `_mut`
删除旧两个函数，改为：
```rust
/// 取 CSPM 容器 Property（Array 或 Map），供下面两助手使用
fn cspm_property<'a>(gvas: &'a GvasFile) -> Option<&'a Property> { /* 同原导航: worldSaveData→CharacterSaveParameterMap */ }
fn cspm_property_mut<'a>(gvas: &'a mut GvasFile) -> Option<&'a mut Property> { /* 同上 mut 版 */ }

/// 只读：返回所有 CSPM 元素的 CustomStruct 引用（Array / Map 都覆盖）
fn cspm_elements(cmp: &Property) -> Vec<&StructPropertyValue> {
    let mut out = Vec::new();
    if let Some(structs) = sav_io::as_struct_array(cmp) {
        for s in structs { out.push(s); }
    } else if let Some(map) = sav_io::as_props_map(cmp) {
        for (_k, v) in map.0.iter() {
            if let Property::StructProperty(sp) = v {
                if let StructPropertyValue::CustomStruct(_) = &sp.value {
                    out.push(&sp.value);
                }
            }
        }
    }
    out
}

/// 可变：删除不满足 keep 的 CSPM 元素（Array 用 retain；Map 用 IndexMap::retain）
fn cspm_retain(cmp: &mut Property, mut keep: impl FnMut(&StructPropertyValue) -> bool) {
    if let Some(structs) = sav_io::as_struct_array_mut(cmp) {
        structs.retain(|el| keep(el));
    } else if let Some(map) = sav_io::as_props_map_mut(cmp) {
        map.0.retain(|_k, v| {
            if let Property::StructProperty(sp) = v {
                if let StructPropertyValue::CustomStruct(_) = &sp.value {
                    return keep(&sp.value);
                }
            }
            true // 非预期结构，保留
        });
    }
}
```
- `element_player_uid` / `element_instance_id`（`fix_host.rs:414,437`）签名不变，直接吃 `&StructPropertyValue`，对 Array/Map 都适用，**无需改**。

### 5.3 改写 `fix_host_save_multi` 的 Phase 1 / Phase 3
- Phase 1（预扫描，`fix_host.rs:516-541`）：把 `if let Some(structs) = level_char_map_structs(&gvas0) { for el in structs {...} }` 换成 `if let Some(cmp) = cspm_property(&gvas0) { for el in cspm_elements(cmp) {...} }`。
- Phase 3（去重，`fix_host.rs:554-573`）：把 `if let Some(structs) = level_char_map_structs_mut(&mut gvas) { structs.retain(|el| {...}); }` 换成 `if let Some(cmp) = cspm_property_mut(&mut gvas) { cspm_retain(cmp, |el| { !(element_player_uid(el)==Some(new_guid) && element_instance_id(el).map_or(false,|iid| to_delete.contains(&iid))) }); }`。

### 5.4 修真机测试计数器 `cspm_player_uid_eq_count`（`sav_io.rs:1733-1775`）
把内部的 `as_struct_array(cmp)` 分支也加上 `as_props_map` 分支（遍历 `value.0` 的 value，取 `Property::StructProperty(sp).value` 的 CustomStruct 再读 `key.PlayerUId`）。否则 §4 的断言永远失败。

### 5.5 补一个 Map 形状的合成去重测试（关键防回归）
现有合成测试 `make_synthetic_level`（`fix_host.rs:1467`）只造 `ArrayProperty::Structs` 夹具 → 只覆盖 Array 路径。新增：
- `make_synthetic_map_level(...)`：用 `MapProperty::Properties` 造 CSPM（key/value 均为 `StructProperty`→`CustomStruct`，value 含 `key={PlayerUId,InstanceId}`）；
- 测试 `phase_b_dedup_keeps_source_only_map`：跑 `fix_host_save_multi`，断言煜2（new_uid 原有）被删、煜保留。
这样能保证"真机 Map 路径"被合成测试锁死，不再被 Array 夹具的绿掩盖。

> 顺带：同款 `as_struct_array` 误用也存在于 `transfer.rs::get_char_map_structs_mut`（`transfer.rs:283`）——真实角色转移在真机上同样读不到 CSPM。**本轮不修**（超出本任务范围），仅记录为已知 follow-up。

## 6. 构建 / 测试约束（沙箱必读，否则白等）

- **F: 是 exFAT** → 易残留损坏的 `.cargo-build-lock`（OS error 5）。用 NTFS 目标目录 `C:\pal_build`，并 `MSYS_NO_PATHCONV=1`，跑前先 `rm -f C:/pal_build/debug/.cargo-build-lock`。
- 完整 `cargo test`（Tauri crate）首次编译 **45–76 分钟**（工程师上一轮 76 分钟）。增量编译快很多，前提是 `C:\pal_build` 有缓存。
- 测试过滤器只接受一个名字：`cargo test save_edit` 或 `cargo test 具体测试名`，不要传两个名字（会报 "unexpected argument"）。
- **沙箱铁律**：只跑 `cargo test` / `vue-tsc --noEmit` 做逻辑验证。**绝不**在沙箱启 `tauri:dev` / dev server（弹不出窗口、纯浪费 45min）。**真实存档只读副本**测试（`sav_io.rs` 已复制到临时目录、零写原档）；绝不碰 `E:\1A91A615` 原档。
- 真机验收由**老板**在本机 `npm run tauri:dev` 做，不是沙箱的事。
- **不要 git 提交**未经老板拍板。仓库在 `F:/study` 与 `F:/study/ai-learning`（均已 `git init`）。

## 7. 验收标准（何时算真完成）

跑 `cargo test save_edit`（用 `C:\pal_build`）：
1. 之前因 `MissingHint` 失败的 2 个真实样本测试（`gvas_roundtrip_lossless_probe_real`、`practice_migration_on_live_copy`）应**变绿**（40 条 hint 已覆盖）；
2. 新增的 `phase_b_dedup_keeps_source_only_map`（Map 形状去重）**通过**；
3. 真机测试 `sav_io.rs` 里 Phase B 副本断言 `cspm_player_uid_eq_count == 1` **通过**（煜保留、煜2 删除）；
4. 既有合成 Array 测试 `phase_b_three_phase_dedup_keeps_source_only` 仍绿。
全绿 + 工程师手跑确认 = 可交付。然后老板本机 `npm run tauri:dev` 真机验收。

## 8. 关键文件索引

| 文件 | 作用 |
|---|---|
| `Palworld/src-tauri/src/save_edit/fix_host.rs` | Phase B 三阶段去重主体（**待修 §3/§5**） |
| `Palworld/src-tauri/src/save_edit/sav_io.rs` | GVAS 解析 + 40 条 hint + 真机测试（**待加 §5.1/§5.4 助手与计数器**） |
| `Palworld/src-tauri/src/save_edit/transfer.rs:283` | 同款 Array-only bug（已知 follow-up，本轮不修） |
| `C:\Users\pengq\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\gvas-0.11.0\src\properties\map_property.rs` | gvas `MapProperty` 类型定义（确认 `value.0` 是 `IndexMap`） |
| `F:\study\Palworld-Server-Manager\palworld-character-migration-report.md` | 任务来源报告（§4 头号 follow-up = 本任务） |

## 9. 一句话给接手 AI

解析层已修好，但去重逻辑只认合成 Array 夹具、真机 Map 上静默失效；按 §5 把 CSPM 访问改成 Array+Map 双兼容（含修测试计数器 + 补 Map 合成测试），再用 `cargo test save_edit`（目标 `C:\pal_build`）验证全绿即可收尾。
