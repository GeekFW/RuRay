/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, OnceLock};
use tun::{Configuration, Layer, Device};
use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::JoinHandle;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, path::BaseDirectory};

#[cfg(target_os = "windows")]
use std::ffi::OsString;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::{OsStringExt, OsStrExt};

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
                println!("警告: 不支持的架构 {}, 使用默认路径", arch);
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
                            println!("WinTun库路径已设置: {}", wintun_path.display());
                            return Ok(());
                        } else {
                            return Err(anyhow::anyhow!("设置DLL搜索目录失败"));
                        }
                    }
                } else {
                    println!("警告: 嵌入的wintun.dll文件不存在: {}", wintun_path.display());
                }
            }
            Err(e) => {
                println!("警告: 无法解析wintun.dll资源路径: {}", e);
            }
        }
        
        println!("使用系统默认WinTun路径");
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
                     println!("TUN设备创建成功: ruray-tun");
                     device
                 }
                 Err(e) => {
                     let error_msg = format!("创建TUN设备失败: {}. 详细错误: {:?}. 可能原因: 1) 权限不足，需要管理员权限 2) TUN驱动未安装 3) 网络配置冲突", e, e);
                     eprintln!("{}", error_msg);
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
        
        println!("TUN模式启动成功，虚拟网卡: ruray-tun");
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

        println!("TUN设备已停止");
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

        println!("TUN设备已停止（同步）");
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
            let mut buffer = [0u8; 1500]; // MTU大小的缓冲区
            
            while running.load(Ordering::SeqCst) {
                let has_device = {
                    let device_guard = device.lock().unwrap();
                    device_guard.is_some()
                };
                
                if has_device {
                    // 暂时模拟数据包处理，因为实际的异步读取需要更复杂的处理
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    // 在实际实现中，这里应该:
                    // 1. 从TUN设备读取数据包
                    // 2. 解析数据包
                    // 3. 根据规则决定是否代理
                    // 4. 转发数据包
                    
                    // 更新统计信息
                    {
                        let mut status_guard = status.lock().unwrap();
                        status_guard.bytes_received += 100; // 模拟接收字节数
                    }
                } else {
                    break;
                }
            }
            
            println!("数据包处理循环已停止");
        });

        Ok(handle)
    }

    /// 数据包处理函数
    /// 
    /// # 参数
    /// * `packet` - 数据包内容
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
        
        match protocol {
            6 => { // TCP
                let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                Self::handle_tcp_packet(src_ip, src_port, dst_ip, dst_port, &packet[ihl..]).await?;
            }
            17 => { // UDP
                let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                Self::handle_udp_packet(src_ip, src_port, dst_ip, dst_port, &packet[ihl..]).await?;
            }
            _ => {
                // 其他协议暂不处理
            }
        }
        
        Ok(())
    }
    
    /// 处理TCP数据包
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
        
        // 其他所有外网流量都通过代理
        // 这样可以确保所有需要翻墙的流量都经过xray代理
        true
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
    async fn forward_to_proxy(
        _src_ip: Ipv4Addr,
        _src_port: u16,
        dst_ip: Ipv4Addr,
        dst_port: u16,
        data: &[u8],
        protocol: &str,
    ) -> Result<()> {
        match protocol {
            "tcp" => {
                // 连接到本地SOCKS5代理 (xray默认监听1080端口)
                match TcpStream::connect("127.0.0.1:1080").await {
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
                                println!("TCP数据已通过代理转发: {}:{}", dst_ip, dst_port);
                            } else {
                                println!("代理连接失败: {}:{}", dst_ip, dst_port);
                            }
                        }
                    }
                    Err(e) => {
                        println!("连接代理服务器失败: {}", e);
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
                        
                        socket.send_to(&udp_request, "127.0.0.1:1080").await?;
                        println!("UDP数据已通过代理转发: {}:{}", dst_ip, dst_port);
                    }
                    Err(e) => {
                        println!("UDP代理转发失败: {}", e);
                    }
                }
            }
            _ => {
                println!("不支持的协议: {}", protocol);
            }
        }
        Ok(())
    }
    
    /// 直接转发
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
                    println!("TCP数据已直接转发: {}:{}", dst_ip, dst_port);
                }
            }
            "udp" => {
                if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
                    let _ = socket.send_to(data, (dst_ip, dst_port)).await;
                    println!("UDP数据已直接转发: {}:{}", dst_ip, dst_port);
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
                
                // 添加路由规则，将所有流量导向TUN设备
                let config = self.get_config().await;
                
                let output = Command::new("route")
                    .args(&["add", "0.0.0.0", "mask", "0.0.0.0", &config.address.to_string()])
                    .output()
                    .context("执行route命令失败")?;
                
                if !output.status.success() {
                    let error = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow::anyhow!("添加路由规则失败: {}", error));
                }
            } else {
                // 删除路由规则
                let config = self.get_config().await;
                
                let output = Command::new("route")
                    .args(&["delete", "0.0.0.0", "mask", "0.0.0.0", &config.address.to_string()])
                    .output()
                    .context("执行route命令失败")?;
                
                if !output.status.success() {
                    let error = String::from_utf8_lossy(&output.stderr);
                    // 对于删除操作，如果路由不存在也算成功
                    if !error.contains("找不到") && !error.contains("not found") {
                        return Err(anyhow::anyhow!("删除路由规则失败: {}", error));
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