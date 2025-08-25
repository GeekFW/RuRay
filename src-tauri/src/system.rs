/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Manager, path::BaseDirectory};

use sysinfo::{System, Networks};

use crate::commands::SystemStats;

/// 系统管理器
pub struct SystemManager {
    system: std::sync::Mutex<System>,
    networks: std::sync::Mutex<Networks>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
}

// 全局单例实例
static SYSTEM_MANAGER: OnceLock<SystemManager> = OnceLock::new();

impl SystemManager {
    /// 获取全局系统管理器实例（单例模式）
    pub fn instance() -> &'static SystemManager {
        SYSTEM_MANAGER.get_or_init(|| {
            Self {
                system: std::sync::Mutex::new(System::new_all()),
                networks: std::sync::Mutex::new(Networks::new_with_refreshed_list()),
                app_handle: Arc::new(Mutex::new(None)),
            }
        })
    }

    /// 设置应用句柄
    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
    }

    /// 获取系统统计信息
    pub async fn get_stats(&self) -> Result<SystemStats> {
        let mut system = self.system.lock().unwrap();
        let mut networks = self.networks.lock().unwrap();
        
        // 刷新系统信息
        system.refresh_all();
        networks.refresh();
        
        // 获取CPU使用率（所有核心的平均值）
        let cpu_usage = system.global_cpu_info().cpu_usage();
        
        // 获取内存信息
        let memory_total = system.total_memory();
        let memory_used = system.used_memory();
        let memory_usage = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };
        
        // 获取网络统计信息
        let mut total_received = 0;
        let mut total_transmitted = 0;
        
        for (_interface_name, network) in networks.iter() {
            total_received += network.received();
            total_transmitted += network.transmitted();
        }
        
        Ok(SystemStats {
            cpu_usage,
            memory_usage,
            memory_total,
            memory_used,
            network_upload: total_transmitted,
            network_download: total_received,
        })
    }

    /// 设置系统代理
    pub async fn set_proxy(&self, proxy_url: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.set_windows_proxy(proxy_url).await
        }

        #[cfg(target_os = "macos")]
        {
            self.set_macos_proxy(proxy_url).await
        }

        #[cfg(target_os = "linux")]
        {
            self.set_linux_proxy(proxy_url).await
        }
    }

    /// 根据代理模式设置系统代理
    /// 
    /// # 参数
    /// * `proxy_mode` - 代理模式："global"、"pac"、"direct"
    /// * `http_port` - HTTP代理端口
    /// * `socks_port` - SOCKS5代理端口
    pub async fn set_proxy_with_mode(&self, proxy_mode: &str, http_port: u16, socks_port: u16) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.set_windows_proxy_with_mode(proxy_mode, http_port, socks_port).await
        }

        #[cfg(target_os = "macos")]
        {
            // macOS暂时使用原有逻辑
            match proxy_mode {
                "global" => {
                    let socks_proxy = format!("socks5://127.0.0.1:{}", socks_port);
                    self.set_macos_proxy(&socks_proxy).await
                },
                "pac" => {
                    let http_proxy = format!("http://127.0.0.1:{}", http_port);
                    self.set_macos_proxy(&http_proxy).await
                },
                "direct" => {
                    // 直连模式不设置代理
                    Ok(())
                },
                _ => {
                    let http_proxy = format!("http://127.0.0.1:{}", http_port);
                    self.set_macos_proxy(&http_proxy).await
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux暂时使用原有逻辑
            match proxy_mode {
                "global" => {
                    let socks_proxy = format!("socks5://127.0.0.1:{}", socks_port);
                    self.set_linux_proxy(&socks_proxy).await
                },
                "pac" => {
                    let http_proxy = format!("http://127.0.0.1:{}", http_port);
                    self.set_linux_proxy(&http_proxy).await
                },
                "direct" => {
                    // 直连模式不设置代理
                    Ok(())
                },
                _ => {
                    let http_proxy = format!("http://127.0.0.1:{}", http_port);
                    self.set_linux_proxy(&http_proxy).await
                }
            }
        }
    }

    /// 取消系统代理
    pub async fn unset_proxy(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.unset_windows_proxy().await
        }

        #[cfg(target_os = "macos")]
        {
            self.unset_macos_proxy().await
        }

        #[cfg(target_os = "linux")]
        {
            self.unset_linux_proxy().await
        }
    }

    /// 获取系统代理状态
    pub async fn get_proxy_status(&self) -> Result<serde_json::Value> {
        #[cfg(target_os = "windows")]
        {
            self.get_windows_proxy_status().await
        }

        #[cfg(target_os = "macos")]
        {
            self.get_macos_proxy_status().await
        }

        #[cfg(target_os = "linux")]
        {
            self.get_linux_proxy_status().await
        }
    }

    #[cfg(target_os = "windows")]
    async fn set_windows_proxy(&self, proxy_url: &str) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let internet_settings = hkcu
            .open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings", KEY_WRITE)
            .context("无法打开注册表项")?;

        // 解析代理URL，支持不同的代理类型
        let proxy_server = if proxy_url.starts_with("socks5://") {
            // SOCKS5 代理格式：socks=127.0.0.1:1080
            let addr = proxy_url.strip_prefix("socks5://").unwrap_or(proxy_url);
            format!("socks={}", addr)
        } else if proxy_url.starts_with("http://") {
            // HTTP 代理格式：http=127.0.0.1:8080
            let addr = proxy_url.strip_prefix("http://").unwrap_or(proxy_url);
            format!("http={};https={}", addr, addr)
        } else {
            // 默认作为 HTTP 代理处理
            format!("http={};https={}", proxy_url, proxy_url)
        };

        // 启用代理
        internet_settings
            .set_value("ProxyEnable", &1u32)
            .context("无法设置 ProxyEnable")?;

        // 设置代理服务器
        internet_settings
            .set_value("ProxyServer", &proxy_server)
            .context("无法设置 ProxyServer")?;

        // 设置代理覆盖（本地地址不使用代理）
        // 参考 Privoxy 的实现，排除本地网络和私有网络
        let proxy_override = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;*.local;<local>";
        internet_settings
            .set_value("ProxyOverride", &proxy_override)
            .context("无法设置 ProxyOverride")?;

        // 设置自动检测设置为关闭，避免冲突
        internet_settings
            .set_value("AutoDetect", &0u32)
            .context("无法设置 AutoDetect")?;

        // 关闭自动配置脚本
        internet_settings
            .set_value("AutoConfigURL", &"")
            .context("无法设置 AutoConfigURL")?;

        // 刷新系统设置
        // self.refresh_windows_proxy_settings().await?;

        Ok(())
    }

    /// Windows平台根据代理模式设置系统代理
    /// 
    /// # 参数
    /// * `proxy_mode` - 代理模式："global"、"pac"、"direct"
    /// * `http_port` - HTTP代理端口
    /// * `socks_port` - SOCKS5代理端口
    #[cfg(target_os = "windows")]
    async fn set_windows_proxy_with_mode(&self, proxy_mode: &str, http_port: u16, socks_port: u16) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;


        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let internet_settings = hkcu
            .open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings", KEY_WRITE)
            .context("无法打开注册表项")?;

        match proxy_mode {
            "global" => {
                // 全局模式：设置HTTP和SOCKS5代理
                let proxy_server = format!("http=127.0.0.1:{};https=127.0.0.1:{};socks=127.0.0.1:{}", 
                                          http_port, http_port, socks_port);
                
                // 启用代理
                internet_settings
                    .set_value("ProxyEnable", &1u32)
                    .context("无法设置 ProxyEnable")?;

                // 设置代理服务器
                internet_settings
                    .set_value("ProxyServer", &proxy_server)
                    .context("无法设置 ProxyServer")?;

                // 设置代理覆盖（本地地址不使用代理）
                let proxy_override = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;*.local;<local>";
                internet_settings
                    .set_value("ProxyOverride", &proxy_override)
                    .context("无法设置 ProxyOverride")?;

                // 关闭自动检测和PAC脚本
                internet_settings
                    .set_value("AutoDetect", &0u32)
                    .context("无法设置 AutoDetect")?;
                internet_settings
                    .set_value("AutoConfigURL", &"")
                    .context("无法设置 AutoConfigURL")?;
            },
            "pac" => {
                // PAC模式：设置自动配置脚本和代理服务器
                let pac_file_path = std::env::current_exe()
                    .context("无法获取当前执行文件路径")?
                    .parent()
                    .context("无法获取父目录")?
                    .join("runtime.pac");
                
                // 生成动态PAC文件
                self.generate_pac_file(&pac_file_path, http_port, socks_port).await?;
                
                let pac_url = format!("file:///{}", pac_file_path.to_string_lossy().replace("\\", "/"));
                
                // PAC模式需要同时设置代理服务器和PAC脚本
                // 设置代理服务器（PAC脚本会引用这些代理）
                let proxy_server = format!("http=127.0.0.1:{};https=127.0.0.1:{};socks=127.0.0.1:{}", 
                                          http_port, http_port, socks_port);
                
                // 启用代理
                internet_settings
                    .set_value("ProxyEnable", &1u32)
                    .context("无法设置 ProxyEnable")?;
                
                // 设置代理服务器
                internet_settings
                    .set_value("ProxyServer", &proxy_server)
                    .context("无法设置 ProxyServer")?;
                
                // 设置PAC脚本URL
                internet_settings
                    .set_value("AutoConfigURL", &pac_url)
                    .context("无法设置 AutoConfigURL")?;
                
                // 关闭自动检测
                internet_settings
                    .set_value("AutoDetect", &0u32)
                    .context("无法设置 AutoDetect")?;
                
                // 设置代理覆盖（本地地址不使用代理）
                let proxy_override = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;*.local;<local>";
                internet_settings
                    .set_value("ProxyOverride", &proxy_override)
                    .context("无法设置 ProxyOverride")?;
            },
            "direct" => {
                // 直连模式：清除所有代理设置
                // 禁用代理
                internet_settings
                    .set_value("ProxyEnable", &0u32)
                    .context("无法禁用代理")?;
                
                // 清除代理服务器设置
                internet_settings
                    .set_value("ProxyServer", &"")
                    .context("无法清除代理服务器")?;
                
                // 清除PAC脚本URL
                internet_settings
                    .set_value("AutoConfigURL", &"")
                    .context("无法清除PAC脚本URL")?;
                
                // 关闭自动检测
                internet_settings
                    .set_value("AutoDetect", &0u32)
                    .context("无法关闭自动检测")?;
                
                // 清除代理覆盖设置
                internet_settings
                    .set_value("ProxyOverride", &"")
                    .context("无法清除代理覆盖设置")?;
            },
            _ => {
                // 默认模式：同时设置HTTP和SOCKS5代理
                let proxy_server = format!("http=127.0.0.1:{};https=127.0.0.1:{};socks=127.0.0.1:{}", 
                                          http_port, http_port, socks_port);
                
                // 启用代理
                internet_settings
                    .set_value("ProxyEnable", &1u32)
                    .context("无法设置 ProxyEnable")?;

                // 设置代理服务器
                internet_settings
                    .set_value("ProxyServer", &proxy_server)
                    .context("无法设置 ProxyServer")?;

                // 设置代理覆盖（本地地址不使用代理）
                let proxy_override = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;*.local;<local>";
                internet_settings
                    .set_value("ProxyOverride", &proxy_override)
                    .context("无法设置 ProxyOverride")?;

                // 关闭自动检测和PAC脚本
                internet_settings
                    .set_value("AutoDetect", &0u32)
                    .context("无法设置 AutoDetect")?;
                internet_settings
                    .set_value("AutoConfigURL", &"")
                    .context("无法设置 AutoConfigURL")?;
            }
        }

        Ok(())
    }

    /// 生成动态PAC文件
    #[cfg(target_os = "windows")]
    async fn generate_pac_file(&self, pac_file_path: &std::path::Path, http_port: u16, socks_port: u16) -> Result<()> {
        use std::fs;
        
        let app_handle_guard = self.app_handle.lock().unwrap();
        let app_handle = app_handle_guard.as_ref()
            .context("应用句柄未设置，请先调用 set_app_handle")?;
        
        // 使用Tauri的路径解析API获取资源文件路径
        let default_pac_resource_path = "default.pac";
        
        let pac_template = match (*app_handle).path().resolve(default_pac_resource_path, BaseDirectory::Resource) {
            Ok(pac_path) => {
                if pac_path.exists() {
                    fs::read_to_string(&pac_path)
                        .context("无法读取默认PAC文件")?  
                } else {
                    return Err(anyhow::anyhow!("PAC模板文件不存在: {:?}", pac_path));
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("无法解析PAC模板文件路径: {}", e));
            }
        };
        
        // 替换代理配置
        let pac_content = pac_template
            .replace(
                "var proxy = \"SOCKS 127.0.0.1:1080\";",
                &format!("var proxy = \"PROXY 127.0.0.1:{}; SOCKS 127.0.0.1:{}\";", http_port, socks_port)
            );
        
        // 写入运行时PAC文件
        fs::write(pac_file_path, pac_content)
            .context("无法写入运行时PAC文件")?;
        
        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn unset_windows_proxy(&self) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let internet_settings = hkcu
            .open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings", KEY_WRITE)
            .context("无法打开注册表项")?;

        // 禁用代理
        internet_settings
            .set_value("ProxyEnable", &0u32)
            .context("无法设置 ProxyEnable")?;

        // 清除代理服务器设置
        internet_settings
            .set_value("ProxyServer", &"")
            .context("无法清除 ProxyServer")?;

        // 清除PAC脚本设置
        internet_settings
            .set_value("AutoConfigURL", &"")
            .context("无法清除 AutoConfigURL")?;

        // 关闭自动检测
        internet_settings
            .set_value("AutoDetect", &0u32)
            .context("无法设置 AutoDetect")?;

        // 刷新系统设置
        self.refresh_windows_proxy_settings().await?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn refresh_windows_proxy_settings(&self) -> Result<()> {
        // 方法1: 使用 Windows API 通知系统设置变更
        #[cfg(target_os = "windows")]
        {
            use std::ptr;
            
            // 使用 InternetSetOption 刷新代理设置
            // 这是参考 Privoxy 和其他代理软件的标准做法
            unsafe {
                // 定义 Windows API 常量
                const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
                const INTERNET_OPTION_REFRESH: u32 = 37;
                
                // 加载 wininet.dll
                let wininet = libloading::Library::new("wininet.dll")
                    .context("无法加载 wininet.dll")?;
                
                // 获取 InternetSetOption 函数
                let internet_set_option: libloading::Symbol<unsafe extern "system" fn(
                    hinternet: *mut std::ffi::c_void,
                    dwoption: u32,
                    lpbuffer: *const std::ffi::c_void,
                    dwbufferlength: u32,
                ) -> i32> = wininet.get(b"InternetSetOptionA")
                    .context("无法获取 InternetSetOptionA 函数")?;
                
                // 通知系统设置已更改
                internet_set_option(ptr::null_mut(), INTERNET_OPTION_SETTINGS_CHANGED, ptr::null(), 0);
                
                // 刷新设置
                internet_set_option(ptr::null_mut(), INTERNET_OPTION_REFRESH, ptr::null(), 0);
            }
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn get_windows_proxy_status(&self) -> Result<serde_json::Value> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let internet_settings = hkcu
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
            .context("无法打开注册表项")?;

        // 读取代理启用状态
        let proxy_enable: u32 = internet_settings
            .get_value("ProxyEnable")
            .unwrap_or(0);

        let is_enabled = proxy_enable == 1;

        if is_enabled {
            // 读取代理服务器设置
            let proxy_server: String = internet_settings
                .get_value("ProxyServer")
                .unwrap_or_default();

            let proxy_override: String = internet_settings
                .get_value("ProxyOverride")
                .unwrap_or_default();

            let auto_detect: u32 = internet_settings
                .get_value("AutoDetect")
                .unwrap_or(0);

            let auto_config_url: String = internet_settings
                .get_value("AutoConfigURL")
                .unwrap_or_default();

            Ok(serde_json::json!({
                "enabled": true,
                "proxy_server": proxy_server,
                "proxy_override": proxy_override,
                "auto_detect": auto_detect == 1,
                "auto_config_url": auto_config_url,
                "type": if proxy_server.contains("socks=") { "socks" } else { "http" }
            }))
        } else {
            Ok(serde_json::json!({
                "enabled": false,
                "proxy_server": "",
                "proxy_override": "",
                "auto_detect": false,
                "auto_config_url": "",
                "type": "none"
            }))
        }
    }

    #[cfg(target_os = "macos")]
    async fn set_macos_proxy(&self, proxy_url: &str) -> Result<()> {
        // 解析代理 URL
        let url = url::Url::parse(proxy_url)
            .context("无法解析代理 URL")?;
        
        let host = url.host_str().context("无法获取代理主机")?;
        let port = url.port().context("无法获取代理端口")?;

        // 获取网络服务列表
        let output = Command::new("networksetup")
            .args(&["-listallnetworkservices"])
            .output()
            .context("无法获取网络服务列表")?;

        let services = String::from_utf8_lossy(&output.stdout);
        
        for line in services.lines() {
            if line.starts_with("*") || line.trim().is_empty() {
                continue;
            }
            
            let service = line.trim();
            
            // 设置 HTTP 代理
            Command::new("networksetup")
                .args(&["-setwebproxy", service, host, &port.to_string()])
                .output()
                .context("无法设置 HTTP 代理")?;
            
            // 设置 HTTPS 代理
            Command::new("networksetup")
                .args(&["-setsecurewebproxy", service, host, &port.to_string()])
                .output()
                .context("无法设置 HTTPS 代理")?;
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn unset_macos_proxy(&self) -> Result<()> {
        // 获取网络服务列表
        let output = Command::new("networksetup")
            .args(&["-listallnetworkservices"])
            .output()
            .context("无法获取网络服务列表")?;

        let services = String::from_utf8_lossy(&output.stdout);
        
        for line in services.lines() {
            if line.starts_with("*") || line.trim().is_empty() {
                continue;
            }
            
            let service = line.trim();
            
            // 禁用 HTTP 代理
            Command::new("networksetup")
                .args(&["-setwebproxystate", service, "off"])
                .output()
                .context("无法禁用 HTTP 代理")?;
            
            // 禁用 HTTPS 代理
            Command::new("networksetup")
                .args(&["-setsecurewebproxystate", service, "off"])
                .output()
                .context("无法禁用 HTTPS 代理")?;
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn set_linux_proxy(&self, proxy_url: &str) -> Result<()> {
        // 在 Linux 上设置环境变量
        std::env::set_var("http_proxy", proxy_url);
        std::env::set_var("https_proxy", proxy_url);
        std::env::set_var("HTTP_PROXY", proxy_url);
        std::env::set_var("HTTPS_PROXY", proxy_url);
        
        // TODO: 根据不同的桌面环境设置系统代理
        // 这里可以添加对 GNOME、KDE 等桌面环境的支持
        
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn get_macos_proxy_status(&self) -> Result<serde_json::Value> {
        // 获取网络服务列表
        let output = Command::new("networksetup")
            .args(&["-listallnetworkservices"])
            .output()
            .context("无法获取网络服务列表")?;

        let services = String::from_utf8_lossy(&output.stdout);
        let mut proxy_info = serde_json::json!({
            "enabled": false,
            "http_proxy": "",
            "https_proxy": "",
            "type": "none"
        });

        for line in services.lines() {
            if line.starts_with("*") || line.trim().is_empty() {
                continue;
            }
            
            let service = line.trim();
            
            // 检查 HTTP 代理状态
            let http_output = Command::new("networksetup")
                .args(&["-getwebproxy", service])
                .output();
            
            if let Ok(output) = http_output {
                let result = String::from_utf8_lossy(&output.stdout);
                if result.contains("Enabled: Yes") {
                    proxy_info["enabled"] = serde_json::Value::Bool(true);
                    proxy_info["type"] = serde_json::Value::String("http".to_string());
                    
                    // 提取代理服务器信息
                    for line in result.lines() {
                        if line.starts_with("Server:") {
                            let server = line.replace("Server:", "").trim().to_string();
                            proxy_info["http_proxy"] = serde_json::Value::String(server);
                        }
                    }
                    break;
                }
            }
        }

        Ok(proxy_info)
    }

    #[cfg(target_os = "linux")]
    async fn unset_linux_proxy(&self) -> Result<()> {
        // 在 Linux 上清除环境变量
        std::env::remove_var("http_proxy");
        std::env::remove_var("https_proxy");
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn get_linux_proxy_status(&self) -> Result<serde_json::Value> {
        // 检查环境变量
        let http_proxy = std::env::var("http_proxy").or_else(|_| std::env::var("HTTP_PROXY"));
        let https_proxy = std::env::var("https_proxy").or_else(|_| std::env::var("HTTPS_PROXY"));

        let has_proxy = http_proxy.is_ok() || https_proxy.is_ok();

        Ok(serde_json::json!({
            "enabled": has_proxy,
            "http_proxy": http_proxy.unwrap_or_default(),
            "https_proxy": https_proxy.unwrap_or_default(),
            "type": if has_proxy { "http" } else { "none" }
        }))
    }
}