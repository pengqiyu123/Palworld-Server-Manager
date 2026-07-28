use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_VISIBLE_LINES: usize = 1_000;
static WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn log_path() -> Result<PathBuf, String> {
    // 日志目录统一由 app_paths 解析：
    // - 便携模式：EXE 同级 /data/logs/app.log
    // - 安装模式：%LocalAppData%/PalworldServerManager/logs/app.log
    let directory = crate::app_paths::current()?.logs_dir().to_path_buf();
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("创建项目日志目录失败: {error}"))?;
    Ok(directory.join("app.log"))
}

fn redact_sensitive(input: &str) -> String {
    let mut output = input.replace(['\r', '\n'], " ");
    for key in ["adminpassword", "rcon_password", "rconpassword", "password"] {
        let mut search_from = 0;
        loop {
            let lower = output.to_ascii_lowercase();
            let Some(relative_start) = lower[search_from..].find(key) else {
                break;
            };
            let key_start = search_from + relative_start;
            let key_end = key_start + key.len();
            let bytes = output.as_bytes();
            let mut value_start = key_end;
            while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
                value_start += 1;
            }
            if value_start >= bytes.len() || !matches!(bytes[value_start], b'=' | b':') {
                search_from = key_end;
                continue;
            }
            value_start += 1;
            while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
                value_start += 1;
            }
            let mut value_end = value_start;
            if value_start < bytes.len() && matches!(bytes[value_start], b'\'' | b'"') {
                let quote = bytes[value_start];
                value_start += 1;
                value_end = value_start;
                while value_end < bytes.len() && bytes[value_end] != quote {
                    value_end += 1;
                }
            } else {
                while value_end < bytes.len()
                    && !bytes[value_end].is_ascii_whitespace()
                    && !matches!(bytes[value_end], b',' | b';' | b'&')
                {
                    value_end += 1;
                }
            }
            output.replace_range(value_start..value_end, "<redacted>");
            search_from = value_start + "<redacted>".len();
        }
    }
    output
}

pub(crate) fn append_to_path(
    path: &Path,
    level: &str,
    operation: &str,
    message: &str,
    context: &[(&str, &str)],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建项目日志目录失败: {error}"))?;
    }
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let context = context
        .iter()
        .map(|(key, value)| redact_sensitive(&format!("{key}={value}")))
        .collect::<Vec<_>>()
        .join(" ");
    let mut line = format!(
        "[{timestamp_ms}] [{}] {} | {}",
        level.to_ascii_uppercase(),
        operation,
        redact_sensitive(message)
    );
    if !context.is_empty() {
        line.push_str(" | ");
        line.push_str(&context);
    }
    line.push('\n');

    let _guard = WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "项目日志写入锁已损坏".to_string())?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(line.as_bytes()))
        .map_err(|error| format!("写入项目日志失败: {error}"))
}

pub(crate) fn record(level: &str, operation: &str, message: &str, context: &[(&str, &str)]) {
    match log_path().and_then(|path| append_to_path(&path, level, operation, message, context)) {
        Ok(()) => {}
        Err(error) => eprintln!("[app-log] {error}"),
    }
}

pub(crate) fn read_from_path(path: &Path) -> Result<Vec<String>, String> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let content =
        std::fs::read_to_string(path).map_err(|error| format!("读取项目日志失败: {error}"))?;
    let lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    Ok(lines
        .into_iter()
        .rev()
        .take(MAX_VISIBLE_LINES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect())
}

pub(crate) fn export_system_logs_to(path: &Path, app_logs: &[String]) -> Result<usize, String> {
    let content = format!(
        "Palworld Server Manager 系统日志\n\n{} 条记录\n{}\n",
        app_logs.len(),
        app_logs.join("\n")
    );
    std::fs::write(path, content).map_err(|error| format!("导出系统日志失败: {error}"))?;
    Ok(app_logs.len())
}

pub(crate) fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record("ERROR", "app.panic", &info.to_string(), &[]);
        previous(info);
    }));
}

#[tauri::command]
pub async fn get_app_logs() -> Result<Vec<String>, String> {
    read_from_path(&log_path()?)
}

#[tauri::command]
pub async fn clear_app_logs() -> Result<(), String> {
    let path = log_path()?;
    std::fs::write(path, "").map_err(|error| format!("清空项目日志失败: {error}"))
}

#[tauri::command]
pub fn write_app_log(level: String, operation: String, message: String) -> Result<(), String> {
    append_to_path(&log_path()?, &level, &operation, &message, &[])
}

#[tauri::command]
pub fn export_system_logs(path: String) -> Result<usize, String> {
    let app_logs = read_from_path(&log_path()?)?;
    export_system_logs_to(Path::new(&path), &app_logs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("palworld-manager-{name}-{nonce}.log"))
    }

    #[test]
    fn app_log_persists_context_and_redacts_credentials() {
        let path = temp_file("redaction");
        append_to_path(
            &path,
            "ERROR",
            "local_save.parse",
            "读取失败 AdminPassword=666666 rcon_password=friend-secret",
            &[("world_path", "F:/Pal/Saved/SaveGames/account/world")],
        )
        .unwrap();

        let logs = read_from_path(&path).unwrap();
        let content = logs.join("\n");
        assert!(content.contains("local_save.parse"));
        assert!(content.contains("world_path=F:/Pal/Saved/SaveGames/account/world"));
        assert!(content.contains("<redacted>"));
        assert!(!content.contains("666666"));
        assert!(!content.contains("friend-secret"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn system_log_export_contains_only_application_logs() {
        let path = temp_file("system-log");
        let app_logs = vec!["[ERROR] local_save.parse failed".to_string()];

        let count = export_system_logs_to(&path, &app_logs).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        assert_eq!(count, 1);
        assert!(content.contains("Palworld Server Manager 系统日志"));
        assert!(!content.contains("诊断报告"));
        assert!(!content.contains("服务器日志"));
        assert!(content.contains("local_save.parse failed"));

        let _ = std::fs::remove_file(path);
    }
}
