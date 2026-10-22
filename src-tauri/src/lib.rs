mod audio;
mod backup;
mod clock;
mod domain;
mod engine;
mod logging;
mod maintenance;
mod preferences;
mod storage;
mod update;
use std::{fs, path::PathBuf, sync::Mutex, time::Duration};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--tray"])
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let bootstrap = app.path().app_data_dir()?.join("bootstrap.json");
            let root = fs::read(bootstrap)
                .ok()
                .and_then(|b| serde_json::from_slice::<PathBuf>(&b).ok());
            if let Some(path) = root.as_ref().filter(|path| path.join("data.db").is_file())
                && let Err(e) = storage::initialize(path)
            {
                log::error!("database initialization/migration failed: {e}");
            }
            #[cfg(debug_assertions)]
            let root = if let Some(test_path) = std::env::var_os("ONTIME_TEST_ROOT") {
                let path = PathBuf::from(test_path);
                storage::initialize(&path)?;
                Some(path)
            } else {
                root
            };
            let root = match root {
                Some(path) => Some(path),
                None => {
                    #[cfg(debug_assertions)]
                    let path = app.path().app_data_dir()?.join("development");
                    #[cfg(all(not(debug_assertions), target_os = "macos"))]
                    let path = app.path().app_data_dir()?;
                    #[cfg(all(not(debug_assertions), not(target_os = "macos")))]
                    let path = std::env::current_exe()?
                        .parent()
                        .ok_or("无法确定安装目录")?
                        .to_path_buf();
                    match storage::initialize(&path) {
                        Ok(()) => Some(path),
                        Err(error) => {
                            log::error!("默认数据目录初始化失败：{error}");
                            None
                        }
                    }
                }
            };
            logging::install(root.as_deref());
            app.manage(update::UpdateState::default());
            app.manage(update::installer::InstallState::default());
            app.manage(engine::Engine {
                root: Mutex::new(root),
                hidden_alerts: Mutex::new(Default::default()),
                inspecting_snoozed: std::sync::atomic::AtomicBool::new(false),
                audio: audio::AudioHandle::start(handle.clone()),
            });
            let open = MenuItem::with_id(app, "open", "打开 onTime", true, None::<&str>)?;
            let quick_10 =
                MenuItem::with_id(app, "quick-10", "快速计时 · 10 分钟", true, None::<&str>)?;
            let quick_30 =
                MenuItem::with_id(app, "quick-30", "快速计时 · 30 分钟", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出（停止提醒）", true, None::<&str>)?;
            let active = MenuItem::with_id(app, "active", "查看活动提醒", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &active, &quick_10, &quick_30, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("onTime · 重要时刻，准时提醒")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "active" => {
                        let _ =
                            engine::show_active_alert(app.clone(), app.state::<engine::Engine>());
                    }
                    "quick-10" => {
                        if let Err(error) = engine::create_quick_timer(app, 10) {
                            log::error!("quick timer failed: {error}");
                        }
                    }
                    "quick-30" => {
                        if let Err(error) = engine::create_quick_timer(app, 30) {
                            log::error!("quick timer failed: {error}");
                        }
                    }
                    "quit" => {
                        app.state::<engine::Engine>().audio.stop();
                        app.exit(0);
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if std::env::args().any(|a| a == "--tray")
                && let Some(w) = app.get_webview_window("main")
            {
                let _ = w.hide();
            }
            std::thread::spawn(move || {
                let mut last_error = String::new();
                let mut last_backup_day = String::new();
                let mut last_backup_attempt = 0_i64;
                let mut tracker = clock::ClockTracker::default();
                let mut pending = clock::ClockChange::default();
                loop {
                    let wall = chrono::Utc::now();
                    if let Some(uptime) = clock::uptime_ms() {
                        pending.merge(tracker.observe(
                            wall.timestamp_millis(),
                            uptime,
                            chrono::Local::now().offset().local_minus_utc(),
                        ));
                    }
                    let result = engine::apply_clock_change(
                        &handle,
                        &handle.state::<engine::Engine>(),
                        pending,
                        wall.timestamp(),
                    )
                    .and_then(|_| {
                        pending = clock::ClockChange::default();
                        let day = chrono::Local::now().date_naive().to_string();
                        if let Some(path) = handle
                            .state::<engine::Engine>()
                            .root
                            .lock()
                            .ok()
                            .and_then(|root| root.clone())
                        {
                            let backup_key = format!("{}:{day}", path.display());
                            if last_backup_day != backup_key
                                || wall.timestamp() - last_backup_attempt >= 3600
                            {
                                last_backup_day = backup_key;
                                last_backup_attempt = wall.timestamp();
                                let app = handle.clone();
                                std::thread::spawn(move || {
                                    if let Err(error) = storage::automatic_backup(&path, &day) {
                                        let message = format!("自动备份失败：{error}");
                                        log::warn!("{message}");
                                        let _ = app.emit("backup-error", message);
                                    }
                                });
                            }
                        }
                        engine::reconcile(
                            &handle,
                            &handle.state::<engine::Engine>(),
                            wall.timestamp(),
                        )
                    });
                    if let Err(e) = result {
                        handle.state::<engine::Engine>().audio.stop_alert();
                        if let Some(window) = handle.get_webview_window("alert") {
                            let _ = window.hide();
                        }
                        if last_error != e {
                            let _ = handle.emit("storage-error", &e);
                            log::error!("scheduler: data storage unavailable");
                            last_error = e;
                        }
                    } else {
                        if !last_error.is_empty() {
                            let _ = handle.emit("state-changed", ());
                        }
                        last_error.clear();
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(
                    event,
                    tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(false)
                )
                && window.is_minimized().unwrap_or(false)
            {
                let _ = window.hide();
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "alert" {
                    let _ = window.emit("close-alert-request", ());
                } else {
                    if window.emit("main-close-request", ()).is_err() {
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            engine::get_snapshot,
            maintenance::read_retention,
            maintenance::save_retention,
            maintenance::cleanup_data,
            engine::hide_active_alert,
            engine::show_active_alert,
            engine::delete_alarms,
            engine::matching_alarm_dates,
            engine::get_history,
            engine::get_history_facets,
            engine::get_home_summary,
            engine::check_alarm_conflicts,
            engine::preview_next_runs,
            engine::test_notification,
            preferences::read_appearance,
            preferences::save_appearance,
            engine::choose_data_root,
            engine::open_data_root,
            engine::save_alarm,
            engine::delete_alarm,
            engine::set_enabled,
            engine::save_group,
            engine::delete_group,
            engine::save_tag,
            engine::delete_tag,
            engine::settle_occurrence,
            engine::import_sound,
            engine::preview_sound,
            backup::export_backup,
            backup::import_backup,
            update::check_update,
            update::installer::install_downloaded_update,
            update::installer::take_update_result,
            update::open_releases,
            update::reveal_installer,
            update::download_update,
            update::open_update_link
        ])
        .run(tauri::generate_context!())
        .expect("无法启动 onTime");
}
