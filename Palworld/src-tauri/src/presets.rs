use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
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

/// 获取 presets 目录绝对路径
/// 优先使用可执行文件同级目录，回退到 CARGO_MANIFEST_DIR（开发模式）
fn presets_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let dir = parent.join("presets");
            if dir.exists() {
                return dir;
            }
        }
    }
    // 开发模式回退：src-tauri/presets
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets")
}

/// 读取指定预设文件，返回 (Vec<PresetEntry>, 预设总描述)
fn load_preset_file(name: &str) -> Result<(Vec<PresetEntry>, String), String> {
    let path = presets_dir().join(format!("{}.json", name));
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取预设文件失败 [{}]: {}", path.display(), e))?;
    let entries: Vec<PresetEntry> = serde_json::from_str(&content)
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
    // 内置预设清单（名称 → 文件名）
    let builtin = vec![
        ("默认", "default"),
        ("PvE 友好", "pve-friendly"),
        ("PvP 竞技", "pvp-competitive"),
        ("速通", "speedrun"),
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
