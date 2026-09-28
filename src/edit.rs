//! Straighten and crop, as pure geometry plus the pixel work that writes a new JPEG.
//!
//! A quarter turn only changes the EXIF orientation (see the rating writer). A fine angle
//! keeps the original pixel size: the photo is rotated and scaled just enough that the
//! original frame stays full, so nothing further has to be cropped. A crop copies a
//! rectangle of a chosen aspect ratio.

use std::path::Path;

use anyhow::{Context as _, Result, bail};

use crate::decode;
use crate::library::Format;
use crate::metadata;

/// One wheel notch or arrow press, in radians (0.1°).
pub const FINE_STEP: f64 = 0.1 * std::f64::consts::PI / 180.0;
/// The same with Shift held (0.01°).
pub const FINER_STEP: f64 = 0.01 * std::f64::consts::PI / 180.0;
/// Fine rotation stays inside this range; quarter turns are a separate, lossless step.
pub const MAX_ANGLE: f64 = 45.0 * std::f64::consts::PI / 180.0;
/// Shortest side of a crop that is written. Smaller frames are refused.
pub const MIN_CROP_SIDE: u32 = 32;
const JPEG_QUALITY: u8 = 95;

/// One clockwise quarter turn from each EXIF orientation (index 1..=8).
const CLOCKWISE: [u16; 9] = [0, 6, 7, 8, 5, 2, 3, 4, 1];

/// `radians` clamped to ±45°.
pub fn clamp_angle(radians: f64) -> f64 {
    radians.clamp(-MAX_ANGLE, MAX_ANGLE)
}

/// Smallest scale ≥ 1 that lets a `width`×`height` photo rotated by `radians` still cover
/// a frame of that same size. At 0° it is 1; a square at 45° needs √2.
pub fn cover_scale(width: u32, height: u32, radians: f64) -> f64 {
    let (sin, cos) = radians.sin_cos();
    let (sin, cos) = (sin.abs(), cos.abs());
    let (w, h) = (f64::from(width), f64::from(height));
    let cover_width = (w * cos + h * sin) / w;
    let cover_height = (h * cos + w * sin) / h;
    cover_width.max(cover_height).max(1.0)
}

/// EXIF orientation after `quarters` clockwise 90° turns (`-1` is counter-clockwise).
/// Values outside 1..=8 are left as they are.
pub fn rotate_orientation(orientation: u16, quarters: i32) -> u16 {
    if !(1..=8).contains(&orientation) {
        return orientation;
    }
    let steps = quarters.rem_euclid(4) as u16;
    (0..steps).fold(orientation, |current, _| CLOCKWISE[current as usize])
}

/// The frame's proportions. `Original` is the photo's own ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ratio {
    Original,
    ThreeTwo,
    FourThree,
    SixteenNine,
    Square,
}

impl Ratio {
    pub const ALL: [Ratio; 5] = [
        Self::Original,
        Self::ThreeTwo,
        Self::FourThree,
        Self::SixteenNine,
        Self::Square,
    ];

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|ratio| *ratio == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// Width / height of the crop. `landscape` picks the wide orientation of `ratio`.
pub fn ratio_aspect(ratio: Ratio, image_w: u32, image_h: u32, landscape: bool) -> f64 {
    let long_over_short = match ratio {
        Ratio::Original => {
            let (w, h) = (f64::from(image_w), f64::from(image_h));
            w.max(h) / w.min(h).max(1.0)
        }
        Ratio::ThreeTwo => 3.0 / 2.0,
        Ratio::FourThree => 4.0 / 3.0,
        Ratio::SixteenNine => 16.0 / 9.0,
        Ratio::Square => 1.0,
    };
    if landscape {
        long_over_short
    } else {
        1.0 / long_over_short
    }
}

/// A crop in image pixels. Origin is the top left; `w` / `h` is the aspect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Crop {
    /// Largest rectangle of `aspect` (width / height) that fits, centred.
    pub fn max_centered(image_w: f64, image_h: f64, aspect: f64) -> Self {
        let aspect = aspect.max(1e-6);
        let (w, h) = if image_w / image_h >= aspect {
            (image_h * aspect, image_h)
        } else {
            (image_w, image_w / aspect)
        };
        Self {
            x: (image_w - w) / 2.0,
            y: (image_h - h) / 2.0,
            w,
            h,
        }
    }

    /// Slides the rectangle, keeping it inside the image.
    pub fn translate(self, dx: f64, dy: f64, image_w: f64, image_h: f64) -> Self {
        Self {
            x: (self.x + dx).clamp(0.0, (image_w - self.w).max(0.0)),
            y: (self.y + dy).clamp(0.0, (image_h - self.h).max(0.0)),
            ..self
        }
    }

    /// The whole photo, within half a pixel.
    pub fn covers_image(self, image_w: f64, image_h: f64) -> bool {
        self.w >= image_w - 0.5 && self.h >= image_h - 0.5
    }
}

/// Which corner is being dragged. The opposite corner stays put.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    Nw,
    Ne,
    Sw,
    Se,
}

impl Corner {
    pub fn anchor(self, crop: Crop) -> (f64, f64) {
        match self {
            Self::Nw => (crop.x + crop.w, crop.y + crop.h),
            Self::Ne => (crop.x, crop.y + crop.h),
            Self::Sw => (crop.x + crop.w, crop.y),
            Self::Se => (crop.x, crop.y),
        }
    }
}

/// Grows a rectangle from a fixed anchor toward `pointer`, keeping `aspect` (width / height)
/// and staying inside the image. `min_side` is the shortest side, in pixels.
pub fn resize_from_anchor(
    anchor: (f64, f64),
    pointer: (f64, f64),
    aspect: f64,
    image: (f64, f64),
    min_side: f64,
) -> Crop {
    let aspect = aspect.max(1e-6);
    let (ax, ay) = anchor;
    let (px, py) = pointer;
    let sign_x = if px >= ax { 1.0 } else { -1.0 };
    let sign_y = if py >= ay { 1.0 } else { -1.0 };
    let max_w = if sign_x > 0.0 { image.0 - ax } else { ax };
    let max_h = if sign_y > 0.0 { image.1 - ay } else { ay };

    let mut w = (px - ax).abs();
    let mut h = (py - ay).abs();
    if w < h * aspect {
        h = w / aspect;
    } else {
        w = h * aspect;
    }
    if w > max_w {
        w = max_w.max(0.0);
        h = w / aspect;
    }
    if h > max_h {
        h = max_h.max(0.0);
        w = h * aspect;
    }

    let floor = min_side.min(max_w).min(max_h * aspect).max(0.0);
    if w.min(h) < floor && floor > 0.0 {
        if aspect >= 1.0 {
            h = floor;
            w = h * aspect;
        } else {
            w = floor;
            h = w / aspect;
        }
        if w > max_w || h > max_h {
            let fit = (max_w / w).min(max_h / h).min(1.0);
            w *= fit;
            h *= fit;
        }
    }

    let x = if sign_x > 0.0 { ax } else { ax - w };
    let y = if sign_y > 0.0 { ay } else { ay - h };
    Crop { x, y, w, h }
}

/// Shortest side the interaction may shrink to: 32 px, or the whole photo when it is smaller.
pub fn min_side(image_w: u32, image_h: u32) -> f64 {
    f64::from(MIN_CROP_SIDE.min(image_w.min(image_h)))
}

/// Integer pixel rectangle inside the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl PixelRect {
    pub fn from_crop(crop: Crop, image_w: u32, image_h: u32) -> Self {
        let x = crop.x.round().clamp(0.0, f64::from(image_w)) as u32;
        let y = crop.y.round().clamp(0.0, f64::from(image_h)) as u32;
        let w = (crop.w.round() as u32)
            .min(image_w.saturating_sub(x))
            .max(1);
        let h = (crop.h.round() as u32)
            .min(image_h.saturating_sub(y))
            .max(1);
        Self { x, y, w, h }
    }

    pub fn is_entire(self, image_w: u32, image_h: u32) -> bool {
        self.x == 0 && self.y == 0 && self.w == image_w && self.h == image_h
    }
}

/// Rotates `rgb` clockwise by `radians` and scales so the original frame stays covered.
/// The output has the same size as the input.
pub fn rotate_cover(rgb: &[u8], width: u32, height: u32, radians: f64) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    if radians.abs() < 1e-12 {
        return rgb.to_vec();
    }
    let scale = cover_scale(width, height, radians);
    let (sin, cos) = radians.sin_cos();
    let cx = (f64::from(width) - 1.0) / 2.0;
    let cy = (f64::from(height) - 1.0) / 2.0;
    let mut out = vec![0u8; rgb.len()];
    for y in 0..h {
        for x in 0..w {
            let dx = f64::from(x as u32) - cx;
            let dy = f64::from(y as u32) - cy;
            let sx = (dx * cos + dy * sin) / scale + cx;
            let sy = (-dx * sin + dy * cos) / scale + cy;
            let pixel = sample_bilinear(rgb, w, h, sx, sy);
            let dest = (y * w + x) * 3;
            out[dest..dest + 3].copy_from_slice(&pixel);
        }
    }
    out
}

fn sample_bilinear(rgb: &[u8], w: usize, h: usize, x: f64, y: f64) -> [u8; 3] {
    let x = x.clamp(0.0, (w - 1) as f64);
    let y = y.clamp(0.0, (h - 1) as f64);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = (x - x0 as f64) as f32;
    let ty = (y - y0 as f64) as f32;
    let at = |x: usize, y: usize| -> [f32; 3] {
        let index = (y * w + x) * 3;
        [
            f32::from(rgb[index]),
            f32::from(rgb[index + 1]),
            f32::from(rgb[index + 2]),
        ]
    };
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    let color = mix(
        mix(at(x0, y0), at(x1, y0), tx),
        mix(at(x0, y1), at(x1, y1), tx),
        ty,
    );
    [
        color[0].round() as u8,
        color[1].round() as u8,
        color[2].round() as u8,
    ]
}

/// Copies `rect` out of an RGB image. `rect` must lie inside it.
pub fn copy_rect(rgb: &[u8], width: u32, rect: PixelRect) -> Vec<u8> {
    let mut out = Vec::with_capacity((rect.w * rect.h * 3) as usize);
    for y in rect.y..rect.y + rect.h {
        let start = ((y * width + rect.x) * 3) as usize;
        let end = start + (rect.w * 3) as usize;
        out.extend_from_slice(&rgb[start..end]);
    }
    out
}

/// JPEG at quality 95 with no chroma subsampling (4:4:4).
pub fn encode_jpeg(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>> {
    let width = u16::try_from(width).context("image is wider than a JPEG allows")?;
    let height = u16::try_from(height).context("image is taller than a JPEG allows")?;
    let expected = width as usize * height as usize * 3;
    if rgb.len() < expected {
        bail!("not enough pixels for a {width}×{height} JPEG");
    }
    let mut jpeg = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut jpeg, JPEG_QUALITY);
    encoder.set_sampling_factor(jpeg_encoder::SamplingFactor::R_4_4_4);
    encoder
        .encode(rgb, width, height, jpeg_encoder::ColorType::Rgb)
        .context("JPEG encoding failed")?;
    Ok(jpeg)
}

/// Full-resolution clockwise rotation, encoded as a new JPEG. Orientation is already applied.
pub fn render_rotation(path: &Path, radians: f64) -> Result<Vec<u8>> {
    let (width, height, rgb) = decode_jpeg(path)?;
    let turned = rotate_cover(&rgb, width, height, radians);
    encode_jpeg(width, height, &turned)
}

/// Full-resolution crop, encoded as a new JPEG.
pub fn render_crop(path: &Path, rect: PixelRect) -> Result<Vec<u8>> {
    let (width, height, rgb) = decode_jpeg(path)?;
    if rect.x + rect.w > width || rect.y + rect.h > height {
        bail!("crop extends outside the photo");
    }
    let cropped = copy_rect(&rgb, width, rect);
    encode_jpeg(rect.w, rect.h, &cropped)
}

fn decode_jpeg(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let bytes = std::fs::read(path).context("cannot read file")?;
    let meta = metadata::read(&bytes);
    let decoded =
        decode::decode_for_display(&bytes, Format::Jpeg, meta.orientation, [u32::MAX; 2])?;
    Ok((decoded.width, decoded.height, decoded.rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_scale_is_one_at_zero_and_root_two_for_a_square() {
        assert!((cover_scale(300, 200, 0.0) - 1.0).abs() < 1e-12);
        let square = cover_scale(100, 100, std::f64::consts::FRAC_PI_4);
        assert!(
            (square - std::f64::consts::SQRT_2).abs() < 1e-12,
            "{square}"
        );
    }

    #[test]
    fn cover_scale_of_a_small_angle_fills_both_edges() {
        let radians = 1.0_f64.to_radians();
        let scale = cover_scale(3, 2, radians);
        assert!(scale > 1.0 && scale < 1.03, "{scale}");
        let (sin, cos) = radians.sin_cos();
        let cover_width = (3.0 * cos + 2.0 * sin) / 3.0;
        let cover_height = (2.0 * cos + 3.0 * sin) / 2.0;
        assert!((scale - cover_width.max(cover_height)).abs() < 1e-12);
        assert!(scale >= cover_width && scale >= cover_height);
    }

    #[test]
    fn orientation_cycles_clockwise_and_back() {
        for window in [1_u16, 6, 3, 8, 1].windows(2) {
            assert_eq!(rotate_orientation(window[0], 1), window[1]);
        }
        for window in [2_u16, 7, 4, 5, 2].windows(2) {
            assert_eq!(rotate_orientation(window[0], 1), window[1]);
        }
        for orientation in 1..=8 {
            assert_eq!(rotate_orientation(orientation, 4), orientation);
            assert_eq!(
                rotate_orientation(rotate_orientation(orientation, 1), -1),
                orientation
            );
            assert_eq!(
                rotate_orientation(orientation, -1),
                rotate_orientation(orientation, 3)
            );
        }
        assert_eq!(rotate_orientation(0, 1), 0);
        assert_eq!(rotate_orientation(9, -1), 9);
    }

    #[test]
    fn crop_keeps_its_aspect_and_stays_inside() {
        let landscape =
            Crop::max_centered(200.0, 100.0, ratio_aspect(Ratio::ThreeTwo, 200, 100, true));
        assert!((landscape.w / landscape.h - 1.5).abs() < 1e-9);
        assert!(landscape.x >= 0.0 && landscape.y >= 0.0);
        assert!(landscape.x + landscape.w <= 200.0 + 1e-9);
        assert!(landscape.y + landscape.h <= 100.0 + 1e-9);
        assert!(landscape.w > landscape.h);

        let portrait =
            Crop::max_centered(200.0, 100.0, ratio_aspect(Ratio::ThreeTwo, 200, 100, false));
        assert!(portrait.h > portrait.w, "portrait swaps the sides");
        assert!((portrait.h / portrait.w - 1.5).abs() < 1e-9);

        let moved = Crop {
            x: 10.0,
            y: 10.0,
            w: 100.0,
            h: 50.0,
        }
        .translate(1_000.0, -100.0, 200.0, 100.0);
        assert_eq!(moved.x, 100.0);
        assert_eq!(moved.y, 0.0);
        assert_eq!((moved.w, moved.h), (100.0, 50.0));
    }

    #[test]
    fn resize_keeps_the_aspect_and_the_anchor() {
        let crop = resize_from_anchor((10.0, 10.0), (110.0, 80.0), 2.0, (200.0, 100.0), 32.0);
        assert!((crop.w / crop.h - 2.0).abs() < 1e-9, "{crop:?}");
        assert!((crop.x - 10.0).abs() < 1e-9 && (crop.y - 10.0).abs() < 1e-9);
        assert!(crop.w <= 100.0 + 1e-6);
        assert!(crop.x + crop.w <= 200.0 + 1e-6);
        assert!(crop.y + crop.h <= 100.0 + 1e-6);
    }

    #[test]
    fn rotation_keeps_the_centre_pixel_and_the_size() {
        let mut rgb = vec![0u8; 5 * 5 * 3];
        let centre = (2 * 5 + 2) * 3;
        rgb[centre] = 255;
        let turned = rotate_cover(&rgb, 5, 5, 0.2);
        assert_eq!(turned.len(), rgb.len());
        assert_eq!(&turned[centre..centre + 3], &[255, 0, 0]);
        assert_eq!(rotate_cover(&rgb, 5, 5, 0.0), rgb);
    }

    #[test]
    fn jpeg_round_trip_keeps_the_size() {
        use zune_core::bytestream::ZCursor;
        use zune_core::colorspace::ColorSpace;
        use zune_core::options::DecoderOptions;
        use zune_jpeg::JpegDecoder;

        let rgb: Vec<u8> = (0..8 * 8).flat_map(|i| [i as u8, 40, 200]).collect();
        let jpeg = encode_jpeg(8, 8, &rgb).unwrap();
        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&jpeg), options);
        let decoded = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!((info.width, info.height), (8, 8));
        assert_eq!(decoded.len(), 8 * 8 * 3);
    }
}
