use crate::{
    audio::{AudioHandle, Play},
    domain::{Alarm, Calendar, Occurrence, Tag, eligible_for_catchup},
    storage::{self, Snapshot},
};
use chrono::{Datelike, Local, Utc};
use rusqlite::{Connection, params};
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;

pub struct Engine {
    pub root: Mutex<Option<PathBuf>>,
    pub audio: AudioHandle,
}
fn error<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
fn persist_occurrence(conn: &Connection, o: &Occurrence) -> Result<(), String> {
    conn.execute("INSERT INTO occurrences(id,alarm_id,scheduled_at,json) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET json=excluded.json", params![o.id, o.alarm_id, o.scheduled_at, serde_json::to_string(o).map_err(error)?]).map_err(error)?;
    Ok(())
}
fn calendar_for(calendars: &[Calendar]) -> Option<&Calendar> {
    calendars.iter().find(|c| c.year == Local::now().year())
}
fn cancel_instances(conn: &Connection, alarm_id: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT json FROM occurrences WHERE alarm_id=?1")
        .map_err(error)?;
    let rows = stmt
        .query_map([alarm_id], |r| r.get::<_, String>(0))
        .map_err(error)?;
    for row in rows {
        let mut o: Occurrence = serde_json::from_str(&row.map_err(error)?).map_err(error)?;
        if ["ringing", "snoozed"].contains(&o.status.as_str()) {
            o.status = "cancelled".into();
            persist_occurrence(conn, &o)?;
        }
    }
    Ok(())
}
fn show_alert(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("alert") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "alert",
        tauri::WebviewUrl::App("index.html#/alert".into()),
    )
    .title("onTime · 时间到了")
    .inner_size(500.0, 600.0)
    .min_inner_size(420.0, 360.0)
    .always_on_top(true)
    .build();
}
fn announce(app: &AppHandle, o: &Occurrence) {
    let _ = app
        .notification()
        .builder()
        .title("onTime · 时间到了")
        .body(&o.title)
        .show();
    show_alert(app);
}
fn root(state: &Engine) -> Result<std::sync::MutexGuard<'_, Option<PathBuf>>, String> {
    let guard = state.root.lock().map_err(error)?;
    if guard.is_none() {
        return Err("请先选择数据目录".into());
    }
    Ok(guard)
}
fn notify(app: &AppHandle) {
    let _ = app.emit("state-changed", ());
}

#[tauri::command]
pub fn get_snapshot(state: tauri::State<Engine>) -> Result<Option<Snapshot>, String> {
    let guard = state.root.lock().map_err(error)?;
    guard.as_ref().map(|p| storage::snapshot(p)).transpose()
}
#[tauri::command]
pub async fn choose_data_root(app: AppHandle) -> Result<Option<Snapshot>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(folder) = app
            .dialog()
            .file()
            .set_title("选择 onTime 数据目录")
            .blocking_pick_folder()
        else {
            return Ok(None);
        };
        let path = folder.into_path().map_err(error)?;
        let state = app.state::<Engine>();
        let mut guard = state.root.lock().map_err(error)?;
        storage::initialize(&path)?;
        crate::calendar::seed(&path)?;
        apply_clock_change_database(
            &path,
            crate::clock::ClockChange {
                shift_seconds: 0,
                timezone_changed: true,
            },
            Utc::now().timestamp(),
        )?;
        let snap = storage::snapshot(&path)?;
        let bootstrap = app.path().app_data_dir().map_err(error)?;
        fs::create_dir_all(&bootstrap).map_err(error)?;
        let temp = bootstrap.join("bootstrap.tmp");
        fs::write(&temp, serde_json::to_vec(&path).map_err(error)?).map_err(error)?;
        fs::rename(temp, bootstrap.join("bootstrap.json")).map_err(error)?;
        crate::logging::set_root(Some(&path));
        *guard = Some(path);
        state.audio.stop();
        drop(guard);
        notify(&app);
        Ok(Some(snap))
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub fn save_alarm(
    app: AppHandle,
    state: tauri::State<Engine>,
    mut alarm: Alarm,
) -> Result<(), String> {
    alarm.title = alarm.title.trim().into();
    alarm.validate()?;
    let guard = root(&state)?;
    let path = guard.as_ref().unwrap();
    let snap = storage::snapshot(path)?;
    let old = snap.alarms.iter().find(|a| a.id == alarm.id);
    if let Some(old) = old {
        if alarm.revision != old.revision {
            return Err("提醒已变化，请关闭编辑器后重新打开".into());
        }
    }
    alarm.revision = old.map_or(1, |a| a.revision + 1);
    if alarm.kind == "timer" && alarm.enabled {
        alarm.deadline = Some(Utc::now().timestamp() + alarm.duration_seconds);
    }
    if old.is_none() {
        alarm.id = uuid::Uuid::new_v4().to_string();
        alarm.created_at = Utc::now().timestamp();
    }
    let next = alarm.next_after(Utc::now().timestamp(), calendar_for(&snap.calendars));
    if alarm.enabled && next.is_none() {
        return Err("请设置未来的提醒时间".into());
    }

    storage::save_alarm(path, &alarm, next)?;
    drop(guard);
    notify(&app);
    Ok(())
}
#[tauri::command]
pub fn delete_alarm(app: AppHandle, state: tauri::State<Engine>, id: String) -> Result<(), String> {
    let guard = root(&state)?;
    let mut conn = storage::open(guard.as_ref().unwrap())?;
    let tx = conn.transaction().map_err(error)?;
    cancel_instances(&tx, &id)?;
    tx.execute("DELETE FROM alarms WHERE id=?1", [id])
        .map_err(error)?;
    tx.commit().map_err(error)?;
    drop(guard);
    notify(&app);
    Ok(())
}
fn set_enabled_database(
    path: &std::path::Path,
    ids: &[String],
    enabled: bool,
    now: i64,
) -> Result<(), String> {
    let snap = storage::snapshot(path)?;
    let mut conn = storage::open(path)?;
    let tx = conn.transaction().map_err(error)?;

    for mut alarm in snap.alarms.into_iter().filter(|a| ids.contains(&a.id)) {
        if alarm.enabled == enabled {
            continue;
        }
        alarm.enabled = enabled;
        alarm.revision += 1;
        if enabled && alarm.kind == "timer" {
            alarm.deadline = Some(now + alarm.duration_seconds);
        }
        let next = alarm.next_after(now, calendar_for(&snap.calendars));
        if enabled && next.is_none() {
            return Err(format!("{} 的一次性时间已过，请先编辑", alarm.title));
        }
        cancel_instances(&tx, &alarm.id)?;
        tx.execute(
            "UPDATE alarms SET json=?2,next_at=?3 WHERE id=?1",
            params![
                alarm.id,
                serde_json::to_string(&alarm).map_err(error)?,
                next
            ],
        )
        .map_err(error)?;
    }
    tx.commit().map_err(error)?;
    Ok(())
}

#[tauri::command]
pub fn set_enabled(
    app: AppHandle,
    state: tauri::State<Engine>,
    ids: Vec<String>,
    enabled: bool,
) -> Result<(), String> {
    let guard = root(&state)?;
    let path = guard.as_ref().unwrap();
    set_enabled_database(path, &ids, enabled, Utc::now().timestamp())?;
    drop(guard);
    notify(&app);
    Ok(())
}
#[tauri::command]
pub fn save_tag(app: AppHandle, state: tauri::State<Engine>, mut tag: Tag) -> Result<(), String> {
    tag.name = tag.name.trim().into();
    if tag.name.is_empty()
        || tag.name.chars().count() > 24
        || tag.color.len() != 7
        || !tag.color.starts_with('#')
        || !tag.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("标签名称或颜色无效".into());
    }
    if tag.id.is_empty() {
        tag.id = uuid::Uuid::new_v4().to_string();
    }
    let guard = root(&state)?;
    let conn = storage::open(guard.as_ref().unwrap())?;
    conn.execute(
        "INSERT INTO tags(id,json) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        params![tag.id, serde_json::to_string(&tag).map_err(error)?],
    )
    .map_err(error)?;
    drop(guard);
    notify(&app);
    Ok(())
}
#[tauri::command]
pub fn delete_tag(app: AppHandle, state: tauri::State<Engine>, id: String) -> Result<(), String> {
    let guard = root(&state)?;
    let path = guard.as_ref().unwrap();
    let snap = storage::snapshot(path)?;
    let mut conn = storage::open(path)?;
    let tx = conn.transaction().map_err(error)?;
    tx.execute("DELETE FROM tags WHERE id=?1", [&id])
        .map_err(error)?;
    for mut a in snap.alarms {
        if a.tags.contains(&id) {
            a.tags.retain(|t| t != &id);
            a.revision += 1;
            tx.execute(
                "UPDATE alarms SET json=?2 WHERE id=?1",
                params![a.id, serde_json::to_string(&a).map_err(error)?],
            )
            .map_err(error)?;
        }
    }
    tx.commit().map_err(error)?;
    drop(guard);
    notify(&app);
    Ok(())
}
#[tauri::command]
pub fn settle_occurrence(
    app: AppHandle,
    state: tauri::State<Engine>,
    id: String,
    snooze_minutes: Option<u32>,
) -> Result<(), String> {
    let guard = root(&state)?;
    let path = guard.as_ref().unwrap();
    let snap = storage::snapshot(path)?;
    let mut o = snap
        .occurrences
        .into_iter()
        .find(|o| o.id == id)
        .ok_or("提醒不存在")?;
    if o.status != "ringing" {
        return Err("提醒已结束，请刷新".into());
    }
    let a = snap
        .alarms
        .iter()
        .find(|a| a.id == o.alarm_id)
        .ok_or("提醒已删除")?;
    if let Some(m) = snooze_minutes {
        if ![5, 10].contains(&m) || o.snooze_count >= a.snooze_limit {
            return Err("已达到贪睡上限".into());
        }
        o.snooze_count += 1;
        o.snooze_until = Some(Utc::now().timestamp() + i64::from(m) * 60);
        o.status = "snoozed".into();
    } else {
        o.status = "dismissed".into();
    }
    let conn = storage::open(path)?;
    persist_occurrence(&conn, &o)?;
    drop(guard);
    state.audio.stop();
    notify(&app);
    Ok(())
}
#[tauri::command]
pub async fn import_sound(app: AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .set_title("导入铃声")
            .add_filter("音频", &["mp3", "wav", "ogg", "flac", "m4a", "aac"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let source = file.into_path().map_err(error)?;
        if fs::metadata(&source).map_err(error)?.len() > 50 * 1024 * 1024 {
            return Err("音频不能超过 50 MB".into());
        }
        rodio::Decoder::try_from(fs::File::open(&source).map_err(error)?)
            .map_err(|e| format!("音频无法解码：{e}"))?;
        let state = app.state::<Engine>();
        let guard = root(&state)?;
        let extension = source.extension().and_then(|s| s.to_str()).unwrap_or("wav");
        let name = format!("{}.{}", uuid::Uuid::new_v4(), extension);
        fs::copy(
            source,
            guard.as_ref().unwrap().join("resources/sounds").join(&name),
        )
        .map_err(error)?;
        Ok(Some(name))
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub fn preview_sound(
    state: tauri::State<Engine>,
    sound: Option<String>,
    volume: f32,
    stop: bool,
) -> Result<(), String> {
    if stop {
        state.audio.stop();
        return Ok(());
    }
    if !(0.0..=1.0).contains(&volume) {
        return Err("音量无效".into());
    }
    let guard = root(&state)?;
    let path = sound
        .map(|s| storage::safe_sound(guard.as_ref().unwrap(), &s))
        .transpose()?;
    state.audio.play(Play {
        key: format!("preview-{}", uuid::Uuid::new_v4()),
        path,
        volume,
        gradual: false,
    });
    Ok(())
}

fn apply_clock_change_database(
    path: &std::path::Path,
    change: crate::clock::ClockChange,
    now: i64,
) -> Result<bool, String> {
    if change.shift_seconds == 0 && !change.timezone_changed {
        return Ok(false);
    }
    let snap = storage::snapshot(path)?;
    let mut conn = storage::open(path)?;
    let tx = conn.transaction().map_err(error)?;
    let mut changed = false;
    for mut alarm in snap.alarms.iter().filter(|a| a.enabled).cloned() {
        if alarm.kind == "timer" && change.shift_seconds != 0 {
            if let Some(deadline) = alarm.deadline {
                alarm.deadline = Some(
                    deadline
                        .checked_add(change.shift_seconds)
                        .ok_or("计时截止超出范围")?,
                );
                let next = snap
                    .next_runs
                    .get(&alarm.id)
                    .map(|value| {
                        value
                            .checked_add(change.shift_seconds)
                            .ok_or("计时截止超出范围")
                    })
                    .transpose()?;
                tx.execute(
                    "UPDATE alarms SET json=?2,next_at=?3 WHERE id=?1",
                    params![
                        alarm.id,
                        serde_json::to_string(&alarm).map_err(error)?,
                        next
                    ],
                )
                .map_err(error)?;
                changed = true;
            }
        } else if alarm.kind == "clock"
            && change.timezone_changed
            && snap.next_runs.get(&alarm.id).is_some_and(|due| *due > now)
        {
            tx.execute(
                "UPDATE alarms SET next_at=?2 WHERE id=?1",
                params![
                    alarm.id,
                    alarm.next_after(now, calendar_for(&snap.calendars))
                ],
            )
            .map_err(error)?;
            changed = true;
        }
    }
    if change.shift_seconds != 0 {
        for mut occurrence in snap.occurrences {
            if !["ringing", "snoozed"].contains(&occurrence.status.as_str()) {
                continue;
            }
            occurrence.started_at = occurrence
                .started_at
                .map(|value| {
                    value
                        .checked_add(change.shift_seconds)
                        .ok_or("提醒计时超出范围")
                })
                .transpose()?;
            occurrence.snooze_until = occurrence
                .snooze_until
                .map(|value| {
                    value
                        .checked_add(change.shift_seconds)
                        .ok_or("贪睡截止超出范围")
                })
                .transpose()?;
            persist_occurrence(&tx, &occurrence)?;
            changed = true;
        }
    }
    tx.commit().map_err(error)?;
    Ok(changed)
}
pub fn apply_clock_change(
    app: &AppHandle,
    state: &Engine,
    change: crate::clock::ClockChange,
    now: i64,
) -> Result<(), String> {
    if change.shift_seconds == 0 && !change.timezone_changed {
        return Ok(());
    }
    let guard = state.root.lock().map_err(error)?;
    let Some(path) = guard.as_ref() else {
        return Ok(());
    };
    let changed = apply_clock_change_database(path, change, now)?;
    drop(guard);
    if changed {
        notify(app);
    }
    Ok(())
}
fn reconcile_database(path: &std::path::Path, now: i64) -> Result<(bool, Vec<Occurrence>), String> {
    let snap = storage::snapshot(path)?;
    let mut conn = storage::open(path)?;
    let tx = conn.transaction().map_err(error)?;
    let mut changed = false;
    let mut announce_list = vec![];
    for a in &snap.alarms {
        if !a.enabled {
            continue;
        }
        let next: Option<i64> = tx
            .query_row("SELECT next_at FROM alarms WHERE id=?1", [&a.id], |r| {
                r.get(0)
            })
            .map_err(error)?;
        if let Some(due) = next.filter(|d| *d <= now) {
            // Keep one historical summary and enumerate only the recent catch-up window.
            let mut times = vec![due];
            if due < now - 1800 && a.kind == "clock" && a.repeat != "once" {
                let mut cursor = now - 1801;
                for _ in 0..5 {
                    let Some(t) = a.next_after(cursor, calendar_for(&snap.calendars)) else {
                        break;
                    };
                    if t > now {
                        break;
                    }
                    times.push(t);
                    cursor = t;
                }
            }
            for due in times {
                let id = format!("{}:{}:{}", a.id, a.revision, due);
                let exists: i64 = tx
                    .query_row("SELECT count(*) FROM occurrences WHERE id=?1", [&id], |r| {
                        r.get(0)
                    })
                    .map_err(error)?;
                if exists != 0 {
                    continue;
                }
                let valid = eligible_for_catchup(due, now);
                let o = Occurrence {
                    id,
                    alarm_id: a.id.clone(),
                    title: a.title.clone(),
                    scheduled_at: due,
                    status: if valid { "ringing" } else { "skipped_expired" }.into(),
                    snooze_count: 0,
                    snooze_until: None,
                    started_at: valid.then_some(now),
                    reason: if now - due > 3 { "补响" } else { "准时" }.into(),
                };
                persist_occurrence(&tx, &o)?;
                if valid {
                    announce_list.push(o);
                }
            }
            tx.execute(
                "UPDATE alarms SET next_at=?2 WHERE id=?1",
                params![a.id, a.next_after(now, calendar_for(&snap.calendars))],
            )
            .map_err(error)?;
            changed = true;
        }
    }
    for mut o in snap.occurrences {
        let Some(a) = snap.alarms.iter().find(|a| a.id == o.alarm_id) else {
            continue;
        };
        if o.status == "snoozed" && o.snooze_until.is_some_and(|t| t <= now) {
            let valid = eligible_for_catchup(o.snooze_until.unwrap(), now);
            o.status = if valid { "ringing" } else { "skipped_expired" }.into();
            o.started_at = valid.then_some(now);
            o.reason = "贪睡".into();
            persist_occurrence(&tx, &o)?;
            if valid {
                announce_list.push(o.clone());
            }
            changed = true;
        }
        if o.status == "ringing" && now - o.started_at.unwrap_or(now) >= i64::from(a.ring_seconds) {
            o.status = "auto_stopped".into();
            persist_occurrence(&tx, &o)?;
            changed = true;
        }
    }
    tx.execute("DELETE FROM occurrences WHERE scheduled_at<?1 AND json NOT LIKE '%ringing%' AND json NOT LIKE '%snoozed%'",[now-90*86400]).map_err(error)?;
    for a in &snap.alarms {
        if !a.enabled || !(a.kind == "timer" || a.repeat == "once") {
            continue;
        }
        let next: Option<i64> = tx
            .query_row("SELECT next_at FROM alarms WHERE id=?1", [&a.id], |r| {
                r.get(0)
            })
            .map_err(error)?;
        let active: i64 = tx.query_row("SELECT count(*) FROM occurrences WHERE alarm_id=?1 AND json_extract(json, '$.status') IN ('ringing','snoozed')", [&a.id], |r| r.get(0)).map_err(error)?;
        if next.is_none() && active == 0 {
            let mut completed = a.clone();
            completed.enabled = false;
            completed.revision += 1;
            tx.execute(
                "UPDATE alarms SET json=?2 WHERE id=?1",
                params![
                    completed.id,
                    serde_json::to_string(&completed).map_err(error)?
                ],
            )
            .map_err(error)?;
            changed = true;
        }
    }
    tx.commit().map_err(error)?;
    Ok((changed, announce_list))
}

pub fn reconcile(app: &AppHandle, state: &Engine, now: i64) -> Result<(), String> {
    let guard = state.root.lock().map_err(error)?;
    let Some(path) = guard.as_ref() else {
        return Ok(());
    };
    let (changed, announce_list) = reconcile_database(path, now)?;
    let updated = storage::snapshot(path)?;
    if let Some(o) = updated.occurrences.iter().find(|o| o.status == "ringing") {
        if app
            .get_webview_window("alert")
            .is_none_or(|w| !w.is_visible().unwrap_or(false))
        {
            show_alert(app);
        }
        if let Some(a) = updated.alarms.iter().find(|a| a.id == o.alarm_id) {
            let sound = a
                .sound
                .as_ref()
                .and_then(|s| storage::safe_sound(path, s).ok());
            state.audio.play(Play {
                key: o.id.clone(),
                path: sound,
                volume: a.volume,
                gradual: a.gradual,
            });
        }
    } else {
        state.audio.stop_alert();
        if let Some(w) = app.get_webview_window("alert") {
            let _ = w.hide();
        }
    }
    drop(guard);
    for o in announce_list {
        announce(app, &o);
    }
    if changed {
        notify(app);
    }
    Ok(())
}

#[cfg(test)]
mod scheduler_tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(due: i64) -> Self {
            let path =
                std::env::temp_dir().join(format!("ontime-scheduler-{}", uuid::Uuid::new_v4()));
            storage::initialize(&path).unwrap();
            let a: Alarm = serde_json::from_value(serde_json::json!({"id":"timer","title":"计时","enabled":true,"kind":"timer","time":"09:00","date":null,"repeat":"once","weekdays":[],"skipHolidays":false,"durationSeconds":60,"deadline":due,"sound":null,"volume":0.5,"gradual":true,"ringSeconds":60,"snoozeLimit":3,"tags":[],"revision":1,"createdAt":1})).unwrap();
            storage::save_alarm(&path, &a, Some(due)).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn catchup_boundary_and_no_duplicate_after_restart() {
        let f = Fixture::new(10000);
        assert_eq!(reconcile_database(&f.0, 11800).unwrap().1.len(), 1);
        assert!(storage::snapshot(&f.0).unwrap().alarms[0].enabled);
        assert!(reconcile_database(&f.0, 11800).unwrap().1.is_empty());
        assert_eq!(storage::snapshot(&f.0).unwrap().occurrences.len(), 1);
        reconcile_database(&f.0, 11860).unwrap();
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.occurrences[0].status, "auto_stopped");
        assert!(!snap.alarms[0].enabled);
    }
    #[test]
    fn expired_one_shot_is_recorded_and_disabled() {
        let f = Fixture::new(10000);
        assert!(reconcile_database(&f.0, 11801).unwrap().1.is_empty());
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.occurrences[0].status, "skipped_expired");
        assert!(!snap.alarms[0].enabled);
    }
    #[test]
    fn snoozed_one_shot_stays_enabled_until_finished() {
        let f = Fixture::new(10000);
        reconcile_database(&f.0, 10000).unwrap();
        let mut o = storage::snapshot(&f.0).unwrap().occurrences.remove(0);
        o.status = "snoozed".into();
        o.snooze_until = Some(10300);
        o.snooze_count = 1;
        persist_occurrence(&storage::open(&f.0).unwrap(), &o).unwrap();
        reconcile_database(&f.0, 10299).unwrap();
        assert!(storage::snapshot(&f.0).unwrap().alarms[0].enabled);
        assert_eq!(reconcile_database(&f.0, 10300).unwrap().1.len(), 1);
        assert_eq!(
            storage::snapshot(&f.0).unwrap().occurrences[0].status,
            "ringing"
        );
    }
    #[test]
    fn batch_enable_does_not_restart_enabled_timer() {
        let f = Fixture::new(10060);
        let ids = vec!["timer".into()];
        set_enabled_database(&f.0, &ids, true, 10010).unwrap();
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.next_runs.get("timer"), Some(&10060));
        assert_eq!(snap.alarms[0].revision, 1);
        set_enabled_database(&f.0, &ids, false, 10020).unwrap();
        assert!(!storage::snapshot(&f.0).unwrap().alarms[0].enabled);
        set_enabled_database(&f.0, &ids, true, 10030).unwrap();
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.next_runs.get("timer"), Some(&10090));
        assert_eq!(snap.alarms[0].revision, 3);
    }
    #[test]
    fn batch_enable_does_not_cancel_current_ring() {
        let f = Fixture::new(10000);
        reconcile_database(&f.0, 10000).unwrap();
        set_enabled_database(&f.0, &["timer".into()], true, 10001).unwrap();
        assert_eq!(
            storage::snapshot(&f.0).unwrap().occurrences[0].status,
            "ringing"
        );
    }
    #[test]
    fn clock_adjustment_preserves_timer_remaining_time() {
        let f = Fixture::new(10060);
        apply_clock_change_database(
            &f.0,
            crate::clock::ClockChange {
                shift_seconds: 300,
                timezone_changed: false,
            },
            10300,
        )
        .unwrap();
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.alarms[0].deadline, Some(10360));
        assert_eq!(snap.next_runs.get("timer"), Some(&10360));
        assert!(reconcile_database(&f.0, 10359).unwrap().1.is_empty());
        assert_eq!(reconcile_database(&f.0, 10360).unwrap().1.len(), 1);
    }
    #[test]
    fn clock_adjustment_preserves_ring_duration_and_snooze() {
        let f = Fixture::new(10000);
        reconcile_database(&f.0, 10000).unwrap();
        apply_clock_change_database(
            &f.0,
            crate::clock::ClockChange {
                shift_seconds: 300,
                timezone_changed: false,
            },
            10300,
        )
        .unwrap();
        reconcile_database(&f.0, 10330).unwrap();
        let mut o = storage::snapshot(&f.0).unwrap().occurrences.remove(0);
        assert_eq!(o.status, "ringing");
        o.status = "snoozed".into();
        o.snooze_until = Some(10630);
        persist_occurrence(&storage::open(&f.0).unwrap(), &o).unwrap();
        apply_clock_change_database(
            &f.0,
            crate::clock::ClockChange {
                shift_seconds: -200,
                timezone_changed: false,
            },
            10130,
        )
        .unwrap();
        assert_eq!(
            storage::snapshot(&f.0).unwrap().occurrences[0].snooze_until,
            Some(10430)
        );
        assert!(reconcile_database(&f.0, 10429).unwrap().1.is_empty());
        assert_eq!(reconcile_database(&f.0, 10430).unwrap().1.len(), 1);
    }
}
