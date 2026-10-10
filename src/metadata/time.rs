//! Capture time: EXIF date and time with offset → milliseconds, and back.

/// `2026:09:12 14:03:22` plus an optional sub-second string (`"42"` → 420 ms) → Unix-style
/// milliseconds of that wall clock. Years before 1 and impossible dates give `None`.
pub(super) fn capture_millis(date: Option<String>, subsec: Option<String>) -> Option<i64> {
    let date = date?;
    let (day, time) = date.split_once(' ')?;
    let mut parts = day.split(':');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    let mut clock = time.split(':');
    let hour: u32 = clock.next()?.parse().ok()?;
    let minute: u32 = clock.next()?.parse().ok()?;
    let second: u32 = clock.next()?.parse().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    let mut millis = days * 86_400_000
        + i64::from(hour) * 3_600_000
        + i64::from(minute) * 60_000
        + i64::from(second) * 1000;
    if let Some(sub) = subsec {
        let digits: String = sub.chars().filter(|c| c.is_ascii_digit()).take(3).collect();
        if !digits.is_empty() {
            let mut fraction: i64 = digits.parse().ok()?;
            for _ in digits.len()..3 {
                fraction *= 10;
            }
            millis += fraction;
        }
    }
    Some(millis)
}

/// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`). `None` for a day the month
/// does not have.
pub(super) fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let mut y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    y -= i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    Some(era * 146_097 + doe as i64 - 719_468)
}

/// A capture time in milliseconds as `YYYY-MM-DD HH:MM`, the format of `CameraInfo::taken` –
/// for a time a camera offset has moved.
pub fn format_millis(ms: i64) -> String {
    let rest = ms.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(ms.div_euclid(86_400_000));
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rest / 3_600_000,
        rest / 60_000 % 60
    )
}

/// The inverse of `days_from_civil` (days since 1970-01-01 → year, month, day).
pub(super) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe as i64 + era * 400 + i64::from(month <= 2), month, day)
}

pub(super) fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

pub(super) fn is_leap(year: i32) -> bool {
    let y = year;
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// `2026:09:12 14:03:22` → `2026-09-12 14:03`
pub(super) fn format_datetime(exif: &str) -> Option<String> {
    let (date, time) = exif.split_once(' ')?;
    let date = date.replace(':', "-");
    let time = time.get(..5)?;
    (date.len() == 10 && !date.starts_with("0000")).then(|| format!("{date} {time}"))
}
