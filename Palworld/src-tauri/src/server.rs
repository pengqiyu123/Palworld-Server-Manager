use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use crate::rcon::RconState;
use tauri::{command, Emitter, State};

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub server_path: String,
    pub log_count: usize,
}

pub struct ServerState {
    pub process: Mutex<Option<std::process::Child>>,
    pub server_path: Mutex<String>,
    pub logs: Arc<Mutex<Vec<String>>>,
}

// ==================== 进程路径解析（★D4） ====================

/// 解析 PalServer 可执行文件路径。
/// 主路径：{server_path}/Pal/Binaries/Win64/PalServer-Win64-Shipping-Cmd.exe（Cmd 版，stdout 可捕获）
/// 回退：  {server_path}/PalServer.exe（wrapper 版，日志不可用）
/// 两者都不存在才报错（Q4：Cmd 找不到仅提示横条，不阻断启动 —— 走回退即可）。
fn resolve_exe_path(server_path: &str) -> Result<(PathBuf, &'static str), String> {
    let cmd_exe = Path::new(server_path)
        .join("Pal")
        .join("Binaries")
        .join("Win64")
        .join("PalServer-Win64-Shipping-Cmd.exe");
    if cmd_exe.exists() {
        return Ok((cmd_exe, "cmd"));
    }
    let legacy_exe = Path::new(server_path).join("PalServer.exe");
    if legacy_exe.exists() {
        return Ok((legacy_exe, "wrapper"));
    }
    Err(format!(
        "服务器程序不存在: {} 或 {}",
        cmd_exe.display(),
        legacy_exe.display()
    ))
}

/// 检查服务器进程是否正在运行（供 network.rs 的 Radmin 检测复用，不跨函数持锁）。
pub fn is_server_process_running(state: &ServerState) -> bool {
    let mut process = state.process.lock().unwrap();
    if let Some(ref mut child) = *process {
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) | Err(_) => {
                *process = None;
                false
            }
        }
    } else {
        false
    }
}

// ==================== 服务器进程管理 ====================

#[command]
pub async fn init_server_state(state: State<'_, ServerState>) -> Result<ServerStatus, String> {
    let process = state.process.lock().unwrap();
    let pid = process.as_ref().map(|p| p.id());
    let logs = state.logs.lock().unwrap();
    let server_path = state.server_path.lock().unwrap().clone();
    Ok(ServerStatus {
        running: process.is_some(),
        pid,
        server_path,
        log_count: logs.len(),
    })
}

#[command]
pub async fn start_server(
    state: State<'_, ServerState>,
    app_handle: tauri::AppHandle,
    path: String,
) -> Result<ServerStatus, String> {
    let mut process = state.process.lock().unwrap();
    if process.is_some() {
        return Err("服务器已在运行".to_string());
    }

    // ★D4：优先 spawn Cmd 版（stdout 直接进捕获管道），
    // 回退老 PalServer.exe（wrapper 模式，日志不可用）。
    let (exe_path, source_tag) = resolve_exe_path(&path)?;

    let args = vec![
        "-useperfthreads".to_string(),
        "-NoAsyncLoadingThread".to_string(),
        "-UseMultithreadForDS".to_string(),
    ];

    let mut child = Command::new(&exe_path)
        .args(&args)
        .current_dir(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动失败: {}", e))?;

    // 一次性告知前端日志来源（cmd 模式日志可用 / wrapper 模式日志不可用，仅提示横条不阻断）。
    let _ = app_handle.emit("server-log-source", source_tag);

    // 启动日志收集线程
    if let Some(stdout) = child.stdout.take() {
        let logs = state.logs.clone();
        let app = app_handle.clone();
        let path_for_thread = path.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let mut logs_guard = logs.lock().unwrap();
                    logs_guard.push(line.clone());
                    // 日志上限：循环删除直到 ≤ 500
                    while logs_guard.len() > 500 {
                        logs_guard.remove(0);
                    }
                    drop(logs_guard);
                    let _ = app.emit("server-log", line);
                }
            }
            // 进程退出：推送最终日志并发出状态变更事件
            let mut logs_guard = logs.lock().unwrap();
            logs_guard.push("[INFO] 服务器进程已退出".to_string());
            while logs_guard.len() > 500 {
                logs_guard.remove(0);
            }
            let log_count = logs_guard.len();
            drop(logs_guard);
            let _ = app.emit("server-status-change", ServerStatus {
                running: false,
                pid: None,
                server_path: path_for_thread,
                log_count,
            });
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let logs = state.logs.clone();
        let app = app_handle.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let err_line = format!("[ERR] {}", line);
                    let mut logs_guard = logs.lock().unwrap();
                    logs_guard.push(err_line.clone());
                    while logs_guard.len() > 500 {
                        logs_guard.remove(0);
                    }
                    drop(logs_guard);
                    let _ = app.emit("server-log", err_line);
                }
            }
        });
    }

    *process = Some(child);
    *state.server_path.lock().unwrap() = path.clone();

    let pid = process.as_ref().unwrap().id();
    let log_count = state.logs.lock().unwrap().len();

    Ok(ServerStatus {
        running: true,
        pid: Some(pid),
        server_path: path,
        log_count,
    })
}

#[command]
pub async fn stop_server(
    state: State<'_, ServerState>,
    rcon_state: State<'_, RconState>,
) -> Result<ServerStatus, String> {
    // 先取出进程句柄并立即释放 ServerState 锁，避免长时间持锁阻塞状态查询
    let process_option = {
        let mut process = state.process.lock().unwrap();
        process.take()
    };

    // 尝试优雅关机：如果 RCON 已连接，先发送 Shutdown 命令
    // 仅在发送命令期间持有 RconState 锁，发送完立即释放，避免阻塞其它 RCON 命令
    {
        let mut rcon = rcon_state.client.lock().await;
        if rcon.is_connected() {
            let _ = rcon.send_command("Shutdown").await;
        }
    }

    // 给几秒让服务器保存存档；此刻不持有任何锁，避免阻塞 get_server_status / rcon_send_command
    std::thread::sleep(std::time::Duration::from_secs(3));

    match process_option {
        Some(mut child) => {
            // 如果优雅关机没成功，再强制 kill
            let _ = child.kill();
            let _ = child.wait();
        }
        None => {
            return Err("服务器未运行".to_string());
        }
    }

    let server_path = state.server_path.lock().unwrap().clone();
    let log_count = state.logs.lock().unwrap().len();

    Ok(ServerStatus {
        running: false,
        pid: None,
        server_path,
        log_count,
    })
}

#[command]
pub async fn get_server_status(state: State<'_, ServerState>) -> Result<ServerStatus, String> {
    let mut process = state.process.lock().unwrap();
    let running = if let Some(ref mut child) = *process {
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) | Err(_) => {
                *process = None;
                false
            }
        }
    } else {
        false
    };

    let pid = process.as_ref().map(|p| p.id());
    let server_path = state.server_path.lock().unwrap().clone();
    let log_count = state.logs.lock().unwrap().len();

    Ok(ServerStatus {
        running,
        pid,
        server_path,
        log_count,
    })
}

#[command]
pub async fn get_server_logs(state: State<'_, ServerState>) -> Result<Vec<String>, String> {
    let logs = state.logs.lock().unwrap().clone();
    Ok(logs)
}

#[command]
pub async fn clear_server_logs(state: State<'_, ServerState>) -> Result<(), String> {
    let mut logs = state.logs.lock().unwrap();
    logs.clear();
    Ok(())
}

#[command]
pub fn export_server_logs(path: String, state: State<'_, ServerState>) -> Result<usize, String> {
    // 读取日志缓冲区 → 用 \n 连接 → 写入用户选择的路径
    let logs = state.logs.lock().unwrap();
    let content = logs.join("\n");
    let count = logs.len();
    drop(logs);
    std::fs::write(&path, content)
        .map_err(|e| format!("导出日志失败: {}", e))?;
    Ok(count)
}
