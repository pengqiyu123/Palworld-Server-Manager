//! 路由切换文件轮询（E2E 验收用）
//!
//! 外部 PowerShell 脚本写入 `src-tauri/.route-switch` 文件，
//! 本模块轮询该文件，读取后删除，并通过 `WebviewWindow::eval()` 调用
//! 前端 `window.__router.push(path)` 实现路由切换。
//!
//! Note: cargo run 的工作目录是 `src-tauri/`，所以 trigger file 放在这里。
//! Only enabled in debug/development mode (debug_assertions).

use tauri::AppHandle;
#[cfg(debug_assertions)]
use tauri::Manager;

/// 启动路由切换文件轮询线程 (only in debug mode)
pub fn spawn_route_switch_poll(_app_handle: AppHandle) {
    // 仅在调试/开发模式启用路由轮询（减小生产环境安全面）
    #[cfg(debug_assertions)]
    {
        std::thread::spawn(move || {
            let switch_file = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(".route-switch");

            loop {
                std::thread::sleep(std::time::Duration::from_millis(400));

                if let Ok(path) = std::fs::read_to_string(&switch_file) {
                    let _ = std::fs::remove_file(&switch_file);
                    let path = path.trim().to_string();
                    if path.is_empty() {
                        continue;
                    }
                    if let Some(window) = _app_handle.get_webview_window("main") {
                        let js = format!("window.__router && window.__router.push({:?})", path);
                        let _ = window.eval(&js);
                    }
                }
            }
        });
    }
}
