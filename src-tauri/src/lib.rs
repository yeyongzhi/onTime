mod audio;
mod preferences;
mod backup;
mod calendar;
mod clock;
mod domain;
mod engine;
mod logging;
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
            #[cfg(debug_assertions)]
            let root = if let Some(test_path) = std::env::var_os("ONTIME_TEST_ROOT") {
                let path = PathBuf::from(test_path);
                storage::initialize(&path)?;
                calendar::seed(&path)?;
                Some(path)
            } else {
                root
            };
            logging::install(root.as_deref());
            app.manage(engine::Engine {
                root: Mutex::new(root),
                audio: audio::AudioHandle::start(handle.clone()),
            });
            let open = MenuItem::with_id(app, "open", "打开 onTime", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出（停止提醒）", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
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
            if std::env::args().any(|a| a == "--tray") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            std::thread::spawn(move || {
                let mut last_error = String::new();
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
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            engine::get_snapshot,
            preferences::read_appearance,
            preferences::save_appearance,
            engine::choose_data_root,
            engine::save_alarm,
            engine::delete_alarm,
            engine::set_enabled,
            engine::save_tag,
            engine::delete_tag,
            engine::settle_occurrence,
            engine::import_sound,
            engine::preview_sound,
            calendar::refresh_calendar,
            backup::export_backup,
            backup::import_backup,
            update::check_update,
            update::open_update_link
        ])
        .run(tauri::generate_context!())
        .expect("无法启动 onTime");
}

