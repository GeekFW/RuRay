/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use tokio::io::AsyncWriteExt;
use futures_util::StreamExt;

use crate::config::AppConfig;

/// GitHub Release 信息
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

/// GitHub Asset 信息
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Xray Core 管理器
pub struct XrayManager {
    client: Client,
}

impl XrayManager {
    /// 创建新的 Xray 管理器实例
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// 检查 Xray Core 更新
    pub async fn check_update(&self) -> Result<Option<String>> {
        let current_version = self.get_version().await.unwrap_or_else(|_| "unknown".to_string());
        let latest_version = self.get_latest_version().await?;

        if current_version != latest_version && current_version != "unknown" {
            Ok(Some(latest_version))
        } else if current_version == "unknown" {
            Ok(Some(latest_version))
        } else {
            Ok(None)
        }
    }

    /// 获取最新版本信息
    async fn get_latest_version(&self) -> Result<String> {
        let url = "https://api.github.com/repos/XTLS/Xray-core/releases/latest";
        
        let response = self.client
            .get(url)
            .header("User-Agent", "RuRay/1.0.0")
            .send()
            .await
            .context("无法获取最新版本信息")?;

        let release: GitHubRelease = response
            .json()
            .await
            .context("无法解析版本信息")?;

        Ok(release.tag_name)
    }

    /// 下载 Xray Core 更新
    /// 
    /// # 参数
    /// * `version` - 要下载的版本号
    /// 
    /// # 返回值
    /// * `Result<()>` - 下载结果
    /// 
    /// # 异常
    /// * 当 Xray 正在运行时返回错误
    /// * 当下载失败时返回错误
    /// * 当文件替换失败时返回错误
    pub async fn download_update(&self, version: &str) -> Result<()> {
        // 检查 Xray 是否正在运行
        if self.is_xray_running().await? {
            return Err(anyhow::anyhow!("无法在 Xray Core 运行时进行更新，请先停止代理"));
        }

        let download_url = self.get_download_url(version).await?;
        let xray_dir = AppConfig::xray_dir()?;
        
        // 创建临时目录
        let temp_dir = xray_dir.join("temp_update");
        tokio::fs::create_dir_all(&temp_dir)
            .await
            .context("无法创建临时目录")?;
        
        // 下载文件
        let response = self.client
            .get(&download_url)
            .send()
            .await
            .context("无法下载 Xray Core")?;

        let bytes = response
            .bytes()
            .await
            .context("无法读取下载内容")?;

        // 保存到临时文件
        let temp_file = temp_dir.join("xray_temp.zip");
        let mut file = tokio::fs::File::create(&temp_file)
            .await
            .context("无法创建临时文件")?;
        
        file.write_all(&bytes)
            .await
            .context("无法写入临时文件")?;

        // 解压到临时目录
        self.extract_xray_to_temp(&temp_file, &temp_dir).await?;

        // 原子性替换文件
        self.replace_xray_executable(&temp_dir, &xray_dir).await?;

        // 清理临时目录
        tokio::fs::remove_dir_all(&temp_dir)
            .await
            .context("无法删除临时目录")?;

        Ok(())
    }

    /// 下载 Xray Core 更新（带进度回调）
    /// 
    /// # 参数
    /// * `version` - 要下载的版本号
    /// * `progress_callback` - 进度回调函数
    /// 
    /// # 返回值
    /// * `Result<()>` - 下载结果
    /// 
    /// # 异常
    /// * 当 Xray 正在运行时返回错误
    /// * 当下载失败时返回错误
    /// * 当文件替换失败时返回错误
    pub async fn download_update_with_progress<F>(&self, version: &str, mut progress_callback: F) -> Result<()>
    where
        F: FnMut(u64, u64, String) + Send,
    {
        progress_callback(0, 100, "检查运行状态...".to_string());
        
        // 检查 Xray 是否正在运行
        if self.is_xray_running().await? {
            return Err(anyhow::anyhow!("无法在 Xray Core 运行时进行更新，请先停止代理"));
        }
        
        progress_callback(5, 100, "正在获取下载信息...".to_string());
        
        let download_url = self.get_download_url(version).await?;
        let xray_dir = AppConfig::xray_dir()?;
        
        // 创建临时目录
        let temp_dir = xray_dir.join("temp_update");
        tokio::fs::create_dir_all(&temp_dir)
            .await
            .context("无法创建临时目录")?;
        
        progress_callback(10, 100, "开始下载...".to_string());
        
        // 发起下载请求
        let response = self.client
            .get(&download_url)
            .send()
            .await
            .context("无法下载 Xray Core")?;

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        // 保存到临时文件
        let temp_file = temp_dir.join("xray_temp.zip");
        let mut file = tokio::fs::File::create(&temp_file)
            .await
            .context("无法创建临时文件")?;

        // 流式下载并更新进度
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("下载过程中出现错误")?;
            file.write_all(&chunk)
                .await
                .context("无法写入临时文件")?;
            
            downloaded += chunk.len() as u64;
            
            if total_size > 0 {
                let progress = (downloaded * 75 / total_size) + 10; // 10-85% 为下载进度
                progress_callback(progress, 100, format!("下载中... {:.1}MB/{:.1}MB", 
                    downloaded as f64 / 1024.0 / 1024.0, 
                    total_size as f64 / 1024.0 / 1024.0));
            } else {
                progress_callback(50, 100, format!("下载中... {:.1}MB", downloaded as f64 / 1024.0 / 1024.0));
            }
        }

        progress_callback(85, 100, "正在解压文件...".to_string());

        // 解压到临时目录
        self.extract_xray_to_temp(&temp_file, &temp_dir).await?;

        progress_callback(90, 100, "备份原文件...".to_string());

        // 原子性替换文件
        self.replace_xray_executable(&temp_dir, &xray_dir).await?;

        progress_callback(95, 100, "清理临时文件...".to_string());

        // 清理临时目录
        tokio::fs::remove_dir_all(&temp_dir)
            .await
            .context("无法删除临时目录")?;

        progress_callback(100, 100, "更新完成！".to_string());

        Ok(())
    }

    /// 获取下载链接
    async fn get_download_url(&self, version: &str) -> Result<String> {
        let url = format!("https://api.github.com/repos/XTLS/Xray-core/releases/tags/{}", version);
        
        let response = self.client
            .get(&url)
            .header("User-Agent", "RuRay/1.0.0")
            .send()
            .await
            .context("无法获取版本信息")?;

        let release: GitHubRelease = response
            .json()
            .await
            .context("无法解析版本信息")?;

        // 根据操作系统选择合适的资源
        let asset_name = self.get_asset_name();
        
        for asset in release.assets {
            if asset.name.contains(&asset_name) {
                return Ok(asset.browser_download_url);
            }
        }

        Err(anyhow::anyhow!("未找到适合的下载资源"))
    }

    /// 获取资源名称
    fn get_asset_name(&self) -> String {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            "windows-64".to_string()
        }
        #[cfg(all(target_os = "windows", target_arch = "x86"))]
        {
            "windows-32".to_string()
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            "macos-64".to_string()
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            "macos-arm64".to_string()
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            "linux-64".to_string()
        }
        #[cfg(all(target_os = "linux", target_arch = "x86"))]
        {
            "linux-32".to_string()
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            "linux-arm64".to_string()
        }
        #[cfg(not(any(
            all(target_os = "windows", target_arch = "x86_64"),
            all(target_os = "windows", target_arch = "x86"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "x86"),
            all(target_os = "linux", target_arch = "aarch64")
        )))]
        {
            "unknown".to_string()
        }
    }

    /// 解压 Xray Core
    async fn extract_xray(&self, zip_path: &Path, extract_dir: &Path) -> Result<()> {
        let file = std::fs::File::open(zip_path)
            .context("无法打开压缩文件")?;

        let mut archive = zip::ZipArchive::new(file)
            .context("无法读取压缩文件")?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .context("无法读取压缩文件内容")?;

            let file_name = file.name();
            
            // 只提取 xray 可执行文件
            if file_name == "xray" || file_name == "xray.exe" {
                let output_path = extract_dir.join(file_name);
                
                let mut output_file = std::fs::File::create(&output_path)
                    .context("无法创建输出文件")?;

                std::io::copy(&mut file, &mut output_file)
                    .context("无法复制文件内容")?;

                // 在 Unix 系统上设置执行权限
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = output_file.metadata()?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&output_path, perms)?;
                }

                break;
            }
        }

        Ok(())
    }

    /// 获取当前 Xray Core 版本
    pub async fn get_version(&self) -> Result<String> {
        let xray_executable = AppConfig::xray_executable()?;
        
        if !xray_executable.exists() {
            return Err(anyhow::anyhow!("Xray Core 未安装"));
        }

        let output = Command::new(&xray_executable)
            .arg("version")
            .output()
            .context("无法执行 Xray Core")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("获取版本信息失败"));
        }

        let version_output = String::from_utf8_lossy(&output.stdout);
        
        // 解析版本信息，提取 "Xray X.X.X" 格式的版本号
        for line in version_output.lines() {
            if line.contains("Xray") {
                // 查找 "Xray" 后面的版本号
                if let Some(xray_pos) = line.find("Xray") {
                    let after_xray = &line[xray_pos..];
                    // 使用正则表达式或简单的字符串处理来提取版本号
                    if let Some(space_pos) = after_xray.find(' ') {
                        let version_part = &after_xray[space_pos + 1..];
                        // 查找版本号结束位置（遇到空格或括号）
                        let end_pos = version_part.find(' ').unwrap_or_else(|| {
                            version_part.find('(').unwrap_or(version_part.len())
                        });
                        let version = format!("Xray {}", &version_part[..end_pos]);
                        return Ok(version);
                    }
                }
            }
        }

        Err(anyhow::anyhow!("无法解析版本信息"))
    }

    /// 检查并下载必需的数据文件（geoip.dat 和 geosite.dat）
    /// 
    /// # 参数
    /// * `progress_callback` - 进度回调函数，接收 (当前进度, 总进度, 状态消息)
    /// 
    /// # 返回值
    /// * `Result<()>` - 下载结果
    pub async fn download_geo_files<F>(&self, mut progress_callback: F) -> Result<()>
    where
        F: FnMut(u64, u64, String) + Send,
    {
        let xray_dir = AppConfig::xray_dir()?;
        
        // 确保目录存在
        tokio::fs::create_dir_all(&xray_dir)
            .await
            .context("无法创建 Xray 目录")?;

        progress_callback(0, 100, "开始下载地理位置数据文件...".to_string());

        // 下载 geoip.dat
        progress_callback(10, 100, "下载 geoip.dat...".to_string());
        self.download_geo_file(
            "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geoip.dat",
            &xray_dir.join("geoip.dat"),
            |progress| {
                let adjusted_progress = 10 + (progress * 40 / 100); // 10-50%
                progress_callback(adjusted_progress, 100, format!("下载 geoip.dat... {}%", progress));
            }
        ).await?;

        // 下载 geosite.dat
        progress_callback(50, 100, "下载 geosite.dat...".to_string());
        self.download_geo_file(
            "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat",
            &xray_dir.join("geosite.dat"),
            |progress| {
                let adjusted_progress = 50 + (progress * 40 / 100); // 50-90%
                progress_callback(adjusted_progress, 100, format!("下载 geosite.dat... {}%", progress));
            }
        ).await?;

        progress_callback(100, 100, "地理位置数据文件下载完成！".to_string());
        Ok(())
    }

    /// 下载单个地理位置数据文件
    /// 
    /// # 参数
    /// * `url` - 下载链接
    /// * `output_path` - 输出文件路径
    /// * `progress_callback` - 进度回调函数
    /// 
    /// # 返回值
    /// * `Result<()>` - 下载结果
    async fn download_geo_file<F>(&self, url: &str, output_path: &Path, mut progress_callback: F) -> Result<()>
    where
        F: FnMut(u64) + Send,
    {
        let response = self.client
            .get(url)
            .header("User-Agent", "RuRay/1.0.0")
            .send()
            .await
            .context("无法下载地理位置数据文件")?;

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        let mut file = tokio::fs::File::create(output_path)
            .await
            .context("无法创建输出文件")?;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("下载过程中出现错误")?;
            file.write_all(&chunk)
                .await
                .context("无法写入文件")?;
            
            downloaded += chunk.len() as u64;
            
            if total_size > 0 {
                let progress = (downloaded * 100 / total_size) as u64;
                progress_callback(progress);
            }
        }

        Ok(())
    }

    /// 检查地理位置数据文件是否存在
    /// 
    /// # 返回值
    /// * `Result<bool>` - 文件是否有一个存在
    pub fn check_geo_files_exist(&self) -> Result<bool> {
        let xray_dir = AppConfig::xray_dir()?;
        let geoip_path = xray_dir.join("geoip.dat");
        let geosite_path = xray_dir.join("geosite.dat");
        
        Ok(geoip_path.exists() || geosite_path.exists())
    }

    /// 确保所有必需文件都存在（Xray 可执行文件和地理位置数据文件）
    /// 
    /// # 参数
    /// * `progress_callback` - 进度回调函数
    /// 
    /// # 返回值
    /// * `Result<()>` - 检查和下载结果
    pub async fn ensure_all_files<F>(&self, mut progress_callback: F) -> Result<()>
    where
        F: FnMut(u64, u64, String) + Send,
    {
        progress_callback(0, 100, "检查 Xray Core 文件...".to_string());
        
        let xray_executable = AppConfig::xray_executable()?;
        let geo_files_exist = self.check_geo_files_exist()?;
        
        if !xray_executable.exists() {
            progress_callback(10, 100, "Xray Core 未安装，开始下载...".to_string());
            
            // 下载最新版本的 Xray Core
            let latest_version = self.get_latest_version().await?;
            self.download_update_with_progress(&latest_version, |progress, total, message| {
                let adjusted_progress = 10 + (progress * 40 / 100); // 10-50%
                progress_callback(adjusted_progress, total, message);
            }).await?;
        } else {
            progress_callback(50, 100, "Xray Core 已存在".to_string());
        }
        
        if !geo_files_exist {
            progress_callback(50, 100, "地理位置数据文件缺失，开始下载...".to_string());
            
            self.download_geo_files(|progress, total, message| {
                let adjusted_progress = 50 + (progress * 50 / 100); // 50-100%
                progress_callback(adjusted_progress, total, message);
            }).await?;
        } else {
            progress_callback(100, 100, "所有文件已就绪".to_string());
        }
        
        Ok(())
    }

    /// 检查 Xray 是否正在运行
    /// 
    /// # 返回值
    /// * `Result<bool>` - Xray 是否正在运行
    async fn is_xray_running(&self) -> Result<bool> {
        use crate::proxy::ProxyManager;
        let proxy_manager = ProxyManager::instance();
        Ok(proxy_manager.is_process_running())
    }

    /// 解压 Xray Core 到临时目录
    /// 
    /// # 参数
    /// * `zip_path` - 压缩文件路径
    /// * `temp_dir` - 临时目录路径
    /// 
    /// # 返回值
    /// * `Result<()>` - 解压结果
    async fn extract_xray_to_temp(&self, zip_path: &Path, temp_dir: &Path) -> Result<()> {
        let file = std::fs::File::open(zip_path)
            .context("无法打开压缩文件")?;

        let mut archive = zip::ZipArchive::new(file)
            .context("无法读取压缩文件")?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .context("无法读取压缩文件内容")?;

            let file_name = file.name();
            
            // 只提取 xray 可执行文件
            if file_name == "xray" || file_name == "xray.exe" {
                let output_path = temp_dir.join(file_name);
                
                let mut output_file = std::fs::File::create(&output_path)
                    .context("无法创建输出文件")?;

                std::io::copy(&mut file, &mut output_file)
                    .context("无法复制文件内容")?;

                // 在 Unix 系统上设置执行权限
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = output_file.metadata()?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&output_path, perms)?;
                }

                break;
            }
        }

        Ok(())
    }

    /// 原子性替换 Xray 可执行文件
    /// 
    /// # 参数
    /// * `temp_dir` - 临时目录路径
    /// * `target_dir` - 目标目录路径
    /// 
    /// # 返回值
    /// * `Result<()>` - 替换结果
    async fn replace_xray_executable(&self, temp_dir: &Path, target_dir: &Path) -> Result<()> {
        let executable_name = if cfg!(windows) { "xray.exe" } else { "xray" };
        let temp_executable = temp_dir.join(executable_name);
        let target_executable = target_dir.join(executable_name);
        let backup_executable = target_dir.join(format!("{}.backup", executable_name));

        // 检查新文件是否存在
        if !temp_executable.exists() {
            return Err(anyhow::anyhow!("临时目录中未找到 Xray 可执行文件"));
        }

        // 如果目标文件存在，先备份
        if target_executable.exists() {
            // 删除旧的备份文件（如果存在）
            if backup_executable.exists() {
                tokio::fs::remove_file(&backup_executable)
                    .await
                    .context("无法删除旧的备份文件")?;
            }

            // 备份当前文件
            tokio::fs::rename(&target_executable, &backup_executable)
                .await
                .context("无法备份当前 Xray 文件")?;
        }

        // 移动新文件到目标位置
        match tokio::fs::rename(&temp_executable, &target_executable).await {
            Ok(_) => {
                // 成功替换，删除备份文件
                if backup_executable.exists() {
                    let _ = tokio::fs::remove_file(&backup_executable).await;
                }
                Ok(())
            }
            Err(e) => {
                // 替换失败，恢复备份文件
                if backup_executable.exists() {
                    let _ = tokio::fs::rename(&backup_executable, &target_executable).await;
                }
                Err(anyhow::anyhow!("文件替换失败: {}", e))
            }
        }
    }
}