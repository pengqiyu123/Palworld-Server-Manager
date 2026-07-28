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
    /// 用户手动选择并经后端验证的 Radmin VPN 可执行文件。
    #[serde(default)]
    pub radmin_path: String,
    /// 用户手动添加的本地存档扫描根目录（跨页面、跨重启复用）。
    #[serde(default)]
    pub local_save_roots: Vec<String>,
    /// 用户手动添加的服务器存档扫描根目录。
    #[serde(default)]
    pub server_save_roots: Vec<String>,
    /// 世界备份的自定义存放根目录（空值表示使用程序默认 `backups` 目录）。
    #[serde(default)]
    pub backup_root: String,
    /// 过去使用过的备份根目录，仅用于继续展示和恢复旧备份。
    #[serde(default)]
    pub backup_roots: Vec<String>,
    /// 用户是否已确认过迁移前自动备份说明。
    #[serde(default)]
    pub migration_backup_notice_seen: bool,
}

pub fn get_settings_path() -> Result<PathBuf, String> {
    // 安装模式：data_dir = LocalAppData/PalworldServerManager（与 HEAD 一致，无行为变化）。
    // 便携模式：data_dir = EXE 同级 /data。
    let app_dir = crate::app_paths::current()?.data_dir().to_path_buf();
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
    let previous = load_settings().unwrap_or_default();
    let mut settings = settings.clone();
    preserve_backup_root_history(&previous, &mut settings)?;
    let content =
        serde_json::to_string_pretty(&settings).map_err(|e| format!("序列化设置失败: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("保存设置失败: {}", e))?;
    Ok(())
}

fn preserve_backup_root_history(
    previous: &AppSettings,
    next: &mut AppSettings,
) -> Result<(), String> {
    let previous_root = crate::backup_service::resolve_backup_root(previous)?;
    let next_root = crate::backup_service::resolve_backup_root(next)?;
    let mut roots = previous.backup_roots.clone();
    roots.extend(next.backup_roots.clone());
    if !paths_equal(&previous_root, &next_root) {
        roots.push(previous_root.to_string_lossy().into_owned());
    }
    roots.retain(|root| {
        !root.trim().is_empty() && !paths_equal(PathBuf::from(root).as_path(), &next_root)
    });
    roots.sort_by_key(|root| root.to_ascii_lowercase());
    roots.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    next.backup_roots = roots;
    Ok(())
}

fn paths_equal(left: &std::path::Path, right: &std::path::Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_backup_root_preserves_the_previous_root_for_history() {
        let previous = AppSettings {
            backup_root: "F:/old-backups".to_string(),
            backup_roots: vec!["E:/older-backups".to_string()],
            ..AppSettings::default()
        };
        let mut next = AppSettings {
            backup_root: "D:/new-backups".to_string(),
            ..AppSettings::default()
        };

        preserve_backup_root_history(&previous, &mut next).unwrap();

        assert!(next
            .backup_roots
            .iter()
            .any(|root| root == "F:/old-backups"));
        assert!(next
            .backup_roots
            .iter()
            .any(|root| root == "E:/older-backups"));
        assert!(!next
            .backup_roots
            .iter()
            .any(|root| root == "D:/new-backups"));
    }
}
