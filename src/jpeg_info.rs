//! What a JPEG's header says about its compression, for Details › File: the quality setting it
//! was most likely saved with, estimated from the luminance quantisation table the way ExifTool
//! and IrfanView do (the standard IJG tables, scaled), and the chroma subsampling. Neither is
//! stored in the file as such. Only the markers before the image data are read for them.
//!
//! And whether the file is whole (`is_complete`): one cut off while it was copied or downloaded
//! ends inside the image data, and the decoder fills the rest grey.

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
    /// The image data reaches its end (`is_complete`).
    pub complete: bool,
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
        complete: is_complete(bytes),
    })
}

/// Whether the image data reaches its end marker (EOI, `FF D9`). The segments are skipped by
/// their length up to the first scan – an EXIF thumbnail ends with `FF D9` too, inside its
/// APP1 segment – then the entropy-coded data is read for markers: `FF 00` is a data byte,
/// `FF D0`–`FF D7` restart markers and `FF FF` fill; any other marker is a segment again (the
/// tables and the next scan of a progressive JPEG). What follows the EOI – the video of a
/// motion photo, a maker's trailer, more pictures – does not matter. A file that is no JPEG,
/// or that has bytes where a marker belongs, counts as complete: nothing is said that isn't
/// known.
pub fn is_complete(bytes: &[u8]) -> bool {
    if bytes.get(..2) != Some(&[0xFF, 0xD8][..]) {
        return true;
    }
    let mut at = 2;
    let mut in_scan = false;
    loop {
        if in_scan {
            let Some(found) = bytes[at..].iter().position(|byte| *byte == 0xFF) else {
                return false;
            };
            at += found;
            match bytes.get(at + 1) {
                None => return false,
                Some(0x00 | 0xD0..=0xD7) => {
                    at += 2;
                    continue;
                }
                Some(0xFF) => {
                    at += 1;
                    continue;
                }
                Some(_) => in_scan = false,
            }
        }
        let (Some(&prefix), Some(&marker)) = (bytes.get(at), bytes.get(at + 1)) else {
            return false;
        };
        if prefix != 0xFF {
            return true;
        }
        match marker {
            0xD9 => return true,
            0xFF => at += 1,
            0x01 | 0xD0..=0xD7 => at += 2,
            _ => {
                let Some(length) = bytes.get(at + 2..at + 4) else {
                    return false;
                };
                let end = at + 2 + usize::from(u16::from_be_bytes([length[0], length[1]]));
                if end > bytes.len() {
                    return false;
                }
                in_scan = marker == 0xDA;
                at = end;
            }
        }
    }
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
        assert!(info.complete);
    }

    /// A larger, noisy picture, so the image data is most of the file.
    fn picture(setup: impl FnOnce(&mut jpeg_encoder::Encoder<&mut Vec<u8>>)) -> Vec<u8> {
        let (w, h) = (320u16, 240u16);
        let rgb: Vec<u8> = (0..usize::from(w) * usize::from(h) * 3)
            .map(|i| (i * 7919 % 251) as u8)
            .collect();
        let mut out = Vec::new();
        let mut encoder = jpeg_encoder::Encoder::new(&mut out, 90);
        setup(&mut encoder);
        encoder
            .encode(&rgb, w, h, jpeg_encoder::ColorType::Rgb)
            .expect("encode");
        out
    }

    /// Whole files are complete in every layout; cut anywhere they are not – also when an EXIF
    /// thumbnail's own end marker comes before the cut.
    #[test]
    fn a_cut_off_jpeg_is_incomplete() {
        let with_thumbnail = |encoder: &mut jpeg_encoder::Encoder<&mut Vec<u8>>| {
            let mut exif = b"Exif\0\0".to_vec();
            exif.extend([0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x02, 0x12, 0x34, 0xFF, 0xD9]);
            encoder.add_app_segment(1, exif).expect("app1");
        };
        let files = [
            picture(|_| {}),
            picture(|encoder| encoder.set_progressive(true)),
            picture(|encoder| encoder.set_restart_interval(4)),
            picture(with_thumbnail),
        ];
        for whole in files {
            assert!(is_complete(&whole));
            for share in [0.95, 0.6, 0.1] {
                let cut = &whole[..(whole.len() as f64 * share) as usize];
                assert!(!is_complete(cut), "cut at {share}");
            }
            // Only the end marker missing.
            assert!(!is_complete(&whole[..whole.len() - 2]));
            assert!(read(&whole).is_some_and(|info| info.complete));
        }
        let fixture = std::fs::read("tests/fixtures/tiny.jpg").expect("fixture");
        assert!(!is_complete(&fixture[..fixture.len() * 9 / 10]));
    }

    /// Bytes after the end – a motion photo's video, a trailer – are no reason to warn, and a
    /// file that is no JPEG is not called incomplete.
    #[test]
    fn what_follows_the_end_does_not_count() {
        let mut motion = picture(|_| {});
        motion.extend(b"\0\0\0\x18ftypmp42 and then a video with \xFF\xD8 and \xFF bytes");
        assert!(is_complete(&motion));
        assert!(is_complete(b"\x89PNG\r\n\x1a\n"));
        assert!(is_complete(&[]));
        // Something where a marker belongs: Cerno can't tell, so it says nothing.
        assert!(is_complete(&[0xFF, 0xD8, 0x12, 0x34]));
    }
}
