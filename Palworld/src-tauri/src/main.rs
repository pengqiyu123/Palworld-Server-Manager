#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_log;
mod app_paths;
mod backup_service;
mod config;
mod firewall;
mod launcher;
mod network;
mod presets;
mod rest_proxy;
mod route_switch;
mod save_edit;
mod save_transfer;
mod server;
mod settings;
mod steam_detect;
mod windows_process;

use server::ServerState;
use std::sync::{Arc, Mutex};
use tauri::{LogicalSize, WebviewUrl, WebviewWindowBuilder};

/// 弹出中文错误对话框（Windows MessageBoxW），用于启动期 fail-fast 错误。
/// 用户从资源管理器双击 EXE 时无控制台可见，仅 stderr 等于「没反应」，
/// 因此必须用系统模态对话框明确告知失败原因。
#[cfg(target_os = "windows")]
fn show_fatal_error(title: &str, message: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SYSTEMMODAL,
    };
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let message_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    // MB_SYSTEMMODAL 确保弹窗置顶，即便用户正在其他窗口也能看到。
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SYSTEMMODAL,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_fatal_error(title: &str, message: &str) {
    eprintln!("[{title}] {message}");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 便携模式 fail-fast：EXE 同级存在 portable.flag 时，若根目录不可写则直接退出，
    // 不进入 Tauri 主循环、不静默回退 LocalAppData。
    // 必须弹窗告知用户（双击启动无控制台），否则表现就是「没反应」。
    if let Err(error) = app_paths::init() {
        eprintln!("[app-paths] {error}");
        show_fatal_error("Palworld Server Manager 启动失败", &error);
        return;
    }
    // 日志接入：panic hook 先于一切业务逻辑安装，确保运行期任意 panic 都落盘；
    // 随后记录一条 INFO 级启动日志，验证日志链路（便携模式落 EXE/data/logs/app.log）。
    app_log::install_panic_hook();
    app_log::record("INFO", "app.start", "项目启动", &[]);
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Ok(settings) = settings::load_settings() {
                if let Ok(backup_roots) = backup_service::configured_backup_roots(&settings) {
                    for backup_root in backup_roots {
                        if let Err(error) =
                            save_edit::v4_workflow::recover_interrupted_workflows(&backup_root)
                        {
                            eprintln!("[backup-recovery] {error}");
                        }
                    }
                }
            }
            // 第一层：Tauri 标准 API（visible=false + 显式 set_size + show）
            app_log::record("INFO", "window.create", "正在创建主窗口", &[]);
            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Palworld Server Manager")
                    .inner_size(1200.0, 760.0)
                    .min_inner_size(900.0, 720.0)
                    .center()
                    .decorations(false)
                    .transparent(true)
                    .resizable(true)
                    .visible(false)
                    .build()?;
            window
                .set_size(LogicalSize::new(1200.0, 760.0))
                .map_err(|error| {
                    app_log::record("ERROR", "window.set_size", &error.to_string(), &[]);
                    error
                })?;
            window.show().map_err(|error| {
                app_log::record("ERROR", "window.show", &error.to_string(), &[]);
                error
            })?;
            window.set_focus().map_err(|error| {
                app_log::record("ERROR", "window.focus", &error.to_string(), &[]);
                error
            })?;
            app_log::record("INFO", "window.shown", "主窗口已显示", &[]);
            // 路由切换文件轮询（E2E 验收用）
            route_switch::spawn_route_switch_poll(app.handle().clone());
            Ok(())
        })
        .manage(ServerState {
            process: Mutex::new(None),
            external_pid: Mutex::new(None),
            server_path: Mutex::new(String::new()),
            logs: Arc::new(Mutex::new(Vec::new())),
        })
        .invoke_handler({
            tauri::generate_handler![
                server::init_server_state,
                server::start_server,
                server::stop_server,
                server::force_stop_server,
                server::get_server_status,
                server::get_server_logs,
                server::clear_server_logs,
                server::export_server_logs,
                app_log::get_app_logs,
                app_log::clear_app_logs,
                app_log::write_app_log,
                app_log::export_system_logs,
                config::read_config,
                config::write_config,
                config::get_default_config,
                config::get_config_descriptions,
                config::list_config_backups,
                config::restore_config_backup,
                config::fill_default_config,
                config::is_config_initialized,
                firewall::check_firewall_rules,
                firewall::add_firewall_rules,
                network::check_port_usage,
                network::check_radmin_lan_status,
                network::check_radmin_readiness,
                launcher::launch_radmin_vpn,
                launcher::validate_radmin_path,
                launcher::detect_palworld_game,
                launcher::launch_game,
                save_transfer::discover_worlds,
                save_transfer::discover_local_worlds,
                save_transfer::backup_world,
                save_transfer::list_world_backups,
                save_transfer::restore_world,
                save_transfer::restore_world_from,
                save_transfer::export_character,
                save_transfer::import_character,
                save_edit::f5_world_summary,
                save_edit::f5_world_summary_by_path,
                save_edit::discover_modifier_worlds,
                save_edit::get_modifier_world,
                save_edit::preview_modifier_action,
                save_edit::apply_modifier_action,
                save_edit::get_player_technology_points,
                save_edit::fix_host_save,
                save_edit::migrate_world_to_server,
                save_edit::transfer_character,
                save_edit::update_player_technology_points,
                save_edit::migrate_world_v2,
                save_edit::rollback_migration_v2,
                save_edit::v4_migration::migrate_world_v4,
                save_edit::v4_character_operation::transfer_full_character_v4,
                save_edit::v4_character_operation::import_friend_character_v4,
                save_edit::v4_guild_recovery::restore_original_guild_v4,
                save_edit::v4_workflow::complete_migration_workflow_v4,
                save_edit::v4_workflow::rollback_migration_workflow_v4,
                settings::load_app_settings,
                settings::save_app_settings,
                backup_service::backup_create_full,
                backup_service::backup_list_full,
                backup_service::backup_get_root,
                backup_service::backup_delete_full,
                backup_service::backup_list_snapshots,
                backup_service::backup_restore_full,
                backup_service::backup_restore_snapshot,
                backup_service::backup_delete_snapshot,
                backup_service::backup_rebuild_index,
                backup_service::backup_load_workflow,
                backup_service::backup_list_workflows,
                presets::list_presets,
                presets::apply_preset,
                steam_detect::detect_palserver_path,
                rest_proxy::rest_get_info,
                rest_proxy::rest_get_metrics,
                rest_proxy::rest_get_players,
                rest_proxy::rest_kick_player,
                rest_proxy::rest_ban_player,
                rest_proxy::rest_unban_player,
                rest_proxy::rest_announce,
                rest_proxy::rest_save_world,
                rest_proxy::rest_shutdown,
                rest_proxy::rest_management_connect,
                rest_proxy::rest_execute_management_command,
            ]
        })
        .run(tauri::generate_context!());

    if let Err(error) = result {
        app_log::record("ERROR", "app.run", &error.to_string(), &[]);
        show_fatal_error(
            "Palworld Server Manager 启动失败",
            "无法创建或显示应用窗口。请在“故障排查”中导出项目日志，或重新启动应用后再试。",
        );
    }
}

fn main() {
    run();
}

#[cfg(test)]
mod tests {
    #[test]
    fn startup_does_not_discard_window_initialization_errors() {
        let source = include_str!("main.rs");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("main.rs must have production code before its tests");

        assert!(!production_source.contains("let _ = window.set_size"));
        assert!(!production_source.contains("let _ = window.show"));
        assert!(!production_source.contains("let _ = window.set_focus"));
    }
}
