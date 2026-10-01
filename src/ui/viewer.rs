//! The photo itself: fit-to-window or zoomed (100 % and beyond), pan by dragging.
//!
//! Zoom and position survive switching photos, so a series can be compared at the same spot.

use eframe::egui::{
    Color32, Mesh, Painter, Pos2, Rect, Shape, TextureHandle, Vec2, epaint::Vertex, pos2, vec2,
};

use crate::loader::{FullImage, LoadedImage, Tile};

/// Largest zoom: 8 screen pixels per image pixel.
const MAX_SCALE: f32 = 8.0;

/// `scale` is physical screen pixels per image pixel (1.0 = 100 %); `None` fits the window.
/// `center` is the image point (0..1 in both axes) shown in the middle of the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom {
    pub scale: Option<f32>,
    pub center: Vec2,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            scale: None,
            center: vec2(0.5, 0.5),
        }
    }
}

/// Geometry of one frame: view area, full image size, the display image's size and the
/// display's pixel density.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub area: Rect,
    pub image_size: [u32; 2],
    /// The display image (the loader's texture), decoded to fit the photo area.
    pub display_size: [u32; 2],
    pub pixels_per_point: f32,
}

impl Frame {
    fn size(&self) -> Vec2 {
        vec2(self.image_size[0] as f32, self.image_size[1] as f32)
    }

    /// Fits into the view, never enlarged beyond 100 %.
    pub fn fit_scale(&self) -> f32 {
        let area = self.area.size() * self.pixels_per_point;
        let size = self.size();
        (area.x / size.x).min(area.y / size.y).min(1.0)
    }

    /// Whether the display image is the fitted photo, give or take the rounding of its size.
    /// Then fitting draws it pixel for pixel: scaled by a hair, the GPU would blend
    /// neighbouring pixels across the whole photo. Not while the area has just changed and the
    /// image for the new size is still being decoded.
    fn display_is_fitted(&self) -> bool {
        let fitted = self.size() * self.fit_scale();
        (fitted.x - self.display_size[0] as f32).abs() <= 1.5
            && (fitted.y - self.display_size[1] as f32).abs() <= 1.5
    }
}

/// Photo areas below this many pixels (a minimized window) set no decode size: everything
/// would be decoded again for a few pixels, and once more when the window comes back.
const MIN_DECODE_SIDE: u32 = 32;

/// The decode size for photo `areas` (in points): the smallest of them in physical pixels –
/// the narrower half in compare mode – so a fitted photo fits each one; at most `max_side`.
/// `None` without a usable area.
pub fn decode_size(areas: &[Rect], pixels_per_point: f32, max_side: u32) -> Option<[u32; 2]> {
    // The small allowance keeps 1155.9999 px from becoming 1155.
    let side = |len: f32| ((len * pixels_per_point + 0.01).floor().max(0.0) as u32).min(max_side);
    let size = areas
        .iter()
        .map(|area| [side(area.width()), side(area.height())])
        .reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1])])?;
    (size[0] >= MIN_DECODE_SIDE && size[1] >= MIN_DECODE_SIDE).then_some(size)
}

/// Whether more detail than the display image holds is needed: zoomed in beyond it, as at
/// 100 % of a photo larger than the area. No tolerance – stretched by 4.4 %, the display image
/// kept less than half of the finest detail.
pub fn needs_full(frame: &Frame, zoom: &Zoom) -> bool {
    let shown = zoom.effective_scale(frame) * frame.image_size[0] as f32;
    zoom.is_zoomed()
        && frame.display_size[0] < frame.image_size[0]
        && shown > frame.display_size[0] as f32 + 0.5
}

impl Zoom {
    pub fn is_zoomed(&self) -> bool {
        self.scale.is_some()
    }

    pub fn effective_scale(&self, frame: &Frame) -> f32 {
        self.scale.unwrap_or_else(|| frame.fit_scale())
    }

    /// Where the whole image lies on screen, in points. Snapped to physical pixels so 100 %
    /// really is 1:1 – and so is a fitted display image (see [`Frame::display_is_fitted`]).
    pub fn image_rect(&self, frame: &Frame) -> Rect {
        let ppp = frame.pixels_per_point;
        let size = if self.scale.is_none() && frame.display_is_fitted() {
            vec2(frame.display_size[0] as f32, frame.display_size[1] as f32) / ppp
        } else {
            frame.size() * self.effective_scale(frame) / ppp
        };
        let area = frame.area;
        let axis = |start: f32, end: f32, len: f32, center: f32| {
            if len <= end - start {
                (start + end - len) / 2.0
            } else {
                let min = (start + end) / 2.0 - center * len;
                min.clamp(end - len, start)
            }
        };
        let min = pos2(
            axis(area.left(), area.right(), size.x, self.center.x),
            axis(area.top(), area.bottom(), size.y, self.center.y),
        );
        let snap = |v: f32| (v * ppp).round() / ppp;
        Rect::from_min_size(pos2(snap(min.x), snap(min.y)), size)
    }

    /// Switches between fitting and 100 %, keeping the point under the cursor in place.
    pub fn toggle(&mut self, frame: &Frame, anchor: Option<Pos2>) {
        if self.is_zoomed() {
            self.scale = None;
        } else {
            let anchor = anchor.unwrap_or(frame.area.center());
            self.set_scale(frame, 1.0_f32.max(frame.fit_scale() * 1.01), anchor);
        }
    }

    /// Zooms by `factor` around `anchor`; zooming out below "fit" returns to fitting.
    pub fn zoom_by(&mut self, frame: &Frame, factor: f32, anchor: Pos2) {
        let scale = self.effective_scale(frame) * factor;
        if scale <= frame.fit_scale() * 1.001 {
            self.scale = None;
        } else {
            self.set_scale(frame, scale.min(MAX_SCALE), anchor);
        }
    }

    fn set_scale(&mut self, frame: &Frame, scale: f32, anchor: Pos2) {
        let before = self.image_rect(frame);
        let point = ((anchor - before.min) / before.size()).clamp(Vec2::ZERO, Vec2::splat(1.0));
        let size = frame.size() * scale / frame.pixels_per_point;
        let min = anchor - point * size;
        self.scale = Some(scale);
        self.center = (frame.area.center() - min) / size;
    }

    /// Moves the image by `delta` points (dragging).
    pub fn pan(&mut self, frame: &Frame, delta: Vec2) {
        let size = self.image_rect(frame).size();
        self.center = (self.center - delta / size).clamp(Vec2::ZERO, Vec2::splat(1.0));
    }
}

/// The check overlay of one photo, as far as it is computed: over the display image and tile
/// by tile over the full resolution.
#[derive(Default, Clone, Copy)]
pub struct Overlay<'a> {
    pub display: Option<&'a TextureHandle>,
    pub full: Option<&'a [Tile]>,
}

/// Draws the photo, then the overlay over it in the same place. Returns whether more detail
/// than the display texture has is needed, i.e. the full-resolution image should be loaded.
pub fn draw(
    painter: &Painter,
    frame: &Frame,
    zoom: &Zoom,
    display: &LoadedImage,
    full: Option<&FullImage>,
    overlay: Overlay<'_>,
    straighten: Option<f64>,
) -> bool {
    let rect = zoom.image_rect(frame);
    let painter = painter.with_clip_rect(frame.area);
    if let Some(radians) = straighten {
        let scale = crate::edit::cover_scale(frame.image_size[0], frame.image_size[1], radians);
        draw_rotated(
            &painter.with_clip_rect(rect),
            rect,
            display.texture.id(),
            radians,
            scale as f32,
        );
        return false;
    }
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let needs_full = needs_full(frame, zoom);

    match full {
        Some(full) if needs_full => {
            let per_pixel = rect.width() / full.size[0] as f32;
            let tiles = |tiles: &[Tile]| {
                for tile in tiles {
                    let min =
                        rect.min + vec2(tile.origin[0] as f32, tile.origin[1] as f32) * per_pixel;
                    let tile_rect = Rect::from_min_size(
                        min,
                        vec2(tile.size[0] as f32, tile.size[1] as f32) * per_pixel,
                    );
                    if tile_rect.intersects(frame.area) {
                        painter.image(tile.texture.id(), tile_rect, uv, Color32::WHITE);
                    }
                }
            };
            tiles(&full.tiles);
            // Until the overlay's own tiles are there, the display one lies over it, softer.
            match (overlay.full, overlay.display) {
                (Some(overlay), _) => tiles(overlay),
                (None, Some(overlay)) => {
                    painter.image(overlay.id(), rect, uv, Color32::WHITE);
                }
                (None, None) => {}
            }
        }
        _ => {
            painter.image(display.texture.id(), rect, uv, Color32::WHITE);
            if let Some(overlay) = overlay.display {
                painter.image(overlay.id(), rect, uv, Color32::WHITE);
            }
        }
    }
    needs_full
}

/// The display texture, scaled by `scale` and rotated clockwise about the centre of `rect`.
fn draw_rotated(
    painter: &Painter,
    rect: Rect,
    texture: eframe::egui::TextureId,
    radians: f64,
    scale: f32,
) {
    let (sin, cos) = (radians as f32).sin_cos();
    let center = rect.center();
    let half = rect.size() * scale * 0.5;
    let corners = [
        (vec2(-half.x, -half.y), pos2(0.0, 0.0)),
        (vec2(half.x, -half.y), pos2(1.0, 0.0)),
        (vec2(half.x, half.y), pos2(1.0, 1.0)),
        (vec2(-half.x, half.y), pos2(0.0, 1.0)),
    ];
    let mut mesh = Mesh::with_texture(texture);
    for (offset, uv) in corners {
        let turned = vec2(
            offset.x * cos - offset.y * sin,
            offset.x * sin + offset.y * cos,
        );
        mesh.vertices.push(Vertex {
            pos: center + turned,
            uv,
            color: Color32::WHITE,
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame {
            area: Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0)),
            image_size: [4000, 2000],
            display_size: [1000, 500],
            pixels_per_point: 1.0,
        }
    }

    /// A 4000 × 2256 photo in the 2560 × 1156 px photo area of a maximized window at 125 %.
    fn scaled_frame(display_size: [u32; 2]) -> Frame {
        Frame {
            area: Rect::from_min_size(pos2(0.0, 0.0), vec2(2048.0, 924.8)),
            image_size: [4000, 2256],
            display_size,
            pixels_per_point: 1.25,
        }
    }

    /// The rect in physical pixels.
    fn pixels(rect: Rect) -> [f32; 4] {
        let r = rect * 1.25;
        [r.min.x, r.min.y, r.width(), r.height()]
    }

    #[test]
    fn a_fitted_display_image_is_drawn_pixel_for_pixel() {
        // The loader decodes 2050 × 1156 for this area; fitting would be 2049.6 px wide.
        let rect = Zoom::default().image_rect(&scaled_frame([2050, 1156]));
        let [x, y, w, h] = pixels(rect);
        assert!(
            (w - 2050.0).abs() < 1e-3 && (h - 1156.0).abs() < 1e-3,
            "{w} × {h}"
        );
        assert!(
            (x - x.round()).abs() < 1e-3 && (y - y.round()).abs() < 1e-3,
            "{x}, {y}"
        );
        // An image decoded for another area (being decoded again) is scaled to fit meanwhile.
        let [_, _, w, _] = pixels(Zoom::default().image_rect(&scaled_frame([3830, 2160])));
        assert!((w - 4000.0 * 1156.0 / 2256.0).abs() < 0.01, "{w}");
    }

    #[test]
    fn the_decode_size_is_the_photo_area_in_pixels() {
        let area = |w: f32, h: f32| Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
        assert_eq!(
            decode_size(&[area(2048.0, 924.8)], 1.25, 8192),
            Some([2560, 1156])
        );
        // Compare mode: both halves, the smaller wins.
        assert_eq!(
            decode_size(&[area(1022.0, 924.8), area(1021.6, 924.8)], 1.25, 8192),
            Some([1277, 1156])
        );
        assert_eq!(
            decode_size(&[area(9000.0, 100.0)], 1.0, 8192),
            Some([8192, 100])
        );
        assert_eq!(
            decode_size(&[area(2048.0, 0.0)], 1.25, 8192),
            None,
            "minimized"
        );
        assert_eq!(decode_size(&[], 1.25, 8192), None);
    }

    #[test]
    fn full_resolution_as_soon_as_the_display_image_would_be_stretched() {
        let mut zoom = Zoom::default();
        // The 4K start-up decode, 3830 px wide: 100 % used to stretch it by 4.4 %.
        let frame = scaled_frame([3830, 2160]);
        assert!(
            !needs_full(&frame, &zoom),
            "fitted, the display image is enough"
        );
        zoom.toggle(&frame, None);
        assert_eq!(zoom.scale, Some(1.0));
        assert!(needs_full(&frame, &zoom));
        // A photo smaller than the area is its own display image: nothing more to load.
        let small = Frame {
            image_size: [800, 600],
            display_size: [800, 600],
            ..frame
        };
        let mut zoom = Zoom::default();
        zoom.toggle(&small, None);
        assert!(!needs_full(&small, &zoom));
    }

    #[test]
    fn fits_and_centres() {
        let zoom = Zoom::default();
        assert_eq!(frame().fit_scale(), 0.25);
        assert_eq!(
            zoom.image_rect(&frame()),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0))
        );
    }

    #[test]
    fn toggling_keeps_the_point_under_the_cursor() {
        let mut zoom = Zoom::default();
        let anchor = pos2(250.0, 125.0); // image point (0.25, 0.25)
        zoom.toggle(&frame(), Some(anchor));
        assert_eq!(zoom.scale, Some(1.0));
        let rect = zoom.image_rect(&frame());
        assert_eq!(rect.size(), vec2(4000.0, 2000.0));
        let under_cursor = (anchor - rect.min) / rect.size();
        assert!(
            (under_cursor - vec2(0.25, 0.25)).length() < 1e-3,
            "{under_cursor:?}"
        );
        zoom.toggle(&frame(), None);
        assert_eq!(zoom.scale, None);
    }

    #[test]
    fn panning_stops_at_the_edges() {
        let mut zoom = Zoom {
            scale: Some(1.0),
            center: vec2(0.5, 0.5),
        };
        zoom.pan(&frame(), vec2(100_000.0, 0.0));
        let rect = zoom.image_rect(&frame());
        assert_eq!(rect.left(), 0.0, "left edge stays at the view's left edge");
    }

    #[test]
    fn zooming_out_past_fit_returns_to_fit() {
        let mut zoom = Zoom::default();
        zoom.zoom_by(&frame(), 2.0, pos2(500.0, 250.0));
        assert_eq!(zoom.scale, Some(0.5));
        zoom.zoom_by(&frame(), 0.25, pos2(500.0, 250.0));
        assert_eq!(zoom.scale, None);
    }
}
