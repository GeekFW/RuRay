/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::Emitter;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::network::NetworkSpeedStats;
use crate::proxy::ProxyManager;
use crate::system::SystemManager;
use crate::tun::{TunConfig, TunManager, TunStatus};
use crate::xray::XrayManager;

/// 服务器信息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub config: HashMap<String, serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// 代理状态结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub is_running: bool,
    pub status: String, // "connected" | "connecting" | "disconnected"
    pub current_server: Option<String>,
    pub proxy_mode: String,
    pub uptime: u64,
    pub upload_speed: u64,
    pub download_speed: u64,
    pub total_upload: u64,
    pub total_download: u64,
}

/// 系统统计信息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub network_upload: u64,
    pub network_download: u64,
}

/// 获取服务器列表
#[tauri::command]
pub async fn get_servers() -> Result<Vec<ServerInfo>, String> {
    let config = AppConfig::load().map_err(|e| e.to_string())?;
    Ok(config.servers)
}

/// 添加服务器
#[tauri::command]
pub async fn add_server(server: ServerInfo) -> Result<String, String> {
    let mut config = AppConfig::load().map_err(|e| e.to_string())?;
    let mut new_server = server;
    new_server.id = Uuid::new_v4().to_string();
    new_server.created_at = chrono::Utc::now().to_rfc3339();
    new_server.updated_at = new_server.created_at.clone();
    
    config.servers.push(new_server.clone());
    config.save().map_err(|e| e.to_string())?;
    
    Ok(new_server.id)
}

/// 更新服务器
#[tauri::command]
pub async fn update_server(server: ServerInfo) -> Result<(), String> {
    let mut config = AppConfig::load().map_err(|e| e.to_string())?;
    
    if let Some(existing_server) = config.servers.iter_mut().find(|s| s.id == server.id) {
        existing_server.name = server.name;
        existing_server.protocol = server.protocol;
        existing_server.address = server.address;
        existing_server.port = server.port;
        existing_server.config = server.config;
        existing_server.updated_at = chrono::Utc::now().to_rfc3339();
        
        config.save().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("服务器不存在".to_string())
    }
}

/// 删除服务器
#[tauri::command]
pub async fn delete_server(server_id: String) -> Result<(), String> {
    let mut config = AppConfig::load().map_err(|e| e.to_string())?;
    
    // 查找要删除的服务器信息，用于清理配置文件
    let server_to_delete = config.servers.iter().find(|s| s.id == server_id);
    
    if let Some(server) = server_to_delete {
        // 清理对应的配置文件
        let proxy_manager = ProxyManager::instance();
        let _ = proxy_manager.cleanup_server_config(&server.id, &server.name);
    }
    
    config.servers.retain(|s| s.id != server_id);
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

// ==================== TUN 模式相关命令 ====================

/// 启动TUN模式
/// 
/// # 参数
/// * `config` - TUN配置
/// 
/// # 返回值
/// * `Result<(), String>` - 启动结果
#[tauri::command]
pub async fn start_tun_mode(config: TunConfig) -> Result<(), String> {
    let tun_manager = TunManager::instance();
    tun_manager.start(config).await.map_err(|e| e.to_string())
}

/// 停止TUN模式
/// 
/// # 返回值
/// * `Result<(), String>` - 停止结果
#[tauri::command]
pub async fn stop_tun_mode() -> Result<(), String> {
    let tun_manager = TunManager::instance();
    tun_manager.stop().await.map_err(|e| e.to_string())
}

/// 获取TUN模式状态
/// 
/// # 返回值
/// * `Result<TunStatus, String>` - TUN状态
#[tauri::command]
pub async fn get_tun_status() -> Result<TunStatus, String> {
    let tun_manager = TunManager::instance();
    Ok(tun_manager.get_status().await)
}

/// 检查TUN模式是否运行中
/// 
/// # 返回值
/// * `Result<bool, String>` - 是否运行中
#[tauri::command]
pub async fn is_tun_running() -> Result<bool, String> {
    let tun_manager = TunManager::instance();
    Ok(tun_manager.is_running().await)
}

/// 获取TUN配置
/// 
/// # 返回值
/// * `Result<TunConfig, String>` - TUN配置
#[tauri::command]
pub async fn get_tun_config() -> Result<TunConfig, String> {
    let tun_manager = TunManager::instance();
    Ok(tun_manager.get_config().await)
}

/// 更新TUN配置
/// 
/// # 参数
/// * `config` - 新的TUN配置
/// 
/// # 返回值
/// * `Result<(), String>` - 更新结果
#[tauri::command]
pub async fn update_tun_config(config: TunConfig) -> Result<(), String> {
    let tun_manager = TunManager::instance();
    tun_manager.update_config(config).await.map_err(|e| e.to_string())
}

/// 保存TUN配置到文件
/// 
/// # 参数
/// * `config` - 要保存的TUN配置
/// 
/// # 返回值
/// * `Result<(), String>` - 保存结果
#[tauri::command]
pub async fn save_tun_config(config: TunConfig) -> Result<(), String> {
    // 更新TUN管理器中的配置
    let tun_manager = TunManager::instance();
    tun_manager.update_config(config.clone()).await.map_err(|e| e.to_string())?;
    
    // 保存到应用配置文件
    let mut app_config = AppConfig::load().map_err(|e| e.to_string())?;
    app_config.tun_config = config;
    app_config.save().map_err(|e| e.to_string())?;
    
    Ok(())
}

/// 设置系统路由（启用/禁用TUN模式路由）
/// 
/// # 参数
/// * `enable` - 是否启用路由
/// 
/// # 返回值
/// * `Result<(), String>` - 设置结果
#[tauri::command]
pub async fn set_tun_system_route(_enable: bool) -> Result<(), String> {
    // 注意：使用tun2proxy时，系统路由由tun2proxy自动管理
    // 这个函数保留用于兼容性，但实际上不执行任何操作
    Ok(())
}

/// 切换TUN模式开关
/// 
/// # 参数
/// * `enabled` - 是否启用TUN模式
/// 
/// # 返回值
/// * `Result<(), String>` - 切换结果
#[tauri::command]
pub async fn toggle_tun_mode(enabled: bool) -> Result<(), String> {
    let mut config = AppConfig::load().map_err(|e| e.to_string())?;
    config.tun_enabled = enabled;
    config.save().map_err(|e| e.to_string())?;
    
    let tun_manager = TunManager::instance();
    
    if enabled {
        // 启用TUN模式
        let tun_config = config.tun_config.clone();
        if let Err(e) = tun_manager.start(tun_config).await {
            // TUN启动失败时，重置配置并保存
            let mut reset_config = AppConfig::load().map_err(|e| e.to_string())?;
            reset_config.tun_enabled = false;
            reset_config.save().map_err(|e| e.to_string())?;
            return Err(e.to_string());
        }
        // 注意：使用tun2proxy时，系统路由由tun2proxy自动管理
    } else {
        // 禁用TUN模式
        // 注意：使用tun2proxy时，系统路由由tun2proxy自动管理
        tun_manager.stop().await.map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

/// 测试服务器连接
/// 使用真实的 Xray 环境进行连接测试
#[tauri::command]
pub async fn test_server_connection(server_id: String) -> Result<serde_json::Value, String> {
    let config = AppConfig::load().map_err(|e| e.to_string())?;
    
    if let Some(server) = config.servers.iter().find(|s| s.id == server_id) {
        // 创建临时的代理管理器进行测试
        let proxy_manager = ProxyManager::instance();
        
        let start_time = std::time::Instant::now();
        
        match proxy_manager.test_connection(server).await {
            Ok(success) => {
                let latency = start_time.elapsed().as_millis() as u64;
                
                if success {
                    Ok(serde_json::json!({
                        "success": true,
                        "ping": latency,
                        "message": "连接测试成功"
                    }))
                } else {
                    Ok(serde_json::json!({
                        "success": false,
                        "ping": 0,
                        "message": "连接测试失败"
                    }))
                }
            }
            Err(e) => {
                Ok(serde_json::json!({
                    "success": false,
                    "ping": 0,
                    "message": format!("连接测试失败: {}", e)
                }))
            }
        }
    } else {
        Err("服务器不存在".to_string())
    }
}

/// 启动代理
/// 启动代理服务并自动配置系统代理设置
#[tauri::command]
pub async fn start_proxy(server_id: String) -> Result<(), String> {
    let config = AppConfig::load().map_err(|e| e.to_string())?;
    
    if let Some(server) = config.servers.iter().find(|s| s.id == server_id) {
        let proxy_manager = ProxyManager::instance();
        
        // 启动代理服务
        proxy_manager.start(server).await.map_err(|e| e.to_string())?;
        
        // 自动配置系统代理
        let system_manager = SystemManager::instance();
        
        // 根据代理模式设置系统代理，同时启用HTTP和SOCKS5
        system_manager.set_proxy_with_mode(
            &config.proxy_mode, 
            config.http_port, 
            config.socks_port
        ).await.map_err(|e| {
            format!("设置系统代理失败: {}", e)
        })?;
        
        Ok(())
    } else {
        Err("服务器不存在".to_string())
    }
}

/// 停止代理
/// 停止代理服务并自动清除系统代理设置
#[tauri::command]
pub async fn stop_proxy() -> Result<(), String> {
    let proxy_manager = ProxyManager::instance();
    
    // 停止代理服务
    proxy_manager.stop().await.map_err(|e| e.to_string())?;
    
    // 根据代理模式决定是否清除系统代理设置
    let config = AppConfig::load().map_err(|e| e.to_string())?;
    
    // 只有在全局代理或PAC模式下才需要清除系统代理
    // 直连模式下本来就没有设置系统代理，无需清除
    if config.proxy_mode == "global" || config.proxy_mode == "pac" {
        let system_manager = SystemManager::instance();
        system_manager.unset_proxy().await.map_err(|e| {
            format!("清除系统代理失败: {}", e)
        })?;
    }
    
    Ok(())
}

/// 获取代理状态
#[tauri::command]
pub async fn get_proxy_status() -> Result<ProxyStatus, String> {
    let proxy_manager = ProxyManager::instance();
    proxy_manager.get_status().await.map_err(|e| e.to_string())
}

/// 设置代理模式
#[tauri::command]
pub async fn set_proxy_mode(mode: String) -> Result<(), String> {
    let mut config = AppConfig::load().map_err(|e| e.to_string())?;
    config.proxy_mode = mode;
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

/// 获取系统统计信息
#[tauri::command]
pub async fn get_system_stats() -> Result<SystemStats, String> {
    let system_manager = SystemManager::instance();
    system_manager.get_stats().await.map_err(|e| e.to_string())
}

/// 设置系统代理
#[tauri::command]
pub async fn set_system_proxy(proxy_url: String) -> Result<(), String> {
    let system_manager = SystemManager::instance();
    system_manager.set_proxy(&proxy_url).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// 清除系统代理
#[tauri::command]
pub async fn clear_system_proxy() -> Result<(), String> {
    let system_manager = SystemManager::instance();
    system_manager.unset_proxy().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// 获取系统代理状态
#[tauri::command]
pub async fn get_system_proxy_status() -> Result<serde_json::Value, String> {
    let system_manager = SystemManager::instance();
    system_manager.get_proxy_status().await.map_err(|e| e.to_string())
}

/// 清理未使用的配置文件
/// 根据当前服务器列表，清理不再使用的配置文件
#[tauri::command]
pub async fn cleanup_unused_configs() -> Result<(), String> {
    let config = AppConfig::load().map_err(|e| e.to_string())?;
    let active_server_ids: Vec<String> = config.servers.iter().map(|s| s.id.clone()).collect();
    
    let proxy_manager = ProxyManager::instance();
    proxy_manager.cleanup_unused_configs(&active_server_ids).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// 检查 Xray Core 更新
#[tauri::command]
pub async fn check_xray_update() -> Result<Option<String>, String> {
    let xray_manager = XrayManager::new();
    xray_manager.check_update().await.map_err(|e| e.to_string())
}

/// 下载 Xray Core 更新
#[tauri::command]
pub async fn download_xray_update(version: String) -> Result<(), String> {
    let xray_manager = XrayManager::new();
    xray_manager.download_update(&version).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// 下载 Xray Core 更新（带进度回调）
#[tauri::command]
pub async fn download_xray_update_with_progress(
    app_handle: tauri::AppHandle,
    version: String,
) -> Result<(), String> {
    let xray_manager = XrayManager::new();
    
    xray_manager.download_update_with_progress(&version, |current, total, message| {
        let progress = if total > 0 { (current * 100 / total) as u32 } else { 0 };
        
        // 发送进度事件到前端
        let _ = app_handle.emit("xray-download-progress", serde_json::json!({
            "progress": progress,
            "message": message
        }));
    }).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

/// 获取 Xray Core 版本
#[tauri::command]
pub async fn get_xray_version() -> Result<String, String> {
    let xray_manager = XrayManager::new();
    xray_manager.get_version().await.map_err(|e| e.to_string())
}

/// 检查 Xray Core 是否存在
#[tauri::command]
pub async fn check_xray_exists() -> Result<bool, String> {
    AppConfig::check_xray_exists().map_err(|e| e.to_string())
}

/// 获取 Xray Core 可执行文件路径
#[tauri::command]
pub async fn get_xray_path() -> Result<String, String> {
    let path = AppConfig::xray_executable().map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// 下载地理位置数据文件（geoip.dat 和 geosite.dat）
/// 
/// # 参数
/// * `app_handle` - Tauri 应用句柄，用于发送进度事件
/// 
/// # 返回值
/// * `Result<(), String>` - 下载结果
#[tauri::command]
pub async fn download_geo_files(app_handle: tauri::AppHandle) -> Result<(), String> {
    let xray_manager = XrayManager::new();
    
    xray_manager.download_geo_files(|progress, total, message| {
        let _ = app_handle.emit("geo-download-progress", serde_json::json!({
            "progress": progress,
            "total": total,
            "message": message
        }));
    }).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

/// 检查地理位置数据文件是否存在
/// 
/// # 返回值
/// * `Result<bool, String>` - 文件是否都存在
#[tauri::command]
pub async fn check_geo_files_exist() -> Result<bool, String> {
    let xray_manager = XrayManager::new();
    xray_manager.check_geo_files_exist().map_err(|e| e.to_string())
}

/// 确保所有 Xray 文件都存在（可执行文件和地理位置数据文件）
/// 
/// # 参数
/// * `app_handle` - Tauri 应用句柄，用于发送进度事件
/// 
/// # 返回值
/// * `Result<(), String>` - 检查和下载结果
#[tauri::command]
pub async fn ensure_xray_files(app_handle: tauri::AppHandle) -> Result<(), String> {
    let xray_manager = XrayManager::new();
    
    xray_manager.ensure_all_files(|progress, total, message| {
        let _ = app_handle.emit("xray-setup-progress", serde_json::json!({
            "progress": progress,
            "total": total,
            "message": message
        }));
    }).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

/// 测试 Xray 配置有效性
/// 测试 Xray 配置的有效性
/// 
/// # 参数
/// * `server_id` - 服务器ID
/// 
/// # 返回值
/// * `Ok(String)` - 配置验证成功的消息
/// * `Err(String)` - 配置验证失败的错误信息
/// 
/// # 异常
/// * 当服务器不存在时返回错误
/// * 当 Xray Core 可执行文件不存在时返回错误
/// * 当配置生成失败时返回错误
/// * 当配置验证失败时返回错误
#[tauri::command]
pub async fn test_xray_config(server_id: String) -> Result<String, String> {
    let config = AppConfig::load().map_err(|e| format!("加载配置失败: {}", e))?;
    
    if let Some(server) = config.servers.iter().find(|s| s.id == server_id) {
        let proxy_manager = ProxyManager::instance();
        
        // 检查 Xray Core 是否存在
        let xray_executable = AppConfig::xray_executable().map_err(|e| format!("获取 Xray 路径失败: {}", e))?;
        if !xray_executable.exists() {
            return Err(format!("Xray Core 可执行文件不存在: {}", xray_executable.display()));
        }

        // 生成 Xray 配置
        let xray_config = proxy_manager.generate_xray_config(server).map_err(|e| format!("生成配置失败: {}", e))?;
        
        // 保存测试配置到临时文件
        let config_path = proxy_manager.save_test_config(&xray_config).map_err(|e| format!("保存测试配置失败: {}", e))?;
        
        // 使用 Xray 的 -test 参数验证配置
        let output = std::process::Command::new(&xray_executable)
            .arg("-config")
            .arg(&config_path)
            .arg("-test")
            .output()
            .map_err(|e| format!("执行 Xray Core 失败: {}", e))?;

        // 清理测试配置文件
        let _ = std::fs::remove_file(&config_path);

        if output.status.success() {
            Ok("配置验证成功".to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            // 提取更有用的错误信息
            let error_msg = if !stderr.is_empty() {
                stderr.to_string()
            } else if !stdout.is_empty() {
                stdout.to_string()
            } else {
                format!("配置验证失败 (退出码: {})", output.status.code().unwrap_or(-1))
            };
            
            Err(error_msg)
        }
    } else {
        Err(format!("服务器不存在: {}", server_id))
    }
}

/// 获取网络速度统计
/// 返回当前的上传下载速度和总流量统计
#[tauri::command]
pub async fn get_network_speed() -> Result<NetworkSpeedStats, String> {
    // 确保网络统计监控已启动
    let manager = crate::network::NetworkStatsManager::instance();
    
    // 检查监控是否已启动，如果没有则启动
    let need_start = {
        match manager.stats_task.lock() {
            Ok(task_guard) => task_guard.is_none(),
            Err(_) => true, // 如果锁失败，假设需要启动
        }
    };
    
    if need_start {
        println!("[DEBUG] 网络统计监控未启动，正在自动启动...");
        if let Err(e) = manager.start_monitoring().await {
            eprintln!("[ERROR] 自动启动网络统计监控失败: {}", e);
            return Err(format!("启动网络统计监控失败: {}", e));
        } else {
            println!("[DEBUG] 网络统计监控自动启动成功");
        }
    }
    
    Ok(manager.get_current_speed())
}

/// 重置网络统计
/// 重置总流量统计，从当前时刻开始重新计算
#[tauri::command]
pub async fn reset_network_stats() -> Result<(), String> {
    crate::network::reset_network_stats()
        .await
        .map_err(|e| format!("重置网络统计失败: {}", e))
}

/// 获取应用配置
#[tauri::command]
pub async fn get_app_config() -> Result<AppConfig, String> {
    AppConfig::load().map_err(|e| e.to_string())
}

/// 保存应用配置
#[tauri::command]
pub async fn save_app_config(config: AppConfig) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())
}

/// 导出配置
#[tauri::command]
pub async fn export_config() -> Result<String, String> {
    let config = AppConfig::load().map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

/// 导入配置
#[tauri::command]
pub async fn import_config(config_json: String) -> Result<(), String> {
    let config: AppConfig = serde_json::from_str(&config_json).map_err(|e| e.to_string())?;
    config.save().map_err(|e| e.to_string())
}

/// 重新生成服务器配置文件
/// 强制重新生成指定服务器的配置文件，覆盖现有文件
/// 
/// # 参数
/// * `server_id` - 服务器ID
/// 
/// # 返回值
/// * `Ok(())` - 成功重新生成配置文件
/// * `Err(String)` - 重新生成失败的错误信息
/// 
/// # 异常
/// * 当服务器不存在时返回错误
/// * 当生成配置文件失败时返回错误
#[tauri::command]
pub async fn regenerate_server_config(server_id: String) -> Result<(), String> {
    let config = AppConfig::load().map_err(|e| format!("加载配置失败: {}", e))?;
    
    if let Some(server) = config.servers.iter().find(|s| s.id == server_id) {
        let proxy_manager = ProxyManager::instance();
        
        proxy_manager.regenerate_config(server).await.map_err(|e| {
            format!("重新生成配置文件失败: {}", e)
        })?;
        
        Ok(())
    } else {
        Err("服务器不存在".to_string())
    }
}

/// 打开服务器配置文件
/// 打开指定服务器的配置文件，如果文件不存在则打开配置目录
/// 
/// # 参数
/// * `server_id` - 服务器ID
/// 
/// # 返回值
/// * `Ok(())` - 成功打开文件或目录
/// * `Err(String)` - 打开失败的错误信息
/// 
/// # 异常
/// * 当服务器不存在时返回错误
/// * 当无法打开文件或目录时返回错误
#[tauri::command]
pub async fn open_server_config_file(server_id: String) -> Result<(), String> {
    let config = AppConfig::load().map_err(|e| format!("加载配置失败: {}", e))?;
    
    if let Some(server) = config.servers.iter().find(|s| s.id == server_id) {
        let proxy_manager = ProxyManager::instance();
        
        // 获取服务器配置文件路径
        let config_file_path = proxy_manager.get_server_config_path(&server.id, &server.name);
        
        if config_file_path.exists() {
            // 配置文件存在，直接打开文件
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("cmd")
                    .args(["/C", "start", "", &config_file_path.to_string_lossy()])
                    .spawn()
                    .map_err(|e| format!("打开配置文件失败: {}", e))?;
            }
            
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open")
                    .arg(&config_file_path)
                    .spawn()
                    .map_err(|e| format!("打开配置文件失败: {}", e))?;
            }
            
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("xdg-open")
                    .arg(&config_file_path)
                    .spawn()
                    .map_err(|e| format!("打开配置文件失败: {}", e))?;
            }
        } else {
            // 配置文件不存在，打开配置目录
            let config_dir = config_file_path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| {
                    AppConfig::servers_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                });
            
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("explorer")
                    .arg(&config_dir)
                    .spawn()
                    .map_err(|e| format!("打开配置目录失败: {}", e))?;
            }
            
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open")
                    .arg(&config_dir)
                    .spawn()
                    .map_err(|e| format!("打开配置目录失败: {}", e))?;
            }
            
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("xdg-open")
                    .arg(&config_dir)
                    .spawn()
                    .map_err(|e| format!("打开配置目录失败: {}", e))?;
            }
        }
        
        Ok(())
    } else {
        Err(format!("服务器不存在: {}", server_id))
    }
}