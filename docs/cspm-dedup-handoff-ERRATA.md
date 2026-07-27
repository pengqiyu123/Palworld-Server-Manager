# CSPM 去重交接文档 — 勘误 / 校正（2026-07-26 主理人独立核验后）

> 配套文档：`docs/cspm-dedup-handoff.md`
> 核验方式：直接读 `Palworld/src-tauri/src/save_edit/{fix_host.rs, sav_io.rs, world_copy.rs}` 源码，非听信自报。
> 用途：把本勘误 **随 `cspm-dedup-handoff.md` 一起** 交给 Codex，否则 Codex 照 §5 改完，**真机去重仍是静默 no-op**。

---

## 已确认属实的部分（handoff §3 / §4 准确，不必改）

- **生产 bug 属实**：`fix_host.rs:460-474` 的 `level_char_map_structs` / `_mut` 调 `sav_io::as_struct_array` / `as_struct_array_mut`（`sav_io.rs:973-985`），这两个函数**只匹配 `ArrayProperty::Structs`**。真实 `Level.sav` 的 CSPM 是 `MapProperty::Properties` → `as_struct_array` 返回 `None` → Phase 1（`:520`）/ Phase 3（`:558`）整段被 `if let Some(...)` 跳过 → 真机去重不生效。✅
- **测试计数器 bug 属实**：`sav_io.rs:1777` `cspm_player_uid_eq_count` 内部 `as_struct_array(cmp)` 在真机 Map 上返回 `0` → `assert_eq!(count, 1)`（`:1752`）必 FAIL。✅

---

## ⚠️ 勘误 1：§5.1「在 sav_io.rs 新增 as_props_map」——函数已存在，勿重复加

- `as_props_map` **已存在于** `world_copy.rs:280`（签名 `pub(crate) fn as_props_map(p: &Property) -> Option<&HashableIndexMap<Property, Property>>`）。
- 它是 `pub(crate)`，同模块内的 `fix_host.rs` / `sav_io.rs` 都能直接 `world_copy::as_props_map(...)` 调用，**不要**在 sav_io.rs 再定义一遍（会重定义冲突）。
- **真正缺失的**是 `as_props_map_mut`（可变版）。全仓库 grep 仅 `world_copy.rs:280` 有 `as_props_map`，**没有任何 `_mut` 变体**。Codex 应新增 `as_props_map_mut`（镜像 `as_props_map`，匹配 `MapProperty::Properties` 返回 `&mut`），放在 `world_copy.rs`（保持就地复用），或按需 `pub(crate)`。

## ⚠️ 勘误 2（致命）：§5.2 的 `cspm_elements` / `cspm_retain` 对 Map 值的匹配写错，照抄仍 no-op

handoff §5.2 这样写 Map 分支：

```rust
} else if let Some(map) = sav_io::as_props_map(cmp) {
    for (_k, v) in map.0.iter() {
        if let Property::StructProperty(sp) = v {        // ← 错：真机 Map 值不是这个变体
            if let StructPropertyValue::CustomStruct(_) = &sp.value { out.push(&sp.value); }
        }
    }
}
```

**为什么错**：`world_copy.rs:287-291` 明确记载——gvas 对 `MapProperty::Properties` 的**键/值**走 `include_header=false`，返回的是裸 `Property::StructPropertyValue` 变体，**不是** `Property::StructProperty`。所以 `if let Property::StructProperty(sp) = v` 在真机 Map 条目上**永远不匹配** → `cspm_elements` 推空 → `cspm_retain` 什么也不删 → 即使按 handoff §5.2 改完，**真机去重依旧静默 no-op**（只是恰好让合成 Array 测试继续绿）。

**正确做法**：复用项目已有的 `world_copy.rs:293` 的 `as_struct_value` 助手，它同时兼容两种包装：

```rust
fn as_struct_value(p: &Property) -> Option<&StructPropertyValue> {
    match p {
        Property::StructProperty(s) => Some(&s.value),
        Property::StructPropertyValue(spv) => Some(spv),  // ← 真机 Map 值走这条
        _ => None,
    }
}
```

改写后的 Map 分支应长这样（与 `world_copy.rs:617` 用法一致，已验证可行）：

```rust
} else if let Some(map) = world_copy::as_props_map(cmp) {
    for (_k, v) in map.0.iter() {
        if let Some(sv) = as_struct_value(v) {            // 兼容 StructProperty + StructPropertyValue
            if matches!(sv, StructPropertyValue::CustomStruct(_)) { out.push(sv); }
        }
    }
}
```

> 注意：`as_struct_value` 当前是 `world_copy.rs` 私有 `fn`。Codex 要么把它改成 `pub(crate)`，要么在 `fix_host.rs` 内联同样的二臂 `match`。推荐前者（避免重复逻辑）。

---

## 给 Codex 的落地清单（校正版）

1. `world_copy.rs`：把 `as_struct_value` 改 `pub(crate)`；新增 `as_props_map_mut`（镜像 `as_props_map`）。
2. `fix_host.rs`：
   - 删除 `level_char_map_structs` / `level_char_map_structs_mut`（Array-only），改为统一访问器 `cspm_property` / `cspm_property_mut` + `cspm_elements` / `cspm_retain`，**Map 分支必须走 `world_copy::as_props_map` + `as_struct_value`**（见勘误 2）。
   - 改写 Phase 1（`:516-541`）/ Phase 3（`:554-573`）改用 `cspm_elements` / `cspm_retain`。
   - `element_player_uid` / `element_instance_id`（`:414,437`）签名不变，直接吃 `&StructPropertyValue`（由 `as_struct_value` 给出），Array/Map 通吃。
3. `sav_io.rs:1777` `cspm_player_uid_eq_count`：加 Map 分支 —— `if let Some(map)=world_copy::as_props_map(cmp) { for (_k,v) in map.0.iter() { if let Some(sv)=as_struct_value(v){ ...读 sv 的 key.PlayerUId... } } }`，否则 `assert_eq!(count,1)` 永远 FAIL。
4. 补 `make_synthetic_map_level`（MapProperty 形状 CSPM）+ `phase_b_dedup_keeps_source_only_map` 测试，锁死真机 Map 路径（handoff §5.5 不变）。
5. 构建/测试约束照 handoff §6（`C:\pal_build` + `MSYS_NO_PATHCONV=1` + 先删 `.cargo-build-lock`；只 `cargo test`，禁 `tauri:dev`）。
6. 验收照 handoff §7（`cargo test save_edit` 全绿：4 条断言全过）。

---

## 为什么本勘误必要

handoff §5.2 的"修复"在合成 Array 夹具上能过，但因为对真机 Map 值匹配了错误的 `Property` 变体，**真机去重仍不生效**——这恰好是上一轮被标记为"自报完成、实测未落地"的同一类陷阱（只看合成测试绿，没在真机 Map 上验证）。本项目 `world_copy.rs` 早就有正确的 Map 访问范式，Codex 直接复用即可避开这个坑。
