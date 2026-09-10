//! Crontab-style reminders with a fat-cat popup.
//!
//! Each reminder is one line, either a classic 5-field cron expression or a
//! convenience form, followed by the message text:
//!
//! ```text
//! # min hour dom mon dow   message
//! 0 * * * *                该喝水啦，摸摸胖猫
//! */30 9-18 * * 1-5         起来活动一下颈椎
//! 30 12 * * *              午饭时间到
//! @hourly                  整点报时
//! @daily                   新的一天开始
//! @every 45m               眨眨眼，看看远处
//! ```
//!
//! Cron fields support `*`, `N`, `A-B`, `A-B/S`, `*/S`, and comma lists.
//! `@every <dur>` takes a duration like `90s`, `30m`, `1h`, or `1h30m`.

use crate::clock::DateTime;

/// A set of allowed integer values in `0..=63`, held as a bitmask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FieldSet {
    mask: u64,
    /// True when the source was a bare `*` (relevant to cron day-matching).
    star: bool,
}

impl FieldSet {
    fn contains(&self, v: u32) -> bool {
        v < 64 && self.mask & (1u64 << v) != 0
    }

    /// Parse one cron field constrained to `min..=max`. `names` maps textual
    /// aliases (e.g. `sun`, `jan`) to numbers for the weekday/month fields.
    fn parse(spec: &str, min: u32, max: u32, names: &[(&str, u32)]) -> Result<FieldSet, String> {
        let spec = substitute_names(spec, names)?;
        let spec = spec.as_str();
        let star = spec == "*";
        let mut mask = 0u64;
        for part in spec.split(',') {
            let (range, step) = match part.split_once('/') {
                Some((r, s)) => {
                    let step: u32 = s.parse().map_err(|_| format!("bad step in {part:?}"))?;
                    if step == 0 {
                        return Err(format!("zero step in {part:?}"));
                    }
                    (r, step)
                }
                None => (part, 1),
            };
            let (lo, hi) = if range == "*" {
                (min, max)
            } else if let Some((a, b)) = range.split_once('-') {
                (
                    a.parse().map_err(|_| format!("bad range in {part:?}"))?,
                    b.parse().map_err(|_| format!("bad range in {part:?}"))?,
                )
            } else {
                let v: u32 = range.parse().map_err(|_| format!("bad value {range:?}"))?;
                (v, v)
            };
            if lo < min || hi > max || lo > hi {
                return Err(format!("{part:?} out of range {min}..={max}"));
            }
            let mut v = lo;
            while v <= hi {
                mask |= 1u64 << v;
                v += step;
            }
        }
        Ok(FieldSet { mask, star })
    }

    /// The allowed values within `min..=max`, ascending.
    fn values(&self, min: u32, max: u32) -> Vec<u32> {
        (min..=max).filter(|v| self.contains(*v)).collect()
    }

    /// True when every value in `min..=max` is allowed (i.e. `*`).
    fn is_all(&self, min: u32, max: u32) -> bool {
        (min..=max).all(|v| self.contains(v))
    }

    /// The single allowed value, if exactly one.
    fn single(&self, min: u32, max: u32) -> Option<u32> {
        let vs = self.values(min, max);
        (vs.len() == 1).then(|| vs[0])
    }

    /// A contiguous inclusive range `a..=b` (with `b > a`), if the set is one.
    fn range(&self, min: u32, max: u32) -> Option<(u32, u32)> {
        let vs = self.values(min, max);
        if vs.len() >= 2 && vs.windows(2).all(|w| w[1] - w[0] == 1) {
            Some((vs[0], *vs.last().unwrap()))
        } else {
            None
        }
    }

    /// A uniform step `s >= 2` starting at `min` and covering to the end
    /// (i.e. the `*/s` shape), if the set is one.
    fn step(&self, min: u32, max: u32) -> Option<u32> {
        let vs = self.values(min, max);
        if vs.len() < 2 || vs[0] != min {
            return None;
        }
        let s = vs[1] - vs[0];
        if s >= 2 && vs.windows(2).all(|w| w[1] - w[0] == s) && vs.last().unwrap() + s > max {
            Some(s)
        } else {
            None
        }
    }
}

/// Weekday aliases (0 = Sunday). Accepted in the day-of-week field.
const WEEKDAYS: &[(&str, u32)] = &[
    ("sun", 0),
    ("mon", 1),
    ("tue", 2),
    ("wed", 3),
    ("thu", 4),
    ("fri", 5),
    ("sat", 6),
];

/// Month aliases (1 = January). Accepted in the month field.
const MONTHS: &[(&str, u32)] = &[
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

/// Replace textual aliases (`sun`, `jan`, …) in a field spec with their
/// numbers, leaving digits and `*-,/` untouched.
fn substitute_names(spec: &str, names: &[(&str, u32)]) -> Result<String, String> {
    if names.is_empty() {
        return Ok(spec.to_string());
    }
    let mut out = String::new();
    let mut word = String::new();
    for c in spec.chars() {
        if c.is_ascii_alphabetic() {
            word.push(c);
        } else {
            if !word.is_empty() {
                out.push_str(&resolve_name(&word, names)?);
                word.clear();
            }
            out.push(c);
        }
    }
    if !word.is_empty() {
        out.push_str(&resolve_name(&word, names)?);
    }
    Ok(out)
}

fn resolve_name(word: &str, names: &[(&str, u32)]) -> Result<String, String> {
    let lw = word.to_ascii_lowercase();
    names
        .iter()
        .find(|(n, _)| *n == lw)
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| format!("unknown name {word:?}"))
}

/// A parsed 5-field cron expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cron {
    min: FieldSet,
    hour: FieldSet,
    dom: FieldSet,
    mon: FieldSet,
    dow: FieldSet,
}

impl Cron {
    fn parse(fields: &[&str]) -> Result<Cron, String> {
        if fields.len() != 5 {
            return Err(format!("expected 5 cron fields, got {}", fields.len()));
        }
        let mut dow = FieldSet::parse(fields[4], 0, 7, WEEKDAYS)?;
        // Cron treats both 0 and 7 as Sunday; fold 7 onto 0 for matching.
        if dow.contains(7) {
            dow.mask |= 1;
        }
        Ok(Cron {
            min: FieldSet::parse(fields[0], 0, 59, &[])?,
            hour: FieldSet::parse(fields[1], 0, 23, &[])?,
            dom: FieldSet::parse(fields[2], 1, 31, &[])?,
            mon: FieldSet::parse(fields[3], 1, 12, MONTHS)?,
            dow,
        })
    }

    /// Does this expression fire during the minute of `dt`?
    fn matches(&self, dt: &DateTime) -> bool {
        if !self.min.contains(dt.m as u32) || !self.hour.contains(dt.h as u32) {
            return false;
        }
        if !self.mon.contains(dt.month) {
            return false;
        }
        let dom_ok = self.dom.contains(dt.day);
        let dow_ok = self.dow.contains(dt.dow);
        // Classic cron: when both day fields are restricted, either may match.

        if self.dom.star || self.dow.star {
            dom_ok && dow_ok
        } else {
            dom_ok || dow_ok
        }
    }

    /// A human-readable (Chinese) description of when this fires.
    fn describe(&self) -> String {
        let day = self.day_phrase();
        let time = self.time_phrase();
        // "每天 每小时…" is redundant — keep just the recurring time phrase.
        if day == "每天" && time.starts_with('每') {
            time
        } else {
            format!("{day} {time}")
        }
    }

    /// The day/month portion: "每天", "每周日", "每月 1 日", "每周一至周五" …
    fn day_phrase(&self) -> String {
        let dom_all = self.dom.is_all(1, 31);
        let dow_all = self.dow.is_all(0, 6);
        let mon_all = self.mon.is_all(1, 12);

        let mut day = if dom_all && dow_all {
            "每天".to_string()
        } else if dom_all {
            format!("每{}", weekday_phrase(&self.dow))
        } else if dow_all {
            format!("每月 {} 日", set_str(&self.dom, 1, 31))
        } else {
            format!(
                "每{} 或 每月 {} 日",
                weekday_phrase(&self.dow),
                set_str(&self.dom, 1, 31)
            )
        };
        if !mon_all {
            day = format!("{} 月的 {day}", set_str(&self.mon, 1, 12));
        }
        day
    }

    /// The time-of-day portion: "04:05", "每小时第 0 分", "每 30 分钟" …
    fn time_phrase(&self) -> String {
        let m = &self.min;
        let h = &self.hour;
        let m_all = m.is_all(0, 59);
        let h_all = h.is_all(0, 23);

        if let (Some(mm), Some(hh)) = (m.single(0, 59), h.single(0, 23)) {
            return format!("{hh:02}:{mm:02}");
        }
        if m_all && h_all {
            return "每分钟".to_string();
        }

        let min_part = if m_all {
            "每分钟".to_string()
        } else if let Some(s) = m.step(0, 59) {
            format!("每 {s} 分钟")
        } else if let Some(mm) = m.single(0, 59) {
            format!("第 {mm} 分")
        } else {
            format!("第 {} 分", set_str(m, 0, 59))
        };

        if h_all {
            // Whole-hour cadence.
            if let Some(mm) = m.single(0, 59) {
                return format!("每小时第 {mm} 分");
            }
            return min_part;
        }

        let hour_part = if let Some((a, b)) = h.range(0, 23) {
            format!("{a}-{b} 点")
        } else if let Some(hh) = h.single(0, 23) {
            format!("{hh} 点")
        } else if let Some(s) = h.step(0, 23) {
            format!("每 {s} 小时")
        } else {
            format!("{} 点", set_str(h, 0, 23))
        };
        format!("{hour_part}{min_part}")
    }
}

/// Chinese weekday phrase for a day-of-week set: "周日", "周一至周五", "周一、周三".
fn weekday_phrase(f: &FieldSet) -> String {
    const NAMES: [&str; 7] = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
    if let Some((a, b)) = f.range(0, 6) {
        format!("{}至{}", NAMES[a as usize], NAMES[b as usize])
    } else {
        f.values(0, 6)
            .iter()
            .map(|v| NAMES[*v as usize].to_string())
            .collect::<Vec<_>>()
            .join("、")
    }
}

/// Compact string for a value set: "9-18" for a range, else "0、30".
fn set_str(f: &FieldSet, min: u32, max: u32) -> String {
    if let Some((a, b)) = f.range(min, max) {
        format!("{a}-{b}")
    } else {
        f.values(min, max)
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("、")
    }
}

/// Human-readable (Chinese) duration, e.g. "45 分钟", "1 小时 30 分钟".
fn human_duration(mut secs: i64) -> String {
    let mut parts = Vec::new();
    let days = secs / 86_400;
    secs %= 86_400;
    let hours = secs / 3_600;
    secs %= 3_600;
    let mins = secs / 60;
    let s = secs % 60;
    if days > 0 {
        parts.push(format!("{days} 天"));
    }
    if hours > 0 {
        parts.push(format!("{hours} 小时"));
    }
    if mins > 0 {
        parts.push(format!("{mins} 分钟"));
    }
    if s > 0 || parts.is_empty() {
        parts.push(format!("{s} 秒"));
    }
    parts.join(" ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Schedule {
    Cron(Cron),
    Every { period_secs: i64 },
}

/// A schedule paired with the message to show when it fires.
#[derive(Clone, Debug)]
pub struct Reminder {
    schedule: Schedule,
    pub message: String,
    /// The original source line, for `--list-reminders`.
    pub source: String,
}

impl Reminder {
    /// A human-readable (Chinese) description of the schedule.
    pub fn describe(&self) -> String {
        match &self.schedule {
            Schedule::Cron(c) => c.describe(),
            Schedule::Every { period_secs } => format!("每 {}", human_duration(*period_secs)),
        }
    }

    /// The next UNIX timestamp (>= `from_unix`) at which this fires, using
    /// local `offset_secs` for cron matching. Searches up to ~400 days.
    pub fn next_fire(&self, from_unix: i64, offset_secs: i32) -> Option<i64> {
        match &self.schedule {
            Schedule::Every { period_secs } => Some(from_unix + period_secs),
            Schedule::Cron(c) => {
                let mut t = (from_unix.div_euclid(60) + 1) * 60; // next minute boundary
                for _ in 0..(400 * 24 * 60) {
                    let dt = crate::clock::datetime_at(t, offset_secs);
                    if c.matches(&dt) {
                        return Some(t);
                    }
                    t += 60;
                }
                None
            }
        }
    }

    /// Parse one reminder line. Returns `Ok(None)` for blank/comment lines.
    pub fn parse(line: &str) -> Result<Option<Reminder>, String> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Ok(None);
        }
        let source = trimmed.to_string();

        if let Some(rest) = trimmed.strip_prefix('@') {
            let mut it = rest.splitn(2, char::is_whitespace);
            let kw = it.next().unwrap_or("");
            let remainder = it.next().unwrap_or("").trim();
            let (schedule, message) = match kw {
                "every" => {
                    let mut r = remainder.splitn(2, char::is_whitespace);
                    let dur = r.next().unwrap_or("");
                    let msg = r.next().unwrap_or("").trim();
                    let secs = parse_duration(dur)?;
                    (Schedule::Every { period_secs: secs }, msg)
                }
                "minutely" => (
                    Schedule::Cron(Cron::parse(&["*", "*", "*", "*", "*"])?),
                    remainder,
                ),
                "hourly" => (
                    Schedule::Cron(Cron::parse(&["0", "*", "*", "*", "*"])?),
                    remainder,
                ),
                "daily" | "midnight" => (
                    Schedule::Cron(Cron::parse(&["0", "0", "*", "*", "*"])?),
                    remainder,
                ),
                other => return Err(format!("unknown @keyword: @{other}")),
            };
            return Ok(Some(Reminder {
                schedule,
                message: message.to_string(),
                source,
            }));
        }

        // Five whitespace-separated cron fields, then the message.
        let mut parts = trimmed.split_whitespace();
        let fields: Vec<&str> = parts.by_ref().take(5).collect();
        let cron = Cron::parse(&fields)?;
        let message = trimmed
            .splitn(6, char::is_whitespace)
            .nth(5)
            .unwrap_or("")
            .trim()
            .to_string();
        Ok(Some(Reminder {
            schedule: Schedule::Cron(cron),
            message,
            source,
        }))
    }
}

/// Parse a duration like `90s`, `30m`, `1h`, `1h30m` into seconds.
fn parse_duration(s: &str) -> Result<i64, String> {
    if s.is_empty() {
        return Err("empty duration".into());
    }
    let mut total = 0i64;
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            let n: i64 = num.parse().map_err(|_| format!("bad duration {s:?}"))?;
            num.clear();
            total += match c {
                's' => n,
                'm' => n * 60,
                'h' => n * 3_600,
                'd' => n * 86_400,
                _ => return Err(format!("bad unit {c:?} in {s:?}")),
            };
        }
    }
    if !num.is_empty() {
        // Bare number → seconds.
        total += num
            .parse::<i64>()
            .map_err(|_| format!("bad duration {s:?}"))?;
    }
    if total <= 0 {
        return Err(format!("non-positive duration {s:?}"));
    }
    Ok(total)
}

/// Built-in reminders used when no config file is found.
pub fn default_lines() -> Vec<&'static str> {
    vec![
        "0 * * * *    该喝水啦！起来动一动 (=^.^=)",
        "@every 45m   眨眨眼，看看远处，别盯屏幕太久",
    ]
}

/// Parse many lines, collecting per-line errors (with 1-based line numbers).
pub fn parse_all(lines: &[String]) -> (Vec<Reminder>, Vec<String>) {
    let mut out = Vec::new();
    let mut errs = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        match Reminder::parse(line) {
            Ok(Some(r)) => out.push(r),
            Ok(None) => {}
            Err(e) => errs.push(format!("line {}: {e}", i + 1)),
        }
    }
    (out, errs)
}

/// Tracks which reminders are due, so each fires once per occurrence.
pub struct Manager {
    items: Vec<Reminder>,
    /// For cron items: the last minute-stamp (`unix/60`) at which it fired.
    cron_last: Vec<i64>,
    /// For interval items: the next unix timestamp at which it is due.
    every_next: Vec<i64>,
}

impl Manager {
    pub fn new(items: Vec<Reminder>, now_unix: i64) -> Manager {
        let every_next = items
            .iter()
            .map(|r| match r.schedule {
                Schedule::Every { period_secs } => now_unix + period_secs,
                Schedule::Cron(_) => i64::MAX,
            })
            .collect();
        let cron_last = vec![-1; items.len()];
        Manager {
            items,
            cron_last,
            every_next,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add a reminder at runtime so it fires without restarting.
    pub fn push(&mut self, r: Reminder, now_unix: i64) {
        let next = match r.schedule {
            Schedule::Every { period_secs } => now_unix + period_secs,
            Schedule::Cron(_) => i64::MAX,
        };
        self.items.push(r);
        self.cron_last.push(-1);
        self.every_next.push(next);
    }

    /// Return the message of the first reminder that has become due, advancing
    /// its bookkeeping so it will not re-fire for the same occurrence.
    pub fn poll(&mut self, dt: &DateTime, now_unix: i64) -> Option<String> {
        let minute = now_unix.div_euclid(60);
        for (i, r) in self.items.iter().enumerate() {
            match r.schedule {
                Schedule::Cron(c) => {
                    if c.matches(dt) && self.cron_last[i] != minute {
                        self.cron_last[i] = minute;
                        return Some(r.message.clone());
                    }
                }
                Schedule::Every { period_secs } => {
                    if now_unix >= self.every_next[i] {
                        // Advance past now to avoid a burst after a long gap.
                        let mut next = self.every_next[i] + period_secs;
                        if next <= now_unix {
                            next = now_unix + period_secs;
                        }
                        self.every_next[i] = next;
                        return Some(r.message.clone());
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(month: u32, day: u32, dow: u32, h: u8, m: u8) -> DateTime {
        DateTime {
            year: 2026,
            month,
            day,
            dow,
            h,
            m,
            s: 0,
        }
    }

    #[test]
    fn duration_forms() {
        assert_eq!(parse_duration("90s").unwrap(), 90);
        assert_eq!(parse_duration("30m").unwrap(), 1_800);
        assert_eq!(parse_duration("1h").unwrap(), 3_600);
        assert_eq!(parse_duration("1h30m").unwrap(), 5_400);
        assert_eq!(parse_duration("45").unwrap(), 45);
        assert!(parse_duration("0m").is_err());
        assert!(parse_duration("abc").is_err());
    }

    #[test]
    fn hourly_matches_only_on_minute_zero() {
        let r = Reminder::parse("0 * * * * drink").unwrap().unwrap();
        let Schedule::Cron(c) = r.schedule else {
            panic!("expected cron")
        };
        assert!(c.matches(&dt(9, 10, 4, 13, 0)));
        assert!(!c.matches(&dt(9, 10, 4, 13, 1)));
        assert_eq!(r.message, "drink");
    }

    #[test]
    fn step_and_range_and_weekday() {
        // Every 30 min, 09:00–18:59, Mon–Fri.
        let r = Reminder::parse("*/30 9-18 * * 1-5 stretch")
            .unwrap()
            .unwrap();
        let Schedule::Cron(c) = r.schedule else {
            panic!()
        };
        assert!(c.matches(&dt(9, 10, 4, 9, 0))); // Thu 09:00
        assert!(c.matches(&dt(9, 10, 4, 18, 30))); // Thu 18:30
        assert!(!c.matches(&dt(9, 10, 4, 8, 0))); // before window
        assert!(!c.matches(&dt(9, 10, 4, 9, 15))); // not a 30-min mark
        assert!(!c.matches(&dt(9, 12, 6, 9, 0))); // Saturday
    }

    #[test]
    fn dom_or_dow_when_both_restricted() {
        // Classic cron: day-of-month OR day-of-week when both set.
        let r = Reminder::parse("0 0 13 * 5 payday").unwrap().unwrap();
        let Schedule::Cron(c) = r.schedule else {
            panic!()
        };
        assert!(c.matches(&dt(9, 13, 0, 0, 0))); // the 13th (Sun here)
        assert!(c.matches(&dt(9, 11, 5, 0, 0))); // any Friday
        assert!(!c.matches(&dt(9, 12, 6, 0, 0))); // neither
    }

    #[test]
    fn sunday_accepts_zero_and_seven() {
        let a = Reminder::parse("0 8 * * 0 x").unwrap().unwrap();
        let b = Reminder::parse("0 8 * * 7 x").unwrap().unwrap();
        let (Schedule::Cron(ca), Schedule::Cron(cb)) = (a.schedule, b.schedule) else {
            panic!()
        };
        assert!(ca.matches(&dt(9, 13, 0, 8, 0)));
        assert!(cb.matches(&dt(9, 13, 0, 8, 0)));
    }

    #[test]
    fn convenience_keywords() {
        assert!(matches!(
            Reminder::parse("@hourly beep").unwrap().unwrap().schedule,
            Schedule::Cron(_)
        ));
        let e = Reminder::parse("@every 1h water").unwrap().unwrap();
        assert!(matches!(e.schedule, Schedule::Every { period_secs: 3600 }));
        assert_eq!(e.message, "water");
    }

    fn describe(line: &str) -> String {
        Reminder::parse(line).unwrap().unwrap().describe()
    }

    #[test]
    fn weekday_and_month_names_parse() {
        // crontab.guru's example: "At 04:05 on Sunday."
        assert_eq!(describe("5 4 * * sun x"), "每周日 04:05");
        assert_eq!(describe("0 9 1 jan * x"), "1 月的 每月 1 日 09:00");
        assert!(Reminder::parse("0 0 * * xyz x").is_err());
    }

    #[test]
    fn describes_common_schedules() {
        assert_eq!(describe("0 12 * * * x"), "每天 12:00");
        assert_eq!(describe("30 12 * * * x"), "每天 12:30");
        assert_eq!(describe("0 * * * * x"), "每小时第 0 分");
        assert_eq!(describe("*/5 * * * * x"), "每 5 分钟");
        assert_eq!(describe("0 0 1 * * x"), "每月 1 日 00:00");
        assert_eq!(describe("@every 45m x"), "每 45 分钟");
        assert_eq!(describe("@every 1h30m x"), "每 1 小时 30 分钟");
        assert_eq!(describe("0 9 * * 1-5 x"), "每周一至周五 09:00");
    }

    #[test]
    fn next_fire_is_in_the_future_and_matches() {
        // From 2026-09-10 06:46:21 UTC, next "0 0 * * *" (UTC) is next midnight.
        let base = 1_789_022_781i64;
        let r = Reminder::parse("0 0 * * * x").unwrap().unwrap();
        let next = r.next_fire(base, 0).unwrap();
        assert!(next > base);
        // It must be a minute where the cron actually matches.
        let dt = crate::clock::datetime_at(next, 0);
        assert_eq!((dt.h, dt.m), (0, 0));
    }

    #[test]
    fn next_fire_interval() {
        let r = Reminder::parse("@every 90s x").unwrap().unwrap();
        assert_eq!(r.next_fire(1000, 0), Some(1090));
    }

    #[test]
    fn comments_and_blanks_are_skipped() {
        assert!(Reminder::parse("   ").unwrap().is_none());
        assert!(Reminder::parse("# a note").unwrap().is_none());
    }

    #[test]
    fn interval_fires_once_per_period() {
        let items = vec![Reminder::parse("@every 60s tick").unwrap().unwrap()];
        let base = 1_000_000i64;
        let mut mgr = Manager::new(items, base);
        let d = dt(9, 10, 4, 12, 0);
        assert!(mgr.poll(&d, base + 30).is_none()); // not yet due
        assert_eq!(mgr.poll(&d, base + 60).as_deref(), Some("tick")); // due
        assert!(mgr.poll(&d, base + 90).is_none()); // already fired this period
        assert_eq!(mgr.poll(&d, base + 120).as_deref(), Some("tick")); // next period
    }

    #[test]
    fn cron_fires_once_per_matching_minute() {
        let items = vec![Reminder::parse("0 * * * * water").unwrap().unwrap()];
        let mut mgr = Manager::new(items, 0);
        let at13 = dt(9, 10, 4, 13, 0);
        let base = 13 * 3600; // 13:00:00 unix-of-day
        assert_eq!(mgr.poll(&at13, base).as_deref(), Some("water"));
        assert!(mgr.poll(&at13, base + 30).is_none()); // same minute
        assert!(mgr.poll(&at13, base + 59).is_none());
    }

    #[test]
    fn parse_all_reports_line_errors() {
        let lines = vec![
            "0 * * * * ok".to_string(),
            "99 * * * * bad".to_string(),
            "# comment".to_string(),
        ];
        let (rs, errs) = parse_all(&lines);
        assert_eq!(rs.len(), 1);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].starts_with("line 2:"));
    }
}
