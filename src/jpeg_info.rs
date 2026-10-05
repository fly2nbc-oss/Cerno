//! What a JPEG's header says about its compression, for Details › File: the quality setting it
//! was most likely saved with, estimated from the luminance quantisation table the way ExifTool
//! and IrfanView do (the standard IJG tables, scaled), and the chroma subsampling. Neither is
//! stored in the file as such. Only the markers before the image data are read.

/// The JPEG standard's luminance table (Annex K), in zig-zag order like a DQT segment.
const LUMINANCE: [u16; 64] = [
    16, 11, 12, 14, 12, 10, 16, 14, 13, 14, 18, 17, 16, 19, 24, 40, 26, 24, 22, 22, 24, 49, 35, 37,
    29, 40, 58, 51, 61, 60, 57, 51, 56, 55, 64, 72, 92, 78, 64, 68, 87, 69, 55, 56, 80, 109, 81,
    87, 95, 98, 103, 104, 103, 62, 77, 113, 121, 112, 100, 120, 92, 101, 103, 99,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpegInfo {
    /// 1–100, estimated; `None` without a luminance table.
    pub quality: Option<u8>,
    /// "4:2:0", "4:2:2", "4:4:4" …; `None` for greyscale or an unusual layout.
    pub subsampling: Option<&'static str>,
}

/// Reads the header of a JPEG file; `None` when it is none.
pub fn read(bytes: &[u8]) -> Option<JpegInfo> {
    if bytes.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut luminance: Option<[u16; 64]> = None;
    let mut subsampling = None;
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        // Fill bytes and markers without a length.
        if marker == 0xFF {
            at += 1;
            continue;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            at += 2;
            continue;
        }
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        let segment = bytes.get(at + 4..at + 2 + length)?;
        match marker {
            0xDB => {
                if let Some(table) = luminance_table(segment) {
                    luminance = Some(table);
                }
            }
            // Baseline, extended and progressive frames (not DHT 0xC4, JPG 0xC8, DAC 0xCC).
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                subsampling = sampling(segment);
            }
            // The image data starts: everything wanted comes before it.
            0xDA => break,
            _ => {}
        }
        at += 2 + length;
    }
    Some(JpegInfo {
        quality: luminance.map(|table| estimate_quality(&table)),
        subsampling,
    })
}

/// Table 0 of a DQT segment, which may hold several tables of 8- or 16-bit values.
fn luminance_table(segment: &[u8]) -> Option<[u16; 64]> {
    let mut at = 0;
    while at < segment.len() {
        let (precision, id) = (segment[at] >> 4, segment[at] & 0x0F);
        let size = if precision == 0 { 1 } else { 2 };
        let values = segment.get(at + 1..at + 1 + 64 * size)?;
        if id == 0 {
            let mut table = [0u16; 64];
            for (i, value) in table.iter_mut().enumerate() {
                *value = if size == 1 {
                    u16::from(values[i])
                } else {
                    u16::from_be_bytes([values[2 * i], values[2 * i + 1]])
                };
            }
            return Some(table);
        }
        at += 1 + 64 * size;
    }
    None
}

/// The IJG scaling the table was made with, turned back into the quality setting: `scale` is
/// `5000 / q` below 50 and `200 − 2q` from 50 on.
fn estimate_quality(table: &[u16; 64]) -> u8 {
    let ours: f64 = table.iter().map(|v| f64::from(*v)).sum();
    let standard: f64 = LUMINANCE.iter().map(|v| f64::from(*v)).sum();
    let scale = 100.0 * ours / standard;
    let quality = if scale <= 100.0 {
        (200.0 - scale) / 2.0
    } else {
        5000.0 / scale
    };
    quality.round().clamp(1.0, 100.0) as u8
}

/// The luma's sampling factors against the first chroma component's.
fn sampling(segment: &[u8]) -> Option<&'static str> {
    let components = usize::from(*segment.get(5)?);
    if components < 3 {
        return None;
    }
    let factors = |i: usize| {
        let byte = *segment.get(6 + 3 * i + 1)?;
        Some((byte >> 4, byte & 0x0F))
    };
    let ((yh, yv), (ch, cv)) = (factors(0)?, factors(1)?);
    if ch == 0 || cv == 0 {
        return None;
    }
    Some(match (yh / ch, yv / cv) {
        (1, 1) => "4:4:4",
        (2, 1) => "4:2:2",
        (2, 2) => "4:2:0",
        (1, 2) => "4:4:0",
        (4, 1) => "4:1:1",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(quality: u8, sampling: jpeg_encoder::SamplingFactor) -> Vec<u8> {
        let (w, h) = (64u16, 48u16);
        let rgb: Vec<u8> = (0..usize::from(w) * usize::from(h) * 3)
            .map(|i| (i * 7 % 251) as u8)
            .collect();
        let mut out = Vec::new();
        let mut encoder = jpeg_encoder::Encoder::new(&mut out, quality);
        encoder.set_sampling_factor(sampling);
        encoder
            .encode(&rgb, w, h, jpeg_encoder::ColorType::Rgb)
            .expect("encode");
        out
    }

    /// The estimate lands on the quality the file was saved with (the encoder uses the IJG
    /// tables), and the subsampling is read from the frame.
    #[test]
    fn quality_and_subsampling_come_from_the_header() {
        use jpeg_encoder::SamplingFactor;
        for quality in [30, 50, 75, 90, 95, 100] {
            let info = read(&encode(quality, SamplingFactor::R_4_2_0)).expect("a JPEG");
            let estimate = info.quality.expect("a table");
            assert!(
                estimate.abs_diff(quality) <= 2,
                "saved at {quality}, estimated {estimate}"
            );
            assert_eq!(info.subsampling, Some("4:2:0"));
        }
        let info = read(&encode(90, SamplingFactor::R_4_4_4)).expect("a JPEG");
        assert_eq!(info.subsampling, Some("4:4:4"));
        let info = read(&encode(90, SamplingFactor::R_4_2_2)).expect("a JPEG");
        assert_eq!(info.subsampling, Some("4:2:2"));
    }

    #[test]
    fn other_files_are_no_jpeg() {
        assert_eq!(read(b"\x89PNG\r\n"), None);
        assert_eq!(read(&[]), None);
        // Cut off in the middle of a segment: nothing is made up.
        let mut cut = encode(80, jpeg_encoder::SamplingFactor::R_4_2_0);
        cut.truncate(30);
        assert!(read(&cut).is_none_or(|info| info.quality.is_none()));
    }

    #[test]
    fn the_fixture_reads_like_a_camera_jpeg() {
        let bytes = std::fs::read("tests/fixtures/tiny.jpg").expect("fixture");
        let info = read(&bytes).expect("a JPEG");
        assert!(info.quality.is_some_and(|q| (1..=100).contains(&q)));
    }
}
