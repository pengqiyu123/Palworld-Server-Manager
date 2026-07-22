use std::path::Path;
use tauri::{command, State};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use rcon::{Connection, Error as RconError};

use crate::config;

// ==================== RCON 客户端（★D3：换 rcon = "0.6" crate） ====================
//
// 删除 R2 手工 send_packet / receive_packet / connect 协议实现（约 90 行），
// 改用成熟的 rcon crate（Valve Source RCON 协议，enable_minecraft_quirks(false)）。
// RconState 的 std::sync::Mutex 改为 tokio::sync::Mutex（Q1，与 async 命令兼容）。
// 4 个老命令签名保持不变（前端零改动）；新增 rcon_connect_using_config（Q2）。

pub struct RconClient {
    connection: Option<Connection<TcpStream>>,
}

impl RconClient {
    pub fn new() -> Self {
        Self { connection: None }
    }

    pub async fn connect(&mut self, host: &str, port: u16, password: &str) -> Result<(), String> {
        let addr = format!("{}:{}", host, port);
        let conn = Connection::builder()
            .enable_minecraft_quirks(false)
            .connect(addr, password)
            .await
            .map_err(map_rcon_error)?;
        self.connection = Some(conn);
        Ok(())
    }

    pub async fn send_command(&mut self, command: &str) -> Result<String, String> {
        let conn = self
            .connection
            .as_mut()
            .ok_or_else(|| "RCON未连接".to_string())?;
        conn.cmd(command).await.map_err(map_rcon_error)
    }

    pub fn disconnect(&mut self) {
        self.connection = None;
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }
}

/// rcon crate 的 Error → 人话中文（供前端 toast 分类）。
fn map_rcon_error(e: RconError) -> String {
    match e {
        RconError::Auth => "RCON 认证失败：AdminPassword 错误".to_string(),
        RconError::CommandTooLong => "RCON 命令过长".to_string(),
        RconError::Io(io) => format!("RCON 连接失败：{}", io),
    }
}

pub struct RconState {
    pub client: Mutex<RconClient>,
}

impl RconState {
    pub fn new() -> Self {
        Self {
            client: Mutex::new(RconClient::new()),
        }
    }
}

// ==================== Tauri 命令 ====================

/// 老命令（保留兼容期）：前端显式传 host/port/password。
#[command]
pub async fn rcon_connect(
    state: State<'_, RconState>,
    host: String,
    port: u16,
    password: String,
) -> Result<String, String> {
    let mut client = state.client.lock().await;
    client.disconnect();
    client
        .connect(&host, port, &password)
        .await
        .map_err(|e| format!("RCON连接失败: {}", e))?;
    Ok("RCON连接成功".to_string())
}

/// 新命令（Q2）：前端只传 server_path，host 固定 127.0.0.1，
/// 密码(RCONPort/AdminPassword)从 PalWorldSettings.ini 读取（不进前端 JS，符合 R2 决策 1）。
#[command]
pub async fn rcon_connect_using_config(
    state: State<'_, RconState>,
    server_path: String,
) -> Result<String, String> {
    let (password, port) = config::read_rcon_credentials(&server_path)?;
    if password.is_empty() {
        return Err("RCON 连接失败：未读取到 AdminPassword，请先在配置中设置".to_string());
    }
    let mut client = state.client.lock().await;
    client.disconnect();
    client
        .connect("127.0.0.1", port, &password)
        .await
        .map_err(|e| format!("RCON连接失败: {}", e))?;
    Ok(format!("RCON连接成功（使用配置文件，端口 {}）", port))
}

#[command]
pub async fn rcon_send_command(
    state: State<'_, RconState>,
    command: String,
) -> Result<String, String> {
    let mut client = state.client.lock().await;
    if !client.is_connected() {
        return Err("RCON未连接".to_string());
    }
    client
        .send_command(&command)
        .await
        .map_err(|e| format!("发送命令失败: {}", e))
}

#[command]
pub async fn rcon_disconnect(state: State<'_, RconState>) -> Result<(), String> {
    let mut client = state.client.lock().await;
    client.disconnect();
    Ok(())
}

#[command]
pub async fn rcon_is_connected(state: State<'_, RconState>) -> Result<bool, String> {
    let client = state.client.lock().await;
    Ok(client.is_connected())
}

// ==================== 单元测试（QA 收官 · 严过关） ====================
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error as IoError;

    #[test]
    fn map_rcon_error_auth() {
        // 前端 toast 分类依赖此前缀（RCON 认证失败 → AuthFailed）
        assert_eq!(
            map_rcon_error(RconError::Auth),
            "RCON 认证失败：AdminPassword 错误"
        );
    }

    #[test]
    fn map_rcon_error_command_too_long() {
        assert_eq!(map_rcon_error(RconError::CommandTooLong), "RCON 命令过长");
    }

    #[test]
    fn map_rcon_error_io() {
        let e = RconError::Io(IoError::new(
            std::io::ErrorKind::ConnectionRefused,
            "boom",
        ));
        let msg = map_rcon_error(e);
        assert!(msg.starts_with("RCON 连接失败："), "got: {}", msg);
        assert!(msg.contains("boom"));
    }

    #[test]
    fn rcon_client_new_is_disconnected() {
        let mut c = RconClient::new();
        assert!(!c.is_connected());
        c.disconnect(); // 无副作用
        assert!(!c.is_connected());
    }
}
