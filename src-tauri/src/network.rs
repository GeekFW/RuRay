/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2025-01-15
 */

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use crate::{log_debug, log_error};

#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetIfTable2, FreeMibTable, MIB_IF_TABLE2, MIB_IF_ROW2,
};

/// 网络接口统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceStats {
    pub name: String,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}

/// 网络速度统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSpeedStats {
    pub upload_speed: u64,    // bytes/s
    pub download_speed: u64,  // bytes/s
    pub total_upload: u64,    // total bytes uploaded since start
    pub total_download: u64,  // total bytes downloaded since start
}

/// 网络统计历史记录
#[derive(Debug, Clone)]
struct NetworkSnapshot {
    timestamp: Instant,
    total_sent: u64,
    total_received: u64,
}

/// 网络统计管理器
pub struct NetworkStatsManager {
    /// 上一次的网络统计快照
    last_snapshot: Arc<Mutex<Option<NetworkSnapshot>>>,
    /// 程序启动时的基准统计
    baseline_stats: Arc<Mutex<Option<NetworkSnapshot>>>,
    /// 当前速度统计
    current_speed: Arc<Mutex<NetworkSpeedStats>>,
    /// 统计任务句柄
    pub stats_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

// 全局单例实例
static NETWORK_STATS_MANAGER: OnceLock<NetworkStatsManager> = OnceLock::new();

impl NetworkStatsManager {
    /// 获取全局网络统计管理器实例（单例模式）
    pub fn instance() -> &'static NetworkStatsManager {
        NETWORK_STATS_MANAGER.get_or_init(|| {
            Self {
                last_snapshot: Arc::new(Mutex::new(None)),
                baseline_stats: Arc::new(Mutex::new(None)),
                current_speed: Arc::new(Mutex::new(NetworkSpeedStats {
                    upload_speed: 0,
                    download_speed: 0,
                    total_upload: 0,
                    total_download: 0,
                })),
                stats_task: Arc::new(Mutex::new(None)),
            }
        })
    }

    /// 启动网络统计监控
    pub async fn start_monitoring(&self) -> Result<()> {
        log_debug!("start_monitoring 被调用");
        // 停止现有的监控任务
        self.stop_monitoring().await;

        // 获取初始基准统计
        let initial_stats = self.get_total_network_stats().await?;
        let baseline = NetworkSnapshot {
            timestamp: Instant::now(),
            total_sent: initial_stats.iter().map(|s| s.bytes_sent).sum(),
            total_received: initial_stats.iter().map(|s| s.bytes_received).sum(),
        };

        // 设置基准统计
        *self.baseline_stats.lock().unwrap() = Some(baseline.clone());
        *self.last_snapshot.lock().unwrap() = Some(baseline);

        // 启动定时统计任务
        let last_snapshot = Arc::clone(&self.last_snapshot);
        let baseline_stats = Arc::clone(&self.baseline_stats);
        let current_speed = Arc::clone(&self.current_speed);

        log_debug!("启动定时统计任务，每1秒执行一次");
        let task = std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                
                // 直接调用同步版本的网络统计更新
                let result = Self::update_network_stats_sync(
                    &last_snapshot,
                    &baseline_stats,
                    &current_speed,
                );
            }
        });
        
        let task_handle = tokio::task::spawn_blocking(move || {
            if let Err(e) = task.join() {
                log_error!("定时器线程异常退出: {:?}", e);
            }
        });
        
        *self.stats_task.lock().unwrap() = Some(task_handle);
        log_debug!("网络统计监控任务已启动");
        Ok(())
    }

    /// 停止网络统计监控
    pub async fn stop_monitoring(&self) {
        if let Some(task) = self.stats_task.lock().unwrap().take() {
            task.abort();
        }
    }

    /// 获取当前网络速度统计
    pub fn get_current_speed(&self) -> NetworkSpeedStats {
        self.current_speed.lock().unwrap().clone()
    }

    /// 更新网络统计（同步版本）
    fn update_network_stats_sync(
        last_snapshot: &Arc<Mutex<Option<NetworkSnapshot>>>,
        baseline_stats: &Arc<Mutex<Option<NetworkSnapshot>>>,
        current_speed: &Arc<Mutex<NetworkSpeedStats>>,
    ) -> Result<()> {
        // 获取当前网络统计
        let current_stats = Self::get_total_network_stats_sync()?;
        let current_total_sent: u64 = current_stats.iter().map(|s| s.bytes_sent).sum();
        let current_total_received: u64 = current_stats.iter().map(|s| s.bytes_received).sum();
        let now = Instant::now();

        let mut last_snapshot_guard = last_snapshot.lock().unwrap();
        let baseline_guard = baseline_stats.lock().unwrap();
        
        if let (Some(last), Some(baseline)) = (last_snapshot_guard.as_ref(), baseline_guard.as_ref()) {
            let time_diff = now.duration_since(last.timestamp).as_secs_f64();
            
            if time_diff > 0.0 {
                // 计算速度（bytes/s）
                let sent_diff = current_total_sent.saturating_sub(last.total_sent);
                let received_diff = current_total_received.saturating_sub(last.total_received);
                let upload_speed = (sent_diff as f64 / time_diff) as u64;
                let download_speed = (received_diff as f64 / time_diff) as u64;
                
                // 计算总流量（从程序启动开始）
                let total_upload = current_total_sent.saturating_sub(baseline.total_sent);
                let total_download = current_total_received.saturating_sub(baseline.total_received);
                
                // 更新当前速度统计
                *current_speed.lock().unwrap() = NetworkSpeedStats {
                    upload_speed,
                    download_speed,
                    total_upload,
                    total_download,
                };
            }
        } else {
            log_debug!("缺少快照数据 - last: {}, baseline: {}", 
                last_snapshot_guard.is_some(), baseline_guard.is_some());
        }

        // 更新快照
        *last_snapshot_guard = Some(NetworkSnapshot {
            timestamp: now,
            total_sent: current_total_sent,
            total_received: current_total_received,
        });

        Ok(())
    }

    /// 更新网络统计（内部方法）
    async fn update_network_stats(
        last_snapshot: &Arc<Mutex<Option<NetworkSnapshot>>>,
        baseline_stats: &Arc<Mutex<Option<NetworkSnapshot>>>,
        current_speed: &Arc<Mutex<NetworkSpeedStats>>,
    ) -> Result<()> {
        // 获取当前网络统计
        let current_stats = Self::get_total_network_stats_static().await?;
        let current_total_sent: u64 = current_stats.iter().map(|s| s.bytes_sent).sum();
        let current_total_received: u64 = current_stats.iter().map(|s| s.bytes_received).sum();
        let now = Instant::now();

        let mut last_snapshot_guard = last_snapshot.lock().unwrap();
        let baseline_guard = baseline_stats.lock().unwrap();
        
        if let (Some(last), Some(baseline)) = (last_snapshot_guard.as_ref(), baseline_guard.as_ref()) {
            let time_diff = now.duration_since(last.timestamp).as_secs_f64();
            
            if time_diff > 0.0 {
                // 计算速度（bytes/s）
                let sent_diff = current_total_sent.saturating_sub(last.total_sent);
                let received_diff = current_total_received.saturating_sub(last.total_received);
                let upload_speed = (sent_diff as f64 / time_diff) as u64;
                let download_speed = (received_diff as f64 / time_diff) as u64;
                
                // 计算总流量（从程序启动开始）
                let total_upload = current_total_sent.saturating_sub(baseline.total_sent);
                let total_download = current_total_received.saturating_sub(baseline.total_received);
                
                // 更新当前速度统计
                *current_speed.lock().unwrap() = NetworkSpeedStats {
                    upload_speed,
                    download_speed,
                    total_upload,
                    total_download,
                };
            }
        } else {
            log_debug!("缺少快照数据 - last: {}, baseline: {}", 
                last_snapshot_guard.is_some(), baseline_guard.is_some());
        }

        // 更新快照
        *last_snapshot_guard = Some(NetworkSnapshot {
            timestamp: now,
            total_sent: current_total_sent,
            total_received: current_total_received,
        });

        Ok(())
    }

    /// 获取所有网络接口的统计信息
    pub async fn get_total_network_stats(&self) -> Result<Vec<NetworkInterfaceStats>> {
        Self::get_total_network_stats_static().await
    }

    /// 获取所有网络接口的统计信息（同步版本）
    fn get_total_network_stats_sync() -> Result<Vec<NetworkInterfaceStats>> {
        #[cfg(target_os = "windows")]
        {
            Self::get_windows_network_stats_sync()
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // 对于非Windows系统，使用sysinfo库作为备选方案
            Self::get_sysinfo_network_stats_sync()
        }
    }

    /// 获取所有网络接口的统计信息（静态方法）
    async fn get_total_network_stats_static() -> Result<Vec<NetworkInterfaceStats>> {
        #[cfg(target_os = "windows")]
        {
            Self::get_windows_network_stats().await
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // 对于非Windows系统，使用sysinfo库作为备选方案
            Self::get_sysinfo_network_stats().await
        }
    }

    /// 获取Windows网络接口统计信息（同步版本）
    #[cfg(target_os = "windows")]
    fn get_windows_network_stats_sync() -> Result<Vec<NetworkInterfaceStats>> {
        use std::ptr;
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let mut interfaces = Vec::new();
        let mut table_ptr: *mut MIB_IF_TABLE2 = ptr::null_mut();

        unsafe {
            let result = GetIfTable2(&mut table_ptr);
            if result != 0 {
                return Err(anyhow::anyhow!("获取网络接口表失败: {}", result));
            }

            if table_ptr.is_null() {
                return Err(anyhow::anyhow!("网络接口表为空"));
            }

            let table = &*table_ptr;
            let entries = std::slice::from_raw_parts(
                table.Table.as_ptr(),
                table.NumEntries as usize,
            );

            for (i, entry) in entries.iter().enumerate() {
                // 只统计活跃的网络接口
                if entry.OperStatus == 1 { // IfOperStatusUp
                    // 转换接口名称
                    let name_slice = std::slice::from_raw_parts(
                        entry.Alias.as_ptr(),
                        entry.Alias.len(),
                    );
                    let name = OsString::from_wide(name_slice)
                        .to_string_lossy()
                        .trim_end_matches('\0')
                        .to_string();
                    
                    // 过滤掉虚拟接口和回环接口
                    if !name.is_empty() && 
                       !name.contains("Loopback") && 
                       !name.contains("Teredo") &&
                       !name.contains("isatap") {
                        
                        // 检查是否为物理网络接口（通常包含 "Ethernet" 或 "Wi-Fi" 或 "WLAN"）
                        let is_physical = name.contains("Ethernet") || 
                                        name.contains("Wi-Fi") || 
                                        name.contains("WLAN") ||
                                        name.contains("Local Area Connection") ||
                                        name.contains("Wireless Network Connection");
                        
                        if is_physical {
                            interfaces.push(NetworkInterfaceStats {
                                name,
                                bytes_sent: entry.OutOctets,
                                bytes_received: entry.InOctets,
                                packets_sent: entry.OutUcastPkts + entry.OutNUcastPkts,
                                packets_received: entry.InUcastPkts + entry.InNUcastPkts,
                            });
                        }
                    }
                }
            }

            // 释放内存
            FreeMibTable(table_ptr as *mut _);
        }

        Ok(interfaces)
    }

    /// 获取Windows网络接口统计信息
    #[cfg(target_os = "windows")]
    async fn get_windows_network_stats() -> Result<Vec<NetworkInterfaceStats>> {
        use std::ptr;
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        log_debug!("开始获取Windows网络接口统计信息");
        let mut interfaces = Vec::new();
        let mut table_ptr: *mut MIB_IF_TABLE2 = ptr::null_mut();

        unsafe {
            let result = GetIfTable2(&mut table_ptr);
            if result != 0 {
                return Err(anyhow::anyhow!("获取网络接口表失败: {}", result));
            }

            if table_ptr.is_null() {
                return Err(anyhow::anyhow!("网络接口表为空"));
            }

            let table = &*table_ptr;
            let entries = std::slice::from_raw_parts(
                table.Table.as_ptr(),
                table.NumEntries as usize,
            );

            for (i, entry) in entries.iter().enumerate() {
                // 只统计活跃的网络接口
                if entry.OperStatus == 1 { // IfOperStatusUp
                    // 转换接口名称
                    let name_slice = std::slice::from_raw_parts(
                        entry.Alias.as_ptr(),
                        entry.Alias.len(),
                    );
                    let name = OsString::from_wide(name_slice)
                        .to_string_lossy()
                        .trim_end_matches('\0')
                        .to_string();

                    // 过滤掉虚拟接口和过滤器驱动，只保留真正的物理接口
                    if !name.contains("Filter") && !name.contains("Virtual") && 
                       !name.contains("Loopback") && !name.contains("Teredo") &&
                       !name.contains("isatap") && !name.contains("Scheduler") {
                        interfaces.push(NetworkInterfaceStats {
                            name,
                            bytes_sent: entry.OutOctets,
                            bytes_received: entry.InOctets,
                            packets_sent: entry.OutUcastPkts + entry.OutNUcastPkts,
                            packets_received: entry.InUcastPkts + entry.InNUcastPkts,
                        });
                    }
                }
            }
            
            FreeMibTable(table_ptr as *mut _);
        }

        Ok(interfaces)
    }

    /// 使用sysinfo库获取网络统计信息（同步版本）
    #[cfg(not(target_os = "windows"))]
    fn get_sysinfo_network_stats_sync() -> Result<Vec<NetworkInterfaceStats>> {
        use sysinfo::{Networks, NetworkExt};
        
        let mut networks = Networks::new_with_refreshed_list();
        networks.refresh();
        
        let mut interfaces = Vec::new();
        
        for (interface_name, network) in networks.iter() {
            interfaces.push(NetworkInterfaceStats {
                name: interface_name.clone(),
                bytes_sent: network.transmitted(),
                bytes_received: network.received(),
                packets_sent: network.packets_transmitted(),
                packets_received: network.packets_received(),
            });
        }
        
        Ok(interfaces)
    }

    /// 使用sysinfo库获取网络统计信息（备选方案）
    #[cfg(not(target_os = "windows"))]
    async fn get_sysinfo_network_stats() -> Result<Vec<NetworkInterfaceStats>> {
        use sysinfo::{Networks, NetworkExt};
        
        let mut networks = Networks::new_with_refreshed_list();
        networks.refresh();
        
        let mut interfaces = Vec::new();
        
        for (interface_name, network) in networks.iter() {
            interfaces.push(NetworkInterfaceStats {
                name: interface_name.clone(),
                bytes_sent: network.transmitted(),
                bytes_received: network.received(),
                packets_sent: network.packets_transmitted(),
                packets_received: network.packets_received(),
            });
        }
        
        Ok(interfaces)
    }

    /// 重置统计数据
    pub async fn reset_stats(&self) -> Result<()> {
        // 获取当前统计作为新的基准
        let current_stats = self.get_total_network_stats().await?;
        let new_baseline = NetworkSnapshot {
            timestamp: Instant::now(),
            total_sent: current_stats.iter().map(|s| s.bytes_sent).sum(),
            total_received: current_stats.iter().map(|s| s.bytes_received).sum(),
        };

        *self.baseline_stats.lock().unwrap() = Some(new_baseline.clone());
        *self.last_snapshot.lock().unwrap() = Some(new_baseline);
        
        // 重置当前速度统计
        *self.current_speed.lock().unwrap() = NetworkSpeedStats {
            upload_speed: 0,
            download_speed: 0,
            total_upload: 0,
            total_download: 0,
        };

        Ok(())
    }
}

/// 初始化网络统计管理器
pub fn init_network_stats() {
    log_debug!("========== 开始初始化网络统计管理器 ==========");
    
    let handle = std::thread::spawn(|| {
        log_debug!("========== 网络统计线程已启动 ==========");
        
        match tokio::runtime::Runtime::new() {
            Ok(rt) => {
                log_debug!("Tokio运行时创建成功");
                rt.block_on(async {
                    log_debug!("========== 开始启动网络统计监控 ==========");
                    let manager = NetworkStatsManager::instance();
                    match manager.start_monitoring().await {
                        Ok(_) => {
                            log_debug!("========== 网络统计监控启动成功 ==========");
                        },
                        Err(e) => {
                            log_error!("启动网络统计监控失败: {}", e);
                        }
                    }
                });
            },
            Err(e) => {
                log_error!("创建Tokio运行时失败: {}", e);
            }
        }
    });
    
    log_debug!("网络统计线程句柄创建完成: {:?}", handle.thread().id());
}

/// 获取当前网络速度统计
pub fn get_network_speed() -> Result<NetworkSpeedStats> {
    let manager = NetworkStatsManager::instance();
    Ok(manager.get_current_speed())
}

/// 重置网络统计
pub async fn reset_network_stats() -> Result<()> {
    NetworkStatsManager::instance().reset_stats().await
}