//! What a QuickTime or MP4 file says about itself, read from its box headers only – a few
//! seeks, no decoding: the duration (`moov/mvhd`) and Apple's metadata keys in `moov/meta`
//! (`keys` + `ilst`): the Live Photo content identifier and the capture time
//! (`com.apple.quicktime.creationdate`). Enough to tell a Live Photo's clip from a video
//! (`pairs`) and to name a video by its capture time (`rename`).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use crate::metadata::Clock;

const CONTENT_ID: &str = "com.apple.quicktime.content.identifier";
const CREATION: &str = "com.apple.quicktime.creationdate";

/// A Live Photo's clip lasts about three seconds; a longer one is a video.
pub const LIVE_MAX: Duration = Duration::from_secs(4);

/// The `moov` box is read whole only up to this size (its sample tables can be large).
const MOOV_MAX: u64 = 16 << 20;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Movie {
    pub duration: Option<Duration>,
    /// Apple's identifier shared by a Live Photo's picture and clip.
    pub content_id: Option<String>,
    /// `2026-08-23T19:15:46+0200` as the camera wrote it.
    pub creation: Option<String>,
}

impl Movie {
    /// A Live Photo's clip: Apple's content identifier, or – without it – at most
    /// [`LIVE_MAX`] long.
    pub fn is_live_clip(&self) -> bool {
        self.content_id.is_some() || self.duration.is_some_and(|d| d <= LIVE_MAX)
    }

    /// The capture time as the camera's wall clock, from Apple's creation date (its zone is
    /// the place's, so the clock is what the camera showed). Other videos write only UTC into
    /// `mvhd`, which is no wall clock: `None` then.
    pub fn wall_clock(&self) -> Option<Clock> {
        let date = self.creation.as_deref()?;
        let number = |range: std::ops::Range<usize>| date.get(range)?.parse::<u32>().ok();
        let clock = Clock {
            year: date.get(0..4)?.parse().ok()?,
            month: number(5..7)?,
            day: number(8..10)?,
            hour: number(11..13)?,
            minute: number(14..16)?,
            second: number(17..19)?,
        };
        crate::metadata::millis_of(clock).map(|_| clock)
    }
}

/// Reads `path`'s box headers; an unreadable file or a broken box gives what was found up to
/// there.
pub fn read(path: &Path) -> io::Result<Movie> {
    let mut file = File::open(path)?;
    let end = file.metadata()?.len();
    let mut movie = Movie::default();
    let mut at = 0;
    while at + 8 <= end {
        let Some((kind, body, next)) = header(&mut file, at, end)? else {
            break;
        };
        if &kind == b"moov" {
            let length = next - body;
            if length <= MOOV_MAX {
                let mut moov = vec![0; length as usize];
                file.seek(SeekFrom::Start(body))?;
                file.read_exact(&mut moov)?;
                parse_moov(&moov, &mut movie);
            }
            break;
        }
        at = next;
    }
    Ok(movie)
}

/// The box at `at`: its kind, where its content starts and where the next box is.
fn header(file: &mut File, at: u64, end: u64) -> io::Result<Option<([u8; 4], u64, u64)>> {
    file.seek(SeekFrom::Start(at))?;
    let mut head = [0; 8];
    file.read_exact(&mut head)?;
    let size = u64::from(u32::from_be_bytes([head[0], head[1], head[2], head[3]]));
    let kind = [head[4], head[5], head[6], head[7]];
    let (body, next) = match size {
        0 => (at + 8, end),
        1 => {
            let mut large = [0; 8];
            file.read_exact(&mut large)?;
            (at + 16, at.saturating_add(u64::from_be_bytes(large)))
        }
        size => (at + 8, at + size),
    };
    Ok((next > at && next <= end && body <= next).then_some((kind, body, next)))
}

/// The boxes inside `data`, as (kind, content).
fn children(data: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= data.len() {
        let size = be32(&data[at..]) as usize;
        let kind = [data[at + 4], data[at + 5], data[at + 6], data[at + 7]];
        let (body, next) = match size {
            0 => (at + 8, data.len()),
            1 if at + 16 <= data.len() => {
                (at + 16, at.saturating_add(be64(&data[at + 8..]) as usize))
            }
            size if size >= 8 => (at + 8, at + size),
            _ => break,
        };
        if next > data.len() || body > next {
            break;
        }
        out.push((kind, &data[body..next]));
        at = next;
    }
    out
}

fn be32(data: &[u8]) -> u32 {
    u32::from_be_bytes([data[0], data[1], data[2], data[3]])
}

fn be64(data: &[u8]) -> u64 {
    u64::from_be_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ])
}

fn parse_moov(moov: &[u8], movie: &mut Movie) {
    for (kind, body) in children(moov) {
        match &kind {
            b"mvhd" => movie.duration = duration(body),
            b"meta" => parse_meta(body, movie),
            _ => {}
        }
    }
}

/// `mvhd`: version 0 has 32-bit times, version 1 64-bit ones; then the time scale and the
/// duration.
fn duration(mvhd: &[u8]) -> Option<Duration> {
    let (scale, length) = match *mvhd.first()? {
        0 if mvhd.len() >= 20 => (be32(&mvhd[12..]), u64::from(be32(&mvhd[16..]))),
        1 if mvhd.len() >= 32 => (be32(&mvhd[20..]), be64(&mvhd[24..])),
        _ => return None,
    };
    (scale > 0).then(|| Duration::from_secs_f64(length as f64 / f64::from(scale)))
}

/// QuickTime's `meta` holds `hdlr`, `keys` and `ilst` directly; MP4's starts with a version
/// and flags first.
fn parse_meta(meta: &[u8], movie: &mut Movie) {
    let boxes = if meta.len() >= 8 && &meta[4..8] == b"hdlr" {
        meta
    } else {
        meta.get(4..).unwrap_or_default()
    };
    let mut keys: Vec<String> = Vec::new();
    let mut items: Vec<(u32, &[u8])> = Vec::new();
    for (kind, body) in children(boxes) {
        match &kind {
            b"keys" => keys = key_names(body),
            b"ilst" => {
                items = children(body)
                    .into_iter()
                    .map(|(index, value)| (u32::from_be_bytes(index), value))
                    .collect();
            }
            _ => {}
        }
    }
    for (index, item) in items {
        let Some(name) = index.checked_sub(1).and_then(|i| keys.get(i as usize)) else {
            continue;
        };
        let Some(text) = data_text(item) else {
            continue;
        };
        match name.as_str() {
            CONTENT_ID => movie.content_id = Some(text),
            CREATION => movie.creation = Some(text),
            _ => {}
        }
    }
}

/// `keys`: version and flags, a count, then each key as size, namespace and name.
fn key_names(keys: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let Some(count) = keys.get(4..8).map(be32) else {
        return out;
    };
    let mut at = 8;
    for _ in 0..count {
        let Some(size) = keys.get(at..at + 4).map(be32) else {
            break;
        };
        let size = size as usize;
        if size < 8 || at + size > keys.len() {
            break;
        }
        out.push(String::from_utf8_lossy(&keys[at + 8..at + size]).into_owned());
        at += size;
    }
    out
}

/// An `ilst` item's `data` box as UTF-8 text (type 1).
fn data_text(item: &[u8]) -> Option<String> {
    let (_, data) = children(item)
        .into_iter()
        .find(|(kind, _)| kind == b"data")?;
    (data.len() >= 8 && be32(data) == 1).then(|| String::from_utf8_lossy(&data[8..]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    /// A QuickTime file the way an iPhone writes a Live Photo's clip: `mvhd` with 3 s,
    /// `meta` with Apple's keys – after the `mdat`, as in most files.
    fn live_clip(seconds: u32, with_id: bool) -> Vec<u8> {
        let mut mvhd = vec![0; 20];
        mvhd[12..16].copy_from_slice(&600u32.to_be_bytes());
        mvhd[16..20].copy_from_slice(&(600 * seconds).to_be_bytes());
        let key = |name: &str| {
            let mut entry = ((name.len() + 8) as u32).to_be_bytes().to_vec();
            entry.extend_from_slice(b"mdta");
            entry.extend_from_slice(name.as_bytes());
            entry
        };
        let mut names = vec![CREATION];
        if with_id {
            names.push(CONTENT_ID);
        }
        let mut keys = vec![0, 0, 0, 0];
        keys.extend_from_slice(&(names.len() as u32).to_be_bytes());
        for name in &names {
            keys.extend(key(name));
        }
        let value = |text: &str| {
            let mut data = 1u32.to_be_bytes().to_vec();
            data.extend_from_slice(&[0, 0, 0, 0]);
            data.extend_from_slice(text.as_bytes());
            boxed(b"data", &data)
        };
        let mut ilst = boxed(&1u32.to_be_bytes(), &value("2026-08-23T19:15:46+0200"));
        if with_id {
            ilst.extend(boxed(&2u32.to_be_bytes(), &value("6A1B-ID")));
        }
        let mut meta = boxed(b"hdlr", &[0; 24]);
        meta.extend(boxed(b"keys", &keys));
        meta.extend(boxed(b"ilst", &ilst));
        let mut moov = boxed(b"mvhd", &mvhd);
        moov.extend(boxed(b"meta", &meta));
        let mut file = boxed(b"ftyp", b"qt  \0\0\0\0qt  ");
        file.extend(boxed(b"mdat", &[0; 64]));
        file.extend(boxed(b"moov", &moov));
        file
    }

    fn write(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("cerno-qt-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_live_clip_is_told_apart() {
        let path = write("live.mov", &live_clip(3, true));
        let movie = read(&path).unwrap();
        assert_eq!(movie.duration, Some(Duration::from_secs(3)));
        assert_eq!(movie.content_id.as_deref(), Some("6A1B-ID"));
        assert_eq!(movie.creation.as_deref(), Some("2026-08-23T19:15:46+0200"));
        assert_eq!(
            movie.wall_clock(),
            Some(Clock {
                year: 2026,
                month: 8,
                day: 23,
                hour: 19,
                minute: 15,
                second: 46
            })
        );
        assert!(movie.is_live_clip());
        std::fs::remove_file(path).unwrap();

        // No identifier: a short clip still counts, a long one is a video.
        let path = write("short.mov", &live_clip(3, false));
        assert!(read(&path).unwrap().is_live_clip());
        std::fs::remove_file(path).unwrap();
        let path = write("long.mov", &live_clip(30, false));
        assert!(!read(&path).unwrap().is_live_clip());
        std::fs::remove_file(path).unwrap();
    }

    /// The fixture video (an MP4 without Apple's keys) has its duration and nothing else.
    #[test]
    fn an_mp4_has_its_duration() {
        let movie = read(Path::new("tests/fixtures/tiny.mp4")).unwrap();
        assert!(movie.duration.is_some_and(|d| d > Duration::ZERO));
        assert_eq!(movie.content_id, None);
    }

    #[test]
    fn junk_is_no_movie() {
        let path = write("junk.mov", b"not a movie at all, really");
        assert_eq!(read(&path).unwrap(), Movie::default());
        std::fs::remove_file(path).unwrap();
    }
}
