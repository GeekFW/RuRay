// Project: RuRay
// Author: Lander
// CreateAt: 2024-01-01

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use crate::config::AppConfig;

/// 日志级别
#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// 日志管理器
pub struct Logger {
    file_writer: Option<Arc<Mutex<File>>>,
    is_debug_mode: bool,
}

impl Logger {
    /// 创建新的日志管理器
    pub fn new() -> io::Result<Self> {
        let is_debug_mode = cfg!(debug_assertions);
        
        let file_writer = if !is_debug_mode {
            // Release模式下，创建日志文件
            match AppConfig::load() {
                Ok(config) => {
                    let log_path = Path::new(&config.log_path);
                    
                    // 确保日志目录存在
                    if let Some(parent) = log_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    
                    let file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(log_path)?;
                    
                    Some(Arc::new(Mutex::new(file)))
                }
                Err(_) => {
                    // 如果无法加载配置，使用默认路径
                    let default_log_path = "./log/ruray.log";
                    let log_path = Path::new(default_log_path);
                    
                    if let Some(parent) = log_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    
                    let file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(log_path)?;
                    
                    Some(Arc::new(Mutex::new(file)))
                }
            }
        } else {
            None
        };
        
        Ok(Logger {
            file_writer,
            is_debug_mode,
        })
    }
    
    /// 写入日志
    pub fn log(&self, level: LogLevel, message: &str) {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let formatted_message = format!("[{}] [{}] {}", timestamp, level.as_str(), message);
        
        if self.is_debug_mode {
            // Debug模式下输出到控制台
            match level {
                LogLevel::Error => eprintln!("{}", formatted_message),
                _ => println!("{}", formatted_message),
            }
        } else {
            // Release模式下输出到文件
            if let Some(ref file_writer) = self.file_writer {
                if let Ok(mut file) = file_writer.lock() {
                    let _ = writeln!(file, "{}", formatted_message);
                    let _ = file.flush();
                }
            }
        }
    }
    
    /// 记录调试信息
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }
    
    /// 记录一般信息
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }
    
    /// 记录警告信息
    pub fn warn(&self, message: &str) {
        self.log(LogLevel::Warn, message);
    }
    
    /// 记录错误信息
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }
}

/// 全局日志管理器实例
static mut LOGGER: Option<Logger> = None;
static INIT: std::sync::Once = std::sync::Once::new();

/// 初始化日志系统
pub fn init_logger() -> io::Result<()> {
    unsafe {
        INIT.call_once(|| {
            match Logger::new() {
                Ok(logger) => LOGGER = Some(logger),
                Err(e) => eprintln!("初始化日志系统失败: {}", e),
            }
        });
    }
    Ok(())
}

/// 获取全局日志管理器（内部使用）
pub fn get_logger_internal() -> Option<&'static Logger> {
    unsafe { LOGGER.as_ref() }
}

/// 便捷的日志宏
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logger::get_logger_internal() {
            logger.debug(&format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logger::get_logger_internal() {
            logger.info(&format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logger::get_logger_internal() {
            logger.warn(&format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logger::get_logger_internal() {
            logger.error(&format!($($arg)*));
        }
    };
}

/// 公开获取日志管理器的函数
pub fn get_logger() -> Option<&'static Logger> {
    unsafe { LOGGER.as_ref() }
}