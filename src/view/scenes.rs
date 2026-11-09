//! Scenes for the scene view (`F8`) – the one place where photos are grouped beyond series
//! (the user's decision of 2026-11-09, ticket CER-6). The single view, the filmstrip, the grid
//! and Top N never see them.
//!
//! The view arrives in capture-time order. Series (one camera, at most 2 s apart) and single
//! photos are the blocks; a block joins the scene before it only when it follows within
//! [`SCENE_GAP_MS`], keeps the scene within [`SCENE_SPAN_MS`], and its CLIP embedding is at
//! least [`SIMILAR_MIN`] alike the scene's **first** block – compared with the first, not the
//! last, so A≈B and B≈C never chain A and C together. A block without an embedding stays a
//! scene of its own (its series is all that groups it). Photos without a capture time come
//! last in the view: each run of them is one scene, *without capture time*.

use std::ops::Range;

use super::SIMILAR_MIN;

/// A block joins the scene only when it starts at most this long after the scene's last
/// photo – longer is a hard cut. Measured 2026-11-09 on the author's index (96 folders, 7263
/// series and single photos with a capture time and an embedding): of neighbouring blocks at
/// least 0.85 alike were 77 % within 5 s, 57 % at 5–15 s, 45 % at 15–30 s, 34 % at 30–60 s,
/// 30 % at 1–2 min, 19 % at 2–5 min and 2.5 % more than an hour apart; past 2 min a longer
/// window joined few more (5046 scenes at 2 min, 5207 at 1 min, 4959 at 5 min).
pub const SCENE_GAP_MS: i64 = 120_000;
/// No scene spans more than this from its first photo to its last – a guard against a slow
/// drift, not a tuned value: at a 2 min gap 10, 20, 30 and 60 min and no cap at all gave the
/// same 5046 scenes; 5 min would have cut the largest (92 photos in 5.4 min, a long series).
pub const SCENE_SPAN_MS: i64 = 600_000;

/// What the scenes look at for one photo of the view.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScenePhoto<'a> {
    /// Capture time, camera offsets included.
    pub taken: Option<i64>,
    /// The series' id (`SeriesPlace::id`) when the photo is in one.
    pub series: Option<u32>,
    /// `metadata::camera_id` of the camera model.
    pub camera: Option<u64>,
    /// The CLIP embedding, as the analysis stores it (not normalised).
    pub embedding: Option<&'a [f32]>,
}

/// One scene: a run of the view.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// View indices.
    pub range: Range<usize>,
    /// The first and the last capture time; `None` for the photos without one.
    pub time: Option<(i64, i64)>,
    /// The cameras, in the order their photos come; without the photos that name none.
    pub cameras: Vec<u64>,
}

/// A series or a single photo, the unit scenes are made of.
struct Block {
    range: Range<usize>,
    start: i64,
    end: i64,
    /// The normalised mean of its photos' normalised embeddings.
    embedding: Option<Vec<f32>>,
}

/// The view (`photos`, in capture-time order) cut into scenes: every photo in exactly one,
/// in the view's order. A single photo is a scene of one.
pub fn scenes(photos: &[ScenePhoto<'_>]) -> Vec<Scene> {
    let mut out: Vec<Scene> = Vec::new();
    // The scene being built: its first block's embedding.
    let mut anchor: Option<Vec<f32>> = None;
    let mut index = 0;
    while index < photos.len() {
        if photos[index].taken.is_none() {
            let end = run_end(photos, index, |p| p.taken.is_none());
            out.push(scene(photos, index..end, None));
            anchor = None;
            index = end;
            continue;
        }
        let block = block_at(photos, index);
        index = block.range.end;
        if let (Some(last), Some(first)) = (out.last_mut(), anchor.as_deref())
            && let Some((start, end)) = last.time
            && block.start - end <= SCENE_GAP_MS
            && block.end.max(end) - start <= SCENE_SPAN_MS
            && block
                .embedding
                .as_deref()
                .is_some_and(|e| dot(first, e) >= SIMILAR_MIN)
        {
            last.range.end = block.range.end;
            last.time = Some((start.min(block.start), end.max(block.end)));
            add_cameras(&mut last.cameras, &photos[block.range]);
            continue;
        }
        out.push(scene(
            photos,
            block.range.clone(),
            Some((block.start, block.end)),
        ));
        anchor = block.embedding;
    }
    out
}

fn scene(photos: &[ScenePhoto<'_>], range: Range<usize>, time: Option<(i64, i64)>) -> Scene {
    let mut cameras = Vec::new();
    add_cameras(&mut cameras, &photos[range.clone()]);
    Scene {
        range,
        time,
        cameras,
    }
}

fn add_cameras(cameras: &mut Vec<u64>, photos: &[ScenePhoto<'_>]) {
    for camera in photos.iter().filter_map(|p| p.camera) {
        if !cameras.contains(&camera) {
            cameras.push(camera);
        }
    }
}

/// The first index after `start` whose photo is no longer `alike`.
fn run_end(
    photos: &[ScenePhoto<'_>],
    start: usize,
    alike: impl Fn(&ScenePhoto<'_>) -> bool,
) -> usize {
    start + photos[start..].iter().take_while(|p| alike(p)).count()
}

/// The series starting at `start` (its photos stand together in capture-time order), or the
/// single photo there.
fn block_at(photos: &[ScenePhoto<'_>], start: usize) -> Block {
    let end = match photos[start].series {
        Some(id) => run_end(photos, start, |p| p.series == Some(id) && p.taken.is_some()),
        None => start + 1,
    };
    let members = &photos[start..end];
    let times = members.iter().filter_map(|p| p.taken);
    let start_ms = times.clone().min().unwrap_or(0);
    let end_ms = times.max().unwrap_or(start_ms);
    let mut sum: Option<Vec<f32>> = None;
    for embedding in members.iter().filter_map(|p| p.embedding) {
        let Some(unit) = normalised(embedding) else {
            continue;
        };
        match &mut sum {
            Some(sum) if sum.len() == unit.len() => {
                sum.iter_mut().zip(&unit).for_each(|(s, u)| *s += u);
            }
            Some(_) => {}
            None => sum = Some(unit),
        }
    }
    Block {
        range: start..end,
        start: start_ms,
        end: end_ms,
        embedding: sum.as_deref().and_then(normalised),
    }
}

fn normalised(v: &[f32]) -> Option<Vec<f32>> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    (norm > f32::EPSILON).then(|| v.iter().map(|x| x / norm).collect())
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return f32::MIN;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Which scene holds view index `index`.
pub fn scene_of(scenes: &[Scene], index: usize) -> Option<usize> {
    let at = scenes.partition_point(|s| s.range.end <= index);
    scenes
        .get(at)
        .filter(|s| s.range.contains(&index))
        .map(|_| at)
}

/// Page Down (`down`) / Page Up in the scene view: the next scene's first photo; back, the
/// current scene's first photo, or – already there – the previous scene's. At either end
/// nothing moves.
pub fn scene_step(scenes: &[Scene], current: usize, down: bool) -> usize {
    let Some(at) = scene_of(scenes, current) else {
        return current;
    };
    if down {
        scenes.get(at + 1).map_or(current, |s| s.range.start)
    } else if current > scenes[at].range.start {
        scenes[at].range.start
    } else {
        at.checked_sub(1)
            .map_or(current, |before| scenes[before].range.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    /// Unit vectors: `alike(a, b)` is their cosine.
    fn direction(angle: f32) -> Vec<f32> {
        vec![angle.cos(), angle.sin(), 0.0]
    }

    /// The angle whose cosine is `cos`.
    fn apart(cos: f32) -> f32 {
        cos.acos()
    }

    fn photo(taken: i64, embedding: &[f32]) -> ScenePhoto<'_> {
        ScenePhoto {
            taken: Some(taken),
            series: None,
            camera: Some(1),
            embedding: Some(embedding),
        }
    }

    /// Start and end of each scene (a range of one would trip clippy's array lint).
    fn ranges(scenes: &[Scene]) -> Vec<(usize, usize)> {
        scenes
            .iter()
            .map(|s| (s.range.start, s.range.end))
            .collect()
    }

    #[test]
    fn a_single_photo_is_a_scene_of_one() {
        let e = direction(0.0);
        let out = scenes(&[photo(0, &e)]);
        assert_eq!(ranges(&out), [(0, 1)]);
        assert_eq!(out[0].time, Some((0, 0)));
        assert_eq!(out[0].cameras, [1]);
        assert!(scenes(&[]).is_empty());
    }

    #[test]
    fn alike_photos_close_in_time_are_one_scene_a_gap_cuts() {
        let e = direction(0.0);
        let close = direction(apart(0.95));
        let photos = [
            photo(0, &e),
            photo(30_000, &close),
            photo(30_000 + SCENE_GAP_MS, &e),
            // Just over the gap: a new scene, however alike.
            photo(30_000 + 2 * SCENE_GAP_MS + 1, &e),
        ];
        let out = scenes(&photos);
        assert_eq!(ranges(&out), [(0, 3), (3, 4)]);
        assert_eq!(out[0].time, Some((0, 30_000 + SCENE_GAP_MS)));
    }

    #[test]
    fn different_content_is_another_scene() {
        let e = direction(0.0);
        let other = direction(apart(0.80));
        let out = scenes(&[photo(0, &e), photo(5_000, &other)]);
        assert_eq!(ranges(&out), [(0, 1), (1, 2)]);
    }

    /// A≈B and B≈C, but A and C are not alike: C starts a new scene – every block is compared
    /// with the scene's first one, so similarity never chains.
    #[test]
    fn similarity_does_not_chain() {
        let step = apart(0.90);
        let (a, b, c) = (direction(0.0), direction(step), direction(2.0 * step));
        assert!(dot(&a, &b) >= SIMILAR_MIN && dot(&b, &c) >= SIMILAR_MIN);
        assert!(dot(&a, &c) < SIMILAR_MIN);
        let out = scenes(&[photo(0, &a), photo(10_000, &b), photo(20_000, &c)]);
        assert_eq!(ranges(&out), [(0, 2), (2, 3)]);
    }

    #[test]
    fn a_scene_spans_at_most_the_cap() {
        let e = direction(0.0);
        let photos: Vec<ScenePhoto<'_>> = (0..15).map(|k| photo(k * MIN, &e)).collect();
        let out = scenes(&photos);
        // 0 … 10 min fit, 11 min would make the scene too long.
        assert_eq!(ranges(&out), [(0, 11), (11, 15)]);
        assert!(
            out.iter()
                .all(|s| s.time.is_some_and(|(a, b)| b - a <= SCENE_SPAN_MS))
        );
    }

    /// Without embeddings (no CLIP model) only the series group: a series is one scene, the
    /// photos around it each their own.
    #[test]
    fn without_embeddings_only_series_group() {
        let plain = |taken, series| ScenePhoto {
            taken: Some(taken),
            series,
            camera: Some(1),
            embedding: None,
        };
        let photos = [
            plain(0, None),
            plain(10_000, Some(1)),
            plain(11_000, Some(1)),
            plain(12_000, Some(1)),
            plain(15_000, None),
        ];
        assert_eq!(ranges(&scenes(&photos)), [(0, 1), (1, 4), (4, 5)]);
        // A photo with an embedding next to a series without one stays apart too.
        let e = direction(0.0);
        let mixed = [photo(0, &e), plain(1_000, Some(2)), plain(2_000, Some(2))];
        assert_eq!(ranges(&scenes(&mixed)), [(0, 1), (1, 3)]);
    }

    /// A series is one block: its mean embedding decides, and its photos – sharpest first in
    /// the view, so not in time order – stay together.
    #[test]
    fn a_series_joins_as_a_whole() {
        let e = direction(0.0);
        let close = direction(apart(0.92));
        let series = |taken, embedding| ScenePhoto {
            series: Some(7),
            ..photo(taken, embedding)
        };
        let photos = [
            photo(0, &e),
            series(21_000, &close),
            series(20_000, &close),
            series(22_000, &e),
            photo(40_000, &e),
        ];
        let out = scenes(&photos);
        assert_eq!(ranges(&out), [(0, 5)]);
        assert_eq!(out[0].time, Some((0, 40_000)));
    }

    /// Two cameras firing together make two series; when alike they are one scene, which
    /// names both cameras.
    #[test]
    fn two_cameras_share_a_scene() {
        let e = direction(0.0);
        let photos = [
            ScenePhoto {
                series: Some(1),
                ..photo(0, &e)
            },
            ScenePhoto {
                series: Some(1),
                ..photo(1_000, &e)
            },
            ScenePhoto {
                series: Some(2),
                camera: Some(9),
                ..photo(500, &e)
            },
            ScenePhoto {
                camera: None,
                ..photo(3_000, &e)
            },
        ];
        let out = scenes(&photos);
        assert_eq!(ranges(&out), [(0, 4)]);
        assert_eq!(out[0].cameras, [1, 9]);
        assert_eq!(out[0].time, Some((0, 3_000)));
    }

    /// Photos without a capture time come last in the view: together one scene without a time.
    #[test]
    fn photos_without_a_time_are_one_scene_at_the_end() {
        let e = direction(0.0);
        let untimed = ScenePhoto {
            taken: None,
            ..photo(0, &e)
        };
        let photos = [photo(0, &e), photo(1_000, &e), untimed, untimed, untimed];
        let out = scenes(&photos);
        assert_eq!(ranges(&out), [(0, 2), (2, 5)]);
        assert_eq!(out[1].time, None);
    }

    #[test]
    fn every_photo_is_in_exactly_one_scene() {
        let e = direction(0.0);
        let far = direction(1.2);
        let photos: Vec<ScenePhoto<'_>> = (0..40)
            .map(|k| match k % 7 {
                0 => ScenePhoto {
                    taken: None,
                    ..photo(0, &e)
                },
                3 => photo(k * 50_000, &far),
                5 => ScenePhoto {
                    embedding: None,
                    ..photo(k * 50_000, &e)
                },
                _ => photo(k * 50_000, &e),
            })
            .collect();
        let out = scenes(&photos);
        let mut next = 0;
        for scene in &out {
            assert_eq!(scene.range.start, next, "{out:?}");
            assert!(!scene.range.is_empty());
            next = scene.range.end;
        }
        assert_eq!(next, photos.len());
    }

    #[test]
    fn page_keys_jump_from_scene_to_scene() {
        let scene = |range: Range<usize>| Scene {
            range,
            time: None,
            cameras: Vec::new(),
        };
        let all = [scene(0..3), scene(3..4), scene(4..9)];
        assert_eq!(scene_of(&all, 0), Some(0));
        assert_eq!(scene_of(&all, 3), Some(1));
        assert_eq!(scene_of(&all, 8), Some(2));
        assert_eq!(scene_of(&all, 9), None);
        assert_eq!(scene_step(&all, 1, true), 3);
        assert_eq!(scene_step(&all, 3, true), 4);
        assert_eq!(
            scene_step(&all, 6, true),
            6,
            "the last scene: nothing moves"
        );
        assert_eq!(
            scene_step(&all, 6, false),
            4,
            "back to the scene's start first"
        );
        assert_eq!(scene_step(&all, 4, false), 3);
        assert_eq!(scene_step(&all, 3, false), 0);
        assert_eq!(scene_step(&all, 0, false), 0, "the first: nothing moves");
    }
}
