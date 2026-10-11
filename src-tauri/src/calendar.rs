use crate::{domain::Calendar, engine::Engine, storage};
use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Deserialize)]
struct YearData {
    year: i32,
    papers: Vec<String>,
    days: Vec<Day>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Day {
    date: String,
    is_off_day: bool,
}
const SOURCE: &str = "NateScarlet/holiday-cn（依据国务院公告，MIT）";
pub fn parse(bytes: &[u8], year: i32) -> Result<Calendar, String> {
    if bytes.len() > 1024 * 1024 {
        return Err("日历数据过大".into());
    }
    let raw: YearData = serde_json::from_slice(bytes).map_err(|e| format!("日历格式错误：{e}"))?;
    if raw.year != year || raw.papers.is_empty() || raw.days.is_empty() || raw.days.len() > 366 {
        return Err("日历尚未公布或年份无效，保留已有缓存".into());
    }
    let mut dates = BTreeMap::new();
    for day in raw.days {
        let date = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d").map_err(|_| "日历日期无效")?;
        if date.year() != year || dates.insert(day.date, day.is_off_day).is_some() {
            return Err("日历包含重复日期或错误年份".into());
        }
    }
    Ok(Calendar {
        year,
        source: SOURCE.into(),
        fetched_at: Utc::now().timestamp(),
        dates,
    })
}
pub fn seed(root: &std::path::Path) -> Result<(), String> {
    let conn = storage::open(root)?;
    for (year, bytes) in [
        (
            2025,
            include_bytes!("../resources/calendar/2025.json").as_slice(),
        ),
        (
            2026,
            include_bytes!("../resources/calendar/2026.json").as_slice(),
        ),
    ] {
        let calendar = parse(bytes, year)?;
        conn.execute(
            "INSERT OR IGNORE INTO calendars(year,json) VALUES(?1,?2)",
            rusqlite::params![
                year,
                serde_json::to_string(&calendar).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub async fn refresh_calendar(app: AppHandle, year: i32) -> Result<String, String> {
    if !(2020..=2100).contains(&year) {
        return Err("年份超出范围".into());
    }
    tauri::async_runtime::spawn_blocking(move|| {
        let client=reqwest::blocking::Client::builder().timeout(Duration::from_secs(20)).user_agent("onTime/0.1.0").build().map_err(|e|e.to_string())?;
        let response=client.get(format!("https://raw.githubusercontent.com/NateScarlet/holiday-cn/master/{year}.json")).send().map_err(|e|format!("网络失败，保留日历缓存：{e}"))?.error_for_status().map_err(|e|format!("日历暂不可用：{e}"))?;
        let mut bytes=vec![]; response.take(1024*1024+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
        let calendar=parse(&bytes,year)?;
        let state=app.state::<Engine>(); let guard=state.root.lock().map_err(|e|e.to_string())?;
        let root=guard.as_ref().ok_or("请先选择数据目录")?;
        let snap=storage::snapshot(root)?;
        let mut conn=storage::open(root)?; let tx=conn.transaction().map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO calendars(year,json) VALUES(?1,?2) ON CONFLICT(year) DO UPDATE SET json=excluded.json",rusqlite::params![year,serde_json::to_string(&calendar).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        if year==chrono::Local::now().year() {
            for alarm in snap.alarms.iter().filter(|a|a.kind=="clock") {
                if snap.next_runs.get(&alarm.id).is_some_and(|due|*due<=Utc::now().timestamp()) { continue; }
                tx.execute("UPDATE alarms SET next_at=?2 WHERE id=?1",rusqlite::params![alarm.id,alarm.next_after(Utc::now().timestamp(),Some(&calendar))]).map_err(|e|e.to_string())?;
            }
        }
        tx.commit().map_err(|e|e.to_string())?;
        std::fs::write(root.join("calendar").join(format!("{year}.json")),&bytes).map_err(|e|e.to_string())?;
        drop(guard); let _=app.emit("state-changed",()); Ok(format!("{year} 年日历已更新"))
    }).await.map_err(|e|e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_calendar_has_official_makeup_day() {
        let c = parse(include_bytes!("../resources/calendar/2026.json"), 2026).unwrap();
        assert_eq!(c.dates.get("2026-10-01"), Some(&true));
        assert_eq!(c.dates.get("2026-10-10"), Some(&false));
    }
    #[test]
    fn empty_future_calendar_not_accepted() {
        assert!(parse(br#"{"year":2027,"papers":[],"days":[]}"#, 2027).is_err());
        assert!(parse(include_bytes!("../resources/calendar/2026.json"), 2027).is_err());
    }
}
