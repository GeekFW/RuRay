/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use uuid::Uuid;

use crate::config::{AppConfig, ServerConfig};
use crate::proxy::ProxyManager;
use crate::system::SystemManager;
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
        let proxy_manager = ProxyManager::new();
        let _ = proxy_manager.cleanup_server_config(&server.id, &server.name);
    }
    
    config.servers.retain(|s| s.id != server_id);
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

/// 测试服务器连接
/// 使用真实的 Xray 环境进行连接测试
#[tauri::command]
pub async fn test_server_connection(server_id: String) -> Result<serde_json::Value, String> {
    let config = AppConfig::load().map_err(|e| e.to_string())?;
    
    if let Some(server) = config.servers.iter().find(|s| s.id == server_id) {
        // 创建临时的代理管理器进行测试
        let proxy_manager = ProxyManager::new();
        
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
        let proxy_manager = ProxyManager::new();
        
        // 启动代理服务
        proxy_manager.start(server).await.map_err(|e| e.to_string())?;
        
        // 自动配置系统代理
        let system_manager = SystemManager::new();
        
        // 根据代理模式设置系统代理
        match config.proxy_mode.as_str() {
            "global" => {
                // 全局模式：使用 SOCKS 代理
                let socks_proxy = format!("socks5://127.0.0.1:{}", config.socks_port);
                system_manager.set_proxy(&socks_proxy).await.map_err(|e| {
                    format!("设置系统代理失败: {}", e)
                })?;
            },
            "pac" => {
                // PAC 模式：使用 HTTP 代理
                let http_proxy = format!("127.0.0.1:{}", config.http_port);
                system_manager.set_proxy(&http_proxy).await.map_err(|e| {
                    format!("设置系统代理失败: {}", e)
                })?;
            },
            "direct" => {
                // 直连模式：不设置系统代理
                // 仅启动代理服务，不修改系统设置
            },
            _ => {
                // 默认使用 HTTP 代理
                let http_proxy = format!("127.0.0.1:{}", config.http_port);
                system_manager.set_proxy(&http_proxy).await.map_err(|e| {
                    format!("设置系统代理失败: {}", e)
                })?;
            }
        }
        
        Ok(())
    } else {
        Err("服务器不存在".to_string())
    }
}

/// 停止代理
/// 停止代理服务并自动清除系统代理设置
#[tauri::command]
pub async fn stop_proxy() -> Result<(), String> {
    let proxy_manager = ProxyManager::new();
    
    // 停止代理服务
    proxy_manager.stop().await.map_err(|e| e.to_string())?;
    
    // 自动清除系统代理设置
    let system_manager = SystemManager::new();
    system_manager.unset_proxy().await.map_err(|e| {
        format!("清除系统代理失败: {}", e)
    })?;
    
    Ok(())
}

/// 获取代理状态
#[tauri::command]
pub async fn get_proxy_status() -> Result<ProxyStatus, String> {
    let proxy_manager = ProxyManager::new();
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
    let system_manager = SystemManager::new();
    system_manager.get_stats().await.map_err(|e| e.to_string())
}

/// 设置系统代理
#[tauri::command]
pub async fn set_system_proxy(proxy_url: String) -> Result<(), String> {
    let system_manager = SystemManager::new();
    system_manager.set_proxy(&proxy_url).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// 清除系统代理
#[tauri::command]
pub async fn clear_system_proxy() -> Result<(), String> {
    let system_manager = SystemManager::new();
    system_manager.unset_proxy().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// 获取系统代理状态
#[tauri::command]
pub async fn get_system_proxy_status() -> Result<serde_json::Value, String> {
    let system_manager = SystemManager::new();
    system_manager.get_proxy_status().await.map_err(|e| e.to_string())
}

/// 清理未使用的配置文件
/// 根据当前服务器列表，清理不再使用的配置文件
#[tauri::command]
pub async fn cleanup_unused_configs() -> Result<(), String> {
    let config = AppConfig::load().map_err(|e| e.to_string())?;
    let active_server_ids: Vec<String> = config.servers.iter().map(|s| s.id.clone()).collect();
    
    let proxy_manager = ProxyManager::new();
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
        let proxy_manager = ProxyManager::new();
        
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
        let proxy_manager = ProxyManager::new();
        
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