//! Rating, orientation and camera data, read from the file bytes that were loaded for
//! decoding anyway.

use std::borrow::Cow;
use std::io::Cursor;

use exif::{Exif, In, Tag, Value};
use memchr::memmem;

/// Microsoft's EXIF rating tag (IFD0 0x4746), written by Windows Explorer.
const EXIF_RATING: exif::Tag = exif::Tag(exif::Context::Tiff, 0x4746);

/// Bumped when capture time or colour-label reading changes, so the analysis re-reads metadata
/// without decoding the image again.
pub const VERSION: i64 = 1;

/// A photo's rating field (`xmp:Rating`): the XMP standard defines -1 as "rejected" and 0 (or
/// no value) as "unrated".
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rating {
    #[default]
    Unrated,
    /// Out of the running (`X`), but kept – nothing is deleted.
    Rejected,
    /// 1..=5
    Stars(u8),
}

impl Rating {
    /// From the tag value: -1 rejected, 1..=5 stars, anything else unrated.
    pub fn from_value(value: i64) -> Self {
        match value {
            -1 => Self::Rejected,
            1..=5 => Self::Stars(value as u8),
            _ => Self::Unrated,
        }
    }

    /// What goes into `xmp:Rating` and the index; `None` removes it.
    pub fn value(self) -> Option<i64> {
        match self {
            Self::Unrated => None,
            Self::Rejected => Some(-1),
            Self::Stars(n) => Some(i64::from(n)),
        }
    }

    pub fn stars(self) -> Option<u8> {
        match self {
            Self::Stars(n) => Some(n),
            _ => None,
        }
    }
}

/// Colour label stored in `xmp:Label`. The file holds the English name Lightroom and Bridge
/// use; reading also accepts the names a localised Lightroom writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Label {
    Red,
    Yellow,
    Green,
    Blue,
    Purple,
}

impl Label {
    pub const ALL: [Label; 5] = [
        Self::Red,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Purple,
    ];

    /// The string written to `xmp:Label`.
    pub fn xmp_name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.id() == id)
    }

    /// `red` (the settings id) or `Red` (the XMP name).
    pub fn from_stored(value: &str) -> Option<Self> {
        Self::from_id(value).or_else(|| {
            Self::ALL
                .into_iter()
                .find(|label| label.xmp_name().eq_ignore_ascii_case(value))
        })
    }
}

/// What `xmp:Label` held. `Other` is a text Cerno does not use as a colour; it is left alone
/// until the user sets a colour of their own.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LabelInfo {
    #[default]
    None,
    Known(Label),
    Other,
}

impl LabelInfo {
    pub fn known(self) -> Option<Label> {
        match self {
            Self::Known(label) => Some(label),
            _ => None,
        }
    }

    pub fn parse(text: &str) -> Self {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Self::None;
        }
        let folded = trimmed.to_lowercase();
        LABEL_NAMES
            .iter()
            .find(|(name, _)| *name == folded)
            .map(|(_, label)| Self::Known(*label))
            .unwrap_or(Self::Other)
    }
}

/// English names plus the colour names of a German, French, Spanish or Italian Lightroom.
const LABEL_NAMES: &[(&str, Label)] = &[
    ("red", Label::Red),
    ("rot", Label::Red),
    ("rouge", Label::Red),
    ("rojo", Label::Red),
    ("rosso", Label::Red),
    ("yellow", Label::Yellow),
    ("gelb", Label::Yellow),
    ("jaune", Label::Yellow),
    ("amarillo", Label::Yellow),
    ("giallo", Label::Yellow),
    ("green", Label::Green),
    ("grün", Label::Green),
    ("gruen", Label::Green),
    ("vert", Label::Green),
    ("verde", Label::Green),
    ("blue", Label::Blue),
    ("blau", Label::Blue),
    ("bleu", Label::Blue),
    ("azul", Label::Blue),
    ("blu", Label::Blue),
    ("purple", Label::Purple),
    ("lila", Label::Purple),
    ("violet", Label::Purple),
    ("violett", Label::Purple),
    ("morado", Label::Purple),
    ("púrpura", Label::Purple),
    ("purpura", Label::Purple),
    ("viola", Label::Purple),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RatingInfo {
    pub value: Rating,
    /// Windows Explorer keeps extra copies of the rating. If they exist they are updated too,
    /// so Explorer never shows a stale value.
    pub has_exif_rating: bool,
    pub has_ms_photo_rating: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileMetadata {
    pub rating: RatingInfo,
    pub label: LabelInfo,
    /// EXIF orientation 1..=8 (1 = as stored).
    pub orientation: u16,
    pub camera: CameraInfo,
}

/// Capture settings for the info bar.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CameraInfo {
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub focal_mm: Option<f64>,
    pub focal_35mm: Option<u32>,
    pub f_number: Option<f64>,
    pub exposure_s: Option<f64>,
    pub iso: Option<u32>,
    /// `YYYY-MM-DD HH:MM`
    pub taken: Option<String>,
    /// `DateTimeOriginal` (else `DateTimeDigitized`) as local wall-clock milliseconds, with
    /// sub-seconds when the file has them. Compared only with other photos, never with UTC.
    pub taken_ms: Option<i64>,
    /// Latitude and longitude in degrees (south and west negative).
    pub gps: Option<(f64, f64)>,
    /// EXIF `DigitalZoomRatio`, only when a digital zoom was used (> 1).
    pub digital_zoom: Option<f64>,
}

impl CameraInfo {
    /// `35 mm · f/2.8 · 1/250 s · ISO 400`
    pub fn exposure_line(&self) -> String {
        let mut parts = Vec::new();
        if let Some(focal) = self.focal_mm {
            let mut text = format!("{} mm", trim_decimal(focal, 0));
            if let Some(eq) = self
                .focal_35mm
                .filter(|eq| (f64::from(*eq) - focal).abs() >= 1.0)
            {
                text.push_str(&format!(" ({eq} mm eq.)"));
            }
            parts.push(text);
        }
        if let Some(f) = self.f_number {
            parts.push(format!("f/{}", trim_decimal(f, 1)));
        }
        if let Some(t) = self.exposure_s {
            parts.push(format_exposure(t));
        }
        if let Some(iso) = self.iso {
            parts.push(format!("ISO {iso}"));
        }
        parts.join("  ·  ")
    }

    /// `Sony ILCE-7M4  ·  FE 24-70mm F2.8 GM`
    pub fn gear_line(&self) -> String {
        [self.camera.as_deref(), self.lens.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("  ·  ")
    }
}

pub fn read(bytes: &[u8]) -> FileMetadata {
    let exif = exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok();
    let exif_uint = |tag| {
        exif.as_ref()
            .and_then(|e| e.get_field(tag, In::PRIMARY))
            .and_then(|f| f.value.get_uint(0))
    };

    let orientation = exif_uint(Tag::Orientation)
        .filter(|o| (1..=8).contains(o))
        .map_or(1, |o| o as u16);
    let exif_rating = exif_uint(EXIF_RATING);
    let camera = exif.as_ref().map(camera_info).unwrap_or_default();

    let xmp = find_xmp_packet(bytes);
    let xmp_rating = xmp.as_deref().and_then(parse_xmp_rating);
    let value = xmp_rating
        .map(i64::from)
        .or(exif_rating.map(i64::from))
        .map_or(Rating::Unrated, Rating::from_value);
    let label = xmp
        .as_deref()
        .and_then(parse_xmp_label)
        .as_deref()
        .map_or(LabelInfo::None, LabelInfo::parse);

    FileMetadata {
        rating: RatingInfo {
            value,
            has_exif_rating: exif_rating.is_some(),
            has_ms_photo_rating: xmp.is_some_and(|x| x.contains("MicrosoftPhoto:Rating")),
        },
        label,
        orientation,
        camera,
    }
}

fn camera_info(exif: &Exif) -> CameraInfo {
    let field = |tag| exif.get_field(tag, In::PRIMARY).map(|f| &f.value);
    let text = |tag| match field(tag)? {
        Value::Ascii(parts) => {
            let s = String::from_utf8_lossy(parts.first()?)
                .trim_matches(['\0', ' '])
                .to_owned();
            (!s.is_empty()).then_some(s)
        }
        _ => None,
    };
    let number = |tag| match field(tag)? {
        Value::Rational(v) => v
            .first()
            .map(|r| r.to_f64())
            .filter(|x| x.is_finite() && *x > 0.0),
        other => other.get_uint(0).map(f64::from),
    };
    let uint = |tag| field(tag)?.get_uint(0).filter(|v| *v > 0);

    // "Canon" + "Canon EOS R5" → "Canon EOS R5"; "NIKON CORPORATION" + "NIKON Z 6" → "NIKON Z 6".
    let camera = match (text(Tag::Make), text(Tag::Model)) {
        (Some(make), Some(model)) => {
            let brand = make
                .split_whitespace()
                .next()
                .unwrap_or(&make)
                .to_lowercase();
            if model.to_lowercase().starts_with(&brand) {
                Some(model)
            } else {
                Some(format!("{make} {model}"))
            }
        }
        (make, model) => model.or(make),
    };
    CameraInfo {
        camera,
        lens: text(Tag::LensModel),
        focal_mm: number(Tag::FocalLength),
        focal_35mm: uint(Tag::FocalLengthIn35mmFilm),
        f_number: number(Tag::FNumber),
        exposure_s: number(Tag::ExposureTime),
        iso: uint(Tag::PhotographicSensitivity),
        taken: text(Tag::DateTimeOriginal).and_then(|t| format_datetime(&t)),
        taken_ms: capture_millis(text(Tag::DateTimeOriginal), text(Tag::SubSecTimeOriginal))
            .or_else(|| {
                capture_millis(text(Tag::DateTimeDigitized), text(Tag::SubSecTimeDigitized))
            }),
        gps: gps(exif),
        digital_zoom: number(Tag::DigitalZoomRatio).filter(|z| *z > 1.01),
    }
}

/// Degrees/minutes/seconds plus N/S and E/W → signed decimal degrees. `(0, 0)` is what some
/// cameras write without a fix, so it counts as no position.
fn gps(exif: &Exif) -> Option<(f64, f64)> {
    let coordinate = |tag, reference, negative: u8| -> Option<f64> {
        let Value::Rational(parts) = &exif.get_field(tag, In::PRIMARY)?.value else {
            return None;
        };
        if parts.is_empty() || parts.iter().any(|r| r.denom == 0) {
            return None;
        }
        let degrees: f64 = parts
            .iter()
            .take(3)
            .zip([1.0, 60.0, 3600.0])
            .map(|(r, unit)| r.to_f64() / unit)
            .sum();
        let sign = match exif.get_field(reference, In::PRIMARY).map(|f| &f.value) {
            Some(Value::Ascii(v)) if v.first().and_then(|s| s.first()) == Some(&negative) => -1.0,
            _ => 1.0,
        };
        Some(sign * degrees)
    };
    let lat = coordinate(Tag::GPSLatitude, Tag::GPSLatitudeRef, b'S')?;
    let lon = coordinate(Tag::GPSLongitude, Tag::GPSLongitudeRef, b'W')?;
    let valid = lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0;
    (valid && (lat, lon) != (0.0, 0.0)).then_some((lat, lon))
}

/// The position in Google Maps.
pub fn maps_url((lat, lon): (f64, f64)) -> String {
    format!("https://www.google.com/maps/search/?api=1&query={lat:.6},{lon:.6}")
}

/// `2026:09:12 14:03:22` plus an optional sub-second string (`"42"` → 420 ms) → Unix-style
/// milliseconds of that wall clock. Years before 1 and impossible dates give `None`.
fn capture_millis(date: Option<String>, subsec: Option<String>) -> Option<i64> {
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
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
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

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap(year: i32) -> bool {
    let y = year;
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// `2026:09:12 14:03:22` → `2026-09-12 14:03`
fn format_datetime(exif: &str) -> Option<String> {
    let (date, time) = exif.split_once(' ')?;
    let date = date.replace(':', "-");
    let time = time.get(..5)?;
    (date.len() == 10 && !date.starts_with("0000")).then(|| format!("{date} {time}"))
}

/// `1/250 s`, `0.3 s`, `2 s`, `30 s`
fn format_exposure(seconds: f64) -> String {
    if seconds < 0.25 {
        format!("1/{} s", (1.0 / seconds).round())
    } else {
        format!("{} s", trim_decimal(seconds, 1))
    }
}

fn trim_decimal(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

/// The first XMP packet in the file. JPEG keeps it in APP1, HEIC in a metadata item – both
/// uncompressed, so a byte search finds it in either container.
fn find_xmp_packet(bytes: &[u8]) -> Option<Cow<'_, str>> {
    for (open, close) in [
        (&b"<x:xmpmeta"[..], &b"</x:xmpmeta>"[..]),
        (&b"<x:xapmeta"[..], &b"</x:xapmeta>"[..]),
    ] {
        if let Some(start) = memmem::find(bytes, open)
            && let Some(len) = memmem::find(&bytes[start..], close)
        {
            return Some(String::from_utf8_lossy(
                &bytes[start..start + len + close.len()],
            ));
        }
    }
    None
}

/// `xmp:Rating` as attribute (`xmp:Rating="4"`) or element (`<xmp:Rating>4</xmp:Rating>`).
/// Old files use the `xap` prefix for the same namespace.
fn parse_xmp_rating(xmp: &str) -> Option<i32> {
    parse_xmp_text(xmp, &["xmp:Rating", "xap:Rating"])
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v.round() as i32)
}

fn parse_xmp_label(xmp: &str) -> Option<String> {
    parse_xmp_text(xmp, &["xmp:Label", "xap:Label"])
}

/// The text of the first matching XMP attribute or element. A longer tag that only starts
/// with the name (`xmp:RatingPercent`) is skipped.
fn parse_xmp_text(xmp: &str, names: &[&str]) -> Option<String> {
    for name in names {
        let mut rest = xmp;
        while let Some(pos) = rest.find(name) {
            rest = &rest[pos + name.len()..];
            let value = if let Some(attr) = rest.trim_start().strip_prefix('=') {
                let attr = attr.trim_start();
                let quote = attr.chars().next().filter(|c| *c == '"' || *c == '\'');
                quote.and_then(|q| attr[1..].split(q).next())
            } else {
                rest.strip_prefix('>').and_then(|el| el.split('<').next())
            };
            if let Some(value) = value {
                return Some(value.trim().to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmp_rating_attribute_and_element() {
        assert_eq!(
            parse_xmp_rating(r#"<rdf:Description xmp:Rating="4" xmp:Label="x"/>"#),
            Some(4)
        );
        assert_eq!(parse_xmp_rating("<a xmp:Rating = '2'/>"), Some(2));
        assert_eq!(
            parse_xmp_rating("<xmp:Rating>5</xmp:Rating><xmp:Label>a</xmp:Label>"),
            Some(5)
        );
        assert_eq!(parse_xmp_rating(r#"<a xap:Rating="3.0"/>"#), Some(3));
        assert_eq!(parse_xmp_rating(r#"<a xmp:Rating="-1"/>"#), Some(-1));
        assert_eq!(parse_xmp_rating(r#"<a MicrosoftPhoto:Rating="75"/>"#), None);
    }

    #[test]
    fn packet_found_in_binary_noise() {
        let mut bytes = vec![0xFF, 0xD8, 0x00, 0x13];
        bytes.extend_from_slice(
            br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:Description xmp:Rating="3" MicrosoftPhoto:Rating="50"/></x:xmpmeta>"#,
        );
        bytes.extend_from_slice(&[0x00, 0xFF, 0xD9]);
        let meta = read(&bytes);
        assert_eq!(meta.rating.value, Rating::Stars(3));
        assert!(meta.rating.has_ms_photo_rating);
        assert!(!meta.rating.has_exif_rating);
        assert_eq!(meta.orientation, 1);
    }

    fn exif_from(fields: &[exif::Field]) -> Exif {
        let mut writer = exif::experimental::Writer::new();
        for field in fields {
            writer.push_field(field);
        }
        let mut tiff = Cursor::new(Vec::new());
        writer.write(&mut tiff, false).unwrap();
        exif::Reader::new().read_raw(tiff.into_inner()).unwrap()
    }

    fn field(tag: Tag, value: Value) -> exif::Field {
        exif::Field {
            tag,
            ifd_num: In::PRIMARY,
            value,
        }
    }

    fn rational(num: u32, denom: u32) -> Value {
        Value::Rational(vec![exif::Rational { num, denom }])
    }

    #[test]
    fn camera_info_from_exif() {
        let exif = exif_from(&[
            field(Tag::Make, Value::Ascii(vec![b"NIKON CORPORATION".to_vec()])),
            field(Tag::Model, Value::Ascii(vec![b"NIKON Z 6_2".to_vec()])),
            field(
                Tag::LensModel,
                Value::Ascii(vec![b"NIKKOR Z 24-70mm f/4 S".to_vec()]),
            ),
            field(Tag::FocalLength, rational(35, 1)),
            field(Tag::FNumber, rational(28, 10)),
            field(Tag::ExposureTime, rational(1, 250)),
            field(Tag::PhotographicSensitivity, Value::Short(vec![400])),
            field(
                Tag::DateTimeOriginal,
                Value::Ascii(vec![b"2026:09:12 14:03:22".to_vec()]),
            ),
        ]);
        let info = camera_info(&exif);
        assert_eq!(info.camera.as_deref(), Some("NIKON Z 6_2"));
        assert_eq!(info.taken.as_deref(), Some("2026-09-12 14:03"));
        assert_eq!(
            info.exposure_line(),
            "35 mm  ·  f/2.8  ·  1/250 s  ·  ISO 400"
        );
        assert_eq!(info.gear_line(), "NIKON Z 6_2  ·  NIKKOR Z 24-70mm f/4 S");
    }

    fn rationals(values: &[(u32, u32)]) -> Value {
        Value::Rational(
            values
                .iter()
                .map(|&(num, denom)| exif::Rational { num, denom })
                .collect(),
        )
    }

    #[test]
    fn gps_position_and_digital_zoom() {
        let ascii = |s: &str| Value::Ascii(vec![s.as_bytes().to_vec()]);
        // 48° 31' 17.76" N, 9° 3' 27.36" W
        let exif = exif_from(&[
            field(Tag::GPSLatitudeRef, ascii("N")),
            field(
                Tag::GPSLatitude,
                rationals(&[(48, 1), (31, 1), (1776, 100)]),
            ),
            field(Tag::GPSLongitudeRef, ascii("W")),
            field(Tag::GPSLongitude, rationals(&[(9, 1), (3, 1), (2736, 100)])),
            field(Tag::DigitalZoomRatio, rational(2, 1)),
        ]);
        let info = camera_info(&exif);
        let (lat, lon) = info.gps.unwrap();
        assert!((lat - 48.5216).abs() < 1e-6, "{lat}");
        assert!((lon + 9.0576).abs() < 1e-6, "{lon}");
        assert_eq!(info.digital_zoom, Some(2.0));
        assert_eq!(
            maps_url((lat, lon)),
            "https://www.google.com/maps/search/?api=1&query=48.521600,-9.057600"
        );

        // No fix, a broken rational and "no digital zoom" (1/1 or 0/0) give nothing.
        let exif = exif_from(&[
            field(Tag::GPSLatitude, rationals(&[(0, 1), (0, 1), (0, 1)])),
            field(Tag::GPSLongitude, rationals(&[(0, 1), (0, 1), (0, 1)])),
            field(Tag::DigitalZoomRatio, rational(1, 1)),
        ]);
        let info = camera_info(&exif);
        assert_eq!((info.gps, info.digital_zoom), (None, None));
        let exif = exif_from(&[
            field(Tag::GPSLatitude, rationals(&[(48, 0)])),
            field(Tag::GPSLongitude, rationals(&[(9, 1)])),
            field(Tag::DigitalZoomRatio, rational(0, 0)),
        ]);
        let info = camera_info(&exif);
        assert_eq!((info.gps, info.digital_zoom), (None, None));
    }

    #[test]
    fn exposure_formatting() {
        assert_eq!(format_exposure(1.0 / 8000.0), "1/8000 s");
        assert_eq!(format_exposure(0.3), "0.3 s");
        assert_eq!(format_exposure(2.0), "2 s");
        let info = CameraInfo {
            camera: Some("Sony ILCE-7M4".into()),
            focal_mm: Some(4.25),
            focal_35mm: Some(26),
            f_number: Some(1.8),
            ..CameraInfo::default()
        };
        assert_eq!(info.exposure_line(), "4 mm (26 mm eq.)  ·  f/1.8");
        assert_eq!(info.gear_line(), "Sony ILCE-7M4");
        assert_eq!(CameraInfo::default().exposure_line(), "");
    }

    #[test]
    fn labels_round_trip_including_localised_names() {
        assert_eq!(LabelInfo::parse("Red"), LabelInfo::Known(Label::Red));
        assert_eq!(LabelInfo::parse("  gelb "), LabelInfo::Known(Label::Yellow));
        assert_eq!(LabelInfo::parse("Grün"), LabelInfo::Known(Label::Green));
        assert_eq!(LabelInfo::parse("Violet"), LabelInfo::Known(Label::Purple));
        assert_eq!(LabelInfo::parse("Púrpura"), LabelInfo::Known(Label::Purple));
        assert_eq!(LabelInfo::parse(""), LabelInfo::None);
        assert_eq!(LabelInfo::parse("Selects"), LabelInfo::Other);
        for label in Label::ALL {
            assert_eq!(Label::from_id(label.id()), Some(label));
            assert_eq!(LabelInfo::parse(label.xmp_name()), LabelInfo::Known(label));
        }
        let meta = read(br#"<x:xmpmeta><a xmp:Label="Rouge" xmp:Rating="2"/></x:xmpmeta>"#);
        assert_eq!(meta.label, LabelInfo::Known(Label::Red));
        assert_eq!(meta.rating.value, Rating::Stars(2));
        let meta = read(br#"<x:xmpmeta><xmp:Label>Blue</xmp:Label></x:xmpmeta>"#);
        assert_eq!(meta.label, LabelInfo::Known(Label::Blue));
        let meta = read(br#"<x:xmpmeta><a xmp:Label="Kundenauswahl"/></x:xmpmeta>"#);
        assert_eq!(meta.label, LabelInfo::Other);
    }

    #[test]
    fn capture_time_uses_subseconds_and_falls_back() {
        assert_eq!(days_from_civil(1970, 1, 1), Some(0));
        assert_eq!(days_from_civil(2000, 1, 1), Some(10_957));
        assert_eq!(days_from_civil(2024, 2, 29), Some(19_782));
        assert_eq!(days_from_civil(2023, 2, 29), None);
        assert_eq!(
            capture_millis(Some("1970:01:01 00:00:00".into()), None),
            Some(0)
        );
        assert_eq!(
            capture_millis(Some("1970:01:01 00:00:01".into()), Some("42".into())),
            Some(1_420)
        );
        assert_eq!(
            capture_millis(Some("1970:01:01 00:00:01".into()), Some("4".into())),
            Some(1_400)
        );
        assert_eq!(
            capture_millis(Some("0000:00:00 00:00:00".into()), None),
            None
        );

        let exif = exif_from(&[
            field(
                Tag::DateTimeOriginal,
                Value::Ascii(vec![b"2026:09:12 14:03:22".to_vec()]),
            ),
            field(Tag::SubSecTimeOriginal, Value::Ascii(vec![b"5".to_vec()])),
        ]);
        let info = camera_info(&exif);
        let again = capture_millis(Some("2026:09:12 14:03:22".into()), Some("5".into()));
        assert_eq!(info.taken_ms, again);
        assert!(info.taken_ms.unwrap() % 1000 == 500);

        let exif = exif_from(&[field(
            Tag::DateTimeDigitized,
            Value::Ascii(vec![b"2026:09:12 14:03:22".to_vec()]),
        )]);
        assert_eq!(
            camera_info(&exif).taken_ms,
            capture_millis(Some("2026:09:12 14:03:22".into()), None)
        );
    }

    #[test]
    fn rejected_and_unrated() {
        let meta = read(br#"<x:xmpmeta><a xmp:Rating="-1"/></x:xmpmeta>"#);
        assert_eq!(meta.rating.value, Rating::Rejected);
        assert_eq!(meta.rating.value.stars(), None);
        let meta = read(br#"<x:xmpmeta><a xmp:Rating="0"/></x:xmpmeta>"#);
        assert_eq!(meta.rating.value, Rating::Unrated);
        assert_eq!(read(b"no metadata").rating.value, Rating::Unrated);
        for rating in [Rating::Unrated, Rating::Rejected, Rating::Stars(4)] {
            assert_eq!(
                rating.value().map_or(Rating::Unrated, Rating::from_value),
                rating
            );
        }
    }
}
