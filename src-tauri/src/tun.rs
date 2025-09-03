/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, path::BaseDirectory};
use tokio::process::{Child, Command};
use std::process::Stdio;
use std::fs::OpenOptions;


// 导入日志宏
use crate::{log_info, log_warn, log_error};
use crate::proxy::ProxyManager;

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;



#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Console::{GenerateConsoleCtrlEvent, CTRL_C_EVENT};



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
    config: Arc<Mutex<TunConfig>>,
    status: Arc<Mutex<TunStatus>>,
    running: Arc<AtomicBool>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    /// tun2proxy子进程
    tun2proxy_process: Arc<Mutex<Option<Child>>>,
}

// 全局单例实例
static TUN_MANAGER: OnceLock<TunManager> = OnceLock::new();

impl TunManager {
    /// 获取全局TUN管理器实例（单例模式）
    pub fn instance() -> &'static TunManager {
        TUN_MANAGER.get_or_init(|| {
            Self {
                config: Arc::new(Mutex::new(TunConfig::default())),
                status: Arc::new(Mutex::new(TunStatus {
                    is_running: false,
                    device_name: String::new(),
                    ip_address: String::new(),
                    bytes_received: 0,
                    bytes_sent: 0,
                    error: None,
                    process_id: None,
                })),
                running: Arc::new(AtomicBool::new(false)),
                app_handle: Arc::new(Mutex::new(None)),
                tun2proxy_process: Arc::new(Mutex::new(None)),
            }
        })
    }

    /// 设置应用句柄
    /// 
    /// # Arguments
    /// 
    /// * `handle` - Tauri应用句柄
    pub fn set_app_handle(&self, handle: AppHandle) {
        let mut app_handle_guard = self.app_handle.lock().unwrap();
        *app_handle_guard = Some(handle);
    }

    /// 初始化WinTun库路径（仅Windows平台）
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 初始化结果
    #[cfg(target_os = "windows")]
    fn init_wintun_path(&self) -> Result<()> {
        let app_handle_guard = self.app_handle.lock().unwrap();
        let app_handle = app_handle_guard.as_ref()
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
        // 非Windows平台不需要WinTun
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
        let app_handle_guard = self.app_handle.lock().unwrap();
        let app_handle = app_handle_guard.as_ref()
            .context("应用句柄未设置，请先调用 set_app_handle")?;
        
        let tun2proxy_resource_path = "tun2proxy/tun2proxy-bin.exe";
        
        let tun2proxy_path = app_handle.path().resolve(tun2proxy_resource_path, BaseDirectory::Resource)
            .context("无法解析tun2proxy可执行文件路径")?;
        
        if !tun2proxy_path.exists() {
            return Err(anyhow::anyhow!("tun2proxy可执行文件不存在: {}", tun2proxy_path.display()));
        }
        
        Ok(tun2proxy_path)
    }

    /// 获取当前激活服务器的地址
    /// 
    /// # Returns
    /// 
    /// * `Result<String>` - 返回服务器地址，如果获取失败则返回错误
    async fn get_current_server_address(&self) -> Result<String> {
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
    pub async fn start(&self, config: TunConfig) -> Result<()> {
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

        // 如果已经在运行，先停止
        if self.is_running().await {
            self.stop().await?;
        }

        // 更新配置
        {
            let mut current_config = self.config.lock().unwrap();
            *current_config = config.clone();
        }

        // 获取tun2proxy可执行文件路径
        let tun2proxy_path = self.get_tun2proxy_path()?;
        
        log_info!("启动tun2proxy: {}", tun2proxy_path.display());
        
        // 获取当前激活服务器的地址信息
        let server_address = self.get_current_server_address().await
            .context("获取当前服务器地址失败")?;
        
        // 获取程序运行目录并创建日志文件路径
        let app_dir = std::env::current_exe()
            .context("获取程序路径失败")?
            .parent()
            .context("获取程序目录失败")?
            .to_path_buf();
        let log_file_path = app_dir.join("tun.log");
        
        // 创建或打开日志文件
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .context("创建tun日志文件失败")?;
        
        log_info!("tun2proxy日志将输出到: {}", log_file_path.display());
        
        // 构建tun2proxy命令参数
        let mut cmd = Command::new(&tun2proxy_path);
        cmd.arg("--setup")  // 启动程序
            .arg("--dns")   // 设置DNS处理方式
            .arg("over-tcp") // 使用TCP方式处理DNS
            .arg("--tun")   // 指定TUN网卡名称
            .arg(&config.name)
            .arg("--proxy") // 指定代理服务器
            .arg(&proxy_url)
            .arg("--bypass") // 绕过代理的地址
            .arg(&server_address) // 当前激活服务器的IP地址
            .stdout(Stdio::from(log_file.try_clone().context("克隆日志文件句柄失败")?))
            .stderr(Stdio::from(log_file));
        
        // 添加用户配置的绕过IP地址
        for bypass_ip in &config.bypass_ips {
            if !bypass_ip.trim().is_empty() {
                cmd.arg("--bypass").arg(bypass_ip.trim());
            }
        }
        
        // Windows平台下隐藏命令行窗口
        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        // 打印完整的命令行参数用于调试
        let args: Vec<String> = std::iter::once(tun2proxy_path.to_string_lossy().to_string())
            .chain(cmd.as_std().get_args().map(|arg| arg.to_string_lossy().to_string()))
            .collect();
        log_info!("tun2proxy命令行参数: {}", args.join(" "));
        
        // 启动tun2proxy进程
        let child = cmd.spawn()
            .context("启动tun2proxy进程失败")?;
        
        let process_id = child.id();
        log_info!("tun2proxy进程已启动，PID: {:?}", process_id);
        
        // 存储子进程引用
        {
            let mut process_guard = self.tun2proxy_process.lock().unwrap();
            *process_guard = Some(child);
        }

        // 更新状态
        {
            let mut status = self.status.lock().unwrap();
            status.is_running = true;
            status.device_name = config.name.clone();
            status.ip_address = config.address.to_string();
            status.bytes_received = 0;
            status.bytes_sent = 0;
            status.error = None;
            status.process_id = process_id;
        }

        // 更新运行状态
        self.running.store(true, Ordering::SeqCst);
        
        log_info!("TUN模式启动成功，使用tun2proxy代理，虚拟网卡: {}", config.name);
        Ok(())
    }

    /// 停止TUN设备
    /// 
    /// # 返回值
    /// * `Result<()>` - 停止结果
    pub async fn stop(&self) -> Result<()> {
        // 检查是否在运行
        if !self.is_running().await {
            return Ok(()); // 已经停止
        }

        // 停止运行状态
        self.running.store(false, Ordering::SeqCst);

        // 停止tun2proxy进程
        let child_opt = {
            let mut process_guard = self.tun2proxy_process.lock().unwrap();
            process_guard.take()
        };
        
        if let Some(mut child) = child_opt {
            log_info!("正在停止tun2proxy进程，PID: {:?}", child.id());
            
            // 尝试发送Ctrl+C信号优雅停止进程
            let pid = child.id().unwrap_or(0);
            let mut graceful_stop = false;
            
            #[cfg(target_os = "windows")]
            {
                // 在Windows上发送Ctrl+C事件
                unsafe {
                    let result = GenerateConsoleCtrlEvent(CTRL_C_EVENT, pid);
                    if result != 0 {
                        log_info!("已向tun2proxy进程发送Ctrl+C信号，PID: {}", pid);
                        graceful_stop = true;
                        
                        // 等待进程优雅退出（最多等待5秒）
                        let mut wait_count = 0;
                        while wait_count < 50 {
                            match child.try_wait() {
                                Ok(Some(_)) => {
                                    log_info!("tun2proxy进程已优雅停止");
                                    break;
                                }
                                Ok(None) => {
                                    // 进程仍在运行，继续等待
                                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                    wait_count += 1;
                                }
                                Err(e) => {
                                    log_warn!("检查进程状态失败: {}", e);
                                    break;
                                }
                            }
                        }
                        
                        if wait_count >= 50 {
                            log_warn!("进程未在预期时间内退出，将强制终止");
                            graceful_stop = false;
                        }
                    } else {
                        log_warn!("发送Ctrl+C信号失败，将使用强制终止");
                    }
                }
            }
            
            // 如果优雅停止失败，则强制终止
            if !graceful_stop {
                if let Err(e) = child.kill().await {
                    log_warn!("强制终止tun2proxy进程失败: {}", e);
                } else {
                    // 等待进程退出
                    if let Err(e) = child.wait().await {
                        log_warn!("等待tun2proxy进程退出失败: {}", e);
                    } else {
                        log_info!("tun2proxy进程已强制停止");
                    }
                }
            }
        }

        // 更新状态
        {
            let mut status = self.status.lock().unwrap();
            status.is_running = false;
            status.error = None;
            status.process_id = None;
        }

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

        // 停止tun2proxy进程
        {
            let mut process_guard = self.tun2proxy_process.lock().unwrap();
            if let Some(mut child) = process_guard.take() {
                log_info!("正在同步停止tun2proxy进程，PID: {:?}", child.id());
                
                // 尝试发送Ctrl+C信号优雅停止进程
                let pid = child.id().unwrap_or(0);
                let mut graceful_stop = false;
                
                #[cfg(target_os = "windows")]
                {
                    // 在Windows上发送Ctrl+C事件
                    unsafe {
                        let result = GenerateConsoleCtrlEvent(CTRL_C_EVENT, pid);
                        if result != 0 {
                            log_info!("已向tun2proxy进程发送Ctrl+C信号（同步），PID: {}", pid);
                            graceful_stop = true;
                            
                            // 在同步上下文中等待一段时间
                            std::thread::sleep(std::time::Duration::from_millis(2000));
                            
                            // 检查进程是否已退出
                            match child.try_wait() {
                                Ok(Some(_)) => {
                                    log_info!("tun2proxy进程已优雅停止（同步）");
                                }
                                Ok(None) => {
                                    log_warn!("进程未在预期时间内退出，将强制终止（同步）");
                                    graceful_stop = false;
                                }
                                Err(e) => {
                                    log_warn!("检查进程状态失败（同步）: {}", e);
                                    graceful_stop = false;
                                }
                            }
                        } else {
                            log_warn!("发送Ctrl+C信号失败，将使用强制终止（同步）");
                        }
                    }
                }
                
                // 如果优雅停止失败，则强制终止
                if !graceful_stop {
                    if let Err(e) = child.start_kill() {
                        log_warn!("强制终止tun2proxy进程失败（同步）: {}", e);
                    } else {
                        log_info!("tun2proxy进程强制终止信号已发送（同步）");
                    }
                }
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
    pub async fn is_running(&self) -> bool {
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
    pub async fn get_status(&self) -> TunStatus {
        let status = self.status.lock().unwrap();
        status.clone()
    }

    /// 获取TUN设备配置
    /// 
    /// # 返回值
    /// * `TunConfig` - 设备配置
    pub async fn get_config(&self) -> TunConfig {
        let config = self.config.lock().unwrap();
        config.clone()
    }

    /// 更新TUN设备配置
    /// 
    /// # 参数
    /// * `config` - 新的配置
    /// 
    /// # 返回值
    /// * `Result<()>` - 更新结果
    pub async fn update_config(&self, config: TunConfig) -> Result<()> {
        let was_running = self.is_running().await;
        
        if was_running {
            self.stop().await?;
        }
        
        {
            let mut current_config = self.config.lock().unwrap();
            *current_config = config.clone();
        }
        
        if was_running && config.enabled {
            self.start(config).await?;
        }
        
        Ok(())
    }

    /// 检查tun2proxy进程状态
    /// 
    /// # 返回值
    /// * `bool` - 进程是否仍在运行
    pub async fn check_process_status(&self) -> bool {
        let process_status = {
            let mut process_guard = self.tun2proxy_process.lock().unwrap();
            if let Some(child) = process_guard.as_mut() {
                child.try_wait()
            } else {
                return false;
            }
        };
        
        match process_status {
            Ok(Some(status)) => {
                log_warn!("tun2proxy进程已退出，状态: {:?}", status);
                // 进程已退出，更新状态
                self.running.store(false, Ordering::SeqCst);
                {
                    let mut status_guard = self.status.lock().unwrap();
                    status_guard.is_running = false;
                    status_guard.process_id = None;
                }
                false
            }
            Ok(None) => {
                // 进程仍在运行
                true
            }
            Err(e) => {
                log_error!("检查tun2proxy进程状态失败: {}", e);
                false
            }
        }
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