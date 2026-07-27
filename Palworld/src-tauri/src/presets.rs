use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::command;

// ==================== 预设管理 ====================

/// 预设元数据（list_presets 返回结构）
#[derive(Serialize, Deserialize, Clone)]
pub struct PresetMeta {
    pub name: String,
    pub description: String,
    /// 前 5 项作为预览
    pub key_params: Vec<(String, String)>,
}

/// 预设文件中的单条参数定义
#[derive(Serialize, Deserialize, Clone)]
struct PresetEntry {
    name: String,
    value: String,
    description: String,
}

/// 预设编译进程序，发布时不依赖可执行文件旁的外部 JSON。
fn load_preset_file(name: &str) -> Result<(Vec<PresetEntry>, String), String> {
    let content = match name {
        "casual" => include_str!("../presets/casual.json"),
        "normal" => include_str!("../presets/normal.json"),
        "challenge" => include_str!("../presets/challenge.json"),
        _ => return Err(format!("未知配置预设: {}", name)),
    };
    let entries: Vec<PresetEntry> = serde_json::from_str(content)
        .map_err(|e| format!("解析预设 JSON 失败 [{}]: {}", name, e))?;
    // 总描述：取前 3 项 description 拼接，避免过长
    let description = entries
        .iter()
        .take(3)
        .map(|e| e.description.clone())
        .collect::<Vec<_>>()
        .join(" / ");
    Ok((entries, description))
}

#[command]
pub fn list_presets() -> Result<Vec<PresetMeta>, String> {
    // 面向新手的三档体验，数值来自 docs/palworld-difficulty-presets-research.md。
    let builtin = vec![
        ("休闲", "casual"),
        ("正常", "normal"),
        ("挑战", "challenge"),
    ];

    let mut result = Vec::new();
    for (display_name, file_name) in builtin {
        match load_preset_file(file_name) {
            Ok((entries, desc)) => {
                let key_params: Vec<(String, String)> = entries
                    .iter()
                    .take(5)
                    .map(|e| (e.name.clone(), e.value.clone()))
                    .collect();
                result.push(PresetMeta {
                    name: file_name.to_string(),
                    description: format!("{} - {}", display_name, desc),
                    key_params,
                });
            }
            Err(e) => return Err(e),
        }
    }
    Ok(result)
}

#[command]
pub fn apply_preset(
    name: String,
    config: HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    let (entries, _desc) = load_preset_file(&name)?;
    // 合并：preset 中的值覆盖 config 中的同名 key
    let mut merged = config;
    for entry in entries {
        merged.insert(entry.name, entry.value);
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beginner_presets_are_available_and_preserve_unrelated_options() {
        let presets = list_presets().expect("内置预设应可加载");
        let names: Vec<&str> = presets.iter().map(|preset| preset.name.as_str()).collect();
        assert_eq!(names, vec!["casual", "normal", "challenge"]);

        let mut existing = HashMap::new();
        existing.insert("ServerName".to_string(), "\"好友世界\"".to_string());
        existing.insert("UnknownFutureOption".to_string(), "KeepMe".to_string());

        let merged = apply_preset("casual".to_string(), existing).expect("休闲预设应可套用");
        assert_eq!(merged.get("Difficulty"), Some(&"None".to_string()));
        assert_eq!(merged.get("ExpRate"), Some(&"1.800000".to_string()));
        assert_eq!(
            merged.get("PalEggDefaultHatchingTime"),
            Some(&"0.000000".to_string())
        );
        assert_eq!(merged.get("BaseCampWorkerMaxNum"), Some(&"50".to_string()));
        assert_eq!(
            merged.get("UnknownFutureOption"),
            Some(&"KeepMe".to_string())
        );

        let normal = apply_preset("normal".to_string(), HashMap::new()).expect("正常预设应可套用");
        assert_eq!(normal.get("Difficulty"), Some(&"None".to_string()));
        assert_eq!(normal.get("ExpRate"), Some(&"1.200000".to_string()));
        assert_eq!(
            normal.get("DeathPenalty"),
            Some(&"ItemAndEquipment".to_string())
        );
        assert_eq!(normal.get("BaseCampWorkerMaxNum"), Some(&"25".to_string()));

        let challenge =
            apply_preset("challenge".to_string(), HashMap::new()).expect("挑战预设应可套用");
        assert_eq!(challenge.get("Difficulty"), Some(&"None".to_string()));
        assert_eq!(
            challenge.get("PalCaptureRate"),
            Some(&"0.700000".to_string())
        );
        assert_eq!(
            challenge.get("PlayerDamageRateDefense"),
            Some(&"1.500000".to_string())
        );
        assert_eq!(
            challenge.get("bEnableFriendlyFire"),
            Some(&"True".to_string())
        );
    }
}
