//! Per-channel RGB histograms for the details panel (computed from the display decode).

pub type RgbHistogram = [[u32; 256]; 3];

/// Every how many pixels one is counted. The panel draws the curve scaled to its highest bin,
/// so a quarter of the pixels draws the same curve – at a quarter of the cost: every display
/// decode computes it, also the neighbours' (measured 2026-10-11, release: 3.4 ms for a
/// 3100 × 1600 decode, 5.8 ms at 4K).
const STEP: usize = 4;

/// Counts each channel 0..255 over every `STEP`th pixel of `rgb` (packed RGB8).
pub fn compute(rgb: &[u8]) -> RgbHistogram {
    let mut hist = [[0u32; 256]; 3];
    for &[r, g, b] in rgb.as_chunks::<3>().0.iter().step_by(STEP) {
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
        let red = compute(&[200, 0, 0].repeat(2 * STEP));
        assert_eq!(red[0][200], 2);
        assert_eq!(red[1][0], 2);
    }

    /// Every `STEP`th pixel counts, the first one always.
    #[test]
    fn every_fourth_pixel_counts() {
        let mut rgb = [10, 10, 10].repeat(3 * STEP);
        rgb[3..6].copy_from_slice(&[250, 250, 250]);
        let hist = compute(&rgb);
        assert_eq!(hist[0][10], 3);
        assert_eq!(hist[0][250], 0, "the second pixel is not counted");
    }

    #[test]
    #[ignore = "measurement: cargo test --release -- --ignored --nocapture histogram_cost"]
    fn histogram_cost() {
        for (w, h) in [(2560usize, 1440usize), (3100, 1600), (3840, 2160)] {
            let rgb: Vec<u8> = (0..w * h * 3).map(|i| (i * 7 % 251) as u8).collect();
            let mut best = std::time::Duration::MAX;
            for _ in 0..5 {
                let t = std::time::Instant::now();
                std::hint::black_box(compute(std::hint::black_box(&rgb)));
                best = best.min(t.elapsed());
            }
            println!("{w} x {h}: {best:?}");
        }
    }
}
