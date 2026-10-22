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
    #[serde(default)]
    pub times: Vec<String>,
    #[serde(default)]
    pub interval_minutes: Option<u32>,
    #[serde(default)]
    pub schedule_start: Option<i64>,
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
    #[serde(default)]
    pub context: Option<HistoryContext>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryContext {
    #[serde(default)]
    pub inferred: bool,
    pub kind: String,
    pub group: Option<Group>,
    pub tags: Vec<Tag>,
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
            if self
                .interval_minutes
                .is_some_and(|n| ![30, 60, 120].contains(&n))
            {
                return Err("提醒间隔无效".into());
            }
            if self.times.len() > 48 {
                return Err("最多添加 48 个提醒时间".into());
            }
            for time in &self.times {
                NaiveTime::parse_from_str(time, "%H:%M").map_err(|_| "时间格式无效")?;
            }
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

        match self.repeat.as_str() {
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
        }
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
        if self.repeat == "once" {
            first = NaiveDate::parse_from_str(self.date.as_deref()?, "%Y-%m-%d").ok()?;
        }
        if let Some(start) = self
            .effective_start
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        {
            first = first.max(start);
        }
        let mut times = if self.repeat == "once" {
            vec![self.time.clone()]
        } else if let Some(interval) = self.interval_minutes.filter(|n| [30, 60, 120].contains(n)) {
            use chrono::Timelike;
            let anchor = NaiveTime::parse_from_str(&self.time, "%H:%M").ok()?;
            let phase = (anchor.hour() * 60 + anchor.minute()) % interval;
            (phase..1440)
                .step_by(interval as usize)
                .map(|n| format!("{:02}:{:02}", n / 60, n % 60))
                .collect()
        } else if self.times.is_empty() {
            vec![self.time.clone()]
        } else {
            self.times.clone()
        };
        times.sort();
        times.dedup();
        for offset in 0..=370 {
            let date = first.checked_add_signed(Duration::days(offset))?;
            if !self.applies_on(date) {
                continue;
            }
            // On a DST overlap use the earlier instant; on a gap advance to the first valid minute.
            for time in &times {
                let naive = date.and_time(NaiveTime::parse_from_str(time, "%H:%M").ok()?);
                for advance in 0..=180 {
                    if let Some(local) = Local
                        .from_local_datetime(&(naive + Duration::minutes(advance)))
                        .earliest()
                    {
                        let timestamp = local.timestamp();
                        if timestamp > after
                            && (self.repeat == "once"
                                || self.schedule_start.is_none_or(|start| timestamp >= start))
                        {
                            return Some(timestamp);
                        }
                        break;
                    }
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
            times: vec![],
            interval_minutes: None,
            schedule_start: None,
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
    fn multiple_times_are_sorted_and_skip_past_slots() {
        let mut a = alarm();
        a.repeat = "daily".into();
        a.times = vec![
            "11:00".into(),
            "09:00".into(),
            "10:00".into(),
            "10:00".into(),
        ];
        let at = |h, m| {
            Local
                .with_ymd_and_hms(2026, 10, 22, h, m, 0)
                .unwrap()
                .timestamp()
        };
        assert_eq!(a.next_after(at(9, 30)), Some(at(10, 0)));
        assert_eq!(a.next_after(at(10, 0)), Some(at(11, 0)));
        a.effective_end = Some("2026-10-22".into());
        assert_eq!(a.next_after(at(11, 0)), None);
    }
    #[test]
    fn interval_anchor_skip_and_midnight() {
        let mut a = alarm();
        a.repeat = "daily".into();
        a.interval_minutes = Some(120);
        a.time = "11:00".into();
        let at = |d, h, m| {
            Local
                .with_ymd_and_hms(2026, 10, d, h, m, 0)
                .unwrap()
                .timestamp()
        };
        a.schedule_start = Some(at(22, 11, 0));
        assert_eq!(a.next_after(at(22, 8, 59)), Some(at(22, 11, 0)));
        assert_eq!(a.next_after(at(22, 11, 0)), Some(at(22, 13, 0)));
        assert_eq!(a.next_after(at(22, 23, 0)), Some(at(23, 1, 0)));
        a.interval_minutes = Some(30);
        a.time = "09:00".into();
        a.schedule_start = None;
        assert_eq!(a.next_after(at(22, 8, 29)), Some(at(22, 8, 30)));
        assert_eq!(a.next_after(at(22, 8, 59)), Some(at(22, 9, 0)));
        a.interval_minutes = Some(0);
        assert!(a.validate().is_err());
    }
    #[test]
    fn catchup_boundary() {
        assert!(eligible_for_catchup(1, 1800));
        assert!(eligible_for_catchup(1, 1801));
        assert!(!eligible_for_catchup(1, 1802));
        assert!(!eligible_for_catchup(2, 1));
    }
    #[test]
    fn distant_one_time_date_and_date_range_boundaries() {
        let mut a = alarm();
        a.repeat = "once".into();
        a.date = Some("2035-12-31".into());
        let now = Local
            .with_ymd_and_hms(2026, 10, 21, 0, 0, 0)
            .unwrap()
            .timestamp();
        let due = a.next_after(now).unwrap();
        assert_eq!(
            DateTime::from_timestamp(due, 0)
                .unwrap()
                .with_timezone(&Local)
                .date_naive()
                .to_string(),
            "2035-12-31"
        );
        assert!(a.next_after(due).is_none());
        a.repeat = "daily".into();
        a.date = None;
        assert!(
            a.next_after(now + 86400 - 1)
                .is_some_and(|t| t >= now + 86400 && t < now + 2 * 86400)
        );
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
