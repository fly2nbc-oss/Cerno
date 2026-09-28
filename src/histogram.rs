//! Per-channel RGB histograms for the details panel (computed from the display decode).

pub type RgbHistogram = [[u32; 256]; 3];

/// Counts each channel 0..255 over `rgb` (packed RGB8).
pub fn compute(rgb: &[u8]) -> RgbHistogram {
    let mut hist = [[0u32; 256]; 3];
    for &[r, g, b] in rgb.as_chunks::<3>().0 {
        hist[0][r as usize] += 1;
        hist[1][g as usize] += 1;
        hist[2][b as usize] += 1;
    }
    hist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_and_black() {
        let white = compute(&[255, 255, 255]);
        assert_eq!(white[0][255], 1);
        let black = compute(&[0, 0, 0]);
        assert_eq!(black[2][0], 1);
    }

    #[test]
    fn single_channel() {
        let red = compute(&[200, 0, 0, 200, 0, 0]);
        assert_eq!(red[0][200], 2);
        assert_eq!(red[1][0], 2);
    }
}
