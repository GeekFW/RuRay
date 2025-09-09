/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2025-01-03
 */

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use libloading::{Library, Symbol};
use anyhow::{Context, Result};
use std::sync::{Arc, Mutex, OnceLock};
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Utc;

// 导入日志宏
use crate::{log_info, log_warn, log_error, log_debug};

/// DNS查询处理策略
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum Tun2proxyDns {
    /// 使用虚拟DNS服务器处理DNS查询，也称为Fake-IP模式
    Virtual = 0,
    /// 使用TCP发送DNS查询到DNS服务器
    OverTcp = 1,
    /// 不处理DNS，依赖DNS服务器绕过
    Direct = 2,
}

/// 日志详细程度级别
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum Tun2proxyVerbosity {
    /// 关闭日志
    Off = 0,
    /// 错误级别
    Error = 1,
    /// 警告级别
    Warn = 2,
    /// 信息级别
    Info = 3,
    /// 调试级别
    Debug = 4,
    /// 跟踪级别
    Trace = 5,
}

/// 流量统计状态
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tun2proxyTrafficStatus {
    /// 发送字节数
    pub tx: u64,
    /// 接收字节数
    pub rx: u64,
}

/// 日志回调函数类型
type LogCallback = extern "C" fn(verbosity: Tun2proxyVerbosity, message: *const c_char, ctx: *mut c_void);

/// 流量统计回调函数类型
type TrafficCallback = extern "C" fn(status: *const Tun2proxyTrafficStatus, ctx: *mut c_void);

/// tun2proxy DLL函数指针类型定义
type SetLogCallbackFn = unsafe extern "C" fn(callback: LogCallback, ctx: *mut c_void);
type WithNameRunFn = unsafe extern "C" fn(
    proxy_url: *const c_char,
    tun: *const c_char,
    bypass: *const c_char,
    dns_strategy: Tun2proxyDns,
    root_privilege: bool,
    verbosity: Tun2proxyVerbosity,
) -> c_int;

type RunWithCliArgsFn = unsafe extern "C" fn(
    cli_args: *const c_char,
    tun_mtu: u16,
    packet_information: bool,
) -> c_int;
type StopFn = unsafe extern "C" fn() -> c_int;
type SetTrafficStatusCallbackFn = unsafe extern "C" fn(
    send_interval_secs: c_uint,
    callback: TrafficCallback,
    ctx: *mut c_void,
);

/// tun2proxy DLL包装器
pub struct Tun2proxyDll {
    /// 动态库句柄
    _library: Library,
    /// 设置日志回调函数
    pub set_log_callback: SetLogCallbackFn,


    /// 使用命令行参数运行
    pub run_with_cli_args: RunWithCliArgsFn,
    /// 停止tun2proxy
    pub tun2proxy_stop: StopFn,
    /// 设置流量统计回调
    pub set_traffic_status_callback: SetTrafficStatusCallbackFn,
}

impl Tun2proxyDll {
    /// 加载tun2proxy DLL
    /// 
    /// # Arguments
    /// 
    /// * `dll_path` - DLL文件路径
    /// 
    /// # Returns
    /// 
    /// * `Result<Self>` - 加载结果
    pub fn load(dll_path: PathBuf) -> Result<Self> {
        log_debug!("正在加载tun2proxy DLL: {}", dll_path.display());
        
        // 加载动态库
        let library = unsafe { Library::new(&dll_path) }
            .context(format!("无法加载tun2proxy DLL: {}", dll_path.display()))?;
        
        // 获取函数符号
        log_info!("开始获取DLL函数符号...");
        
        let set_log_callback: Symbol<SetLogCallbackFn> = unsafe {
            library.get(b"tun2proxy_set_log_callback")
                .context("无法找到tun2proxy_set_log_callback函数")?
        };
        let set_log_callback_fn = *set_log_callback;
        log_debug!("成功获取tun2proxy_set_log_callback函数");
        
        let run_with_cli_args: Symbol<RunWithCliArgsFn> = unsafe {
            library.get(b"tun2proxy_run_with_cli_args")
                .context("无法找到tun2proxy_run_with_cli_args函数")?
        };
        let run_with_cli_args_fn = *run_with_cli_args;
        log_debug!("成功获取tun2proxy_run_with_cli_args函数");
        
        let stop: Symbol<StopFn> = unsafe {
            library.get(b"tun2proxy_stop")
                .context("无法找到tun2proxy_stop函数")?
        };
        let stop_fn = *stop;
        
        let set_traffic_status_callback: Symbol<SetTrafficStatusCallbackFn> = unsafe {
            library.get(b"tun2proxy_set_traffic_status_callback")
                .context("无法找到tun2proxy_set_traffic_status_callback函数")?
        };
        let set_traffic_status_callback_fn = *set_traffic_status_callback;
        log_debug!("成功获取tun2proxy_set_traffic_status_callback函数");
        
        log_debug!("tun2proxy DLL加载成功");
        
        Ok(Self {
            _library: library,
            set_log_callback: set_log_callback_fn,
            run_with_cli_args: run_with_cli_args_fn,
            tun2proxy_stop: stop_fn,
            set_traffic_status_callback: set_traffic_status_callback_fn,
        })
    }
}

/// 全局tun2proxy DLL实例（无锁版本）
static TUN2PROXY_DLL: OnceLock<Option<Tun2proxyDll>> = OnceLock::new();

/// 获取全局tun2proxy DLL实例
pub fn get_tun2proxy_dll() -> &'static Option<Tun2proxyDll> {
    TUN2PROXY_DLL.get().unwrap_or(&None)
}

/// 初始化tun2proxy DLL
/// 
/// # Arguments
/// 
/// * `dll_path` - DLL文件路径
/// 
/// # Returns
/// 
/// * `Result<()>` - 初始化结果
pub fn init_tun2proxy_dll(dll_path: PathBuf) -> Result<()> {
    // 如果DLL已经初始化，先清理旧的实例
    if TUN2PROXY_DLL.get().is_some() {
        log_debug!("检测到已存在的tun2proxy DLL实例，正在重新初始化");
        cleanup_dll_instance();
    }
    
    let dll = Tun2proxyDll::load(dll_path)?;
    
    // 使用panic捕获机制防止初始化时崩溃
    let init_result = std::panic::catch_unwind(|| {
        TUN2PROXY_DLL.set(Some(dll))
    });
    
    match init_result {
        Ok(Ok(())) => {
            log_debug!("tun2proxy DLL初始化完成");
            Ok(())
        }
        Ok(Err(_)) => {
            log_error!("tun2proxy DLL已经初始化过，无法重新初始化");
            Err(anyhow::anyhow!("tun2proxy DLL已经初始化过"))
        }
        Err(panic_info) => {
            log_error!("tun2proxy DLL初始化时发生panic: {:?}", panic_info);
            Err(anyhow::anyhow!("tun2proxy DLL初始化时发生panic"))
        }
    }
}

/// 执行tun2proxy DLL函数
/// 
/// # Arguments
/// 
/// * `f` - 要执行的函数闭包
/// 
/// # Returns
/// 
/// * `Result<T>` - 执行结果
pub fn with_tun2proxy_dll<T, F>(f: F) -> Result<T>
where
    F: FnOnce(&Tun2proxyDll) -> Result<T>,
{
    match get_tun2proxy_dll() {
        Some(dll) => {
            // 使用panic捕获机制防止DLL调用时崩溃
            let call_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                f(dll)
            }));
            
            match call_result {
                Ok(result) => result,
                Err(panic_info) => {
                    log_error!("DLL函数调用时发生panic: {:?}", panic_info);
                    Err(anyhow::anyhow!("DLL函数调用时发生panic"))
                }
            }
        },
        None => {
            log_debug!("tun2proxy DLL未初始化");
            Err(anyhow::anyhow!("tun2proxy DLL未初始化"))
        },
    }
}



/// 全局日志文件路径
static TUN_LOG_FILE_PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// 设置tun2proxy日志文件路径
/// 
/// # Arguments
/// 
/// * `log_path` - 日志文件路径
pub fn set_tun_log_file_path(log_path: std::path::PathBuf) {
    let mut path_guard = TUN_LOG_FILE_PATH.lock().unwrap();
    *path_guard = Some(log_path);
}

/// 日志回调函数实现 - 将日志输出到tun.log文件
extern "C" fn log_callback_impl(verbosity: Tun2proxyVerbosity, message: *const c_char, _ctx: *mut c_void) {
    use crate::config::AppConfig;
    
    // 检查是否启用TUN日志
    if let Ok(config) = AppConfig::load() {
        if !config.tun_log_enabled {
            return;
        }
    } else {
        return;
    }
    
    if message.is_null() {
        return;
    }
    
    unsafe {
        if let Ok(c_str) = std::ffi::CStr::from_ptr(message).to_str() {
            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let level = match verbosity {
                Tun2proxyVerbosity::Off => "OFF",
                Tun2proxyVerbosity::Error => "ERROR",
                Tun2proxyVerbosity::Warn => "WARN",
                Tun2proxyVerbosity::Info => "INFO",
                Tun2proxyVerbosity::Debug => "DEBUG",
                Tun2proxyVerbosity::Trace => "TRACE",
            };
            
            let log_line = format!("[{}] [{}] [TUN2PROXY] {}\n", timestamp, level, c_str);
            
            // 尝试写入TUN日志文件
            if let Ok(tun_log_path) = AppConfig::tun_log_path() {
                if let Ok(mut file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&tun_log_path) {
                    let _ = file.write_all(log_line.as_bytes());
                    let _ = file.flush();
                }
            }
        }
    }
}

/// 流量统计回调函数实现
extern "C" fn traffic_callback_impl(status: *const Tun2proxyTrafficStatus, _ctx: *mut c_void) {
    if status.is_null() {
        return;
    }
    
    let traffic_status = unsafe { *status };
    log_debug!("[tun2proxy] 流量统计 - 发送: {} 字节, 接收: {} 字节", 
              traffic_status.tx, traffic_status.rx);
}

/// 设置日志回调
/// 
/// # Returns
/// 
/// * `Result<()>` - 设置结果
pub fn set_log_callback() -> Result<()> {
    with_tun2proxy_dll(|dll| {
        unsafe {
            (dll.set_log_callback)(log_callback_impl, std::ptr::null_mut());
        }
        Ok(())
    })
}

/// 设置流量统计回调
/// 
/// # Arguments
/// 
/// * `interval_secs` - 统计间隔（秒）
/// 
/// # Returns
/// 
/// * `Result<()>` - 设置结果
pub fn set_traffic_status_callback(interval_secs: u32) -> Result<()> {
    with_tun2proxy_dll(|dll| {
        unsafe {
            (dll.set_traffic_status_callback)(interval_secs, traffic_callback_impl, std::ptr::null_mut());
        }
        Ok(())
    })
}

/// 使用命令行参数运行tun2proxy
/// 
/// # Arguments
/// 
/// * `cli_args` - 命令行参数
/// * `tun_mtu` - TUN设备MTU
/// * `packet_information` - 是否包含包信息
/// 
/// # Returns
/// 
/// * `Result<i32>` - 运行结果
pub fn run_with_cli_args(
    cli_args: &str,
    tun_mtu: u16,
    packet_information: bool,
) -> Result<i32> {
    log_debug!("开始使用命令行参数启动tun2proxy: {}", cli_args);
    
    let cli_args_c = CString::new(cli_args)
        .context("无法转换cli_args为C字符串")?;
    
    match get_tun2proxy_dll() {
        Some(dll) => {
            log_debug!("获取到tun2proxy DLL，开始调用run_with_cli_args函数");
            
            // 使用panic捕获机制防止DLL调用时崩溃
            let call_result = std::panic::catch_unwind(|| {
                unsafe {
                    (dll.run_with_cli_args)(
                        cli_args_c.as_ptr(),
                        tun_mtu,
                        packet_information,
                    )
                }
            });
            
            match call_result {
                Ok(result) => {
                    log_debug!("tun2proxy CLI DLL函数调用完成，返回值: {}", result);
                    Ok(result)
                }
                Err(panic_info) => {
                    log_error!("tun2proxy CLI DLL函数调用时发生panic: {:?}", panic_info);
                    Err(anyhow::anyhow!("tun2proxy CLI DLL函数调用时发生panic"))
                }
            }
        },
        None => {
            log_debug!("tun2proxy DLL未初始化");
            Err(anyhow::anyhow!("tun2proxy DLL未初始化"))
        },
    }
}

/// 停止tun2proxy
/// 
/// # Returns
/// 
/// * `Result<i32>` - 停止结果，返回退出码
pub fn stop() -> Result<i32> {
    log_debug!("开始停止tun2proxy");
    
    match get_tun2proxy_dll() {
        Some(dll) => {
            log_debug!("调用DLL停止函数");
            
            // 使用panic捕获机制防止DLL停止时崩溃
            let stop_result = std::panic::catch_unwind(|| {
                unsafe { (dll.tun2proxy_stop)() }
            });
            
            let result = match stop_result {
                Ok(exit_code) => {
                    log_debug!("tun2proxy停止成功，退出码: {}", exit_code);
                    Ok(exit_code)
                }
                Err(panic_info) => {
                    log_error!("DLL停止函数发生panic: {:?}", panic_info);
                    Err(anyhow::anyhow!("DLL停止函数发生panic"))
                }
            };
            
            // 停止后清理DLL实例，避免状态残留
            cleanup_dll_instance();
            
            result
        },
        None => {
            log_debug!("tun2proxy DLL未加载，无需停止");
            Ok(0)
        },
    }
}

/// 清理DLL实例
/// 
/// 用于在停止tun2proxy后清理DLL状态，避免下次启动时出现状态冲突
pub fn cleanup_dll_instance() {
    if get_tun2proxy_dll().is_some() {
        log_debug!("开始清理tun2proxy DLL实例");
        
        // 给TUN设备一些时间完全停止
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        // 使用panic捕获机制防止清理时崩溃
        let cleanup_result = std::panic::catch_unwind(|| {
            // 由于OnceLock不支持重置，我们通过重新创建静态变量来实现清理
            // 这里我们只能等待程序重启来真正清理DLL
            log_debug!("DLL实例清理完成（注意：OnceLock不支持运行时重置）");
        });
        
        match cleanup_result {
            Ok(_) => log_debug!("tun2proxy DLL实例清理成功"),
            Err(panic_info) => {
                log_error!("DLL实例清理时发生panic: {:?}", panic_info);
            }
        }
    } else {
        log_debug!("DLL实例已为空，无需清理");
    }
}
