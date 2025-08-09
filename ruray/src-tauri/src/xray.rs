/*
 * Project: RuRay
 * Author: Lander
 * CreateAt: 2024-12-20
 */

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use tokio::io::AsyncWriteExt;

use crate::config::AppConfig;

/// GitHub Release 信息
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    assets: Vec<GitHubAsset>,
    published_at: String,
}

/// GitHub Asset 信息
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
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
    pub async fn download_update(&self, version: &str) -> Result<()> {
        let download_url = self.get_download_url(version).await?;
        let xray_dir = AppConfig::xray_dir()?;
        
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
        let temp_file = xray_dir.join("xray_temp.zip");
        let mut file = tokio::fs::File::create(&temp_file)
            .await
            .context("无法创建临时文件")?;
        
        file.write_all(&bytes)
            .await
            .context("无法写入临时文件")?;

        // 解压文件
        self.extract_xray(&temp_file, &xray_dir).await?;

        // 删除临时文件
        tokio::fs::remove_file(&temp_file)
            .await
            .context("无法删除临时文件")?;

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
        return "windows-64".to_string();

        #[cfg(all(target_os = "windows", target_arch = "x86"))]
        return "windows-32".to_string();

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        return "macos-64".to_string();

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return "macos-arm64".to_string();

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        return "linux-64".to_string();

        #[cfg(all(target_os = "linux", target_arch = "x86"))]
        return "linux-32".to_string();

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        return "linux-arm64".to_string();

        // 默认返回
        "unknown".to_string()
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
        
        // 解析版本信息
        for line in version_output.lines() {
            if line.contains("Xray") && line.contains("(") {
                if let Some(start) = line.find("(") {
                    if let Some(end) = line.find(")") {
                        let version = &line[start + 1..end];
                        return Ok(version.to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("无法解析版本信息"))
    }
}