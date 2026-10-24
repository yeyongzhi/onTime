use super::*;
#[cfg(windows)]
use std::{path::PathBuf, sync::Mutex};
#[derive(Default)]
pub struct InstallState {
    #[cfg(windows)]
    ready: Mutex<Option<ReadyInstaller>>,
    #[cfg(windows)]
    gate: Mutex<()>,
}
#[cfg(windows)]
#[derive(Clone)]
struct ReadyInstaller {
    path: PathBuf,
    url: String,
    sha256: String,
}
pub(super) fn can_auto_install() -> bool {
    #[cfg(windows)]
    {
        std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.parent()
                    .map(|parent| parent.join("uninstall.exe").is_file())
            })
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}
pub(super) fn remember(
    app: &AppHandle,
    path: std::path::PathBuf,
    url: String,
    sha256: String,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        *app.state::<InstallState>()
            .ready
            .lock()
            .map_err(|e| e.to_string())? = Some(ReadyInstaller { path, url, sha256 });
    }
    #[cfg(not(windows))]
    {
        let _ = (app, path, url, sha256);
    }
    Ok(())
}
#[cfg(windows)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallPlan {
    installer: PathBuf,
    executable: PathBuf,
    install_dir: PathBuf,
    sha256: String,
    version: String,
    parent_pid: u32,
    ready: PathBuf,
    receipt: PathBuf,
}
#[derive(Deserialize)]
struct Receipt {
    status: String,
    version: String,
    detail: String,
}
#[derive(Serialize)]
pub struct InstallOutcome {
    success: bool,
    message: String,
}
#[tauri::command]
pub fn take_update_result(app: AppHandle) -> Result<Option<InstallOutcome>, String> {
    let path = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("updates/result.json");
    if !path.is_file() {
        return Ok(None);
    }
    let receipt: Receipt = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if receipt.status == "installing" {
        return Ok(None);
    }
    let success =
        receipt.status == "success" && receipt.version == app.package_info().version.to_string();
    if !success {
        log::warn!("自动安装更新失败：{}", receipt.detail);
    }
    fs::remove_file(path).map_err(|e| e.to_string())?;
    Ok(Some(InstallOutcome {
        success,
        message: if success {
            format!("已更新至 {}", receipt.version)
        } else {
            format!("自动安装未完成：{}。请重试更新或从发布页下载安装。", receipt.detail)
        },
    }))
}
#[tauri::command]
pub async fn install_downloaded_update(app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(move || install(&app))
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("当前平台请使用下载安装包完成更新".into())
    }
}
#[cfg(windows)]
fn install(app: &AppHandle) -> Result<(), String> {
    use std::{os::windows::process::CommandExt, process::Command};
    if !can_auto_install() {
        return Err("当前为开发或免安装运行，请先安装正式安装包再使用自动更新".into());
    }
    let state = app.state::<InstallState>();
    let _gate = state
        .gate
        .try_lock()
        .map_err(|_| "正在准备安装，请稍候".to_string())?;
    let package = state
        .ready
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("请先下载并校验更新")?;
    let version = source::authorize_download(app, &package.url, &package.sha256)?;
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("updates");
    let installer = package
        .path
        .canonicalize()
        .map_err(|e| format!("更新文件不可用：{e}"))?;
    let canonical_cache = cache.canonicalize().map_err(|e| e.to_string())?;
    if !installer.starts_with(&canonical_cache)
        || !installer
            .file_name()
            .and_then(|v| v.to_str())
            .is_some_and(|name| asset_matches(name, "windows", std::env::consts::ARCH))
    {
        return Err("更新文件位置或平台无效，请重新下载更新".into());
    }
    let installer = package.path.clone();
    let metadata = fs::metadata(&installer).map_err(|e| e.to_string())?;
    if metadata.len() == 0 || metadata.len() > 300 * 1024 * 1024 {
        return Err("更新文件大小无效".into());
    }
    let mut file = fs::File::open(&installer).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    verify_digest(&format!("{:x}", hasher.finalize()), &package.sha256)?;
    drop(file);
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let install_dir = executable.parent().ok_or("安装目录无效")?.to_path_buf();
    // Back up the real data root, which may be different from the installation directory.
    let engine = app.state::<crate::engine::Engine>();
    let root = engine.root.lock().map_err(|e| e.to_string())?;
    if let Some(root) = root.as_ref() {
        let backup = root.join("backups");
        fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
        crate::backup::export_to(
            root,
            &backup.join(format!("pre-update-{}.ontime.zip", uuid::Uuid::new_v4())),
        )
        .map_err(|e| format!("更新前备份失败，未退出应用：{e}"))?;
    }
    drop(root);
    let directory = installer.parent().ok_or("更新缓存目录无效")?;
    let plan = InstallPlan {
        installer: installer.clone(),
        executable,
        install_dir,
        sha256: package.sha256,
        version,
        parent_pid: std::process::id(),
        ready: directory.join("worker.ready"),
        receipt: cache.join("result.json"),
    };
    if plan.ready.is_file() {
        fs::remove_file(&plan.ready).map_err(|e| e.to_string())?;
    }
    let script = directory.join("install-worker.ps1");
    let plan_path = directory.join("install-plan.json");
    fs::write(&script, include_str!("install-worker.ps1")).map_err(|e| e.to_string())?;
    fs::write(
        &plan_path,
        serde_json::to_vec(&plan).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let powershell = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .ok_or("无法找到系统更新助手")?
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut child = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-PlanPath")
        .arg(plan_path)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| format!("无法启动更新助手，应用保持运行：{e}"))?;
    for _ in 0..100 {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err("更新助手启动失败，应用保持运行，请重试或前往发布页下载".into());
        }
        if plan.ready.is_file() {
            app.exit(0);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    Err("更新助手启动超时，应用保持运行，请重试".into())
}
