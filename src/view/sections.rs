//! What the scene view (`F8`) lays out: its scenes, with every run of single-photo scenes
//! collected under one header (the user's decision of 2026-11-09: two thirds of the scenes on
//! the author's index are one photo, and a row each made the view several times longer than
//! the grid). The scenes themselves stay as `view::scenes` cuts them; nothing is folded away.

use std::ops::Range;

use super::Scene;

/// One block of the scene view: a scene of two or more photos, a run of single photos, or the
/// photos without a capture time.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// View indices.
    pub range: Range<usize>,
    /// The first and the last capture time; `None` for the photos without one.
    pub time: Option<(i64, i64)>,
    /// The cameras, in the order their photos come (empty for collected single photos).
    pub cameras: Vec<u64>,
    /// Single photos collected: each is a scene of one.
    pub singles: bool,
}

/// The scenes as the scene view shows them: a run of consecutive single-photo scenes with a
/// capture time – also a run of one, so the headers stay alike – becomes one section; every
/// other scene stays one. The photos without a capture time stay their own section.
pub fn sections(scenes: &[Scene]) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for scene in scenes {
        let single = scene.range.len() == 1 && scene.time.is_some();
        if single
            && let Some(last) = out.last_mut().filter(|s| s.singles)
            && let (Some((start, end)), Some((from, to))) = (last.time, scene.time)
        {
            last.range.end = scene.range.end;
            last.time = Some((start.min(from), end.max(to)));
            continue;
        }
        out.push(Section {
            range: scene.range.clone(),
            time: scene.time,
            cameras: if single {
                Vec::new()
            } else {
                scene.cameras.clone()
            },
            singles: single,
        });
    }
    out
}

/// Which section of `ranges` holds view index `index`.
pub fn section_of(ranges: &[Range<usize>], index: usize) -> Option<usize> {
    let at = ranges.partition_point(|r| r.end <= index);
    ranges.get(at).filter(|r| r.contains(&index)).map(|_| at)
}

/// Page Down (`down`) / Page Up in the scene view: the next section's first photo; back, the
/// current section's first photo, or – already there – the previous section's. Collected
/// single photos are one stop. At either end nothing moves.
pub fn section_step(ranges: &[Range<usize>], current: usize, down: bool) -> usize {
    let Some(at) = section_of(ranges, current) else {
        return current;
    };
    if down {
        ranges.get(at + 1).map_or(current, |r| r.start)
    } else if current > ranges[at].start {
        ranges[at].start
    } else {
        at.checked_sub(1)
            .map_or(current, |before| ranges[before].start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(range: Range<usize>, time: Option<(i64, i64)>) -> Scene {
        Scene {
            range,
            time,
            cameras: vec![1],
        }
    }

    /// Start, end and whether singles were collected.
    fn shape(sections: &[Section]) -> Vec<(usize, usize, bool)> {
        sections
            .iter()
            .map(|s| (s.range.start, s.range.end, s.singles))
            .collect()
    }

    #[test]
    fn single_photos_in_a_row_share_a_section() {
        let at = |t| Some((t, t));
        let scenes = [
            scene(0..1, at(0)),
            scene(1..2, at(500_000)),
            scene(2..3, at(900_000)),
            scene(3..6, Some((1_000_000, 1_010_000))),
            // One single photo between two scenes: a section of one, headed alike.
            scene(6..7, at(2_000_000)),
            scene(7..9, Some((3_000_000, 3_001_000))),
            scene(9..10, at(4_000_000)),
            // The photos without a capture time stay apart, also a single one.
            scene(10..11, None),
        ];
        let out = sections(&scenes);
        assert_eq!(
            shape(&out),
            [
                (0, 3, true),
                (3, 6, false),
                (6, 7, true),
                (7, 9, false),
                (9, 10, true),
                (10, 11, false),
            ]
        );
        assert_eq!(out[0].time, Some((0, 900_000)), "first to last single");
        assert!(out[0].cameras.is_empty());
        assert_eq!(out[1].cameras, [1], "a scene keeps its cameras");
        assert_eq!(out[5].time, None);
        assert!(sections(&[]).is_empty());
    }

    #[test]
    fn page_keys_jump_from_section_to_section() {
        let all = [0..3, 3..4, 4..9];
        assert_eq!(section_of(&all, 0), Some(0));
        assert_eq!(section_of(&all, 3), Some(1));
        assert_eq!(section_of(&all, 8), Some(2));
        assert_eq!(section_of(&all, 9), None);
        assert_eq!(section_step(&all, 1, true), 3);
        assert_eq!(section_step(&all, 3, true), 4);
        assert_eq!(section_step(&all, 6, true), 6, "the last: nothing moves");
        assert_eq!(section_step(&all, 6, false), 4, "back to its start first");
        assert_eq!(section_step(&all, 4, false), 3);
        assert_eq!(section_step(&all, 3, false), 0);
        assert_eq!(section_step(&all, 0, false), 0, "the first: nothing moves");
    }
}
