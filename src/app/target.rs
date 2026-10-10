//! The decode size: the photo area in physical pixels, so a fitted photo is drawn pixel for
//! pixel. A new size is taken once the area has kept it for [`TARGET_SETTLE`]; the single
//! view's size is saved, so the next start decodes the first photo for it at once.

use std::time::{Duration, Instant};

use eframe::egui::{self, Rect};

use crate::db::Db;
use crate::ui::viewer;

use super::CernoApp;

/// Decode size before the window exists, so the first photo decodes while the GPU starts up.
/// Covers screens up to 4K; once the photo area is known, that photo is decoded again for it.
/// Used only until the photo area has been seen once: then the last one is saved
/// ([`AREA_SETTING`]) and the first photo decodes for it – nothing twice.
const START_TARGET: [u32; 2] = [3840, 2160];

/// The last photo area's decode size, `W×H` in physical pixels.
const AREA_SETTING: &str = "photo_area";

/// `3840x2054` → the decode size; anything else is ignored.
fn parse_area(text: &str) -> Option<[u32; 2]> {
    let (width, height) = text.split_once('x')?;
    let size = [width.parse().ok()?, height.parse().ok()?];
    size.iter()
        .all(|&side| (64..=16384).contains(&side))
        .then_some(size)
}

/// How long the photo area must keep its size before photos are decoded for it: a window
/// dragged larger, or maximized after the first frame, would otherwise decode them at every
/// step.
const TARGET_SETTLE: Duration = Duration::from_millis(200);

pub(super) struct Target {
    /// The size the loader decodes for, once the photo area is known.
    pub(super) size: Option<[u32; 2]>,
    /// What the loader decodes for before `size` is known: the saved area, else 4K.
    pub(super) start: [u32; 2],
    /// A new decode size and since when the photo area has had it (see `TARGET_SETTLE`).
    pending: Option<([u32; 2], Instant)>,
}

/// What `Target::settle` decided about the area's size.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Settle {
    /// The loader has it already.
    Kept,
    /// Not kept long enough yet: look again after this.
    Wait(Duration),
    /// Decode for it now; `first`: the first size since the start (the prefetch starts).
    Take { first: bool },
}

impl Target {
    /// The saved area as the start size; the area itself is not known yet.
    pub(super) fn restore(db: &Db) -> Self {
        Self {
            size: None,
            start: db
                .setting(AREA_SETTING)
                .and_then(|text| parse_area(&text))
                .unwrap_or(START_TARGET),
            pending: None,
        }
    }

    /// The photo area wants `wanted` at `now`: taken once it has kept it for
    /// [`TARGET_SETTLE`] – at start-up at once when it is the start size the first photo was
    /// decoded for.
    pub(super) fn settle(&mut self, wanted: [u32; 2], now: Instant) -> Settle {
        if self.size == Some(wanted) {
            self.pending = None;
            return Settle::Kept;
        }
        let since = match self.pending {
            Some((pending, since)) if pending == wanted => since,
            _ => {
                self.pending = Some((wanted, now));
                now
            }
        };
        let waited = now - since;
        let known = self.size.is_none() && wanted == self.start;
        if waited < TARGET_SETTLE && !known {
            return Settle::Wait(TARGET_SETTLE - waited);
        }
        self.pending = None;
        Settle::Take {
            first: self.size.replace(wanted).is_none(),
        }
    }
}

impl CernoApp {
    /// Decode size: the photo area in physical pixels (`viewer::decode_size`), so a fitted
    /// photo is drawn pixel for pixel (`Target::settle`). The first one also starts the
    /// prefetch – after it, so the neighbours are decoded for the area and not for the
    /// start-up guess.
    pub(super) fn update_target(&mut self, ctx: &egui::Context, areas: &[Rect]) {
        let max_side = ctx.input(|i| i.max_texture_side) as u32;
        let Some(wanted) = viewer::decode_size(areas, ctx.pixels_per_point(), max_side) else {
            return;
        };
        match self.target.settle(wanted, Instant::now()) {
            Settle::Kept => {}
            Settle::Wait(after) => ctx.request_repaint_after(after),
            Settle::Take { first } => {
                self.loader.set_target(wanted);
                if first {
                    self.loader.start_prefetch();
                }
                // The single view's size only: Cerno starts in it.
                if wanted != self.target.start && self.pinned.is_none() && self.quad.is_none() {
                    self.target.start = wanted;
                    self.db
                        .put_setting(AREA_SETTING, &format!("{}x{}", wanted[0], wanted[1]));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(start: [u32; 2]) -> Target {
        Target {
            size: None,
            start,
            pending: None,
        }
    }

    #[test]
    fn the_start_size_is_taken_at_once() {
        let mut t = target([2560, 1300]);
        let now = Instant::now();
        assert_eq!(t.settle([2560, 1300], now), Settle::Take { first: true });
        assert_eq!(t.settle([2560, 1300], now), Settle::Kept);
    }

    #[test]
    fn a_new_size_waits_until_it_stays() {
        let mut t = target(START_TARGET);
        let now = Instant::now();
        assert_eq!(t.settle([1280, 700], now), Settle::Wait(TARGET_SETTLE));
        let later = now + Duration::from_millis(50);
        assert_eq!(
            t.settle([1280, 700], later),
            Settle::Wait(TARGET_SETTLE - Duration::from_millis(50))
        );
        // Another size starts the wait again.
        assert_eq!(t.settle([1300, 700], later), Settle::Wait(TARGET_SETTLE));
        let settled = later + TARGET_SETTLE;
        assert_eq!(t.settle([1300, 700], settled), Settle::Take { first: true });
        assert_eq!(
            t.settle([1400, 700], settled),
            Settle::Wait(TARGET_SETTLE),
            "the next size waits too"
        );
        assert_eq!(
            t.settle([1400, 700], settled + TARGET_SETTLE),
            Settle::Take { first: false }
        );
    }

    #[test]
    fn a_saved_area_must_be_plausible() {
        assert_eq!(parse_area("3840x2054"), Some([3840, 2054]));
        assert_eq!(parse_area("10x2054"), None);
        assert_eq!(parse_area("3840×2054"), None);
        assert_eq!(parse_area(""), None);
    }
}
