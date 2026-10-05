//! Camera clocks set right: an offset per camera model and folder, added to the capture time
//! of that camera's photos for sorting, series and the info bar (`Db::camera_offsets`). Only
//! the index holds it – the files keep their own time. Written and read as `±h:mm:ss`. Pure,
//! so it is tested here.

/// The offset that gives the right photo the left one's time: both were taken at the same
/// moment. `left_offset` is what the left camera already has.
pub fn aligned(left_taken: i64, left_offset: i64, right_taken: i64) -> i64 {
    left_taken + left_offset - right_taken
}

/// `+2:09:37` / `−0:00:05`, whole seconds; `0:00:00` without a sign.
pub fn format_offset(ms: i64) -> String {
    let seconds = (ms.abs() + 500) / 1000;
    if seconds == 0 {
        return "0:00:00".to_owned();
    }
    let sign = if ms < 0 { '−' } else { '+' };
    format!(
        "{sign}{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// What the user typed: `h`, `h:mm` or `h:mm:ss`, with `+`, `-` or `−` in front; empty is 0.
/// Minutes and seconds below 60; `None` for anything else.
pub fn parse_offset(text: &str) -> Option<i64> {
    let text = text.trim();
    let (sign, rest) = if let Some(rest) = text.strip_prefix('+') {
        (1, rest)
    } else if let Some(rest) = text.strip_prefix(['-', '−']) {
        (-1, rest)
    } else {
        (1, text)
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return text.is_empty().then_some(0);
    }
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() > 3
        || parts
            .iter()
            .any(|p| p.is_empty() || p.len() > 6 || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let number = |i: usize| parts.get(i).map_or(Some(0), |p| p.parse::<i64>().ok());
    let (hours, minutes, seconds) = (number(0)?, number(1)?, number(2)?);
    if minutes > 59 || seconds > 59 {
        return None;
    }
    Some(sign * ((hours * 60 + minutes) * 60 + seconds) * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_read_and_write_as_hours_minutes_seconds() {
        assert_eq!(format_offset(-7_777_400), "−2:09:37");
        assert_eq!(format_offset(3_600_000), "+1:00:00");
        assert_eq!(format_offset(400), "0:00:00", "rounded to whole seconds");
        assert_eq!(format_offset(26 * 3_600_000), "+26:00:00", "a wrong day");
        for (text, ms) in [
            ("", Some(0)),
            ("0:00:00", Some(0)),
            ("+2:09:37", Some(7_777_000)),
            ("−2:09:37", Some(-7_777_000)),
            ("-1:30", Some(-5_400_000)),
            (" 3 ", Some(10_800_000)),
            ("1:60", None),
            ("1:2:3:4", None),
            ("1h", None),
            ("-", None),
            ("1::2", None),
        ] {
            assert_eq!(parse_offset(text), ms, "{text:?}");
        }
        assert_eq!(parse_offset(&format_offset(-7_777_000)), Some(-7_777_000));
    }

    /// The right camera gets the time of the left one, whose own offset counts.
    #[test]
    fn the_right_camera_takes_the_left_ones_time() {
        let offset = aligned(10_000, 500, 4_000);
        assert_eq!(4_000 + offset, 10_000 + 500);
    }
}
