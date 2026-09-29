//! Camera RAW files are shown by the JPEG preview they carry – no RAW development. Nearly every
//! format keeps one or more (a thumbnail, often a full-size preview); the largest wins.

use memchr::memmem;

/// Frame headers of baseline, extended and progressive JPEG. The lossless kinds (SOF3 …) hold
/// the raw sensor data in some formats and are no preview.
const PREVIEW_FRAMES: [u8; 3] = [0xC0, 0xC1, 0xC2];

/// The largest embedded preview, from its start marker to the end of the file (the JPEG
/// decoder stops at its own end). `None` when there is none.
pub fn preview(bytes: &[u8]) -> Option<&[u8]> {
    memmem::find_iter(bytes, &[0xFF, 0xD8, 0xFF])
        .filter_map(|start| {
            frame_size(&bytes[start..]).map(|(w, h)| (u64::from(w) * u64::from(h), start))
        })
        .max_by_key(|&(area, start)| (area, std::cmp::Reverse(start)))
        .map(|(_, start)| &bytes[start..])
}

/// Width and height from the frame header of the JPEG starting at `jpeg[0]`, walking its
/// segments; `None` for a lossless frame, a broken structure or a false start.
fn frame_size(jpeg: &[u8]) -> Option<(u16, u16)> {
    let be16 = |at: usize| Some(u16::from_be_bytes([*jpeg.get(at)?, *jpeg.get(at + 1)?]));
    let mut pos = 2;
    for _ in 0..256 {
        if *jpeg.get(pos)? != 0xFF {
            return None;
        }
        let marker = *jpeg.get(pos + 1)?;
        match marker {
            // Fill bytes.
            0xFF => {
                pos += 1;
                continue;
            }
            0xC0..=0xC2 => {
                let (h, w) = (be16(pos + 5)?, be16(pos + 7)?);
                return (w > 0 && h > 0 && PREVIEW_FRAMES.contains(&marker)).then_some((w, h));
            }
            // Any other frame type (lossless, arithmetic, hierarchical) or the scan start
            // before a frame: not a preview we can show.
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF | 0xDA | 0xD8 | 0xD9 => return None,
            // Markers without a length.
            0x01 | 0xD0..=0xD7 => pos += 2,
            _ => pos += 2 + usize::from(be16(pos + 2)?),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A JPEG header down to its frame header: SOI, an APP1 segment, SOFn.
    fn header(sof: u8, w: u16, h: u16) -> Vec<u8> {
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x04, b'x', b'y'];
        jpeg.extend([0xFF, sof, 0x00, 0x11, 0x08]);
        jpeg.extend(h.to_be_bytes());
        jpeg.extend(w.to_be_bytes());
        jpeg.extend([0x03, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        jpeg
    }

    #[test]
    fn the_largest_preview_wins_and_raw_data_is_skipped() {
        let mut file = b"II*\0 some tiff header".to_vec();
        let thumb = file.len();
        file.extend(header(0xC0, 160, 120));
        file.extend([0u8; 40]);
        // Lossless sensor data, bigger than anything else.
        file.extend(header(0xC3, 6000, 4000));
        file.extend([0x12, 0xFF, 0xD8, 0xFF, 0x00]); // a false start inside data
        let big = file.len();
        file.extend(header(0xC2, 6000, 4000));
        file.extend(header(0xC0, 1616, 1080));

        let found = preview(&file).expect("a preview");
        assert_eq!(found.as_ptr(), file[big..].as_ptr());
        assert_eq!(frame_size(&file[thumb..]), Some((160, 120)));
        assert_eq!(preview(b"no jpeg here"), None);
        assert_eq!(preview(&header(0xC3, 10, 10)), None, "lossless only");
    }

    #[test]
    fn the_real_fixture_is_its_own_preview() {
        let jpeg = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/tiny.jpg"
        ))
        .expect("fixture");
        let found = preview(&jpeg).expect("a preview");
        assert_eq!(found.as_ptr(), jpeg.as_ptr());
    }
}
