//! Rating, orientation and camera data, read from the file bytes that were loaded for
//! decoding anyway.

use std::borrow::Cow;
use std::io::Cursor;

use exif::{Exif, In, Tag, Value};
use memchr::memmem;

/// Microsoft's EXIF rating tag (IFD0 0x4746), written by Windows Explorer.
const EXIF_RATING: exif::Tag = exif::Tag(exif::Context::Tiff, 0x4746);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RatingInfo {
    /// 1..=5; `None` for unrated, `0` and `-1` ("rejected").
    pub stars: Option<u8>,
    /// Windows Explorer keeps extra copies of the rating. If they exist they are updated too,
    /// so Explorer never shows a stale value.
    pub has_exif_rating: bool,
    pub has_ms_photo_rating: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileMetadata {
    pub rating: RatingInfo,
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
    let stars = xmp_rating
        .or(exif_rating.map(|r| r as i32))
        .filter(|r| (1..=5).contains(r))
        .map(|r| r as u8);

    FileMetadata {
        rating: RatingInfo {
            stars,
            has_exif_rating: exif_rating.is_some(),
            has_ms_photo_rating: xmp.is_some_and(|x| x.contains("MicrosoftPhoto:Rating")),
        },
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
    }
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
    for name in ["xmp:Rating", "xap:Rating"] {
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
            if let Some(v) = value.and_then(|v| v.trim().parse::<f64>().ok()) {
                return Some(v.round() as i32);
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
        assert_eq!(meta.rating.stars, Some(3));
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
    fn rejected_and_unrated_are_no_stars() {
        let meta = read(br#"<x:xmpmeta><a xmp:Rating="-1"/></x:xmpmeta>"#);
        assert_eq!(meta.rating.stars, None);
        let meta = read(br#"<x:xmpmeta><a xmp:Rating="0"/></x:xmpmeta>"#);
        assert_eq!(meta.rating.stars, None);
    }
}
