//! `crash.log` in the data folder: every panic, with thread, place and version. Release builds
//! have no console, so without it the message of a crash is lost. Only the panic text is
//! written – no photo names beyond what a message itself carries.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::panic::PanicHookInfo;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// A longer log starts over: only the latest panics matter.
const MAX_BYTES: u64 = 50 * 1024;

/// Appends each panic to the log, then runs the default hook (stderr, backtrace).
pub fn install() {
    let Ok(dir) = crate::paths::data_dir() else {
        return;
    };
    let path = dir.join("crash.log");
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = append(&path, &entry(info));
        default(info);
    }));
}

fn entry(info: &PanicHookInfo) -> String {
    let thread = std::thread::current();
    let place = info
        .location()
        .map(|l| format!("{}:{}", l.file(), l.line()))
        .unwrap_or_default();
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    line(
        seconds,
        thread.name().unwrap_or("unnamed"),
        &place,
        info.payload_as_str().unwrap_or("(no message)"),
    )
}

fn line(unix_seconds: u64, thread: &str, place: &str, message: &str) -> String {
    format!(
        "{} UTC  Cerno {}  thread '{thread}' panicked at {place}: {message}\n",
        utc(unix_seconds),
        env!("CARGO_PKG_VERSION"),
    )
}

fn append(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let full = fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(!full)
        .truncate(full)
        .open(path)?;
    file.write_all(text.as_bytes())
}

/// `YYYY-MM-DD HH:MM:SS` for seconds since 1970 (UTC; days to date after Howard Hinnant).
fn utc(unix_seconds: u64) -> String {
    let days = i64::try_from(unix_seconds / 86_400).unwrap_or(0);
    let rest = unix_seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_utc_calendar_dates() {
        assert_eq!(utc(0), "1970-01-01 00:00:00");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00");
        assert_eq!(utc(1_790_000_000), "2026-09-21 14:13:20");
        assert_eq!(utc(4_107_542_399), "2100-02-28 23:59:59");
    }

    #[test]
    fn a_line_names_version_thread_and_place() {
        let text = line(0, "main", "src/ui/filmstrip.rs:58", "index out of bounds");
        assert_eq!(
            text,
            format!(
                "1970-01-01 00:00:00 UTC  Cerno {}  thread 'main' panicked at \
                 src/ui/filmstrip.rs:58: index out of bounds\n",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn the_log_appends_and_starts_over_when_long() {
        let dir = std::env::temp_dir().join(format!("cerno-crashlog-{}", std::process::id()));
        let path = dir.join("crash.log");
        let _ = fs::remove_dir_all(&dir);
        append(&path, "one\n").unwrap();
        append(&path, "two\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\ntwo\n");
        fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        append(&path, "three\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "three\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}
