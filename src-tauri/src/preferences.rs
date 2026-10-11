use std::fs;
use tauri::{Emitter, Manager};

#[tauri::command]
pub fn read_appearance(app: tauri::AppHandle) -> Result<Option<serde_json::Value>, String> {
    let state = app.state::<crate::engine::Engine>();
    let root = state.root.lock().map_err(|e| e.to_string())?;
    let Some(root) = root.as_ref() else { return Ok(None) };
    let path = root.join("setting/appearance.json");
    if !path.exists() { return Ok(None) }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map(Some).map_err(|e| format!("外观偏好读取失败，原文件已保留：{e}"))
}

#[tauri::command]
pub fn save_appearance(app: tauri::AppHandle, value: serde_json::Value) -> Result<(), String> {
    let state = app.state::<crate::engine::Engine>();
    let root = state.root.lock().map_err(|e| e.to_string())?;
    let Some(root) = root.as_ref() else { return Ok(()) };
    let folder = root.join("setting");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let path = folder.join("appearance.json");
    let temporary = folder.join("appearance.json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())?;
    app.emit("appearance-changed", value).map_err(|e| e.to_string())
}
