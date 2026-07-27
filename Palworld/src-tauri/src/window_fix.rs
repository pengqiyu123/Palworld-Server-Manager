//! 窗口尺寸强制修复模块（三重降级方案第二层）
//!
//! 真实根因：Tauri 2.11.5 的 `inner_size` / `set_size` 在部分 Windows 环境下不生效，
//! 窗口实际尺寸仍为 14×14 像素。本模块通过 Win32 `MoveWindow` API 强制修正。
//! 参考方案：Palworld-out/check-window.ps1（已验证有效）。

use std::time::Duration;
use tauri::Manager;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, GetWindowRect, MoveWindow, SetForegroundWindow, ShowWindow, SM_CXSCREEN,
    SM_CYSCREEN, SW_RESTORE,
};

/// 目标窗口尺寸
const TARGET_WIDTH: i32 = 1200;
const TARGET_HEIGHT: i32 = 800;
/// 允许的尺寸误差（±50 像素）
const SIZE_TOLERANCE: i32 = 50;

/// 强制将主窗口调整为 1200×800 居中显示
///
/// 调用契约：
/// - 在 `main.rs` setup hook 中通过 `std::thread::spawn` 延迟 800ms 后调用
/// - 失败时返回 Err，但不阻塞应用启动（main.rs 中 `let _ = ...` 容错）
/// - 第三层（PowerShell verify-window.ps1）会接管本模块失败的场景
pub fn force_resize_via_win32(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 通过 Tauri API 获取主窗口
    let window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;

    // 2. 获取 HWND（Tauri 2 在 Windows 平台返回 windows::Win32::Foundation::HWND）
    let hwnd: HWND = window.hwnd()?;
    let hwnd_addr = hwnd.0 as usize;

    // 3. 读取当前尺寸
    let mut rect = windows::Win32::Foundation::RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect)?;
    }
    let before_w = rect.right - rect.left;
    let before_h = rect.bottom - rect.top;

    eprintln!(
        "[window-fix] HWND=0x{:x}, before={}x{}",
        hwnd_addr, before_w, before_h
    );

    // 4. 若尺寸已在范围内，跳过修复
    if is_size_ok(before_w, before_h) {
        eprintln!("[window-fix] size already correct, skipping");
        return Ok(());
    }

    // 5. 计算居中坐标（优先使用显示器工作区，回退到屏幕尺寸）
    let (x, y) = compute_center_position(hwnd);

    eprintln!(
        "[window-fix] applying MoveWindow to ({},{}) {}x{}",
        x, y, TARGET_WIDTH, TARGET_HEIGHT
    );

    // 6. 调用 MoveWindow 强制设置
    unsafe {
        // 先恢复（若最小化）
        let _ = ShowWindow(hwnd, SW_RESTORE);

        // 强制设置位置和尺寸
        let ok = MoveWindow(hwnd, x, y, TARGET_WIDTH, TARGET_HEIGHT, true);
        if ok.is_err() {
            return Err(format!("MoveWindow failed: {:?}", ok.unwrap_err()).into());
        }

        // 提到前台
        let _ = SetForegroundWindow(hwnd);
    }

    // 7. 等待 500ms 让窗口重绘稳定
    std::thread::sleep(Duration::from_millis(500));

    // 8. 再次验证
    let mut after_rect = windows::Win32::Foundation::RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut after_rect)?;
    }
    let after_w = after_rect.right - after_rect.left;
    let after_h = after_rect.bottom - after_rect.top;

    eprintln!(
        "[window-fix] after={}x{}, expected={}x{}",
        after_w, after_h, TARGET_WIDTH, TARGET_HEIGHT
    );

    if is_size_ok(after_w, after_h) {
        eprintln!("[window-fix] Layer 2 SUCCESS");
        Ok(())
    } else {
        Err(format!(
            "Layer 2 FAILED: after={}x{}, expected={}x{}",
            after_w, after_h, TARGET_WIDTH, TARGET_HEIGHT
        )
        .into())
    }
}

/// 判断尺寸是否在允许范围内
fn is_size_ok(w: i32, h: i32) -> bool {
    (w - TARGET_WIDTH).abs() <= SIZE_TOLERANCE && (h - TARGET_HEIGHT).abs() <= SIZE_TOLERANCE
}

/// 计算窗口居中坐标
///
/// 优先使用窗口当前所在显示器的工作区（排除任务栏），
/// 若获取失败则回退到主屏的 SM_CXSCREEN / SM_CYSCREEN。
fn compute_center_position(hwnd: HWND) -> (i32, i32) {
    // 尝试获取窗口所在显示器的工作区
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let work = info.rcWork;
            let w = work.right - work.left;
            let h = work.bottom - work.top;
            let x = work.left + (w - TARGET_WIDTH) / 2;
            let y = work.top + (h - TARGET_HEIGHT) / 2;
            return (x.max(0), y.max(0));
        }
    }

    // 回退：使用主屏尺寸
    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - TARGET_WIDTH) / 2;
        let y = (screen_h - TARGET_HEIGHT) / 2;
        (x.max(0), y.max(0))
    }
}
