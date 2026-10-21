use crate::{
    engine::Engine,
    storage::{self, Snapshot},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
const LIMIT: u64 = 200 * 1024 * 1024;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    application: String,
    format_version: u32,
    created_at: i64,
}
fn safe_entry(name: &str) -> bool {
    if ["manifest.json", "snapshot.json", "config.json"].contains(&name) {
        return true;
    }
    let Some(sound) = name.strip_prefix("sounds/") else {
        return false;
    };
    let p = Path::new(sound);
    p.components().count() == 1
        && matches!(p.components().next(), Some(Component::Normal(_)))
        && !sound.contains('\\')
        && !sound.contains(':')
}
pub fn export_to(root: &Path, destination: &Path) -> Result<(), String> {
    export_with_unused(root, destination, false)
}
pub fn export_with_unused(
    root: &Path,
    destination: &Path,
    include_unused: bool,
) -> Result<(), String> {
    if destination.extension().and_then(|s| s.to_str()) != Some("zip") {
        return Err("备份文件必须使用 .zip 扩展名，避免覆盖业务数据".into());
    }
    let conn = storage::open(root)?;
    conn.execute_batch("BEGIN DEFERRED").map_err(err)?;
    let snap = storage::snapshot_from(&conn, root, true)?;
    let mut entries = vec![
        (
            "manifest.json".to_string(),
            serde_json::to_vec(&Manifest {
                application: "onTime".into(),
                format_version: 2,
                created_at: chrono::Utc::now().timestamp(),
            })
            .map_err(err)?,
        ),
        (
            "snapshot.json".to_string(),
            serde_json::to_vec(&snap).map_err(err)?,
        ),
        (
            "config.json".to_string(),
            fs::read(root.join("config.json")).map_err(err)?,
        ),
    ];
    let mut used = std::collections::BTreeSet::new();
    for alarm in &snap.alarms {
        if let Some(sound) = &alarm.sound
            && used.insert(sound.clone())
        {
            let path = storage::safe_sound(root, sound)?;
            entries.push((format!("sounds/{sound}"), fs::read(path).map_err(err)?));
        }
    }
    if include_unused {
        for entry in fs::read_dir(root.join("resources/sounds")).map_err(err)? {
            let entry = entry.map_err(err)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().map_err(err)?.is_file() && used.insert(name.clone()) {
                let path = storage::safe_sound(root, &name)?;
                entries.push((format!("sounds/{name}"), fs::read(path).map_err(err)?));
            }
        }
    }
    if entries.iter().map(|(_, b)| b.len() as u64).sum::<u64>() > LIMIT {
        return Err("备份超过 200 MB，请减少导入铃声后再试".into());
    }
    let parent = destination.parent().ok_or("备份路径无效")?;
    let temp = parent.join(format!(".ontime-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let file = fs::File::create(&temp).map_err(err)?;
        let mut writer = ZipWriter::new(file);
        for (name, bytes) in entries {
            writer
                .start_file(
                    name,
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .map_err(err)?;
            writer.write_all(&bytes).map_err(err)?;
        }
        let file = writer.finish().map_err(err)?;
        file.sync_all().map_err(err)?;
        fs::rename(&temp, destination).map_err(err)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn read_archive(file: &Path) -> Result<std::collections::BTreeMap<String, Vec<u8>>, String> {
    if fs::metadata(file).map_err(err)?.len() > LIMIT {
        return Err("备份文件过大".into());
    }
    let mut zip = ZipArchive::new(fs::File::open(file).map_err(err)?).map_err(err)?;
    if zip.len() > 10000 {
        return Err("备份文件数量过多".into());
    }
    let mut entries = std::collections::BTreeMap::new();
    let mut total = 0;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(err)?;
        let name = entry.name().map_err(err)?.to_string();
        if !safe_entry(&name) || entry.is_dir() || entries.contains_key(&name) {
            return Err("备份包含不安全或重复路径".into());
        }
        total += entry.size();
        if total > LIMIT {
            return Err("解压体积超过 200 MB".into());
        }
        let mut bytes = vec![];
        entry.take(LIMIT + 1).read_to_end(&mut bytes).map_err(err)?;
        if bytes.len() as u64 > LIMIT {
            return Err("解压内容过大".into());
        }
        entries.insert(name, bytes);
    }
    Ok(entries)
}
pub fn import_from(root: &Path, file: &Path) -> Result<String, String> {
    let entries = read_archive(file)?;
    let manifest: Manifest =
        serde_json::from_slice(entries.get("manifest.json").ok_or("缺少备份清单")?).map_err(err)?;
    if manifest.application != "onTime" || ![1, 2].contains(&manifest.format_version) {
        return Err("不兼容的备份版本".into());
    }
    let mut snap: Snapshot =
        serde_json::from_slice(entries.get("snapshot.json").ok_or("缺少提醒数据")?).map_err(err)?;
    if manifest.format_version == 1 {
        for alarm in &mut snap.alarms {
            if alarm.repeat == "monthly" && alarm.monthly_day == Some(31) {
                alarm.monthly_day = Some(0);
            }
        }
    }
    let config: serde_json::Value =
        serde_json::from_slice(entries.get("config.json").ok_or("缺少配置")?).map_err(err)?;
    if config.get("schemaVersion").and_then(|v| v.as_u64()) != Some(1) {
        return Err("不兼容的配置版本".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for a in &snap.alarms {
        a.validate()?;
        if a.id.is_empty() || !ids.insert(&a.id) {
            return Err("备份提醒标识重复或无效".into());
        }
        if let Some(sound) = &a.sound {
            if !safe_entry(&format!("sounds/{sound}"))
                || !entries.contains_key(&format!("sounds/{sound}"))
            {
                return Err("备份铃声缺失".into());
            }
            let bytes = &entries[&format!("sounds/{sound}")];
            if bytes.len() > 50 * 1024 * 1024 {
                return Err("单个铃声超过 50 MB".into());
            }
            rodio::Decoder::try_from(std::io::Cursor::new(bytes.clone()))
                .map_err(|e| format!("备份铃声无效：{e}"))?;
        }
    }
    let mut tag_ids = std::collections::BTreeSet::new();
    for t in &snap.tags {
        if t.id.is_empty() || !tag_ids.insert(&t.id) || t.name.trim().is_empty() {
            return Err("备份标签无效".into());
        }
    }
    let mut group_ids = std::collections::BTreeSet::new();
    for g in &snap.groups {
        if g.id.is_empty() || !group_ids.insert(&g.id) || g.name.trim().is_empty() {
            return Err("备份分组无效".into());
        }
    }
    if snap.alarms.iter().any(|a| {
        a.group_id
            .as_ref()
            .is_some_and(|id| !group_ids.contains(id))
    }) {
        return Err("备份提醒的分组不存在".into());
    }
    let mut occurrence_ids = std::collections::BTreeSet::new();
    for o in &snap.occurrences {
        if !occurrence_ids.insert(&o.id) {
            return Err("备份记录标识重复".into());
        }
    }
    // Preflight every resource conflict before changing the active database.
    for (name, bytes) in &entries {
        if let Some(sound) = name.strip_prefix("sounds/") {
            let target = root.join("resources/sounds").join(sound);
            if target.exists() && fs::read(&target).map_err(err)? != *bytes {
                return Err("备份与现有铃声名称冲突，原数据未变更".into());
            }
        }
    }
    let prebackup = root
        .join("backups")
        .join(format!("before-import-{}.ontime.zip", uuid::Uuid::new_v4()));
    export_to(root, &prebackup)?;
    for (name, bytes) in &entries {
        if let Some(sound) = name.strip_prefix("sounds/") {
            let target = root.join("resources/sounds").join(sound);
            if !target.exists() {
                fs::write(target, bytes).map_err(err)?;
            }
        }
    }
    // Stage configuration before touching the database; a failed write leaves current data intact.
    let config_path = root.join("config.json");
    let previous_config = fs::read(&config_path).map_err(err)?;
    let temp = root.join("config.import.tmp");
    fs::write(&temp, serde_json::to_vec(&config).map_err(err)?).map_err(err)?;
    let mut conn = storage::open(root)?;
    let tx = conn.transaction().map_err(err)?;
    tx.execute_batch(
        "DELETE FROM alarms;DELETE FROM groups;DELETE FROM tags;DELETE FROM occurrences;DELETE FROM calendars;",
    )
    .map_err(err)?;
    for t in &snap.tags {
        tx.execute(
            "INSERT INTO tags(id,json) VALUES(?1,?2)",
            rusqlite::params![t.id, serde_json::to_string(t).map_err(err)?],
        )
        .map_err(err)?;
    }
    for g in &snap.groups {
        tx.execute(
            "INSERT INTO groups(id,json) VALUES(?1,?2)",
            rusqlite::params![g.id, serde_json::to_string(g).map_err(err)?],
        )
        .map_err(err)?;
    }
    for a in &snap.alarms {
        tx.execute(
            "INSERT INTO alarms(id,json,next_at) VALUES(?1,?2,?3)",
            rusqlite::params![
                a.id,
                serde_json::to_string(a).map_err(err)?,
                a.next_after(chrono::Utc::now().timestamp())
            ],
        )
        .map_err(err)?;
    }
    for o in &mut snap.occurrences {
        if ["ringing", "snoozed"].contains(&o.status.as_str()) {
            o.status = "cancelled".into();
            o.started_at = None;
            o.snooze_until = None;
            o.reason = "备份恢复时取消活动提醒".into();
        }
        tx.execute(
            "INSERT INTO occurrences(id,alarm_id,scheduled_at,json) VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                o.id,
                o.alarm_id,
                o.scheduled_at,
                serde_json::to_string(o).map_err(err)?
            ],
        )
        .map_err(err)?;
    }
    for c in &snap.calendars {
        tx.execute(
            "INSERT INTO calendars VALUES(?1,?2)",
            rusqlite::params![c.year, serde_json::to_string(c).map_err(err)?],
        )
        .map_err(err)?;
    }
    // A failed configuration replacement rolls the SQL transaction back automatically.
    fs::rename(&temp, &config_path).map_err(err)?;
    if let Err(commit_error) = tx.commit() {
        fs::write(&temp, previous_config).map_err(err)?;
        fs::rename(&temp, &config_path).map_err(err)?;
        return Err(format!("恢复事务失败，原配置已恢复：{commit_error}"));
    }
    storage::backfill_history_context(&mut conn)?;
    Ok(format!(
        "已恢复 {} 个提醒。原数据备份：{}",
        snap.alarms.len(),
        prebackup.display()
    ))
}
#[tauri::command]
pub async fn export_backup(app: AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .set_title("导出 onTime 备份")
            .set_file_name("onTime.ontime.zip")
            .add_filter("onTime 备份", &["zip"])
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = file.into_path().map_err(err)?;
        let state = app.state::<Engine>();
        let guard = state.root.lock().map_err(err)?;
        let source = guard.as_ref().ok_or("请先选择数据目录")?.clone();
        drop(guard);
        export_to(&source, &path)?;
        Ok(Some(format!("备份已保存：{}", path.display())))
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn import_backup(app: AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .set_title("导入 onTime 备份（覆盖当前提醒与记录）")
            .add_filter("onTime 备份", &["zip"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file.into_path().map_err(err)?;
        let state = app.state::<Engine>();
        let guard = state.root.lock().map_err(err)?;
        let message = import_from(guard.as_ref().ok_or("请先选择数据目录")?, &path)?;
        state.audio.stop();
        drop(guard);
        let _ = app.emit("state-changed", ());
        Ok(Some(message))
    })
    .await
    .map_err(err)?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsafe_entries_rejected() {
        for s in [
            "../data.db",
            "sounds/../../config.json",
            "sounds/a/b",
            "sounds/C:\\x",
            "data.db",
        ] {
            assert!(!safe_entry(s));
        }
        assert!(safe_entry("sounds/alarm.mp3"));
    }
    #[test]
    fn backup_roundtrip_and_bad_archive_preserve_data() {
        let root =
            std::env::temp_dir().join(format!("ontime-backup-test-{}", uuid::Uuid::new_v4()));
        storage::initialize(&root).unwrap();
        let conn = storage::open(&root).unwrap();
        let t = crate::domain::Tag {
            created_at: Some(1234567890),
            id: "t".into(),
            name: "生活".into(),
            color: "#abcdef".into(),
        };
        conn.execute(
            "INSERT INTO tags(id,json) VALUES(?1,?2)",
            rusqlite::params![t.id, serde_json::to_string(&t).unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,json) VALUES(?1,?2)",
            rusqlite::params![t.id, serde_json::to_string(&t).unwrap()],
        )
        .unwrap();
        let a: crate::domain::Alarm = serde_json::from_value(serde_json::json!({"id":"a","title":"分组提醒","enabled":false,"kind":"timer","time":"09:00","date":null,"repeat":"daily","weekdays":[],"skipHolidays":false,"durationSeconds":60,"deadline":null,"sound":null,"volume":0.5,"gradual":true,"ringSeconds":60,"snoozeLimit":3,"tags":["t"],"groupId":"t","revision":1,"createdAt":1})).unwrap();
        storage::save_alarm(&root, &a, None).unwrap();
        drop(conn);
        let archive = root.join("export.zip");
        export_to(&root, &archive).unwrap();
        let conn = storage::open(&root).unwrap();
        conn.execute_batch("DELETE FROM tags;DELETE FROM groups;DELETE FROM alarms;")
            .unwrap();
        drop(conn);
        import_from(&root, &archive).unwrap();
        assert_eq!(storage::snapshot(&root).unwrap().tags[0].name, "生活");
        let restored = storage::snapshot(&root).unwrap();
        assert_eq!(restored.groups[0].name, "生活");
        assert_eq!(restored.alarms[0].group_id.as_deref(), Some("t"));
        assert_eq!(restored.alarms[0].tags, vec!["t"]);
        let bad = root.join("bad.zip");
        fs::write(&bad, b"invalid archive").unwrap();
        assert!(import_from(&root, &bad).is_err());
        assert_eq!(storage::snapshot(&root).unwrap().tags.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cleanup_archive_keeps_unreferenced_sounds_recoverable() {
        let root =
            std::env::temp_dir().join(format!("ontime-orphan-backup-{}", uuid::Uuid::new_v4()));
        storage::initialize(&root).unwrap();
        fs::write(root.join("resources/sounds/orphan.wav"), b"orphan audio").unwrap();
        let archive = root.join("cleanup.zip");
        export_with_unused(&root, &archive, true).unwrap();
        let entries = read_archive(&archive).unwrap();
        assert_eq!(entries.get("sounds/orphan.wav").unwrap(), b"orphan audio");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn restoring_backup_cancels_active_instances_and_recomputes_schedule() {
        let root =
            std::env::temp_dir().join(format!("ontime-restore-active-{}", uuid::Uuid::new_v4()));
        storage::initialize(&root).unwrap();
        let conn = storage::open(&root).unwrap();
        for status in ["ringing", "snoozed"] {
            let json = serde_json::json!({"id":status,"alarmId":"deleted", "title":"旧提醒", "scheduledAt":1,"status":status,"snoozeCount":1,"snoozeUntil":9999999999_i64,"startedAt":1,"reason":""});
            conn.execute(
                "INSERT INTO occurrences(id,alarm_id,scheduled_at,json) VALUES(?1,'deleted',1,?2)",
                rusqlite::params![status, json.to_string()],
            )
            .unwrap();
        }
        drop(conn);
        let archive = root.join("active.zip");
        export_to(&root, &archive).unwrap();
        import_from(&root, &archive).unwrap();
        let snap = storage::snapshot(&root).unwrap();
        assert!(
            snap.occurrences
                .iter()
                .all(|o| o.status == "cancelled" && o.snooze_until.is_none())
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn configuration_staging_failure_preserves_database() {
        let root =
            std::env::temp_dir().join(format!("ontime-restore-failure-{}", uuid::Uuid::new_v4()));
        storage::initialize(&root).unwrap();
        let archive = root.join("original.zip");
        export_to(&root, &archive).unwrap();
        let tag = crate::domain::Tag {
            created_at: Some(1234567890),
            id: "keep".into(),
            name: "保留当前数据".into(),
            color: "#123456".into(),
        };
        storage::open(&root)
            .unwrap()
            .execute(
                "INSERT INTO tags(id,json) VALUES(?1,?2)",
                rusqlite::params![tag.id, serde_json::to_string(&tag).unwrap()],
            )
            .unwrap();
        let original_config = fs::read(root.join("config.json")).unwrap();
        fs::create_dir(root.join("config.import.tmp")).unwrap();
        assert!(import_from(&root, &archive).is_err());
        assert_eq!(storage::snapshot(&root).unwrap().tags[0].id, "keep");
        assert_eq!(fs::read(root.join("config.json")).unwrap(), original_config);
        fs::remove_dir_all(root).unwrap();
    }
}
