//! The bar under a playing video: play/pause, the time, the timeline, the speaker and the
//! volume. It sits where the "Play (Enter)" pill was and fades while the video plays and the
//! pointer rests. Layout and the timeline arithmetic are pure functions; the bar itself only
//! paints and reports what was pressed.

use std::time::Duration;

use eframe::egui::{
    Align2, Color32, CursorIcon, FontId, Id, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2,
};

use crate::i18n;
use crate::theme::{text, tokens};
use crate::ui::overlays;

const HEIGHT: f32 = 40.0;
const MAX_WIDTH: f32 = 760.0;
const BUTTON: f32 = 32.0;
const SPEAKER: f32 = 24.0;
const VOLUME_WIDTH: f32 = 72.0;
const GAP: f32 = 10.0;

/// What the bar shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    pub playing: bool,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub volume: f32,
    pub muted: bool,
    /// 1 shown, 0 faded out (then it takes no clicks either).
    pub opacity: f32,
}

/// What was pressed this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ControlsOutput {
    pub toggle: bool,
    /// Where to: while the timeline is dragged to the nearest keyframe, on release exactly.
    pub seek: Option<(Duration, bool)>,
    pub volume: Option<f32>,
    pub mute: bool,
    /// The pointer is on the bar: it stays.
    pub hovered: bool,
}

/// The parts of the bar, left to right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parts {
    pub bar: Rect,
    pub play: Rect,
    pub time: Rect,
    pub timeline: Rect,
    pub speaker: Rect,
    pub volume: Rect,
}

/// The bar centred at the bottom of `area` (where the play pill was), the time text
/// `time_width` wide.
pub fn layout(area: Rect, time_width: f32) -> Parts {
    let width = (area.width() - 32.0).clamp(240.0, MAX_WIDTH);
    let bar = Rect::from_center_size(overlays::bottom_slot(area, 0), vec2(width, HEIGHT));
    let y = bar.center().y;
    let play = Rect::from_center_size(
        pos2(bar.left() + 4.0 + BUTTON / 2.0, y),
        vec2(BUTTON, BUTTON),
    );
    let time = Rect::from_min_size(pos2(play.right() + GAP, y - 9.0), vec2(time_width, 18.0));
    let volume = Rect::from_min_size(
        pos2(bar.right() - 12.0 - VOLUME_WIDTH, y - 9.0),
        vec2(VOLUME_WIDTH, 18.0),
    );
    let speaker = Rect::from_center_size(
        pos2(volume.left() - 6.0 - SPEAKER / 2.0, y),
        vec2(SPEAKER, SPEAKER),
    );
    let timeline = Rect::from_min_max(
        pos2(time.right() + GAP, y - 9.0),
        pos2(
            (speaker.left() - GAP).max(time.right() + GAP + 20.0),
            y + 9.0,
        ),
    );
    Parts {
        bar,
        play,
        time,
        timeline,
        speaker,
        volume,
    }
}

/// The share 0..=1 of `rect`'s width at `x`.
pub fn share_at(rect: Rect, x: f32) -> f32 {
    ((x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0)
}

/// The place in a video of `duration` at `x` on the timeline.
pub fn position_at(timeline: Rect, x: f32, duration: Duration) -> Duration {
    duration.mul_f32(share_at(timeline, x))
}

/// `0:07`, `12:34`, `1:02:03`.
pub fn clock(time: Duration) -> String {
    let s = time.as_secs();
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn show(ui: &Ui, area: Rect, controls: &Controls, id: Id) -> ControlsOutput {
    let mut out = ControlsOutput::default();
    if controls.opacity <= 0.0 {
        return out;
    }
    let total = controls.duration.unwrap_or_default();
    let time_text = format!("{} / {}", clock(controls.position), clock(total));
    let font = FontId::proportional(text::SMALL);
    // Wide enough for the longest time of this video, so the timeline keeps its length.
    let widest = format!("{0} / {0}", clock(total)).replace(char::is_numeric, "0");
    let time_width = ui
        .painter()
        .layout_no_wrap(widest, font.clone(), tokens::TEXT)
        .size()
        .x;
    let parts = layout(area, time_width);
    let mut painter = ui.painter().with_clip_rect(area);
    painter.set_opacity(controls.opacity);
    painter.rect_filled(parts.bar, 8.0, Color32::from_black_alpha(190));
    let t = i18n::t();

    let bar = ui.interact(parts.bar, id.with("bar"), Sense::hover());
    out.hovered = bar.hovered();

    // Play / pause.
    let play = ui
        .interact(parts.play, id.with("play"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(t.video_play_pause);
    let colour = if play.hovered() {
        tokens::ACCENT_STRONG
    } else {
        tokens::TEXT
    };
    paint_play(&painter, parts.play.center(), controls.playing, colour);
    out.toggle = play.clicked();

    painter.text(
        parts.time.left_center(),
        Align2::LEFT_CENTER,
        time_text,
        font,
        tokens::TEXT,
    );

    // Timeline: drag to the nearest keyframe, let go (or click) for the exact place.
    let timeline = ui
        .interact(
            parts.timeline.expand2(vec2(0.0, 6.0)),
            id.with("timeline"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    let track = Rect::from_center_size(parts.timeline.center(), vec2(parts.timeline.width(), 4.0));
    let mut shown = if total.is_zero() {
        0.0
    } else {
        controls.position.as_secs_f32() / total.as_secs_f32()
    };
    if let Some(pointer) = timeline.interact_pointer_pos()
        && !total.is_zero()
    {
        let to = position_at(parts.timeline, pointer.x, total);
        shown = share_at(parts.timeline, pointer.x);
        if timeline.drag_stopped() || timeline.clicked() {
            out.seek = Some((to, true));
        } else if timeline.dragged() {
            out.seek = Some((to, false));
        }
    }
    let shown = shown.clamp(0.0, 1.0);
    painter.rect_filled(track, 2.0, tokens::LINE);
    painter.rect_filled(
        Rect::from_min_size(track.min, vec2(track.width() * shown, track.height())),
        2.0,
        tokens::ACCENT,
    );
    let knob = pos2(track.left() + track.width() * shown, track.center().y);
    let radius = if timeline.hovered() || timeline.dragged() {
        6.0
    } else {
        4.5
    };
    painter.circle_filled(knob, radius, tokens::TEXT);

    // Speaker: sound on / off.
    let speaker = ui
        .interact(parts.speaker, id.with("speaker"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(t.video_mute);
    let colour = if speaker.hovered() {
        tokens::ACCENT_STRONG
    } else {
        tokens::TEXT
    };
    paint_speaker(&painter, parts.speaker.center(), controls.muted, colour);
    out.mute = speaker.clicked();

    // Volume.
    let volume = ui
        .interact(
            parts.volume.expand2(vec2(4.0, 6.0)),
            id.with("volume"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(t.video_volume);
    let mut level = if controls.muted { 0.0 } else { controls.volume };
    if let Some(pointer) = volume.interact_pointer_pos()
        && (volume.dragged() || volume.clicked())
    {
        level = share_at(parts.volume, pointer.x);
        out.volume = Some(level);
    }
    let track = Rect::from_center_size(parts.volume.center(), vec2(parts.volume.width(), 4.0));
    painter.rect_filled(track, 2.0, tokens::LINE);
    painter.rect_filled(
        Rect::from_min_size(track.min, vec2(track.width() * level, track.height())),
        2.0,
        tokens::ACCENT,
    );
    painter.circle_filled(
        pos2(track.left() + track.width() * level, track.center().y),
        4.5,
        tokens::TEXT,
    );
    out.hovered |= play.hovered() || timeline.hovered() || speaker.hovered() || volume.hovered();
    out
}

/// A triangle, or two bars while it plays.
fn paint_play(painter: &eframe::egui::Painter, c: Pos2, playing: bool, colour: Color32) {
    if playing {
        for dx in [-4.5, 4.5] {
            painter.rect_filled(
                Rect::from_center_size(c + vec2(dx, 0.0), vec2(4.0, 14.0)),
                1.0,
                colour,
            );
        }
    } else {
        painter.add(Shape::convex_polygon(
            vec![
                c + vec2(-5.0, -8.0),
                c + vec2(8.0, 0.0),
                c + vec2(-5.0, 8.0),
            ],
            colour,
            Stroke::NONE,
        ));
    }
}

/// A speaker with a wave, or with a cross when muted.
fn paint_speaker(painter: &eframe::egui::Painter, c: Pos2, muted: bool, colour: Color32) {
    let o = c + vec2(-3.0, 0.0);
    painter.add(Shape::convex_polygon(
        vec![
            o + vec2(-6.0, -3.0),
            o + vec2(-2.0, -3.0),
            o + vec2(3.0, -7.0),
            o + vec2(3.0, 7.0),
            o + vec2(-2.0, 3.0),
            o + vec2(-6.0, 3.0),
        ],
        colour,
        Stroke::NONE,
    ));
    let stroke = Stroke::new(1.6, colour);
    if muted {
        let x = c + vec2(6.5, 0.0);
        painter.line_segment([x + vec2(-3.0, -3.0), x + vec2(3.0, 3.0)], stroke);
        painter.line_segment([x + vec2(-3.0, 3.0), x + vec2(3.0, -3.0)], stroke);
    } else {
        let points: Vec<Pos2> = (0..=8)
            .map(|i| {
                let a = -0.9 + 1.8 * i as f32 / 8.0;
                c + vec2(2.5 + 5.0 * a.cos(), 5.0 * a.sin())
            })
            .collect();
        painter.add(Shape::line(points, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_sit_in_the_bar_left_to_right() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0));
        let p = layout(area, 80.0);
        assert!((p.bar.width() - MAX_WIDTH).abs() < 0.5, "capped");
        assert!((p.bar.center().x - 800.0).abs() < 0.5);
        assert!((p.bar.center().y - overlays::bottom_slot(area, 0).y).abs() < 0.5);
        for part in [p.play, p.time, p.timeline, p.speaker, p.volume] {
            assert!(p.bar.contains_rect(part), "{part:?} outside {:?}", p.bar);
        }
        assert!(p.play.right() < p.time.left());
        assert!(p.time.right() < p.timeline.left());
        assert!(p.timeline.right() < p.speaker.left());
        assert!(p.speaker.right() < p.volume.left());
        // A narrow window keeps a usable timeline.
        let narrow = layout(
            Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 400.0)),
            80.0,
        );
        assert!(narrow.timeline.width() >= 20.0);
    }

    #[test]
    fn the_timeline_maps_x_to_a_place() {
        let timeline = Rect::from_min_size(pos2(100.0, 0.0), vec2(200.0, 18.0));
        let minute = Duration::from_secs(60);
        assert_eq!(position_at(timeline, 100.0, minute), Duration::ZERO);
        assert_eq!(
            position_at(timeline, 200.0, minute),
            Duration::from_secs(30)
        );
        assert_eq!(position_at(timeline, 999.0, minute), minute);
        assert_eq!(position_at(timeline, -5.0, minute), Duration::ZERO);
    }

    #[test]
    fn the_clock_reads_like_a_player() {
        assert_eq!(clock(Duration::from_secs(7)), "0:07");
        assert_eq!(clock(Duration::from_secs(754)), "12:34");
        assert_eq!(clock(Duration::from_secs(3723)), "1:02:03");
    }
}
