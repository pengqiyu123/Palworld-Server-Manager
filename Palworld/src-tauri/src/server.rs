use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::rcon::RconState;
use tauri::{command, Emitter, State};
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows::Win32::System::Console::{
    AttachConsole, COORD, CONSOLE_SCREEN_BUFFER_INFO, FreeConsole, GetConsoleScreenBufferInfo,
    GetConsoleWindow, ReadConsoleOutputCharacterW,
};
use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub managed_by_app: bool,
    pub server_path: String,
    pub log_count: usize,
}

pub struct ServerState {
    pub process: Mutex<Option<std::process::Child>>,
    pub external_pid: Mutex<Option<u32>>,
    pub server_path: Mutex<String>,
    pub logs: Arc<Mutex<Vec<String>>>,
}

const PAL_SERVER_UDP_PORT: u16 = 8211;
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
const CONSOLE_ATTACH_TIMEOUT: Duration = Duration::from_secs(8);
const CONSOLE_POLL_INTERVAL: Duration = Duration::from_millis(250);

fn server_launch_args() -> [&'static str; 4] {
    [
        "Pal",
        "-useperfthreads",
        "-NoAsyncLoadingThread",
        "-UseMultithreadForDS",
    ]
}

fn console_line_delta(previous: &[String], current: &[String]) -> Vec<String> {
    let max_overlap = previous.len().min(current.len());
    for overlap in (0..=max_overlap).rev() {
        if previous[previous.len().saturating_sub(overlap)..] == current[..overlap] {
            return current[overlap..].to_vec();
        }
    }
    current.to_vec()
}

fn add_server_log(logs: &Arc<Mutex<Vec<String>>>, app: &tauri::AppHandle, line: String) {
    let mut logs_guard = logs.lock().unwrap();
    logs_guard.push(line.clone());
    while logs_guard.len() > 500 {
        logs_guard.remove(0);
    }
    drop(logs_guard);
    let _ = app.emit("server-log", line);
}

fn attach_server_console(pid: u32) -> Result<HANDLE, String> {
    unsafe {
        // Tauri 没有用户可见的控制台。先确保本进程未附着到开发终端，再连接服务器控制台。
        let _ = FreeConsole();
        AttachConsole(pid).map_err(|error| format!("连接服务器控制台失败: {}", error))?;

        // 服务器控制台只作为日志来源，不应向用户额外弹出黑窗。
        let window = GetConsoleWindow();
        if !window.0.is_null() {
            let _ = ShowWindow(window, SW_HIDE);
        }

        CreateFileW(
            w!("CONOUT$"),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
        .map_err(|error| format!("打开服务器控制台输出失败: {}", error))
    }
}

fn read_visible_console_lines(console: HANDLE) -> Result<Vec<String>, String> {
    unsafe {
        let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
        GetConsoleScreenBufferInfo(console, &mut info)
            .map_err(|error| format!("读取服务器控制台状态失败: {}", error))?;

        let width = info.dwSize.X.max(0) as usize;
        let top = info.srWindow.Top;
        let bottom = info.srWindow.Bottom;
        if width == 0 || bottom < top {
            return Ok(Vec::new());
        }

        let mut lines = Vec::new();
        for y in top..=bottom {
            let mut buffer = vec![0_u16; width];
            let mut read = 0_u32;
            ReadConsoleOutputCharacterW(
                console,
                &mut buffer,
                COORD { X: 0, Y: y },
                &mut read,
            )
            .map_err(|error| format!("读取服务器控制台日志失败: {}", error))?;
            let line = String::from_utf16_lossy(&buffer[..read as usize])
                .trim()
                .to_string();
            if !line.is_empty() {
                lines.push(line);
            }
        }
        Ok(lines)
    }
}

fn stream_server_console_logs(
    pid: u32,
    server_path: String,
    logs: Arc<Mutex<Vec<String>>>,
    app: tauri::AppHandle,
) {
    std::thread::spawn(move || {
        let deadline = Instant::now() + CONSOLE_ATTACH_TIMEOUT;
        let console = loop {
            match attach_server_console(pid) {
                Ok(console) => break Some(console),
                Err(_) if Instant::now() < deadline && is_process_running(pid) => {
                    std::thread::sleep(CONSOLE_POLL_INTERVAL);
                }
                Err(error) => {
                    add_server_log(&logs, &app, format!("[ERR] {}", error));
                    break None;
                }
            }
        };

        let Some(console) = console else {
            return;
        };
        let _ = app.emit("server-log-source", "console");

        let mut previous = Vec::new();
        while is_process_running(pid) {
            match read_visible_console_lines(console) {
                Ok(current) => {
                    for line in console_line_delta(&previous, &current) {
                        add_server_log(&logs, &app, line);
                    }
                    previous = current;
                }
                Err(error) => {
                    add_server_log(&logs, &app, format!("[ERR] {}", error));
                    break;
                }
            }
            std::thread::sleep(CONSOLE_POLL_INTERVAL);
        }

        unsafe {
            let _ = CloseHandle(console);
            let _ = FreeConsole();
        }
        add_server_log(&logs, &app, "[INFO] 服务器进程已退出".to_string());
        let _ = app.emit("server-status-change", ServerStatus {
            running: false,
            pid: None,
            managed_by_app: false,
            server_path,
            log_count: logs.lock().unwrap().len(),
        });
    });
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WindowsProcess {
    process_id: u32,
    executable_path: Option<String>,
}

// ==================== 进程路径解析（★D4） ====================

/// 解析 PalServer 可执行文件路径。
/// 主路径：{server_path}/Pal/Binaries/Win64/PalServer-Win64-Shipping-Cmd.exe（Cmd 控制台可捕获）
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

fn normalize_windows_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

/// 查找已由用户或其他管理器启动、但路径与当前配置相符的 PalServer。
/// 这样不会把该进程占用的 UDP 端口误判为冲突后再重复启动一次。
fn find_existing_server_pid(server_path: &str) -> Option<u32> {
    let candidates = [
        Path::new(server_path)
            .join("Pal")
            .join("Binaries")
            .join("Win64")
            .join("PalServer-Win64-Shipping-Cmd.exe"),
        Path::new(server_path).join("PalServer.exe"),
    ];
    let expected: Vec<String> = candidates.iter().map(|path| normalize_windows_path(path)).collect();
    let script = "Get-CimInstance Win32_Process -Filter \"Name = 'PalServer-Win64-Shipping-Cmd.exe' OR Name = 'PalServer.exe'\" | Select-Object ProcessId, ExecutablePath | ConvertTo-Json -Compress";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .ok()?;
    let raw = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let processes: Vec<WindowsProcess> = if json.is_array() {
        serde_json::from_value(json).ok()?
    } else {
        vec![serde_json::from_value(json).ok()?]
    };

    processes.into_iter().find_map(|process| {
        let executable = process.executable_path?;
        expected
            .iter()
            .any(|path| *path == normalize_windows_path(Path::new(&executable)))
            .then_some(process.process_id)
    })
}

fn is_process_running(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            !stdout.trim().is_empty() && !stdout.contains("INFO: No tasks")
        })
        .unwrap_or(false)
}

fn has_udp_binding(pid: u32, port: u16) -> bool {
    let script = format!(
        "Get-NetUDPEndpoint -OwningProcess {} -LocalPort {} -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty OwningProcess",
        pid, port
    );
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim() == pid.to_string())
        .unwrap_or(false)
}

/// 仅当新进程真的绑定游戏端口 8211/UDP 后才报告启动成功，避免绑定失败时的假运行态。
fn wait_for_udp_binding(pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if has_udp_binding(pid, PAL_SERVER_UDP_PORT) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

/// 检查服务器进程是否正在运行（供 network.rs 的 Radmin 检测复用，不跨函数持锁）。
pub fn is_server_process_running(state: &ServerState) -> bool {
    let child_running = {
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
    };
    if child_running {
        return true;
    }

    let mut external_pid = state.external_pid.lock().unwrap();
    if let Some(pid) = *external_pid {
        if is_process_running(pid) {
            return true;
        }
        *external_pid = None;
    }
    false
}

fn current_server_pid(state: &ServerState) -> Option<u32> {
    if let Some(pid) = state.process.lock().unwrap().as_ref().map(|process| process.id()) {
        return Some(pid);
    }
    *state.external_pid.lock().unwrap()
}

fn take_server_process(
    state: &ServerState,
) -> Result<(Option<std::process::Child>, Option<u32>), String> {
    let process_option = state.process.lock().unwrap().take();
    let external_pid = state.external_pid.lock().unwrap().take();

    if process_option.is_none() && external_pid.is_none() {
        return Err("服务器未运行".to_string());
    }

    Ok((process_option, external_pid))
}

fn force_terminate_server_process(
    process_option: Option<std::process::Child>,
    external_pid: Option<u32>,
) -> Result<(), String> {
    if let Some(mut child) = process_option {
        let _ = child.kill();
        let _ = child.wait();
    }
    if let Some(pid) = external_pid {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .map_err(|e| format!("停止已运行的服务器失败: {}", e))?;
        if !status.success() && is_process_running(pid) {
            return Err(format!("停止已运行的服务器失败（PID {}）", pid));
        }
    }
    Ok(())
}

fn build_server_status(state: &ServerState) -> ServerStatus {
    let running = is_server_process_running(state);
    let pid = running.then(|| current_server_pid(state)).flatten();
    let managed_by_app = running && state.process.lock().unwrap().is_some();
    let server_path = state.server_path.lock().unwrap().clone();
    let log_count = state.logs.lock().unwrap().len();
    ServerStatus {
        running,
        pid,
        managed_by_app,
        server_path,
        log_count,
    }
}

#[command]
pub async fn init_server_state(
    state: State<'_, ServerState>,
    path: String,
) -> Result<ServerStatus, String> {
    if !path.trim().is_empty() {
        *state.server_path.lock().unwrap() = path.clone();
        if !is_server_process_running(&state) {
            *state.external_pid.lock().unwrap() = find_existing_server_pid(&path);
        }
    }
    Ok(build_server_status(&state))
}

// ==================== 服务器进程管理 ====================

#[command]
pub async fn start_server(
    state: State<'_, ServerState>,
    app_handle: tauri::AppHandle,
    path: String,
) -> Result<ServerStatus, String> {
    if is_server_process_running(&state) {
        return Err("服务器已在运行".to_string());
    }
    if let Some(pid) = find_existing_server_pid(&path) {
        *state.external_pid.lock().unwrap() = Some(pid);
        *state.server_path.lock().unwrap() = path;
        return Ok(build_server_status(&state));
    }

    let mut process = state.process.lock().unwrap();
    if process.is_some() {
        return Err("服务器已在运行".to_string());
    }

    // 优先启动 Cmd 版并读取其 Windows 控制台；
    // 回退老 PalServer.exe（wrapper 模式，日志不可用）。
    let (exe_path, source_tag) = resolve_exe_path(&path)?;

    let args = server_launch_args();

    let mut command = Command::new(&exe_path);
    command.args(&args).current_dir(&path);
    if source_tag == "cmd" {
        command.creation_flags(CREATE_NEW_CONSOLE);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("启动失败: {}", e))?;

    if source_tag == "cmd" {
        stream_server_console_logs(
            child.id(),
            path.clone(),
            state.logs.clone(),
            app_handle.clone(),
        );
    } else {
        let _ = app_handle.emit("server-log-source", source_tag);
        add_server_log(
            &state.logs,
            &app_handle,
            "[WARN] 当前启动器不提供可读取的服务器控制台日志".to_string(),
        );
    }

    if !wait_for_udp_binding(child.id()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err("服务器进程未能在启动期绑定 UDP 端口，请检查实时日志和端口设置".to_string());
    }

    *process = Some(child);
    *state.server_path.lock().unwrap() = path;
    drop(process);

    Ok(build_server_status(&state))
}

#[command]
pub async fn force_stop_server(state: State<'_, ServerState>) -> Result<ServerStatus, String> {
    let (process_option, external_pid) = take_server_process(&state)?;
    force_terminate_server_process(process_option, external_pid)?;
    Ok(build_server_status(&state))
}

#[command]
pub async fn stop_server(
    state: State<'_, ServerState>,
    rcon_state: State<'_, RconState>,
) -> Result<ServerStatus, String> {
    let (process_option, external_pid) = take_server_process(&state)?;

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

    force_terminate_server_process(process_option, external_pid)?;

    Ok(build_server_status(&state))
}

#[command]
pub async fn get_server_status(state: State<'_, ServerState>) -> Result<ServerStatus, String> {
    Ok(build_server_status(&state))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_launch_args_start_console_server_mode() {
        let args = server_launch_args();

        assert_eq!(args.first(), Some(&"Pal"));
    }

    #[test]
    fn console_line_delta_emits_only_new_console_lines() {
        let previous = vec![
            "Game version is v1.0.1.100619".to_string(),
            "REST API started on port 8212".to_string(),
        ];
        let current = vec![
            "REST API started on port 8212".to_string(),
            "Running Palworld dedicated server on :8211".to_string(),
        ];

        assert_eq!(
            console_line_delta(&previous, &current),
            vec!["Running Palworld dedicated server on :8211".to_string()]
        );
    }
}
