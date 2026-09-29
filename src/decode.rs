//! File bytes → display-sized, correctly oriented RGB8.

use anyhow::{Context as _, Result, anyhow, bail};
use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use crate::library::Format;

/// Photos with more pixels are not decoded. The decoders allocate the whole frame before
/// reading a pixel, so a tiny file whose header claims 65 535 × 65 535 would ask for 12.9 GB –
/// on every visit of the folder, since the analysis reaches every file. 200 megapixels
/// (600 MB RGB) cover every camera and most panoramas.
pub const MAX_PIXELS: u64 = 200_000_000;

pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// RGB8, row-major, already rotated for display.
    pub rgb: Vec<u8>,
    /// Size of the full image after orientation.
    pub original_size: [u32; 2],
}

/// Decodes and scales the image down to fit `max_size` (never up). `orientation` is the EXIF
/// value, honoured for everything but HEIC: libheif already applies the HEIF `irot`/`imir`
/// transforms. JPEG pixels (and RAW previews) come out in sRGB (see [`to_srgb`]).
pub fn decode_for_display(
    bytes: &[u8],
    format: Format,
    orientation: u16,
    max_size: [u32; 2],
) -> Result<DecodedImage> {
    decode(bytes, format, orientation, max_size, true)
}

/// A JPEG at full size and in display orientation, with the pixels left in the file's own
/// colour space: straighten and crop write them back under the original ICC profile, so a
/// Display P3 or Adobe RGB photo keeps its colours.
pub fn decode_for_edit(bytes: &[u8], orientation: u16) -> Result<DecodedImage> {
    decode(bytes, Format::Jpeg, orientation, [u32::MAX; 2], false)
}

fn decode(
    bytes: &[u8],
    format: Format,
    orientation: u16,
    max_size: [u32; 2],
    srgb: bool,
) -> Result<DecodedImage> {
    let (width, height, rgb, orientation) = match format {
        Format::Jpeg => {
            let (w, h, rgb) = decode_jpeg(bytes, srgb)?;
            (w, h, rgb, orientation)
        }
        Format::Heif => {
            let (w, h, rgb) = decode_heif(bytes)?;
            (w, h, rgb, 1)
        }
        Format::Png | Format::Tiff | Format::WebP | Format::Bmp | Format::Gif => {
            let (w, h, rgb) = decode_raster(bytes, format)?;
            (w, h, rgb, orientation)
        }
        // The embedded preview is stored like the sensor; the RAW's orientation turns it.
        Format::Dng | Format::Raw => {
            let preview = crate::raw::preview(bytes).context("no preview image in the RAW file")?;
            let (w, h, rgb) = decode_jpeg(preview, srgb)?;
            (w, h, rgb, orientation)
        }
        Format::Video => bail!("a video has no picture of its own (see `video::poster`)"),
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

fn decode_jpeg(bytes: &[u8], srgb: bool) -> Result<(u32, u32, Vec<u8>)> {
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGB)
        .set_max_width(usize::from(u16::MAX))
        .set_max_height(usize::from(u16::MAX));
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder
        .decode_headers()
        .map_err(|e| anyhow!("JPEG decoding failed: {e:?}"))?;
    let info = decoder.info().context("JPEG header missing")?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    check_size(u64::from(w), u64::from(h))?;
    let mut pixels = decoder
        .decode()
        .map_err(|e| anyhow!("JPEG decoding failed: {e:?}"))?;
    if srgb && let Some(profile) = decoder.icc_profile() {
        to_srgb(&mut pixels, &profile);
    }
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
    check_size(u64::from(handle.width()), u64::from(handle.height()))?;
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

/// PNG, TIFF, WebP, BMP and GIF (its first frame) through the `image` crate. Transparent parts
/// are laid on neutral grey. No colour conversion: most of these files are sRGB anyway.
fn decode_raster(bytes: &[u8], format: Format) -> Result<(u32, u32, Vec<u8>)> {
    use image::{ImageFormat, ImageReader};
    let kind = match format {
        Format::Png => ImageFormat::Png,
        Format::Tiff => ImageFormat::Tiff,
        Format::WebP => ImageFormat::WebP,
        Format::Bmp => ImageFormat::Bmp,
        _ => ImageFormat::Gif,
    };
    let reader = || ImageReader::with_format(std::io::Cursor::new(bytes), kind);
    let (w, h) = reader()
        .into_dimensions()
        .map_err(|e| anyhow!("{kind:?} header unreadable: {e}"))?;
    check_size(u64::from(w), u64::from(h))?;
    let mut decoder = reader();
    // The size is checked above; the crate's own allocation limit is lower than ours.
    decoder.no_limits();
    let rgba = decoder
        .decode()
        .map_err(|e| anyhow!("{kind:?} decoding failed: {e}"))?
        .into_rgba8();
    const BACKGROUND: u32 = 0x80;
    let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
    for px in rgba.pixels() {
        let alpha = u32::from(px[3]);
        for &c in &px.0[..3] {
            rgb.push(((u32::from(c) * alpha + BACKGROUND * (255 - alpha) + 127) / 255) as u8);
        }
    }
    Ok((w, h, rgb))
}

/// The header's size, checked before anything is allocated for it (see [`MAX_PIXELS`]).
fn check_size(width: u64, height: u64) -> Result<()> {
    if width * height > MAX_PIXELS {
        bail!(
            "photo too large ({width} × {height} pixels, at most {} megapixels)",
            MAX_PIXELS / 1_000_000
        );
    }
    Ok(())
}

/// Runs one decode job. A panic in it – a defective file hitting a bug in a decoder crate –
/// becomes an error, so the worker thread lives on and the photo shows a message instead of
/// loading forever. Needs `panic = "unwind"` (the default); `abort` in a profile would end the
/// whole program instead.
pub fn catch_panic<T>(job: impl FnOnce() -> Result<T>) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("unknown cause");
        bail!("decoder crashed: {message}")
    })
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

/// JPEG pixels are stored in the file's colour space. Convert once, here, so the viewer, the
/// filmstrip and the analysis all see sRGB. Untagged files and sRGB stay as they are. HEIC is
/// already converted by libheif.
fn to_srgb(rgb: &mut [u8], icc: &[u8]) {
    match profile_kind(icc) {
        ProfileKind::Leave => {}
        ProfileKind::AdobeRgb => map_channels(rgb, adobe_rgb_to_srgb),
        ProfileKind::DisplayP3 => map_channels(rgb, display_p3_to_srgb),
        ProfileKind::Other => {
            if let Err(err) = convert_with_cms(rgb, icc) {
                log::debug!("ICC profile left unconverted: {err}");
            }
        }
    }
}

enum ProfileKind {
    Leave,
    AdobeRgb,
    DisplayP3,
    Other,
}

fn profile_kind(icc: &[u8]) -> ProfileKind {
    let text = profile_description(icc).to_ascii_lowercase();
    if text.contains("display p3") || text.contains("display-p3") {
        ProfileKind::DisplayP3
    } else if text.contains("adobe rgb") {
        ProfileKind::AdobeRgb
    } else if text.contains("srgb") {
        ProfileKind::Leave
    } else {
        ProfileKind::Other
    }
}

fn profile_description(icc: &[u8]) -> String {
    let Ok(profile) = moxcms::ColorProfile::new_from_slice(icc) else {
        return String::new();
    };
    match profile.description {
        Some(moxcms::ProfileText::PlainString(text)) => text,
        Some(moxcms::ProfileText::Localizable(parts)) => parts
            .into_iter()
            .map(|part| part.value)
            .collect::<Vec<_>>()
            .join(" "),
        Some(moxcms::ProfileText::Description(text)) => text.ascii_string,
        None => String::new(),
    }
}

fn map_channels(rgb: &mut [u8], f: fn([f32; 3]) -> [f32; 3]) {
    for px in rgb.as_chunks_mut::<3>().0 {
        let encoded = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        ];
        let out = f(encoded);
        for (dst, value) in px.iter_mut().zip(out) {
            *dst = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

/// Adobe RGB (1998), D65, to XYZ.
const ADOBE_TO_XYZ: [[f32; 3]; 3] = [
    [0.5767309, 0.185_554, 0.1881852],
    [0.2973769, 0.6273491, 0.0752741],
    [0.0270343, 0.0706872, 0.9911085],
];

/// Display P3, D65, to XYZ. The transfer curve is the sRGB curve.
const P3_TO_XYZ: [[f32; 3]; 3] = [
    [0.48657095, 0.2656677, 0.19821729],
    [0.22897457, 0.6917385, 0.07928691],
    [0.0, 0.04511338, 1.0439444],
];

/// XYZ (D65) to linear sRGB.
const XYZ_TO_SRGB: [[f32; 3]; 3] = [
    [3.2404542, -1.5371385, -0.4985314],
    [-0.969_266, 1.8760108, 0.041_556],
    [0.0556434, -0.2040259, 1.0572252],
];

fn adobe_rgb_to_srgb(rgb: [f32; 3]) -> [f32; 3] {
    let linear = rgb.map(|c| c.powf(2.199_218_8));
    encode_srgb(mul(XYZ_TO_SRGB, mul(ADOBE_TO_XYZ, linear)))
}

fn display_p3_to_srgb(rgb: [f32; 3]) -> [f32; 3] {
    let linear = rgb.map(decode_srgb);
    encode_srgb(mul(XYZ_TO_SRGB, mul(P3_TO_XYZ, linear)))
}

fn mul(m: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn decode_srgb(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn encode_srgb(linear: [f32; 3]) -> [f32; 3] {
    linear.map(|c| {
        if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    })
}

fn convert_with_cms(rgb: &mut [u8], icc: &[u8]) -> Result<(), String> {
    let src = moxcms::ColorProfile::new_from_slice(icc).map_err(|err| err.to_string())?;
    let dst = moxcms::ColorProfile::new_srgb();
    let transform = src
        .create_transform_8bit(
            moxcms::Layout::Rgb,
            &dst,
            moxcms::Layout::Rgb,
            moxcms::TransformOptions::default(),
        )
        .map_err(|err| err.to_string())?;
    let mut out = vec![0u8; rgb.len()];
    transform
        .transform(rgb, &mut out)
        .map_err(|err| err.to_string())?;
    rgb.copy_from_slice(&out);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each of the five formats the `image` crate reads, written by the same crate: a 2×1
    /// image, left pixel opaque red, right one fully transparent (grey where alpha is kept).
    #[test]
    fn raster_formats_decode_with_transparency_on_grey() {
        use image::{ImageFormat, RgbaImage};
        let rgba = RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0]).unwrap();
        for (format, kind, alpha) in [
            (Format::Png, ImageFormat::Png, true),
            (Format::Tiff, ImageFormat::Tiff, true),
            (Format::WebP, ImageFormat::WebP, true),
            (Format::Bmp, ImageFormat::Bmp, true),
            (Format::Gif, ImageFormat::Gif, true),
        ] {
            let mut bytes = std::io::Cursor::new(Vec::new());
            rgba.write_to(&mut bytes, kind).unwrap();
            let image = decode_for_display(bytes.get_ref(), format, 1, [100, 100])
                .unwrap_or_else(|e| panic!("{format:?}: {e:#}"));
            assert_eq!((image.width, image.height), (2, 1), "{format:?}");
            assert_eq!(&image.rgb[..3], [255, 0, 0], "{format:?}");
            if alpha {
                assert_eq!(&image.rgb[3..], [128, 128, 128], "{format:?}: transparent → grey");
            }
        }
        assert!(decode_for_display(b"not a png", Format::Png, 1, [9, 9]).is_err());
    }

    /// A RAW is shown by its embedded preview: here the fixture JPEG behind some bytes.
    #[test]
    fn a_raw_shows_its_preview() {
        let jpeg = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/tiny.jpg"
        ))
        .unwrap();
        let direct = decode_for_display(&jpeg, Format::Jpeg, 1, [64, 64]).unwrap();
        let mut raw = b"II*\0 sensor data \xFF\xD8\xFF\x00 more".to_vec();
        raw.extend(&jpeg);
        let shown = decode_for_display(&raw, Format::Raw, 1, [64, 64]).unwrap();
        assert_eq!((shown.width, shown.height), (direct.width, direct.height));
        assert_eq!(shown.rgb, direct.rgb);
        assert!(decode_for_display(b"II*\0 nothing", Format::Raw, 1, [9, 9]).is_err());
    }

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

    /// A header that claims 20 000 × 20 000 pixels in a file of a few hundred bytes: refused
    /// before the decoder allocates 1.2 GB for it.
    #[test]
    fn a_huge_header_is_refused_before_decoding() {
        let mut jpeg = fixtures::jpeg(1, 1, &[200, 40, 40]);
        let sof = jpeg
            .windows(2)
            .position(|marker| marker == [0xFF, 0xC0])
            .expect("SOF0 marker");
        jpeg[sof + 5..sof + 7].copy_from_slice(&20_000u16.to_be_bytes());
        jpeg[sof + 7..sof + 9].copy_from_slice(&20_000u16.to_be_bytes());
        for result in [
            decode_for_display(&jpeg, Format::Jpeg, 1, [8, 8]).map(|_| ()),
            decode_for_edit(&jpeg, 1).map(|_| ()),
        ] {
            let err = result.unwrap_err();
            assert!(err.to_string().contains("20000 × 20000"), "{err}");
        }
    }

    #[test]
    fn the_limit_is_200_megapixels() {
        assert!(check_size(20_000, 10_000).is_ok());
        assert!(check_size(20_000, 10_001).is_err());
        assert!(check_size(65_535, 65_535).is_err());
    }

    #[test]
    fn a_panicking_job_becomes_an_error() {
        let err = catch_panic(|| -> Result<()> { panic!("broken file") }).unwrap_err();
        assert!(err.to_string().contains("broken file"), "{err}");
        let err = catch_panic(|| -> Result<()> { panic!("{} bytes", 3) }).unwrap_err();
        assert!(err.to_string().contains("3 bytes"), "{err}");
        assert_eq!(catch_panic(|| Ok(7)).unwrap(), 7);
    }

    #[test]
    fn edit_decode_keeps_the_stored_colours() {
        let plain = fixtures::jpeg(1, 1, &[200, 40, 40]);
        let mut adobe = plain.clone();
        fixtures::insert_icc(&mut adobe, &fixtures::adobe_rgb_profile());
        let bare = decode_for_edit(&plain, 1).unwrap();
        let kept = decode_for_edit(&adobe, 1).unwrap();
        assert_eq!(bare.rgb, kept.rgb, "no conversion for an edit");
        let shown = decode_for_display(&adobe, crate::library::Format::Jpeg, 1, [8, 8]).unwrap();
        assert_ne!(shown.rgb, kept.rgb, "the display converts");
    }

    #[test]
    fn adobe_rgb_jpeg_changes_the_pixel_and_srgb_does_not() {
        let plain = fixtures::jpeg(1, 1, &[200, 40, 40]);
        let bare = decode_for_display(&plain, crate::library::Format::Jpeg, 1, [8, 8]).unwrap();

        let mut adobe_jpeg = plain.clone();
        fixtures::insert_icc(&mut adobe_jpeg, &fixtures::adobe_rgb_profile());
        let adobe =
            decode_for_display(&adobe_jpeg, crate::library::Format::Jpeg, 1, [8, 8]).unwrap();
        assert_ne!(
            bare.rgb, adobe.rgb,
            "Adobe RGB must not be shown as stored bytes"
        );

        let srgb_profile = moxcms::ColorProfile::new_srgb()
            .encode()
            .expect("srgb profile");
        let mut srgb_jpeg = plain.clone();
        fixtures::insert_icc(&mut srgb_jpeg, &srgb_profile);
        let srgb = decode_for_display(&srgb_jpeg, crate::library::Format::Jpeg, 1, [8, 8]).unwrap();
        assert_eq!(bare.rgb, srgb.rgb);
    }
}

/// Small JPEGs with or without an ICC profile, for tests here and in `rating`.
#[cfg(test)]
pub mod fixtures {
    pub fn jpeg(width: u16, height: u16, rgb: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        jpeg_encoder::Encoder::new(&mut out, 100)
            .encode(rgb, width, height, jpeg_encoder::ColorType::Rgb)
            .unwrap();
        out
    }

    pub fn adobe_rgb_profile() -> Vec<u8> {
        moxcms::ColorProfile::new_adobe_rgb()
            .encode()
            .expect("adobe profile")
    }

    /// One APP2 `ICC_PROFILE` segment right after the JPEG start marker.
    pub fn insert_icc(jpeg: &mut Vec<u8>, profile: &[u8]) {
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let mut segment = Vec::new();
        segment.extend_from_slice(b"ICC_PROFILE\0");
        segment.push(1);
        segment.push(1);
        segment.extend_from_slice(profile);
        let len = u16::try_from(segment.len() + 2).unwrap();
        let mut app2 = vec![0xFF, 0xE2];
        app2.extend_from_slice(&len.to_be_bytes());
        app2.extend(segment);
        jpeg.splice(2..2, app2);
    }
}
