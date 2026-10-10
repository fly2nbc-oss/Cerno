//! Time series: photos at most 2 s apart from one camera, the sharpest first.

use super::*;

/// Series of photos from one camera that follow each other by at most [`SERIES_GAP_MS`] – two
/// cameras firing at the same moment make two series. Photos without a camera model only form
/// series with each other. A single photo is not a series. Within a series the sharpest
/// non-rejected photo is index 1; rejected and not-yet-measured photos come last.
pub(super) fn series_places(shown: &[Entry<'_>]) -> Vec<Option<SeriesPlace>> {
    let mut timed: Vec<usize> = shown
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.taken.is_some())
        .map(|(index, _)| index)
        .collect();
    timed.sort_by(|&a, &b| {
        (shown[a].camera, shown[a].taken, a).cmp(&(shown[b].camera, shown[b].taken, b))
    });

    let mut groups: Vec<Vec<usize>> = Vec::new();
    for index in timed {
        let taken = shown[index].taken.unwrap_or(0);
        if let Some(group) = groups.last_mut()
            && let Some(&prev) = group.last()
            && shown[prev].camera == shown[index].camera
            && taken - shown[prev].taken.unwrap_or(0) <= SERIES_GAP_MS
        {
            group.push(index);
            continue;
        }
        groups.push(vec![index]);
    }

    let mut places = vec![None; shown.len()];
    let mut id = 0u32;
    for mut group in groups {
        if group.len() < 2 {
            continue;
        }
        let start_ms = group
            .iter()
            .filter_map(|&index| shown[index].taken)
            .min()
            .unwrap_or(0);
        group.sort_by(|&a, &b| series_rank(&shown[a], &shown[b]).then(a.cmp(&b)));
        let len = group.len() as u32;
        id += 1;
        for (rank, index) in group.into_iter().enumerate() {
            places[index] = Some(SeriesPlace {
                id,
                index: rank as u32 + 1,
                len,
                start_ms,
            });
        }
    }
    places
}

/// Rejected last, then unmeasured, then the sharpest first.
pub(super) fn series_rank(a: &Entry<'_>, b: &Entry<'_>) -> std::cmp::Ordering {
    let key = |entry: &Entry<'_>| (entry.rating == Rating::Rejected, entry.sharp.is_none());
    key(a).cmp(&key(b)).then_with(|| match (a.sharp, b.sharp) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        _ => std::cmp::Ordering::Equal,
    })
}

pub(super) fn taken_order(
    shown: &[Entry<'_>],
    places: &[Option<SeriesPlace>],
    a: usize,
    b: usize,
) -> std::cmp::Ordering {
    // The series id keeps two cameras' series apart when they start in the same millisecond
    // (files with whole seconds only).
    let key = |index: usize| -> Option<(i64, u32, u32)> {
        let taken = shown[index].taken?;
        Some(
            places[index]
                .map(|place| (place.start_ms, place.id, place.index))
                .unwrap_or((taken, 0, 0)),
        )
    };
    match (key(a), key(b)) {
        (Some(x), Some(y)) => x.cmp(&y).then(a.cmp(&b)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(&b),
    }
}
