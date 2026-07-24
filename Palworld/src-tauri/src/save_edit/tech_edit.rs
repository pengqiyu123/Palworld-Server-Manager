//! F5 · T05 科技点编辑 + 玩家基础属性编辑 + 科技列表（P1）。
//!
//! 全部基于 `gvas` 原生字段级改写（Q5，无 Python 运行时）：
//! - 科技点：`UnlockedRecipeTechnologyNames`（顶层 `ArrayProperty::Strings`）增删。
//! - 玩家属性：`SaveData` 内的 `NickName` / `Level`，以及 "Max All" 将若干关键
//!   上限字段拉满（字段缺失则跳过，不影响其余改写）。
//! - `f5_tech_list`：从 vendored `world_data.json` 读取 `technology[]` 列表。

use std::path::PathBuf;

use gvas::properties::array_property::ArrayProperty;
use gvas::properties::Property;
use gvas::types::map::HashableIndexMap;
use gvas::GvasFile;

use crate::save_edit::models::{EditResult, PlayerAttrRequest, TechEditRequest, TechInfo};
use crate::save_edit::path_util;
use crate::save_edit::sav_io::{self, SavFile};

/// "Max All" 要拉满的字段与上限值（缺失则跳过）。
const MAX_STATS: &[(&str, i32)] = &[
    ("MaxHp", 50000),
    ("MaxSP", 50000),
    ("MaxStomach", 1000),
    ("MaxSanity", 1000),
    ("MaxWeight", 100000),
    ("MaxBattery", 100000),
    ("FullStomach", 1000),
    ("SanityValue", 1000),
    ("Health", 50000),
];

/// 取 `SaveData` 自定义字段表（可变）。
fn savedata_fields_mut(gvas: &mut GvasFile) -> Option<&mut HashableIndexMap<String, Vec<Property>>> {
    let sd = sav_io::top_field_mut(gvas, "SaveData")?;
    let csv = sav_io::struct_value_mut(sd)?;
    sav_io::custom_fields_mut(csv)
}

/// 加载玩家 gvas（保留原压缩方式）。
fn load_player_gvas(
    world: &str,
    player_guid: &str,
) -> Result<(GvasFile, SavFile), String> {
    let path = path_util::player_sav_path(world, player_guid)?;
    let sav = SavFile::load(&path)?;
    let gvas = sav.parse()?;
    Ok((gvas, sav))
}

/// 写回玩家 gvas 并计算 round-trip 校验。
fn save_player_gvas(sav: &SavFile, gvas: &GvasFile, path: &std::path::Path) -> Result<EditResult, String> {
    let new_sav = SavFile::from_gvas(gvas, sav.compression)?;
    let rt = new_sav.roundtrip_ok();
    new_sav.save(path)?;
    // 写回后再读一次，确认可解压。
    SavFile::load(path).map_err(|e| format!("写回后校验失败: {}", e))?;
    Ok(EditResult {
        ok: true,
        backup_id: String::new(),
        roundtrip_ok: rt,
        warnings: Vec::new(),
    })
}

/// 编辑科技点（解锁 / 移除，支持单条与批量）。
pub fn edit_tech_impl(req: &TechEditRequest) -> Result<EditResult, String> {
    let path = path_util::player_sav_path(&req.world, &req.player_guid)?;
    let (mut gvas, sav) = load_player_gvas(&req.world, &req.player_guid)?;

    // 取出或创建 UnlockedRecipeTechnologyNames
    let strings = if let Some(prop) = sav_io::top_field_mut(&mut gvas, "UnlockedRecipeTechnologyNames") {
        sav_io::as_strings_mut(prop)
            .ok_or_else(|| "UnlockedRecipeTechnologyNames 不是字符串数组".to_string())?
    } else {
        // 不存在则新建
        let init: Vec<Option<String>> = req
            .add_assets
            .iter()
            .map(|s| Some(s.clone()))
            .collect();
        gvas.properties.insert(
            "UnlockedRecipeTechnologyNames".to_string(),
            Property::ArrayProperty(ArrayProperty::Strings { strings: init }),
        );
        sav_io::top_field_mut(&mut gvas, "UnlockedRecipeTechnologyNames")
            .and_then(sav_io::as_strings_mut)
            .ok_or_else(|| "新建科技数组失败".to_string())?
    };

    let mut changed = Vec::new();

    // 移除
    for rm in &req.remove_assets {
        let before = strings.len();
        strings.retain(|s| s.as_deref() != Some(rm.as_str()));
        if strings.len() != before {
            changed.push(format!("移除 {}", rm));
        }
    }
    // 添加（去重）
    for add in &req.add_assets {
        if !strings.iter().any(|s| s.as_deref() == Some(add.as_str())) {
            strings.push(Some(add.clone()));
            changed.push(format!("解锁 {}", add));
        }
    }

    if changed.is_empty() {
        return Err("没有发生任何科技点改动（可能已存在或参数重复）".to_string());
    }

    let result = save_player_gvas(&sav, &gvas, &path)?;
    Ok(EditResult {
        warnings: changed,
        ..result
    })
}

/// 编辑玩家基础属性（改名 / 设等级 / Max All）。
pub fn edit_player_attr_impl(req: &PlayerAttrRequest) -> Result<EditResult, String> {
    let path = path_util::player_sav_path(&req.world, &req.player_guid)?;
    let (mut gvas, sav) = load_player_gvas(&req.world, &req.player_guid)?;

    let mut fields = savedata_fields_mut(&mut gvas)
        .ok_or_else(|| "未找到 SaveData 字段，存档结构可能异常".to_string())?;

    let mut changed = Vec::new();

    if let Some(name) = &req.rename {
        if sav_io::set_str_in_custom(&mut fields, "NickName", name.clone()) {
            changed.push(format!("改名 → {}", name));
        } else {
            return Err("未找到 NickName 字段，无法改名".to_string());
        }
    }
    if let Some(lv) = req.level {
        let lv = lv.max(1) as i32;
        if sav_io::set_int_in_custom(&mut fields, "Level", lv) {
            changed.push(format!("等级 → {}", lv));
        } else {
            return Err("未找到 Level 字段，无法设等级".to_string());
        }
    }
    if req.max_all {
        for (stat, val) in MAX_STATS {
            sav_io::set_int_in_custom(&mut fields, stat, *val);
        }
        changed.push("关键属性已拉满 (Max All)".to_string());
    }

    if changed.is_empty() {
        return Err("未指定任何属性修改".to_string());
    }

    let result = save_player_gvas(&sav, &gvas, &path)?;
    Ok(EditResult {
        warnings: changed,
        ..result
    })
}

/// 解析 vendored `world_data.json` 中 `technology[]` 的轻量结构。
#[derive(serde::Deserialize)]
struct WorldDataFile {
    technology: Vec<TechEntry>,
}

#[derive(serde::Deserialize)]
struct TechEntry {
    name: String,
    asset: String,
    #[serde(default)]
    tech_type: String,
}

/// 定位 world_data.json（dev / release 多候选路径）。
fn world_data_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let cwd = std::env::current_dir().ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    // release：资源与 exe 同目录的 resources/
    candidates.push(exe_dir.join("resources").join("world_data.json"));
    // dev：exe 在 src-tauri/target/debug 下
    candidates.push(exe_dir.join("..").join("..").join("resources").join("world_data.json"));
    candidates.push(exe_dir.join("..").join("resources").join("world_data.json"));
    // dev：基于当前工作目录（通常为项目根）
    if let Some(cwd) = cwd {
        candidates.push(cwd.join("src-tauri").join("resources").join("world_data.json"));
        candidates.push(cwd.join("resources").join("world_data.json"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// 返回科技列表（来自 vendored world_data.json）。
pub fn f5_tech_list_impl() -> Result<Vec<TechInfo>, String> {
    let path = world_data_path().ok_or_else(|| {
        "未找到 world_data.json（请确认 resources 目录已随程序部署）".to_string()
    })?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取 {} 失败: {}", path.display(), e))?;
    let data: WorldDataFile =
        serde_json::from_str(&text).map_err(|e| format!("解析 world_data.json 失败: {}", e))?;
    let list = data
        .technology
        .into_iter()
        .map(|e| TechInfo {
            name: e.name,
            asset: e.asset,
            tech_type: e.tech_type,
        })
        .collect();
    Ok(list)
}

// ===========================================================================
// F5 单元测试（QA · 严过关）
// 覆盖：f5_tech_list_impl 从 vendored world_data.json 读取 588 条科技，
// 且「科技名 → asset」映射存在（tech_edit 解锁/移除的查表基础）。
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tech_list_mapping_from_world_data() {
        let techs = f5_tech_list_impl().expect("world_data.json 应可被加载并解析");
        assert_eq!(techs.len(), 588, "world_data.json 应有 588 条科技");

        let by_name: std::collections::HashMap<&str, &str> = techs
            .iter()
            .map(|t| (t.name.as_str(), t.asset.as_str()))
            .collect();

        // 已知映射存在（编辑命令靠 asset 名增删）
        assert_eq!(by_name.get("AI Core"), Some(&"AIcore"));
        assert_eq!(by_name.get("Summoning Altar"), Some(&"Altar"));

        // 所有 asset 非空（否则编辑会写入空技术名）
        assert!(
            techs.iter().all(|t| !t.asset.is_empty()),
            "所有科技 asset 不应为空"
        );
    }
}
