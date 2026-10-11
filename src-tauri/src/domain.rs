use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alarm {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub kind: String,
    pub time: String,
    pub date: Option<String>,
    pub repeat: String,
    pub weekdays: Vec<u32>,
    pub skip_holidays: bool,
    pub duration_seconds: i64,
    pub deadline: Option<i64>,
    pub sound: Option<String>,
    pub volume: f32,
    pub gradual: bool,
    pub ring_seconds: u32,
    pub snooze_limit: u32,
    pub tags: Vec<String>,
    pub revision: u32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Occurrence {
    pub id: String,
    pub alarm_id: String,
    pub title: String,
    pub scheduled_at: i64,
    pub status: String,
    pub snooze_count: u32,
    pub snooze_until: Option<i64>,
    pub started_at: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub year: i32,
    pub source: String,
    pub fetched_at: i64,
    /// true = rest day, false = official working day; absent falls back to weekday.
    pub dates: BTreeMap<String, bool>,
}

impl Alarm {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() || self.title.chars().count() > 80 {
            return Err("标题需为 1–80 个字符".into());
        }
        if !["clock", "timer"].contains(&self.kind.as_str()) {
            return Err("不支持的提醒类型".into());
        }
        if !(0.0..=1.0).contains(&self.volume)
            || self.ring_seconds == 0
            || self.ring_seconds > 3600
            || self.snooze_limit > 10
        {
            return Err("音量、响铃时长或贪睡次数无效".into());
        }
        if self.kind == "timer" {
            if !(1..=604800).contains(&self.duration_seconds) {
                return Err("倒计时需为 1 秒到 7 天".into());
            }
        } else {
            NaiveTime::parse_from_str(&self.time, "%H:%M").map_err(|_| "时间格式无效")?;
            if !["once", "daily", "workdays", "weekends", "custom"].contains(&self.repeat.as_str())
            {
                return Err("重复规则无效".into());
            }
            if self.repeat == "once" {
                NaiveDate::parse_from_str(
                    self.date.as_deref().ok_or("一次性提醒需要日期")?,
                    "%Y-%m-%d",
                )
                .map_err(|_| "日期无效")?;
            }
            if self.repeat == "custom"
                && (self.weekdays.is_empty() || self.weekdays.iter().any(|n| *n > 6))
            {
                return Err("请选择有效的星期".into());
            }
        }
        Ok(())
    }

    pub fn applies_on(&self, date: NaiveDate, calendar: Option<&Calendar>) -> bool {
        let weekday = date.weekday().num_days_from_monday();
        let special = calendar
            .filter(|c| c.year == date.year())
            .and_then(|c| c.dates.get(&date.format("%Y-%m-%d").to_string()));
        if self.repeat == "once" {
            return self.date.as_deref() == Some(&date.format("%Y-%m-%d").to_string());
        }
        if self.skip_holidays && self.repeat == "workdays" {
            if let Some(rest) = special {
                return !rest;
            }
        }
        let selected = match self.repeat.as_str() {
            "daily" => true,
            "workdays" => weekday < 5,
            "weekends" => weekday >= 5,
            "custom" => self.weekdays.contains(&weekday),
            _ => false,
        };
        selected && !(self.skip_holidays && special == Some(&true))
    }

    pub fn next_after(&self, after: i64, calendar: Option<&Calendar>) -> Option<i64> {
        if !self.enabled {
            return None;
        }
        if self.kind == "timer" {
            return self.deadline.filter(|d| *d > after);
        }
        let after_utc = DateTime::<Utc>::from_timestamp(after, 0)?;
        let first = after_utc.with_timezone(&Local).date_naive();
        let time = NaiveTime::parse_from_str(&self.time, "%H:%M").ok()?;
        for offset in 0..=370 {
            let date = first.checked_add_signed(Duration::days(offset))?;
            if !self.applies_on(date, calendar) {
                continue;
            }
            // On a DST overlap use the earlier instant; on a gap advance to the first valid minute.
            let naive = date.and_time(time);
            for advance in 0..=180 {
                if let Some(local) = Local
                    .from_local_datetime(&(naive + Duration::minutes(advance)))
                    .earliest()
                {
                    let timestamp = local.timestamp();
                    if timestamp > after {
                        return Some(timestamp);
                    }
                    break;
                }
            }
        }
        None
    }
}

pub fn eligible_for_catchup(scheduled_at: i64, now: i64) -> bool {
    (0..=1800).contains(&(now - scheduled_at))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn alarm() -> Alarm {
        Alarm {
            id: "a".into(),
            title: "会议".into(),
            enabled: true,
            kind: "clock".into(),
            time: "09:00".into(),
            date: None,
            repeat: "workdays".into(),
            weekdays: vec![],
            skip_holidays: true,
            duration_seconds: 1500,
            deadline: None,
            sound: None,
            volume: 0.7,
            gradual: true,
            ring_seconds: 60,
            snooze_limit: 3,
            tags: vec![],
            revision: 1,
            created_at: 0,
        }
    }
    #[test]
    fn catchup_boundary() {
        assert!(eligible_for_catchup(1, 1800));
        assert!(eligible_for_catchup(1, 1801));
        assert!(!eligible_for_catchup(1, 1802));
        assert!(!eligible_for_catchup(2, 1));
    }
    #[test]
    fn workday_calendar_overrides_weekend() {
        let a = alarm();
        let sunday = NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
        let mut calendar = Calendar {
            year: 2026,
            ..Default::default()
        };
        calendar.dates.insert("2026-10-11".into(), false);
        assert!(a.applies_on(sunday, Some(&calendar)));
        assert!(!a.applies_on(sunday, None));
    }
    #[test]
    fn holiday_excluded_and_custom_not_extended() {
        let mut a = alarm();
        let thursday = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let mut calendar = Calendar {
            year: 2026,
            ..Default::default()
        };
        calendar.dates.insert("2026-10-01".into(), true);
        assert!(!a.applies_on(thursday, Some(&calendar)));
        a.repeat = "custom".into();
        a.weekdays = vec![0];
        calendar.dates.insert("2026-10-11".into(), false);
        assert!(!a.applies_on(
            NaiveDate::from_ymd_opt(2026, 10, 11).unwrap(),
            Some(&calendar)
        ));
    }
    #[test]
    fn invalid_rules_rejected() {
        let mut a = alarm();
        a.volume = f32::NAN;
        assert!(a.validate().is_err());
        a.volume = 0.5;
        a.repeat = "custom".into();
        assert!(a.validate().is_err());
    }
}
