//! File bytes → display-sized, correctly oriented RGB8.

use anyhow::{Context as _, Result, anyhow, bail};
use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use crate::library::Format;

pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// RGB8, row-major, already rotated for display.
    pub rgb: Vec<u8>,
    /// Size of the full image after orientation.
    pub original_size: [u32; 2],
}

/// Decodes and scales the image down to fit `max_size` (never up). `orientation` is the EXIF
/// value and only honoured for JPEG: libheif already applies the HEIF `irot`/`imir` transforms.
pub fn decode_for_display(
    bytes: &[u8],
    format: Format,
    orientation: u16,
    max_size: [u32; 2],
) -> Result<DecodedImage> {
    let (width, height, rgb, orientation) = match format {
        Format::Jpeg => {
            let (w, h, rgb) = decode_jpeg(bytes)?;
            (w, h, rgb, orientation)
        }
        Format::Heif => {
            let (w, h, rgb) = decode_heif(bytes)?;
            (w, h, rgb, 1)
        }
    };

    let swaps = swaps_axes(orientation);
    let original_size = if swaps {
        [height, width]
    } else {
        [width, height]
    };
    let [fit_w, fit_h] = fit_within(original_size, max_size);
    // Resize before rotating: it is the cheaper order.
    let (resize_w, resize_h) = if swaps {
        (fit_h, fit_w)
    } else {
        (fit_w, fit_h)
    };
    let rgb = if (resize_w, resize_h) == (width, height) {
        rgb
    } else {
        resize_rgb(rgb, width, height, resize_w, resize_h)?
    };
    let (width, height, rgb) = apply_orientation(rgb, resize_w, resize_h, orientation);

    Ok(DecodedImage {
        width,
        height,
        rgb,
        original_size,
    })
}

fn decode_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGB)
        .set_max_width(usize::from(u16::MAX))
        .set_max_height(usize::from(u16::MAX));
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    let pixels = decoder
        .decode()
        .map_err(|e| anyhow!("JPEG decoding failed: {e:?}"))?;
    let info = decoder.info().context("JPEG header missing")?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    let expected = w as usize * h as usize * 3;
    if pixels.len() != expected {
        bail!(
            "unexpected JPEG output: {} bytes for {w}×{h} RGB",
            pixels.len()
        );
    }
    Ok((w, h, pixels))
}

#[cfg(feature = "heic")]
fn decode_heif(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};

    let lib = LibHeif::new();
    let ctx = HeifContext::read_from_bytes(bytes)?;
    let handle = ctx.primary_image_handle()?;
    let image = lib.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)?;
    let planes = image.planes();
    let plane = planes
        .interleaved
        .context("HEIF image has no interleaved RGB plane")?;
    let (w, h, stride) = (plane.width, plane.height, plane.stride);
    let row = w as usize * 3;
    let mut rgb = Vec::with_capacity(row * h as usize);
    for y in 0..h as usize {
        rgb.extend_from_slice(&plane.data[y * stride..y * stride + row]);
    }
    Ok((w, h, rgb))
}

#[cfg(not(feature = "heic"))]
fn decode_heif(_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    bail!("HEIC support is not built in (build with `--features heic`)")
}

pub fn resize_rgb(rgb: Vec<u8>, w: u32, h: u32, dst_w: u32, dst_h: u32) -> Result<Vec<u8>> {
    let src = Image::from_vec_u8(w, h, rgb, PixelType::U8x3)?;
    let mut dst = Image::new(dst_w, dst_h, PixelType::U8x3);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::CatmullRom));
    Resizer::new().resize(&src, &mut dst, &options)?;
    Ok(dst.into_vec())
}

/// Largest size with the same aspect ratio that fits into `max` – never larger than `size`.
pub fn fit_within(size: [u32; 2], max: [u32; 2]) -> [u32; 2] {
    let scale = (max[0] as f64 / size[0] as f64)
        .min(max[1] as f64 / size[1] as f64)
        .min(1.0);
    if scale >= 1.0 {
        return size;
    }
    [
        ((size[0] as f64 * scale).round() as u32).max(1),
        ((size[1] as f64 * scale).round() as u32).max(1),
    ]
}

/// EXIF orientations 5–8 turn the image by 90°.
fn swaps_axes(orientation: u16) -> bool {
    matches!(orientation, 5..=8)
}

/// Transforms stored pixels into display orientation (EXIF 1–8).
fn apply_orientation(rgb: Vec<u8>, w: u32, h: u32, orientation: u16) -> (u32, u32, Vec<u8>) {
    if !(2..=8).contains(&orientation) {
        return (w, h, rgb);
    }
    let (w, h) = (w as usize, h as usize);
    let (dst_w, dst_h) = if swaps_axes(orientation) {
        (h, w)
    } else {
        (w, h)
    };
    // Maps a destination pixel to the source pixel it comes from.
    let source: fn(usize, usize, usize, usize) -> (usize, usize) = match orientation {
        2 => |x, y, w, _| (w - 1 - x, y),         // mirrored horizontally
        3 => |x, y, w, h| (w - 1 - x, h - 1 - y), // rotated 180°
        4 => |x, y, _, h| (x, h - 1 - y),         // mirrored vertically
        5 => |x, y, _, _| (y, x),                 // transposed
        6 => |x, y, _, h| (y, h - 1 - x),         // rotated 90° clockwise
        7 => |x, y, w, h| (w - 1 - y, h - 1 - x), // transversed
        _ => |x, y, w, _| (w - 1 - y, x),         // 8: rotated 90° counter-clockwise
    };
    let mut out = vec![0u8; rgb.len()];
    for y in 0..dst_h {
        for x in 0..dst_w {
            let (sx, sy) = source(x, y, w, h);
            let (s, d) = ((sy * w + sx) * 3, (y * dst_w + x) * 3);
            out[d..d + 3].copy_from_slice(&rgb[s..s + 3]);
        }
    }
    (dst_w as u32, dst_h as u32, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3×2 test image; every pixel's red channel is its letter:
    /// ```text
    /// A B C
    /// D E F
    /// ```
    fn letters() -> Vec<u8> {
        b"ABCDEF".iter().flat_map(|&c| [c, 0, 0]).collect()
    }

    fn reds(rgb: &[u8]) -> String {
        rgb.chunks(3).map(|p| p[0] as char).collect()
    }

    #[test]
    fn orientation_transforms() {
        let cases = [
            (1, (3, 2), "ABCDEF"),
            (2, (3, 2), "CBAFED"),
            (3, (3, 2), "FEDCBA"),
            (4, (3, 2), "DEFABC"),
            (5, (2, 3), "ADBECF"),
            (6, (2, 3), "DAEBFC"),
            (7, (2, 3), "FCEBDA"),
            (8, (2, 3), "CFBEAD"),
        ];
        for (orientation, (w, h), expected) in cases {
            let (ow, oh, out) = apply_orientation(letters(), 3, 2, orientation);
            assert_eq!((ow, oh), (w, h), "size for orientation {orientation}");
            assert_eq!(reds(&out), expected, "pixels for orientation {orientation}");
        }
    }

    #[test]
    fn fit_never_upscales() {
        assert_eq!(fit_within([6000, 4000], [2560, 1440]), [2160, 1440]);
        assert_eq!(fit_within([4000, 6000], [2560, 1440]), [960, 1440]);
        assert_eq!(fit_within([800, 600], [2560, 1440]), [800, 600]);
    }
}
