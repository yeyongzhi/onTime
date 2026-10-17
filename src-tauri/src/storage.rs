use crate::domain::{Alarm, Calendar, Group, Occurrence, Tag};
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
    #[serde(default)]
    pub groups: Vec<Group>,
    pub occurrences: Vec<Occurrence>,
    pub calendars: Vec<Calendar>,
    pub next_runs: std::collections::BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    pub page: u32,
    pub page_size: u32,
    pub keyword: String,
    pub status: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<Occurrence>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
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
    if (1..=4).contains(&version) {
        return Err("数据升级未完成，请在设置中重新选择该数据目录重试；原数据未修改".into());
    }
    if version != 5 {
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
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|e| e.to_string())?;
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version > 5 {
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
    if (1..=3).contains(&version) {
        let backup_dir = root.join("backups");
        fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
        let backup_path = backup_dir.join(format!(
            "before-schema-v{}-{}.sqlite",
            version + 1,
            uuid::Uuid::new_v4()
        ));
        let mut backup_conn = Connection::open(&backup_path).map_err(|e| e.to_string())?;
        rusqlite::backup::Backup::new(&conn, &mut backup_conn)
            .map_err(|e| e.to_string())?
            .run_to_completion(16, std::time::Duration::from_millis(50), None)
            .map_err(|e| format!("无法创建数据库迁移快照：{e}"))?;
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if version == 1 {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS groups(id TEXT PRIMARY KEY,json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tags(id TEXT PRIMARY KEY,json TEXT NOT NULL);
ALTER TABLE alarms ADD COLUMN title TEXT NOT NULL DEFAULT '';
ALTER TABLE alarms ADD COLUMN content TEXT NOT NULL DEFAULT '';
ALTER TABLE alarms ADD COLUMN enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE alarms ADD COLUMN kind TEXT NOT NULL DEFAULT 'clock';
ALTER TABLE alarms ADD COLUMN time TEXT NOT NULL DEFAULT '09:00';
ALTER TABLE alarms ADD COLUMN date TEXT;
ALTER TABLE alarms ADD COLUMN repeat TEXT NOT NULL DEFAULT 'daily';
ALTER TABLE alarms ADD COLUMN monthly_day INTEGER;
ALTER TABLE alarms ADD COLUMN effective_start TEXT;
ALTER TABLE alarms ADD COLUMN effective_end TEXT;
ALTER TABLE alarms ADD COLUMN weekdays TEXT NOT NULL DEFAULT '[]';
ALTER TABLE alarms ADD COLUMN skip_holidays INTEGER NOT NULL DEFAULT 0;
ALTER TABLE alarms ADD COLUMN duration_seconds INTEGER NOT NULL DEFAULT 1500;
ALTER TABLE alarms ADD COLUMN deadline INTEGER;
ALTER TABLE alarms ADD COLUMN sound TEXT;
ALTER TABLE alarms ADD COLUMN volume REAL NOT NULL DEFAULT 0.7;
ALTER TABLE alarms ADD COLUMN gradual INTEGER NOT NULL DEFAULT 1;
ALTER TABLE alarms ADD COLUMN ring_seconds INTEGER NOT NULL DEFAULT 60;
ALTER TABLE alarms ADD COLUMN snooze_limit INTEGER NOT NULL DEFAULT 3;
ALTER TABLE alarms ADD COLUMN group_id TEXT;
ALTER TABLE alarms ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE alarms ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE groups ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE groups ADD COLUMN color TEXT NOT NULL DEFAULT '#64748b';
ALTER TABLE groups ADD COLUMN created_at INTEGER;
ALTER TABLE tags ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE tags ADD COLUMN color TEXT NOT NULL DEFAULT '#64748b';
ALTER TABLE tags ADD COLUMN created_at INTEGER;",
        )
        .map_err(|e| e.to_string())?;
    } else if version == 0 {
        tx.execute_batch("CREATE TABLE IF NOT EXISTS alarms(id TEXT PRIMARY KEY, json TEXT NOT NULL, next_at INTEGER);
CREATE TABLE IF NOT EXISTS groups(id TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tags(id TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS occurrences(id TEXT PRIMARY KEY, alarm_id TEXT NOT NULL, scheduled_at INTEGER NOT NULL, json TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS occurrence_time ON occurrences(scheduled_at);
CREATE TABLE IF NOT EXISTS calendars(year INTEGER PRIMARY KEY, json TEXT NOT NULL);
ALTER TABLE alarms ADD COLUMN title TEXT NOT NULL DEFAULT '';
ALTER TABLE alarms ADD COLUMN content TEXT NOT NULL DEFAULT '';
ALTER TABLE alarms ADD COLUMN enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE alarms ADD COLUMN kind TEXT NOT NULL DEFAULT 'clock';
ALTER TABLE alarms ADD COLUMN time TEXT NOT NULL DEFAULT '09:00';
ALTER TABLE alarms ADD COLUMN date TEXT;
ALTER TABLE alarms ADD COLUMN repeat TEXT NOT NULL DEFAULT 'daily';
ALTER TABLE alarms ADD COLUMN monthly_day INTEGER;
ALTER TABLE alarms ADD COLUMN effective_start TEXT;
ALTER TABLE alarms ADD COLUMN effective_end TEXT;
ALTER TABLE alarms ADD COLUMN weekdays TEXT NOT NULL DEFAULT '[]';
ALTER TABLE alarms ADD COLUMN skip_holidays INTEGER NOT NULL DEFAULT 0;
ALTER TABLE alarms ADD COLUMN duration_seconds INTEGER NOT NULL DEFAULT 1500;
ALTER TABLE alarms ADD COLUMN deadline INTEGER;
ALTER TABLE alarms ADD COLUMN sound TEXT;
ALTER TABLE alarms ADD COLUMN volume REAL NOT NULL DEFAULT 0.7;
ALTER TABLE alarms ADD COLUMN gradual INTEGER NOT NULL DEFAULT 1;
ALTER TABLE alarms ADD COLUMN ring_seconds INTEGER NOT NULL DEFAULT 60;
ALTER TABLE alarms ADD COLUMN snooze_limit INTEGER NOT NULL DEFAULT 3;
ALTER TABLE alarms ADD COLUMN group_id TEXT;
ALTER TABLE alarms ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE alarms ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE groups ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE groups ADD COLUMN color TEXT NOT NULL DEFAULT '#64748b';
ALTER TABLE groups ADD COLUMN created_at INTEGER;
ALTER TABLE tags ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE tags ADD COLUMN color TEXT NOT NULL DEFAULT '#64748b';
ALTER TABLE tags ADD COLUMN created_at INTEGER;").map_err(|e| e.to_string())?;
    }
    if (1..=3).contains(&version) {
        // v3 treated day 31 as the last day of shorter months; preserve that behavior after separating the rules.
        tx.execute(
            "UPDATE alarms SET json=json_set(json,'$.monthlyDay',0) WHERE json_extract(json,'$.repeat')='monthly' AND (json_extract(json,'$.monthlyDay')=31 OR (json_extract(json,'$.monthlyDay') IS NULL AND substr(json_extract(json,'$.date'),9,2)='31'))",
            [],
        ).map_err(|e| format!("无法迁移每月月底规则：{e}"))?;
    }
    tx.execute_batch("DROP TRIGGER IF EXISTS alarms_project_insert;
DROP TRIGGER IF EXISTS alarms_project_update;
DROP TRIGGER IF EXISTS groups_project_insert;
DROP TRIGGER IF EXISTS groups_project_update;
DROP TRIGGER IF EXISTS tags_project_insert;
DROP TRIGGER IF EXISTS tags_project_update;
CREATE TABLE IF NOT EXISTS alarm_tags(alarm_id TEXT NOT NULL REFERENCES alarms(id) ON DELETE CASCADE, tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE, PRIMARY KEY(alarm_id,tag_id));
CREATE TABLE IF NOT EXISTS alarm_groups(alarm_id TEXT PRIMARY KEY REFERENCES alarms(id) ON DELETE CASCADE, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE);
CREATE INDEX IF NOT EXISTS alarm_tags_by_tag ON alarm_tags(tag_id);
CREATE INDEX IF NOT EXISTS alarm_groups_by_group ON alarm_groups(group_id);
CREATE INDEX IF NOT EXISTS alarms_by_group ON alarms(group_id);
CREATE INDEX IF NOT EXISTS alarms_by_enabled_next ON alarms(enabled,next_at);
CREATE INDEX IF NOT EXISTS alarms_by_title ON alarms(title);
CREATE TRIGGER IF NOT EXISTS alarms_project_insert AFTER INSERT ON alarms BEGIN
 UPDATE alarms SET title=json_extract(NEW.json,'$.title'),content=COALESCE(json_extract(NEW.json,'$.content'),''),enabled=COALESCE(json_extract(NEW.json,'$.enabled'),0),kind=COALESCE(json_extract(NEW.json,'$.kind'),'clock'),time=COALESCE(json_extract(NEW.json,'$.time'),'09:00'),date=json_extract(NEW.json,'$.date'),repeat=COALESCE(json_extract(NEW.json,'$.repeat'),'daily'),monthly_day=json_extract(NEW.json,'$.monthlyDay'),effective_start=json_extract(NEW.json,'$.effectiveStart'),effective_end=json_extract(NEW.json,'$.effectiveEnd'),weekdays=COALESCE(json_extract(NEW.json,'$.weekdays'),'[]'),duration_seconds=COALESCE(json_extract(NEW.json,'$.durationSeconds'),1500),deadline=json_extract(NEW.json,'$.deadline'),sound=json_extract(NEW.json,'$.sound'),volume=COALESCE(json_extract(NEW.json,'$.volume'),0.7),gradual=COALESCE(json_extract(NEW.json,'$.gradual'),1),ring_seconds=COALESCE(json_extract(NEW.json,'$.ringSeconds'),60),snooze_limit=COALESCE(json_extract(NEW.json,'$.snoozeLimit'),3),group_id=json_extract(NEW.json,'$.groupId'),revision=COALESCE(json_extract(NEW.json,'$.revision'),0),created_at=COALESCE(json_extract(NEW.json,'$.createdAt'),0) WHERE id=NEW.id;
 DELETE FROM alarm_tags WHERE alarm_id=NEW.id;
 INSERT OR IGNORE INTO alarm_tags(alarm_id,tag_id) SELECT NEW.id,value FROM json_each(NEW.json,'$.tags') WHERE type='text';
 DELETE FROM alarm_groups WHERE alarm_id=NEW.id;
 INSERT OR IGNORE INTO alarm_groups(alarm_id,group_id) SELECT NEW.id,json_extract(NEW.json,'$.groupId') WHERE json_extract(NEW.json,'$.groupId') IS NOT NULL;
END;
CREATE TRIGGER IF NOT EXISTS alarms_project_update AFTER UPDATE OF json ON alarms BEGIN
 UPDATE alarms SET title=json_extract(NEW.json,'$.title'),content=COALESCE(json_extract(NEW.json,'$.content'),''),enabled=COALESCE(json_extract(NEW.json,'$.enabled'),0),kind=COALESCE(json_extract(NEW.json,'$.kind'),'clock'),time=COALESCE(json_extract(NEW.json,'$.time'),'09:00'),date=json_extract(NEW.json,'$.date'),repeat=COALESCE(json_extract(NEW.json,'$.repeat'),'daily'),monthly_day=json_extract(NEW.json,'$.monthlyDay'),effective_start=json_extract(NEW.json,'$.effectiveStart'),effective_end=json_extract(NEW.json,'$.effectiveEnd'),weekdays=COALESCE(json_extract(NEW.json,'$.weekdays'),'[]'),duration_seconds=COALESCE(json_extract(NEW.json,'$.durationSeconds'),1500),deadline=json_extract(NEW.json,'$.deadline'),sound=json_extract(NEW.json,'$.sound'),volume=COALESCE(json_extract(NEW.json,'$.volume'),0.7),gradual=COALESCE(json_extract(NEW.json,'$.gradual'),1),ring_seconds=COALESCE(json_extract(NEW.json,'$.ringSeconds'),60),snooze_limit=COALESCE(json_extract(NEW.json,'$.snoozeLimit'),3),group_id=json_extract(NEW.json,'$.groupId'),revision=COALESCE(json_extract(NEW.json,'$.revision'),0),created_at=COALESCE(json_extract(NEW.json,'$.createdAt'),0) WHERE id=NEW.id;
 DELETE FROM alarm_tags WHERE alarm_id=NEW.id;
 INSERT OR IGNORE INTO alarm_tags(alarm_id,tag_id) SELECT NEW.id,value FROM json_each(NEW.json,'$.tags') WHERE type='text';
 DELETE FROM alarm_groups WHERE alarm_id=NEW.id;
 INSERT OR IGNORE INTO alarm_groups(alarm_id,group_id) SELECT NEW.id,json_extract(NEW.json,'$.groupId') WHERE json_extract(NEW.json,'$.groupId') IS NOT NULL;
END;
CREATE TRIGGER IF NOT EXISTS groups_project_insert AFTER INSERT ON groups BEGIN UPDATE groups SET name=json_extract(NEW.json,'$.name'),color=COALESCE(json_extract(NEW.json,'$.color'),'#64748b'),created_at=json_extract(NEW.json,'$.createdAt') WHERE id=NEW.id; END;
CREATE TRIGGER IF NOT EXISTS groups_project_update AFTER UPDATE OF json ON groups BEGIN UPDATE groups SET name=json_extract(NEW.json,'$.name'),color=COALESCE(json_extract(NEW.json,'$.color'),'#64748b'),created_at=json_extract(NEW.json,'$.createdAt') WHERE id=NEW.id; END;
CREATE TRIGGER IF NOT EXISTS tags_project_insert AFTER INSERT ON tags BEGIN UPDATE tags SET name=json_extract(NEW.json,'$.name'),color=COALESCE(json_extract(NEW.json,'$.color'),'#64748b'),created_at=json_extract(NEW.json,'$.createdAt') WHERE id=NEW.id; END;
CREATE TRIGGER IF NOT EXISTS tags_project_update AFTER UPDATE OF json ON tags BEGIN UPDATE tags SET name=json_extract(NEW.json,'$.name'),color=COALESCE(json_extract(NEW.json,'$.color'),'#64748b'),created_at=json_extract(NEW.json,'$.createdAt') WHERE id=NEW.id; END;
UPDATE alarms SET json=json;
UPDATE groups SET json=json;
UPDATE tags SET json=json;
").map_err(|e| e.to_string())?;
    if version < 5 {
        let columns = {
            let mut stmt = tx
                .prepare("PRAGMA table_info(occurrences)")
                .map_err(|e| e.to_string())?;
            stmt.query_map([], |row| row.get::<_, String>(1))
                .map_err(|e| e.to_string())?
                .collect::<Result<std::collections::BTreeSet<_>, _>>()
                .map_err(|e| e.to_string())?
        };
        for (name, declaration) in [
            ("title", "TEXT NOT NULL DEFAULT ''"),
            ("status", "TEXT NOT NULL DEFAULT ''"),
            ("snooze_count", "INTEGER NOT NULL DEFAULT 0"),
            ("started_at", "INTEGER"),
            ("updated_at", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            if !columns.contains(name) {
                tx.execute_batch(&format!(
                    "ALTER TABLE occurrences ADD COLUMN {name} {declaration};"
                ))
                .map_err(|e| e.to_string())?;
            }
        }
        tx.execute_batch("UPDATE occurrences SET title=COALESCE(json_extract(json,'$.title'),''),status=COALESCE(json_extract(json,'$.status'),''),snooze_count=COALESCE(json_extract(json,'$.snoozeCount'),0),started_at=json_extract(json,'$.startedAt'),updated_at=COALESCE(json_extract(json,'$.startedAt'),scheduled_at);
CREATE INDEX IF NOT EXISTS occurrence_status_time ON occurrences(status,scheduled_at DESC);
CREATE INDEX IF NOT EXISTS occurrence_title ON occurrences(title COLLATE NOCASE);
CREATE TRIGGER IF NOT EXISTS occurrences_project_insert AFTER INSERT ON occurrences BEGIN UPDATE occurrences SET title=COALESCE(json_extract(NEW.json,'$.title'),''),status=COALESCE(json_extract(NEW.json,'$.status'),''),snooze_count=COALESCE(json_extract(NEW.json,'$.snoozeCount'),0),started_at=json_extract(NEW.json,'$.startedAt'),updated_at=COALESCE(json_extract(NEW.json,'$.startedAt'),NEW.scheduled_at) WHERE id=NEW.id; END;
CREATE TRIGGER IF NOT EXISTS occurrences_project_update AFTER UPDATE OF json ON occurrences BEGIN UPDATE occurrences SET title=COALESCE(json_extract(NEW.json,'$.title'),''),status=COALESCE(json_extract(NEW.json,'$.status'),''),snooze_count=COALESCE(json_extract(NEW.json,'$.snoozeCount'),0),started_at=json_extract(NEW.json,'$.startedAt'),updated_at=CAST(strftime('%s','now') AS INTEGER) WHERE id=NEW.id; END;
PRAGMA user_version=5;").map_err(|e| e.to_string())?;
    }
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
        groups: read_json(&conn, "SELECT json FROM groups ORDER BY rowid")?,
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

pub fn history_page(root: &Path, filter: HistoryFilter) -> Result<HistoryPage, String> {
    let conn = open(root)?;
    let page_size = filter.page_size.clamp(10, 100);
    let page = filter.page.max(1);
    let keyword = filter
        .keyword
        .trim()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{keyword}%");
    let status = filter.status.trim();
    let total: i64 = conn.query_row(
        "SELECT count(*) FROM occurrences WHERE (?1='' OR title LIKE ?2 ESCAPE '\\') AND (?3='' OR status=?3) AND (?4 IS NULL OR scheduled_at>=?4) AND (?5 IS NULL OR scheduled_at<?5)",
        rusqlite::params![keyword, pattern, status, filter.from, filter.to],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let offset = i64::from(page - 1) * i64::from(page_size);
    let mut stmt = conn.prepare("SELECT json FROM occurrences WHERE (?1='' OR title LIKE ?2 ESCAPE '\\') AND (?3='' OR status=?3) AND (?4 IS NULL OR scheduled_at>=?4) AND (?5 IS NULL OR scheduled_at<?5) ORDER BY scheduled_at DESC LIMIT ?6 OFFSET ?7").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            rusqlite::params![
                keyword,
                pattern,
                status,
                filter.from,
                filter.to,
                page_size,
                offset
            ],
            |row| row.get::<_, String>(0),
        )
        .map_err(|e| e.to_string())?;
    let items = rows
        .map(|row| {
            let json = row.map_err(|e| e.to_string())?;
            serde_json::from_str(&json).map_err(|e| format!("提醒历史数据无效：{e}"))
        })
        .collect::<Result<Vec<Occurrence>, String>>()?;
    Ok(HistoryPage {
        items,
        total: total.max(0) as u64,
        page,
        page_size,
    })
}

pub fn automatic_backup(root: &Path, day: &str) -> Result<(), String> {
    let backups = root.join("backups");
    fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
    let target = backups.join(format!("automatic-{day}.ontime.zip"));
    if target.exists() {
        return Ok(());
    }
    if let Err(error) = crate::backup::export_to(root, &target) {
        let _ = fs::remove_file(&target);
        return Err(error);
    }
    let mut dated = fs::read_dir(&backups)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("automatic-") && name.ends_with(".ontime.zip"))
        })
        .collect::<Vec<_>>();
    dated.sort();
    while dated.len() > 7 {
        let old = dated.remove(0);
        fs::remove_file(old).map_err(|e| e.to_string())?;
    }
    Ok(())
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
        serde_json::from_value(serde_json::json!({"id":"test","title":"提醒","content":"","enabled":true,"kind":"timer","time":"09:00","date":null,"repeat":"daily","weekdays":[],"skipHolidays":false,"durationSeconds":60,"deadline":120,"sound":null,"volume":0.5,"gradual":true,"ringSeconds":60,"snoozeLimit":3,"tags":[],"revision":1,"createdAt":1})).unwrap()
    }
    #[test]
    fn alarm_fields_and_tag_links_are_projected_into_sqlite_columns() {
        let root = TestRoot::new();
        let a = alarm();
        save_alarm(&root.0, &a, Some(120)).unwrap();
        let conn = open(&root.0).unwrap();
        let title: String = conn
            .query_row("SELECT title FROM alarms WHERE id='test'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "提醒");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 5);
        let snap = snapshot(&root.0).unwrap();
        assert!(snap.groups.is_empty());
        assert_eq!(snap.alarms[0].revision, 1);
        assert_eq!(snap.next_runs.get("test"), Some(&120));
    }
    #[test]
    fn schema_v3_monthly_day_31_migrates_to_distinct_month_end_rule() {
        let root = TestRoot::new();
        let mut alarm = alarm();
        alarm.kind = "clock".into();
        alarm.date = Some("2026-01-31".into());
        alarm.repeat = "monthly".into();
        alarm.monthly_day = Some(31);
        save_alarm(&root.0, &alarm, Some(120)).unwrap();
        let conn = open(&root.0).unwrap();
        conn.execute_batch("PRAGMA user_version=3;").unwrap();
        drop(conn);

        initialize(&root.0).unwrap();
        let snap = snapshot(&root.0).unwrap();
        assert_eq!(snap.alarms[0].monthly_day, Some(0));
        let conn = open(&root.0).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 5);
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
            "INSERT INTO occurrences(id,alarm_id,scheduled_at,json) VALUES(?1,?2,?3,?4)",
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
                "INSERT INTO occurrences(id,alarm_id,scheduled_at,json) VALUES(?1,?2,?3,?4)",
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
    fn history_query_filters_and_paginates_in_sqlite() {
        let root = TestRoot::new();
        let conn = open(&root.0).unwrap();
        for n in 0..23 {
            let o = Occurrence {
                id: format!("history-{n}"),
                alarm_id: "test".into(),
                title: if n % 2 == 0 {
                    "用药提醒"
                } else {
                    "会议提醒"
                }
                .into(),
                scheduled_at: 1_000 + n,
                status: if n % 3 == 0 {
                    "skipped_expired"
                } else {
                    "dismissed"
                }
                .into(),
                snooze_count: (n % 2) as u32,
                snooze_until: None,
                started_at: Some(1_000 + n),
                reason: "测试".into(),
            };
            conn.execute(
                "INSERT INTO occurrences(id,alarm_id,scheduled_at,json) VALUES(?1,?2,?3,?4)",
                params![
                    o.id,
                    o.alarm_id,
                    o.scheduled_at,
                    serde_json::to_string(&o).unwrap()
                ],
            )
            .unwrap();
        }
        let filtered = history_page(
            &root.0,
            HistoryFilter {
                page: 1,
                page_size: 10,
                keyword: "用药".into(),
                status: "skipped_expired".into(),
                from: Some(1_000),
                to: Some(2_000),
            },
        )
        .unwrap();
        assert_eq!(filtered.total, 4);
        assert_eq!(filtered.items.len(), 4);
        assert!(
            filtered
                .items
                .iter()
                .all(|item| item.title == "用药提醒" && item.status == "skipped_expired")
        );
        let second_page = history_page(
            &root.0,
            HistoryFilter {
                page: 2,
                page_size: 10,
                keyword: String::new(),
                status: String::new(),
                from: None,
                to: None,
            },
        )
        .unwrap();
        assert_eq!(second_page.total, 23);
        assert_eq!(second_page.items.len(), 10);
    }
    #[test]
    fn future_database_version_is_refused_without_modification() {
        let root = TestRoot::new();
        let conn = open(&root.0).unwrap();
        conn.execute_batch("PRAGMA user_version=6;").unwrap();
        drop(conn);
        let before = fs::read(root.0.join("data.db")).unwrap();
        assert!(open(&root.0).is_err());
        assert_eq!(fs::read(root.0.join("data.db")).unwrap(), before);
    }
}
