use crate::{
    audio::{AudioHandle, Play},
    domain::{Alarm, Group, Occurrence, Tag, eligible_for_catchup},
    storage::{self, Snapshot},
};
use chrono::Utc;
use rusqlite::{Connection, params};
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;

pub struct Engine {
    pub hidden_alerts: Mutex<std::collections::BTreeSet<String>>,
    pub inspecting_snoozed: std::sync::atomic::AtomicBool,
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
    .inner_size(540.0, 520.0)
    .resizable(false)
    .maximizable(false)
    .always_on_top(true)
    .build();
}
fn show_deferred_summary(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("deferred-summary") {
        if !window.is_visible().unwrap_or(false) {
            let _ = window.show();
            let _ = window.emit("deferred-summary-refresh", ());
        }
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(app, "deferred-summary", tauri::WebviewUrl::App("index.html#/deferred-summary".into()))
        .title("onTime · 错过的提醒")
        .inner_size(540.0, 440.0)
        .always_on_top(true)
        .build();
}

#[tauri::command]
pub fn get_deferred_reminders(state: tauri::State<Engine>) -> Result<Vec<Occurrence>, String> {
    let guard = root(&state)?;
    let mut items: Vec<_> = storage::snapshot(guard.as_ref().unwrap())?.occurrences
        .into_iter().filter(|o| o.status == "deferred").collect();
    items.sort_by_key(|o| (o.scheduled_at, o.title.clone()));
    Ok(items)
}

#[tauri::command]
pub fn acknowledge_deferred_reminders(app: AppHandle, state: tauri::State<Engine>) -> Result<(), String> {
    let guard = root(&state)?;
    let mut conn = storage::open(guard.as_ref().unwrap())?;
    let tx = conn.transaction().map_err(error)?;
    let mut stmt = tx.prepare("SELECT json FROM occurrences WHERE status='deferred'").map_err(error)?;
    let items: Vec<String> = stmt.query_map([], |row| row.get(0)).map_err(error)?
        .collect::<Result<_, _>>().map_err(error)?;
    drop(stmt);
    for json in items {
        let mut occurrence: Occurrence = serde_json::from_str(&json).map_err(error)?;
        occurrence.status = "skipped_expired".into();
        occurrence.reason = "息屏/锁屏期间延迟提醒，已汇总告知".into();
        persist_occurrence(&tx, &occurrence)?;
    }
    tx.commit().map_err(error)?;
    drop(guard);
    if let Some(window) = app.get_webview_window("deferred-summary") { let _ = window.hide(); }
    notify(&app);
    Ok(())
}
fn announce(app: &AppHandle, occurrences: &[Occurrence]) {
    if occurrences.is_empty() {
        return;
    }
    let title = if occurrences.len() == 1 {
        "onTime · 时间到了".to_string()
    } else {
        format!("onTime · {} 个提醒同时触发", occurrences.len())
    };
    let body = if occurrences.len() == 1 {
        occurrences[0].title.clone()
    } else {
        let names = occurrences
            .iter()
            .take(3)
            .map(|o| o.title.as_str())
            .collect::<Vec<_>>()
            .join("、");
        if occurrences.len() > 3 {
            format!(
                "{names} 等 {} 个提醒，请打开 onTime 逐项处理。",
                occurrences.len()
            )
        } else {
            format!("{names}，请打开 onTime 逐项处理。")
        }
    };
    let mode = crate::preferences::notification_mode(app);
    if mode != "internal" && let Err(e) = app.notification().builder().title(title).body(body).show() {
        let message = format!("系统通知发送失败：{e}");
        log::warn!("{message}");
        let _ = app.emit("notification-error", message);
    }
    if mode != "system" { show_alert(app); }
}

#[tauri::command]
pub fn hide_active_alert(app: AppHandle, state: tauri::State<Engine>) -> Result<(), String> {
    let guard = root(&state)?;
    let snap = storage::snapshot(guard.as_ref().unwrap())?;
    state
        .inspecting_snoozed
        .store(false, std::sync::atomic::Ordering::Relaxed);
    state.hidden_alerts.lock().map_err(error)?.extend(
        snap.occurrences
            .iter()
            .filter(|o| o.status == "ringing")
            .map(|o| o.id.clone()),
    );
    if let Some(w) = app.get_webview_window("alert") {
        w.hide().map_err(error)?;
    }
    Ok(())
}
#[tauri::command]
pub fn show_active_alert(app: AppHandle, state: tauri::State<Engine>) -> Result<(), String> {
    if crate::preferences::delay_when_inactive(&app) && crate::screen_state::inactive() {
        return Err("息屏或锁屏期间已延迟提醒，解锁后查看汇总".into());
    }
    let guard = root(&state)?;
    let snap = storage::snapshot(guard.as_ref().unwrap())?;
    if !snap
        .occurrences
        .iter()
        .any(|o| ["ringing", "snoozed"].contains(&o.status.as_str()))
    {
        return Err("当前没有活动提醒".into());
    }
    state
        .inspecting_snoozed
        .store(true, std::sync::atomic::Ordering::Relaxed);
    state.hidden_alerts.lock().map_err(error)?.clear();
    show_alert(&app);
    Ok(())
}
#[tauri::command]
pub fn delete_alarms(
    app: AppHandle,
    state: tauri::State<Engine>,
    ids: Vec<String>,
) -> Result<(), String> {
    let guard = root(&state)?;
    delete_alarms_database(guard.as_ref().unwrap(), &ids)?;
    drop(guard);
    notify(&app);
    Ok(())
}
fn delete_alarms_database(path: &std::path::Path, ids: &[String]) -> Result<(), String> {
    let mut conn = storage::open(path)?;
    let tx = conn.transaction().map_err(error)?;
    for id in ids {
        cancel_instances(&tx, id)?;
        tx.execute("DELETE FROM alarms WHERE id=?1", [id])
            .map_err(error)?;
    }
    tx.commit().map_err(error)
}
#[tauri::command]
pub fn matching_alarm_dates(
    state: tauri::State<Engine>,
    from: i64,
    to: i64,
) -> Result<Vec<String>, String> {
    if from >= to {
        return Err("日期范围无效".into());
    }
    let guard = root(&state)?;
    Ok(storage::snapshot(guard.as_ref().unwrap())?
        .alarms
        .into_iter()
        .filter(|a| {
            let mut candidate = a.clone();
            candidate.enabled = true;
            candidate
                .next_after(from.saturating_sub(1))
                .is_some_and(|next| next < to)
        })
        .map(|a| a.id)
        .collect())
}

#[tauri::command]
pub fn test_notification(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("onTime 通知测试")
        .body("系统通知已连接，可以正常接收提醒。")
        .show()
        .map_err(|e| format!("系统通知发送失败：{e}"))
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
pub fn get_home_summary(
    state: tauri::State<Engine>,
    now: i64,
    today_start: i64,
    today_end: i64,
    week_end: i64,
) -> Result<serde_json::Value, String> {
    let guard = root(&state)?;
    let path = guard.as_ref().unwrap();
    let snap = storage::snapshot(path)?;
    let mut upcoming = 0_u64;
    let mut reminder_ids = std::collections::BTreeSet::new();
    for alarm in snap.alarms.iter().filter(|a| a.enabled) {
        let mut cursor = now;
        let mut count = 0;
        // At most 48 slots per local day, including the boundary days of the weekly window.
        for _ in 0..(48 * 9) {
            let Some(next) = alarm.next_after(cursor) else {
                break;
            };
            if next >= week_end {
                break;
            }
            count += 1;
            cursor = next;
        }
        upcoming += count;
        if count > 0 {
            reminder_ids.insert(alarm.id.clone());
        }
    }
    for o in snap.occurrences.iter().filter(|o| {
        o.status == "snoozed" && o.snooze_until.is_some_and(|t| t >= now && t < week_end)
    }) {
        upcoming += 1;
        reminder_ids.insert(o.alarm_id.clone());
    }
    let reminders = reminder_ids.len();
    let conn = storage::open(path)?;
    let processed: i64 = conn.query_row("SELECT count(*) FROM occurrences WHERE scheduled_at>=?1 AND scheduled_at<?2 AND json_extract(json, '$.status') IN ('dismissed','auto_stopped')", params![today_start, today_end], |row| row.get(0)).map_err(error)?;
    Ok(serde_json::json!({"upcoming": upcoming, "reminders": reminders, "processed": processed}))
}

#[tauri::command]
pub fn get_history_facets(state: tauri::State<Engine>) -> Result<serde_json::Value, String> {
    let guard = root(&state)?;
    let conn = storage::open(guard.as_ref().unwrap())?;
    let snap = storage::snapshot(guard.as_ref().unwrap())?;
    let mut groups = snap
        .groups
        .into_iter()
        .map(|g| (g.id.clone(), g))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut tags = snap
        .tags
        .into_iter()
        .map(|g| (g.id.clone(), g))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut stmt = conn
        .prepare("SELECT DISTINCT json_extract(json,'$.context') FROM occurrences WHERE json_type(json,'$.context')='object'")
        .map_err(error)?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(error)?;
    for row in rows {
        let c: crate::domain::HistoryContext =
            serde_json::from_str(&row.map_err(error)?).map_err(error)?;
        {
            if let Some(g) = c.group {
                groups.entry(g.id.clone()).or_insert(g);
            }
            for t in c.tags {
                tags.entry(t.id.clone()).or_insert(t);
            }
        }
    }
    Ok(
        serde_json::json!({"groups": groups.into_values().collect::<Vec<_>>(), "tags": tags.into_values().collect::<Vec<_>>()}),
    )
}

#[tauri::command]
pub fn get_history(
    state: tauri::State<Engine>,
    filter: storage::HistoryFilter,
) -> Result<storage::HistoryPage, String> {
    let guard = root(&state)?;
    storage::history_page(guard.as_ref().unwrap(), filter)
}
#[tauri::command]
pub fn delete_history(app: AppHandle, state: tauri::State<Engine>, ids: Vec<String>) -> Result<usize, String> {
    if ids.is_empty() { return Ok(0); }
    if ids.len() > 500 { return Err("一次最多删除 500 条历史记录".into()); }
    let guard = root(&state)?;
    let mut conn = storage::open(guard.as_ref().unwrap())?;
    let tx = conn.transaction().map_err(error)?;
    let mut deleted = 0;
    for id in &ids {
        deleted += tx.execute("DELETE FROM occurrences WHERE id=?1 AND status NOT IN ('ringing','snoozed')", [id]).map_err(error)?;
    }
    tx.commit().map_err(error)?;
    drop(guard);
    notify(&app);
    Ok(deleted)
}

#[tauri::command]
pub fn check_alarm_conflicts(
    state: tauri::State<Engine>,
    mut alarm: Alarm,
) -> Result<Vec<String>, String> {
    let guard = root(&state)?;
    let now = Utc::now().timestamp();
    alarm.enabled = true;
    if alarm.kind == "timer" && alarm.deadline.is_none() {
        alarm.deadline = now.checked_add(alarm.duration_seconds);
    }
    let mut candidate = Vec::new();
    let mut cursor = now;
    for _ in 0..3 {
        let Some(next) = alarm.next_after(cursor) else {
            break;
        };
        candidate.push(next);
        cursor = next;
    }
    let snapshot = storage::snapshot(guard.as_ref().unwrap())?;
    let mut conflicts = Vec::new();
    for existing in snapshot
        .alarms
        .iter()
        .filter(|other| other.id != alarm.id && other.enabled)
    {
        let mut existing_cursor = now;
        let mut times = Vec::new();
        for _ in 0..3 {
            let Some(next) = existing.next_after(existing_cursor) else {
                break;
            };
            times.push(next);
            existing_cursor = next;
        }
        if candidate
            .iter()
            .any(|a| times.iter().any(|b| a.abs_diff(*b) <= 60))
        {
            conflicts.push(existing.title.clone());
        }
    }
    conflicts.sort();
    conflicts.dedup();
    Ok(conflicts)
}

pub fn create_quick_timer(app: &AppHandle, minutes: u32) -> Result<(), String> {
    if ![10, 30].contains(&minutes) {
        return Err("快速计时只支持 10 或 30 分钟".into());
    }
    let state = app.state::<Engine>();
    let guard = root(&state)?;
    let now = Utc::now().timestamp();
    let mut alarm: Alarm = serde_json::from_value(serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(), "title": format!("快速计时 · {minutes} 分钟"),
        "content":"从系统托盘快速创建", "enabled":true, "kind":"timer", "time":"09:00", "date":null,
        "repeat":"once", "weekdays":[], "durationSeconds":minutes*60, "deadline":now+i64::from(minutes)*60,
        "sound":null, "volume":0.7, "gradual":true, "ringSeconds":60, "snoozeLimit":3, "tags":[],
        "revision":1, "createdAt":now
    })).map_err(error)?;
    alarm.deadline = Some(now + i64::from(minutes) * 60);
    storage::save_alarm(guard.as_ref().unwrap(), &alarm, alarm.next_after(now))?;
    drop(guard);
    if let Err(error) = app
        .notification()
        .builder()
        .title("快速计时已创建")
        .body(format!("{} 分钟后提醒你。", minutes))
        .show()
    {
        log::warn!("快速计时通知发送失败：{error}");
    }
    notify(app);
    Ok(())
}
#[tauri::command]
pub fn preview_next_runs(mut alarm: Alarm, count: u8) -> Vec<i64> {
    alarm.enabled = true;
    let now = Utc::now().timestamp();
    if alarm.kind == "timer" && alarm.deadline.is_none() {
        let Some(deadline) = now.checked_add(alarm.duration_seconds) else {
            return Vec::new();
        };
        alarm.deadline = Some(deadline);
    }
    let mut cursor = now;
    let mut runs = Vec::new();
    for _ in 0..count.clamp(1, 5) {
        let Some(next) = alarm.next_after(cursor) else {
            break;
        };
        runs.push(next);
        cursor = next;
    }
    runs
}
fn migrate_database(source: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    if storage::snapshot(source)?
        .occurrences
        .iter()
        .any(|o| ["ringing", "snoozed"].contains(&o.status.as_str()))
    {
        return Err("请先处理响铃和贪睡中的提醒，再迁移数据".into());
    }
    let canonical_source = fs::canonicalize(source).map_err(error)?;
    let canonical_target = fs::canonicalize(target).map_err(error)?;
    if canonical_target.starts_with(&canonical_source)
        || canonical_source.starts_with(&canonical_target)
    {
        return Err("迁移目标不能是原目录及其父子目录".into());
    }
    if fs::read_dir(target).map_err(error)?.next().is_some() {
        return Err("请选择空目录用于迁移，避免覆盖现有文件".into());
    }
    storage::initialize(target)?;
    let archive = target.join("migration.ontime.zip");
    crate::backup::export_to(source, &archive)?;
    crate::backup::import_from(target, &archive)?;
    if let Ok(entries) = fs::read_dir(source.join("setting")) {
        fs::create_dir_all(target.join("setting")).map_err(error)?;
        for entry in entries {
            let entry = entry.map_err(error)?;
            if entry.file_type().map_err(error)?.is_file() {
                fs::copy(entry.path(), target.join("setting").join(entry.file_name()))
                    .map_err(error)?;
            }
        }
    }
    fs::remove_file(archive).map_err(error)?;
    Ok(())
}

#[tauri::command]
pub async fn choose_data_root(
    app: AppHandle,
    migrate: Option<bool>,
) -> Result<Option<Snapshot>, String> {
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
        if migrate.unwrap_or(false) {
            let source = guard.as_ref().ok_or("请先连接原数据目录")?;
            migrate_database(source, &path)?;
        }
        storage::initialize(&path)?;
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
        state.hidden_alerts.lock().map_err(error)?.clear();
        state
            .inspecting_snoozed
            .store(false, std::sync::atomic::Ordering::Relaxed);
        state.audio.stop();
        drop(guard);
        notify(&app);
        Ok(Some(snap))
    })
    .await
    .map_err(error)?
}
fn update_timer_deadline(alarm: &mut Alarm, old: Option<&Alarm>, now: i64, restart: bool) {
    if alarm.kind == "timer" && alarm.enabled {
        if restart
            || old.is_none_or(|a| {
                !a.enabled || a.kind != "timer" || a.duration_seconds != alarm.duration_seconds
            })
        {
            alarm.deadline = Some(now + alarm.duration_seconds);
        } else if let Some(old) = old {
            alarm.deadline = old.deadline;
        }
    }
}

#[tauri::command]
pub fn save_alarm(
    app: AppHandle,
    state: tauri::State<Engine>,
    mut alarm: Alarm,
    restart_timer: Option<bool>,
) -> Result<(), String> {
    alarm.title = alarm.title.trim().into();
    alarm.validate()?;
    let guard = root(&state)?;
    let path = guard.as_ref().unwrap();
    let snap = storage::snapshot(path)?;
    if alarm
        .group_id
        .as_ref()
        .is_some_and(|id| !snap.groups.iter().any(|g| &g.id == id))
    {
        return Err("分组已不存在，请重新选择".into());
    }
    if alarm
        .tags
        .iter()
        .any(|id| !snap.tags.iter().any(|t| &t.id == id))
    {
        return Err("标签已不存在，请重新选择".into());
    }
    let old = snap.alarms.iter().find(|a| a.id == alarm.id);
    if let Some(old) = old
        && alarm.revision != old.revision
    {
        return Err("提醒已变化，请关闭编辑器后重新打开".into());
    }
    alarm.revision = old.map_or(1, |a| a.revision + 1);
    update_timer_deadline(
        &mut alarm,
        old,
        Utc::now().timestamp(),
        restart_timer.unwrap_or(false),
    );
    if old.is_none() {
        alarm.id = uuid::Uuid::new_v4().to_string();
        alarm.created_at = Utc::now().timestamp();
    }
    let next = alarm.next_after(Utc::now().timestamp());
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
        let next = alarm.next_after(now);
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
    let guard = root(&state)?;
    let snap = storage::snapshot(guard.as_ref().unwrap())?;
    if snap
        .tags
        .iter()
        .any(|item| item.id != tag.id && item.name.trim().to_lowercase() == tag.name.to_lowercase())
    {
        return Err("已存在同名标签，请换一个名称".into());
    }
    tag.created_at = if tag.id.is_empty() {
        Some(Utc::now().timestamp())
    } else {
        snap.tags
            .iter()
            .find(|item| item.id == tag.id)
            .ok_or("记录已不存在，请刷新后重试")?
            .created_at
    };
    if tag.id.is_empty() {
        tag.id = uuid::Uuid::new_v4().to_string();
    }
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
pub fn save_group(
    app: AppHandle,
    state: tauri::State<Engine>,
    mut group: Group,
) -> Result<(), String> {
    group.name = group.name.trim().into();
    if group.name.is_empty()
        || group.name.chars().count() > 24
        || group.color.len() != 7
        || !group.color.starts_with('#')
        || !group.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("分组名称或颜色无效".into());
    }
    let guard = root(&state)?;
    let snap = storage::snapshot(guard.as_ref().unwrap())?;
    if snap.groups.iter().any(|item| {
        item.id != group.id && item.name.trim().to_lowercase() == group.name.to_lowercase()
    }) {
        return Err("已存在同名分组，请换一个名称".into());
    }
    group.created_at = if group.id.is_empty() {
        Some(Utc::now().timestamp())
    } else {
        snap.groups
            .iter()
            .find(|item| item.id == group.id)
            .ok_or("记录已不存在，请刷新后重试")?
            .created_at
    };
    if group.id.is_empty() {
        group.id = uuid::Uuid::new_v4().to_string();
    }
    let conn = storage::open(guard.as_ref().unwrap())?;
    conn.execute(
        "INSERT INTO groups(id,json) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        params![group.id, serde_json::to_string(&group).map_err(error)?],
    )
    .map_err(error)?;
    drop(guard);
    notify(&app);
    Ok(())
}
#[tauri::command]
pub fn delete_group(app: AppHandle, state: tauri::State<Engine>, id: String) -> Result<(), String> {
    let guard = root(&state)?;
    let path = guard.as_ref().unwrap();
    let snap = storage::snapshot(path)?;
    let mut conn = storage::open(path)?;
    let tx = conn.transaction().map_err(error)?;
    tx.execute("DELETE FROM groups WHERE id=?1", [&id])
        .map_err(error)?;
    for mut a in snap.alarms {
        if a.group_id.as_ref() == Some(&id) {
            a.group_id = None;
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
    if o.status != "ringing" && !(o.status == "snoozed" && snooze_minutes.is_none()) {
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
    let remaining_ringing: i64 = conn.query_row(
        "SELECT count(*) FROM occurrences WHERE status='ringing'",
        [],
        |row| row.get(0),
    ).map_err(error)?;
    drop(guard);
    state.audio.stop_alert();
    if remaining_ringing == 0 {
        state.inspecting_snoozed.store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(window) = app.get_webview_window("alert") {
            window.hide().map_err(error)?;
        }
    } else {
        // Keep the shared window open and start the next simultaneous reminder.
        reconcile(&app, &state, Utc::now().timestamp(), false)?;
    }
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
        state.audio.stop_preview();
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
        tones: None,
    });
    Ok(())
}

#[tauri::command]
pub fn preview_builtin_sound(state: tauri::State<Engine>, frequencies: Vec<f32>, volume: f32) -> Result<(), String> {
    if !(0.0..=1.0).contains(&volume) || frequencies.is_empty() || frequencies.len() > 8 || frequencies.iter().any(|f| !(200.0..=2000.0).contains(f)) { return Err("内置铃声参数无效".into()); }
    state.audio.preview_tones(frequencies, volume);
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
                params![alarm.id, alarm.next_after(now)],
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
fn reconcile_database_with_policy(
    path: &std::path::Path,
    now: i64,
    defer_due: bool,
    catchup: bool,
) -> Result<(bool, Vec<Occurrence>, Vec<Occurrence>), String> {
    let snap = storage::snapshot(path)?;
    let mut conn = storage::open(path)?;
    let tx = conn.transaction().map_err(error)?;
    let mut changed = false;
    let mut announce_list = vec![];
    let mut missed_list = vec![];
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
            if a.kind == "clock" && a.repeat != "once" {
                let mut cursor = due.max(now - 1801);
                for _ in 0..48 {
                    let Some(t) = a.next_after(cursor) else {
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
                let valid = now - due <= 3 || (catchup && eligible_for_catchup(due, now));
                let o = Occurrence {
                    context: Some(crate::domain::HistoryContext {
                        inferred: false,
                        kind: if a.kind == "timer" {
                            "timer"
                        } else if a.repeat == "once" {
                            "once"
                        } else {
                            "repeat"
                        }
                        .into(),
                        group: snap
                            .groups
                            .iter()
                            .find(|g| Some(&g.id) == a.group_id.as_ref())
                            .cloned(),
                        tags: snap
                            .tags
                            .iter()
                            .filter(|t| a.tags.contains(&t.id))
                            .cloned()
                            .collect(),
                    }),
                    id,
                    alarm_id: a.id.clone(),
                    title: a.title.clone(),
                    scheduled_at: due,
                    status: if defer_due { "deferred" } else if valid { "ringing" } else { "skipped_expired" }.into(),
                    snooze_count: 0,
                    snooze_until: None,
                    started_at: valid.then_some(now),
                    reason: if defer_due {
                        "息屏/锁屏期间延迟提醒"
                    } else if valid && now - due > 3 {
                        "补响"
                    } else if valid {
                        "准时"
                    } else {
                        "未补响"
                    }
                    .into(),
                };
                persist_occurrence(&tx, &o)?;
                if defer_due {
                    // The summary reads these occurrences after the session becomes usable.
                } else if valid {
                    announce_list.push(o);
                } else {
                    missed_list.push(o);
                }
            }
            tx.execute(
                "UPDATE alarms SET next_at=?2 WHERE id=?1",
                params![a.id, a.next_after(now)],
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
            let valid = now - o.snooze_until.unwrap() <= 3 || (catchup && eligible_for_catchup(o.snooze_until.unwrap(), now));
            o.status = if defer_due { "deferred" } else if valid { "ringing" } else { "skipped_expired" }.into();
            o.started_at = valid.then_some(now);
            o.reason = if defer_due {
                "息屏/锁屏期间延迟提醒"
            } else if valid {
                "贪睡"
            } else {
                "贪睡后过期，未补响"
            }
            .into();
            persist_occurrence(&tx, &o)?;
            if defer_due {
                // Included in the deferred summary.
            } else if valid {
                announce_list.push(o.clone());
            } else {
                missed_list.push(o.clone());
            }
            changed = true;
        }
        if defer_due && o.status == "ringing" {
            o.status = "deferred".into();
            o.reason = "息屏/锁屏期间延迟提醒".into();
            persist_occurrence(&tx, &o)?;
            changed = true;
        } else if o.status == "ringing" && now - o.started_at.unwrap_or(now) >= i64::from(a.ring_seconds) {
            o.status = "auto_stopped".into();
            persist_occurrence(&tx, &o)?;
            changed = true;
        }
    }
    // Retire exhausted schedules without interrupting ringing or snoozed instances.
    for a in &snap.alarms {
        if !a.enabled {
            continue;
        }
        let one_shot = a.kind == "timer" || a.repeat == "once";
        let end_passed = a.effective_end.as_ref().is_some_and(|end| {
            chrono::DateTime::from_timestamp(now, 0).is_some_and(|date| {
                end.as_str()
                    < date
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d")
                        .to_string()
                        .as_str()
            })
        });
        if !one_shot && !end_passed {
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
    Ok((changed, announce_list, missed_list))
}

#[cfg(test)]
fn reconcile_database(path: &std::path::Path, now: i64) -> Result<(bool, Vec<Occurrence>, Vec<Occurrence>), String> {
    reconcile_database_with_policy(path, now, false, true)
}

pub fn reconcile(app: &AppHandle, state: &Engine, now: i64, resumed: bool) -> Result<(), String> {
    let guard = state.root.lock().map_err(error)?;
    let Some(path) = guard.as_ref() else {
        return Ok(());
    };
    let delay_enabled = crate::preferences::delay_when_inactive(app);
    let inactive = crate::screen_state::inactive();
    let defer_due = delay_enabled && (inactive || resumed);
    let (changed, announce_list, missed_list) = reconcile_database_with_policy(path, now, defer_due, delay_enabled)?;
    let updated = storage::snapshot(path)?;
    let mut hidden = state.hidden_alerts.lock().map_err(error)?;
    hidden.retain(|id| {
        updated
            .occurrences
            .iter()
            .any(|o| &o.id == id && o.status == "ringing")
    });
    let visible_ring = updated
        .occurrences
        .iter()
        .any(|o| o.status == "ringing" && !hidden.contains(&o.id));
    if let Some(o) = updated
        .occurrences
        .iter()
        .filter(|o| o.status == "ringing")
        .min_by_key(|o| (o.scheduled_at, o.alarm_id.as_str()))
    {
        if crate::preferences::notification_mode(app) != "system" && visible_ring
            && app
                .get_webview_window("alert")
                .is_none_or(|w| !w.is_visible().unwrap_or(false))
        {
            show_alert(app);
        }
        if crate::preferences::notification_mode(app) != "system" && let Some(a) = updated.alarms.iter().find(|a| a.id == o.alarm_id) {
            let sound = a
                .sound
                .as_ref()
                .and_then(|s| storage::safe_sound(path, s).ok());
            let builtin = sound.is_none();
            state.audio.play(Play {
                key: o.id.clone(),
                path: sound,
                volume: a.volume,
                gradual: a.gradual,
                tones: if builtin { Some(crate::preferences::builtin_frequencies(&crate::preferences::builtin_sound(app))) } else { None },
            });
        }
    } else {
        state.audio.stop_alert();
        if (defer_due || !state
            .inspecting_snoozed
            .load(std::sync::atomic::Ordering::Relaxed)
            || !updated.occurrences.iter().any(|o| o.status == "snoozed"))
            && let Some(w) = app.get_webview_window("alert")
        {
            let _ = w.hide();
        }
    }
    drop(hidden);
    drop(guard);
    if !announce_list.is_empty() {
        announce(app, &announce_list);
    }
    if !delay_enabled && !missed_list.is_empty() {
        // With the option off, missed reminders remain in history only.
    } else if !missed_list.is_empty() && !inactive {
        let title = format!("onTime · {} 个提醒过期未补响", missed_list.len());
        let body = if missed_list.len() == 1 {
            format!(
                "{} 已记入提醒记录，打开 onTime 查看并调整。",
                missed_list[0].title
            )
        } else {
            "应用未运行超过补响时限，这些提醒已记入记录。打开 onTime 查看。".into()
        };
        if let Err(e) = app.notification().builder().title(title).body(body).show() {
            let message = format!("漏响摘要通知发送失败：{e}");
            log::warn!("{message}");
            let _ = app.emit("notification-error", message);
        }
    }
    if !inactive && updated.occurrences.iter().any(|o| o.status == "deferred") {
        show_deferred_summary(app);
    } else if inactive && let Some(window) = app.get_webview_window("deferred-summary") {
        let _ = window.hide();
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
    fn inactive_reminder_is_saved_for_summary_without_ringing_and_not_duplicated() {
        let f = Fixture::new(10000);
        let (_, ringing, missed) = reconcile_database_with_policy(&f.0, 10000, true, true).unwrap();
        assert!(ringing.is_empty() && missed.is_empty());
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.occurrences.len(), 1);
        assert_eq!(snap.occurrences[0].status, "deferred");
        reconcile_database_with_policy(&f.0, 10001, false, true).unwrap();
        assert_eq!(storage::snapshot(&f.0).unwrap().occurrences.len(), 1);
    }
    #[test]
    fn locking_while_alarm_rings_defers_it_and_disabled_catchup_records_only_history() {
        let f = Fixture::new(10000);
        assert_eq!(reconcile_database_with_policy(&f.0, 10000, false, false).unwrap().1.len(), 1);
        reconcile_database_with_policy(&f.0, 10001, true, true).unwrap();
        assert_eq!(storage::snapshot(&f.0).unwrap().occurrences[0].status, "deferred");
        let later = Fixture::new(10000);
        let (_, ringing, missed) = reconcile_database_with_policy(&later.0, 10010, false, false).unwrap();
        assert!(ringing.is_empty());
        assert_eq!(missed.len(), 1);
        assert_eq!(storage::snapshot(&later.0).unwrap().occurrences[0].status, "skipped_expired");
    }
    #[test]
    fn migration_copies_data_and_settings_and_preserves_source() {
        let f = Fixture::new(Utc::now().timestamp() + 600);
        let target =
            std::env::temp_dir().join(format!("ontime-migration-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(f.0.join("setting")).unwrap();
        fs::write(f.0.join("setting/appearance.json"), b"{\"fontSize\":18}").unwrap();
        migrate_database(&f.0, &target).unwrap();
        assert_eq!(storage::snapshot(&target).unwrap().alarms.len(), 1);
        assert_eq!(storage::snapshot(&f.0).unwrap().alarms.len(), 1);
        assert_eq!(
            fs::read(target.join("setting/appearance.json")).unwrap(),
            b"{\"fontSize\":18}"
        );
        assert!(migrate_database(&f.0, &target).is_err());
        assert!(migrate_database(&f.0, &f.0).is_err());
        fs::remove_dir_all(target).unwrap();
    }
    #[test]
    fn migration_refuses_active_reminders_before_writing_target() {
        let f = Fixture::new(10000);
        reconcile_database(&f.0, 10000).unwrap();
        let target =
            std::env::temp_dir().join(format!("ontime-active-migration-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&target).unwrap();
        assert!(migrate_database(&f.0, &target).is_err());
        assert!(fs::read_dir(&target).unwrap().next().is_none());
        fs::remove_dir_all(target).unwrap();
    }
    #[test]
    fn batch_delete_rolls_back_cancellation_and_deletion_on_failure() {
        let f = Fixture::new(10000);
        reconcile_database(&f.0, 10000).unwrap();
        let conn = storage::open(&f.0).unwrap();
        let mut other = storage::snapshot(&f.0).unwrap().alarms.remove(0);
        other.id = "fail".into();
        storage::save_alarm(&f.0, &other, None).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_delete BEFORE DELETE ON alarms WHEN OLD.id='fail' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert!(delete_alarms_database(&f.0, &["timer".into(), "fail".into()]).is_err());
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.alarms.len(), 2);
        assert_eq!(snap.occurrences[0].status, "ringing");
        conn.execute_batch("DROP TRIGGER fail_delete").unwrap();
        delete_alarms_database(&f.0, &["timer".into(), "fail".into()]).unwrap();
        assert!(storage::snapshot(&f.0).unwrap().alarms.is_empty());
        assert_eq!(
            storage::snapshot(&f.0).unwrap().occurrences[0].status,
            "cancelled"
        );
    }
    #[test]
    fn editing_timer_preserves_deadline_unless_restarted() {
        let f = Fixture::new(10060);
        let old = storage::snapshot(&f.0).unwrap().alarms.remove(0);
        let mut edited = old.clone();
        edited.title = "修改标题".into();
        edited.deadline = Some(1);
        update_timer_deadline(&mut edited, Some(&old), 10030, false);
        assert_eq!(edited.deadline, Some(10060));
        update_timer_deadline(&mut edited, Some(&old), 10030, true);
        assert_eq!(edited.deadline, Some(10090));
        edited.duration_seconds = 120;
        update_timer_deadline(&mut edited, Some(&old), 10030, false);
        assert_eq!(edited.deadline, Some(10150));
    }
    #[test]
    fn occurrence_keeps_context_after_alarm_is_deleted() {
        let f = Fixture::new(10000);
        reconcile_database(&f.0, 10000).unwrap();
        storage::open(&f.0)
            .unwrap()
            .execute("DELETE FROM alarms", [])
            .unwrap();
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.occurrences[0].context.as_ref().unwrap().kind, "timer");
        let page = storage::history_page(
            &f.0,
            storage::HistoryFilter {
                criteria: serde_json::json!({"kind":"timer"}),
                alarm_ids: None,
                page: 1,
                page_size: 20,
                keyword: String::new(),
                status: String::new(),
                from: None,
                to: None,
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
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
    fn multi_time_catchup_persists_all_recent_slots_without_duplicates() {
        use chrono::TimeZone;
        let at = |hour, minute| {
            chrono::Local
                .with_ymd_and_hms(2026, 10, 22, hour, minute, 0)
                .unwrap()
                .timestamp()
        };
        let f = Fixture::new(at(9, 0));
        let mut alarm = storage::snapshot(&f.0).unwrap().alarms.remove(0);
        alarm.kind = "clock".into();
        alarm.repeat = "daily".into();
        alarm.times = vec![
            "09:00".into(),
            "09:10".into(),
            "09:20".into(),
            "10:00".into(),
        ];
        alarm.revision += 1;
        storage::save_alarm(&f.0, &alarm, Some(at(9, 0))).unwrap();
        let snapshot = storage::snapshot(&f.0).unwrap();
        assert_eq!(snapshot.alarms[0].times, alarm.times);
        assert_eq!(reconcile_database(&f.0, at(9, 25)).unwrap().1.len(), 3);
        assert_eq!(
            storage::snapshot(&f.0).unwrap().next_runs.get("timer"),
            Some(&at(10, 0))
        );
        assert!(reconcile_database(&f.0, at(9, 25)).unwrap().1.is_empty());
        assert_eq!(storage::snapshot(&f.0).unwrap().occurrences.len(), 3);
    }
    #[test]
    fn expired_one_shot_is_automatically_disabled() {
        let f = Fixture::new(10000);
        let (_, ringing, missed) = reconcile_database(&f.0, 11801).unwrap();
        assert!(ringing.is_empty());
        assert_eq!(missed.len(), 1);
        let snap = storage::snapshot(&f.0).unwrap();
        assert_eq!(snap.occurrences[0].status, "skipped_expired");
        assert_eq!(snap.occurrences[0].reason, "未补响");
        assert!(!snap.alarms[0].enabled);
    }

    #[test]
    fn expired_snooze_is_recorded_as_missed() {
        let f = Fixture::new(10000);
        reconcile_database(&f.0, 10000).unwrap();
        let mut o = storage::snapshot(&f.0).unwrap().occurrences.remove(0);
        o.status = "snoozed".into();
        o.snooze_until = Some(10300);
        o.snooze_count = 1;
        persist_occurrence(&storage::open(&f.0).unwrap(), &o).unwrap();
        let (_, ringing, missed) = reconcile_database(&f.0, 12101).unwrap();
        assert!(ringing.is_empty());
        assert_eq!(missed.len(), 1);
        let recorded = storage::snapshot(&f.0).unwrap().occurrences.remove(0);
        assert_eq!(recorded.status, "skipped_expired");
        assert_eq!(recorded.reason, "贪睡后过期，未补响");
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

#[tauri::command]
pub fn open_data_root(app: AppHandle, state: tauri::State<Engine>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let guard = root(&state)?;
    app.opener()
        .open_path(
            guard.as_ref().unwrap().to_string_lossy().to_string(),
            None::<&str>,
        )
        .map_err(|e| e.to_string())
}
