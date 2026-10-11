use crate::domain::{Alarm, Calendar, Occurrence, Tag};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub data_root: String,
    pub alarms: Vec<Alarm>,
    pub tags: Vec<Tag>,
    pub occurrences: Vec<Occurrence>,
    pub calendars: Vec<Calendar>,
    pub next_runs: std::collections::BTreeMap<String, i64>,
}

pub fn open(root: &Path) -> Result<Connection, String> {
    if !root.is_dir() {
        return Err("数据目录不可用，请重新定位".into());
    }
    let file = root.join("data.db");
    if !file.is_file() {
        return Err("数据文件不存在，请重新选择原目录".into());
    }
    let conn = Connection::open_with_flags(file, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| e.to_string())?;
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version != 1 {
        return Err("数据版本不兼容，请使用匹配版本的 onTime；原数据未修改".into());
    }
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

pub fn initialize(root: &Path) -> Result<(), String> {
    let text = root.to_string_lossy().to_lowercase();
    if text.contains(".app/") || text.ends_with(".app") || text.contains(".app\\") {
        return Err("不能将应用包内部作为数据目录".into());
    }
    fs::create_dir_all(root).map_err(|e| format!("无法创建数据目录：{e}"))?;
    let probe = root.join(format!(".ontime-write-{}", uuid::Uuid::new_v4()));
    fs::write(&probe, b"test").map_err(|e| format!("目录不可写：{e}"))?;
    fs::remove_file(probe).map_err(|e| e.to_string())?;
    let db = root.join("data.db");
    let exists = db.exists();
    let mut conn = Connection::open(&db).map_err(|e| e.to_string())?;
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version > 1 {
        return Err("数据版本较新，请升级 onTime 后打开".into());
    }
    if exists && version == 0 {
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count != 0 {
            return Err("此目录的数据库不属于 onTime，未覆盖".into());
        }
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS alarms(id TEXT PRIMARY KEY, json TEXT NOT NULL, next_at INTEGER);
CREATE TABLE IF NOT EXISTS tags(id TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS occurrences(id TEXT PRIMARY KEY, alarm_id TEXT NOT NULL, scheduled_at INTEGER NOT NULL, json TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS occurrence_time ON occurrences(scheduled_at);
CREATE TABLE IF NOT EXISTS calendars(year INTEGER PRIMARY KEY, json TEXT NOT NULL);
PRAGMA user_version=1;").map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    for dir in ["resources/sounds", "calendar", "logs", "backups"] {
        fs::create_dir_all(root.join(dir)).map_err(|e| e.to_string())?;
    }
    let config = root.join("config.json");
    if !config.exists() {
        fs::write(config, b"{\"schemaVersion\":1,\"recordRetentionDays\":90}")
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(
    conn: &Connection,
    sql: &str,
) -> Result<Vec<T>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| {
        let s = r.map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("数据内容无效：{e}"))
    })
    .collect()
}

pub fn snapshot(root: &Path) -> Result<Snapshot, String> {
    let conn = open(root)?;
    Ok(Snapshot {
        data_root: root.to_string_lossy().into(),
        alarms: read_json(&conn, "SELECT json FROM alarms ORDER BY rowid DESC")?,
        tags: read_json(&conn, "SELECT json FROM tags ORDER BY rowid")?,
        occurrences: read_json(
            &conn,
            "SELECT json FROM occurrences WHERE id IN (SELECT id FROM occurrences ORDER BY scheduled_at DESC LIMIT 500) OR json_extract(json, '$.status') IN ('ringing','snoozed') ORDER BY scheduled_at DESC",
        )?,
        calendars: read_json(&conn, "SELECT json FROM calendars ORDER BY year DESC")?,
        next_runs: {
            let mut stmt = conn
                .prepare("SELECT id,next_at FROM alarms WHERE next_at IS NOT NULL")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        },
    })
}

pub fn save_alarm(root: &Path, alarm: &Alarm, next: Option<i64>) -> Result<(), String> {
    alarm.validate()?;
    let mut conn = open(root)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let previous: Option<String> = tx
        .query_row("SELECT json FROM alarms WHERE id=?1", [&alarm.id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(previous) = previous {
        let old: Alarm = serde_json::from_str(&previous).map_err(|e| e.to_string())?;
        if alarm.revision != old.revision + 1 {
            return Err("提醒已发生变化，请刷新后重试".into());
        }
    }
    {
        let mut stmt = tx
            .prepare("SELECT json FROM occurrences WHERE alarm_id=?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&alarm.id], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let mut o: Occurrence = serde_json::from_str(&row.map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            if ["ringing", "snoozed"].contains(&o.status.as_str()) {
                o.status = "cancelled".into();
                tx.execute(
                    "UPDATE occurrences SET json=?2 WHERE id=?1",
                    params![o.id, serde_json::to_string(&o).map_err(|e| e.to_string())?],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    tx.execute("INSERT INTO alarms(id,json,next_at) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET json=excluded.json,next_at=excluded.next_at", params![alarm.id, serde_json::to_string(alarm).map_err(|e| e.to_string())?, next]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

pub fn safe_sound(root: &Path, name: &str) -> Result<PathBuf, String> {
    let relative = Path::new(name);
    if relative.components().count() != 1
        || !matches!(
            relative.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err("无效的铃声路径".into());
    }
    let sounds = root
        .join("resources/sounds")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let result = sounds
        .join(relative)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !result.starts_with(&sounds) {
        return Err("铃声不在授权目录".into());
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct TestRoot(PathBuf);
    impl TestRoot {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("ontime-test-{}", uuid::Uuid::new_v4()));
            initialize(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn alarm() -> Alarm {
        serde_json::from_value(serde_json::json!({"id":"test","title":"提醒","enabled":true,"kind":"timer","time":"09:00","date":null,"repeat":"daily","weekdays":[],"skipHolidays":false,"durationSeconds":60,"deadline":120,"sound":null,"volume":0.5,"gradual":true,"ringSeconds":60,"snoozeLimit":3,"tags":[],"revision":1,"createdAt":1})).unwrap()
    }
    #[test]
    fn persistence_and_revision_conflict() {
        let root = TestRoot::new();
        let mut a = alarm();
        save_alarm(&root.0, &a, Some(120)).unwrap();
        assert_eq!(snapshot(&root.0).unwrap().next_runs.get("test"), Some(&120));
        assert!(save_alarm(&root.0, &a, Some(500)).is_err());
        assert_eq!(snapshot(&root.0).unwrap().next_runs.get("test"), Some(&120));
        a.revision = 2;
        a.title = "新标题".into();
        save_alarm(&root.0, &a, Some(500)).unwrap();
        assert_eq!(snapshot(&root.0).unwrap().alarms[0].title, "新标题");
    }
    #[test]
    fn edit_cancels_active_in_same_transaction() {
        let root = TestRoot::new();
        let mut a = alarm();
        save_alarm(&root.0, &a, Some(120)).unwrap();
        let conn = open(&root.0).unwrap();
        let o = Occurrence {
            id: "o".into(),
            alarm_id: a.id.clone(),
            title: a.title.clone(),
            scheduled_at: 120,
            status: "ringing".into(),
            snooze_count: 0,
            snooze_until: None,
            started_at: Some(120),
            reason: "准时".into(),
        };
        conn.execute(
            "INSERT INTO occurrences VALUES(?1,?2,?3,?4)",
            params![
                o.id,
                o.alarm_id,
                o.scheduled_at,
                serde_json::to_string(&o).unwrap()
            ],
        )
        .unwrap();
        assert!(save_alarm(&root.0, &a, Some(300)).is_err());
        assert_eq!(snapshot(&root.0).unwrap().occurrences[0].status, "ringing");
        a.revision = 2;
        save_alarm(&root.0, &a, Some(300)).unwrap();
        assert_eq!(
            snapshot(&root.0).unwrap().occurrences[0].status,
            "cancelled"
        );
    }
    #[test]
    fn unavailable_root_not_silently_recreated() {
        let root = TestRoot::new();
        let missing = root.0.join("missing");
        assert!(open(&missing).is_err());
        assert!(!missing.exists());
    }
    #[test]
    fn sound_path_traversal_rejected() {
        let root = TestRoot::new();
        assert!(safe_sound(&root.0, "../../data.db").is_err());
        assert!(safe_sound(&root.0, "data.db").is_err());
    }
    #[test]
    fn active_occurrence_survives_history_limit() {
        let root = TestRoot::new();
        let mut conn = open(&root.0).unwrap();
        let tx = conn.transaction().unwrap();
        for n in 0..=501 {
            let o = Occurrence {
                id: format!("record-{n}"),
                alarm_id: "test".into(),
                title: "提醒".into(),
                scheduled_at: n,
                status: if n == 0 { "snoozed" } else { "dismissed" }.into(),
                snooze_count: 1,
                snooze_until: Some(1000),
                started_at: None,
                reason: "测试".into(),
            };
            tx.execute(
                "INSERT INTO occurrences VALUES(?1,?2,?3,?4)",
                params![
                    o.id,
                    o.alarm_id,
                    o.scheduled_at,
                    serde_json::to_string(&o).unwrap()
                ],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        let snap = snapshot(&root.0).unwrap();
        assert_eq!(snap.occurrences.len(), 501);
        assert!(
            snap.occurrences
                .iter()
                .any(|o| o.id == "record-0" && o.status == "snoozed")
        );
    }
    #[test]
    fn future_database_version_is_refused_without_modification() {
        let root = TestRoot::new();
        let conn = open(&root.0).unwrap();
        conn.execute_batch("PRAGMA user_version=2;").unwrap();
        drop(conn);
        let before = fs::read(root.0.join("data.db")).unwrap();
        assert!(open(&root.0).is_err());
        assert_eq!(fs::read(root.0.join("data.db")).unwrap(), before);
    }
}
