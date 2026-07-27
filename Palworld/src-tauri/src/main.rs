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

use server::ServerState;
use std::sync::{Arc, Mutex};
use tauri::{LogicalSize, WebviewUrl, WebviewWindowBuilder};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            let _ = window.set_size(LogicalSize::new(1200.0, 760.0));
            // 窗口配置已全部在上方 WebviewWindowBuilder 声明完成（transparent/decorations/resizable）
            // 已移除 window_fix Win32 MoveWindow hack（不再需要）
            let _ = window.show();
            let _ = window.set_focus();
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
