use std::fs;
use tauri::{Emitter, Manager};
pub fn notification_mode(app: &tauri::AppHandle) -> String {
    app.path().app_data_dir().ok().and_then(|dir| fs::read_to_string(dir.join("notification-mode.txt")).ok()).filter(|mode| ["internal", "system", "both"].contains(&mode.as_str())).unwrap_or_else(|| "both".into())
}
#[tauri::command]
pub fn read_notification_mode(app: tauri::AppHandle) -> String { notification_mode(&app) }
#[tauri::command]
pub fn save_notification_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    if !["internal", "system", "both"].contains(&mode.as_str()) { return Err("通知方式无效".into()); }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join("notification-mode.txt"), mode).map_err(|e| e.to_string())
}
pub fn builtin_sound(app: &tauri::AppHandle) -> String {
    app.path().app_data_dir().ok().and_then(|dir| fs::read_to_string(dir.join("builtin-sound.txt")).ok()).filter(|sound| ["gentle", "bright", "deep", "pulse"].contains(&sound.as_str())).unwrap_or_else(|| "gentle".into())
}
pub fn builtin_frequencies(sound: &str) -> Vec<f32> {
    match sound { "bright" => vec![880.0, 1175.0, 880.0], "deep" => vec![440.0, 554.0], "pulse" => vec![740.0, 740.0, 988.0], _ => vec![660.0, 880.0] }
}
#[tauri::command]
pub fn read_builtin_sound(app: tauri::AppHandle) -> String { builtin_sound(&app) }
#[tauri::command]
pub fn save_builtin_sound(app: tauri::AppHandle, sound: String) -> Result<(), String> {
    if !["gentle", "bright", "deep", "pulse"].contains(&sound.as_str()) { return Err("内置铃声无效".into()); }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join("builtin-sound.txt"), sound).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_appearance(app: tauri::AppHandle) -> Result<Option<serde_json::Value>, String> {
    let state = app.state::<crate::engine::Engine>();
    let root = state.root.lock().map_err(|e| e.to_string())?;
    let Some(root) = root.as_ref() else {
        return Ok(None);
    };
    let path = root.join("setting/appearance.json");
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("外观偏好读取失败，原文件已保留：{e}"))
}

#[tauri::command]
pub fn save_appearance(app: tauri::AppHandle, value: serde_json::Value) -> Result<(), String> {
    let state = app.state::<crate::engine::Engine>();
    let root = state.root.lock().map_err(|e| e.to_string())?;
    let Some(root) = root.as_ref() else {
        return Ok(());
    };
    let folder = root.join("setting");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let path = folder.join("appearance.json");
    let temporary = folder.join("appearance.json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())?;
    app.emit("appearance-changed", value)
        .map_err(|e| e.to_string())
}
