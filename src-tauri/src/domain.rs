use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alarm {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub content: String,
    pub enabled: bool,
    pub kind: String,
    pub time: String,
    pub date: Option<String>,
    pub repeat: String,
    #[serde(default)]
    pub monthly_day: Option<u32>,
    #[serde(default)]
    pub effective_start: Option<String>,
    #[serde(default)]
    pub effective_end: Option<String>,
    pub weekdays: Vec<u32>,
    pub duration_seconds: i64,
    pub deadline: Option<i64>,
    pub sound: Option<String>,
    pub volume: f32,
    pub gradual: bool,
    pub ring_seconds: u32,
    pub snooze_limit: u32,
    pub tags: Vec<String>,
    #[serde(default)]
    pub group_id: Option<String>,
    pub revision: u32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<i64>,
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

pub type Group = Tag;

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
            if ![
                "once",
                "daily",
                "workdays",
                "weekends",
                "custom",
                "monthly",
                "quarterly",
                "yearly",
            ]
            .contains(&self.repeat.as_str())
            {
                return Err("重复规则无效".into());
            }
            if self.repeat == "once" || self.repeat == "yearly" {
                NaiveDate::parse_from_str(
                    self.date.as_deref().ok_or("此重复规则需要选择日期")?,
                    "%Y-%m-%d",
                )
                .map_err(|_| "日期无效")?;
            }
            if self.repeat == "monthly"
                && self
                    .monthly_day
                    .or_else(|| {
                        self.date
                            .as_deref()
                            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                            .map(|d| d.day())
                    })
                    .is_none_or(|day| day > 31)
            {
                return Err("请选择有效的每月提醒日期".into());
            }
            if self.repeat == "custom"
                && (self.weekdays.is_empty() || self.weekdays.iter().any(|n| *n > 6))
            {
                return Err("请选择有效的星期".into());
            }
            if self.repeat != "once" {
                match (&self.effective_start, &self.effective_end) {
                    (None, None) => {}
                    (Some(start), Some(end)) => {
                        let start = NaiveDate::parse_from_str(start, "%Y-%m-%d")
                            .map_err(|_| "生效开始日期无效")?;
                        let end = NaiveDate::parse_from_str(end, "%Y-%m-%d")
                            .map_err(|_| "生效结束日期无效")?;
                        if start > end {
                            return Err("生效日期范围的开始日期不能晚于结束日期".into());
                        }
                    }
                    _ => return Err("请选择完整的生效日期范围".into()),
                }
            }
        }
        Ok(())
    }

    pub fn applies_on(&self, date: NaiveDate) -> bool {
        let weekday = date.weekday().num_days_from_monday();
        if self.repeat == "once" {
            return self.date.as_deref() == Some(&date.format("%Y-%m-%d").to_string());
        }
        if self
            .effective_start
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .is_some_and(|start| date < start)
            || self
                .effective_end
                .as_deref()
                .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
                .is_some_and(|end| date > end)
        {
            return false;
        }
        let anchor = self
            .date
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        let selected = match self.repeat.as_str() {
            "daily" => true,
            "workdays" => weekday < 5,
            "weekends" => weekday >= 5,
            "custom" => self.weekdays.contains(&weekday),
            "monthly" => self
                .monthly_day
                .or_else(|| anchor.map(|a| a.day()))
                .is_some_and(|day| {
                    if day == 0 {
                        date.day() == days_in_month(date.year(), date.month())
                    } else {
                        date.day() == day
                    }
                }),
            "quarterly" => {
                [3, 6, 9, 12].contains(&date.month())
                    && date.day() == days_in_month(date.year(), date.month())
            }
            "yearly" => anchor.is_some_and(|a| {
                date.month() == a.month()
                    && date.day() == a.day().min(days_in_month(date.year(), date.month()))
            }),
            _ => false,
        };
        selected
    }

    pub fn next_after(&self, after: i64) -> Option<i64> {
        if !self.enabled {
            return None;
        }
        if self.kind == "timer" {
            return self.deadline.filter(|d| *d > after);
        }
        let after_utc = DateTime::<Utc>::from_timestamp(after, 0)?;
        let mut first = after_utc.with_timezone(&Local).date_naive();
        if let Some(start) = self
            .effective_start
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        {
            first = first.max(start);
        }
        let time = NaiveTime::parse_from_str(&self.time, "%H:%M").ok()?;
        for offset in 0..=370 {
            let date = first.checked_add_signed(Duration::days(offset))?;
            if !self.applies_on(date) {
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

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.and_then(|date| date.pred_opt())
        .map(|date| date.day())
        .unwrap_or(31)
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
            content: String::new(),
            enabled: true,
            kind: "clock".into(),
            time: "09:00".into(),
            date: None,
            repeat: "workdays".into(),
            monthly_day: None,
            effective_start: None,
            effective_end: None,
            weekdays: vec![],
            duration_seconds: 1500,
            deadline: None,
            sound: None,
            volume: 0.7,
            gradual: true,
            ring_seconds: 60,
            snooze_limit: 3,
            tags: vec![],
            group_id: None,
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
    fn invalid_rules_rejected() {
        let mut a = alarm();
        a.volume = f32::NAN;
        assert!(a.validate().is_err());
        a.volume = 0.5;
        a.repeat = "custom".into();
        assert!(a.validate().is_err());
    }

    #[test]
    fn monthly_end_of_month_is_distinct_from_day_31() {
        let mut a = alarm();
        a.repeat = "monthly".into();
        a.monthly_day = Some(0);
        assert!(a.applies_on(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()));
        a.monthly_day = Some(31);
        assert!(!a.applies_on(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()));
        assert!(a.applies_on(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()));
    }

    #[test]
    fn empty_weekly_selection_never_matches() {
        let mut a = alarm();
        a.repeat = "custom".into();
        a.weekdays.clear();
        assert!(!a.applies_on(NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()));
    }

    #[test]
    fn future_effective_start_beyond_one_year_is_schedulable() {
        let mut a = alarm();
        a.repeat = "daily".into();
        a.time = "09:00".into();
        a.effective_start = Some("2034-01-01".into());
        let after = Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2026, 10, 17)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
            )
            .single()
            .unwrap()
            .timestamp();
        let next = a.next_after(after).unwrap();
        assert_eq!(
            DateTime::<Utc>::from_timestamp(next, 0)
                .unwrap()
                .with_timezone(&Local)
                .date_naive(),
            NaiveDate::from_ymd_opt(2034, 1, 1).unwrap()
        );
    }
}
