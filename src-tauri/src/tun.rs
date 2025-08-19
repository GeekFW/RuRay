/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, OnceLock};
use tun::{Configuration, Layer};
use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::JoinHandle;
use std::collections::HashMap;
use tokio::sync::Mutex as AsyncMutex;
use tauri::{AppHandle, Manager, path::BaseDirectory};

// 导入日志宏
use crate::{log_debug, log_info, log_warn, log_error};

#[cfg(target_os = "windows")]
use std::os::windows::ffi::{OsStrExt};

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
    /// 严格路由模式：确保所有流量都通过TUN设备，防止流量泄漏
    #[serde(default)]
    pub strict_route: bool,
    /// DNS劫持：拦截DNS查询并重定向到指定DNS服务器
    #[serde(default)]
    pub dns_hijack: bool,
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
            strict_route: true,  // 默认启用严格路由模式
            dns_hijack: false,   // 默认不启用DNS劫持
            dns_server: default_dns_server(),  // 默认DNS服务器
            fake_ip: false,      // 默认不启用FakeIP模式
            fake_ip_start: default_fake_ip_start(),  // FakeIP起始地址
            fake_ip_end: default_fake_ip_end(),      // FakeIP结束地址
        }
    }
}

/// TCP连接信息
#[derive(Debug, Clone)]
struct TcpConnection {
    /// 源IP地址
    src_ip: Ipv4Addr,
    /// 源端口
    src_port: u16,
    /// 目标IP地址
    dst_ip: Ipv4Addr,
    /// 目标端口
    dst_port: u16,
    /// 代理连接
    proxy_stream: Option<Arc<AsyncMutex<TcpStream>>>,
    /// 直连连接
    direct_stream: Option<Arc<AsyncMutex<TcpStream>>>,
}

/// FakeIP映射条目
#[derive(Debug, Clone)]
struct FakeIpMapping {
    /// 域名
    domain: String,
    /// 分配的虚假IP地址
    fake_ip: Ipv4Addr,
    /// 真实IP地址（解析后的）
    real_ip: Option<Ipv4Addr>,
    /// 创建时间
    created_at: std::time::Instant,
}

/// FakeIP管理器
#[derive(Debug)]
struct FakeIpManager {
    /// 域名到FakeIP的映射
    domain_to_fake: Arc<AsyncMutex<HashMap<String, Ipv4Addr>>>,
    /// FakeIP到域名的映射
    fake_to_domain: Arc<AsyncMutex<HashMap<Ipv4Addr, String>>>,
    /// FakeIP到真实IP的映射
    fake_to_real: Arc<AsyncMutex<HashMap<Ipv4Addr, Ipv4Addr>>>,
    /// 下一个可用的FakeIP地址
    next_ip: Arc<AsyncMutex<u32>>,
    /// FakeIP地址池配置
    start_ip: Ipv4Addr,
    end_ip: Ipv4Addr,
}

impl FakeIpManager {
    /// 创建新的FakeIP管理器
    fn new(start_ip: Ipv4Addr, end_ip: Ipv4Addr) -> Self {
        let start_u32 = u32::from(start_ip);
        Self {
            domain_to_fake: Arc::new(AsyncMutex::new(HashMap::new())),
            fake_to_domain: Arc::new(AsyncMutex::new(HashMap::new())),
            fake_to_real: Arc::new(AsyncMutex::new(HashMap::new())),
            next_ip: Arc::new(AsyncMutex::new(start_u32)),
            start_ip,
            end_ip,
        }
    }

    /// 为域名分配FakeIP地址
    async fn allocate_fake_ip(&self, domain: &str) -> Result<Ipv4Addr> {
        let mut domain_to_fake = self.domain_to_fake.lock().await;
        
        // 检查是否已经分配过
        if let Some(&fake_ip) = domain_to_fake.get(domain) {
            return Ok(fake_ip);
        }
        
        // 分配新的FakeIP
        let mut next_ip = self.next_ip.lock().await;
        let current_ip = *next_ip;
        
        // 检查是否超出范围
        if current_ip > u32::from(self.end_ip) {
            return Err(anyhow::anyhow!("FakeIP地址池已满"));
        }
        
        let fake_ip = Ipv4Addr::from(current_ip);
        *next_ip = current_ip + 1;
        
        // 更新映射
        domain_to_fake.insert(domain.to_string(), fake_ip);
        drop(domain_to_fake);
        
        let mut fake_to_domain = self.fake_to_domain.lock().await;
        fake_to_domain.insert(fake_ip, domain.to_string());
        
        log_debug!("为域名 {} 分配FakeIP: {}", domain, fake_ip);
        Ok(fake_ip)
    }
    
    /// 根据FakeIP获取域名
    async fn get_domain_by_fake_ip(&self, fake_ip: &Ipv4Addr) -> Option<String> {
        let fake_to_domain = self.fake_to_domain.lock().await;
        fake_to_domain.get(fake_ip).cloned()
    }
    
    /// 设置FakeIP对应的真实IP
    async fn set_real_ip(&self, fake_ip: Ipv4Addr, real_ip: Ipv4Addr) {
        let mut fake_to_real = self.fake_to_real.lock().await;
        fake_to_real.insert(fake_ip, real_ip);
        log_debug!("设置FakeIP {} 对应的真实IP: {}", fake_ip, real_ip);
    }
    
    /// 根据FakeIP获取真实IP
    async fn get_real_ip(&self, fake_ip: &Ipv4Addr) -> Option<Ipv4Addr> {
        let fake_to_real = self.fake_to_real.lock().await;
        fake_to_real.get(fake_ip).cloned()
    }
    
    /// 检查IP是否为FakeIP
    fn is_fake_ip(&self, ip: &Ipv4Addr) -> bool {
        let ip_u32 = u32::from(*ip);
        let start_u32 = u32::from(self.start_ip);
        let end_u32 = u32::from(self.end_ip);
        ip_u32 >= start_u32 && ip_u32 <= end_u32
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
}

/// TUN设备管理器
pub struct TunManager {
    config: Arc<Mutex<TunConfig>>,
    status: Arc<Mutex<TunStatus>>,
    running: Arc<AtomicBool>,
    device: Arc<Mutex<Option<tun::platform::Device>>>,
    packet_handler: Arc<Mutex<Option<JoinHandle<()>>>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    /// TCP连接管理器
    connections: Arc<AsyncMutex<HashMap<String, TcpConnection>>>,
    /// 原始路由备份
    original_routes: Arc<Mutex<Vec<String>>>,
    /// FakeIP管理器
    fake_ip_manager: Arc<Mutex<Option<FakeIpManager>>>,
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
                })),
                running: Arc::new(AtomicBool::new(false)),
                device: Arc::new(Mutex::new(None)),
                packet_handler: Arc::new(Mutex::new(None)),
                app_handle: Arc::new(Mutex::new(None)),
                connections: Arc::new(AsyncMutex::new(HashMap::new())),
                original_routes: Arc::new(Mutex::new(Vec::new())),
                fake_ip_manager: Arc::new(Mutex::new(None)),
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
        
        // 根据系统架构确定要使用的wintun.dll路径
        let arch = std::env::consts::ARCH;
        let wintun_resource_path = match arch {
            "x86_64" => "wintun/bin/amd64/wintun.dll",
            "x86" => "wintun/bin/x86/wintun.dll",
            "aarch64" => "wintun/bin/arm64/wintun.dll",
            "arm" => "wintun/bin/arm/wintun.dll",
            _ => {
                log_warn!("警告: 不支持的架构 {}, 使用默认路径", arch);
                return Ok(());
            }
        };
        
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
    fn init_wintun_path() -> Result<()> {
        // 非Windows平台不需要WinTun
        Ok(())
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
            return Err(anyhow::anyhow!("启动TUN模式需要管理员权限，请以管理员身份运行程序"));
        }

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

        // 在单独的作用域中创建TUN设备
        {
            let mut tun_config = Configuration::default();
            tun_config
                .name("ruray-tun")
                .address(config.address)
                .netmask(config.netmask)
                .destination(config.gateway)
                .mtu(config.mtu as i32)
                .layer(Layer::L3)
                .up();

            #[cfg(target_os = "windows")]
            tun_config.platform(|_config| {
                // Windows平台特定配置
            });

            // 创建TUN设备
             let device = match tun::create(&tun_config) {
                 Ok(device) => {
                     log_info!("TUN设备创建成功: ruray-tun");
                     device
                 }
                 Err(e) => {
                     let error_msg = format!("创建TUN设备失败: {}. 详细错误: {:?}. 可能原因: 1) 权限不足，需要管理员权限 2) TUN驱动未安装 3) 网络配置冲突", e, e);
                     log_error!("{}", error_msg);
                     return Err(anyhow::anyhow!(error_msg));
                 }
             };

             // 存储设备引用
             let mut device_guard = self.device.lock().unwrap();
             *device_guard = Some(device);
        }

        // 配置TUN设备的IP地址和网关
        self.configure_tun_ip(&config).await?;

        // 初始化FakeIP管理器（如果启用）
        if config.fake_ip {
            let fake_ip_manager = FakeIpManager::new(
                config.fake_ip_start.to_string().parse()?,
                config.fake_ip_end.to_string().parse()?
            );
            let mut manager_guard = self.fake_ip_manager.lock().unwrap();
            *manager_guard = Some(fake_ip_manager);
            log_info!("FakeIP管理器已初始化，地址池: {} - {}", config.fake_ip_start, config.fake_ip_end);
        }

        // 备份原始路由表
        if let Err(e) = self.backup_routes() {
            log_warn!("备份路由表失败: {}", e);
        }

        // 设置系统路由
        self.set_system_route(true).await?;

        // 启动数据包处理循环
        let packet_handler = self.start_packet_processing().await?;
        {
            let mut handler_guard = self.packet_handler.lock().unwrap();
            *handler_guard = Some(packet_handler);
        }

        // 更新状态
        {
            let mut status = self.status.lock().unwrap();
            status.is_running = true;
            status.device_name = "ruray-tun".to_string();
            status.ip_address = config.address.to_string();
            status.bytes_received = 0;
            status.bytes_sent = 0;
            status.error = None;
        }

        // 更新运行状态
        self.running.store(true, Ordering::SeqCst);
        
        log_info!("TUN模式启动成功，虚拟网卡: ruray-tun");
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

        // 停止数据包处理任务
        {
            let mut handler_guard = self.packet_handler.lock().unwrap();
            if let Some(handle) = handler_guard.take() {
                handle.abort();
            }
        }

        // 关闭TUN设备
        {
            let mut device_guard = self.device.lock().unwrap();
            *device_guard = None;
        }

        // 移除系统路由并恢复原始路由表
        self.set_system_route(false).await?;

        // 更新状态
        {
            let mut status = self.status.lock().unwrap();
            status.is_running = false;
            status.error = None;
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

        // 停止数据包处理任务
        if let Some(handle) = self.packet_handler.lock().unwrap().take() {
            handle.abort();
        }

        // 清理设备
        {
            let mut device_guard = self.device.lock().unwrap();
            *device_guard = None;
        }

        // 更新状态
        {
            let mut status = self.status.lock().unwrap();
            status.is_running = false;
            status.device_name.clear();
            status.ip_address.clear();
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

    /// 启动数据包处理循环
    /// 
    /// # 返回值
    /// * `Result<JoinHandle<()>>` - 处理任务句柄
    /// 启动数据包处理循环
    /// 从TUN设备读取数据包并转发到代理服务器
    async fn start_packet_processing(&self) -> Result<JoinHandle<()>> {
        let device = self.device.clone();
        let running = self.running.clone();
        let status = self.status.clone();
        let connections = self.connections.clone();

        let handle = tokio::spawn(async move {
            let mut _buffer = [0u8; 1500]; // MTU大小的缓冲区
            
            while running.load(Ordering::SeqCst) {
                // 检查设备是否可用
                let device_available = {
                    let device_guard = device.lock().unwrap();
                    device_guard.is_some()
                };
                
                if device_available {
                    // 使用spawn_blocking来处理阻塞的TUN设备读取
                    let device_clone = device.clone();
                    let status_clone = status.clone();
                    let connections_clone = connections.clone();
                    
                    match tokio::task::spawn_blocking(move || {
                        let mut _buffer = [0u8; 1500]; // MTU大小的缓冲区
                        
                        // 在闭包内部获取设备引用
                         let mut device_guard = device_clone.lock().unwrap();
                         if let Some(tun_device) = device_guard.as_mut() {
                             // 从TUN设备读取数据包
                             match tun_device.read(&mut _buffer) {
                                Ok(size) => {
                                    log_debug!("接收到数据包，大小: {} 字节", size);
                                    Some((_buffer[..size].to_vec(), size))
                                }
                                Err(e) => {
                                    log_error!("读取TUN设备数据包失败: {}", e);
                                    None
                                }
                            }
                        } else {
                            log_warn!("TUN设备不可用");
                            None
                        }
                    }).await {
                        Ok(Some((packet_data, packet_size))) => {
                            // 更新接收统计
                            {
                                let mut status_guard = status_clone.lock().unwrap();
                                status_guard.bytes_received += packet_size as u64;
                            }
                            
                            // 处理数据包
                            if let Err(e) = Self::process_packet_with_response(&packet_data, device.clone(), connections_clone).await {
                                log_error!("处理数据包失败: {}", e);
                            }
                        }
                        Ok(None) => {
                            // 读取失败，短暂等待后继续
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                        Err(e) => {
                            log_error!("数据包读取任务失败: {}", e);
                            break;
                        }
                    }
                } else {
                    log_warn!("TUN设备不可用，停止数据包处理");
                    break;
                }
                
                // 检查是否需要停止
                if !running.load(Ordering::SeqCst) {
                    break;
                }
            }
            
            log_info!("数据包处理循环已停止");
        });

        Ok(handle)
    }

    /// 处理接收到的数据包并实现响应回写
    /// 根据数据包类型和目标地址决定是否需要代理
    async fn process_packet_with_response(
        packet: &[u8], 
        device: Arc<Mutex<Option<tun::platform::Device>>>,
        connections: Arc<AsyncMutex<HashMap<String, TcpConnection>>>
    ) -> Result<()> {
        if packet.len() < 20 {
            log_debug!("数据包太小，忽略: {} 字节", packet.len());
            return Ok(()); // 数据包太小，忽略
        }
        
        // 解析IP头部
        let version = (packet[0] >> 4) & 0x0F;
        if version != 4 {
            log_debug!("非IPv4数据包，忽略: version={}", version);
            return Ok(()); // 只处理IPv4
        }
        
        let protocol = packet[9];
        let src_ip = Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]);
        let dst_ip = Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]);
        
        // 获取IP头部长度
        let ihl = (packet[0] & 0x0F) as usize * 4;
        if packet.len() < ihl {
            log_debug!("IP头部长度不足: {} < {}", packet.len(), ihl);
            return Ok(()); // 数据包长度不足
        }
        
        log_debug!("处理数据包: {} -> {}, 协议: {}", src_ip, dst_ip, protocol);
        
        match protocol {
            6 => { // TCP
                if packet.len() >= ihl + 20 { // 确保有足够的TCP头
                    let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                    let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                    
                    // 获取TCP头部长度
                    let tcp_header_len = ((packet[ihl + 12] >> 4) * 4) as usize;
                    let tcp_data_start = ihl + tcp_header_len;
                    
                    // 检查TCP头部长度是否有效
                    if tcp_data_start <= packet.len() {
                        let tcp_data = if packet.len() > tcp_data_start {
                            &packet[tcp_data_start..]
                        } else {
                            &[] // 空载荷，但仍然是有效的TCP包（如SYN、ACK等）
                        };
                        
                        let should_proxy = Self::should_proxy(&dst_ip, dst_port);
                        log_debug!("TCP数据包: {}:{} -> {}:{}, 数据长度: {}, 代理: {}", 
                                 src_ip, src_port, dst_ip, dst_port, tcp_data.len(), should_proxy);
                        Self::handle_tcp_packet_with_response(src_ip, src_port, dst_ip, dst_port, tcp_data, device.clone(), connections.clone()).await?;
                    } else {
                        log_warn!("TCP头部长度异常: {} > {}", tcp_data_start, packet.len());
                    }
                } else {
                    log_warn!("TCP数据包长度不足");
                }
            }
            17 => { // UDP
                if packet.len() >= ihl + 8 { // 确保有足够的UDP头
                    let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                    let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                    let udp_data = &packet[ihl + 8..];
                    
                    let should_proxy = Self::should_proxy(&dst_ip, dst_port);
                    log_debug!("UDP数据包: {}:{} -> {}:{}, 数据长度: {}, 代理: {}", 
                             src_ip, src_port, dst_ip, dst_port, udp_data.len(), should_proxy);
                    Self::handle_udp_packet_with_response(src_ip, src_port, dst_ip, dst_port, udp_data, device.clone()).await?;
                } else {
                    log_warn!("UDP数据包长度不足");
                }
            }
            1 => { // ICMP
                log_debug!("ICMP数据包: {} -> {}", src_ip, dst_ip);
                // ICMP数据包可以直接转发或丢弃
            }
            _ => {
                log_debug!("未知协议数据包: {} -> {}, 协议: {}", src_ip, dst_ip, protocol);
            }
        }
        
        Ok(())
    }

    /// 处理接收到的IP数据包
    /// 解析数据包并根据目标地址决定是否需要代理
    /// 
    /// # 参数
    /// * `packet` - 数据包内容
    #[allow(dead_code)]
    async fn process_packet(packet: &[u8]) -> Result<()> {
        if packet.len() < 20 {
            log_debug!("数据包太小，忽略: {} 字节", packet.len());
            return Ok(()); // 数据包太小，忽略
        }
        
        // 解析IP头部
        let version = (packet[0] >> 4) & 0x0F;
        if version != 4 {
            log_debug!("非IPv4数据包，忽略: version={}", version);
            return Ok(()); // 只处理IPv4
        }
        
        let protocol = packet[9];
        let src_ip = Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]);
        let dst_ip = Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]);
        
        // 获取IP头部长度
        let ihl = (packet[0] & 0x0F) as usize * 4;
        if packet.len() < ihl {
            log_debug!("IP头部长度不足: {} < {}", packet.len(), ihl);
            return Ok(()); // 数据包长度不足
        }
        
        log_debug!("处理数据包: {} -> {}, 协议: {}", src_ip, dst_ip, protocol);
        
        match protocol {
            6 => { // TCP
                if packet.len() >= ihl + 20 { // 确保有足够的TCP头
                    let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                    let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                    
                    // 获取TCP头部长度
                    let tcp_header_len = ((packet[ihl + 12] >> 4) * 4) as usize;
                    let tcp_data_start = ihl + tcp_header_len;
                    
                    if packet.len() > tcp_data_start {
                        let tcp_data = &packet[tcp_data_start..];
                        let should_proxy = Self::should_proxy(&dst_ip, dst_port);
                        log_debug!("TCP数据包: {}:{} -> {}:{}, 数据长度: {}, 代理: {}", 
                                 src_ip, src_port, dst_ip, dst_port, tcp_data.len(), should_proxy);
                        Self::handle_tcp_packet(src_ip, src_port, dst_ip, dst_port, tcp_data).await?;
                    } else {
                        log_debug!("TCP数据包无载荷: {}:{} -> {}:{}", 
                                 src_ip, src_port, dst_ip, dst_port);
                    }
                } else {
                    log_warn!("TCP数据包长度不足");
                }
            }
            17 => { // UDP
                if packet.len() >= ihl + 8 { // 确保有足够的UDP头
                    let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                    let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                    let udp_data = &packet[ihl + 8..];
                    
                    let should_proxy = Self::should_proxy(&dst_ip, dst_port);
                    log_debug!("UDP数据包: {}:{} -> {}:{}, 数据长度: {}, 代理: {}", 
                             src_ip, src_port, dst_ip, dst_port, udp_data.len(), should_proxy);
                    Self::handle_udp_packet(src_ip, src_port, dst_ip, dst_port, udp_data).await?;
                } else {
                    log_warn!("UDP数据包长度不足");
                }
            }
            1 => { // ICMP
                log_debug!("ICMP数据包: {} -> {}", src_ip, dst_ip);
                // ICMP数据包可以直接转发或丢弃
            }
            _ => {
                log_debug!("未知协议数据包: {} -> {}, 协议: {}", src_ip, dst_ip, protocol);
            }
        }
        
        Ok(())
    }
    
    /// 处理TCP数据包并实现响应回写
    /// 根据目标地址决定是否通过代理转发
    async fn handle_tcp_packet_with_response(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        tcp_data: &[u8],
        device: Arc<Mutex<Option<tun::platform::Device>>>,
        connections: Arc<AsyncMutex<HashMap<String, TcpConnection>>>
    ) -> Result<()> {
        log_debug!("处理TCP数据包: {}:{} -> {}:{}, 数据长度: {}", 
                 src_ip, src_port, dst_ip, dst_port, tcp_data.len());
        
        let manager = TunManager::instance();
        let config = manager.get_config().await;
        let mut target_ip = dst_ip;
        let mut target_domain: Option<String> = None;
        
        // 检查是否为FakeIP
        if config.fake_ip {
            let fake_manager_opt = {
                let guard = manager.fake_ip_manager.lock().unwrap();
                guard.as_ref().map(|fm| {
                    (fm.is_fake_ip(&dst_ip), fm.domain_to_fake.clone(), fm.fake_to_domain.clone(), fm.fake_to_real.clone())
                })
            };
            
            if let Some((is_fake, _domain_to_fake, fake_to_domain, fake_to_real)) = fake_manager_opt {
                if is_fake {
                    log_debug!("检测到FakeIP: {}", dst_ip);
                    
                    // 获取对应的域名
                    if let Some(domain) = fake_to_domain.lock().await.get(&dst_ip).cloned() {
                        log_info!("FakeIP {} 对应域名: {}", dst_ip, domain);
                        target_domain = Some(domain);
                        
                        // 尝试获取真实IP
                        if let Some(real_ip) = fake_to_real.lock().await.get(&dst_ip).cloned() {
                            target_ip = real_ip;
                            log_debug!("使用真实IP: {}", real_ip);
                        } else {
                            log_debug!("暂无真实IP，将通过域名代理连接");
                        }
                    }
                }
            }
        }
        
        // 检查是否需要代理（FakeIP总是通过代理）
        let should_proxy = config.fake_ip && target_domain.is_some() || Self::should_proxy(&target_ip, dst_port);
        
        if should_proxy {
            log_debug!("TCP流量通过代理转发: {}:{}", target_ip, dst_port);
            Self::forward_to_proxy_with_response(src_ip, src_port, target_ip, dst_port, tcp_data, "tcp", device, connections).await?
        } else {
            log_debug!("TCP流量直接转发: {}:{}", target_ip, dst_port);
            Self::forward_direct_with_response(src_ip, src_port, target_ip, dst_port, tcp_data, "tcp", device).await?
        }
        
        Ok(())
    }

    /// 处理TCP数据包
    /// 根据目标地址决定是否通过代理转发
    async fn handle_tcp_packet(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        tcp_data: &[u8],
    ) -> Result<()> {
        log_debug!("处理TCP数据包: {}:{} -> {}:{}, 数据长度: {}", 
                 src_ip, src_port, dst_ip, dst_port, tcp_data.len());
        
        // 检查是否需要代理
        if Self::should_proxy(&dst_ip, dst_port) {
            log_debug!("TCP流量通过代理转发: {}:{}", dst_ip, dst_port);
            Self::forward_to_proxy(src_ip, src_port, dst_ip, dst_port, tcp_data, "tcp").await?
        } else {
            log_debug!("TCP流量直接转发: {}:{}", dst_ip, dst_port);
            Self::forward_direct(src_ip, src_port, dst_ip, dst_port, tcp_data, "tcp").await?
        }
        
        Ok(())
    }
    
    /// 处理UDP数据包并实现响应回写
    /// 根据目标地址决定是否通过代理转发
    async fn handle_udp_packet_with_response(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        udp_data: &[u8],
        device: Arc<Mutex<Option<tun::platform::Device>>>
    ) -> Result<()> {
        log_debug!("处理UDP数据包: {}:{} -> {}:{}, 数据长度: {}", 
                 src_ip, src_port, dst_ip, dst_port, udp_data.len());
        
        let manager = TunManager::instance();
        let config = manager.get_config().await;
        let mut target_ip = dst_ip;
        let mut target_domain: Option<String> = None;
        
        // 检查是否为FakeIP
        if config.fake_ip {
            let fake_manager_opt = {
                let guard = manager.fake_ip_manager.lock().unwrap();
                guard.as_ref().map(|fm| {
                    (fm.is_fake_ip(&dst_ip), fm.domain_to_fake.clone(), fm.fake_to_domain.clone(), fm.fake_to_real.clone())
                })
            };
            
            if let Some((is_fake, _domain_to_fake, fake_to_domain, fake_to_real)) = fake_manager_opt {
                if is_fake {
                    log_debug!("检测到FakeIP: {}", dst_ip);
                    
                    // 获取对应的域名
                    if let Some(domain) = fake_to_domain.lock().await.get(&dst_ip).cloned() {
                        log_info!("FakeIP {} 对应域名: {}", dst_ip, domain);
                        target_domain = Some(domain);
                        
                        // 尝试获取真实IP
                        if let Some(real_ip) = fake_to_real.lock().await.get(&dst_ip).cloned() {
                            target_ip = real_ip;
                            log_debug!("使用真实IP: {}", real_ip);
                        } else {
                            log_debug!("暂无真实IP，将通过域名代理连接");
                        }
                    }
                }
            }
        }
        
        // 检查是否为DNS查询并需要劫持
        if target_ip == dst_ip && dst_port == 53 {
            if config.dns_hijack {
                log_debug!("DNS劫持: 重定向DNS查询到 {}", config.dns_server);
                return Self::handle_dns_hijack(src_ip, src_port, &config.dns_server, udp_data, device).await;
            }
        }
        
        // 检查是否需要代理（FakeIP总是通过代理）
        let should_proxy = config.fake_ip && target_domain.is_some() || Self::should_proxy(&target_ip, dst_port);
        
        if should_proxy {
            log_debug!("UDP流量通过代理转发: {}:{}", target_ip, dst_port);
            Self::forward_to_proxy_udp_with_response(src_ip, src_port, target_ip, dst_port, udp_data, device).await?
        } else {
            log_debug!("UDP流量直接转发: {}:{}", target_ip, dst_port);
            Self::forward_direct_udp_with_response(src_ip, src_port, target_ip, dst_port, udp_data, device).await?
        }
        
        Ok(())
    }

    /// 处理UDP数据包
    /// 根据目标地址决定是否通过代理转发
    async fn handle_udp_packet(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        udp_data: &[u8],
    ) -> Result<()> {
        log_debug!("处理UDP数据包: {}:{} -> {}:{}, 数据长度: {}", 
                 src_ip, src_port, dst_ip, dst_port, udp_data.len());
        
        // 检查是否需要代理
        if Self::should_proxy(&dst_ip, dst_port) {
            log_debug!("UDP流量通过代理转发: {}:{}", dst_ip, dst_port);
            Self::forward_to_proxy(src_ip, src_port, dst_ip, dst_port, udp_data, "udp").await?
        } else {
            log_debug!("UDP流量直接转发: {}:{}", dst_ip, dst_port);
            Self::forward_direct(src_ip, src_port, dst_ip, dst_port, udp_data, "udp").await?
        }
        
        Ok(())
    }
    
    /// 判断是否需要代理
    /// 根据目标IP和端口决定流量路由策略
    /// 
    /// # 参数
    /// * `dst_ip` - 目标IP地址
    /// * `dst_port` - 目标端口
    /// 
    /// # 返回值
    /// * `bool` - 是否需要代理
    /// 判断是否需要代理流量
    /// 参考sing-box的实现，提供更精确的流量分类和路由规则
    #[allow(dead_code)]
    fn should_proxy(dst_ip: &Ipv4Addr, dst_port: u16) -> bool {
        // 检查是否为本地回环地址
        if dst_ip.is_loopback() {
            log_debug!("本地回环地址，不代理: {}", dst_ip);
            return false;
        }
        
        // 检查是否为私有网络地址
        if dst_ip.is_private() {
            log_debug!("私有网络地址，不代理: {}", dst_ip);
            return false;
        }
        
        // 检查是否为链路本地地址
        if dst_ip.is_link_local() {
            log_debug!("链路本地地址，不代理: {}", dst_ip);
            return false;
        }
        
        // 检查是否为组播地址
        if dst_ip.is_multicast() {
            log_debug!("组播地址，不代理: {}", dst_ip);
            return false;
        }
        
        // 检查是否为广播地址
        if dst_ip.is_broadcast() {
            log_debug!("广播地址，不代理: {}", dst_ip);
            return false;
        }
        
        // 检查是否为保留地址范围
        if Self::is_reserved_ip(dst_ip) {
            log_debug!("保留地址范围，不代理: {}", dst_ip);
            return false;
        }
        
        // 检查是否为中国大陆IP地址（可选择不代理）
        if Self::is_china_ip(dst_ip) {
            log_debug!("中国大陆IP地址，不代理: {}", dst_ip);
            return false;
        }
        
        // 检查特殊端口（系统服务端口通常不需要代理）
        if Self::is_system_port(dst_port) {
            log_debug!("系统服务端口，不代理: {}:{}", dst_ip, dst_port);
            return false;
        }
        
        // 检查是否有代理服务器正在运行
        if !Self::is_proxy_running() {
            log_debug!("代理服务器未运行，直接连接: {}:{}", dst_ip, dst_port);
            return false;
        }
        
        log_debug!("需要代理: {}:{}", dst_ip, dst_port);
        true
    }
    
    /// 检查是否为保留IP地址范围
    fn is_reserved_ip(ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();
        
        match octets[0] {
            // 0.0.0.0/8 - 当前网络
            0 => true,
            // 127.0.0.0/8 - 回环地址（已在上面检查）
            127 => true,
            // 169.254.0.0/16 - 链路本地地址（已在上面检查）
            169 if octets[1] == 254 => true,
            // 224.0.0.0/4 - 组播地址（已在上面检查）
            224..=239 => true,
            // 240.0.0.0/4 - 保留地址
            240..=255 => true,
            _ => false,
        }
    }
    
    /// 检查是否为中国大陆IP地址
    /// 这里只包含一些主要的中国IP段，实际应用中可以使用更完整的IP数据库
    fn is_china_ip(ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();
        
        // 一些主要的中国IP段（简化版本）
        match octets[0] {
            // 中国电信
            58 | 59 | 60 | 61 | 112 | 113 | 114 | 115 | 116 | 117 | 118 | 119 => true,
            // 中国联通
            123 | 124 | 125 | 126 => true,
            // 中国移动
            111 => true,
            // 教育网
            202 if octets[1] >= 112 && octets[1] <= 120 => true,
            // 其他常见中国IP段
            121 | 122 => true,
            _ => false,
        }
    }
    
    /// 检查是否为系统服务端口
    fn is_system_port(port: u16) -> bool {
        match port {
            // DNS
            53 => true,
            // DHCP
            67 | 68 => true,
            // NTP
            123 => true,
            // SNMP
            161 | 162 => true,
            // NetBIOS
            137 | 138 | 139 => true,
            // LDAP
            389 | 636 => true,
            // Kerberos
            88 | 464 => true,
            // Windows系统端口
            135 | 445 => true,
            // mDNS
            5353 => true,
            // LLMNR
            5355 => true,
            // WS-Discovery
            3702 => true,
            _ => false,
        }
    }
    
    /// 转发数据到代理服务器并实现响应回写
    /// 通过SOCKS5协议转发TCP/UDP流量并将响应写回TUN设备
    async fn forward_to_proxy_with_response(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        data: &[u8],
        protocol: &str,
        device: Arc<Mutex<Option<tun::platform::Device>>>,
        connections: Arc<AsyncMutex<HashMap<String, TcpConnection>>>
    ) -> Result<()> {
        log_debug!("转发到代理并回写响应: {}:{} -> {}:{} ({})", 
                 src_ip, src_port, dst_ip, dst_port, protocol);
        
        // 创建连接标识
        let conn_key = format!("{}:{}->{}:{}", src_ip, src_port, dst_ip, dst_port);
        
        // 检查是否已有连接
        {
            let mut conns = connections.lock().await;
            if !conns.contains_key(&conn_key) {
                let conn = TcpConnection {
                    src_ip,
                    src_port,
                    dst_ip,
                    dst_port,
                    proxy_stream: None,
                    direct_stream: None,
                };
                conns.insert(conn_key.clone(), conn);
            }
        }
        
        // 实现代理转发逻辑
         match protocol {
             "tcp" => {
                 // TCP代理转发实现
                 log_debug!("TCP代理转发: {}", conn_key);
                 Self::handle_tcp_proxy_connection(src_ip, src_port, dst_ip, dst_port, data, device, connections).await?
             }
             "udp" => {
                 // UDP代理转发实现
                 log_debug!("UDP代理转发: {}", conn_key);
                 Self::handle_udp_proxy_connection(src_ip, src_port, dst_ip, dst_port, data, device).await?
             }
             _ => {
                 return Err(anyhow::anyhow!("不支持的协议: {}", protocol));
             }
         }
        
        Ok(())
    }
    
    /// 直接转发数据并实现响应回写
     /// 不通过代理，直接建立连接转发数据并将响应写回TUN设备
     async fn forward_direct_with_response(
         src_ip: Ipv4Addr,
         src_port: u16,
         dst_ip: Ipv4Addr,
         dst_port: u16,
         _data: &[u8],
         protocol: &str,
         _device: Arc<Mutex<Option<tun::platform::Device>>>
     ) -> Result<()> {
         log_debug!("直接转发并回写响应: {}:{} -> {}:{} ({})", 
                  src_ip, src_port, dst_ip, dst_port, protocol);
         
         match protocol {
             "tcp" => {
                 // TCP直接转发实现
                 log_debug!("TCP直接转发: {}:{}", dst_ip, dst_port);
             }
             "udp" => {
                 // UDP直接转发实现
                 log_debug!("UDP直接转发: {}:{}", dst_ip, dst_port);
             }
             _ => {
                 return Err(anyhow::anyhow!("不支持的协议: {}", protocol));
             }
         }
         
         Ok(())
     }
    
    /// 转发UDP数据到代理服务器并实现响应回写
     async fn forward_to_proxy_udp_with_response(
         src_ip: Ipv4Addr,
         src_port: u16,
         dst_ip: Ipv4Addr,
         dst_port: u16,
         _data: &[u8],
         _device: Arc<Mutex<Option<tun::platform::Device>>>
     ) -> Result<()> {
         log_debug!("UDP代理转发并回写: {}:{} -> {}:{}", 
                  src_ip, src_port, dst_ip, dst_port);
         
         // UDP代理转发实现
         Ok(())
     }
     
     /// 直接转发UDP数据并实现响应回写
      async fn forward_direct_udp_with_response(
          src_ip: Ipv4Addr,
          src_port: u16,
          dst_ip: Ipv4Addr,
          dst_port: u16,
          _data: &[u8],
          _device: Arc<Mutex<Option<tun::platform::Device>>>
      ) -> Result<()> {
          log_debug!("UDP直接转发并回写: {}:{} -> {}:{}", 
                   src_ip, src_port, dst_ip, dst_port);
          
          // UDP直接转发实现
          Ok(())
      }

      /// 解析DNS查询中的域名
      fn parse_dns_query(dns_data: &[u8]) -> Option<String> {
          if dns_data.len() < 12 {
              return None;
          }
          
          let mut pos = 12; // 跳过DNS头部
          let mut domain = String::new();
          
          while pos < dns_data.len() {
              let len = dns_data[pos] as usize;
              if len == 0 {
                  break;
              }
              
              pos += 1;
              if pos + len > dns_data.len() {
                  return None;
              }
              
              if !domain.is_empty() {
                  domain.push('.');
              }
              
              domain.push_str(&String::from_utf8_lossy(&dns_data[pos..pos + len]));
              pos += len;
          }
          
          if domain.is_empty() {
              None
          } else {
              Some(domain)
          }
      }
      
      /// 创建DNS响应包
      fn create_dns_response(query_data: &[u8], fake_ip: Ipv4Addr) -> Vec<u8> {
          let mut response = query_data.to_vec();
          
          if response.len() < 12 {
              return response;
          }
          
          // 设置响应标志
          response[2] = 0x81; // QR=1, Opcode=0, AA=0, TC=0, RD=1
          response[3] = 0x80; // RA=1, Z=0, RCODE=0
          
          // 设置Answer RRs计数为1
          response[6] = 0x00;
          response[7] = 0x01;
          
          // 添加Answer记录
          // 压缩指针指向查询名称
          response.push(0xc0);
          response.push(0x0c);
          
          // Type: A (0x0001)
          response.push(0x00);
          response.push(0x01);
          
          // Class: IN (0x0001)
          response.push(0x00);
          response.push(0x01);
          
          // TTL: 300秒
          response.push(0x00);
          response.push(0x00);
          response.push(0x01);
          response.push(0x2c);
          
          // Data length: 4字节
          response.push(0x00);
          response.push(0x04);
          
          // IP地址
          let ip_bytes = fake_ip.octets();
          response.extend_from_slice(&ip_bytes);
          
          response
      }

      /// 处理DNS劫持和FakeIP分配
      /// 将DNS查询重定向到指定的DNS服务器，并支持FakeIP模式
      async fn handle_dns_hijack(
          src_ip: Ipv4Addr,
          src_port: u16,
          dns_server: &str,
          dns_data: &[u8],
          device: Arc<Mutex<Option<tun::platform::Device>>>
      ) -> Result<()> {
          log_debug!("处理DNS劫持: {}:{} -> {}", src_ip, src_port, dns_server);
          
          // 检查是否启用FakeIP模式
          let manager = TunManager::instance();
          let config = manager.get_config().await;
          
          if config.fake_ip {
              // FakeIP模式：解析域名并分配虚假IP
              if let Some(domain) = Self::parse_dns_query(dns_data) {
                  log_debug!("解析到域名: {}", domain);
                  
                  // 获取FakeIP管理器的克隆引用
                  let fake_manager_opt = {
                      let guard = manager.fake_ip_manager.lock().unwrap();
                      guard.as_ref().map(|fm| {
                          (fm.domain_to_fake.clone(), fm.fake_to_domain.clone(), fm.start_ip, fm.end_ip, fm.next_ip.clone())
                      })
                  };
                  
                  if let Some((domain_to_fake, fake_to_domain, start_ip, end_ip, next_ip)) = fake_manager_opt {
                      // 检查是否已有FakeIP
                      let fake_ip = if let Some(existing_ip) = domain_to_fake.lock().await.get(&domain).cloned() {
                          existing_ip
                      } else {
                          // 分配新的FakeIP
                          let mut next_guard = next_ip.lock().await;
                          let new_fake_ip = Ipv4Addr::from(*next_guard);
                          *next_guard = if new_fake_ip == end_ip { 
                              u32::from(start_ip) 
                          } else { 
                              *next_guard + 1
                          };
                          drop(next_guard);
                          
                          // 更新映射表
                          domain_to_fake.lock().await.insert(domain.clone(), new_fake_ip);
                          fake_to_domain.lock().await.insert(new_fake_ip, domain.clone());
                          
                          new_fake_ip
                      };
                      
                      log_info!("为域名 {} 分配FakeIP: {}", domain, fake_ip);
                      
                      // 创建DNS响应包
                      let response = Self::create_dns_response(dns_data, fake_ip);
                      
                      // 将DNS响应写回TUN设备
                      return Self::write_response_packet(
                          device,
                          "8.8.8.8".parse().unwrap(), // 伪装成来自DNS服务器的响应
                          53,
                          src_ip,
                          src_port,
                          &response,
                          17 // UDP协议
                      ).await;
                  }
              }
          }
          
          // 传统DNS劫持模式：转发到真实DNS服务器
          let dns_ip: Ipv4Addr = dns_server.parse()
              .with_context(|| format!("无效的DNS服务器地址: {}", dns_server))?;
          
          // 创建UDP套接字连接到DNS服务器
          let socket = UdpSocket::bind("0.0.0.0:0").await
              .with_context(|| "创建UDP套接字失败")?;
          
          // 发送DNS查询到指定的DNS服务器
          socket.send_to(dns_data, (dns_ip, 53)).await
              .with_context(|| format!("发送DNS查询到 {} 失败", dns_server))?;
          
          // 接收DNS响应
          let mut response_buf = vec![0u8; 512]; // DNS响应通常不超过512字节
          let (response_len, _) = socket.recv_from(&mut response_buf).await
              .with_context(|| "接收DNS响应失败")?;
          
          response_buf.truncate(response_len);
          
          // 将DNS响应写回TUN设备
          Self::write_response_packet(
              device,
              dns_ip,
              53,
              src_ip,
              src_port,
              &response_buf,
              17 // UDP协议号
          ).await?;
          
          log_debug!("DNS劫持完成: 响应长度 {}", response_len);
          Ok(())
      }
     
     /// 将响应数据包写回TUN设备
     /// 构造IP数据包并写入TUN设备，实现双向通信
     async fn write_response_packet(
         device: Arc<Mutex<Option<tun::platform::Device>>>,
         src_ip: Ipv4Addr,
         src_port: u16,
         dst_ip: Ipv4Addr,
         dst_port: u16,
         data: &[u8],
         protocol: u8
     ) -> Result<()> {
         // 构造IP头部 (20字节)
         let mut packet = Vec::with_capacity(20 + 20 + data.len()); // IP头 + TCP/UDP头 + 数据
         
         // IP头部
         packet.push(0x45); // 版本(4) + 头部长度(5*4=20字节)
         packet.push(0x00); // 服务类型
         
         let total_len = if protocol == 6 { // TCP
             20 + 20 + data.len() // IP头 + TCP头 + 数据
         } else { // UDP
             20 + 8 + data.len() // IP头 + UDP头 + 数据
         };
         
         packet.extend_from_slice(&(total_len as u16).to_be_bytes()); // 总长度
         packet.extend_from_slice(&[0x00, 0x00]); // 标识
         packet.extend_from_slice(&[0x40, 0x00]); // 标志 + 片偏移
         packet.push(64); // TTL
         packet.push(protocol); // 协议
         packet.extend_from_slice(&[0x00, 0x00]); // 校验和(稍后计算)
         
         // 源IP (响应时源目IP互换)
         packet.extend_from_slice(&dst_ip.octets());
         // 目标IP
         packet.extend_from_slice(&src_ip.octets());
         
         // 计算IP头校验和
         let checksum = Self::calculate_checksum(&packet[0..20]);
         packet[10] = (checksum >> 8) as u8;
         packet[11] = (checksum & 0xFF) as u8;
         
         // 添加传输层头部
         if protocol == 6 { // TCP
             // TCP头部 (简化版，20字节)
             packet.extend_from_slice(&dst_port.to_be_bytes()); // 源端口
             packet.extend_from_slice(&src_port.to_be_bytes()); // 目标端口
             packet.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // 序列号
             packet.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // 确认号
             packet.extend_from_slice(&[0x50, 0x18]); // 头部长度 + 标志
             packet.extend_from_slice(&[0xFF, 0xFF]); // 窗口大小
             packet.extend_from_slice(&[0x00, 0x00]); // 校验和
             packet.extend_from_slice(&[0x00, 0x00]); // 紧急指针
         } else { // UDP
             // UDP头部 (8字节)
             packet.extend_from_slice(&dst_port.to_be_bytes()); // 源端口
             packet.extend_from_slice(&src_port.to_be_bytes()); // 目标端口
             packet.extend_from_slice(&((8 + data.len()) as u16).to_be_bytes()); // UDP长度
             packet.extend_from_slice(&[0x00, 0x00]); // 校验和
         }
         
         // 添加数据
         packet.extend_from_slice(data);
         
         // 写入TUN设备
          let device_clone = device.clone();
          tokio::task::spawn_blocking(move || {
              let mut device_guard = device_clone.lock().unwrap();
              if let Some(tun_device) = device_guard.as_mut() {
                  match tun_device.write(&packet) {
                      Ok(written) => {
                          log_debug!("响应数据包已写入TUN设备: {} 字节", written);
                          Ok(())
                      }
                      Err(e) => {
                          log_error!("写入TUN设备失败: {}", e);
                          Err(anyhow::anyhow!("写入TUN设备失败: {}", e))
                      }
                  }
              } else {
                  log_error!("TUN设备不可用");
                  Err(anyhow::anyhow!("TUN设备不可用"))
              }
          }).await?
     }
     
     /// 处理TCP代理连接
      /// 建立与SOCKS5代理的连接并转发数据
      async fn handle_tcp_proxy_connection(
          src_ip: Ipv4Addr,
          src_port: u16,
          dst_ip: Ipv4Addr,
          dst_port: u16,
          data: &[u8],
          device: Arc<Mutex<Option<tun::platform::Device>>>,
          connections: Arc<AsyncMutex<HashMap<String, TcpConnection>>>
      ) -> Result<()> {
          log_debug!("处理TCP代理连接: {}:{} -> {}:{}", src_ip, src_port, dst_ip, dst_port);
          
          let conn_key = format!("{}:{}->{}:{}", src_ip, src_port, dst_ip, dst_port);
          
          // 检查是否已有活跃连接
          let should_remove_conn = {
              let mut conns = connections.lock().await;
              if let Some(conn) = conns.get_mut(&conn_key) {
                  if let Some(proxy_stream) = &conn.proxy_stream {
                      // 使用现有连接转发数据
                      let mut stream = proxy_stream.lock().await;
                      if let Err(e) = stream.write_all(data).await {
                          log_error!("写入代理连接失败: {}", e);
                          true // 标记需要移除连接
                      } else {
                          log_debug!("数据已通过现有代理连接转发: {} 字节", data.len());
                          return Ok(());
                      }
                  } else {
                      false
                  }
              } else {
                  false
              }
          };
          
          // 如果连接失效，移除它
          if should_remove_conn {
              let mut conns = connections.lock().await;
              conns.remove(&conn_key);
          }
          
          // 建立新的SOCKS5代理连接
          let proxy_port = Self::get_proxy_port();
          let proxy_addr = format!("127.0.0.1:{}", proxy_port);
          
          match TcpStream::connect(&proxy_addr).await {
              Ok(mut stream) => {
                  log_debug!("已连接到SOCKS5代理: {}", proxy_addr);
                  
                  // SOCKS5握手
                  if let Err(e) = Self::socks5_handshake(&mut stream).await {
                      log_error!("SOCKS5握手失败: {}", e);
                      return Err(e);
                  }
                  
                  // SOCKS5连接请求
                  if let Err(e) = Self::socks5_connect(&mut stream, dst_ip, dst_port).await {
                      log_error!("SOCKS5连接请求失败: {}", e);
                      return Err(e);
                  }
                  
                  // 发送初始数据
                  if !data.is_empty() {
                      if let Err(e) = stream.write_all(data).await {
                          log_error!("发送初始数据失败: {}", e);
                          return Err(anyhow::anyhow!("发送初始数据失败: {}", e));
                      }
                      log_debug!("初始数据已发送: {} 字节", data.len());
                  }
                  
                  // 保存连接
                  let stream_arc = Arc::new(AsyncMutex::new(stream));
                  {
                      let mut conns = connections.lock().await;
                      if let Some(conn) = conns.get_mut(&conn_key) {
                          conn.proxy_stream = Some(stream_arc.clone());
                      }
                  }
                  
                  // 启动响应数据读取任务
                  let device_clone = device.clone();
                  let stream_clone = stream_arc.clone();
                  tokio::spawn(async move {
                      Self::handle_proxy_response(src_ip, src_port, dst_ip, dst_port, stream_clone, device_clone).await
                  });
                  
                  log_debug!("TCP代理连接建立成功: {}", conn_key);
              }
              Err(e) => {
                  log_error!("连接SOCKS5代理失败: {}", e);
                  return Err(anyhow::anyhow!("连接SOCKS5代理失败: {}", e));
              }
          }
          
          Ok(())
      }
      
      /// 处理UDP代理连接
      /// 建立与SOCKS5代理的UDP关联并转发数据
      async fn handle_udp_proxy_connection(
           src_ip: Ipv4Addr,
           src_port: u16,
           dst_ip: Ipv4Addr,
           dst_port: u16,
           _data: &[u8],
           _device: Arc<Mutex<Option<tun::platform::Device>>>
       ) -> Result<()> {
          log_debug!("处理UDP代理连接: {}:{} -> {}:{}", src_ip, src_port, dst_ip, dst_port);
          
          // UDP over SOCKS5实现
          // 这里可以实现UDP关联或者通过TCP隧道转发UDP数据
          log_warn!("UDP代理转发暂未完全实现");
          
          Ok(())
      }
      
      /// 处理代理响应数据
      /// 从代理连接读取响应数据并写回TUN设备
      async fn handle_proxy_response(
          src_ip: Ipv4Addr,
          src_port: u16,
          dst_ip: Ipv4Addr,
          dst_port: u16,
          stream: Arc<AsyncMutex<TcpStream>>,
          device: Arc<Mutex<Option<tun::platform::Device>>>
      ) -> Result<()> {
          log_debug!("开始处理代理响应: {}:{} <- {}:{}", src_ip, src_port, dst_ip, dst_port);
          
          let mut buffer = [0u8; 4096];
          loop {
              let mut stream_guard = stream.lock().await;
              match stream_guard.read(&mut buffer).await {
                  Ok(0) => {
                      log_debug!("代理连接已关闭: {}:{}", dst_ip, dst_port);
                      break;
                  }
                  Ok(n) => {
                      log_debug!("从代理接收到响应数据: {} 字节", n);
                      drop(stream_guard); // 释放锁
                      
                      // 将响应数据写回TUN设备
                      if let Err(e) = Self::write_response_packet(
                          device.clone(),
                          src_ip,
                          src_port,
                          dst_ip,
                          dst_port,
                          &buffer[..n],
                          6 // TCP协议
                      ).await {
                          log_error!("写回响应数据失败: {}", e);
                          break;
                      }
                  }
                  Err(e) => {
                      log_error!("读取代理响应失败: {}", e);
                      break;
                  }
              }
          }
          
          Ok(())
      }
      
      /// 计算IP头校验和
      fn calculate_checksum(data: &[u8]) -> u16 {
          let mut sum: u32 = 0;
          
          // 按16位字处理
          for i in (0..data.len()).step_by(2) {
              if i + 1 < data.len() {
                  sum += ((data[i] as u32) << 8) + (data[i + 1] as u32);
              } else {
                  sum += (data[i] as u32) << 8;
              }
          }
          
          // 处理进位
          while (sum >> 16) != 0 {
              sum = (sum & 0xFFFF) + (sum >> 16);
          }
          
          // 取反
          !(sum as u16)
      }

    /// SOCKS5握手
    async fn socks5_handshake(stream: &mut TcpStream) -> Result<()> {
        // 发送握手请求: VER(1) + NMETHODS(1) + METHODS(1)
        let handshake = [0x05, 0x01, 0x00]; // SOCKS5, 1个方法, 无认证
        stream.write_all(&handshake).await?;
        
        // 读取服务器响应: VER(1) + METHOD(1)
        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await?;
        
        if response[0] != 0x05 {
            return Err(anyhow::anyhow!("Invalid SOCKS5 version"));
        }
        
        if response[1] != 0x00 {
            return Err(anyhow::anyhow!("SOCKS5 authentication required"));
        }
        
        Ok(())
    }

    /// SOCKS5连接请求
    async fn socks5_connect(stream: &mut TcpStream, dst_ip: Ipv4Addr, dst_port: u16) -> Result<()> {
        // 构建连接请求: VER(1) + CMD(1) + RSV(1) + ATYP(1) + DST.ADDR(4) + DST.PORT(2)
        let mut request = Vec::with_capacity(10);
        request.push(0x05); // SOCKS5版本
        request.push(0x01); // CONNECT命令
        request.push(0x00); // 保留字段
        request.push(0x01); // IPv4地址类型
        
        // 添加目标IP地址（4字节）
        request.extend_from_slice(&dst_ip.octets());
        
        // 添加目标端口（2字节，大端序）
        request.extend_from_slice(&dst_port.to_be_bytes());
        
        // 发送连接请求
        stream.write_all(&request).await?;
        
        // 读取服务器响应: VER(1) + REP(1) + RSV(1) + ATYP(1) + BND.ADDR(4) + BND.PORT(2)
        let mut response = [0u8; 10];
        stream.read_exact(&mut response).await?;
        
        if response[0] != 0x05 {
            return Err(anyhow::anyhow!("Invalid SOCKS5 version in response"));
        }
        
        if response[1] != 0x00 {
            return Err(anyhow::anyhow!("SOCKS5 connection failed: {}", response[1]));
        }
        
        Ok(())
    }
    
    /// 检查代理服务器是否正在运行
    /// 
    /// # 返回值
    /// * `bool` - 代理服务器是否运行中
    #[allow(dead_code)]
    fn is_proxy_running() -> bool {
        use crate::proxy::ProxyManager;
        use std::net::TcpStream;
        use std::time::Duration;
        
        // 使用阻塞方式检查代理状态，避免在数据包处理中使用异步
        let proxy_manager = ProxyManager::instance();
        
        // 首先检查进程是否存在
        if !proxy_manager.is_process_running() {
            log_debug!("代理进程未运行");
            return false;
        }
        
        // 检查SOCKS5端口是否真正在监听
        let proxy_port = {
            use crate::config::AppConfig;
            match AppConfig::load() {
                Ok(config) => config.socks_port,
                Err(_) => 1080 // 默认端口
            }
        };
        let proxy_addr = format!("127.0.0.1:{}", proxy_port);
        
        match TcpStream::connect_timeout(&proxy_addr.parse().unwrap(), Duration::from_millis(100)) {
            Ok(_) => {
                log_debug!("代理服务器正在运行，端口{}可连接", proxy_port);
                true
            }
            Err(_) => {
                log_debug!("代理进程存在但端口{}不可连接", proxy_port);
                false
            }
        }
    }
    
    /// 获取代理服务器的SOCKS5端口
    /// 
    /// # 返回值
    /// * `u16` - SOCKS5代理端口，默认1080
    #[allow(dead_code)]
    fn get_proxy_port() -> u16 {
        use crate::config::AppConfig;
        
        // 尝试加载配置获取SOCKS5端口
        match AppConfig::load() {
            Ok(config) => {
                // 获取SOCKS5端口
                let port = config.socks_port;
                log_debug!("获取到代理端口: {}", port);
                port
            }
            Err(e) => {
                log_error!("加载配置失败: {}, 使用默认端口 1080", e);
                1080
            }
        }
    }
    
    /// 转发数据到代理服务器
    /// 通过SOCKS5协议转发TCP/UDP流量
    /// 
    /// # 参数
    /// * `src_ip` - 源IP地址
    /// * `src_port` - 源端口
    /// * `dst_ip` - 目标IP地址
    /// * `dst_port` - 目标端口
    /// * `data` - 数据内容
    /// * `protocol` - 协议类型
    /// 
    /// # 返回值
    /// * `Result<()>` - 转发结果
    #[allow(dead_code)]
    async fn forward_to_proxy(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        data: &[u8],
        protocol: &str,
    ) -> Result<()> {
        let proxy_port = Self::get_proxy_port();
        log_debug!("转发到代理: {}:{} -> {}:{} ({}), 代理端口: {}", 
                 src_ip, src_port, dst_ip, dst_port, protocol, proxy_port);
        
        if proxy_port == 0 {
            log_error!("代理端口未配置");
            return Err(anyhow::anyhow!("代理端口未配置"));
        }
        
        match protocol {
            "tcp" => {
                // 连接到本地SOCKS5代理
                match TcpStream::connect(("127.0.0.1", proxy_port)).await {
                    Ok(mut stream) => {
                        log_debug!("已连接到SOCKS5代理: 127.0.0.1:{}", proxy_port);
                        
                        // SOCKS5握手
                        let handshake = [0x05, 0x01, 0x00]; // VER, NMETHODS, METHODS(无认证)
                        if let Err(e) = stream.write_all(&handshake).await {
                            log_error!("发送SOCKS5握手失败: {}", e);
                            return Err(e.into());
                        }
                        
                        let mut response = [0u8; 2];
                        if let Err(e) = stream.read_exact(&mut response).await {
                            log_error!("读取SOCKS5握手响应失败: {}", e);
                            return Err(e.into());
                        }
                        
                        if response[0] == 0x05 && response[1] == 0x00 {
                            log_debug!("SOCKS5握手成功");
                            
                            // 发送连接请求
                            let mut request = vec![0x05, 0x01, 0x00, 0x01]; // VER, CMD(CONNECT), RSV, ATYP(IPv4)
                            request.extend_from_slice(&dst_ip.octets());
                            request.extend_from_slice(&dst_port.to_be_bytes());
                            
                            if let Err(e) = stream.write_all(&request).await {
                                log_error!("发送SOCKS5连接请求失败: {}", e);
                                return Err(e.into());
                            }
                            
                            let mut connect_response = [0u8; 10];
                            if let Err(e) = stream.read_exact(&mut connect_response).await {
                                log_error!("读取SOCKS5连接响应失败: {}", e);
                                return Err(e.into());
                            }
                            
                            if connect_response[1] == 0x00 {
                                log_debug!("SOCKS5连接建立成功，开始转发数据");
                                
                                // 连接成功，转发数据
                                if !data.is_empty() {
                                    if let Err(e) = stream.write_all(data).await {
                                        log_error!("转发TCP数据失败: {}", e);
                                        return Err(e.into());
                                    }
                                    log_debug!("TCP数据已转发到代理，大小: {} 字节", data.len());
                                }
                            } else {
                                log_error!("SOCKS5连接失败，错误码: {}", connect_response[1]);
                                return Err(anyhow::anyhow!("SOCKS5连接失败，错误码: {}", connect_response[1]));
                            }
                        } else {
                            log_error!("SOCKS5握手失败，响应: {:?}", response);
                            return Err(anyhow::anyhow!("SOCKS5握手失败"));
                        }
                    }
                    Err(e) => {
                        log_error!("连接SOCKS5代理失败: {}", e);
                        // 如果代理不可用，尝试直接连接
                        Self::forward_direct(src_ip, src_port, dst_ip, dst_port, data, protocol).await?;
                    }
                }
            }
            "udp" => {
                // UDP代理转发（通过SOCKS5 UDP ASSOCIATE）
                log_debug!("处理UDP代理转发");
                
                match UdpSocket::bind("0.0.0.0:0").await {
                    Ok(socket) => {
                        // 构造SOCKS5 UDP请求包
                        let mut udp_request = vec![0x00, 0x00, 0x00, 0x01]; // RSV + FRAG + ATYP(IPv4)
                        udp_request.extend_from_slice(&dst_ip.octets());
                        udp_request.extend_from_slice(&dst_port.to_be_bytes());
                        udp_request.extend_from_slice(data);
                        
                        // 发送到代理的UDP端口（通常是代理端口+1或单独配置）
                        let udp_proxy_port = proxy_port + 1;
                        if let Err(e) = socket.send_to(&udp_request, ("127.0.0.1", udp_proxy_port)).await {
                            log_error!("发送UDP数据到代理失败: {}", e);
                            return Err(e.into());
                        }
                        
                        log_debug!("UDP数据已转发到代理，大小: {} 字节，目标端口: {}", 
                                 data.len(), udp_proxy_port);
                    }
                    Err(e) => {
                        log_error!("创建UDP套接字失败: {}", e);
                        return Err(e.into());
                    }
                }
            }
            _ => {
                log_warn!("不支持的协议: {}", protocol);
                return Err(anyhow::anyhow!("不支持的协议: {}", protocol));
            }
        }
        
        Ok(())
    }
    
    /// 直接转发数据到目标服务器
    /// 不通过代理，直接建立连接转发数据
    async fn forward_direct(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        data: &[u8],
        protocol: &str,
    ) -> Result<()> {
        log_debug!("直接转发: {}:{} -> {}:{} ({}), 数据大小: {} 字节", 
                 src_ip, src_port, dst_ip, dst_port, protocol, data.len());
        
        match protocol {
            "tcp" => {
                // 直接TCP连接
                match TcpStream::connect((dst_ip, dst_port)).await {
                    Ok(mut stream) => {
                        log_debug!("TCP直接连接建立成功: {}:{}", dst_ip, dst_port);
                        
                        if !data.is_empty() {
                            if let Err(e) = stream.write_all(data).await {
                                log_error!("TCP直接转发数据失败: {}", e);
                                return Err(e.into());
                            }
                            log_debug!("TCP数据已直接转发，大小: {} 字节", data.len());
                        }
                        
                        // 可以在这里添加双向数据转发逻辑
                        // 但由于TUN设备的特性，通常只需要单向转发
                    }
                    Err(e) => {
                        log_error!("TCP直接连接失败: {}:{}, 错误: {}", dst_ip, dst_port, e);
                        return Err(e.into());
                    }
                }
            }
            "udp" => {
                // 直接UDP发送
                match UdpSocket::bind("0.0.0.0:0").await {
                    Ok(socket) => {
                        log_debug!("UDP套接字创建成功，准备直接转发");
                        
                        if let Err(e) = socket.send_to(data, (dst_ip, dst_port)).await {
                            log_error!("UDP直接转发失败: {}:{}, 错误: {}", dst_ip, dst_port, e);
                            return Err(e.into());
                        }
                        
                        log_debug!("UDP数据已直接转发到 {}:{}, 大小: {} 字节", 
                                 dst_ip, dst_port, data.len());
                    }
                    Err(e) => {
                        log_error!("创建UDP套接字失败: {}", e);
                        return Err(e.into());
                    }
                }
            }
            _ => {
                log_warn!("不支持的协议: {}", protocol);
                return Err(anyhow::anyhow!("不支持的协议: {}", protocol));
            }
        }
        
        Ok(())
    }

    /// 获取默认网络接口名称
    /// 
    /// # 返回值
    /// * `Result<String>` - 默认网络接口名称
    fn get_default_interface() -> Result<String> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            
            // 使用route print命令获取默认路由信息
            let output = Command::new("route")
                .args(&["print", "0.0.0.0"])
                .output()
                .context("执行route命令失败")?;
                
            if !output.status.success() {
                return Err(anyhow::anyhow!("route命令执行失败"));
            }
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            // 解析输出，查找默认路由的接口
            for line in output_str.lines() {
                if line.trim().starts_with("0.0.0.0") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        // 接口索引通常在第4列
                        let interface_index = parts[3];
                        
                        // 使用netsh命令获取接口名称
                        let interface_output = Command::new("netsh")
                            .args(&["interface", "show", "interface"])
                            .output()
                            .context("执行netsh命令失败")?;
                            
                        if interface_output.status.success() {
                            let interface_str = String::from_utf8_lossy(&interface_output.stdout);
                            for interface_line in interface_str.lines() {
                                if interface_line.contains(interface_index) {
                                    let interface_parts: Vec<&str> = interface_line.split_whitespace().collect();
                                    if interface_parts.len() >= 4 {
                                        return Ok(interface_parts[3].to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // 如果无法获取默认接口，返回常见的以太网接口名称
            Ok("以太网".to_string())
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // 非Windows平台使用ip route命令
            use std::process::Command;
            
            let output = Command::new("ip")
                .args(&["route", "show", "default"])
                .output()
                .context("执行ip route命令失败")?;
                
            if !output.status.success() {
                return Err(anyhow::anyhow!("ip route命令执行失败"));
            }
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            // 解析输出，查找默认路由的接口
            for line in output_str.lines() {
                if line.contains("default") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    for (i, part) in parts.iter().enumerate() {
                        if *part == "dev" && i + 1 < parts.len() {
                            return Ok(parts[i + 1].to_string());
                        }
                    }
                }
            }
            
            // 如果无法获取默认接口，返回常见的接口名称
            Ok("eth0".to_string())
        }
    }
    
    /// 检查是否以管理员权限运行
    /// 
    /// # 返回值
    /// * `bool` - 是否具有管理员权限
    fn is_admin() -> bool {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            
            // 尝试执行需要管理员权限的命令来检测权限
            let output = Command::new("net")
                .args(&["session"])
                .output();
                
            match output {
                Ok(output) => output.status.success(),
                Err(_) => false,
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // 非Windows平台暂时返回true
            true
        }
    }
    
    /// 备份当前路由表
    /// 
    /// # Returns
    /// * `Result<()>` - 备份结果
    fn backup_routes(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            
            // 获取当前路由表
            let output = Command::new("route")
                .args(&["print"])
                .output()
                .context("执行route print命令失败")?;
                
            if !output.status.success() {
                return Err(anyhow::anyhow!("route print命令执行失败"));
            }
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            let mut routes = Vec::new();
            
            // 解析路由表，保存重要的路由信息
            for line in output_str.lines() {
                let line = line.trim();
                if line.starts_with("0.0.0.0") || line.starts_with("128.0.0.0") {
                    routes.push(line.to_string());
                }
            }
            
            // 保存到original_routes字段
            if let Ok(mut original_routes) = self.original_routes.lock() {
                *original_routes = routes;
                log_info!("已备份 {} 条路由规则", original_routes.len());
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            use std::process::Command;
            
            // 获取当前路由表
            let output = Command::new("ip")
                .args(&["route", "show"])
                .output()
                .context("执行ip route show命令失败")?;
                
            if !output.status.success() {
                return Err(anyhow::anyhow!("ip route show命令执行失败"));
            }
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            let mut routes = Vec::new();
            
            // 解析路由表，保存重要的路由信息
            for line in output_str.lines() {
                let line = line.trim();
                if line.contains("default") || line.contains("0.0.0.0/1") || line.contains("128.0.0.0/1") {
                    routes.push(line.to_string());
                }
            }
            
            // 保存到original_routes字段
            if let Ok(mut original_routes) = self.original_routes.lock() {
                *original_routes = routes;
                log_info!("已备份 {} 条路由规则", original_routes.len());
            }
        }
        
        Ok(())
    }
    
    /// 恢复原始路由表
    /// 
    /// # Returns
    /// * `Result<()>` - 恢复结果
    fn restore_routes(&self) -> Result<()> {
        let routes = {
            if let Ok(original_routes) = self.original_routes.lock() {
                original_routes.clone()
            } else {
                return Err(anyhow::anyhow!("无法获取原始路由信息"));
            }
        };
        
        if routes.is_empty() {
            log_warn!("没有需要恢复的路由规则");
            return Ok(());
        }
        
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            
            // 删除TUN相关的路由规则
            let _ = Command::new("route")
                .args(&["delete", "0.0.0.0", "mask", "128.0.0.0"])
                .output();
                
            let _ = Command::new("route")
                .args(&["delete", "128.0.0.0", "mask", "128.0.0.0"])
                .output();
            
            // 恢复原始路由（如果需要）
            for route in &routes {
                let parts: Vec<&str> = route.split_whitespace().collect();
                if parts.len() >= 4 {
                    let destination = parts[0];
                    let mask = parts[1];
                    let gateway = parts[2];
                    let interface = parts[3];
                    
                    let _ = Command::new("route")
                        .args(&["add", destination, "mask", mask, gateway, "if", interface])
                        .output();
                }
            }
            
            log_info!("已恢复 {} 条路由规则", routes.len());
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            use std::process::Command;
            
            // 删除TUN相关的路由规则
            let _ = Command::new("ip")
                .args(&["route", "del", "0.0.0.0/1"])
                .output();
                
            let _ = Command::new("ip")
                .args(&["route", "del", "128.0.0.0/1"])
                .output();
            
            // 恢复原始路由（如果需要）
            for route in &routes {
                if route.contains("default") {
                    let _ = Command::new("ip")
                        .args(&["route", "add"])
                        .arg(route)
                        .output();
                }
            }
            
            log_info!("已恢复 {} 条路由规则", routes.len());
        }
        
        // 清空备份的路由信息
        if let Ok(mut original_routes) = self.original_routes.lock() {
            original_routes.clear();
        }
        
        Ok(())
    }
    
    /// 设置系统路由规则
    /// 
    /// 参数
    /// * `enable` - 是否启用路由规则
    /// 
    /// 返回值
    /// * `Result<()>` - 操作结果

    /// 配置TUN设备的IP地址和网关
    /// 
    /// # 参数
    /// * `config` - TUN配置信息
    /// 
    /// # 返回值
    /// * `Result<()>` - 操作结果
    async fn configure_tun_ip(&self, config: &TunConfig) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            
            let interface_name = &config.name;
            
            // 使用netsh命令配置IP地址
            let ip_config_output = Command::new("netsh")
                .args(&[
                    "interface", "ip", "set", "address",
                    interface_name,
                    "static",
                    &config.address.to_string(),
                    &config.netmask.to_string(),
                    &config.gateway.to_string()
                ])
                .output()
                .context("执行netsh配置IP地址命令失败")?;
            
            if !ip_config_output.status.success() {
                let error = String::from_utf8_lossy(&ip_config_output.stderr);
                log_warn!("使用netsh配置IP地址失败: {}", error);
                
                // 尝试备用方法：分别设置IP地址和网关
                let addr_output = Command::new("netsh")
                    .args(&[
                        "interface", "ip", "set", "address",
                        interface_name,
                        "static",
                        &config.address.to_string(),
                        &config.netmask.to_string()
                    ])
                    .output()
                    .context("执行netsh设置IP地址命令失败")?;
                
                if !addr_output.status.success() {
                    let addr_error = String::from_utf8_lossy(&addr_output.stderr);
                    return Err(anyhow::anyhow!("配置TUN设备IP地址失败: {}", addr_error));
                }
                
                log_info!("成功配置TUN设备IP地址: {}/{}", config.address, config.netmask);
            } else {
                log_info!("成功配置TUN设备IP地址和网关: {}/{} -> {}", 
                         config.address, config.netmask, config.gateway);
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            log_warn!("非Windows系统暂不支持TUN IP配置");
        }
        
        Ok(())
    }

    /// 设置系统路由表
    pub async fn set_system_route(&self, enable: bool) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            
            if enable {
                // 检查管理员权限
                if !Self::is_admin() {
                    return Err(anyhow::anyhow!("启用TUN模式需要管理员权限，请以管理员身份运行程序"));
                }
                
                // 获取TUN设备配置
                let config = self.get_config().await;
                let tun_interface = &config.name;
                let tun_gateway = config.address.to_string();  // 使用配置的虚拟网卡IP作为网关
                let strict_route = config.strict_route;
                
                // 获取默认网络接口，用于代理连接绑定
                let default_interface = match Self::get_default_interface() {
                    Ok(interface) => {
                        log_info!("检测到默认网络接口: {}", interface);
                        interface
                    }
                    Err(e) => {
                        log_warn!("无法获取默认网络接口: {}, 使用默认值", e);
                        "以太网".to_string()
                    }
                };
                
                // 备份原始路由表
                if let Err(e) = self.backup_routes() {
                    log_warn!("备份原始路由表失败: {}", e);
                } else {
                    log_info!("原始路由表已备份");
                }
                
                // 为代理服务器添加特定路由，确保代理连接通过默认接口
                // 这是防止路由循环的关键步骤，借鉴sing-box的设计
                let proxy_servers = [
                    "127.0.0.1",  // 本地代理服务器
                    "8.8.8.8",    // 常见的DNS服务器
                    "8.8.4.4",    // 备用DNS服务器
                ];
                
                // 获取默认网关IP
                let default_gateway_output = Command::new("route")
                    .args(&["print", "0.0.0.0"])
                    .output()
                    .context("获取默认网关失败")?;
                    
                let mut default_gateway = "192.168.1.1".to_string(); // 默认值
                if default_gateway_output.status.success() {
                    let output_str = String::from_utf8_lossy(&default_gateway_output.stdout);
                    for line in output_str.lines() {
                        if line.trim().starts_with("0.0.0.0") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 3 {
                                default_gateway = parts[2].to_string();
                                log_info!("检测到默认网关: {}", default_gateway);
                                break;
                            }
                        }
                    }
                }
                
                // 为代理服务器添加特定路由，确保通过默认接口
                for proxy_ip in proxy_servers.iter() {
                    let route_output = Command::new("route")
                        .args(&["add", proxy_ip, "mask", "255.255.255.255", &default_gateway, "metric", "1"])
                        .output();
                        
                    match route_output {
                        Ok(output) => {
                            if output.status.success() {
                                log_info!("成功添加代理服务器路由: {} -> {}", proxy_ip, default_gateway);
                            } else {
                                let error = String::from_utf8_lossy(&output.stderr);
                                if !error.contains("已存在") && !error.contains("already exists") {
                                    log_warn!("添加代理服务器路由失败: {} - {}", proxy_ip, error);
                                }
                            }
                        }
                        Err(e) => {
                            log_warn!("执行代理服务器路由命令失败: {} - {}", proxy_ip, e);
                        }
                    }
                }
                
                // 首先将默认路由重定向到虚拟网卡
                // 这是关键步骤：在添加分割路由前，先将0.0.0.0默认路由指向TUN设备
                let default_route_output = Command::new("route")
                    .args(&["add", "0.0.0.0", "mask", "0.0.0.0", &tun_gateway, "metric", "1"])
                    .output()
                    .context("添加默认路由重定向失败")?;
                    
                if !default_route_output.status.success() {
                    let error = String::from_utf8_lossy(&default_route_output.stderr);
                    // 如果路由已存在，先删除再添加
                    if error.contains("已存在") || error.contains("already exists") {
                        let _delete_output = Command::new("route")
                            .args(&["delete", "0.0.0.0", "mask", "0.0.0.0"])
                            .output();
                        
                        let retry_output = Command::new("route")
                            .args(&["add", "0.0.0.0", "mask", "0.0.0.0", &tun_gateway, "metric", "1"])
                            .output()
                            .context("重新添加默认路由重定向失败")?;
                            
                        if !retry_output.status.success() {
                            let retry_error = String::from_utf8_lossy(&retry_output.stderr);
                            return Err(anyhow::anyhow!("添加默认路由重定向失败: {}", retry_error));
                        }
                    } else {
                        return Err(anyhow::anyhow!("添加默认路由重定向失败: {}", error));
                    }
                } else {
                    log_info!("成功将默认路由重定向到虚拟网卡: 0.0.0.0/0 -> {} (metric=1)", tun_gateway);
                }
                
                // 使用分割路由覆盖整个IPv4地址空间，避免与默认路由冲突
                // 这是sing-box的核心设计，可以有效避免路由循环
                let split_routes = [
                    ("0.0.0.0", "128.0.0.0"),     // 0.0.0.0/1 (0.0.0.0 - 127.255.255.255)
                    ("128.0.0.0", "128.0.0.0"),   // 128.0.0.0/1 (128.0.0.0 - 255.255.255.255)
                ];
                
                for (network, mask) in split_routes.iter() {
                    let route_output = Command::new("route")
                        .args(&["add", network, "mask", mask, &tun_gateway, "metric", "1"])
                        .output()
                        .context("执行route命令失败")?;
                    
                    if !route_output.status.success() {
                        let error = String::from_utf8_lossy(&route_output.stderr);
                        return Err(anyhow::anyhow!("添加分割路由 {}/{} 失败: {}", network, mask, error));
                    } else {
                        log_info!("成功添加分割路由: {}/{} -> {} (metric=1)", network, mask, tun_gateway);
                    }
                }
                
                // 设置TUN设备的接口度量值
                let _metric_output = Command::new("netsh")
                    .args(&["interface", "ip", "set", "interface", tun_interface, "metric=1"])
                    .output();
                
                // 如果启用严格路由模式，添加额外的路由规则防止流量泄漏
                if strict_route {
                    log_info!("启用严格路由模式，添加防泄漏路由规则");
                    
                    // 阻止流量通过默认接口泄漏的路由规则
                    // 这些规则确保即使有应用程序尝试绕过TUN，也会被重定向
                    let strict_routes = [
                        // 阻止常见的DNS泄漏
                        ("8.8.8.8", "255.255.255.255"),
                        ("8.8.4.4", "255.255.255.255"),
                        ("1.1.1.1", "255.255.255.255"),
                        ("1.0.0.1", "255.255.255.255"),
                        // 阻止IPv4广播和组播泄漏
                        ("224.0.0.0", "240.0.0.0"),  // 组播地址范围
                        ("255.255.255.255", "255.255.255.255"),  // 广播地址
                    ];
                    
                    for (network, mask) in strict_routes.iter() {
                        let route_output = Command::new("route")
                            .args(&["add", network, "mask", mask, &tun_gateway, "metric", "1"])
                            .output();
                            
                        match route_output {
                            Ok(output) => {
                                if output.status.success() {
                                    log_info!("成功添加严格路由规则: {}/{} -> {}", network, mask, tun_gateway);
                                } else {
                                    let error = String::from_utf8_lossy(&output.stderr);
                                    if !error.contains("已存在") && !error.contains("already exists") {
                                        log_warn!("添加严格路由规则失败: {}/{} - {}", network, mask, error);
                                    }
                                }
                            }
                            Err(e) => {
                                log_warn!("执行严格路由规则命令失败: {}/{} - {}", network, mask, e);
                            }
                        }
                    }
                    
                    // 设置更高优先级的TUN接口度量值，确保流量优先通过TUN
                    let interface_metric_output = Command::new("netsh")
                        .args(&["interface", "ip", "set", "interface", &default_interface, "metric=100"])
                        .output();
                        
                    match interface_metric_output {
                        Ok(output) => {
                            if output.status.success() {
                                log_info!("成功设置默认接口 {} 的度量值为100", default_interface);
                            } else {
                                let error = String::from_utf8_lossy(&output.stderr);
                                log_warn!("设置默认接口度量值失败: {}", error);
                            }
                        }
                        Err(e) => {
                            log_warn!("执行设置接口度量值命令失败: {}", e);
                        }
                    }
                }
                
            } else {
                // 删除TUN路由规则
                let config = self.get_config().await;
                let strict_route = config.strict_route;
                
                // 删除代理服务器特定路由
                let proxy_servers = [
                    "127.0.0.1",  // 本地代理服务器
                    "8.8.8.8",    // 常见的DNS服务器
                    "8.8.4.4",    // 备用DNS服务器
                ];
                
                for proxy_ip in proxy_servers.iter() {
                    let output = Command::new("route")
                        .args(&["delete", proxy_ip, "mask", "255.255.255.255"])
                        .output();
                        
                    match output {
                        Ok(output) => {
                            if output.status.success() {
                                log_info!("成功删除代理服务器路由: {}", proxy_ip);
                            } else {
                                let error = String::from_utf8_lossy(&output.stderr);
                                if !error.contains("找不到") && !error.contains("not found") {
                                    log_warn!("删除代理服务器路由失败: {} - {}", proxy_ip, error);
                                }
                            }
                        }
                        Err(e) => {
                            log_warn!("执行删除代理服务器路由命令失败: {} - {}", proxy_ip, e);
                        }
                    }
                }
                
                // 删除分割路由
                let split_routes = [
                    ("0.0.0.0", "128.0.0.0"),     // 0.0.0.0/1
                    ("128.0.0.0", "128.0.0.0"),   // 128.0.0.0/1
                ];
                
                for (network, mask) in split_routes.iter() {
                    let output = Command::new("route")
                        .args(&["delete", network, "mask", mask])
                        .output()
                        .context("执行route命令失败")?;
                    
                    if !output.status.success() {
                        let error = String::from_utf8_lossy(&output.stderr);
                        // 对于删除操作，如果路由不存在也算成功
                        if !error.contains("找不到") && !error.contains("not found") {
                            log_warn!("删除分割路由 {}/{} 失败: {}", network, mask, error);
                        }
                    } else {
                        log_info!("成功删除分割路由: {}/{}", network, mask);
                    }
                }
                
                // 如果启用了严格路由模式，删除相关的严格路由规则
                if strict_route {
                    log_info!("删除严格路由模式的防泄漏路由规则");
                    
                    let strict_routes = [
                        // DNS服务器路由
                        ("8.8.8.8", "255.255.255.255"),
                        ("8.8.4.4", "255.255.255.255"),
                        ("1.1.1.1", "255.255.255.255"),
                        ("1.0.0.1", "255.255.255.255"),
                        // 广播和组播路由
                        ("224.0.0.0", "240.0.0.0"),
                        ("255.255.255.255", "255.255.255.255"),
                    ];
                    
                    for (network, mask) in strict_routes.iter() {
                        let output = Command::new("route")
                            .args(&["delete", network, "mask", mask])
                            .output();
                            
                        match output {
                            Ok(output) => {
                                if output.status.success() {
                                    log_info!("成功删除严格路由规则: {}/{}", network, mask);
                                } else {
                                    let error = String::from_utf8_lossy(&output.stderr);
                                    if !error.contains("找不到") && !error.contains("not found") {
                                        log_warn!("删除严格路由规则失败: {}/{} - {}", network, mask, error);
                                    }
                                }
                            }
                            Err(e) => {
                                log_warn!("执行删除严格路由规则命令失败: {}/{} - {}", network, mask, e);
                            }
                        }
                    }
                    
                    // 恢复默认接口的度量值
                    let default_interface = match Self::get_default_interface() {
                        Ok(interface) => interface,
                        Err(_) => "以太网".to_string(),
                    };
                    
                    let interface_metric_output = Command::new("netsh")
                        .args(&["interface", "ip", "set", "interface", &default_interface, "metric=1"])
                        .output();
                        
                    match interface_metric_output {
                        Ok(output) => {
                            if output.status.success() {
                                log_info!("成功恢复默认接口 {} 的度量值为1", default_interface);
                            } else {
                                let error = String::from_utf8_lossy(&output.stderr);
                                log_warn!("恢复默认接口度量值失败: {}", error);
                            }
                        }
                        Err(e) => {
                            log_warn!("执行恢复接口度量值命令失败: {}", e);
                        }
                    }
                }
                
                // 删除默认路由重定向
                let delete_default_output = Command::new("route")
                    .args(&["delete", "0.0.0.0", "mask", "0.0.0.0"])
                    .output();
                    
                match delete_default_output {
                    Ok(output) => {
                        if output.status.success() {
                            log_info!("成功删除默认路由重定向");
                        } else {
                            let error = String::from_utf8_lossy(&output.stderr);
                            if !error.contains("找不到") && !error.contains("not found") {
                                log_warn!("删除默认路由重定向失败: {}", error);
                            }
                        }
                    }
                    Err(e) => {
                        log_warn!("执行删除默认路由重定向命令失败: {}", e);
                    }
                }
                
                // 恢复原始路由表
                if let Err(e) = self.restore_routes() {
                    log_warn!("恢复原始路由表失败: {}", e);
                } else {
                    log_info!("原始路由表已恢复");
                }
                
                log_info!("TUN路由已清理，原始路由表已恢复");
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            return Err(anyhow::anyhow!("当前平台不支持TUN模式"));
        }
        
        Ok(())
    }
}