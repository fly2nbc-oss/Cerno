//! Top N: the best photos of the folder, one per series first.

use super::*;

/// The best `n` photos of what the other filters leave, for "Top N". A photo's value is the
/// mean of what is known about it, each 0..=1: its own stars (else the For-you prediction),
/// the aesthetics (`aesthetic::as_percent`, the info bar's scale) and the subject sharpness
/// within the folder. Rejected, probably blurry and incomplete photos, copies, videos and
/// photos without any value never count. Round one takes the best photo of every series and every photo
/// outside one, best first; round two the second best of each series, and so on – a burst
/// can't fill the list with look-alikes. The caller keeps the result: picking again after
/// every mark would slip the next photo into a rejected one's place unnoticed.
pub fn pick_top(
    all: &[PathBuf],
    options: ViewOptions,
    facts: impl Fn(&Path) -> Option<Facts>,
    session_ratings: &HashMap<PathBuf, Rating>,
    session_labels: &HashMap<PathBuf, Option<Label>>,
    hidden: impl Fn(&Path) -> bool,
    n: usize,
) -> HashSet<PathBuf> {
    let candidates = build(
        all,
        ViewOptions {
            top: None,
            media: Media::Photos,
            ..options
        },
        &facts,
        session_ratings,
        session_labels,
        &hidden,
    );
    let scores: Vec<Scores> = all
        .iter()
        .filter_map(|p| facts(p))
        .map(|f| f.scores)
        .collect();
    let percentiles = Percentiles::from_scores(scores.iter());
    // Each series is one group, each photo outside a series a group of its own.
    let mut groups: Vec<Vec<(f32, &PathBuf)>> = Vec::new();
    let mut of_series: HashMap<u32, usize> = HashMap::new();
    for (i, path) in candidates.paths.iter().enumerate() {
        if candidates.duplicate_of[i].is_some() {
            continue;
        }
        let Some(known) = facts(path) else {
            continue;
        };
        let rating = session_ratings.get(path).copied().unwrap_or(known.rating);
        if rating == Rating::Rejected
            || known.deleted
            || percentiles.is_blurry(&known.scores)
            || known.scores.truncated == Some(true)
        {
            continue;
        }
        let Some(value) = top_value(rating, &known, &percentiles) else {
            continue;
        };
        let group = match candidates.series[i] {
            Some(place) => *of_series.entry(place.id).or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            }),
            None => {
                groups.push(Vec::new());
                groups.len() - 1
            }
        };
        groups[group].push((value, path));
    }
    for group in &mut groups {
        group.sort_by(|a, b| b.0.total_cmp(&a.0));
    }
    let mut picked = HashSet::new();
    for round in 0.. {
        let mut best: Vec<(f32, &PathBuf)> = groups
            .iter()
            .filter_map(|group| group.get(round).copied())
            .collect();
        if best.is_empty() {
            break;
        }
        best.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, path) in best {
            if picked.len() == n {
                return picked;
            }
            picked.insert(path.clone());
        }
    }
    picked
}

/// One photo's value for Top N, 0..=1: the mean of what is known – none of it, no value.
pub(super) fn top_value(rating: Rating, facts: &Facts, percentiles: &Percentiles) -> Option<f32> {
    let stars = match rating {
        Rating::Stars(n) => Some(f32::from(n) / 5.0),
        Rating::Unrated => facts.personal.map(|p| p / 5.0),
        Rating::Rejected => None,
    };
    let aesthetics = aesthetic::combined(facts.scores.aesthetic, facts.scores.aesthetic25)
        .map(aesthetic::as_percent);
    let sharpness = percentiles.subject(&facts.scores).map(|(p, _)| p);
    let known: Vec<f32> = [stars, aesthetics, sharpness]
        .into_iter()
        .flatten()
        .collect();
    (!known.is_empty()).then(|| known.iter().sum::<f32>() / known.len() as f32)
}
