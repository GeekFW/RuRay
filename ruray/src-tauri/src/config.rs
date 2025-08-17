/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::commands::ServerInfo;
use crate::tun::TunConfig;

/// 为 rule_type 字段提供默认值
fn default_rule_type() -> String {
    "field".to_string()
}

/// 路由规则结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    #[serde(rename = "type", alias = "rule_type", default = "default_rule_type")]
    pub rule_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,
    #[serde(rename = "outboundTag", alias = "outbound_tag")]
    pub outbound_tag: String,
}

/// 路由配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    #[serde(rename = "domainStrategy", default = "default_domain_strategy")]
    pub domain_strategy: String,
    #[serde(default)]
    pub rules: Vec<RoutingRule>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            domain_strategy: "AsIs".to_string(),
            rules: vec![
                RoutingRule {
                    rule_type: "field".to_string(),
                    ip: Some(vec!["geoip:private".to_string()]),
                    domain: None,
                    outbound_tag: "direct".to_string(),
                }
            ],
        }
    }
}

/// 为 domain_strategy 字段提供默认值
fn default_domain_strategy() -> String {
    "AsIs".to_string()
}

/// 为theme_color字段提供默认值
fn default_theme_color() -> String {
    "green".to_string()
}

fn default_auth_method() -> String {
    "noauth".to_string()
}

/// 为 log_path 字段提供默认值
fn default_log_path() -> String {
    // 默认日志路径为配置目录下的 log/ruray.log
    match dirs::config_dir() {
        Some(config_dir) => config_dir
            .join("RuRay")
            .join("log")
            .join("ruray.log")
            .to_string_lossy()
            .to_string(),
        None => "./log/ruray.log".to_string(),
    }
}

/// 应用配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: String,
    pub servers: Vec<ServerInfo>,
    pub current_server: Option<String>,
    pub proxy_mode: String,
    pub auto_start: bool,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub theme: String,
    /// 主题色配置
    #[serde(default = "default_theme_color")]
    pub theme_color: String,
    pub language: String,
    pub log_level: String,
    /// 日志文件路径配置
    #[serde(default = "default_log_path")]
    pub log_path: String,
    pub http_port: u16,
    pub socks_port: u16,
    pub pac_port: u16,
    /// inbound 配置
    #[serde(default)]
    pub inbound_sniffing_enabled: bool,
    #[serde(default)]
    pub inbound_udp_enabled: bool,
    #[serde(default = "default_auth_method")]
    pub inbound_auth_method: String,
    #[serde(default)]
    pub inbound_allow_transparent: bool,
    /// Xray Core 可执行文件路径
    pub xray_path: Option<String>,
    /// 路由配置
    #[serde(default)]
    pub routing_config: RoutingConfig,
    /// TUN模式配置
    #[serde(default)]
    pub tun_config: TunConfig,
    /// 是否启用TUN模式
    #[serde(default)]
    pub tun_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 服务器配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub uuid: Option<String>,
    pub password: Option<String>,
    pub method: Option<String>,
    pub network: Option<String>,
    pub security: Option<String>,
    pub sni: Option<String>,
    pub alpn: Option<Vec<String>>,
    pub path: Option<String>,
    pub host: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            servers: Vec::new(),
            current_server: None,
            proxy_mode: "pac".to_string(),
            auto_start: false,
            minimize_to_tray: true,
            start_minimized: false,
            theme: "dark".to_string(),
            theme_color: "green".to_string(),
            language: "zh-CN".to_string(),
            log_level: "info".to_string(),
            log_path: default_log_path(),
            http_port: 10086,
            socks_port: 10087,
            pac_port: 8090,
            inbound_sniffing_enabled: false,
            inbound_udp_enabled: false,
            inbound_auth_method: "noauth".to_string(),
            inbound_allow_transparent: false,
            xray_path: None,
            routing_config: RoutingConfig::default(),
            tun_config: TunConfig::default(),
            tun_enabled: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

impl AppConfig {
    /// 获取配置文件路径
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("无法获取配置目录")?
            .join("RuRay");
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .context("无法创建配置目录")?;
        }
        
        Ok(config_dir.join("config.json"))
    }

    /// 加载配置
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .context("无法读取配置文件")?;
            
            let mut config: AppConfig = serde_json::from_str(&content)
                .context("无法解析配置文件")?;
            
            config.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(config)
        } else {
            let config = AppConfig::default();
            config.save()?;
            Ok(config)
        }
    }

    /// 保存配置
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let mut config = self.clone();
        config.updated_at = chrono::Utc::now().to_rfc3339();
        
        let content = serde_json::to_string_pretty(&config)
            .context("无法序列化配置")?;
        
        fs::write(&config_path, content)
            .context("无法写入配置文件")?;
        
        Ok(())
    }

    /// 获取服务器配置目录
    pub fn servers_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("无法获取配置目录")?
            .join("RuRay")
            .join("server")
            .join("conf");
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .context("无法创建服务器配置目录")?;
        }
        
        Ok(config_dir)
    }

    /// 获取日志目录
    pub fn logs_dir() -> Result<PathBuf> {
        let logs_dir = dirs::config_dir()
            .context("无法获取配置目录")?
            .join("RuRay")
            .join("logs");
        
        if !logs_dir.exists() {
            fs::create_dir_all(&logs_dir)
                .context("无法创建日志目录")?;
        }
        
        Ok(logs_dir)
    }

    /// 获取 Xray Core 目录
    pub fn xray_dir() -> Result<PathBuf> {
        let xray_dir = dirs::config_dir()
            .context("无法获取配置目录")?
            .join("RuRay")
            .join("xray");
        
        if !xray_dir.exists() {
            fs::create_dir_all(&xray_dir)
                .context("无法创建 Xray 目录")?;
        }
        
        Ok(xray_dir)
    }

    /// 获取 Xray Core 可执行文件路径
    /// 优先使用用户配置的路径，如果没有配置则使用默认路径
    pub fn xray_executable() -> Result<PathBuf> {
        // 尝试加载配置获取用户自定义路径
        if let Ok(config) = Self::load() {
            if let Some(custom_path) = config.xray_path {
                return Ok(PathBuf::from(custom_path));
            }
        }
        
        // 使用默认路径
        let xray_dir = Self::xray_dir()?;
        
        #[cfg(target_os = "windows")]
        let executable = xray_dir.join("xray.exe");
        
        #[cfg(not(target_os = "windows"))]
        let executable = xray_dir.join("xray");
        
        Ok(executable)
    }

    /// 检查 Xray Core 是否存在
    /// 
    /// # Returns
    /// 
    /// * `Result<bool>` - 如果文件存在返回 true，否则返回 false
    pub fn check_xray_exists() -> Result<bool> {
        let executable = Self::xray_executable()?;
        Ok(executable.exists())
    }
}

/// 初始化应用配置
pub fn init_app_config() -> Result<()> {
    let _config = AppConfig::load()?;
    Ok(())
}