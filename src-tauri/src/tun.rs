/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, path::BaseDirectory};



// 导入日志宏
use crate::{log_info, log_warn, log_error, log_debug};
use crate::proxy::ProxyManager;
use crate::tun2proxy_ffi::{self, Tun2proxyDns, Tun2proxyVerbosity};

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;



#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

// Windows相关导入已移除，因为不再使用进程管理



/// 为 gateway 字段提供默认值
fn default_gateway() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 55, 1))
}

/// 默认DNS服务器
fn default_dns_server() -> String {
    "8.8.8.8".to_string()
}

/// 默认FakeIP起始地址
fn default_fake_ip_start() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1))
}

/// 默认FakeIP结束地址
fn default_fake_ip_end() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(198, 18, 255, 254))
}

/// TUN设备配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunConfig {
    /// 虚拟网卡名称
    pub name: String,
    /// IP地址
    pub address: IpAddr,
    /// 子网掩码
    pub netmask: IpAddr,
    /// 网关地址
    #[serde(default = "default_gateway")]
    pub gateway: IpAddr,
    /// MTU大小
    pub mtu: u16,
    /// 是否启用
    pub enabled: bool,

    /// 自定义DNS服务器地址
    #[serde(default = "default_dns_server")]
    pub dns_server: String,
    /// FakeIP模式：为域名分配虚假IP地址，实现DNS劫持和流量重定向
    #[serde(default)]
    pub fake_ip: bool,
    /// FakeIP地址池范围起始地址
    #[serde(default = "default_fake_ip_start")]
    pub fake_ip_start: IpAddr,
    /// FakeIP地址池范围结束地址
    #[serde(default = "default_fake_ip_end")]
    pub fake_ip_end: IpAddr,
    /// 路由绕过IP地址列表
    #[serde(default, rename = "bypassIps")]
    pub bypass_ips: Vec<String>,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            name: "ruray-tun".to_string(),
            address: IpAddr::V4(Ipv4Addr::new(192, 168, 55, 1)),  // 虚拟网卡IP设为网关地址
            netmask: IpAddr::V4(Ipv4Addr::new(255, 255, 255, 252)), // 使用/30子网掩码，参考sing-box
            gateway: IpAddr::V4(Ipv4Addr::new(192, 168, 55, 1)),
            mtu: 1500,
            enabled: false,
            dns_server: default_dns_server(),  // 默认DNS服务器
            fake_ip: false,      // 默认不启用FakeIP模式
            fake_ip_start: default_fake_ip_start(),  // FakeIP起始地址
            fake_ip_end: default_fake_ip_end(),      // FakeIP结束地址
            bypass_ips: Vec::new(),
        }
    }
}

/// TUN设备状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunStatus {
    /// 是否运行中
    pub is_running: bool,
    /// 设备名称
    pub device_name: String,
    /// IP地址
    pub ip_address: String,
    /// 接收字节数
    pub bytes_received: u64,
    /// 发送字节数
    pub bytes_sent: u64,
    /// 错误信息
    pub error: Option<String>,
    /// tun2proxy进程ID
    pub process_id: Option<u32>,
}

/// TUN设备管理器
pub struct TunManager {
    config: Mutex<TunConfig>,
    status: Mutex<TunStatus>,
    running: AtomicBool,
    app_handle: Mutex<Option<AppHandle>>,
}

// 全局单例实例
static TUN_MANAGER: OnceLock<TunManager> = OnceLock::new();

impl TunManager {
    /// 获取全局TUN管理器实例（单例模式）
    pub fn instance() -> &'static TunManager {
        TUN_MANAGER.get_or_init(|| {
            Self {
                config: Mutex::new(TunConfig::default()),
                status: Mutex::new(TunStatus {
                    is_running: false,
                    device_name: String::new(),
                    ip_address: String::new(),
                    bytes_received: 0,
                    bytes_sent: 0,
                    error: None,
                    process_id: None,
                }),
                running: AtomicBool::new(false),
                app_handle: Mutex::new(None),
            }
        })
    }

    /// 设置应用句柄
    /// 
    /// # Arguments
    /// 
    /// * `handle` - Tauri应用句柄
    pub fn set_app_handle(&self, handle: AppHandle) {
        let mut app_handle = self.app_handle.lock().unwrap();
        *app_handle = Some(handle);
    }

    /// 初始化WinTun库路径（仅Windows平台）
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 初始化结果
    #[cfg(target_os = "windows")]
    fn init_wintun_path(&self) -> Result<()> {
        let binding = self.app_handle.lock().unwrap();
        let app_handle = binding
            .as_ref()
            .context("应用句柄未设置，请先调用 set_app_handle")?;
        
        // 使用tun2proxy目录下的wintun.dll
        let wintun_resource_path = "tun2proxy/wintun.dll";
        
        // 使用Tauri的路径解析API获取资源文件路径
        match app_handle.path().resolve(wintun_resource_path, BaseDirectory::Resource) {
            Ok(wintun_path) => {
                if wintun_path.exists() {
                    // 将wintun.dll所在目录添加到DLL搜索路径
                    let dll_dir = wintun_path.parent().unwrap();
                    let dll_dir_wide: Vec<u16> = dll_dir.as_os_str()
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    
                    unsafe {
                        use windows_sys::Win32::System::LibraryLoader::SetDllDirectoryW;
                        if SetDllDirectoryW(dll_dir_wide.as_ptr()) != 0 {
                            log_info!("WinTun库路径已设置: {}", wintun_path.display());
                            return Ok(());
                        } else {
                            return Err(anyhow::anyhow!("设置DLL搜索目录失败"));
                        }
                    }
                } else {
                    log_warn!("警告: 嵌入的wintun.dll文件不存在: {}", wintun_path.display());
                }
            }
            Err(e) => {
                log_warn!("警告: 无法解析wintun.dll资源路径: {}", e);
            }
        }
        
        log_info!("使用系统默认WinTun路径");
        Ok(())
    }
    
    #[cfg(not(target_os = "windows"))]
    fn init_wintun_path(&self) -> Result<()> {
        log_info!("非Windows系统，跳过WinTun路径设置");
        Ok(())
    }

    /// 初始化tun2proxy DLL路径
    /// 
    /// # 返回值
    /// * `Result<()>` - 初始化结果
    #[cfg(target_os = "windows")]
    fn init_tun2proxy_dll(&self) -> Result<()> {
        let binding = self.app_handle.lock().unwrap();
        let app_handle = binding
            .as_ref()
            .context("应用句柄未设置，请先调用 set_app_handle")?;
        
        // 使用tun2proxy目录下的tun2proxy.dll
        let tun2proxy_resource_path = "tun2proxy/tun2proxy.dll";
        
        log_debug!("开始初始化tun2proxy DLL...");
        
        // 使用Tauri的路径解析API获取资源文件路径
        match app_handle.path().resolve(tun2proxy_resource_path, BaseDirectory::Resource) {
            Ok(dll_path) => {
                log_debug!("解析到tun2proxy.dll路径: {}", dll_path.display());
                if dll_path.exists() {
                    log_debug!("找到tun2proxy.dll文件: {}", dll_path.display());
                    
                    // 初始化tun2proxy DLL
                    match tun2proxy_ffi::init_tun2proxy_dll(dll_path.clone()) {
                        Ok(_) => {
                            log_debug!("tun2proxy DLL初始化成功");
                            return Ok(());
                        }
                        Err(e) => {
                            log_debug!("使用资源路径初始化tun2proxy DLL失败: {}", e);
                        }
                    }
                } else {
                    log_debug!("警告: 嵌入的tun2proxy.dll文件不存在: {}", dll_path.display());
                }
            }
            Err(e) => {
                log_debug!("警告: 无法解析tun2proxy.dll资源路径: {}", e);
            }
        }
        
        // 如果资源路径解析失败，尝试使用旧的方法
        log_debug!("尝试使用程序目录下的tun2proxy.dll");
        match self.get_tun2proxy_path() {
            Ok(tun2proxy_bin_path) => {
                let dll_path = tun2proxy_bin_path.parent()
                    .ok_or_else(|| anyhow::anyhow!("无法获取tun2proxy目录"))?
                    .join("tun2proxy.dll");
                
                log_info!("尝试加载DLL文件: {}", dll_path.display());
                
                if dll_path.exists() {
                    match tun2proxy_ffi::init_tun2proxy_dll(dll_path.clone()) {
                        Ok(_) => {
                            log_debug!("使用程序目录下的tun2proxy DLL初始化成功");
                            return Ok(());
                        }
                        Err(e) => {
                            log_debug!("使用程序目录初始化tun2proxy DLL失败: {}", e);
                            return Err(e.context("初始化tun2proxy DLL失败"));
                        }
                    }
                } else {
                    log_debug!("tun2proxy.dll文件不存在: {}", dll_path.display());
                    return Err(anyhow::anyhow!("tun2proxy.dll文件不存在: {}", dll_path.display()));
                }
            }
            Err(e) => {
                log_debug!("获取tun2proxy路径失败: {}", e);
                return Err(e.context("获取tun2proxy路径失败"));
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn init_tun2proxy_dll(&self) -> Result<()> {
        log_debug!("非Windows系统，跳过tun2proxy DLL初始化");
        Ok(())
    }

    /// 检查管理员权限
    /// 
    /// # Returns
    /// 
    /// * `bool` - 是否具有管理员权限
    #[cfg(target_os = "windows")]
    fn is_admin() -> bool {
        unsafe {
            let mut token = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            
            let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut size = 0;
            
            let result = GetTokenInformation(
                token,
                TokenElevation,
                &mut elevation as *mut _ as *mut _,
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            );
            
            if result != 0 {
                elevation.TokenIsElevated != 0
            } else {
                false
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    fn is_admin() -> bool {
        // 非Windows平台检查是否为root用户
        unsafe { libc::geteuid() == 0 }
    }

    /// 获取tun2proxy可执行文件路径
    /// 
    /// # Returns
    /// 
    /// * `Result<std::path::PathBuf>` - tun2proxy可执行文件路径
    fn get_tun2proxy_path(&self) -> Result<std::path::PathBuf> {
        let app_handle = self.app_handle.lock().unwrap()
            .as_ref()
            .context("应用句柄未设置，请先调用 set_app_handle")?
            .clone();
        
        let tun2proxy_resource_path = "tun2proxy/tun2proxy-bin.exe";
        
        let tun2proxy_path = app_handle.path().resolve(tun2proxy_resource_path, BaseDirectory::Resource)
            .context("无法解析tun2proxy可执行文件路径")?;
        
        if !tun2proxy_path.exists() {
            return Err(anyhow::anyhow!("tun2proxy可执行文件不存在: {}", tun2proxy_path.display()));
        }
        
        Ok(tun2proxy_path)
    }

    /// 验证IP地址或CIDR格式是否有效
    /// 
    /// # 参数
    /// * `addr` - 要验证的地址字符串
    /// 
    /// # 返回值
    /// * `bool` - 地址格式是否有效
    fn is_valid_ip_or_cidr(addr: &str) -> bool {
        // 检查是否为CIDR格式
        if addr.contains('/') {
            if let Some((ip_part, mask_part)) = addr.split_once('/') {
                // 验证IP部分
                if ip_part.parse::<IpAddr>().is_err() {
                    return false;
                }
                // 验证掩码部分
                if let Ok(mask) = mask_part.parse::<u8>() {
                    // IPv4掩码范围: 0-32, IPv6掩码范围: 0-128
                    if ip_part.parse::<Ipv4Addr>().is_ok() {
                        return mask <= 32;
                    } else if ip_part.parse::<Ipv6Addr>().is_ok() {
                        return mask <= 128;
                    }
                }
                return false;
            }
        } else {
            // 检查是否为有效的IP地址
            return addr.parse::<IpAddr>().is_ok();
        }
        false
    }
    
    /// 获取当前激活服务器的地址
    /// 
    /// # Returns
    /// 
    /// * `Result<String>` - 返回服务器地址，如果获取失败则返回错误
    fn get_current_server_address(&self) -> Result<String> {
        let proxy_manager = ProxyManager::instance();
        
        // 通过ProxyManager的公共方法获取当前服务器地址
        if let Some(address) = proxy_manager.get_current_server_address()? {
            Ok(address)
        } else {
            Err(anyhow::anyhow!("未找到当前激活的服务器"))
        }
    }

    /// 启动TUN设备
    /// 
    /// # 参数
    /// * `config` - TUN设备配置
    /// 
    /// # 返回值
    /// * `Result<()>` - 启动结果
    pub fn start(&self, config: TunConfig) -> Result<()> {
        // 检查管理员权限
        if !Self::is_admin() {
            return Err(anyhow::anyhow!("TUN_ERROR_ADMIN"));
        }

        // 检查代理服务器是否正在运行并获取代理地址
        let proxy_manager = ProxyManager::instance();
        let proxy_urls = proxy_manager.get_proxy_urls()
            .context("获取代理服务器地址失败")?;
        
        let proxy_url = match proxy_urls {
            Some((socks_url, _http_url)) => {
                log_info!("检测到代理服务器正在运行，使用SOCKS代理: {}", socks_url);
                socks_url
            }
            None => {
                return Err(anyhow::anyhow!("TUN_ERROR_NO_PROXY"));
            }
        };

        // 初始化WinTun库路径
        self.init_wintun_path()?;

        // 初始化tun2proxy DLL
        self.init_tun2proxy_dll()?;

        // 获取当前激活服务器的地址信息
        let server_address = self.get_current_server_address()
            .context("获取当前服务器地址失败")?;
        
        log_info!("TUN设备启动中...");
        
        // 构建绕过地址列表
        let mut bypass_addresses = Vec::new();
        
        // 验证并添加服务器地址
        if Self::is_valid_ip_or_cidr(&server_address) {
            bypass_addresses.push(server_address.clone());
            log_info!("添加服务器地址到绕过列表: {}", server_address);
        } else {
            log_warn!("服务器地址格式无效，跳过: {} (可能是域名，需要IP地址)", server_address);
        }
        
        // 验证并添加配置中的绕过IP
        for bypass_ip in &config.bypass_ips {
            let ip = bypass_ip.trim();
            if !ip.is_empty() {
                if Self::is_valid_ip_or_cidr(ip) {
                    bypass_addresses.push(ip.to_string());
                    log_info!("添加绕过IP: {}", ip);
                } else {
                    log_warn!("绕过IP格式无效，跳过: {}", ip);
                }
            }
        }
        
        log_debug!("启动tun2proxy DLL，TUN设备: {}, 代理: {}, 绕过地址: {:?}", 
                 config.name, proxy_url, bypass_addresses);
        
        // 构造命令行参数，支持多个绕过地址
        let mut cli_args = vec![
            "tun2proxy".to_string(),
            "--setup".to_string(),
            "--proxy".to_string(),
            proxy_url.clone(),
            "--tun".to_string(),
            config.name.clone(),
            "--dns".to_string(),
            "over-tcp".to_string(),
            "--verbosity".to_string(),
            "error".to_string(),
        ];
        
        // 添加绕过地址参数
        for bypass_addr in &bypass_addresses {
            cli_args.push("--bypass".to_string());
            cli_args.push(bypass_addr.clone());
        }
        
        let cli_args_str = cli_args.join(" ");
        log_debug!("tun2proxy命令行参数: {}", cli_args_str);
        
        // 根据配置设置TUN日志回调
        if let Ok(app_config) = crate::config::AppConfig::load() {
            if app_config.tun_log_enabled {
                log_debug!("TUN日志已启用，设置日志回调");
                if let Err(e) = tun2proxy_ffi::set_log_callback() {
                    log_warn!("设置TUN日志回调失败: {}", e);
                }
            } else {
                log_debug!("TUN日志已禁用，跳过日志回调设置");
            }
        }
        
        // 更新状态
        {
            let mut current_config = self.config.lock().unwrap();
            *current_config = config.clone();
        }
        {
            let mut status = self.status.lock().unwrap();
            status.is_running = true;
            status.device_name = config.name.clone();
            status.ip_address = config.address.to_string();
            status.bytes_received = 0;
            status.bytes_sent = 0;
            status.error = None;
            status.process_id = None;
        }
        
        // 更新运行状态
        self.running.store(true, Ordering::SeqCst);
        
        // 使用panic捕获机制调用DLL函数
        let start_result = std::panic::catch_unwind(|| {
            tun2proxy_ffi::run_with_cli_args(
                &cli_args_str,
                config.mtu,
                false, // packet_information
            )
        });
        
        match start_result {
            Ok(Ok(exit_code)) => {
                log_debug!("tun2proxy启动成功，退出码: {}", exit_code);
                if exit_code != 0 {
                    {
                        let mut status = self.status.lock().unwrap();
                        status.error = Some(format!("tun2proxy退出，退出码: {}", exit_code));
                        status.is_running = false;
                    }
                    self.running.store(false, Ordering::SeqCst);
                    return Err(anyhow::anyhow!("tun2proxy启动失败，退出码: {}", exit_code));
                }
            }
            Ok(Err(e)) => {
                log_error!("tun2proxy启动失败: {}", e);
                {
                    let mut status = self.status.lock().unwrap();
                    status.error = Some(format!("启动失败: {}", e));
                    status.is_running = false;
                }
                self.running.store(false, Ordering::SeqCst);
                return Err(e);
            }
            Err(panic_info) => {
                log_error!("tun2proxy启动时发生panic: {:?}", panic_info);
                {
                    let mut status = self.status.lock().unwrap();
                    status.error = Some("启动时发生panic".to_string());
                    status.is_running = false;
                }
                self.running.store(false, Ordering::SeqCst);
                return Err(anyhow::anyhow!("tun2proxy启动时发生panic"));
            }
        }
        
        log_info!("TUN设备启动成功");
        Ok(())
    }

    /// 停止TUN设备
    /// 
    /// # 返回值
    /// * `Result<()>` - 停止结果
    pub fn stop(&self) -> Result<()> {
        log_info!("开始停止TUN设备");
        
        // 检查是否正在运行
        if !self.running.load(Ordering::SeqCst) {
            log_info!("TUN设备未运行，无需停止");
            return Ok(());
        }
        
        log_debug!("正在停止tun2proxy DLL");
        
        // 使用panic捕获机制调用DLL的stop函数
        let stop_result = std::panic::catch_unwind(|| {
            tun2proxy_ffi::stop()
        });
        
        match stop_result {
            Ok(Ok(exit_code)) => {
                if exit_code == 0 {
                    log_debug!("tun2proxy DLL已成功停止");
                } else {
                    log_debug!("tun2proxy DLL停止时返回非零退出码: {}", exit_code);
                }
            }
            Ok(Err(e)) => {
                log_debug!("停止tun2proxy DLL失败: {}", e);
            }
            Err(panic_info) => {
                log_error!("停止tun2proxy DLL时发生panic: {:?}", panic_info);
            }
        }
        
        // 更新状态
        {
            let mut status = self.status.lock().unwrap();
            status.is_running = false;
            status.error = None;
            status.process_id = None;
        }
        
        // 更新运行状态
        self.running.store(false, Ordering::SeqCst);
        
        log_info!("TUN设备已停止");
        Ok(())
    }

    /// 停止TUN设备（同步版本，用于应用关闭时调用）
    /// 
    /// # 返回值
    /// * `Result<()>` - 停止结果
    pub fn stop_sync(&self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        // 设置停止标志
        self.running.store(false, Ordering::SeqCst);

        log_info!("正在同步停止tun2proxy DLL");
        
        // 使用panic捕获机制调用DLL的stop函数
        let stop_result = std::panic::catch_unwind(|| {
            tun2proxy_ffi::stop()
        });
        
        match stop_result {
            Ok(Ok(exit_code)) => {
                if exit_code == 0 {
                    log_info!("tun2proxy DLL已成功停止（同步）");
                } else {
                    log_warn!("tun2proxy DLL停止时返回非零退出码（同步）: {}", exit_code);
                }
            }
            Ok(Err(e)) => {
                log_warn!("停止tun2proxy DLL失败（同步）: {}", e);
            }
            Err(panic_info) => {
                log_error!("停止tun2proxy DLL时发生panic（同步）: {:?}", panic_info);
            }
        }

        // 更新状态
        {
            let mut status = self.status.lock().unwrap();
            status.is_running = false;
            status.device_name.clear();
            status.ip_address.clear();
            status.process_id = None;
        }

        log_info!("TUN设备已停止（同步）");
        Ok(())
    }

    /// 检查TUN设备是否正在运行
    /// 
    /// # 返回值
    /// * `bool` - 是否运行中
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 检查TUN设备是否正在运行（同步版本）
    /// 
    /// # 返回值
    /// * `bool` - 是否运行中
    pub fn is_running_sync(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 获取TUN设备状态
    /// 
    /// # 返回值
    /// * `TunStatus` - 设备状态
    pub fn get_status(&self) -> TunStatus {
        self.status.lock().unwrap().clone()
    }

    /// 获取TUN设备配置
    /// 
    /// # 返回值
    /// * `TunConfig` - 设备配置
    pub fn get_config(&self) -> TunConfig {
        self.config.lock().unwrap().clone()
    }

    /// 更新TUN设备配置
    /// 
    /// # 参数
    /// * `config` - 新的配置
    /// 
    /// # 返回值
    /// * `Result<()>` - 更新结果
    pub fn update_config(&self, config: TunConfig) -> Result<()> {
        let was_running = self.is_running();
        
        if was_running {
            self.stop()?;
        }
        
        {
            let mut current_config = self.config.lock().unwrap();
            *current_config = config.clone();
        }
        
        if was_running && config.enabled {
            self.start(config)?;
        }
        
        Ok(())
    }

    /// 检查tun2proxy DLL状态
    /// 
    /// # 返回值
    /// * `bool` - DLL是否仍在运行
    pub fn check_process_status(&self) -> bool {
        // 在DLL模式下，直接返回运行状态
        self.running.load(Ordering::SeqCst)
    }
}

/// 在应用关闭时清理TUN设备
impl Drop for TunManager {
    fn drop(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            if let Err(e) = self.stop_sync() {
                log_error!("清理TUN设备时出错: {}", e);
            }
        }
    }
}