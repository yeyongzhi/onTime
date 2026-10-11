#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClockChange {
    pub shift_seconds: i64,
    pub timezone_changed: bool,
}
impl ClockChange {
    pub fn merge(&mut self, other: Self) {
        self.shift_seconds = self.shift_seconds.saturating_add(other.shift_seconds);
        self.timezone_changed |= other.timezone_changed;
    }
}
#[derive(Default)]
pub struct ClockTracker {
    baseline: Option<(i64, u64)>,
    offset: Option<i32>,
}
impl ClockTracker {
    pub fn observe(&mut self, wall_ms: i64, uptime_ms: u64, offset: i32) -> ClockChange {
        let timezone_changed = self.offset != Some(offset);
        self.offset = Some(offset);
        let Some((wall_start, uptime_start)) = self.baseline else {
            self.baseline = Some((wall_ms, uptime_ms));
            return ClockChange {
                shift_seconds: 0,
                timezone_changed,
            };
        };
        let elapsed = i64::try_from(uptime_ms.saturating_sub(uptime_start)).unwrap_or(i64::MAX);
        let drift = wall_ms.saturating_sub(wall_start).saturating_sub(elapsed);
        let shift_seconds = if drift.unsigned_abs() >= 2000 {
            drift / 1000
        } else {
            0
        };
        if shift_seconds != 0 {
            self.baseline = Some((
                wall_start.saturating_add(shift_seconds.saturating_mul(1000)),
                uptime_start,
            ));
        }
        ClockChange {
            shift_seconds,
            timezone_changed,
        }
    }
}
// Both native sources advance during sleep and are independent of wall-clock adjustment.
#[cfg(target_os = "windows")]
pub fn uptime_ms() -> Option<u64> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetTickCount64() -> u64;
    }
    // SAFETY: GetTickCount64 takes no pointers and is available on supported Windows versions.
    Some(unsafe { GetTickCount64() })
}
#[cfg(target_os = "macos")]
pub fn uptime_ms() -> Option<u64> {
    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }
    unsafe extern "C" {
        fn mach_continuous_time() -> u64;
        fn mach_timebase_info(info: *mut Timebase) -> i32;
    }
    static SCALE: std::sync::OnceLock<Option<(u32, u32)>> = std::sync::OnceLock::new();
    let &(numer, denom) = SCALE
        .get_or_init(|| {
            let mut info = Timebase { numer: 0, denom: 0 };
            // SAFETY: info points to a correctly sized, aligned writable native timebase structure.
            if unsafe { mach_timebase_info(&mut info) } == 0 && info.denom != 0 {
                Some((info.numer, info.denom))
            } else {
                None
            }
        })
        .as_ref()?;
    // SAFETY: This parameterless API is available since macOS 10.12; onTime targets 11+.
    let ticks = unsafe { mach_continuous_time() };
    u64::try_from(u128::from(ticks) * u128::from(numer) / u128::from(denom) / 1_000_000).ok()
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn uptime_ms() -> Option<u64> {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    u64::try_from(
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_millis(),
    )
    .ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sleep_advances_both_clocks_without_deadline_shift() {
        let mut tracker = ClockTracker::default();
        assert!(tracker.observe(1000000, 1000, 28800).timezone_changed);
        assert_eq!(
            tracker.observe(4600000, 3601000, 28800),
            ClockChange::default()
        );
    }
    #[test]
    fn wall_clock_moves_are_detected_once_in_both_directions() {
        let mut tracker = ClockTracker::default();
        tracker.observe(1000000, 1000, 28800);
        assert_eq!(tracker.observe(1300500, 1500, 28800).shift_seconds, 300);
        assert_eq!(tracker.observe(1301000, 2000, 28800).shift_seconds, 0);
        assert_eq!(tracker.observe(1101500, 2500, 28800).shift_seconds, -200);
    }
    #[test]
    fn timezone_change_and_gradual_drift_are_detected() {
        let mut tracker = ClockTracker::default();
        tracker.observe(1000000, 1000, 28800);
        assert!(tracker.observe(1000500, 1500, 0).timezone_changed);
        assert_eq!(tracker.observe(1001999, 2000, 0).shift_seconds, 0);
        assert_eq!(tracker.observe(1004000, 3000, 0).shift_seconds, 2);
    }
    #[test]
    fn native_clock_is_available_and_monotonic() {
        let first = uptime_ms().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(uptime_ms().unwrap() >= first);
    }
}
