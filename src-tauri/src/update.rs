use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: String,
    latest: Option<String>,
    available: bool,
    notes: String,
    url: Option<String>,
    message: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
    percent: Option<u8>,
}
fn valid_download(url: &str) -> bool {
    url.starts_with("https://github.com/yeyongzhi/onTime/releases/download/")
        && !url.contains(['\r', '\n'])
}
fn asset_matches(name: &str, os: &str, arch: &str) -> bool {
    match (os, arch) {
        ("windows", "x86_64") => name.ends_with("_x64-setup.exe"),
        ("windows", "aarch64") => name.ends_with("_arm64-setup.exe"),
        ("macos", "aarch64") => name.ends_with("_aarch64.dmg"),
        ("macos", "x86_64") => name.ends_with("_x64.dmg"),
        _ => false,
    }
}
fn process(bytes: &[u8], current: &str, os: &str, arch: &str) -> Result<UpdateInfo, String> {
    let release: Release =
        serde_json::from_slice(bytes).map_err(|e| format!("更新信息无效：{e}"))?;
    if release.draft || release.prerelease {
        return Err("稳定渠道不接受预发布版本".into());
    }
    let latest = semver::Version::parse(release.tag_name.trim_start_matches('v'))
        .map_err(|_| "更新版本号无效")?;
    if !latest.pre.is_empty() {
        return Err("稳定渠道不接受预发布版本".into());
    }
    let current_version = semver::Version::parse(current).map_err(|_| "当前版本无效")?;
    let available = latest > current_version;
    let url = if available {
        let asset = release
            .assets
            .iter()
            .find(|a| asset_matches(&a.name, os, arch))
            .ok_or("新版本尚无适合当前系统的安装包")?;
        if asset.size == 0
            || asset.size > 300 * 1024 * 1024
            || !valid_download(&asset.browser_download_url)
        {
            return Err("更新产物或下载地址无效，已拒绝".into());
        }
        Some(asset.browser_download_url.clone())
    } else {
        None
    };
    Ok(UpdateInfo {
        current: current.into(),
        latest: Some(latest.to_string()),
        available,
        notes: release.body.unwrap_or_default(),
        url,
        message: if available {
            "发现新版本，可下载完整安装包手动升级"
        } else {
            "已是最新稳定版本"
        }
        .into(),
    })
}
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("onTime-update-check")
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .get("https://api.github.com/repos/yeyongzhi/onTime/releases/latest")
            .send()
            .map_err(|e| format!("无法检查更新，当前应用仍可使用：{e}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(UpdateInfo {
                current,
                latest: None,
                available: false,
                notes: String::new(),
                url: None,
                message: "仓库尚未发布稳定版本".into(),
            });
        }
        let response = response
            .error_for_status()
            .map_err(|e| format!("更新服务不可用：{e}"))?;
        let mut bytes = vec![];
        response
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("更新信息超过大小限制".into());
        }
        process(
            &bytes,
            &current,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn open_update_link(app: AppHandle, url: String) -> Result<(), String> {
    if !valid_download(&url) {
        return Err("只允许打开 onTime 官方仓库安装包链接".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("无法打开浏览器：{e}"))
}

#[tauri::command]
pub async fn download_update(app: AppHandle, url: String) -> Result<Option<String>, String> {
    if !valid_download(&url) {
        return Err("只允许下载 onTime 官方仓库安装包".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let filename = url
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("onTime-installer");
        let Some(target) = app
            .dialog()
            .file()
            .set_title("保存 onTime 更新安装包")
            .set_file_name(filename)
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let target = target
            .into_path()
            .map_err(|e| format!("保存位置无效：{e}"))?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(180))
            .user_agent("onTime-update-download")
            .build()
            .map_err(|e| e.to_string())?;
        let mut response = client
            .get(&url)
            .send()
            .map_err(|e| format!("下载安装包失败：{e}"))?
            .error_for_status()
            .map_err(|e| format!("下载安装包失败：{e}"))?;
        let total = response.content_length();
        const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;
        if total.is_some_and(|size| size == 0 || size > MAX_DOWNLOAD) {
            return Err("安装包大小无效或超过 300 MB 限制".into());
        }
        let mut temporary = target.clone().into_os_string();
        temporary.push(format!(".ontime-{}.part", uuid::Uuid::new_v4()));
        let temporary = std::path::PathBuf::from(temporary);
        let result = (|| -> Result<(), String> {
            let mut file =
                fs::File::create(&temporary).map_err(|e| format!("无法创建下载文件：{e}"))?;
            let mut downloaded = 0_u64;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let count = response
                    .read(&mut buffer)
                    .map_err(|e| format!("下载中断：{e}"))?;
                if count == 0 {
                    break;
                }
                downloaded += count as u64;
                if downloaded > MAX_DOWNLOAD || total.is_some_and(|size| downloaded > size) {
                    return Err("安装包大小异常，已停止下载".into());
                }
                file.write_all(&buffer[..count])
                    .map_err(|e| format!("写入安装包失败：{e}"))?;
                let percent = total
                    .filter(|size| *size > 0)
                    .map(|size| ((downloaded.saturating_mul(100) / size).min(100)) as u8);
                let _ = app.emit(
                    "update-download-progress",
                    DownloadProgress {
                        downloaded,
                        total,
                        percent,
                    },
                );
            }
            if total.is_some_and(|size| downloaded != size) {
                return Err("安装包未完整下载".into());
            }
            file.flush().map_err(|e| format!("写入安装包失败：{e}"))?;
            drop(file);
            if target.exists() {
                fs::remove_file(&target).map_err(|e| format!("无法替换目标文件：{e}"))?;
            }
            fs::rename(&temporary, &target).map_err(|e| format!("无法保存安装包：{e}"))?;
            let _ = app.emit(
                "update-download-progress",
                DownloadProgress {
                    downloaded,
                    total,
                    percent: Some(100),
                },
            );
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map(|()| Some(target.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str, url: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"tag_name":tag,"body":"说明","draft":false,"prerelease":false,"assets":[{"name":"onTime_0.2.0_x64-setup.exe","browser_download_url":url,"size":1024}]})).unwrap()
    }
    #[test]
    fn checks_version_and_rejects_bad_artifacts() {
        let url = "https://github.com/yeyongzhi/onTime/releases/download/v0.2.0/onTime_0.2.0_x64-setup.exe";
        assert!(
            process(&release("v0.2.0", url), "0.1.0", "windows", "x86_64")
                .unwrap()
                .available
        );
        assert!(
            !process(&release("v0.2.0", url), "0.2.0", "windows", "x86_64")
                .unwrap()
                .available
        );
        assert!(
            process(
                &release("v0.2.0", "https://evil.example/app.exe"),
                "0.1.0",
                "windows",
                "x86_64"
            )
            .is_err()
        );
        assert!(process(&release("v0.2.0-beta.1", url), "0.1.0", "windows", "x86_64").is_err());
        assert!(process(&release("v0.2.0", url), "0.1.0", "macos", "aarch64").is_err());
    }
}
