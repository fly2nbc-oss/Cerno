//! Focus measure: variance of the Laplacian, per tile.
//!
//! Only the sharpest tiles count, so a deliberately blurred background (bokeh) doesn't pull a
//! sharp subject down. The raw value depends on content and contrast; the UI shows it as a
//! percentile within the folder, which is what matters when culling a series.

/// Stored with every value; bump when the algorithm or its input size changes.
pub const VERSION: i64 = 1;

const GRID: usize = 8;
const TOP_TILES: usize = 3;

pub fn measure(rgb: &[u8], width: u32, height: u32) -> f32 {
    let (w, h) = (width as usize, height as usize);
    if w < 3 || h < 3 || rgb.len() < w * h * 3 {
        return 0.0;
    }
    let luma: Vec<f32> = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .map(|[r, g, b]| 0.299 * f32::from(*r) + 0.587 * f32::from(*g) + 0.114 * f32::from(*b))
        .collect();

    let mut tiles = Vec::with_capacity(GRID * GRID);
    for ty in 0..GRID {
        for tx in 0..GRID {
            let (x0, x1) = ((tx * w / GRID).max(1), ((tx + 1) * w / GRID).min(w - 1));
            let (y0, y1) = ((ty * h / GRID).max(1), ((ty + 1) * h / GRID).min(h - 1));
            let (mut sum, mut sum_sq, mut n) = (0.0f64, 0.0f64, 0usize);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * w + x;
                    let laplacian =
                        luma[i - 1] + luma[i + 1] + luma[i - w] + luma[i + w] - 4.0 * luma[i];
                    sum += f64::from(laplacian);
                    sum_sq += f64::from(laplacian * laplacian);
                    n += 1;
                }
            }
            if n > 0 {
                let mean = sum / n as f64;
                tiles.push((sum_sq / n as f64 - mean * mean) as f32);
            }
        }
    }
    tiles.sort_by(|a, b| b.total_cmp(a));
    let top = &tiles[..TOP_TILES.min(tiles.len())];
    top.iter().sum::<f32>() / top.len().max(1) as f32
}

/// Share of `sorted` values below `value`, 0.0..=1.0.
pub fn percentile(value: f32, sorted: &[f32]) -> f32 {
    if sorted.len() < 2 {
        return 1.0;
    }
    let below = sorted.partition_point(|v| *v < value);
    below as f32 / (sorted.len() - 1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard(size: usize, cell: usize) -> Vec<u8> {
        (0..size * size)
            .flat_map(|i| {
                let (x, y) = (i % size, i / size);
                let v = if (x / cell + y / cell).is_multiple_of(2) {
                    30
                } else {
                    220
                };
                [v, v, v]
            })
            .collect()
    }

    fn box_blur(rgb: &[u8], size: usize, radius: usize) -> Vec<u8> {
        let mut out = rgb.to_vec();
        for y in 0..size {
            for x in 0..size {
                let (mut sum, mut n) = (0u32, 0u32);
                for yy in y.saturating_sub(radius)..(y + radius + 1).min(size) {
                    for xx in x.saturating_sub(radius)..(x + radius + 1).min(size) {
                        sum += u32::from(rgb[(yy * size + xx) * 3]);
                        n += 1;
                    }
                }
                let v = (sum / n) as u8;
                out[(y * size + x) * 3..(y * size + x) * 3 + 3].copy_from_slice(&[v, v, v]);
            }
        }
        out
    }

    #[test]
    fn sharp_beats_blurred() {
        let sharp = checkerboard(128, 4);
        let blurred = box_blur(&sharp, 128, 2);
        let flat = vec![128u8; 128 * 128 * 3];
        let (s, b, f) = (
            measure(&sharp, 128, 128),
            measure(&blurred, 128, 128),
            measure(&flat, 128, 128),
        );
        assert!(s > b * 2.0, "sharp {s} vs blurred {b}");
        assert!(b > f, "blurred {b} vs flat {f}");
        assert_eq!(f, 0.0);
    }

    #[test]
    fn a_sharp_subject_wins_over_an_evenly_soft_frame() {
        // Blurred everywhere except one corner (subject in focus, bokeh around it).
        let sharp = checkerboard(128, 4);
        let mut subject = box_blur(&sharp, 128, 3);
        for y in 0..40 {
            let row = y * 128 * 3;
            subject[row..row + 40 * 3].copy_from_slice(&sharp[row..row + 40 * 3]);
        }
        let soft = box_blur(&sharp, 128, 1);
        assert!(measure(&subject, 128, 128) > measure(&soft, 128, 128));
    }

    #[test]
    fn percentile_within_series() {
        let sorted = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(1.0, &sorted), 0.0);
        assert_eq!(percentile(3.0, &sorted), 0.5);
        assert_eq!(percentile(5.0, &sorted), 1.0);
        assert_eq!(percentile(9.0, &[9.0]), 1.0);
    }
}
