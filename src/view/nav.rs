//! Moving through the view: past the pinned photo, the four-up view's window.

/// `index`, or – if that is the pinned photo (compare mode) – its neighbour in `direction`,
/// else the other one. `None` if nothing but the pinned photo is left.
pub fn skip_pinned(
    len: usize,
    index: usize,
    pinned: Option<usize>,
    direction: isize,
) -> Option<usize> {
    if index >= len {
        return None;
    }
    if Some(index) != pinned {
        return Some(index);
    }
    let valid = |i: Option<usize>| i.filter(|&i| i < len);
    valid(index.checked_add_signed(direction)).or(valid(index.checked_add_signed(-direction)))
}

/// How many photos the four-up view shows.
pub const QUAD: usize = 4;

/// The first of the four photos on screen in the four-up view (`Shift+C`): the window keeps its
/// place while `current` is in it and moves by a row (two photos) when `current` leaves it, so
/// the photos don't jump at every step; at the end it shows the last four.
pub fn quad_start(start: usize, current: usize, len: usize) -> usize {
    let mut start = start;
    while current < start {
        start = start.saturating_sub(2);
    }
    while current >= start + QUAD {
        start += 2;
    }
    start.min(len.saturating_sub(QUAD))
}
