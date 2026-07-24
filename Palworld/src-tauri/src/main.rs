mod config;
mod firewall;
mod launcher;
mod network;
mod save_transfer;
mod save_edit;
mod presets;
mod rcon;
mod rest_proxy;
mod route_switch;
mod server;
mod settings;
mod steam_detect;
mod window_fix;

use rcon::RconState;
use server::ServerState;
use std::sync::{Arc, Mutex};
use tauri::{LogicalSize, WebviewUrl, WebviewWindowBuilder};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // 第一层：Tauri 标准 API（visible=false + 显式 set_size + show）
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
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
        .manage(RconState::new())
        .invoke_handler({
            // check_radmin_lan_status 已标记 #[deprecated]（R3 起删除），此处注册仍需引用，故允许弃用警告。
            #[allow(deprecated)]
            tauri::generate_handler![
                server::init_server_state, server::start_server, server::stop_server,
                server::force_stop_server,
                server::get_server_status, server::get_server_logs, server::clear_server_logs,
                server::export_server_logs,
                config::read_config, config::write_config, config::get_default_config,
                config::get_config_descriptions, config::list_config_backups,
                config::restore_config_backup,
                config::fill_default_config, config::is_config_initialized,
                firewall::check_firewall_rules, firewall::add_firewall_rules,
                network::check_port_usage, network::check_radmin_lan_status,
                network::check_radmin_readiness,
                launcher::launch_radmin_vpn, launcher::launch_game,
                save_transfer::discover_worlds, save_transfer::discover_local_worlds,
                save_transfer::backup_world,
                save_transfer::list_world_backups, save_transfer::restore_world, save_transfer::restore_world_from,
                save_transfer::export_character, save_transfer::import_character,
                save_edit::f5_world_summary, save_edit::f5_world_summary_by_path, save_edit::f5_tech_list,
                save_edit::fix_host_save, save_edit::migrate_world_to_server,
                save_edit::transfer_character, save_edit::edit_tech,
                save_edit::edit_player_attr,
                rcon::rcon_connect, rcon::rcon_send_command, rcon::rcon_disconnect,
                rcon::rcon_is_connected, rcon::rcon_connect_using_config,
                settings::load_app_settings, settings::save_app_settings,
                presets::list_presets, presets::apply_preset,
                steam_detect::detect_palserver_path,
                rest_proxy::rest_get_info, rest_proxy::rest_get_metrics,
                rest_proxy::rest_get_players, rest_proxy::rest_kick_player,
                rest_proxy::rest_ban_player, rest_proxy::rest_unban_player,
                rest_proxy::rest_announce, rest_proxy::rest_shutdown,
            ]
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
