//! REST API 代理模块。
//!
//! 所有帕鲁 REST API（端口 8212）调用经 Rust 代理，AdminPassword 在 Rust 侧
//! 从 PalWorldSettings.ini 读取、不传前端。前端只传 `server_path`。
//!
//! 认证：HTTP Basic Auth，username=`admin`，password=AdminPassword。
//! Base URL：`http://127.0.0.1:{RESTAPIPort}`（默认 8212，从配置读取）。

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::command;

use crate::config;

// ==================== 全局复用 HTTP Client（Q2 遗留清理） ====================
// 避免每次 REST 调用都重建连接池 / 重复 TCP 握手。30s 超时 + 默认连接池。
use once_cell::sync::Lazy;

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("构建全局 HTTP 客户端失败")
});

// ==================== REST 响应结构体 ====================

/// GET /v1/api/info 响应：服名/版本/世界GUID
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInfo {
    pub version: String,
    pub servername: String,
    pub description: String,
    pub worldguid: String,
}

/// GET /v1/api/metrics 响应：FPS/人数/天数/运行时长
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerMetrics {
    pub currentplayernum: u32,
    pub serverfps: f64,
    pub serverfpsaverage: f64,
    pub serverframetime: f64,
    pub days: u32,
    pub maxplayernum: u32,
    pub basecampnum: u32,
    pub uptime: u64,
}

/// GET /v1/api/players 响应中的单个玩家。
/// 注意：REST API 返回的字段名有大小写混合（iP / userId / playerId / location_x），
/// serde 直接按原字段名反序列化，透传给前端。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerInfo {
    pub name: String,
    #[serde(rename = "playerId")]
    pub player_id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "iP")]
    pub ip: String,
    pub ping: f64,
    pub location_x: f64,
    pub location_y: f64,
    pub level: u32,
}

/// /v1/api/players 的外层 JSON 包装：{"players": [...]}
#[derive(Deserialize)]
struct PlayersResponse {
    players: Vec<PlayerInfo>,
}

// ==================== 内部辅助函数 ====================

/// 从 server_path 拼接配置文件路径并读取 AdminPassword + RESTAPIPort。
///
/// 路径：`{server_path}/Pal/Saved/Config/WindowsServer/PalWorldSettings.ini`
///
/// 返回 `(admin_password, rest_port)`，密码为空字符串时仍返回（调用方会收到 401 错误）。
fn read_rest_config(server_path: &str) -> Result<(String, u16), String> {
    let config_path = Path::new(server_path)
        .join("Pal")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
        .join("PalWorldSettings.ini");

    let config_map = config::read_config_from_file(
        config_path.to_str().ok_or("服务器路径包含非法字符")?,
    )?;

    let admin_password = config::extract_admin_password(&config_map);
    let rest_port = config::extract_rest_port(&config_map);

    Ok((admin_password, rest_port))
}

/// 统一处理 reqwest 错误 → 人话中文错误信息。
fn map_reqwest_error(err: reqwest::Error) -> String {
    if err.is_connect() {
        "REST API 不可达：请确认服务器已启动且 RESTAPIEnabled=True".to_string()
    } else if err.is_timeout() {
        "REST 请求超时：服务器响应过慢或网络异常".to_string()
    } else {
        format!("REST 请求失败: {}", err)
    }
}

/// 统一处理 HTTP 响应状态码 → 人话中文错误信息。
async fn check_http_status(resp: reqwest::Response) -> Result<reqwest::Response, String> {
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        Err("REST 认证失败：请检查 AdminPassword 配置".to_string())
    } else if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(format!("REST 请求失败: {} {}", status, body))
    } else {
        Ok(resp)
    }
}

// ==================== GET 类命令（读取状态） ====================

/// GET /v1/api/info — 服名/版本/世界GUID
#[command]
pub async fn rest_get_info(server_path: String) -> Result<ServerInfo, String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/info", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .get(&url)
        .basic_auth("admin", Some(&password))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    let resp = check_http_status(resp).await?;
    let info = resp.json::<ServerInfo>().await.map_err(|e| {
        format!("解析服务器信息失败: {}", e)
    })?;

    Ok(info)
}

/// GET /v1/api/metrics — FPS/人数/天数/运行时长
#[command]
pub async fn rest_get_metrics(server_path: String) -> Result<ServerMetrics, String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/metrics", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .get(&url)
        .basic_auth("admin", Some(&password))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    let resp = check_http_status(resp).await?;
    let metrics = resp.json::<ServerMetrics>().await.map_err(|e| {
        format!("解析服务器指标失败: {}", e)
    })?;

    Ok(metrics)
}

/// GET /v1/api/players — 在线玩家列表
#[command]
pub async fn rest_get_players(server_path: String) -> Result<Vec<PlayerInfo>, String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/players", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .get(&url)
        .basic_auth("admin", Some(&password))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    let resp = check_http_status(resp).await?;
    let players_resp = resp.json::<PlayersResponse>().await.map_err(|e| {
        format!("解析玩家列表失败: {}", e)
    })?;

    Ok(players_resp.players)
}

// ==================== POST 类命令（管理动作） ====================

/// POST /v1/api/kick — 踢人（body: {"userid": "steam_xxx"}）
#[command]
pub async fn rest_kick_player(server_path: String, userid: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/kick", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "userid": userid }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/ban — 封人（body: {"userid": "steam_xxx"}）
#[command]
pub async fn rest_ban_player(server_path: String, userid: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/ban", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "userid": userid }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/unban — 解封（body: {"userid": "steam_xxx"}）
#[command]
pub async fn rest_unban_player(server_path: String, userid: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/unban", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "userid": userid }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/announce — 全服广播（body: {"message": "..."}）
#[command]
pub async fn rest_announce(server_path: String, message: String) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/announce", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "message": message }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}

/// POST /v1/api/shutdown — 优雅关服（body: {"waittime": 30, "message": "..."}）
///
/// REST /shutdown 是异步的：服务器在 waittime 秒后自行退出，
/// 前端不等返回就显示「正在关闭」状态，进程退出由 server-status-change 事件检测。
#[command]
pub async fn rest_shutdown(
    server_path: String,
    waittime: u32,
    message: String,
) -> Result<(), String> {
    let (password, port) = read_rest_config(&server_path)?;
    let url = format!("http://127.0.0.1:{}/v1/api/shutdown", port);

    let client = &*HTTP_CLIENT;

    let resp = client
        .post(&url)
        .basic_auth("admin", Some(&password))
        .json(&serde_json::json!({ "waittime": waittime, "message": message }))
        .send()
        .await
        .map_err(map_reqwest_error)?;

    check_http_status(resp).await?;
    Ok(())
}
