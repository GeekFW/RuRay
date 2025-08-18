// Project: RuRay
// Author: Lander
// CreateAt: 2024-01-01

use tauri::{
    menu::{Menu, MenuItem, Submenu},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime, WindowEvent,
};

mod commands;
mod config;
mod logger;
mod proxy;
mod system;
mod tun;
mod xray;

/// 构建系统托盘菜单
/// 
/// # Arguments
/// * `app` - 应用句柄
/// 
/// # Returns
/// * `Result<Menu<R>, tauri::Error>` - 托盘菜单对象
async fn build_tray_menu<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<Menu<R>, tauri::Error> {
    // 获取当前代理状态
    let proxy_status = match commands::get_proxy_status().await {
        Ok(status) => status,
        Err(_) => commands::ProxyStatus {
            is_running: false,
            status: "disconnected".to_string(),
            current_server: None,
            proxy_mode: "pac".to_string(),
            uptime: 0,
            upload_speed: 0,
            download_speed: 0,
            total_upload: 0,
            total_download: 0,
        }
    };

    // 获取服务器列表
    let servers = match commands::get_servers().await {
        Ok(servers) => servers,
        Err(_) => vec![]
    };

    // 创建代理管理子菜单
    let proxy_submenu = if proxy_status.is_running {
        // 如果代理正在运行，只显示关闭代理选项
        let stop_proxy_item = MenuItem::with_id(app, "stop_proxy", "关闭代理", true, None::<&str>)?;
        Submenu::with_id_and_items(app, "proxy_menu", "代理管理", true, &[&stop_proxy_item])?
    } else {
        // 如果代理未运行，显示服务器列表供选择
        if servers.is_empty() {
            let no_servers_item = MenuItem::with_id(app, "no_servers", "无可用服务器", false, None::<&str>)?;
            Submenu::with_id_and_items(app, "proxy_menu", "开启代理", true, &[&no_servers_item])?
        } else {
            let mut server_items = Vec::new();
            for server in &servers {
                let server_item = MenuItem::with_id(
                    app, 
                    &format!("start_server_{}", server.id), 
                    &format!("{} ({}:{})", server.name, server.address, server.port), 
                    true, 
                    None::<&str>
                )?;
                server_items.push(server_item);
            }
            
            // 将MenuItem转换为&dyn IsMenuItem<R>
            let server_item_refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = server_items.iter()
                .map(|item| item as &dyn tauri::menu::IsMenuItem<R>)
                .collect();
            
            Submenu::with_id_and_items(app, "proxy_menu", "开启代理", true, &server_item_refs)?
        }
    };
    
    let config_item = MenuItem::with_id(app, "open_config", "查看配置", true, None::<&str>)?;
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", "隐藏窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    Menu::with_items(app, &[&proxy_submenu, &config_item, &show_item, &hide_item, &quit_item])
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
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match event.id.as_ref() {
            "quit" => {
                app_handle.exit(0);
            }
            "show" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "hide" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            "stop_proxy" => {
                // 停止代理
                if let Err(e) = handle_stop_proxy(&app_handle).await {
                    log_error!("停止代理失败: {}", e);
                }
            }
            "open_config" => {
                // 打开配置文件目录
                if let Err(e) = open_config_directory().await {
                    log_error!("打开配置目录失败: {}", e);
                }
            }
            id if id.starts_with("start_server_") => {
                // 处理启动特定服务器
                let server_id = id.strip_prefix("start_server_").unwrap_or("");
                if let Err(e) = handle_start_server(&app_handle, server_id).await {
                    log_error!("启动服务器 {} 失败: {}", server_id, e);
                }
            }
            _ => {}
        }
    });
}

/// 处理停止代理
/// 
/// # Arguments
/// * `app` - 应用句柄
/// 
/// # Returns
/// * `Result<(), String>` - 操作结果
async fn handle_stop_proxy<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    // 获取当前代理状态
    let proxy_status = commands::get_proxy_status().await.map_err(|e| e.to_string())?;
    
    if proxy_status.is_running {
        // 当前代理正在运行，停止代理
        commands::stop_proxy().await?;
        log_info!("代理已停止");
        
        // 发射代理状态变化事件
        let _ = app.emit("proxy-status-changed", serde_json::json!({
            "is_running": false,
            "current_server": null
        }));
    } else {
        // 代理未运行，什么也不做
        log_info!("代理未运行，无需停止");
    }
    
    // 重新构建托盘菜单以更新状态
    if let Ok(new_menu) = build_tray_menu(app).await {
        // 获取托盘实例并更新菜单
        if let Some(tray) = app.tray_by_id("main-tray") {
            if let Err(e) = tray.set_menu(Some(new_menu)) {
                log_error!("更新托盘菜单失败: {}", e);
            }
        }
    }
    
    Ok(())
}

/// 处理启动特定服务器
/// 
/// # Arguments
/// * `app` - 应用句柄
/// * `server_id` - 服务器ID
/// 
/// # Returns
/// * `Result<(), String>` - 操作结果
async fn handle_start_server<R: Runtime>(app: &tauri::AppHandle<R>, server_id: &str) -> Result<(), String> {
    // 检查当前代理状态
    let proxy_status = commands::get_proxy_status().await.map_err(|e| e.to_string())?;
    
    if proxy_status.is_running {
        // 如果代理正在运行，先停止当前代理
        commands::stop_proxy().await?;
        log_info!("已停止当前代理");
    }
    
    // 启动指定的服务器
    commands::start_proxy(server_id.to_string()).await?;
    log_info!("已启动服务器: {}", server_id);
    
    // 发射代理状态变化事件
    let _ = app.emit("proxy-status-changed", serde_json::json!({
        "is_running": true,
        "current_server": server_id
    }));
    
    // 重新构建托盘菜单以更新状态
    if let Ok(new_menu) = build_tray_menu(app).await {
        // 获取托盘实例并更新菜单
        if let Some(tray) = app.tray_by_id("main-tray") {
            if let Err(e) = tray.set_menu(Some(new_menu)) {
                log_error!("更新托盘菜单失败: {}", e);
            }
        }
    }
    
    Ok(())
}

/// 打开配置文件目录
/// 
/// # Returns
/// * `Result<(), String>` - 操作结果
async fn open_config_directory() -> Result<(), String> {
    use std::process::Command;
    
    // 获取配置目录路径
    let config_dir = config::AppConfig::config_path()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("无法获取配置目录")?
        .to_path_buf();
    
    // 根据操作系统打开目录
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&config_dir)
            .spawn()
            .map_err(|e| format!("打开配置目录失败: {}", e))?;
    }
    
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&config_dir)
            .spawn()
            .map_err(|e| format!("打开配置目录失败: {}", e))?;
    }
    
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&config_dir)
            .spawn()
            .map_err(|e| format!("打开配置目录失败: {}", e))?;
    }
    
    Ok(())
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
            
            // 初始化日志系统
    if let Err(e) = logger::init_logger() {
        eprintln!("初始化日志系统失败: {}", e);
    }
    
    // 测试日志系统
    log_info!("RuRay 应用程序启动");
    log_debug!("当前运行模式: {}", if cfg!(debug_assertions) { "Debug" } else { "Release" });

            // 设置TunManager的应用句柄
            tun::TunManager::instance().set_app_handle(app.handle().clone());

            // 创建系统托盘 - 使用异步任务
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(tray_menu) = build_tray_menu(&app_handle).await {
                    let _tray = TrayIconBuilder::with_id("main-tray")  // 设置托盘ID
                        .icon(app_handle.default_window_icon().unwrap().clone())
                        .menu(&tray_menu)
                        .on_tray_icon_event(|tray, event| {
                            handle_tray_event(tray.app_handle(), event);
                        })
                        .on_menu_event(|app, event| {
                            handle_menu_event(app, event);
                        })
                        .build(&app_handle);
                    
                    if let Err(e) = _tray {
                        log_error!("创建系统托盘失败: {}", e);
                    }
                } else {
                    log_error!("构建托盘菜单失败");
                }
            });

            Ok(())
        })
        .on_window_event(|_window, event| {
            match event {
                WindowEvent::CloseRequested { .. } => {
                    // 在窗口关闭时停止所有服务
                    tauri::async_runtime::spawn(async move {
                        // 检查并停止代理服务器
                        let proxy_manager = proxy::ProxyManager::instance();
                        if proxy_manager.is_process_running() {
                            log_info!("应用关闭中，检测到正在运行的代理服务器，正在停止...");
                            if let Err(e) = proxy_manager.stop().await {
                                log_error!("停止代理服务器失败: {}", e);
                            } else {
                                log_info!("代理服务器已停止");
                            }
                        }
                        
                        // 停止TUN模式
                        if tun::TunManager::instance().is_running_sync() {
                            log_info!("应用关闭中，正在停止TUN模式...");
                            if let Err(e) = tun::TunManager::instance().stop_sync() {
                                log_error!("停止TUN模式失败: {}", e);
                            } else {
                                log_info!("TUN模式已停止");
                            }
                        }
                        
                        log_info!("应用清理完成，准备退出");
                    });
                }
                _ => {}
            }
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
