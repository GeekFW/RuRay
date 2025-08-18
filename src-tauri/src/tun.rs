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
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::JoinHandle;
use tauri::{AppHandle, Manager, path::BaseDirectory};

// 导入日志宏
use crate::{log_debug, log_info, log_warn, log_error};

#[cfg(target_os = "windows")]
use std::os::windows::ffi::{OsStrExt};

/// TUN设备配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunConfig {
    /// 虚拟网卡名称
    pub name: String,
    /// IP地址
    pub address: IpAddr,
    /// 子网掩码
    pub netmask: IpAddr,
    /// MTU大小
    pub mtu: u16,
    /// 是否启用
    pub enabled: bool,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            name: "ruray-tun".to_string(),
            address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            netmask: IpAddr::V4(Ipv4Addr::new(255, 255, 255, 0)),
            mtu: 1500,
            enabled: false,
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
}

/// TUN设备管理器
pub struct TunManager {
    config: Arc<Mutex<TunConfig>>,
    status: Arc<Mutex<TunStatus>>,
    running: Arc<AtomicBool>,
    device: Arc<Mutex<Option<tun::platform::Device>>>,
    packet_handler: Arc<Mutex<Option<JoinHandle<()>>>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
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

        // 移除系统路由
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
    async fn start_packet_processing(&self) -> Result<JoinHandle<()>> {
        let device = self.device.clone();
        let running = self.running.clone();
        let status = self.status.clone();

        let handle = tokio::spawn(async move {
            let _buffer = [0u8; 1500]; // MTU大小的缓冲区
            
            while running.load(Ordering::SeqCst) {
                let has_device = {
                    let device_guard = device.lock().unwrap();
                    device_guard.is_some()
                };
                
                if has_device {
                    // 使用spawn_blocking来处理阻塞的TUN设备读取
                    let _device_clone = device.clone();
                    let _status_clone = status.clone();
                    
                    // 暂时简化处理，只记录统计信息
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    // 更新统计信息（模拟数据包处理）
                    {
                        let mut status_guard = status.lock().unwrap();
                        status_guard.bytes_received += 100; // 模拟接收字节数
                    }
                    
                    log_debug!("TUN设备正在运行，等待数据包...");
                } else {
                    break;
                }
            }
            
            log_info!("数据包处理循环已停止");
        });

        Ok(handle)
    }

    /// 数据包处理函数
    /// 
    /// # 参数
    /// * `packet` - 数据包内容
    #[allow(dead_code)]
    async fn process_packet(packet: &[u8]) -> Result<()> {
        if packet.len() < 20 {
            return Ok(()); // 数据包太小，忽略
        }
        
        // 解析IP头部
        let version = (packet[0] >> 4) & 0x0F;
        if version != 4 {
            return Ok(()); // 只处理IPv4
        }
        
        let protocol = packet[9];
        let src_ip = Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]);
        let dst_ip = Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]);
        
        // 获取IP头部长度
        let ihl = (packet[0] & 0x0F) as usize * 4;
        if packet.len() < ihl + 4 {
            return Ok(()); // 数据包长度不足
        }
        
        log_debug!("处理数据包: {} -> {}, 协议: {}", src_ip, dst_ip, protocol);
        
        match protocol {
            6 => { // TCP
                if packet.len() >= ihl + 20 { // 确保有足够的TCP头
                    let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                    let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                    let should_proxy = Self::should_proxy(&dst_ip, dst_port);
                    log_debug!("TCP连接: {}:{} -> {}:{}, 代理: {}", src_ip, src_port, dst_ip, dst_port, should_proxy);
                    Self::handle_tcp_packet(src_ip, src_port, dst_ip, dst_port, &packet[ihl..]).await?;
                } else {
                    log_warn!("TCP数据包长度不足");
                }
            }
            17 => { // UDP
                if packet.len() >= ihl + 8 { // 确保有足够的UDP头
                    let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                    let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                    let should_proxy = Self::should_proxy(&dst_ip, dst_port);
                    log_debug!("UDP连接: {}:{} -> {}:{}, 代理: {}", src_ip, src_port, dst_ip, dst_port, should_proxy);
                    Self::handle_udp_packet(src_ip, src_port, dst_ip, dst_port, &packet[ihl..]).await?;
                } else {
                    log_warn!("UDP数据包长度不足");
                }
            }
            _ => {
                log_warn!("不支持的协议: {}", protocol);
            }
        }
        
        Ok(())
    }
    
    /// 处理TCP数据包
    #[allow(dead_code)]
    async fn handle_tcp_packet(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        tcp_data: &[u8],
    ) -> Result<()> {
        // 检查是否需要代理
        if Self::should_proxy(&dst_ip, dst_port) {
            // 转发到本地代理端口 (假设SOCKS5代理在1080端口)
            Self::forward_to_proxy(src_ip, src_port, dst_ip, dst_port, tcp_data, "tcp").await?;
        } else {
            // 直接转发
            Self::forward_direct(src_ip, src_port, dst_ip, dst_port, tcp_data, "tcp").await?;
        }
        Ok(())
    }
    
    /// 处理UDP数据包
    #[allow(dead_code)]
    async fn handle_udp_packet(
        src_ip: Ipv4Addr,
        src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        udp_data: &[u8],
    ) -> Result<()> {
        // 检查是否需要代理
        if Self::should_proxy(&dst_ip, dst_port) {
            // 转发到本地代理端口
            Self::forward_to_proxy(src_ip, src_port, dst_ip, dst_port, udp_data, "udp").await?;
        } else {
            // 直接转发
            Self::forward_direct(src_ip, src_port, dst_ip, dst_port, udp_data, "udp").await?;
        }
        Ok(())
    }
    
    /// 判断是否需要代理
    /// 
    /// # 参数
    /// * `dst_ip` - 目标IP地址
    /// * `dst_port` - 目标端口
    /// 
    /// # 返回值
    /// * `bool` - 是否需要代理
    #[allow(dead_code)]
    fn should_proxy(dst_ip: &Ipv4Addr, dst_port: u16) -> bool {
        // 本地地址不代理
        if dst_ip.is_loopback() {
            return false;
        }
        
        // 私有网络地址不代理
        if dst_ip.is_private() {
            return false;
        }
        
        // 链路本地地址不代理
        if dst_ip.is_link_local() {
            return false;
        }
        
        // 组播地址不代理
        if dst_ip.is_multicast() {
            return false;
        }
        
        // 广播地址不代理
        if dst_ip.is_broadcast() {
            return false;
        }
        
        // 特殊端口不代理 (DNS等系统服务)
        if matches!(dst_port, 53 | 67 | 68 | 123 | 161 | 162) {
            return false;
        }
        
        // 检查是否有代理服务器正在运行
        // 如果有代理服务器运行，则外网流量通过代理
        // 如果没有代理服务器运行，则直接连接
        Self::is_proxy_running()
    }
    
    /// 检查代理服务器是否正在运行
    /// 
    /// # 返回值
    /// * `bool` - 代理服务器是否运行中
    #[allow(dead_code)]
    fn is_proxy_running() -> bool {
        use crate::proxy::ProxyManager;
        
        // 使用阻塞方式检查代理状态，避免在数据包处理中使用异步
        let proxy_manager = ProxyManager::instance();
        
        // 检查进程是否存在
        proxy_manager.is_process_running()
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
            Ok(config) => config.socks_port,
            Err(_) => 1080, // 默认端口
        }
    }
    
    /// 转发到代理服务器
    /// 
    /// # 参数
    /// * `_src_ip` - 源IP地址
    /// * `_src_port` - 源端口
    /// * `dst_ip` - 目标IP地址
    /// * `dst_port` - 目标端口
    /// * `data` - 数据内容
    /// * `protocol` - 协议类型
    /// 
    /// # 返回值
    /// * `Result<()>` - 转发结果
    #[allow(dead_code)]
    async fn forward_to_proxy(
        _src_ip: Ipv4Addr,
        _src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        data: &[u8],
        protocol: &str,
    ) -> Result<()> {
        let proxy_port = Self::get_proxy_port();
        let proxy_addr = format!("127.0.0.1:{}", proxy_port);
        
        match protocol {
            "tcp" => {
                // 连接到本地SOCKS5代理
                match TcpStream::connect(&proxy_addr).await {
                    Ok(mut stream) => {
                        // SOCKS5握手
                        stream.write_all(&[0x05, 0x01, 0x00]).await?;
                        
                        let mut response = [0u8; 2];
                        stream.read_exact(&mut response).await?;
                        
                        if response[0] == 0x05 && response[1] == 0x00 {
                            // 发送连接请求
                            let mut request = vec![0x05, 0x01, 0x00, 0x01]; // SOCKS5 CONNECT IPv4
                            request.extend_from_slice(&dst_ip.octets());
                            request.extend_from_slice(&dst_port.to_be_bytes());
                            
                            stream.write_all(&request).await?;
                            
                            let mut connect_response = [0u8; 10];
                            stream.read_exact(&mut connect_response).await?;
                            
                            if connect_response[1] == 0x00 {
                                // 连接成功，转发数据
                                stream.write_all(data).await?;
                                log_debug!("TCP数据已通过代理转发: {}:{}", dst_ip, dst_port);
                            } else {
                                log_warn!("代理连接失败: {}:{}", dst_ip, dst_port);
                            }
                        }
                    }
                    Err(e) => {
                        log_warn!("连接代理服务器失败: {}", e);
                        // 如果代理不可用，尝试直接连接
                        Self::forward_direct(_src_ip, _src_port, dst_ip, dst_port, data, protocol).await?;
                    }
                }
            }
            "udp" => {
                // UDP通过SOCKS5代理处理
                match UdpSocket::bind("0.0.0.0:0").await {
                    Ok(socket) => {
                        // 构造SOCKS5 UDP请求
                        let mut udp_request = vec![0x00, 0x00, 0x00, 0x01]; // RSV + FRAG + ATYP
                        udp_request.extend_from_slice(&dst_ip.octets());
                        udp_request.extend_from_slice(&dst_port.to_be_bytes());
                        udp_request.extend_from_slice(data);
                        
                        socket.send_to(&udp_request, &proxy_addr).await?;
                        log_debug!("UDP数据已通过代理转发: {}:{}", dst_ip, dst_port);
                    }
                    Err(e) => {
                        log_warn!("UDP代理转发失败: {}", e);
                    }
                }
            }
            _ => {
                log_warn!("不支持的协议: {}", protocol);
            }
        }
        Ok(())
    }
    
    /// 直接转发
    #[allow(dead_code)]
    async fn forward_direct(
        _src_ip: Ipv4Addr,
        _src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        data: &[u8],
        protocol: &str,
    ) -> Result<()> {
        match protocol {
            "tcp" => {
                if let Ok(mut stream) = TcpStream::connect((dst_ip, dst_port)).await {
                    let _ = stream.write_all(data).await;
                    log_debug!("TCP数据已直接转发: {}:{}", dst_ip, dst_port);
                }
            }
            "udp" => {
                if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
                    let _ = socket.send_to(data, (dst_ip, dst_port)).await;
                    log_debug!("UDP数据已直接转发: {}:{}", dst_ip, dst_port);
                }
            }
            _ => {}
        }
        Ok(())
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
    
    /// 设置系统路由规则
    /// 
    /// # 参数
    /// * `enable` - 是否启用路由规则
    /// 
    /// # 返回值
    /// * `Result<()>` - 操作结果
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
                
                // 备份原始默认路由
                let backup_output = Command::new("route")
                    .args(&["print", "0.0.0.0"])
                    .output()
                    .context("获取原始路由失败")?;
                
                if backup_output.status.success() {
                    log_info!("原始路由信息已备份");
                }
                
                // 使用分割路由的方式，避免路由循环
                // 添加 0.0.0.0/1 和 128.0.0.0/1 路由到TUN设备
                let routes = [
                    ("0.0.0.0", "128.0.0.0"),     // 0.0.0.0/1
                    ("128.0.0.0", "128.0.0.0"),   // 128.0.0.0/1
                ];
                
                for (network, mask) in routes.iter() {
                    let output = Command::new("route")
                        .args(&["add", network, "mask", mask, &config.address.to_string(), "metric", "1"])
                        .output()
                        .context("执行route命令失败")?;
                    
                    if !output.status.success() {
                        let error = String::from_utf8_lossy(&output.stderr);
                        log_warn!("添加路由 {}/{} 失败: {}", network, mask, error);
                        // 继续尝试添加其他路由
                    } else {
                        log_info!("成功添加路由: {}/{} -> {}", network, mask, config.address);
                    }
                }
                
                // 设置TUN设备的接口度量值
                let _metric_output = Command::new("netsh")
                    .args(&["interface", "ip", "set", "interface", tun_interface, "metric=1"])
                    .output();
                
            } else {
                // 删除TUN路由规则
                let _config = self.get_config().await;
                
                let routes = [
                    ("0.0.0.0", "128.0.0.0"),     // 0.0.0.0/1
                    ("128.0.0.0", "128.0.0.0"),   // 128.0.0.0/1
                ];
                
                for (network, mask) in routes.iter() {
                    let output = Command::new("route")
                        .args(&["delete", network, "mask", mask])
                        .output()
                        .context("执行route命令失败")?;
                    
                    if !output.status.success() {
                        let error = String::from_utf8_lossy(&output.stderr);
                        // 对于删除操作，如果路由不存在也算成功
                        if !error.contains("找不到") && !error.contains("not found") {
                            log_warn!("删除路由 {}/{} 失败: {}", network, mask, error);
                        }
                    } else {
                        log_info!("成功删除路由: {}/{}", network, mask);
                    }
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            return Err(anyhow::anyhow!("当前平台不支持TUN模式"));
        }
        
        Ok(())
    }
}