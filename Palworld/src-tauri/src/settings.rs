use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::command;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AppSettings {
    pub server_path: String,
    pub config_path: String,
    pub rcon_host: String,
    pub rcon_port: u16,
    pub rcon_password: String,
}

pub fn get_settings_path() -> Result<PathBuf, String> {
    let app_dir = dirs::data_local_dir()
        .ok_or("无法获取本地数据目录")?
        .join("PalworldServerManager");

    std::fs::create_dir_all(&app_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    Ok(app_dir.join("settings.json"))
}

pub fn load_settings() -> Result<AppSettings, String> {
    let path = get_settings_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取设置失败: {}", e))?;
    let settings: AppSettings =
        serde_json::from_str(&content).map_err(|e| format!("解析设置失败: {}", e))?;
    Ok(settings)
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path()?;
    let content = serde_json::to_string_pretty(settings).map_err(|e| format!("序列化设置失败: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("保存设置失败: {}", e))?;
    Ok(())
}

// ==================== Tauri 命令 ====================

#[command]
pub async fn load_app_settings() -> Result<AppSettings, String> {
    load_settings()
}

#[command]
pub async fn save_app_settings(settings: AppSettings) -> Result<(), String> {
    save_settings(&settings)
}
