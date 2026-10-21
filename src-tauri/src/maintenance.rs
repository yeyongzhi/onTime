use crate::{engine::Engine, storage};
use std::{fs, path::Path};
use tauri::{AppHandle, Emitter};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn policy(root: &Path) -> Result<u32, String> {
    let path = root.join("setting/retention.json");
    if !path.exists() {
        return Ok(0);
    }
    let days: u32 = serde_json::from_slice(&fs::read(path).map_err(err)?).map_err(err)?;
    if ![0, 30, 90, 180, 365].contains(&days) {
        return Err("历史保留策略无效".into());
    }
    Ok(days)
}
pub fn apply_retention(root: &Path) -> Result<usize, String> {
    let days = policy(root)?;
    if days == 0 {
        return Ok(0);
    }
    let conn = storage::open(root)?;
    conn.execute(
        "DELETE FROM occurrences WHERE scheduled_at<?1 AND status NOT IN ('ringing','snoozed')",
        [chrono::Utc::now().timestamp() - i64::from(days) * 86400],
    )
    .map_err(err)
}
#[tauri::command]
pub fn read_retention(state: tauri::State<Engine>) -> Result<u32, String> {
    let guard = state.root.lock().map_err(err)?;
    policy(guard.as_ref().ok_or("请先选择数据目录")?)
}
#[tauri::command]
pub fn save_retention(state: tauri::State<Engine>, days: u32) -> Result<(), String> {
    if ![0, 30, 90, 180, 365].contains(&days) {
        return Err("历史保留策略无效".into());
    }
    let guard = state.root.lock().map_err(err)?;
    let root = guard.as_ref().ok_or("请先选择数据目录")?;
    fs::create_dir_all(root.join("setting")).map_err(err)?;
    let temp = root.join("setting/retention.tmp");
    fs::write(&temp, days.to_string()).map_err(err)?;
    fs::rename(temp, root.join("setting/retention.json")).map_err(err)
}
#[tauri::command]
pub fn cleanup_data(app: AppHandle, state: tauri::State<Engine>) -> Result<String, String> {
    let guard = state.root.lock().map_err(err)?;
    let root = guard.as_ref().ok_or("请先选择数据目录")?;
    let backup = root.join("backups").join(format!(
        "before-cleanup-{}.ontime.zip",
        uuid::Uuid::new_v4()
    ));
    crate::backup::export_with_unused(root, &backup, true)?;
    let snap = storage::snapshot(root)?;
    let used = snap
        .alarms
        .iter()
        .filter_map(|a| a.sound.as_ref())
        .collect::<std::collections::BTreeSet<_>>();
    let mut removed = 0;
    for entry in fs::read_dir(root.join("resources/sounds")).map_err(err)? {
        let entry = entry.map_err(err)?;
        if !entry.file_type().map_err(err)?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !used.contains(&name) {
            fs::remove_file(entry.path()).map_err(err)?;
            removed += 1;
        }
    }
    let history = apply_retention(root)?;
    drop(guard);
    app.emit("state-changed", ()).map_err(err)?;
    Ok(format!(
        "已清理 {history} 条历史、{removed} 个未使用铃声。清理前业务数据备份：{}",
        backup.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retention_preserves_active_records_and_defaults_to_forever() {
        let root = std::env::temp_dir().join(format!("ontime-retention-{}", uuid::Uuid::new_v4()));
        storage::initialize(&root).unwrap();
        let conn = storage::open(&root).unwrap();
        for status in ["ringing", "snoozed", "dismissed", "auto_stopped"] {
            let json = serde_json::json!({"id": status, "alarmId":"a", "title":"历史", "scheduledAt":1, "status":status, "snoozeCount":0, "snoozeUntil":null, "startedAt":null, "reason":""});
            conn.execute(
                "INSERT INTO occurrences(id,alarm_id,scheduled_at,json) VALUES(?1,'a',1,?2)",
                rusqlite::params![status, json.to_string()],
            )
            .unwrap();
        }
        assert_eq!(apply_retention(&root).unwrap(), 0);
        fs::create_dir_all(root.join("setting")).unwrap();
        fs::write(root.join("setting/retention.json"), "30").unwrap();
        assert_eq!(apply_retention(&root).unwrap(), 2);
        assert_eq!(storage::snapshot(&root).unwrap().occurrences.len(), 2);
        drop(conn);
        fs::remove_dir_all(root).unwrap();
    }
}
