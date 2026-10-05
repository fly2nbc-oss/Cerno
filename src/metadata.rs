//! Rating, orientation and camera data, read from the file bytes that were loaded for
//! decoding anyway.

use std::borrow::Cow;
use std::io::Cursor;

use exif::{Exif, In, Tag, Value};
use memchr::memmem;

/// Microsoft's EXIF rating tag (IFD0 0x4746), written by Windows Explorer.
const EXIF_RATING: exif::Tag = exif::Tag(exif::Context::Tiff, 0x4746);

/// Bumped when capture time, camera or colour-label reading changes, so the analysis re-reads
/// metadata without decoding the image again. 2: the camera model (series per camera).
pub const VERSION: i64 = 2;

/// Stable id of a camera model for series (FNV-1a, not `DefaultHasher`): case and outer
/// spaces don't count.
pub fn camera_id(model: &str) -> u64 {
    model
        .trim()
        .to_lowercase()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}

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
    pub description: Description,
}

/// A photo's comment and keywords as other programs show them: XMP `dc:description` /
/// `dc:subject`, else IPTC Caption-Abstract / Keywords. Written back through ExifTool's MWG
/// tags, which keep IPTC, XMP and the EXIF description in step.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Description {
    /// Empty: none.
    pub comment: String,
    pub keywords: Vec<String>,
}

impl Description {
    /// Adds a keyword unless it is empty or already there (case-insensitive). Returns whether
    /// it was added.
    pub fn add_keyword(&mut self, keyword: &str) -> bool {
        let keyword = keyword.trim();
        if keyword.is_empty()
            || self
                .keywords
                .iter()
                .any(|k| k.to_lowercase() == keyword.to_lowercase())
        {
            return false;
        }
        self.keywords.push(keyword.to_owned());
        true
    }
}

/// Capture settings for the info bar.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CameraInfo {
    pub camera: Option<String>,
    /// Tells cameras apart for series: the EXIF model, else the make. The model alone, because
    /// some files of the same camera lack the make ("FC7503" next to "DJI FC7503"). Two bodies
    /// of the same model look alike – phones write no serial number.
    pub model: Option<String>,
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

/// The metadata of the file at `path`, from its bytes. A RAW whose container the EXIF reader
/// does not know (CR3, RAF, ORF, RW2 …) gives its camera data and orientation through its
/// JPEG preview. Where the marks live in an XMP sidecar, rating and label come from there as
/// soon as one exists – until then the file's own (a rating given in the camera) stay.
pub fn read_for(path: &std::path::Path, bytes: &[u8]) -> FileMetadata {
    let format = crate::library::format_of(path);
    let mut meta = read(bytes);
    if format.is_some_and(crate::library::Format::is_raw)
        && meta.camera.model.is_none()
        && let Some(preview) = crate::raw::preview(bytes)
    {
        let inner = read(preview);
        meta.camera = inner.camera;
        meta.orientation = inner.orientation;
    }
    if crate::sidecar::applies(path) {
        with_sidecar(&mut meta, path);
    }
    meta
}

/// Only what the photo's sidecar says (defaults without one): for a video, which is never read
/// whole, and for the writer once the sidecar exists.
pub fn read_sidecar(path: &std::path::Path) -> FileMetadata {
    let mut meta = read(&[]);
    with_sidecar(&mut meta, path);
    meta
}

/// The sidecar's rating, label, comment and keywords replace the file's. It holds no Windows
/// rating tags.
fn with_sidecar(meta: &mut FileMetadata, path: &std::path::Path) {
    if let Some(bytes) = crate::sidecar::read(path) {
        let side = read(&bytes);
        meta.rating = RatingInfo {
            value: side.rating.value,
            has_exif_rating: false,
            has_ms_photo_rating: false,
        };
        meta.label = side.label;
        meta.description = side.description;
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
    let description = read_description(bytes, xmp.as_deref());

    FileMetadata {
        rating: RatingInfo {
            value,
            has_exif_rating: exif_rating.is_some(),
            has_ms_photo_rating: xmp.is_some_and(|x| x.contains("MicrosoftPhoto:Rating")),
        },
        label,
        orientation,
        camera,
        description,
    }
}

/// XMP first (what current programs write), IPTC IIM for older files. Each part on its own:
/// a file may have XMP keywords but only an IPTC caption.
fn read_description(bytes: &[u8], xmp: Option<&str>) -> Description {
    let iim = iptc_iim(bytes);
    let comment = xmp
        .and_then(|x| xmp_alt_default(x, "dc:description"))
        .or_else(|| iim.as_ref().and_then(|i| i.caption.clone()))
        .unwrap_or_default();
    let keywords = xmp
        .and_then(|x| xmp_bag(x, "dc:subject"))
        .or_else(|| iim.map(|i| i.keywords).filter(|k| !k.is_empty()))
        .unwrap_or_default();
    Description {
        comment: comment.trim().to_owned(),
        keywords,
    }
}

/// The `rdf:li` texts of an array property (`<dc:subject><rdf:Bag><rdf:li>…`).
fn xmp_items(xmp: &str, property: &str) -> Option<Vec<(String, String)>> {
    let open = format!("<{property}");
    // `<dc:subjectCode` is another property: look on after it.
    let mut search = xmp;
    let rest = loop {
        let start = search.find(&open)?;
        let rest = &search[start + open.len()..];
        if rest.starts_with(['>', ' ', '\t', '\r', '\n', '/']) {
            break rest;
        }
        search = rest;
    };
    let body = &rest[..rest.find(&format!("</{property}>")).unwrap_or(0)];
    let mut items = Vec::new();
    let mut tail = body;
    while let Some(pos) = tail.find("<rdf:li") {
        tail = &tail[pos + "<rdf:li".len()..];
        let Some(end_of_tag) = tail.find('>') else {
            break;
        };
        let attributes = tail[..end_of_tag].to_owned();
        if attributes.ends_with('/') {
            tail = &tail[end_of_tag + 1..];
            continue;
        }
        tail = &tail[end_of_tag + 1..];
        let Some(close) = tail.find("</rdf:li>") else {
            break;
        };
        items.push((attributes, xml_unescape(&tail[..close])));
        tail = &tail[close..];
    }
    Some(items)
}

/// Keywords: every item of the bag, trimmed, empty ones left out.
fn xmp_bag(xmp: &str, property: &str) -> Option<Vec<String>> {
    let items = xmp_items(xmp, property)?;
    Some(
        items
            .into_iter()
            .map(|(_, text)| text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .collect(),
    )
}

/// A language alternative: the `x-default` entry, else the first one.
fn xmp_alt_default(xmp: &str, property: &str) -> Option<String> {
    let items = xmp_items(xmp, property)?;
    items
        .iter()
        .find(|(attributes, _)| attributes.contains("x-default"))
        .or(items.first())
        .map(|(_, text)| text.clone())
}

fn xml_unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.find(';').filter(|&s| s <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Only what the IPTC IIM block says – for tests that check both copies were written.
#[cfg(test)]
pub fn iptc_description(bytes: &[u8]) -> Description {
    iptc_iim(bytes)
        .map(|iim| Description {
            comment: iim.caption.unwrap_or_default(),
            keywords: iim.keywords,
        })
        .unwrap_or_default()
}

/// IPTC IIM from a JPEG's Photoshop block (APP13, resource 0x0404).
#[derive(Debug, Default, PartialEq)]
struct Iim {
    caption: Option<String>,
    keywords: Vec<String>,
}

fn iptc_iim(bytes: &[u8]) -> Option<Iim> {
    const PHOTOSHOP: &[u8] = b"Photoshop 3.0\0";
    let search = &bytes[..bytes.len().min(512 * 1024)];
    let mut pos = memmem::find(search, PHOTOSHOP)? + PHOTOSHOP.len();
    let be16 = |at: usize| Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let be32 = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    while bytes.get(pos..pos + 4) == Some(b"8BIM") {
        let id = be16(pos + 4)?;
        // Pascal name, padded to an even length together with its length byte.
        let name_len = usize::from(*bytes.get(pos + 6)?);
        let size_at = pos + 6 + (name_len + 2) / 2 * 2;
        let size = be32(size_at)? as usize;
        let data = bytes.get(size_at + 4..size_at + 4 + size)?;
        if id == 0x0404 {
            return Some(parse_iim(data));
        }
        pos = size_at + 4 + size + size % 2;
    }
    None
}

fn parse_iim(data: &[u8]) -> Iim {
    let mut utf8 = false;
    let mut caption = None;
    let mut keywords = Vec::new();
    let mut i = 0;
    while i + 5 <= data.len() && data[i] == 0x1C {
        let (record, dataset) = (data[i + 1], data[i + 2]);
        let size = usize::from(u16::from_be_bytes([data[i + 3], data[i + 4]]));
        // Extended sizes (high bit) only occur for huge binary data; stop there.
        if size & 0x8000 != 0 {
            break;
        }
        let Some(value) = data.get(i + 5..i + 5 + size) else {
            break;
        };
        match (record, dataset) {
            (1, 90) => utf8 = value == [0x1B, 0x25, 0x47],
            (2, 25) => keywords.push(value.to_vec()),
            (2, 120) => caption = Some(value.to_vec()),
            _ => {}
        }
        i += 5 + size;
    }
    // Without the UTF-8 marker, text that is valid UTF-8 still is (many programs leave the
    // marker out); anything else is taken as Latin-1.
    let text = |raw: &[u8]| -> String {
        let decoded = if utf8 {
            String::from_utf8_lossy(raw).into_owned()
        } else {
            std::str::from_utf8(raw)
                .map(str::to_owned)
                .unwrap_or_else(|_| raw.iter().map(|&b| char::from(b)).collect())
        };
        decoded.trim_matches(['\0', ' ']).trim().to_owned()
    };
    Iim {
        caption: caption.map(|raw| text(&raw)).filter(|c| !c.is_empty()),
        keywords: keywords
            .iter()
            .map(|raw| text(raw))
            .filter(|k| !k.is_empty())
            .collect(),
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

    let model = text(Tag::Model).or_else(|| text(Tag::Make));
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
        model,
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

/// The position in OpenStreetMap, with a marker, at street level.
pub fn osm_url((lat, lon): (f64, f64)) -> String {
    format!("https://www.openstreetmap.org/?mlat={lat:.6}&mlon={lon:.6}#map=16/{lat:.6}/{lon:.6}")
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
fn civil_from_days(days: i64) -> (i64, u32, u32) {
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
        assert_eq!(info.model.as_deref(), Some("NIKON Z 6_2"));
    }

    /// A drone writes "DJI" + "FC7503", but some of its files only the model: still one camera.
    #[test]
    fn the_model_tells_cameras_apart() {
        let with_make = camera_info(&exif_from(&[
            field(Tag::Make, Value::Ascii(vec![b"DJI".to_vec()])),
            field(Tag::Model, Value::Ascii(vec![b"FC7503".to_vec()])),
        ]));
        let model_only = camera_info(&exif_from(&[field(
            Tag::Model,
            Value::Ascii(vec![b"FC7503".to_vec()]),
        )]));
        assert_eq!(with_make.camera.as_deref(), Some("DJI FC7503"));
        let id = |info: &CameraInfo| info.model.as_deref().map(camera_id);
        assert_eq!(id(&with_make), id(&model_only));
        assert_eq!(camera_id("Pixel 7a"), camera_id(" pixel 7A "));
        assert_ne!(camera_id("Pixel 7a"), camera_id("moto g42"));
        // Without a model the make stands in.
        let make_only = camera_info(&exif_from(&[field(
            Tag::Make,
            Value::Ascii(vec![b"Ricoh".to_vec()]),
        )]));
        assert_eq!(make_only.model.as_deref(), Some("Ricoh"));
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
        assert_eq!(
            osm_url((lat, lon)),
            "https://www.openstreetmap.org/?mlat=48.521600&mlon=-9.057600\
             #map=16/48.521600/-9.057600"
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
    fn xmp_comment_and_keywords() {
        let xmp = r#"<x:xmpmeta><rdf:Description>
            <dc:subjectCode>not the keywords</dc:subjectCode>
            <dc:description><rdf:Alt>
              <rdf:li xml:lang="de">Hallo</rdf:li>
              <rdf:li xml:lang="x-default">Erste Zeile&#xA;Zweite &amp; mehr</rdf:li>
            </rdf:Alt></dc:description>
            <dc:subject><rdf:Bag>
              <rdf:li>Straße</rdf:li><rdf:li> Äpfel </rdf:li><rdf:li></rdf:li><rdf:li/>
            </rdf:Bag></dc:subject>
            </rdf:Description></x:xmpmeta>"#;
        let d = read_description(b"", Some(xmp));
        assert_eq!(d.comment, "Erste Zeile\nZweite & mehr");
        assert_eq!(d.keywords, ["Straße", "Äpfel"]);
        // Only one language: that one.
        let single = r#"<dc:description><rdf:Alt><rdf:li xml:lang="en">Only</rdf:li></rdf:Alt></dc:description>"#;
        assert_eq!(read_description(b"", Some(single)).comment, "Only");
        assert_eq!(read_description(b"", None), Description::default());
        assert_eq!(
            xml_unescape("a &lt;b&gt; &#252; &unknown; & c"),
            "a <b> ü &unknown; & c"
        );
    }

    /// A Photoshop APP13 block with IPTC datasets, as old cameras and programs write it.
    fn app13(datasets: &[(u8, u8, &[u8])]) -> Vec<u8> {
        let mut iim = Vec::new();
        for (record, dataset, value) in datasets {
            iim.extend([0x1C, *record, *dataset]);
            iim.extend((value.len() as u16).to_be_bytes());
            iim.extend(*value);
        }
        let mut out = b"\xFF\xD8\xFF\xED\0\0Photoshop 3.0\0".to_vec();
        // An unrelated resource first, with an odd size (padded) and a one-letter name.
        out.extend(b"8BIM\x04\x0C\x01x");
        out.extend(3u32.to_be_bytes());
        out.extend(b"abc\0");
        out.extend(b"8BIM\x04\x04\0\0");
        out.extend((iim.len() as u32).to_be_bytes());
        out.extend(iim);
        out
    }

    #[test]
    fn iptc_iim_in_utf8_and_latin1() {
        let utf8 = app13(&[
            (1, 90, b"\x1B%G"),
            (2, 25, "Straße".as_bytes()),
            (2, 25, "Äpfel".as_bytes()),
            (2, 120, "Grüße aus Köln".as_bytes()),
        ]);
        let d = read_description(&utf8, None);
        assert_eq!(d.keywords, ["Straße", "Äpfel"]);
        assert_eq!(d.comment, "Grüße aus Köln");

        let latin1 = app13(&[(2, 25, b"Stra\xDFe"), (2, 120, b"Gr\xFC\xDFe")]);
        let d = read_description(&latin1, None);
        assert_eq!(d.keywords, ["Straße"]);
        assert_eq!(d.comment, "Grüße");

        // XMP wins where it has a value; IPTC fills the rest.
        let xmp = "<dc:subject><rdf:Bag><rdf:li>neu</rdf:li></rdf:Bag></dc:subject>";
        let d = read_description(&utf8, Some(xmp));
        assert_eq!(d.keywords, ["neu"]);
        assert_eq!(d.comment, "Grüße aus Köln");
    }

    #[test]
    fn keywords_are_added_once() {
        let mut d = Description::default();
        assert!(d.add_keyword(" Urlaub "));
        assert!(!d.add_keyword("urlaub"));
        assert!(!d.add_keyword("  "));
        assert!(d.add_keyword("Strand"));
        assert_eq!(d.keywords, ["Urlaub", "Strand"]);
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

    /// `format_millis` reads back what `capture_millis` made, across a leap day, a year's end
    /// and before 1970.
    #[test]
    fn millis_format_back_to_the_capture_time() {
        for date in [
            "2026:08:24 14:04:59",
            "2024:02:29 00:00:00",
            "1999:12:31 23:59:30",
            "1969:07:20 20:17:40",
        ] {
            let ms = capture_millis(Some(date.into()), None).expect(date);
            let expected = format!(
                "{}-{}-{} {}",
                &date[0..4],
                &date[5..7],
                &date[8..10],
                &date[11..16]
            );
            assert_eq!(format_millis(ms), expected);
        }
        let ms = capture_millis(Some("2026:01:01 00:30:00".into()), None).unwrap();
        assert_eq!(format_millis(ms - 3_600_000), "2025-12-31 23:30");
    }
}
