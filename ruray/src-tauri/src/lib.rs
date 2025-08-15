// Project: RuRay
// Author: Lander
// CreateAt: 2024-01-01

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime, WindowEvent,
};

mod commands;
mod config;
mod proxy;
mod system;
mod tun;
mod xray;

/// 构建系统托盘菜单
/// 
/// # Returns
/// * `Result<Menu<R>, tauri::Error>` - 托盘菜单对象
fn build_tray_menu<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<Menu<R>, tauri::Error> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", "隐藏窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    Menu::with_items(app, &[&show_item, &hide_item, &quit_item])
}

/// 处理系统托盘图标事件
/// 
/// # Arguments
/// * `app` - 应用句柄
/// * `event` - 托盘事件
fn handle_tray_event<R: Runtime>(app: &tauri::AppHandle<R>, event: TrayIconEvent) {
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            ..
        } => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        _ => {}
    }
}

/// 处理托盘菜单事件
/// 
/// # Arguments
/// * `app` - 应用句柄
/// * `event` - 菜单事件
fn handle_menu_event<R: Runtime>(app: &tauri::AppHandle<R>, event: tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        "quit" => {
            app.exit(0);
        }
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "hide" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        _ => {}
    }
}

/// 应用程序入口点
/// 
/// # Returns
/// * `Result<(), Box<dyn std::error::Error>>` - 运行结果
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            // 服务器管理
            commands::get_servers,
            commands::add_server,
            commands::update_server,
            commands::delete_server,
            commands::test_server_connection,
            commands::regenerate_server_config,
            commands::open_server_config_file,
            // 代理控制
            commands::start_proxy,
            commands::stop_proxy,
            commands::get_proxy_status,
            commands::set_proxy_mode,
            // 系统功能
            commands::get_system_stats,
            commands::set_system_proxy,
            commands::clear_system_proxy,
            commands::get_system_proxy_status,
            // 配置文件管理
            commands::cleanup_unused_configs,
            // Xray Core 管理
            commands::check_xray_update,
            commands::download_xray_update,
            commands::download_xray_update_with_progress,
            commands::get_xray_version,
            commands::check_xray_exists,
            commands::get_xray_path,
            commands::download_geo_files,
            commands::check_geo_files_exist,
            commands::ensure_xray_files,
            commands::test_xray_config,
            // 配置管理
            commands::get_app_config,
            commands::save_app_config,
            commands::import_config,
            commands::export_config,
            // TUN 模式管理
            commands::start_tun_mode,
            commands::stop_tun_mode,
            commands::get_tun_status,
            commands::is_tun_running,
            commands::get_tun_config,
            commands::update_tun_config,
            commands::set_tun_system_route,
            commands::toggle_tun_mode,
        ])
        .setup(|app| {
            // 初始化应用配置
            config::init_app_config()?;

            // 设置TunManager的应用句柄
            tun::TunManager::instance().set_app_handle(app.handle().clone());

            // 创建系统托盘
            let tray_menu = build_tray_menu(app.handle())?;
            let _tray = TrayIconBuilder::with_id("main")
                .menu(&tray_menu)
                .on_tray_icon_event(|tray, event| {
                    handle_tray_event(tray.app_handle(), event);
                })
                .on_menu_event(|app, event| {
                    handle_menu_event(app, event);
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { .. } => {
                     // 在窗口关闭时停止TUN模式
                     if tun::TunManager::instance().is_running_sync() {
                         println!("应用关闭中，正在停止TUN模式...");
                         if let Err(e) = tun::TunManager::instance().stop_sync() {
                             eprintln!("停止TUN模式失败: {}", e);
                         } else {
                             println!("TUN模式已停止");
                         }
                     }
                 }
                _ => {}
            }
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
