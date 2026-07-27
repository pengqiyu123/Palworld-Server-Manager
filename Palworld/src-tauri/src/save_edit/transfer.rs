//! F5 · T04 跨服角色转移（P1）。
//!
//! 将源世界选中的玩家角色迁移到目标世界，并为每个角色分配一个**不冲突的新 GUID**
//! （最多尝试 1000 次，满足架构 "GUID/instance-id collision bump (≤1000)" 要求）。
//!
//! 实现要点：
//! 1. 玩家存档：拷贝 `Players/<old>.sav` → `Players/<new>.sav`，内部 GUID 做字节级
//!    替换（old→new），保证角色自身数据自洽。
//! 2. Level.sav 联动（尽力而为）：在源 Level.sav 上先做 old→new 字节替换并解析，
//!    取出该角色的 `CharacterSaveParameterMap` 条目（已含新 GUID），克隆后追加进
//!    目标 Level.sav 的同名表并整体重序列化。若该步因解析失败无法完成，仅记警告，
//!    玩家存档拷贝仍成功。
//! 3. 公会：跨世界合并为 P2（老板决议不在此版本做），本版本不做公会迁移。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gvas::properties::struct_property::StructPropertyValue;
use gvas::properties::Property;
use gvas::types::Guid;
use gvas::GvasFile;

use crate::save_edit::models::{EditResult, TransferRequest};
use crate::save_edit::path_util;
use crate::save_edit::sav_io::{self, SavFile};

/// 简易确定性 PRNG（splitmix64），用于生成不冲突的新 GUID。
static SEED: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn random_guid() -> Guid {
    let base = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        ^ (std::process::id() as u64)
        ^ SEED.fetch_add(0x2545_F491_4F6C_DD1D, Ordering::Relaxed);
    let mut s = base;
    let a = splitmix64(&mut s);
    let b = splitmix64(&mut s);
    Guid::from_u32([a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32])
}

/// GUID → 文件名（32 位小写 hex，对应磁盘 `.sav` 命名，与内部 16 字节一致）。
fn guid_to_filename(g: &Guid) -> String {
    let bytes = g.to_u8();
    let mut s = String::with_capacity(32);
    for byte in bytes.iter() {
        s.push_str(&format!("{:02x}", byte));
    }
    s
}

/// 文件名 → GUID（容忍带/不带连字符的 hex 形式）。
fn filename_to_guid(s: &str) -> Result<Guid, String> {
    let clean: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() != 32 {
        return Err(format!("GUID 文件名非法: {}", s));
    }
    let mut bytes = [0u8; 16];
    for i in 0..16 {
        bytes[i] = u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(Guid::from_u8(bytes))
}

/// 生成不与 `existing` 冲突的新 GUID（最多 1000 次）。
fn gen_unique_guid(existing: &[Guid]) -> Option<Guid> {
    for _ in 0..1000 {
        let g = random_guid();
        if !g.is_zero() && !existing.contains(&g) {
            return Some(g);
        }
    }
    None
}

/// 收集目标世界已有的全部 GUID（玩家文件名 + Level.sav 角色实例 id），用于冲突检测。
fn collect_target_guids(target: &str) -> Vec<Guid> {
    let mut out: Vec<Guid> = Vec::new();
    if let Ok(players_dir) = path_util::players_dir(target) {
        if let Ok(entries) = std::fs::read_dir(&players_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_file() && p.extension().map_or(false, |x| x == "sav") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(g) = filename_to_guid(stem) {
                            out.push(g);
                        }
                    }
                }
            }
        }
    }
    if let Ok(data_dir) = path_util::world_data_dir(target) {
        let level = data_dir.join("Level.sav");
        if level.is_file() {
            if let Ok(sav) = SavFile::load(&level) {
                if let Ok(gvas) = sav.parse() {
                    if let Some(ids) = level_instance_ids(&gvas) {
                        out.extend(ids);
                    }
                }
            }
        }
    }
    out
}

/// 取 Level.sav 中所有角色实例 id（CharacterSaveParameterMap 的 key.InstanceId）。
fn level_instance_ids(gvas: &GvasFile) -> Option<Vec<Guid>> {
    let wsd = sav_io::top_field(gvas, "worldSaveData")?;
    let csv = sav_io::struct_value(wsd)?;
    let fields = sav_io::custom_fields(csv)?;
    let cmp = sav_io::field(fields, "CharacterSaveParameterMap")?;
    let structs = sav_io::as_struct_array(cmp)?;
    let mut ids = Vec::new();
    for el in structs {
        if let Some(g) = element_instance_id(el) {
            ids.push(g);
        }
    }
    Some(ids)
}

/// 取单个 CharacterSaveParameterMap 元素的实例 id。
fn element_instance_id(el: &StructPropertyValue) -> Option<Guid> {
    if let StructPropertyValue::CustomStruct(map) = el {
        if let Some(key_prop) = map.get("key").and_then(|v| v.first()) {
            if let Property::StructProperty(s) = key_prop {
                if let StructPropertyValue::CustomStruct(km) = &s.value {
                    if let Some(inst_prop) = km.get("InstanceId").and_then(|v| v.first()) {
                        if let Property::StructProperty(s2) = inst_prop {
                            if let StructPropertyValue::Guid(g) = &s2.value {
                                return Some(*g);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 跨服角色转移主流程。
pub fn transfer_character_impl(req: &TransferRequest) -> Result<EditResult, String> {
    if req.selected_players.is_empty() {
        return Err("未选择任何要转移的角色".to_string());
    }

    let src_data = path_util::world_data_dir(&req.source_world)?;
    let tgt_data = path_util::world_data_dir(&req.target_world)?;
    let tgt_players = tgt_data.join("Players");
    std::fs::create_dir_all(&tgt_players)
        .map_err(|e| format!("创建目标 Players 目录失败: {}", e))?;

    let mut existing = collect_target_guids(&req.target_world);
    let mut warnings: Vec<String> = Vec::new();
    let mut transferred = 0usize;

    for old_guid in &req.selected_players {
        let old_bytes = match sav_io::guid_bytes(old_guid) {
            Ok(b) => b,
            Err(e) => {
                warnings.push(format!("跳过 {}: {}", old_guid, e));
                continue;
            }
        };
        let new_guid = match gen_unique_guid(&existing) {
            Some(g) => g,
            None => {
                warnings.push(format!("跳过 {}: 无法生成不冲突的新 GUID", old_guid));
                continue;
            }
        };
        let new_bytes = new_guid.to_u8();
        let new_name = guid_to_filename(&new_guid);

        // 1) 拷贝玩家 .sav 并做字节替换
        let src_player = src_data.join("Players").join(format!("{}.sav", old_guid));
        let dst_player = tgt_players.join(format!("{}.sav", new_name));
        if !src_player.is_file() {
            warnings.push(format!("跳过 {}: 源玩家存档不存在", old_guid));
            continue;
        }
        {
            let mut file = SavFile::load(&src_player)?;
            file.replace_guid_bytes(&old_bytes, &new_bytes);
            file.save(&dst_player)?;
            SavFile::load(&dst_player).map_err(|e| format!("写回玩家存档校验失败: {}", e))?;
        }

        // 2) Level.sav 联动（尽力而为）
        let src_level = src_data.join("Level.sav");
        let tgt_level = tgt_data.join("Level.sav");
        if src_level.is_file() && tgt_level.is_file() {
            if let Err(e) = link_character_in_level(&src_level, &tgt_level, &old_bytes, &new_bytes)
            {
                warnings.push(format!(
                    "角色 {} 已拷贝，但 Level.sav 联动未完成: {}",
                    old_guid, e
                ));
            }
        }

        existing.push(new_guid);
        transferred += 1;
        warnings.push(format!("已转移 {} → {}", old_guid, new_name));
    }

    if transferred == 0 {
        return Err(format!("未成功转移任何角色。详情: {}", warnings.join("; ")));
    }

    Ok(EditResult {
        ok: true,
        backup_id: String::new(),
        roundtrip_ok: true,
        warnings,
    })
}

/// 在源 Level.sav 上做 old→new 字节替换后解析，取出角色条目克隆进目标 Level.sav。
fn link_character_in_level(
    src_level: &std::path::Path,
    tgt_level: &std::path::Path,
    old_bytes: &[u8; 16],
    new_bytes: &[u8; 16],
) -> Result<(), String> {
    // 源：替换后解析，取出已含新 GUID 的角色条目
    let mut src_file = SavFile::load(src_level)?;
    src_file.replace_guid_bytes(old_bytes, new_bytes);
    let src_gvas = src_file.parse()?;

    let src_structs = match level_instance_ids(&src_gvas) {
        Some(_ids) => {
            // 找到实例 id == new 的条目
            let new_guid = Guid::from_u8(*new_bytes);
            let cmp = sav_io::top_field(&src_gvas, "worldSaveData")
                .and_then(sav_io::struct_value)
                .and_then(sav_io::custom_fields)
                .and_then(|f| sav_io::field(f, "CharacterSaveParameterMap"))
                .and_then(sav_io::as_struct_array);
            match cmp {
                Some(s) => s,
                None => return Err("源 Level.sav 缺少 CharacterSaveParameterMap".to_string()),
            }
            .iter()
            .find(|el| element_instance_id(el) == Some(new_guid))
            .cloned()
        }
        None => None,
    };

    let cloned = match src_structs {
        Some(c) => c,
        None => return Err("未在源 Level.sav 找到该角色条目".to_string()),
    };

    // 目标：追加克隆条目并整体重序列化
    let tgt_file = SavFile::load(tgt_level)?;
    let mut tgt_gvas = tgt_file.parse()?;
    {
        let structs = get_char_map_structs_mut(&mut tgt_gvas)
            .ok_or_else(|| "目标 Level.sav 缺少 CharacterSaveParameterMap".to_string())?;
        structs.push(cloned);
    }
    let new_tgt = SavFile::from_gvas(&tgt_gvas, tgt_file.compression)?;
    new_tgt.save(tgt_level)?;
    SavFile::load(tgt_level).map_err(|e| format!("写回目标 Level.sav 校验失败: {}", e))?;
    Ok(())
}

/// 取目标 Level.sav 的 CharacterSaveParameterMap 结构体数组（可变）。
fn get_char_map_structs_mut(gvas: &mut GvasFile) -> Option<&mut Vec<StructPropertyValue>> {
    let wsd = sav_io::top_field_mut(gvas, "worldSaveData")?;
    let csv = sav_io::struct_value_mut(wsd)?;
    let fields = sav_io::custom_fields_mut(csv)?;
    let cmp = sav_io::field_mut(fields, "CharacterSaveParameterMap")?;
    sav_io::as_struct_array_mut(cmp)
}
