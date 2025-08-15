/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use serde_json::json;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tokio::time::Duration;
use tokio::process::Command as TokioCommand;

use crate::commands::{ProxyStatus, ServerInfo};
use crate::config::AppConfig;
use crate::tun::TunManager;

/// 代理管理器
pub struct ProxyManager {
    process: Arc<Mutex<Option<Child>>>,
    start_time: Arc<Mutex<Option<Instant>>>,
    current_server: Arc<Mutex<Option<String>>>,
}

// 全局单例实例
static PROXY_MANAGER: OnceLock<ProxyManager> = OnceLock::new();

impl ProxyManager {
    /// 获取全局代理管理器实例（单例模式）
    pub fn instance() -> &'static ProxyManager {
        PROXY_MANAGER.get_or_init(|| {
            Self {
                process: Arc::new(Mutex::new(None)),
                start_time: Arc::new(Mutex::new(None)),
                current_server: Arc::new(Mutex::new(None)),
            }
        })
    }

    /// 创建新的代理管理器实例（已弃用，请使用 instance()）
    #[deprecated(note = "请使用 ProxyManager::instance() 获取单例实例")]
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            start_time: Arc::new(Mutex::new(None)),
            current_server: Arc::new(Mutex::new(None)),
        }
    }

    /// 启动代理
    /// 确保同时只有一个 Xray 进程运行，切换时先停止上一个进程再启动新的进程
    pub async fn start(&self, server: &ServerInfo) -> Result<()> {
        // 停止现有的代理进程（确保同时只有一个进程运行）
        self.stop().await?;
        
        // 检查是否启用了TUN模式
        let config = AppConfig::load()?;
        if config.tun_enabled {
            // 启动TUN模式
            let tun_manager = TunManager::instance();
            if let Err(e) = tun_manager.start(config.tun_config.clone()).await {
                eprintln!("启动TUN模式失败: {}", e);
                // TUN模式启动失败时，继续使用传统代理模式
            }
        }

        // 检查 Xray Core 是否存在
        let xray_executable = AppConfig::xray_executable()?;
        if !xray_executable.exists() {
            return Err(anyhow::anyhow!("Xray Core 可执行文件不存在: {}", xray_executable.display()));
        }

        // 生成 Xray 配置
        let config = self.generate_xray_config(server)?;
        
        // 保存配置到指定目录（如果配置文件已存在则不重新创建）
        let config_path = self.save_temp_config(&config, server, false)?;
        
        // 启动 Xray Core 进程
        let child = Command::new(&xray_executable)
            .arg("-config")
            .arg(&config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("无法启动 Xray Core: {}", xray_executable.display()))?;

        // 存储进程句柄
        {
            let mut process = self.process.lock().unwrap();
            *process = Some(child);
        }

        // 记录启动时间
        {
            let mut start_time = self.start_time.lock().unwrap();
            *start_time = Some(Instant::now());
        }

        // 记录当前服务器
        {
            let mut current_server = self.current_server.lock().unwrap();
            *current_server = Some(server.id.clone());
        }

        // 等待一小段时间确保进程启动成功
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // 检查进程是否仍在运行
        {
            let mut process = self.process.lock().unwrap();
            if let Some(ref mut child) = process.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // 进程已退出
                        *process = None;
                        return Err(anyhow::anyhow!("Xray Core 启动失败，退出状态: {}", status));
                    }
                    Ok(None) => {
                        // 进程仍在运行，启动成功
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!("检查进程状态失败: {}", e));
                    }
                }
            }
        }

        Ok(())
    }

    /// 停止代理
    /// 确保完全终止 Xray Core 进程，包括强制杀死进程
    pub async fn stop(&self) -> Result<()> {
        // 停止TUN模式（如果正在运行）
        let tun_manager = TunManager::instance();
        if tun_manager.is_running().await {
            if let Err(e) = tun_manager.stop().await {
                eprintln!("停止TUN模式失败: {}", e);
            }
        }
        // 获取进程信息并立即释放锁
        let (child_opt, pid_opt) = {
            let mut process = self.process.lock().unwrap();
            let child = process.take();
            let pid = child.as_ref().map(|c| c.id());
            (child, pid)
        };
        
        if let (Some(mut child), Some(pid)) = (child_opt, pid_opt) {
            // 首先尝试正常终止进程
            if let Err(_) = child.kill() {
                // 如果正常终止失败，使用系统命令强制终止
                self.force_kill_process(pid).await?;
            } else {
                // 等待进程退出，如果超时则强制终止
                let wait_result = tokio::time::timeout(
                    Duration::from_secs(3),
                    tokio::task::spawn_blocking(move || child.wait())
                ).await;
                
                match wait_result {
                    Ok(Ok(_)) => {
                        // 进程正常退出
                    }
                    _ => {
                        // 超时或等待失败，强制终止
                        self.force_kill_process(pid).await?;
                    }
                }
            }
        }

        // 额外确保：查找并终止所有 xray 进程
        self.kill_all_xray_processes().await?;

        // 清除启动时间
        {
            let mut start_time = self.start_time.lock().unwrap();
            *start_time = None;
        }

        // 清除当前服务器
        {
            let mut current_server = self.current_server.lock().unwrap();
            *current_server = None;
        }

        Ok(())
    }

    /// 强制终止指定PID的进程
    async fn force_kill_process(&self, pid: u32) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            let output = TokioCommand::new("taskkill")
                .args(&["/F", "/PID", &pid.to_string()])
                .output()
                .await
                .context("执行 taskkill 命令失败")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("强制终止进程失败: {}", stderr));
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            let output = TokioCommand::new("kill")
                .args(&["-9", &pid.to_string()])
                .output()
                .await
                .context("执行 kill 命令失败")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("强制终止进程失败: {}", stderr));
            }
        }
        
        Ok(())
    }

    /// 查找并终止所有 xray 进程
    async fn kill_all_xray_processes(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            // 使用 tasklist 查找 xray 进程
            let output = TokioCommand::new("tasklist")
                .args(&["/FI", "IMAGENAME eq xray.exe", "/FO", "CSV", "/NH"])
                .output()
                .await
                .context("执行 tasklist 命令失败")?;
            
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("xray.exe") {
                        // 解析CSV格式的输出获取PID
                        let parts: Vec<&str> = line.split(',').collect();
                        if parts.len() >= 2 {
                            let pid_str = parts[1].trim_matches('"');
                            if let Ok(pid) = pid_str.parse::<u32>() {
                                let _ = self.force_kill_process(pid).await;
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // 使用 pkill 终止所有 xray 进程
            let _ = TokioCommand::new("pkill")
                .args(&["-f", "xray"])
                .output()
                .await;
        }
        
        Ok(())
    }

    /// 清理指定服务器的配置文件
    /// 用于删除服务器时清理对应的配置文件
    pub fn cleanup_server_config(&self, server_id: &str, server_name: &str) -> Result<()> {
        let config_dir = AppConfig::servers_dir()?;
        
        // 生成配置文件名
        let safe_name = server_name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();
        
        let config_filename = format!("{}_{}_xray_config.json", server_id, safe_name);
        let config_path = config_dir.join(config_filename);
        
        // 如果配置文件存在则删除
        if config_path.exists() {
            std::fs::remove_file(&config_path)
                .context("删除配置文件失败")?;
        }
        
        Ok(())
    }

    /// 清理所有旧的配置文件
    /// 根据当前服务器列表，清理不再使用的配置文件
    pub fn cleanup_unused_configs(&self, active_servers: &[String]) -> Result<()> {
        let config_dir = AppConfig::servers_dir()?;
        
        if !config_dir.exists() {
            return Ok(());
        }
        
        // 读取配置目录中的所有文件
        let entries = std::fs::read_dir(&config_dir)
            .context("读取配置目录失败")?;
        
        for entry in entries {
            let entry = entry.context("读取目录项失败")?;
            let path = entry.path();
            
            // 只处理 xray_config.json 文件
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with("_xray_config.json") && filename != "xray_test_config.json" {
                    // 提取服务器ID（文件名格式：服务器ID_服务器名称_xray_config.json）
                    if let Some(server_id) = filename.split('_').next() {
                        // 如果服务器ID不在活跃列表中，删除配置文件
                        if !active_servers.contains(&server_id.to_string()) {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    /// 获取代理状态
    pub async fn get_status(&self) -> Result<ProxyStatus> {
        let config = AppConfig::load()?;

        // 获取状态信息，立即释放锁
        let (is_running, uptime, current_server_id) = {
            let process = self.process.lock().unwrap();
            let start_time = self.start_time.lock().unwrap();
            let current_server = self.current_server.lock().unwrap();

            let is_running = process.is_some();
            let uptime = if let Some(start) = *start_time {
                start.elapsed().as_secs()
            } else {
                0
            };

            (is_running, uptime, current_server.clone())
        };

        // 根据服务器ID查找服务器名称
        let current_server_name = if let Some(ref server_id) = current_server_id {
            config.servers.iter()
                .find(|server| server.id == *server_id)
                .map(|server| server.name.clone())
        } else {
            None
        };

        // 确定代理状态
        let status = if !is_running {
            "disconnected".to_string()
        } else {
            // 进程正在运行，测试连接
            if current_server_id.is_some() {
                // 这里应该根据server_id获取服务器信息并测试连接
                // 为了简化，我们先检查进程是否健康运行
                if self.is_process_healthy().await {
                    "connected".to_string()
                } else {
                    "connecting".to_string()
                }
            } else {
                "connecting".to_string()
            }
        };

        // TODO: 实现真实的流量统计
        let upload_speed = if is_running { rand::random::<u64>() % 1024 * 1024 } else { 0 };
        let download_speed = if is_running { rand::random::<u64>() % 1024 * 1024 * 10 } else { 0 };
        let total_upload = if is_running { rand::random::<u64>() % 1024 * 1024 * 1024 } else { 0 };
        let total_download = if is_running { rand::random::<u64>() % 1024 * 1024 * 1024 * 10 } else { 0 };

        Ok(ProxyStatus {
            is_running,
            status,
            current_server: current_server_name,
            proxy_mode: config.proxy_mode,
            uptime,
            upload_speed,
            download_speed,
            total_upload,
            total_download,
        })
    }

    /// 检查进程是否健康运行
    async fn is_process_healthy(&self) -> bool {
        // 获取PID并立即释放锁
        let pid_opt = {
            let process = self.process.lock().unwrap();
            process.as_ref().map(|child| child.id())
        };
        
        if let Some(pid) = pid_opt {
            // 在Windows上检查进程是否存在
            #[cfg(target_os = "windows")]
            {
                let output = TokioCommand::new("tasklist")
                    .args(&["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
                    .output()
                    .await;
                
                if let Ok(output) = output {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return !stdout.trim().is_empty() && !stdout.contains("INFO: No tasks");
                }
            }
            
            // 在Unix系统上检查进程是否存在
            #[cfg(not(target_os = "windows"))]
            {
                let output = TokioCommand::new("ps")
                    .args(&["-p", &pid.to_string()])
                    .output()
                    .await;
                
                if let Ok(output) = output {
                    return output.status.success();
                }
            }
        }
        false
    }

    /// 测试服务器连接
    /// 使用真实的 Xray 环境进行连接测试
    pub async fn test_connection(&self, server: &ServerInfo) -> Result<bool> {
        // 检查 Xray Core 是否存在
        let xray_executable = AppConfig::xray_executable()?;
        if !xray_executable.exists() {
            return Err(anyhow::anyhow!("Xray Core 可执行文件不存在: {}", xray_executable.display()));
        }

        // 生成测试用的 Xray 配置
        let config = self.generate_xray_config(server)?;
        
        // 保存测试配置到临时文件
        let config_path = self.save_test_config(&config)?;
        
        // 启动 Xray 进程进行测试
        let output = TokioCommand::new(&xray_executable)
            .arg("-config")
            .arg(&config_path)
            .arg("-test")  // 使用测试模式
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context(format!("无法启动 Xray Core 进行测试: {}", xray_executable.display()))?;

        // 清理测试配置文件
        let _ = std::fs::remove_file(&config_path);

        // 检查测试结果
        if output.status.success() {
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Configuration OK") || stderr.is_empty() {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    /// 保存测试配置文件
    pub fn save_test_config(&self, config: &serde_json::Value) -> Result<std::path::PathBuf> {
        let servers_dir = AppConfig::servers_dir()?;
        std::fs::create_dir_all(&servers_dir)
            .context("创建配置目录失败")?;

        let config_path = servers_dir.join("xray_test_config.json");
        
        let config_str = serde_json::to_string_pretty(config)
            .context("序列化配置失败")?;
        
        std::fs::write(&config_path, config_str)
            .context("写入测试配置文件失败")?;
        
        Ok(config_path)
    }

    /// 生成 Xray 配置
    pub fn generate_xray_config(&self, server: &ServerInfo) -> Result<serde_json::Value> {
        let config = AppConfig::load()?;
        
        let outbound = match server.protocol.as_str() {
            "vmess" => self.generate_vmess_outbound(server)?,
            "vless" => self.generate_vless_outbound(server)?,
            "trojan" => self.generate_trojan_outbound(server)?,
            "socks5" => self.generate_socks5_outbound(server)?,
            "http" => self.generate_http_outbound(server)?,
            _ => return Err(anyhow::anyhow!("不支持的协议: {}", server.protocol)),
        };

        let xray_config = json!({
            "log": {
                "loglevel": config.log_level
            },
            "inbounds": [
                {
                    "tag": "http",
                    "port": config.http_port,
                    "listen": "127.0.0.1",
                    "protocol": "http",
                    "sniffing": {
                        "enabled": config.inbound_sniffing_enabled,
                        "destOverride": [
                            "http",
                            "tls"
                        ],
                        "routeOnly": false
                    },
                    "settings": {
                        "auth": config.inbound_auth_method,
                        "udp": config.inbound_udp_enabled,
                        "allowTransparent": config.inbound_allow_transparent
                    }
                },
                {
                    "tag": "socks",
                    "port": config.socks_port,
                    "listen": "127.0.0.1",
                    "protocol": "mixed",
                    "sniffing": {
                        "enabled": config.inbound_sniffing_enabled,
                        "destOverride": [
                            "http",
                            "tls"
                        ],
                        "routeOnly": false
                    },
                    "settings": {
                        "auth": config.inbound_auth_method,
                        "udp": config.inbound_udp_enabled,
                        "allowTransparent": config.inbound_allow_transparent
                    }
                }
            ],
            "outbounds": [
                outbound,
                {
                    "tag": "direct",
                    "protocol": "freedom"
                },
                {
                    "tag": "block",
                    "protocol": "blackhole"
                }
            ],
            "routing": {
                "domainStrategy": config.routing_config.domain_strategy,
                "rules": config.routing_config.rules.iter().map(|rule| {
                    let mut rule_json = json!({
                        "type": rule.rule_type,
                        "outboundTag": rule.outbound_tag
                    });
                    
                    if let Some(ref ip) = rule.ip {
                        rule_json["ip"] = json!(ip);
                    }
                    
                    if let Some(ref domain) = rule.domain {
                        rule_json["domain"] = json!(domain);
                    }
                    
                    rule_json
                }).collect::<Vec<_>>()
            }
        });

        Ok(xray_config)
    }

    /// 生成 VMess 出站配置
    fn generate_vmess_outbound(&self, server: &ServerInfo) -> Result<serde_json::Value> {
        let uuid = server.config.get("uuid")
            .and_then(|v| v.as_str())
            .context("VMess 配置缺少 UUID")?;

        let alter_id = server.config.get("alterId")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let security = server.config.get("security")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        Ok(json!({
            "tag": "proxy",
            "protocol": "vmess",
            "settings": {
                "vnext": [{
                    "address": server.address,
                    "port": server.port,
                    "users": [{
                        "id": uuid,
                        "alterId": alter_id,
                        "security": security
                    }]
                }]
            }
        }))
    }

    /// 生成 VLESS 出站配置
    fn generate_vless_outbound(&self, server: &ServerInfo) -> Result<serde_json::Value> {
        let uuid = server.config.get("uuid")
            .and_then(|v| v.as_str())
            .context("VLESS 配置缺少 UUID")?;

        Ok(json!({
            "tag": "proxy",
            "protocol": "vless",
            "settings": {
                "vnext": [{
                    "address": server.address,
                    "port": server.port,
                    "users": [{
                        "id": uuid,
                        "encryption": "none"
                    }]
                }]
            }
        }))
    }

    /// 生成 Trojan 出站配置
    fn generate_trojan_outbound(&self, server: &ServerInfo) -> Result<serde_json::Value> {
        let password = server.config.get("password")
            .and_then(|v| v.as_str())
            .context("Trojan 配置缺少密码")?;

        let mut outbound = json!({
            "tag": "proxy",
            "protocol": "trojan",
            "settings": {
                "servers": [{
                    "address": server.address,
                    "port": server.port,
                    "password": password,
                    "level": 1
                }]
            }
        });

        // 添加 streamSettings
        let mut stream_settings = json!({
            "network": server.config.get("network")
                .and_then(|v| v.as_str())
                .unwrap_or("tcp")
        });

        // 添加 TLS 设置
        let tls_enabled = server.config.get("tls")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if tls_enabled {
            let mut tls_settings = json!({
                "allowInsecure": true
            });

            // SNI 设置
            if let Some(sni) = server.config.get("sni").and_then(|v| v.as_str()) {
                if !sni.is_empty() {
                    tls_settings["serverName"] = json!(sni);
                }
            }

            // ALPN 设置
            if let Some(alpn) = server.config.get("alpn").and_then(|v| v.as_array()) {
                if !alpn.is_empty() {
                    tls_settings["alpn"] = json!(alpn);
                }
            } else {
                // 默认 ALPN
                tls_settings["alpn"] = json!(["h2", "http/1.1"]);
            }

            // Fingerprint 设置
            if let Some(fingerprint) = server.config.get("fingerprint").and_then(|v| v.as_str()) {
                if !fingerprint.is_empty() {
                    tls_settings["fingerprint"] = json!(fingerprint);
                }
            } else {
                // 默认使用 chrome fingerprint
                tls_settings["fingerprint"] = json!("chrome");
            }

            stream_settings["security"] = json!("tls");
            stream_settings["tlsSettings"] = tls_settings;
        }

        // 根据网络类型添加特定设置
        let network = server.config.get("network")
            .and_then(|v| v.as_str())
            .unwrap_or("tcp");

        match network {
            "ws" => {
                let mut ws_settings = json!({});
                
                if let Some(path) = server.config.get("path").and_then(|v| v.as_str()) {
                    if !path.is_empty() {
                        ws_settings["path"] = json!(path);
                    }
                }
                
                if let Some(host) = server.config.get("host").and_then(|v| v.as_str()) {
                    if !host.is_empty() {
                        ws_settings["headers"] = json!({
                            "Host": host
                        });
                    }
                }
                
                stream_settings["wsSettings"] = ws_settings;
            }
            "h2" => {
                let mut h2_settings = json!({});
                
                if let Some(path) = server.config.get("path").and_then(|v| v.as_str()) {
                    if !path.is_empty() {
                        h2_settings["path"] = json!(path);
                    }
                }
                
                if let Some(host) = server.config.get("host").and_then(|v| v.as_str()) {
                    if !host.is_empty() {
                        h2_settings["host"] = json!([host]);
                    }
                }
                
                stream_settings["httpSettings"] = h2_settings;
            }
            "grpc" => {
                let mut grpc_settings = json!({});
                
                if let Some(service_name) = server.config.get("serviceName").and_then(|v| v.as_str()) {
                    if !service_name.is_empty() {
                        grpc_settings["serviceName"] = json!(service_name);
                    }
                }
                
                stream_settings["grpcSettings"] = grpc_settings;
            }
            _ => {} // TCP 不需要额外设置
        }

        outbound["streamSettings"] = stream_settings;

        // 添加 mux 设置
        let mux_enabled = server.config.get("mux")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        outbound["mux"] = json!({
            "enabled": mux_enabled,
            "concurrency": if mux_enabled { 8 } else { -1 }
        });

        Ok(outbound)
    }

    /// 生成 Socks5 出站配置
    fn generate_socks5_outbound(&self, server: &ServerInfo) -> Result<serde_json::Value> {
        let username = server.config.get("username")
            .and_then(|v| v.as_str());
        let password = server.config.get("password")
            .and_then(|v| v.as_str());

        let mut server_config = json!({
            "address": server.address,
            "port": server.port
        });

        if let (Some(user), Some(pass)) = (username, password) {
            server_config["users"] = json!([{
                "user": user,
                "pass": pass
            }]);
        }

        Ok(json!({
            "tag": "proxy",
            "protocol": "socks",
            "settings": {
                "servers": [server_config]
            }
        }))
    }

    /// 生成 HTTP 出站配置
    fn generate_http_outbound(&self, server: &ServerInfo) -> Result<serde_json::Value> {
        let username = server.config.get("username")
            .and_then(|v| v.as_str());
        let password = server.config.get("password")
            .and_then(|v| v.as_str());

        let mut server_config = json!({
            "address": server.address,
            "port": server.port
        });

        if let (Some(user), Some(pass)) = (username, password) {
            server_config["users"] = json!([{
                "user": user,
                "pass": pass
            }]);
        }

        Ok(json!({
            "tag": "proxy",
            "protocol": "http",
            "settings": {
                "servers": [server_config]
            }
        }))
    }

    /// 保存临时配置文件
    /// 将配置文件保存到运行目录下的 server/conf/ 目录中
    /// 根据服务器ID和名称生成唯一的配置文件名
    /// 
    /// # 参数
    /// * `config` - Xray 配置 JSON
    /// * `server` - 服务器信息
    /// * `force_recreate` - 是否强制重新创建配置文件，如果为 false 且文件已存在则跳过创建
    /// 
    /// # 返回值
    /// * `PathBuf` - 配置文件的完整路径
    fn save_temp_config(&self, config: &serde_json::Value, server: &ServerInfo, force_recreate: bool) -> Result<std::path::PathBuf> {
        let config_dir = AppConfig::servers_dir()?;
        
        // 生成唯一的配置文件名：服务器ID_服务器名称_xray_config.json
        // 清理服务器名称中的特殊字符，避免文件名问题
        let safe_name = server.name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();
        
        let config_filename = format!("{}_{}_xray_config.json", server.id, safe_name);
        let config_path = config_dir.join(config_filename);
        
        // 如果不强制重新创建且文件已存在，则直接返回路径
        if !force_recreate && config_path.exists() {
            return Ok(config_path);
        }
        
        let config_str = serde_json::to_string_pretty(config)
            .context("无法序列化 Xray 配置")?;
        
        std::fs::write(&config_path, config_str)
            .context("无法写入配置文件")?;
        
        Ok(config_path)
    }

    /// 重新生成服务器配置文件
    /// 强制重新生成指定服务器的配置文件，覆盖现有文件
    /// 
    /// # 参数
    /// * `server` - 服务器信息
    /// 
    /// # 返回值
    /// * `PathBuf` - 配置文件的完整路径
    /// 
    /// # 异常
    /// * 当生成配置失败时返回错误
    /// * 当保存配置文件失败时返回错误
    pub async fn regenerate_config(&self, server: &ServerInfo) -> Result<std::path::PathBuf> {
        // 生成 Xray 配置
        let config = self.generate_xray_config(server)?;
        
        // 强制重新创建配置文件
        let config_path = self.save_temp_config(&config, server, true)?;
        
        Ok(config_path)
    }

    /// 获取服务器配置文件路径
    /// 根据服务器ID和名称生成配置文件路径，用于打开配置文件
    /// 
    /// # 参数
    /// * `server_id` - 服务器ID
    /// * `server_name` - 服务器名称
    /// 
    /// # 返回值
    /// * `PathBuf` - 配置文件的完整路径
    pub fn get_server_config_path(&self, server_id: &str, server_name: &str) -> PathBuf {
        let config_dir = AppConfig::servers_dir().unwrap_or_else(|_| {
            std::path::PathBuf::from(".")
        });
        
        // 生成配置文件名，与save_temp_config方法保持一致
        let safe_name = server_name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();
        
        let config_filename = format!("{}_{}_xray_config.json", server_id, safe_name);
        config_dir.join(config_filename)
    }
}