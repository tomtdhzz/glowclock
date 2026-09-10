//! Local wall-clock time and calendar date without external date crates.
//!
//! The UTC offset is read once from the OS (`date +%z`), matching the
//! convention used elsewhere in this workspace, then applied to the system
//! clock to derive local time, date, and weekday.

use std::time::{SystemTime, UNIX_EPOCH};

/// Local time-of-day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hms {
    pub h: u8,
    pub m: u8,
    pub s: u8,
}

/// Local civil date + time. `dow` is 0=Sunday .. 6=Saturday.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub dow: u32,
    pub h: u8,
    pub m: u8,
    pub s: u8,
}

impl DateTime {
    pub fn hms(&self) -> Hms {
        Hms {
            h: self.h,
            m: self.m,
            s: self.s,
        }
    }
}

/// Current UNIX timestamp in seconds.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Current local time-of-day using `offset_secs` (seconds east of UTC).
pub fn now_local(offset_secs: i32) -> Hms {
    from_epoch(now_unix(), offset_secs)
}

/// Current local date + time using `offset_secs`.
pub fn now_datetime(offset_secs: i32) -> DateTime {
    from_epoch_dt(now_unix(), offset_secs)
}

/// Convert a UNIX timestamp + offset into local time-of-day.
fn from_epoch(unix_secs: i64, offset_secs: i32) -> Hms {
    let tod = (unix_secs + offset_secs as i64).rem_euclid(86_400);
    Hms {
        h: (tod / 3_600) as u8,
        m: ((tod % 3_600) / 60) as u8,
        s: (tod % 60) as u8,
    }
}

/// Convert a UNIX timestamp + offset into a local civil date-time.
fn from_epoch_dt(unix_secs: i64, offset_secs: i32) -> DateTime {
    let local = unix_secs + offset_secs as i64;
    let days = local.div_euclid(86_400);
    let tod = local.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    DateTime {
        year,
        month,
        day,
        dow: weekday_from_days(days),
        h: (tod / 3_600) as u8,
        m: ((tod % 3_600) / 60) as u8,
        s: (tod % 60) as u8,
    }
}

/// Days-since-epoch → `(year, month, day)` (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

/// Days-since-epoch → weekday, 0=Sunday .. 6=Saturday. 1970-01-01 was Thursday.
fn weekday_from_days(z: i64) -> u32 {
    ((z.rem_euclid(7) + 4) % 7) as u32
}

/// Short English weekday name for a `dow` (0=Sun .. 6=Sat).
pub fn weekday_name(dow: u32) -> &'static str {
    const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    NAMES[(dow % 7) as usize]
}

/// The local UTC offset in seconds, read from `date +%z` (e.g. `+0800` →
/// 28800). Falls back to UTC (`0`) if unavailable or unparsable.
pub fn local_utc_offset_seconds() -> i32 {
    use std::process::Command;
    Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| parse_offset(String::from_utf8_lossy(&o.stdout).trim()))
        .unwrap_or(0)
}

/// Parse a `±HHMM` numeric timezone offset into seconds.
fn parse_offset(s: &str) -> Option<i32> {
    let bytes = s.as_bytes();
    if bytes.len() < 5 {
        return None;
    }
    let sign = match bytes[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let hh: i32 = s.get(1..3)?.parse().ok()?;
    let mm: i32 = s.get(3..5)?.parse().ok()?;
    Some(sign * (hh * 3_600 + mm * 60))
}

/// Format a time into the clock's `HH:MM:SS` display string. The colon is
/// steady (no blink); `hour24` selects 24- vs 12-hour hours.
pub fn format_display(t: Hms, hour24: bool) -> String {
    let h = if hour24 {
        t.h
    } else {
        let h = t.h % 12;
        if h == 0 {
            12
        } else {
            h
        }
    };
    format!("{h:02}:{:02}:{:02}", t.m, t.s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_zero_is_midnight_utc() {
        assert_eq!(from_epoch(0, 0), Hms { h: 0, m: 0, s: 0 });
    }

    #[test]
    fn applies_positive_offset() {
        assert_eq!(from_epoch(0, 8 * 3600), Hms { h: 8, m: 0, s: 0 });
    }

    #[test]
    fn wraps_across_day_boundary_with_negative_offset() {
        assert_eq!(from_epoch(30, -3600), Hms { h: 23, m: 0, s: 30 });
    }

    #[test]
    fn parses_offsets() {
        assert_eq!(parse_offset("+0800"), Some(28_800));
        assert_eq!(parse_offset("-0530"), Some(-19_800));
        assert_eq!(parse_offset("+0000"), Some(0));
        assert_eq!(parse_offset("junk"), None);
    }

    #[test]
    fn twelve_hour_midnight_is_twelve() {
        let t = Hms { h: 0, m: 5, s: 9 };
        assert_eq!(format_display(t, false), "12:05:09");
        assert_eq!(format_display(t, true), "00:05:09");
    }

    #[test]
    fn colon_is_steady() {
        let t = Hms { h: 6, m: 46, s: 21 };
        assert_eq!(format_display(t, true), "06:46:21");
    }

    #[test]
    fn civil_date_epoch_is_1970_01_01_thursday() {
        let dt = from_epoch_dt(0, 0);
        assert_eq!((dt.year, dt.month, dt.day), (1970, 1, 1));
        assert_eq!(dt.dow, 4); // Thursday
        assert_eq!(weekday_name(dt.dow), "Thu");
    }

    #[test]
    fn civil_date_known_timestamp() {
        // 2026-09-10 06:46:21 UTC = 1789022781.
        let dt = from_epoch_dt(1_789_022_781, 0);
        assert_eq!((dt.year, dt.month, dt.day), (2026, 9, 10));
        assert_eq!((dt.h, dt.m, dt.s), (6, 46, 21));
    }

    #[test]
    fn offset_can_shift_the_date() {
        // 1970-01-01 23:30 UTC + 1h → 1970-01-02 00:30.
        let dt = from_epoch_dt(23 * 3600 + 30 * 60, 3600);
        assert_eq!((dt.year, dt.month, dt.day), (1970, 1, 2));
        assert_eq!((dt.h, dt.m), (0, 30));
    }
}
