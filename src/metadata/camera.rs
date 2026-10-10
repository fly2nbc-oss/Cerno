//! Camera, lens, exposure and GPS position from the EXIF data.

use super::*;

pub(super) fn camera_info(exif: &Exif) -> CameraInfo {
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
pub(super) fn gps(exif: &Exif) -> Option<(f64, f64)> {
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

/// `1/250 s`, `0.3 s`, `2 s`, `30 s`
pub(super) fn format_exposure(seconds: f64) -> String {
    if seconds < 0.25 {
        format!("1/{} s", (1.0 / seconds).round())
    } else {
        format!("{} s", trim_decimal(seconds, 1))
    }
}

pub(super) fn trim_decimal(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}
