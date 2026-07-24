//! F5 · T03 Fix Host Save（U01 灵魂步骤）—— 双向交换重写。
//!
//! 当单机存档迁移到专用服务器后，原单机主机的角色 UID 需要与原专用服中
//! 新角色的 UID 做**对称交换**，专用服才能正确识别两玩家的公会 / 帕鲁 / 建筑等引用。
//!
//! 本实现严格对齐参考 `PalworldSaveTools::fix_host_save.py::combined_task` 的语义：
//!   1. `Level.sav` 同时含 old / new 两个 GUID，做 **3-pass 双向交换**（`sav_io::swap_guids`，
//!      old→TEMP→new→old，避免单缓冲双 GUID 互相污染）。
//!   2. 两个 `Players/<guid>.sav` 各自**单向交换**（old 文件 old→new；new 文件 new→old）。
//!   3. `_dps.sav`（每个玩家一个）：交换 `OwnerPlayerUId`（UID 字面量），并**显式赋值**
//!      `SlotId.ContainerId.ID = 对方玩家的 PalStorageContainerId`（容器 ID 不是 UID，
//!      不能互换，须按参考 `copy_dps_file` 设值）。gvas 解析失败（R-GVAS-1）时
//!      降级为仅做 `OwnerPlayerUId` 字节交换（R-DPS-1）。
//!   4. **最后交换文件名** `<old>.sav ↔ <new>.sav`（`<old>_dps.sav ↔ <new>_dps.sav` 由
//!      第 3 步直接写入对端文件名完成），保证「文件名 = 身份」一致。
//!   5. 回写经现有 `sav_io::SavFile::save()`（PlM 自动降级 PLZ），并回读校验。
//!
//! 为什么用字节级双向交换而非逐字段：参考靠完整自定义属性 schema 才能逐字段解析公会 /
//! 角色 RawData；我们 `gvas` crate 未必完整覆盖嵌套 RawData（见 R-GVAS-1）。UID 在 GVAS
//! 中均以 16 字节字面量出现，**字节级双向交换 + 文件名交换 + `_dps` 容器 ID 显式赋值**
//! 能产生与参考完全相同的最终状态，且对解析器差异免疫。

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use gvas::properties::Property;
use gvas::properties::struct_property::StructPropertyValue;
use gvas::types::Guid;
use gvas::GvasFile;

use crate::save_edit::models::FixHostRequest;
use crate::save_edit::path_util;
use crate::save_edit::sav_io::{self, SavFile};

/// 为 `_dps` 临时目录生成唯一序号（避免并发测试互相覆盖）。
static DPS_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// 判断原始字节是否包含某 GUID 的 16 字节。
fn contains_guid(raw: &[u8], g: &[u8; 16]) -> bool {
    let mut i = 0;
    let n = raw.len();
    while i + 16 <= n {
        if raw[i..i + 16] == *g {
            return true;
        }
        i += 1;
    }
    false
}

/// 把 16 字节 GUID 格式化为 32 位大写十六进制串（与磁盘文件名一致，
/// 匹配参考 `old_guid.replace('-', '').upper()`）。
fn guid_to_stem(bytes: &[u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in bytes {
        s.push_str(&format!("{:02X}", b));
    }
    s
}

/// 把 `Property`（须为 `type_name == "Guid"` 的 StructProperty）的值设为 `value`。
/// 返回是否命中并改写。
fn set_guid_property(prop: &mut Property, value: &[u8; 16]) -> bool {
    if let Property::StructProperty(s) = prop {
        if s.type_name == "Guid" {
            if let StructPropertyValue::Guid(g) = &mut s.value {
                *g = Guid::from_u8(*value);
                return true;
            }
        }
    }
    false
}

/// 从玩家存档的结构化 GVAS 中提取 `PalStorageContainerId.ID` 与 `IndividualId.InstanceId`
/// （均为 16 字节 GUID）。解析失败或字段缺失时对应返回 `None`。
///
/// - `PalStorageContainerId.ID`：用于 `_dps` 容器 ID 显式赋值（对方玩家的容器）。
/// - `IndividualId.InstanceId`：用于 R-INST-1 防御校验（两角色 InstanceId 不应相同）。
fn extract_player_uids(gvas: &GvasFile) -> (Option<[u8; 16]>, Option<[u8; 16]>) {
    let mut container = None;
    let mut instance = None;
    if let Some(save_data) = sav_io::top_field(gvas, "SaveData") {
        if let Some(csv) = sav_io::struct_value(save_data) {
            if let Some(map) = sav_io::custom_fields(csv) {
                // PalStorageContainerId.ID
                if let Some(psc) = sav_io::field(map, "PalStorageContainerId") {
                    if let Some(psc_csv) = sav_io::struct_value(psc) {
                        if let Some(psc_map) = sav_io::custom_fields(psc_csv) {
                            if let Some(id_prop) = sav_io::field(psc_map, "ID") {
                                if let Some(g) = sav_io::as_guid(id_prop) {
                                    container = Some(g.to_u8());
                                }
                            }
                        }
                    }
                }
                // IndividualId.InstanceId
                if let Some(iid) = sav_io::field(map, "IndividualId") {
                    if let Some(iid_csv) = sav_io::struct_value(iid) {
                        if let Some(iid_map) = sav_io::custom_fields(iid_csv) {
                            if let Some(inst_prop) = sav_io::field(iid_map, "InstanceId") {
                                if let Some(g) = sav_io::as_guid(inst_prop) {
                                    instance = Some(g.to_u8());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (container, instance)
}

/// 在结构化解析后的 `_dps` GVAS 中给每个 Pal 条目：
/// 1. `OwnerPlayerUId`（Guid）→ `target_uid`；
/// 2. `SlotId.ContainerId.ID`（Guid）→ `target_container`（若提供）。
///
/// 返回实际更新的字段数（用于断言 / 调试）。
fn patch_dps(gvas: &mut GvasFile, target_uid: &[u8; 16], target_container: Option<&[u8; 16]>) -> usize {
    let mut updated = 0usize;
    if let Some(prop) = sav_io::top_field_mut(gvas, "SaveParameterArray") {
        if let Some(structs) = sav_io::as_struct_array_mut(prop) {
            for spv in structs.iter_mut() {
                // 每个元素应为 CustomStruct（Pal 条目）
                if let Some(map) = sav_io::custom_fields_mut(spv) {
                    // SaveParameter -> CustomStruct
                    if let Some(save_param) = sav_io::field_mut(map, "SaveParameter") {
                        if let Some(inner) = sav_io::struct_value_mut(save_param) {
                            if let Some(inner_map) = sav_io::custom_fields_mut(inner) {
                                // OwnerPlayerUId (Guid) -> target_uid
                                if let Some(opu) = sav_io::field_mut(inner_map, "OwnerPlayerUId") {
                                    if set_guid_property(opu, target_uid) {
                                        updated += 1;
                                    }
                                }
                                // SlotId -> ContainerId -> ID (Guid) -> target_container
                                if let Some(slot) = sav_io::field_mut(inner_map, "SlotId") {
                                    if let Some(slot_csv) = sav_io::struct_value_mut(slot) {
                                        if let Some(slot_map) = sav_io::custom_fields_mut(slot_csv) {
                                            if let Some(cid) = sav_io::field_mut(slot_map, "ContainerId") {
                                                if let Some(cid_csv) = sav_io::struct_value_mut(cid) {
                                                    if let Some(cid_map) =
                                                        sav_io::custom_fields_mut(cid_csv)
                                                    {
                                                        if let Some(id_prop) =
                                                            sav_io::field_mut(cid_map, "ID")
                                                        {
                                                            if let Some(c) = target_container {
                                                                if set_guid_property(id_prop, c) {
                                                                    updated += 1;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    updated
}

/// 处理单个 `_dps.sav`：交换 `OwnerPlayerUId` 并（结构化解析可行时）显式赋值
/// `SlotId.ContainerId.ID = 对方 PalStorageContainerId`。
///
/// - `src`：原始 `_dps` 文件路径（已拷到临时目录，避免被覆盖）。
/// - `dst`：目标路径（交换后的文件名）。
/// - `source_uid` / `target_uid`：`OwnerPlayerUId` 的源 / 目标 16 字节 GUID。
/// - `target_container`：对方玩家的 `PalStorageContainerId`（None 时降级，不设置）。
///
/// 返回 `true` 表示走了降级路径（gvas 解析失败，或无法设置 ContainerId），
/// 此时仅做 `OwnerPlayerUId` 的字节级交换（R-DPS-1）。
fn patch_dps_file(
    src: &Path,
    dst: &Path,
    source_uid: &[u8; 16],
    target_uid: &[u8; 16],
    target_container: Option<&[u8; 16]>,
) -> Result<bool, String> {
    let file = SavFile::load(src)?;
    match file.parse() {
        Ok(mut gvas) => {
            let _updated = patch_dps(&mut gvas, target_uid, target_container);
            let patched = SavFile::from_gvas(&gvas, file.compression)
                .map_err(|e| format!("{} _dps 重新序列化失败: {}", src.display(), e))?;
            patched.save(dst)?;
            SavFile::load(dst)
                .map_err(|e| format!("{} _dps 写回校验失败: {}", dst.display(), e))?;
            // 若无法得知对方容器 ID，即便结构化成功也标记降级（ContainerId 未动）。
            if target_container.is_none() {
                eprintln!(
                    "[warn] _dps {}: 缺少对方 PalStorageContainerId，未设置 ContainerId.ID（R-DPS-1 降级）。",
                    dst.display()
                );
                return Ok(true);
            }
            Ok(false)
        }
        Err(_) => {
            // R-DPS-1 降级：仅字节级交换 OwnerPlayerUId（source -> target），不动 ContainerId。
            let mut raw_file = file;
            raw_file.replace_guid_bytes(source_uid, target_uid);
            raw_file.save(dst)?;
            SavFile::load(dst)
                .map_err(|e| format!("{} _dps 降级写回校验失败: {}", dst.display(), e))?;
            eprintln!(
                "[warn] _dps {}: gvas 解析失败，降级为仅 OwnerPlayerUId 字节交换（R-DPS-1）。",
                dst.display()
            );
            Ok(true)
        }
    }
}

/// 核心实现：在给定世界数据目录 `data_dir` 内执行双向交换。
///
/// `old_bytes` / `new_bytes` 为目标交换的两个 16 字节 GUID（磁盘序）。
/// 返回被成功改写并写回的文件数（Level + 两个 player + 可能存在的 _dps）。
///
/// ⚠️ T4 可见性调整：`pub(crate)`（仅放开可见性，T3 双向交换逻辑未改动），
/// 供 `save_edit.rs` 的 `run_fix_host_with_guard` 在任意临时 `data_dir` 上复用，
/// 以支持独立于 settings / 运行态的单元测试。
pub(crate) fn fix_host_save_in_dir(
    data_dir: &Path,
    old_bytes: &[u8; 16],
    new_bytes: &[u8; 16],
) -> Result<usize, String> {
    let players_dir = data_dir.join("Players");
    if !players_dir.is_dir() {
        return Err(format!("Players 目录不存在: {}", players_dir.display()));
    }
    if old_bytes == new_bytes {
        return Err("旧主机 GUID 与新角色 GUID 相同，无需替换".to_string());
    }

    let old_stem = guid_to_stem(old_bytes);
    let new_stem = guid_to_stem(new_bytes);
    let old_path = players_dir.join(format!("{}.sav", old_stem));
    let new_path = players_dir.join(format!("{}.sav", new_stem));
    if !old_path.is_file() {
        return Err(format!("旧主机角色存档不存在: {}", old_path.display()));
    }
    if !new_path.is_file() {
        return Err(format!("新角色存档不存在: {}", new_path.display()));
    }
    let level_path = data_dir.join("Level.sav");
    if !level_path.is_file() {
        return Err(format!("Level.sav 不存在: {}", level_path.display()));
    }

    let mut changed = 0usize;
    let mut degraded_dps = false;

    // 提取两玩家的 PalStorageContainerId / InstanceId（用于 _dps 赋值 + R-INST-1 防御）。
    // gvas 解析失败（R-GVAS-1）时降级：容器 ID 取不到 → _dps 仅做 UID 字节交换。
    let old_sav = SavFile::load(&old_path)?;
    let new_sav = SavFile::load(&new_path)?;
    let (old_container, old_inst) = match old_sav.parse() {
        Ok(g) => extract_player_uids(&g),
        Err(_) => (None, None),
    };
    let (new_container, new_inst) = match new_sav.parse() {
        Ok(g) => extract_player_uids(&g),
        Err(_) => (None, None),
    };
    // R-INST-1 防御：两角色 InstanceId 不应相同（否则 3-pass 交换会退化）。
    if let (Some(a), Some(b)) = (old_inst, new_inst) {
        if a == b {
            return Err("两个角色的 InstanceId 相同，无法安全交换（R-INST-1）".to_string());
        }
    }

    // (a) Level.sav：3-pass 双向交换（同时含 old/new 两个 GUID）。
    {
        let mut level = SavFile::load(&level_path)?;
        sav_io::swap_guids(&mut level.raw, old_bytes, new_bytes)
            .map_err(|e| format!("Level.sav 双向交换失败: {}", e))?;
        level.save(&level_path)?;
        SavFile::load(&level_path)
            .map_err(|e| format!("Level.sav 写回校验失败: {}", e))?;
        changed += 1;
    }

    // (b) 两个 player 文件：单向交换（old 文件 old→new；new 文件 new→old）。
    {
        let mut old_file = SavFile::load(&old_path)?;
        old_file.replace_guid_bytes(old_bytes, new_bytes);
        old_file.save(&old_path)?;
        SavFile::load(&old_path)
            .map_err(|e| format!("旧主机存档写回校验失败: {}", e))?;
        changed += 1;
    }
    {
        let mut new_file = SavFile::load(&new_path)?;
        new_file.replace_guid_bytes(new_bytes, old_bytes);
        new_file.save(&new_path)?;
        SavFile::load(&new_path)
            .map_err(|e| format!("新角色存档写回校验失败: {}", e))?;
        changed += 1;
    }

    // (c) _dps.sav：交换 OwnerPlayerUId + 显式设 ContainerId.ID = 对方 PalStorageContainerId。
    //     先把两个原始 _dps 拷到临时目录，避免「写 new_dps 时覆盖未读的 old_dps」。
    let old_dps_path = players_dir.join(format!("{}_dps.sav", old_stem));
    let new_dps_path = players_dir.join(format!("{}_dps.sav", new_stem));
    let tmp_dir = std::env::temp_dir().join(format!(
        "palworld_fixhost_dps_{}_{}",
        std::process::id(),
        DPS_TMP_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let old_dps_tmp = tmp_dir.join(format!("{}_dps.sav", old_stem));
    let new_dps_tmp = tmp_dir.join(format!("{}_dps.sav", new_stem));
    if old_dps_path.is_file() {
        std::fs::copy(&old_dps_path, &old_dps_tmp)
            .map_err(|e| format!("拷贝 {} 失败: {}", old_dps_path.display(), e))?;
    }
    if new_dps_path.is_file() {
        std::fs::copy(&new_dps_path, &new_dps_tmp)
            .map_err(|e| format!("拷贝 {} 失败: {}", new_dps_path.display(), e))?;
    }
    // old_dps 内容 → 写入 new_dps_path（OwnerPlayerUId=old→new，ContainerId=new_container）
    if old_dps_tmp.is_file() {
        let deg =
            patch_dps_file(&old_dps_tmp, &new_dps_path, old_bytes, new_bytes, new_container.as_ref())?;
        if deg {
            degraded_dps = true;
        }
        changed += 1;
    }
    // new_dps 内容 → 写入 old_dps_path（OwnerPlayerUId=new→old，ContainerId=old_container）
    if new_dps_tmp.is_file() {
        let deg =
            patch_dps_file(&new_dps_tmp, &old_dps_path, new_bytes, old_bytes, old_container.as_ref())?;
        if deg {
            degraded_dps = true;
        }
        changed += 1;
    }
    let _ = std::fs::remove_dir_all(&tmp_dir);

    // (d) 最后交换文件名：OLD.sav ↔ NEW.sav（与参考 fix_host_save.py 对齐）。
    //     （_dps 文件名交换已在第 (c) 步直接写入对端文件名完成。）
    let tmp_path = players_dir.join(format!("{}.sav.tmp_swap", old_stem));
    std::fs::rename(&old_path, &tmp_path)
        .map_err(|e| format!("重命名 {} 失败: {}", old_path.display(), e))?;
    if new_path.is_file() {
        std::fs::rename(&new_path, &old_path)
            .map_err(|e| format!("重命名 {} 失败: {}", new_path.display(), e))?;
    }
    std::fs::rename(&tmp_path, &new_path)
        .map_err(|e| format!("重命名 {} 失败: {}", tmp_path.display(), e))?;

    if degraded_dps {
        eprintln!(
            "[warn] fix_host: 至少一个 _dps.sav 走了降级路径（gvas 解析失败或缺少 \
             PalStorageContainerId），仅做了 OwnerPlayerUId 字节交换，未设置 ContainerId.ID；\
             该玩家的帕鲁箱归属可能需手动整理（R-DPS-1）。"
        );
    }

    Ok(changed)
}

/// 执行 Fix Host Save（公开入口，供 `save_edit.rs` 调用）。
///
/// `req.world` 经 `path_util::world_data_dir` 解析为世界数据目录；`old_host_guid` /
/// `new_char_guid` 为两个角色存档文件名（去 `.sav`，可为带连字符的标准 GUID 形式）。
///
/// 返回被成功改写并写回的 `.sav` 文件数。
pub fn fix_host_save_impl(req: &FixHostRequest) -> Result<usize, String> {
    let data_dir = path_util::world_data_dir(&req.world)?;
    let old_bytes = sav_io::guid_bytes(&req.old_host_guid)?;
    let new_bytes = sav_io::guid_bytes(&req.new_char_guid)?;
    fix_host_save_in_dir(&data_dir, &old_bytes, &new_bytes)
}

/// 供 `transfer.rs` 复用：对单个 `.sav` 文件做 GUID 单向替换并写回（带回读校验）。
#[allow(dead_code)]
pub(crate) fn swap_guid_in_file(path: &Path, old: &[u8; 16], new: &[u8; 16]) -> Result<(), String> {
    let mut file = SavFile::load(path)?;
    file.replace_guid_bytes(old, new);
    file.save(path)?;
    SavFile::load(path).map_err(|e| format!("写回后校验失败 {}: {}", path.display(), e))?;
    Ok(())
}

// ===========================================================================
// F5 单元测试（QA · 严过关）
// 覆盖：
//   - contains_guid 在原始字节中检测 / 计数 GUID（fix_host 的命中判定）。
//   - patch_dps 在结构化 GVAS 上正确设置 OwnerPlayerUId 与 SlotId.ContainerId.ID
//     （_dps 解析助手的字段级导航）。
//   - 真实样本双向交换：文件名交换 / Level 3-pass 对拍 / 回读无损坏 / 合成损坏输入拒绝。
// UID 全局替换逻辑由 sav_io 单测覆盖；3-pass 交换由 sav_io::swap_guids 单测覆盖。
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use gvas::engine_version::FEngineVersion;
    use gvas::game_version::DeserializedGameVersion;
    use gvas::properties::array_property::ArrayProperty;
    use gvas::properties::struct_property::{StructProperty, StructPropertyValue};
    use gvas::types::map::HashableIndexMap;
    use gvas::GvasFile;
    use gvas::GvasHeader;

    /// 老板真实样本目录（Windows 绝对路径，Rust std 接受正斜杠）。
    const SAMPLE_DIR: &str = "F:/1/0/20260723-235259/1A91A61548C7B6FD7B58B2B70710F7EE";

    /// 样本目录是否存在；不存在时测试自动跳过。
    fn sample_dir() -> Option<PathBuf> {
        let p = PathBuf::from(SAMPLE_DIR);
        if p.is_dir() {
            Some(p)
        } else {
            eprintln!("[skip] 真实样本目录不存在，跳过样本测试: {}", SAMPLE_DIR);
            None
        }
    }

    /// 把真实样本整包拷到临时随机子目录（避免改动原始样本）。
    fn copy_sample_to_temp() -> Option<PathBuf> {
        let src = sample_dir()?;
        let dst = std::env::temp_dir().join(format!(
            "palworld_fixhost_test_{}_{}",
            std::process::id(),
            DPS_TMP_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dst);
        let mut n = 0usize;
        crate::save_edit::path_util::copy_dir_recursive(&src, &dst, &mut n).ok()?;
        Some(dst)
    }

    /// 单向 16 字节 GUID 替换（用于测试期望值的本地对拍，逻辑同 `SavFile::replace_guid_bytes`）。
    fn replace_guid_bytes_local(raw: &mut Vec<u8>, old: &[u8; 16], new: &[u8; 16]) {
        if old == new {
            return;
        }
        let mut i = 0;
        let n = raw.len();
        while i + 16 <= n {
            if raw[i..i + 16] == *old {
                raw[i..i + 16].copy_from_slice(new);
                i += 16;
            } else {
                i += 1;
            }
        }
    }

    #[test]
    fn contains_guid_detects_and_counts() {
        let old: [u8; 16] = [0xABu8; 16];
        let mut raw = Vec::new();
        raw.extend_from_slice(&[0u8; 3]);
        raw.extend_from_slice(&old); // 出现 1
        raw.extend_from_slice(&[0u8; 2]);
        raw.extend_from_slice(&old); // 出现 2

        assert!(contains_guid(&raw, &old), "应检测到 old GUID");
        let other: [u8; 16] = [0xCDu8; 16];
        assert!(!contains_guid(&raw, &other), "不应误报 other GUID");
    }

    /// `_dps` 解析助手 `patch_dps` 的字段级导航：构造最小嵌套 GVAS，断言
    /// `OwnerPlayerUId` 与 `SlotId.ContainerId.ID` 被正确设置。
    #[test]
    fn patch_dps_sets_owner_and_container() {
        let old_uid: [u8; 16] = [0xAAu8; 16];
        let new_uid: [u8; 16] = [0xBBu8; 16];
        let container: [u8; 16] = [0xCCu8; 16];

        // 构造：SaveParameterArray -> [ { SaveParameter: { OwnerPlayerUId: Guid(old),
        //   SlotId: { ContainerId: { ID: Guid(0) } } } } ]
        let zero = Guid::from_u8([0u8; 16]);

        // SlotId.ContainerId.ID
        let mut cid_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        cid_map.insert(
            "ID".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "Guid".to_string(),
                StructPropertyValue::Guid(Guid::from_u8([0u8; 16])),
            ))],
        );
        let mut slot_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        slot_map.insert(
            "ContainerId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(cid_map),
            ))],
        );

        // SaveParameter 内部
        let mut inner_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        inner_map.insert(
            "OwnerPlayerUId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "Guid".to_string(),
                StructPropertyValue::Guid(Guid::from_u8(old_uid)),
            ))],
        );
        inner_map.insert(
            "SlotId".to_string(),
            vec![Property::StructProperty(StructProperty::new(
                zero,
                "StructProperty".to_string(),
                StructPropertyValue::CustomStruct(slot_map),
            ))],
        );

        let save_param = Property::StructProperty(StructProperty::new(
            zero,
            "StructProperty".to_string(),
            StructPropertyValue::CustomStruct(inner_map),
        ));

        let mut sp_map: HashableIndexMap<String, Vec<Property>> = HashableIndexMap::new();
        sp_map.insert("SaveParameter".to_string(), vec![save_param]);

        let structs = vec![StructPropertyValue::CustomStruct(sp_map)];
        let arr = Property::ArrayProperty(ArrayProperty::Structs {
            field_name: "SaveParameterArray".to_string(),
            type_name: "StructProperty".to_string(),
            guid: zero,
            structs,
        });

        let mut props: HashableIndexMap<String, Property> = HashableIndexMap::new();
        props.insert("SaveParameterArray".to_string(), arr);

        let mut gvas = GvasFile {
            deserialized_game_version: DeserializedGameVersion::Default,
            header: GvasHeader::Version2 {
                package_file_version: 0,
                engine_version: FEngineVersion::new(5, 0, 0, 0, String::new()),
                custom_version_format: 3,
                custom_versions: HashableIndexMap::new(),
                save_game_class_name: "TestSave".to_string(),
            },
            properties: props,
        };

        let updated = patch_dps(&mut gvas, &new_uid, Some(&container));
        assert!(
            updated >= 2,
            "应更新 OwnerPlayerUId 与 ContainerId.ID，实际 {updated}"
        );

        // 回读校验
        let arr_prop = gvas
            .properties
            .get("SaveParameterArray")
            .unwrap();
        if let Property::ArrayProperty(ArrayProperty::Structs { structs, .. }) = arr_prop {
            let spv = &structs[0];
            if let StructPropertyValue::CustomStruct(map) = spv {
                let sp = map.get("SaveParameter").unwrap().first().unwrap();
                if let Property::StructProperty(s) = sp {
                    if let StructPropertyValue::CustomStruct(inner) = &s.value {
                        let opu = inner.get("OwnerPlayerUId").unwrap().first().unwrap();
                        if let Property::StructProperty(os) = opu {
                            if let StructPropertyValue::Guid(g) = &os.value {
                                assert_eq!(g.to_u8(), new_uid, "OwnerPlayerUId 应被设为 new_uid");
                            }
                        }
                        let slot = inner.get("SlotId").unwrap().first().unwrap();
                        if let Property::StructProperty(ss) = slot {
                            if let StructPropertyValue::CustomStruct(slot_map) = &ss.value {
                                let cid = slot_map.get("ContainerId").unwrap().first().unwrap();
                                if let Property::StructProperty(cs) = cid {
                                    if let StructPropertyValue::CustomStruct(cid_map) = &cs.value {
                                        let idp = cid_map.get("ID").unwrap().first().unwrap();
                                        if let Property::StructProperty(ids) = idp {
                                            if let StructPropertyValue::Guid(g) = &ids.value {
                                                assert_eq!(
                                                    g.to_u8(),
                                                    container,
                                                    "ContainerId.ID 应被设为 container"
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// 真实样本双向交换验收：文件名交换 / Level 3-pass 对拍 / 回读无损坏。
    #[test]
    fn real_sample_fix_host_swaps_and_verifies() {
        let Some(work) = copy_sample_to_temp() else {
            return;
        };
        // 选两个真实玩家 GUID（来自 Players/ 文件名，磁盘序 32 位小写十六进制）。
        let old_guid = "3F5D130B000000000000000000000000";
        let new_guid = "4E239D4F000000000000000000000000";
        let players = work.join("Players");
        let old_path = players.join(format!("{}.sav", old_guid.to_uppercase()));
        let new_path = players.join(format!("{}.sav", new_guid.to_uppercase()));
        assert!(old_path.is_file(), "old 玩家存档应存在");
        assert!(new_path.is_file(), "new 玩家存档应存在");

        let old_bytes = sav_io::guid_bytes(old_guid).expect("解析 old GUID");
        let new_bytes = sav_io::guid_bytes(new_guid).expect("解析 new GUID");

        // 记录交换前原始字节（事后对拍，证明 swap 与参考一致）。
        let level_path = work.join("Level.sav");
        let old_pre = SavFile::load(&old_path).expect("读 old").raw.clone();
        let new_pre = SavFile::load(&new_path).expect("读 new").raw.clone();
        let level_pre = SavFile::load(&level_path).expect("读 Level").raw.clone();
        let level_old_count_pre = level_pre.windows(16).filter(|w| *w == &old_bytes).count();
        let level_new_count_pre = level_pre.windows(16).filter(|w| *w == &new_bytes).count();

        // 执行双向交换。
        let changed = fix_host_save_in_dir(&work, &old_bytes, &new_bytes)
            .expect("fix_host_save_in_dir 应成功");
        assert!(changed >= 3, "至少 Level + 两个 player 文件应被改写，实际 {changed}");

        // (1) 文件名交换：OLD.sav 现包含（原 new 内容，new→old）；NEW.sav 现包含（原 old 内容，old→new）。
        let old_post = SavFile::load(&old_path).expect("OLD.sav 应仍在").raw;
        let new_post = SavFile::load(&new_path).expect("NEW.sav 应仍在").raw;
        let mut expect_old = new_pre.clone();
        replace_guid_bytes_local(&mut expect_old, &new_bytes, &old_bytes);
        assert_eq!(old_post, expect_old, "OLD.sav 应等于（原 new 内容，new→old 替换）");
        let mut expect_new = old_pre.clone();
        replace_guid_bytes_local(&mut expect_new, &old_bytes, &new_bytes);
        assert_eq!(new_post, expect_new, "NEW.sav 应等于（原 old 内容，old→new 替换）");

        // (2) Level.sav 3-pass 双向交换：old/new 出现次数互换，且与 swap_guids 结果逐字节一致。
        let level_post = SavFile::load(&level_path).expect("Level.sav 应可重新解压").raw;
        let level_old_count_post = level_post.windows(16).filter(|w| *w == &old_bytes).count();
        let level_new_count_post = level_post.windows(16).filter(|w| *w == &new_bytes).count();
        assert_eq!(
            level_old_count_post, level_new_count_pre,
            "Level 中 old 出现次数应 = 交换前 new 的次数"
        );
        assert_eq!(
            level_new_count_post, level_old_count_pre,
            "Level 中 new 出现次数应 = 交换前 old 的次数"
        );
        let mut expect_level = level_pre.clone();
        sav_io::swap_guids(&mut expect_level, &old_bytes, &new_bytes).expect("swap_guids");
        assert_eq!(
            level_post, expect_level,
            "Level.sav 应与 3-pass swap_guids 结果逐字节一致"
        );

        // (3) 回读校验：所有 .sav（含 LevelMeta）必须可解压，无损坏。
        for name in ["Level.sav", "LevelMeta.sav"] {
            let p = work.join(name);
            if p.is_file() {
                SavFile::load(&p).unwrap_or_else(|e| panic!("{} 写回后无法解压: {}", name, e));
            }
        }
        for entry in std::fs::read_dir(&players).expect("读 Players") {
            let p = entry.expect("entry").path();
            if p.extension().map_or(false, |x| x == "sav") {
                SavFile::load(&p).unwrap_or_else(|e| panic!("{} 写回后无法解压: {}", p.display(), e));
            }
        }

        // (4) 若样本含 _dps.sav，断言其被处理（交换文件名 / 容器 ID 赋值或降级）；本样本无 _dps，跳过。
        let has_dps = old_dps_path(&players, old_guid).is_file()
            || old_dps_path(&players, new_guid).is_file();
        if !has_dps {
            println!("[ok] 样本无 _dps.sav，跳过容器 ID 断言（由 patch_dps 单测覆盖）");
        } else {
            // 存在 _dps 时，交换后文件名也应互换：old_dps 内容落在 new_dps 文件名，反之亦然。
            let new_dps_now = old_dps_path(&players, new_guid);
            let old_dps_now = old_dps_path(&players, old_guid);
            assert!(
                new_dps_now.is_file() && old_dps_now.is_file(),
                "_dps 文件名交换后应两文件均存在"
            );
        }

        // 清理临时样本副本（绝不改动原始样本）。
        let _ = std::fs::remove_dir_all(&work);
    }

    /// 小工具：构造 `<guid>_dps.sav` 路径（与实现中一致）。
    fn old_dps_path(players: &std::path::Path, guid: &str) -> PathBuf {
        players.join(format!("{}_dps.sav", guid.to_uppercase()))
    }

    /// 合成损坏输入必须被拒绝（绝不静默产生损坏存档）。
    #[test]
    fn corrupt_sav_rejected() {
        let tmp = std::env::temp_dir()
            .join(format!("palworld_fixhost_corrupt_{}.sav", std::process::id()));
        // 截断 / 非法：仅 7 字节，远不足 12 字节头。
        std::fs::write(&tmp, b"GVAS\x00\x00").unwrap();
        let r = SavFile::load(&tmp);
        assert!(r.is_err(), "截断 / 非法 .sav 必须被拒绝");
        let _ = std::fs::remove_file(&tmp);
    }
}
