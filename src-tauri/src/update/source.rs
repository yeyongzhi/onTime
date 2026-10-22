use super::*;
use chrono::{DateTime, Local, Utc};
use reqwest::blocking::{Client, Response};
use std::{path::PathBuf, sync::Mutex};
const SITE: &str = "https://yeyongzhi.github.io/onTime/updates.json";
const API: &str = "https://api.github.com/repos/yeyongzhi/onTime/releases?per_page=100&page=1";
const AUTO_INTERVAL: i64 = 6 * 3600;
#[derive(Default)]
pub struct UpdateState(pub Mutex<()>);
#[derive(Default, Serialize, Deserialize)]
struct Cache {
    #[serde(default)]
    last_attempt: i64,
    #[serde(default)]
    checked_at: i64,
    #[serde(default)]
    api_retry_at: i64,
    #[serde(default)]
    releases: Vec<Release>,
    #[serde(default)]
    last_error: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    stable: Option<Release>,
    preview: Option<Release>,
}
fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    let engine = app.state::<crate::engine::Engine>();
    let root = engine.root.lock().map_err(|e| e.to_string())?;
    let folder = match root.as_ref() {
        Some(root) => root.join("setting"),
        None => app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("setting"),
    };
    Ok(folder.join("update-cache.json"))
}
fn read_cache(path: &std::path::Path) -> Cache {
    fs::read(path)
        .ok()
        .filter(|bytes| bytes.len() <= 1024 * 1024)
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}
fn save_cache(path: &std::path::Path, cache: &Cache) {
    let result = (|| -> Result<(), String> {
        fs::create_dir_all(path.parent().ok_or("缓存路径无效")?).map_err(|e| e.to_string())?;
        let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        fs::write(&temp, serde_json::to_vec(cache).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if let Err(error) = fs::rename(&temp, path) {
            let _ = fs::remove_file(&temp);
            return Err(error.to_string());
        }
        Ok(())
    })();
    if let Err(error) = result {
        log::warn!("无法保存更新检查缓存：{error}");
    }
}
fn fresh(cache: &Cache, now: i64, manual: bool) -> bool {
    !manual && cache.last_attempt > 0 && (0..AUTO_INTERVAL).contains(&(now - cache.last_attempt))
}
fn cache_info(cache: &Cache, current: &str, cached: bool) -> Result<UpdateInfo, String> {
    if cache.checked_at == 0 || cache.releases.is_empty() {
        return Err(cache
            .last_error
            .clone()
            .unwrap_or_else(|| "请稍后再检查更新，或前往发布页查看".into()));
    }
    let mut info = process(
        cache.releases.clone(),
        current,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )?;
    info.checked_at = cache.checked_at;
    info.cached = cached;
    info.warning = cache.last_error.clone();
    if cached {
        info.message = if info.available {
            "上次检查发现可用更新"
        } else {
            "以下为上次检查结果，尚未重新确认最新版本"
        }
        .into();
    }
    Ok(info)
}
fn limited_until(status: u16, headers: &reqwest::header::HeaderMap, now: i64) -> Option<i64> {
    if status != 403 && status != 429 {
        return None;
    }
    let after = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.parse::<i64>()
                .ok()
                .map(|seconds| now.saturating_add(seconds.max(60)))
                .or_else(|| {
                    DateTime::parse_from_rfc2822(v)
                        .ok()
                        .map(|date| date.timestamp())
                })
        });
    let reset = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok());
    Some(
        after
            .into_iter()
            .chain(reset)
            .max()
            .unwrap_or(now + 60)
            .max(now + 60),
    )
}
fn read_body(mut response: Response) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    (&mut response)
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("更新信息超过大小限制".into());
    }
    Ok(bytes)
}
fn fetch_site(client: &Client, url: &str) -> Result<Vec<Release>, String> {
    let response = client
        .get(url)
        .header("Cache-Control", "no-cache")
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let manifest: Manifest =
        serde_json::from_slice(&read_body(response)?).map_err(|e| e.to_string())?;
    if manifest.schema_version != 1 {
        return Err("不支持的更新清单格式".into());
    }
    let mut releases = Vec::new();
    for (release, preview) in [(manifest.stable, false), (manifest.preview, true)] {
        if let Some(release) = release {
            let parsed = semver::Version::parse(release.tag_name.trim_start_matches('v'))
                .map_err(|_| "更新清单版本无效")?;
            if release.draft || (parsed.pre.is_empty() == preview) {
                return Err("更新清单通道无效".into());
            }
            if release.assets.is_empty()
                || release.assets.iter().any(|asset| {
                    !valid_download(&asset.browser_download_url)
                        || asset.size == 0
                        || asset.size > 300 * 1024 * 1024
                        || !asset
                            .digest
                            .as_deref()
                            .and_then(|value| value.strip_prefix("sha256:"))
                            .is_some_and(valid_hash)
                })
            {
                return Err("更新清单安装包信息无效".into());
            }
            releases.push(release);
        }
    }
    if releases.is_empty() {
        return Err("更新清单尚无已发布版本".into());
    }
    Ok(releases)
}
fn fetch_api(
    client: &Client,
    cache: &mut Cache,
    now: i64,
    url: &str,
) -> Result<Vec<Release>, String> {
    if cache.api_retry_at > now {
        return Err(rate_message(cache.api_retry_at));
    }
    let response = client.get(url).send().map_err(|e| e.to_string())?;
    if let Some(until) = limited_until(response.status().as_u16(), response.headers(), now) {
        cache.api_retry_at = until;
        return Err(rate_message(until));
    }
    let bytes = read_body(response.error_for_status().map_err(|e| e.to_string())?)?;
    // Only one API request per attempt; the site manifest is the authoritative channel index.
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn rate_message(until: i64) -> String {
    let time = DateTime::from_timestamp(until, 0)
        .map(|date| {
            date.with_timezone(&Local)
                .format("%m月%d日 %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "稍后".into());
    format!("更新检查暂时受限，请在 {time} 后重试，或前往发布页下载；提醒仍可正常使用。")
}
fn retrieve(
    client: &Client,
    cache: &mut Cache,
    now: i64,
    current: &str,
    site: &str,
    api: &str,
) -> Result<Vec<Release>, String> {
    let validate = |releases: Vec<Release>| -> Result<Vec<Release>, String> {
        let info = process(
            releases.clone(),
            current,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )?;
        if info.latest.is_none() {
            return Err("未找到有效的已发布版本".into());
        }
        Ok(releases)
    };
    fetch_site(client, site)
        .and_then(validate)
        .or_else(|error| {
            log::warn!("静态更新源不可用：{error}");
            fetch_api(client, cache, now, api).and_then(validate)
        })
}
pub(super) fn check(app: &AppHandle, manual: bool) -> Result<UpdateInfo, String> {
    let state = app.state::<UpdateState>();
    let _guard = state
        .0
        .try_lock()
        .map_err(|_| "正在检查更新，请稍候".to_string())?;
    let path = cache_path(app)?;
    let mut cache = read_cache(&path);
    let now = Utc::now().timestamp();
    let current = app.package_info().version.to_string();
    if fresh(&cache, now, manual) {
        return cache_info(&cache, &current, true);
    }
    cache.last_attempt = now;
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("onTime-update-check")
        .build()
        .map_err(|e| e.to_string())?;
    let result = retrieve(&client, &mut cache, now, &current, SITE, API);
    match result {
        Ok(releases) => {
            cache.releases = releases;
            cache.checked_at = now;
            cache.last_error = None;
        }
        Err(error) => {
            log::warn!("更新检查失败：{error}");
            cache.last_error = Some(if cache.api_retry_at > now {
                rate_message(cache.api_retry_at)
            } else {
                "暂时无法连接更新服务，请稍后重试或前往发布页下载；提醒仍可正常使用。".into()
            });
        }
    }
    save_cache(&path, &cache);
    cache_info(&cache, &current, cache.last_error.is_some())
}
pub(super) fn authorize_download(app: &AppHandle, url: &str, hash: &str) -> Result<String, String> {
    let cache = read_cache(&cache_path(app)?);
    authorized_version(
        &cache.releases,
        &app.package_info().version,
        url,
        hash,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}
fn authorized_version(
    releases: &[Release],
    current: &semver::Version,
    url: &str,
    hash: &str,
    os: &str,
    arch: &str,
) -> Result<String, String> {
    let current = comparison_version(current);
    for release in releases {
        let Ok(version) = semver::Version::parse(release.tag_name.trim_start_matches('v')) else {
            continue;
        };
        if release.draft || comparison_version(&version) <= current {
            continue;
        }
        if release.assets.iter().any(|asset| {
            asset.browser_download_url == url
                && asset_matches(&asset.name, os, arch)
                && asset
                    .digest
                    .as_deref()
                    .and_then(|value| value.strip_prefix("sha256:"))
                    .is_some_and(|value| value.eq_ignore_ascii_case(hash))
        }) {
            return Ok(version.to_string());
        }
    }
    Err("下载信息与当前平台可用的新版本不一致，请重新检查更新".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn serve(responses: Vec<String>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = vec![];
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = [0; 8192];
                let count = stream.read(&mut bytes).unwrap();
                requests.push(String::from_utf8_lossy(&bytes[..count]).to_string());
                stream.write_all(response.as_bytes()).unwrap();
            }
            requests
        });
        (address, handle)
    }
    fn http(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }
    #[test]
    fn static_source_succeeds_without_api_and_cache_survives_restart() {
        let body = serde_json::json!({"schemaVersion":1,"stable":null,"preview":{"tag_name":"v0.0.1-beta4","draft":false,"assets":[{"name":"onTime_0.0.1-beta4_x64-setup.exe","browser_download_url":"https://github.com/yeyongzhi/onTime/releases/download/v0.0.1-beta4/onTime_0.0.1-beta4_x64-setup.exe","size":1000,"digest":format!("sha256:{}", "a".repeat(64))}]}}).to_string();
        let (url, handle) = serve(vec![http(&body)]);
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let mut cache = Cache::default();
        cache.releases = retrieve(
            &client,
            &mut cache,
            1000,
            "0.0.1-beta5",
            &format!("{url}/site"),
            &format!("{url}/api"),
        )
        .unwrap();
        assert!(handle.join().unwrap()[0].starts_with("GET /site "));
        cache.checked_at = 1000;
        cache.last_attempt = 1000;
        let folder =
            std::env::temp_dir().join(format!("ontime-update-test-{}", uuid::Uuid::new_v4()));
        let path = folder.join("cache.json");
        save_cache(&path, &cache);
        let cached = read_cache(&path);
        assert!(fresh(&cached, 1001, false));
        assert!(cache_info(&cached, "0.0.1-beta5", true).unwrap().cached);
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn api_fallback_is_bounded_and_respects_backoff_after_rate_limit() {
        let unavailable =
            "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string();
        let limited = "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nX-RateLimit-Reset: 2000\r\nConnection: close\r\n\r\n".to_string();
        let (url, handle) = serve(vec![unavailable.clone(), limited, unavailable]);
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let mut cache = Cache::default();
        for now in [1000, 1030] {
            let result = retrieve(
                &client,
                &mut cache,
                now,
                "0.0.1-beta5",
                &format!("{url}/site"),
                &format!("{url}/api"),
            );
            assert!(result.unwrap_err().contains("暂时受限"));
            assert_eq!(cache.api_retry_at, 2000);
        }
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.starts_with("GET /api "))
                .count(),
            1
        );
    }
    #[test]
    fn installer_authorization_rejects_downgrades_wrong_platform_and_changed_hash() {
        let hash = "a".repeat(64);
        let url = "https://github.com/yeyongzhi/onTime/releases/download/v0.0.1-beta6/onTime_0.0.1-beta6_x64-setup.exe";
        let releases: Vec<Release> = serde_json::from_value(serde_json::json!([{"tag_name":"v0.0.1-beta6","draft":false,"assets":[{"name":"onTime_0.0.1-beta6_x64-setup.exe","size":1000,"browser_download_url":url,"digest":format!("sha256:{hash}")}]}])).unwrap();
        let current = semver::Version::parse("0.0.1-beta5").unwrap();
        assert_eq!(
            authorized_version(&releases, &current, url, &hash, "windows", "x86_64").unwrap(),
            "0.0.1-beta6"
        );
        assert!(
            authorized_version(
                &releases,
                &semver::Version::parse("0.0.1-beta6").unwrap(),
                url,
                &hash,
                "windows",
                "x86_64"
            )
            .is_err()
        );
        assert!(authorized_version(&releases, &current, url, &hash, "macos", "aarch64").is_err());
        assert!(
            authorized_version(
                &releases,
                &current,
                url,
                &"b".repeat(64),
                "windows",
                "x86_64"
            )
            .is_err()
        );
    }
    #[test]
    fn manual_checks_bypass_cache_and_automatic_checks_respect_interval() {
        let cache = Cache {
            last_attempt: 1000,
            ..Default::default()
        };
        assert!(!fresh(&cache, 1000, true));
        assert!(!fresh(&cache, 1029, true));
        assert!(!fresh(&cache, 1030, true));
        assert!(fresh(&cache, 1000 + AUTO_INTERVAL - 1, false));
        assert!(!fresh(&cache, 1000 + AUTO_INTERVAL, false));
        assert!(!fresh(&cache, 999, false));
    }
    #[test]
    fn rate_limit_headers_and_fallback() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("retry-after", "120".parse().unwrap());
        headers.insert("x-ratelimit-reset", "1500".parse().unwrap());
        assert_eq!(limited_until(403, &headers, 1000), Some(1500));
        assert_eq!(limited_until(200, &headers, 1000), None);
        assert_eq!(limited_until(429, &Default::default(), 1000), Some(1060));
    }
    #[test]
    fn stale_cache_is_not_reported_as_latest() {
        let mut cache = Cache::default();
        assert!(cache_info(&cache, "0.0.1-beta5", true).is_err());
        cache.checked_at = 1000;
        cache.releases = serde_json::from_value(serde_json::json!([{"tag_name":"v0.0.1-beta4","body":"notes","draft":false,"assets":[]}])).unwrap();
        cache.last_error = Some("网络不可用".into());
        let info = cache_info(&cache, "0.0.1-beta5", true).unwrap();
        assert!(info.cached);
        assert_eq!(info.checked_at, 1000);
        assert!(info.message.contains("尚未重新确认"));
        assert!(info.warning.is_some());
    }
}
