/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use std::process::Command;

use crate::commands::SystemStats;

/// 系统管理器
pub struct SystemManager;

impl SystemManager {
    /// 创建新的系统管理器实例
    pub fn new() -> Self {
        Self
    }

    /// 获取系统统计信息
    pub async fn get_stats(&self) -> Result<SystemStats> {
        // TODO: 实现真实的系统统计信息获取
        // 这里暂时返回模拟数据
        Ok(SystemStats {
            cpu_usage: rand::random::<f32>() * 100.0,
            memory_usage: rand::random::<f32>() * 100.0,
            memory_total: 16 * 1024 * 1024 * 1024, // 16GB
            memory_used: (rand::random::<u64>() % 8) * 1024 * 1024 * 1024, // 0-8GB
            network_upload: rand::random::<u64>() % 1024 * 1024,
            network_download: rand::random::<u64>() % 1024 * 1024 * 10,
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
        self.refresh_windows_proxy_settings().await?;

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

        // 刷新系统设置
        self.refresh_windows_proxy_settings().await?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn refresh_windows_proxy_settings(&self) -> Result<()> {
        // 方法1: 使用 Windows API 通知系统设置变更
        #[cfg(target_os = "windows")]
        {
            use std::ffi::CString;
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

        // 方法2: 使用 PowerShell 作为备用方案
        let _output = Command::new("powershell")
            .args(&[
                "-Command",
                r#"
                # 刷新网络设置
                try {
                    # 通知系统代理设置已更改
                    Add-Type -TypeDefinition @'
                        using System;
                        using System.Runtime.InteropServices;
                        public class WinInet {
                            [DllImport("wininet.dll", SetLastError = true)]
                            public static extern bool InternetSetOption(IntPtr hInternet, int dwOption, IntPtr lpBuffer, int dwBufferLength);
                        }
'@
                    [WinInet]::InternetSetOption([IntPtr]::Zero, 39, [IntPtr]::Zero, 0) | Out-Null
                    [WinInet]::InternetSetOption([IntPtr]::Zero, 37, [IntPtr]::Zero, 0) | Out-Null
                } catch {
                    # 如果上述方法失败，使用简单的刷新
                    [System.Windows.Forms.Application]::DoEvents()
                }
                "#
            ])
            .output();

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