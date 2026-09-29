//! The check overlay over a photo (`O`): sharp edges, or blown highlights and crushed shadows.
//!
//! Computed from the decoded pixels in the loader's workers – the uploaded texture can't be
//! read back – as a transparent RGBA image of the same size, drawn over the photo.

use eframe::egui::{Color32, ColorImage};

use crate::analysis::{exposure, sharpness};
use crate::theme::tokens;

/// What the overlay marks. Not saved: Cerno starts without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Off,
    /// Edges at the sharpness the photo has at its sharpest (focus peaking).
    Sharpness,
    /// Pixels without detail: every channel blown out, or every channel crushed black – the
    /// same test as the percentages in the details panel.
    Exposure,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Off, Mode::Sharpness, Mode::Exposure];

    /// `O` steps through the modes.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Sharpness,
            Self::Sharpness => Self::Exposure,
            Self::Exposure => Self::Off,
        }
    }
}

/// Share of the photo's strongest edges that is marked.
const PEAK_SHARE: f32 = 0.02;
/// Edges weaker than this are never marked, however sharp the rest of the photo is. On the
/// author's photos (2026-09-29) the strongest 0.1 % of a blurred photo's edges stayed below 5,
/// the strongest 2.5 % of an average one reached 13–39 and of a sharp one 58–81 – at display
/// size and at full size alike.
const PEAK_MIN: f32 = 8.0;
/// Marks are widened by this many pixels on each side: the display image is drawn smaller
/// than it is, and a line of one pixel would fade away.
const PEAK_WIDEN: usize = 1;
/// How far the marks reach beyond the pixel itself: blur, Laplacian and widening.
const BORDER: usize = 2 + PEAK_WIDEN;
/// About this many pixels decide the threshold, spread evenly over the photo.
const THRESHOLD_SAMPLES: usize = 200_000;
/// Sharp edges are found this many rows at a time.
const STRIP: usize = 512;

const SHARP: Color32 = with_alpha(tokens::LABEL_PURPLE, 235);
const BLOWN: Color32 = with_alpha(tokens::STATUS_ERROR, 225);
const CRUSHED: Color32 = with_alpha(tokens::LABEL_BLUE, 225);

const fn with_alpha(colour: Color32, alpha: u8) -> Color32 {
    let [r, g, b, _] = colour.to_array();
    Color32::from_rgba_unmultiplied_const(r, g, b, alpha)
}

/// A part of the image: `[x, y, width, height]` in pixels.
pub type Region = [u32; 4];

/// The edge strength from which a pixel counts as sharp in this photo: its strongest
/// [`PEAK_SHARE`] of edges, never below [`PEAK_MIN`]. Taken from a sample, so the full
/// resolution costs no more than the display image.
pub fn sharpness_threshold(rgb: &[u8], width: u32, height: u32) -> f32 {
    let (w, h) = (width as usize, height as usize);
    if w < 5 || h < 5 {
        return f32::INFINITY;
    }
    let inner = (w - 4) * (h - 4);
    let step = ((inner as f32 / THRESHOLD_SAMPLES as f32).sqrt().floor() as usize).max(1);
    let mut samples = Vec::with_capacity(inner / (step * step) + 1);
    for y in (2..h - 2).step_by(step) {
        for x in (2..w - 2).step_by(step) {
            samples.push(edge_at(rgb, w, x, y));
        }
    }
    let rank = ((samples.len() as f32 * (1.0 - PEAK_SHARE)) as usize).min(samples.len() - 1);
    let (_, value, _) = samples.select_nth_unstable_by(rank, f32::total_cmp);
    (*value).max(PEAK_MIN)
}

/// The overlay for `region` of an image of `width` × `height` pixels. `threshold` comes from
/// [`sharpness_threshold`] of the whole image, so tiles of one photo mark alike. `None` for
/// [`Mode::Off`].
pub fn render(
    rgb: &[u8],
    width: u32,
    height: u32,
    region: Region,
    mode: Mode,
    threshold: f32,
) -> Option<ColorImage> {
    let [x0, y0, rw, rh] = region.map(|v| v as usize);
    let marks: Vec<Color32> = match mode {
        Mode::Off => return None,
        Mode::Exposure => {
            let mut marks = Vec::with_capacity(rw * rh);
            for y in y0..y0 + rh {
                let row = &rgb[(y * width as usize + x0) * 3..(y * width as usize + x0 + rw) * 3];
                marks.extend(row.as_chunks::<3>().0.iter().map(|&[r, g, b]| {
                    if r.min(g).min(b) >= exposure::HIGHLIGHT {
                        BLOWN
                    } else if r.max(g).max(b) <= exposure::SHADOW {
                        CRUSHED
                    } else {
                        Color32::TRANSPARENT
                    }
                }));
            }
            marks
        }
        Mode::Sharpness => {
            // In strips: a 4096 px tile would otherwise need several buffers of 60 MB.
            let mut marks = Vec::with_capacity(rw * rh);
            for top in (y0..y0 + rh).step_by(STRIP) {
                let rows = STRIP.min(y0 + rh - top);
                let strip = [x0, top, rw, rows].map(|v| v as u32);
                marks.extend(
                    sharp_marks(rgb, width as usize, height as usize, strip, threshold)
                        .into_iter()
                        .map(|on| if on { SHARP } else { Color32::TRANSPARENT }),
                );
            }
            marks
        }
    };
    Some(ColorImage::new([rw, rh], marks))
}

/// The threshold `mode` needs: [`sharpness_threshold`] for sharp edges, nothing otherwise.
pub fn threshold(mode: Mode, rgb: &[u8], width: u32, height: u32) -> f32 {
    match mode {
        Mode::Sharpness => sharpness_threshold(rgb, width, height),
        Mode::Off | Mode::Exposure => 0.0,
    }
}

/// Which pixels of `region` lie on an edge at least `threshold` strong, widened by
/// [`PEAK_WIDEN`]. Works on the region plus a border, so neighbouring tiles join seamlessly.
fn sharp_marks(rgb: &[u8], w: usize, h: usize, region: Region, threshold: f32) -> Vec<bool> {
    let [x0, y0, rw, rh] = region.map(|v| v as usize);
    // The region with its border, clamped to the image.
    let (ex0, ey0) = (x0.saturating_sub(BORDER), y0.saturating_sub(BORDER));
    let (ex1, ey1) = ((x0 + rw + BORDER).min(w), (y0 + rh + BORDER).min(h));
    let (ew, eh) = (ex1 - ex0, ey1 - ey0);
    let mut part = Vec::with_capacity(ew * eh * 3);
    for y in ey0..ey1 {
        part.extend_from_slice(&rgb[(y * w + ex0) * 3..(y * w + ex1) * 3]);
    }
    let luma = sharpness::luma(&part);
    let smooth = blur(&luma, ew, eh);
    // Edges: the Laplacian of the smoothed luma, only where all four neighbours exist.
    let mut edge = vec![false; ew * eh];
    for y in 1..eh.saturating_sub(1) {
        for x in 1..ew - 1 {
            let i = y * ew + x;
            let laplace =
                smooth[i - 1] + smooth[i + 1] + smooth[i - ew] + smooth[i + ew] - 4.0 * smooth[i];
            edge[i] = laplace.abs() >= threshold;
        }
    }
    let wide = widen(&edge, ew, eh, PEAK_WIDEN);
    let mut marks = Vec::with_capacity(rw * rh);
    for y in y0 - ey0..y0 - ey0 + rh {
        marks.extend_from_slice(&wide[y * ew + (x0 - ex0)..y * ew + (x0 - ex0) + rw]);
    }
    marks
}

/// `[1 2 1] / 4` in both directions: keeps the grain of a noisy photo from counting as edges.
/// Edge pixels repeat outwards.
fn blur(values: &[f32], w: usize, h: usize) -> Vec<f32> {
    let at = |i: usize, len: usize, d: isize| (i as isize + d).clamp(0, len as isize - 1) as usize;
    let mut rows = vec![0.0; values.len()];
    for y in 0..h {
        let row = &values[y * w..(y + 1) * w];
        for x in 0..w {
            rows[y * w + x] = (row[at(x, w, -1)] + 2.0 * row[x] + row[at(x, w, 1)]) / 4.0;
        }
    }
    let mut out = vec![0.0; values.len()];
    for y in 0..h {
        let (up, down) = (at(y, h, -1), at(y, h, 1));
        for x in 0..w {
            out[y * w + x] = (rows[up * w + x] + 2.0 * rows[y * w + x] + rows[down * w + x]) / 4.0;
        }
    }
    out
}

/// Every mark grows by `radius` pixels in each direction (a square).
fn widen(marks: &[bool], w: usize, h: usize, radius: usize) -> Vec<bool> {
    let mut rows = vec![false; marks.len()];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(radius), (x + radius).min(w - 1));
            rows[y * w + x] = marks[y * w + a..=y * w + b].iter().any(|&m| m);
        }
    }
    let mut out = vec![false; marks.len()];
    for y in 0..h {
        let (a, b) = (y.saturating_sub(radius), (y + radius).min(h - 1));
        for x in 0..w {
            out[y * w + x] = (a..=b).any(|yy| rows[yy * w + x]);
        }
    }
    out
}

/// The edge strength at one pixel, the same value [`sharp_marks`] compares: blur and Laplacian
/// in one 5 × 5 step. `x` and `y` are at least two pixels from the border.
fn edge_at(rgb: &[u8], w: usize, x: usize, y: usize) -> f32 {
    let luma = |x: usize, y: usize| {
        let i = (y * w + x) * 3;
        0.299 * rgb[i] as f32 + 0.587 * rgb[i + 1] as f32 + 0.114 * rgb[i + 2] as f32
    };
    let smooth = |x: usize, y: usize| {
        let mut sum = 0.0;
        for (dy, wy) in [(-1isize, 1.0), (0, 2.0), (1, 1.0)] {
            for (dx, wx) in [(-1isize, 1.0), (0, 2.0), (1, 1.0)] {
                sum += wx * wy * luma((x as isize + dx) as usize, (y as isize + dy) as usize);
            }
        }
        sum / 16.0
    };
    (smooth(x - 1, y) + smooth(x + 1, y) + smooth(x, y - 1) + smooth(x, y + 1) - 4.0 * smooth(x, y))
        .abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grey image with `paint` deciding each pixel.
    fn image(w: u32, h: u32, paint: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .flat_map(|(x, y)| paint(x, y))
            .collect()
    }

    fn marked(image: &ColorImage) -> Vec<bool> {
        image.pixels.iter().map(|p| p.a() > 0).collect()
    }

    fn whole(w: u32, h: u32) -> Region {
        [0, 0, w, h]
    }

    #[test]
    fn modes_cycle_back_to_off() {
        assert_eq!(Mode::Off.next(), Mode::Sharpness);
        assert_eq!(Mode::Sharpness.next(), Mode::Exposure);
        assert_eq!(Mode::Exposure.next(), Mode::Off);
        assert!(render(&[0; 3], 1, 1, whole(1, 1), Mode::Off, 0.0).is_none());
    }

    #[test]
    fn exposure_marks_blown_and_crushed_pixels_only() {
        let (w, h) = (30, 10);
        let rgb = image(w, h, |x, _| match x {
            0..10 => [255, 252, 250],
            10..20 => [120, 120, 120],
            // Saturated blue sky: one channel at 255 is not blown.
            20..25 => [40, 90, 255],
            _ => [2, 1, 0],
        });
        let out = render(&rgb, w, h, whole(w, h), Mode::Exposure, 0.0).unwrap();
        for (i, pixel) in out.pixels.iter().enumerate() {
            let x = i as u32 % w;
            let expected = match x {
                0..10 => BLOWN,
                25.. => CRUSHED,
                _ => Color32::TRANSPARENT,
            };
            assert_eq!(*pixel, expected, "pixel at x = {x}");
        }
    }

    #[test]
    fn a_hard_edge_is_marked_along_the_edge() {
        let (w, h) = (40, 20);
        let rgb = image(w, h, |x, _| if x < 20 { [20; 3] } else { [230; 3] });
        let threshold = sharpness_threshold(&rgb, w, h);
        let out = marked(&render(&rgb, w, h, whole(w, h), Mode::Sharpness, threshold).unwrap());
        for y in 2..h - 2 {
            let row = &out[(y * w) as usize..((y + 1) * w) as usize];
            assert!(row[18..22].iter().any(|&m| m), "edge marked in row {y}");
            assert!(row[..14].iter().all(|&m| !m), "flat left side stays clear");
            assert!(row[26..].iter().all(|&m| !m), "flat right side stays clear");
        }
    }

    #[test]
    fn a_blurred_photo_shows_no_marks() {
        // A soft gradient: its strongest "edges" are far below the minimum.
        let (w, h) = (64, 64);
        let rgb = image(w, h, |x, y| {
            let v = (128.0 + 60.0 * ((x as f32 / 9.0).sin() + (y as f32 / 11.0).cos())) as u8;
            [v; 3]
        });
        let threshold = sharpness_threshold(&rgb, w, h);
        assert_eq!(threshold, PEAK_MIN);
        let out = marked(&render(&rgb, w, h, whole(w, h), Mode::Sharpness, threshold).unwrap());
        assert!(out.iter().all(|&m| !m));
    }

    /// Full resolution is rendered tile by tile: the tiles must join into the same marks as
    /// one piece.
    #[test]
    fn tiles_mark_the_same_as_the_whole_image() {
        let (w, h) = (50, 36);
        let rgb = image(w, h, |x, y| {
            if (x / 7 + y / 5) % 2 == 0 {
                [30; 3]
            } else {
                [210; 3]
            }
        });
        let threshold = sharpness_threshold(&rgb, w, h);
        let full = marked(&render(&rgb, w, h, whole(w, h), Mode::Sharpness, threshold).unwrap());
        for region in [
            [0, 0, 23, 17],
            [23, 0, 27, 17],
            [0, 17, 23, 19],
            [23, 17, 27, 19],
        ] {
            let [x0, y0, rw, rh] = region;
            let tile = marked(&render(&rgb, w, h, region, Mode::Sharpness, threshold).unwrap());
            for y in 0..rh {
                for x in 0..rw {
                    assert_eq!(
                        tile[(y * rw + x) as usize],
                        full[((y0 + y) * w + x0 + x) as usize],
                        "pixel ({}, {}) of tile {region:?}",
                        x0 + x,
                        y0 + y
                    );
                }
            }
        }
    }

    /// Sharp edges are found in strips of rows: the strips join like the tiles.
    #[test]
    fn strips_join_seamlessly() {
        let (w, h) = (12, (STRIP * 2 + 77) as u32);
        let rgb = image(w, h, |x, y| {
            if (x / 3 + y / 7) % 2 == 0 {
                [40; 3]
            } else {
                [200; 3]
            }
        });
        let threshold = sharpness_threshold(&rgb, w, h);
        let stripped =
            marked(&render(&rgb, w, h, whole(w, h), Mode::Sharpness, threshold).unwrap());
        let at_once = sharp_marks(&rgb, w as usize, h as usize, whole(w, h), threshold);
        assert_eq!(stripped, at_once);
        assert!(at_once.iter().any(|&m| m));
    }

    /// The threshold samples the same edge strength that the marks compare.
    #[test]
    fn the_sampled_edge_matches_the_marks() {
        let (w, h) = (12, 12);
        let rgb = image(w, h, |x, y| [(x * 20 + y * 3) as u8, (y * 17) as u8, 90]);
        let luma = sharpness::luma(&rgb);
        let smooth = blur(&luma, w as usize, h as usize);
        let (x, y, ew) = (5usize, 6usize, w as usize);
        let i = y * ew + x;
        let laplace = (smooth[i - 1] + smooth[i + 1] + smooth[i - ew] + smooth[i + ew]
            - 4.0 * smooth[i])
            .abs();
        assert!((edge_at(&rgb, ew, x, y) - laplace).abs() < 1e-3);
    }
}
