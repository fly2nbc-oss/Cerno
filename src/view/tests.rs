use super::*;

/// The four-up window stays while the frame moves inside it, then moves a row.
#[test]
fn the_four_up_window_moves_a_row_at_a_time() {
    assert_eq!(quad_start(0, 3, 10), 0, "inside: stays");
    assert_eq!(quad_start(0, 4, 10), 2, "one past: a row on");
    assert_eq!(quad_start(4, 3, 10), 2, "one before: a row back");
    assert_eq!(quad_start(0, 9, 10), 6, "a jump to the end: the last four");
    assert_eq!(quad_start(6, 0, 10), 0);
    assert_eq!(quad_start(5, 5, 7), 3, "the last four, not fewer");
    assert_eq!(quad_start(0, 1, 3), 0, "fewer than four");
}

#[test]
fn navigation_steps_over_the_pinned_photo() {
    assert_eq!(skip_pinned(5, 2, None, 1), Some(2));
    assert_eq!(skip_pinned(5, 2, Some(2), 1), Some(3));
    assert_eq!(skip_pinned(5, 2, Some(2), -1), Some(1));
    // At the end there is only the way back.
    assert_eq!(skip_pinned(5, 4, Some(4), 1), Some(3));
    assert_eq!(skip_pinned(5, 0, Some(0), -1), Some(1));
    assert_eq!(skip_pinned(1, 0, Some(0), 1), None);
    assert_eq!(skip_pinned(0, 0, None, 1), None);
}

fn facts(
    rating: Rating,
    aesthetic: Option<f32>,
    sharpness: Option<f32>,
    personal: Option<f32>,
) -> Facts {
    Facts {
        rating,
        scores: Scores {
            sharpness,
            aesthetic,
            ..Scores::default()
        },
        personal,
        ..Facts::default()
    }
}

/// The same facts with a V2.5 score as well.
fn with_v25(mut facts: Facts, v25: f32) -> Facts {
    facts.scores.aesthetic25 = Some(v25);
    facts
}

fn fixture() -> (Vec<PathBuf>, HashMap<PathBuf, Facts>) {
    let all: Vec<PathBuf> = ["a", "b", "c", "d", "e"].map(PathBuf::from).to_vec();
    let known = HashMap::from([
        // Aesthetics: a (4.0 + 8.0) / 2 = 6.0, b LAION alone 6.5, c (5.0 + 4.0) / 2 = 4.5,
        // d V2.5 alone 5.5.
        (
            all[0].clone(),
            with_v25(
                facts(Rating::Stars(2), Some(4.0), Some(10.0), Some(1.0)),
                8.0,
            ),
        ),
        (
            all[1].clone(),
            facts(Rating::Unrated, Some(6.5), Some(500.0), None),
        ),
        (
            all[2].clone(),
            with_v25(
                facts(Rating::Stars(5), Some(5.0), Some(300.0), Some(4.5)),
                4.0,
            ),
        ),
        (
            all[3].clone(),
            with_v25(facts(Rating::Stars(3), None, Some(200.0), Some(3.0)), 5.5),
        ),
        // "e" not analysed yet
    ]);
    (all, known)
}

fn names(view: &[PathBuf]) -> String {
    view.iter().map(|p| p.to_string_lossy()).collect()
}

/// Only photos close enough stay, together with the boxes; one without an embedding can't
/// be judged and stays out.
#[test]
fn the_similar_filter_keeps_close_photos() {
    let (all, mut known) = fixture();
    for (name, similarity) in [("a", 1.0), ("b", 0.91), ("c", 0.6), ("d", SIMILAR_MIN)] {
        if let Some(facts) = known.get_mut(Path::new(name)) {
            facts.similarity = Some(similarity);
        }
    }
    let shown = |options: ViewOptions| {
        let view = build(
            &all,
            options,
            |p: &Path| known.get(p).copied(),
            &HashMap::new(),
            &HashMap::new(),
            |_| false,
        );
        names(&view.paths)
    };
    let mut options = ViewOptions {
        similar: true,
        ..ViewOptions::default()
    };
    assert_eq!(shown(options), "abd");
    options.filter.set(FilterKind::Stars(2), true);
    assert_eq!(shown(options), "a", "and the boxes still apply");
    assert_eq!(shown(ViewOptions::default()), "abcde");
    assert!(
        ViewOptions {
            similar: true,
            ..ViewOptions::default()
        }
        .depends_on_scores()
    );
}

#[test]
fn sorts_descending_with_unknown_last() {
    let (all, known) = fixture();
    let lookup = |p: &Path| known.get(p).copied();
    let sorted = |sort| {
        let options = ViewOptions {
            sort,
            ..ViewOptions::default()
        };
        names(
            &build(
                &all,
                options,
                lookup,
                &HashMap::new(),
                &HashMap::new(),
                |_| false,
            )
            .paths,
        )
    };
    assert_eq!(sorted(SortKey::Name), "abcde");
    assert_eq!(sorted(SortKey::Aesthetics), "badce");
    assert_eq!(sorted(SortKey::Personal), "cdabe");
    assert_eq!(sorted(SortKey::Sharpness), "bcdae");
    assert_eq!(sorted(SortKey::Rating), "cdabe");
}

#[test]
fn filters_by_rating_with_session_override() {
    let (all, known) = fixture();
    let lookup = |p: &Path| known.get(p).copied();
    let session = HashMap::from([
        (PathBuf::from("b"), Rating::Stars(4)),
        (PathBuf::from("c"), Rating::Unrated),
        (PathBuf::from("a"), Rating::Rejected),
    ]);
    let filtered = |kinds: &[FilterKind]| {
        let mut filter = PhotoFilter::default();
        for kind in kinds {
            filter.set(*kind, true);
        }
        let options = ViewOptions {
            filter,
            ..ViewOptions::default()
        };
        names(&build(&all, options, lookup, &session, &HashMap::new(), |_| false).paths)
    };
    // Session: a rejected, b 4 stars, c unrated, d stays 3 stars, e not analysed (unrated).
    assert_eq!(
        filtered(&[FilterKind::Stars(3), FilterKind::Stars(4)]),
        "bd"
    );
    assert_eq!(filtered(&[FilterKind::Stars(1), FilterKind::Stars(2)]), "");
    assert_eq!(filtered(&[FilterKind::Unrated]), "ce");
    assert_eq!(filtered(&[FilterKind::Rejected]), "a");
    assert_eq!(filtered(&[]), "abcde");
}

/// Within the rating group any ticked box, across groups all of them: blurry 2-star photos,
/// not every 2-star photo plus every blurry one.
#[test]
fn groups_go_together_boxes_in_a_group_either_way() {
    let (all, known) = fixture();
    let lookup = |p: &Path| known.get(p).copied();
    let names_of = |kinds: &[FilterKind]| {
        let mut filter = PhotoFilter::default();
        for kind in kinds {
            filter.set(*kind, true);
        }
        names(
            &build(
                &all,
                ViewOptions {
                    filter,
                    ..ViewOptions::default()
                },
                lookup,
                &HashMap::new(),
                &HashMap::new(),
                |_| false,
            )
            .paths,
        )
    };
    // "a" (2 stars) is the least sharp; "e" is not measured, so it is not blurry.
    assert_eq!(names_of(&[FilterKind::Blurry]), "a");
    assert_eq!(names_of(&[FilterKind::Stars(2), FilterKind::Blurry]), "a");
    assert_eq!(names_of(&[FilterKind::Stars(5), FilterKind::Blurry]), "");
    assert_eq!(
        names_of(&[FilterKind::Stars(5), FilterKind::Stars(2)]),
        "ac"
    );
}

/// People: a face found, or none; a photo the face detection hasn't seen is in neither box.
#[test]
fn people_boxes_need_the_face_detection() {
    let filter_of = |kinds: &[FilterKind]| {
        let mut filter = PhotoFilter::default();
        for kind in kinds {
            filter.set(*kind, true);
        }
        filter
    };
    let accepts = |kinds: &[FilterKind], faces: Option<u8>| {
        filter_of(kinds).accepts(Rating::Unrated, Quality::default(), None, faces, false)
    };
    assert!(accepts(&[FilterKind::People], Some(2)));
    assert!(!accepts(&[FilterKind::People], Some(0)));
    assert!(!accepts(&[FilterKind::People], None));
    assert!(accepts(&[FilterKind::NoPeople], Some(0)));
    assert!(!accepts(&[FilterKind::NoPeople], None));
    assert!(accepts(
        &[FilterKind::People, FilterKind::NoPeople],
        Some(0)
    ));
    assert!(
        !accepts(&[FilterKind::People, FilterKind::Stars(5)], Some(1)),
        "the rating group still counts"
    );
    assert!(accepts(&[], None));
    let stored = PhotoFilter::from_stored(&filter_of(&[FilterKind::NoPeople]).id());
    assert!(stored.contains(FilterKind::NoPeople) && !stored.contains(FilterKind::People));
}

/// Deleted photos show only through the 🗑 box, which stands for their rating – 🗑 + ✕ are
/// everything sorted out – and other groups still apply. They don't rank the folder's
/// sharpness or make a photo still there a duplicate.
#[test]
fn deleted_photos_show_only_through_their_box() {
    let (mut all, mut known) = fixture();
    // "x" is a deleted copy of "b" (same pixels) with 5 stars and a red label.
    all.push(PathBuf::from("x"));
    known.get_mut(Path::new("b")).unwrap().fingerprint = Some(7);
    known.insert(
        PathBuf::from("x"),
        Facts {
            rating: Rating::Stars(5),
            label: Some(Label::Red),
            fingerprint: Some(7),
            deleted: true,
            ..facts(Rating::Stars(5), None, Some(1.0), None)
        },
    );
    let lookup = |p: &Path| known.get(p).copied();
    let session = HashMap::from([(PathBuf::from("a"), Rating::Rejected)]);
    let view_of = |kinds: &[FilterKind]| {
        let mut filter = PhotoFilter::default();
        for kind in kinds {
            filter.set(*kind, true);
        }
        let options = ViewOptions {
            filter,
            ..ViewOptions::default()
        };
        build(&all, options, lookup, &session, &HashMap::new(), |_| false)
    };
    assert_eq!(
        names(&view_of(&[]).paths),
        "abcde",
        "hidden without the box"
    );
    assert_eq!(names(&view_of(&[FilterKind::Stars(5)]).paths), "c");
    assert_eq!(names(&view_of(&[FilterKind::Deleted]).paths), "x");
    assert_eq!(
        names(&view_of(&[FilterKind::Rejected, FilterKind::Deleted]).paths),
        "ax"
    );
    assert_eq!(
        names(&view_of(&[FilterKind::Deleted, FilterKind::Colour(Label::Red)]).paths),
        "x"
    );
    assert_eq!(
        names(&view_of(&[FilterKind::Deleted, FilterKind::Colour(Label::Blue)]).paths),
        ""
    );
    assert_eq!(
        names(&view_of(&[FilterKind::Blurry]).paths),
        "a",
        "x, blurrier still, would lift a out of the blurriest fifth"
    );
    let all_shown = view_of(&[FilterKind::Deleted, FilterKind::Unrated]);
    assert_eq!(names(&all_shown.paths), "bex");
    assert!(
        all_shown.duplicate_of.iter().all(Option::is_none),
        "b is no copy of a deleted photo"
    );
    let picked = pick_top(
        &all,
        ViewOptions::default(),
        lookup,
        &HashMap::new(),
        &HashMap::new(),
        |_| false,
        10,
    );
    assert!(!picked.contains(Path::new("x")), "never among the best");
    assert_eq!(
        PhotoFilter::from_stored("*rejected,deleted").id(),
        "*rejected,deleted"
    );
}

/// "without ✕" hides the rejected photos – session rejections too – and nothing else;
/// ✕ and it exclude each other, "Show all" ends it.
#[test]
fn without_rejected_hides_them_and_excludes_the_reject_box() {
    let (all, known) = fixture();
    let lookup = |p: &Path| known.get(p).copied();
    let session = HashMap::from([(PathBuf::from("a"), Rating::Rejected)]);
    let mut options = ViewOptions::default();
    options.toggle_hide_rejected();
    let shown = |options: ViewOptions| {
        names(&build(&all, options, lookup, &session, &HashMap::new(), |_| false).paths)
    };
    assert_eq!(shown(options), "bcde");
    assert!(options.is_filtered());
    let mut five = options;
    five.toggle_filter(FilterKind::Stars(5));
    assert_eq!(shown(five), "c", "together with the boxes");

    options.toggle_filter(FilterKind::Rejected);
    assert!(!options.hide_rejected, "✕ ends it");
    assert_eq!(shown(options), "a");
    options.toggle_hide_rejected();
    assert!(
        !options.filter.contains(FilterKind::Rejected),
        "and the other way round"
    );
    options.clear_filters();
    assert_eq!(options, ViewOptions::default());
}

/// A pasted file-name list keeps just its photos – analysed or not – together with the
/// boxes; "Show all" ends it.
#[test]
fn a_file_list_keeps_its_photos() {
    let (all, mut known) = fixture();
    for name in ["b", "c"] {
        known.get_mut(Path::new(name)).expect("known").listed = true;
    }
    known.insert(
        PathBuf::from("e"),
        Facts {
            listed: true,
            ..Facts::default()
        },
    );
    let lookup = |p: &Path| known.get(p).copied();
    let shown = |options: ViewOptions| {
        names(
            &build(
                &all,
                options,
                lookup,
                &HashMap::new(),
                &HashMap::new(),
                |_| false,
            )
            .paths,
        )
    };
    let mut options = ViewOptions {
        name_list: true,
        ..ViewOptions::default()
    };
    assert_eq!(shown(options), "bce");
    assert!(options.is_filtered());
    options.toggle_filter(FilterKind::Stars(5));
    assert_eq!(shown(options), "c", "together with the boxes");
    options.clear_filters();
    assert_eq!(options, ViewOptions::default());
}

#[test]
fn stored_filters_round_trip() {
    assert!(PhotoFilter::from_stored("").is_all());
    // Anything else that isn't a list of boxes (saved before 1.1) is no filter.
    assert!(PhotoFilter::from_stored("3").is_all());
    let exact = PhotoFilter::from_stored("*3");
    assert!(exact.contains(FilterKind::Stars(3)));
    assert!(!exact.contains(FilterKind::Stars(4)));
    let mixed = PhotoFilter::from_stored("*1,2,unrated,blurry,duplicate");
    assert_eq!(mixed.id(), "*1,2,unrated,blurry,duplicate");
    assert_eq!(PhotoFilter::default().id(), "");
    let red = PhotoFilter::from_stored("*red");
    assert!(red.contains(FilterKind::Colour(Label::Red)));
    assert!(!red.contains(FilterKind::Colour(Label::Blue)));
    assert_eq!(red.id(), "*red");
}

#[test]
fn colour_is_one_more_category() {
    let (all, known) = fixture();
    let lookup = |p: &Path| known.get(p).copied();
    let labels = HashMap::from([
        (PathBuf::from("a"), Some(Label::Red)),
        (PathBuf::from("c"), Some(Label::Green)),
    ]);
    let mut filter = PhotoFilter::default();
    filter.set(FilterKind::Colour(Label::Red), true);
    let options = ViewOptions {
        filter,
        ..ViewOptions::default()
    };
    assert_eq!(
        names(&build(&all, options, lookup, &HashMap::new(), &labels, |_| false,).paths),
        "a"
    );
    // Red and 5 stars: "a" is red but has 2 stars, "c" 5 stars but is green.
    let mut both = options;
    both.filter.set(FilterKind::Stars(5), true);
    assert_eq!(
        names(&build(&all, both, lookup, &HashMap::new(), &labels, |_| false).paths),
        ""
    );
    both.filter.set(FilterKind::Colour(Label::Green), true);
    assert_eq!(
        names(&build(&all, both, lookup, &HashMap::new(), &labels, |_| false).paths),
        "c"
    );
}

fn top_of(
    all: &[PathBuf],
    known: &HashMap<PathBuf, Facts>,
    session: &HashMap<PathBuf, Rating>,
    n: usize,
) -> String {
    let picked = pick_top(
        all,
        ViewOptions::default(),
        |p: &Path| known.get(p).copied(),
        session,
        &HashMap::new(),
        |_| false,
        n,
    );
    let mut names: Vec<String> = picked
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.concat()
}

/// Blurry and rejected photos and those without any value never count; own stars, the
/// aesthetics and the sharpness rank the rest.
#[test]
fn top_takes_the_best_and_leaves_out_what_never_counts() {
    let (all, known) = fixture();
    // a is the blurriest (and clearly soft), e has no values yet.
    assert_eq!(top_of(&all, &known, &HashMap::new(), 10), "bcd");
    // b: aesthetics 75 %, sharpest; c: 5 stars, 42 %, 67 %; d: 3 stars, 58 %, 33 %.
    assert_eq!(top_of(&all, &known, &HashMap::new(), 2), "bc");
    assert_eq!(top_of(&all, &known, &HashMap::new(), 0), "");
    let rejected = HashMap::from([(PathBuf::from("b"), Rating::Rejected)]);
    assert_eq!(top_of(&all, &known, &rejected, 2), "cd");
    // Half of b is grey: it is no best photo either.
    let mut cut = known.clone();
    if let Some(b) = cut.get_mut(Path::new("b")) {
        b.scores.truncated = Some(true);
    }
    assert_eq!(top_of(&all, &cut, &HashMap::new(), 2), "cd");
}

/// Incomplete files are a box of the quality group: any of blurry, duplicates and
/// incomplete, and the stars on top of that.
#[test]
fn incomplete_is_a_quality_box() {
    let mut filter = PhotoFilter::default();
    filter.set(FilterKind::Incomplete, true);
    let cut = Quality {
        incomplete: true,
        ..Quality::default()
    };
    let blurry = Quality {
        blurry: true,
        ..Quality::default()
    };
    assert!(filter.accepts(Rating::Unrated, cut, None, None, false));
    assert!(!filter.accepts(Rating::Unrated, Quality::default(), None, None, false));
    filter.set(FilterKind::Blurry, true);
    assert!(filter.accepts(Rating::Unrated, blurry, None, None, false));
    filter.set(FilterKind::Stars(5), true);
    assert!(!filter.accepts(Rating::Unrated, cut, None, None, false));
    assert!(filter.accepts(Rating::Stars(5), cut, None, None, false));
    assert_eq!(filter.id(), "*5,blurry,incomplete");
    assert_eq!(PhotoFilter::from_stored(&filter.id()), filter);
}

/// A photo's own stars stand in for the For-you prediction.
#[test]
fn own_stars_replace_the_prediction() {
    let same = |rating, personal| Facts {
        rating,
        personal: Some(personal),
        scores: Scores {
            sharpness: Some(500.0),
            aesthetic: Some(6.0),
            ..Scores::default()
        },
        ..Facts::default()
    };
    let all = vec![PathBuf::from("x"), PathBuf::from("y")];
    let known = HashMap::from([
        (all[0].clone(), same(Rating::Stars(5), 1.0)),
        (all[1].clone(), same(Rating::Unrated, 4.0)),
    ]);
    assert_eq!(top_of(&all, &known, &HashMap::new(), 1), "x");
    let unrated = HashMap::from([(all[0].clone(), Rating::Unrated)]);
    assert_eq!(top_of(&all, &known, &unrated, 1), "y");
}

/// Round one takes the best of each series and every single photo; only then the second
/// best of the series – a burst can't fill the list.
#[test]
fn top_takes_one_photo_per_series_first() {
    let photo = |taken: i64, aesthetic: f32, sharpness: f32| Facts {
        taken_ms: Some(taken),
        scores: Scores {
            sharpness: Some(sharpness),
            aesthetic: Some(aesthetic),
            ..Scores::default()
        },
        ..Facts::default()
    };
    let rows = [
        ("s1", photo(0, 8.0, 900.0)),
        ("s2", photo(1_000, 8.0, 800.0)),
        ("s3", photo(1_800, 8.0, 700.0)),
        ("p", photo(60_000, 6.0, 600.0)),
        ("q", photo(120_000, 5.0, 500.0)),
    ];
    let all: Vec<PathBuf> = rows.iter().map(|(name, _)| PathBuf::from(name)).collect();
    let known: HashMap<PathBuf, Facts> = rows
        .into_iter()
        .map(|(name, facts)| (PathBuf::from(name), facts))
        .collect();
    assert_eq!(top_of(&all, &known, &HashMap::new(), 1), "s1");
    assert_eq!(top_of(&all, &known, &HashMap::new(), 2), "ps1");
    assert_eq!(top_of(&all, &known, &HashMap::new(), 3), "pqs1");
    assert_eq!(top_of(&all, &known, &HashMap::new(), 4), "pqs1s2");
}

/// Videos and later copies of a photo never count.
#[test]
fn top_leaves_out_videos_and_copies() {
    let good = |fingerprint| Facts {
        fingerprint: Some(fingerprint),
        scores: Scores {
            sharpness: Some(900.0),
            aesthetic: Some(8.0),
            ..Scores::default()
        },
        ..Facts::default()
    };
    let all: Vec<PathBuf> = ["v.mp4", "z (1).jpg", "z.jpg", "w.jpg"]
        .map(PathBuf::from)
        .to_vec();
    let known: HashMap<PathBuf, Facts> = HashMap::from([
        (all[0].clone(), good(1)),
        (all[1].clone(), good(2)),
        (all[2].clone(), good(2)),
        (all[3].clone(), good(3)),
    ]);
    assert_eq!(top_of(&all, &known, &HashMap::new(), 10), "w.jpgz.jpg");
}

/// With Top N on, the view shows the picked photos – together with the boxes – and the
/// photos / videos choice waits for later.
#[test]
fn the_view_shows_the_picked_photos_with_the_boxes() {
    let (all, mut known) = fixture();
    for name in ["b", "c"] {
        if let Some(facts) = known.get_mut(Path::new(name)) {
            facts.top = true;
        }
    }
    let shown = |options: ViewOptions| {
        names(
            &build(
                &all,
                options,
                |p: &Path| known.get(p).copied(),
                &HashMap::new(),
                &HashMap::new(),
                |_| false,
            )
            .paths,
        )
    };
    let mut options = ViewOptions {
        top: Some(2),
        media: Media::Videos,
        ..ViewOptions::default()
    };
    assert_eq!(shown(options), "bc");
    options.filter.set(FilterKind::Stars(5), true);
    assert_eq!(shown(options), "c");
    assert!(options.is_filtered() && options.depends_on_scores());
    let mut cleared = options;
    cleared.clear_filters();
    assert_eq!(cleared.top, None);
    // Another sort keeps the pick, another filter makes a new one.
    let sorted = ViewOptions {
        sort: SortKey::Taken,
        ..options
    };
    assert_eq!(sorted.top_key(), options.top_key());
    assert_ne!(cleared.top_key(), options.top_key());
    let mut boxes = options;
    boxes.filter.set(FilterKind::Stars(4), true);
    assert_ne!(boxes.top_key(), options.top_key());
    assert_eq!(ViewOptions::default().top_key(), None);
    let mut chosen = ViewOptions::default();
    Scope::Top(50).apply(&mut chosen);
    assert_eq!(Scope::of(&chosen), Scope::Top(50));
    Scope::Media(Media::Photos).apply(&mut chosen);
    assert_eq!((chosen.top, chosen.media), (None, Media::Photos));
}

#[test]
fn a_folder_of_sharp_photos_has_no_blurry_ones() {
    let scores = |s| Scores {
        sharpness: Some(s),
        ..Scores::default()
    };
    let sharp = [900.0, 1500.0, 3000.0, 4000.0, 6000.0].map(scores);
    let p = Percentiles::from_scores(sharp.iter());
    assert!(
        sharp.iter().all(|s| !p.is_blurry(s)),
        "the least sharp photo is still sharp"
    );
    let mixed = [90.0, 1500.0, 3000.0, 4000.0, 6000.0].map(scores);
    let p = Percentiles::from_scores(mixed.iter());
    assert!(p.is_blurry(&mixed[0]));
    assert!(!p.is_blurry(&mixed[1]));
    // The eye region has its own, lower ceiling.
    assert!(is_blurry(0.1, 50.0, true));
    assert!(!is_blurry(0.1, 70.0, true));
    assert!(!is_blurry(0.5, 10.0, false), "not among the blurriest 20 %");
}

#[test]
fn eyes_decide_for_portraits() {
    let scores = |sharpness, eyes| Scores {
        sharpness: Some(sharpness),
        eyes,
        ..Scores::default()
    };
    // A portrait with a soft frame (skin, bokeh) but the sharpest eyes of the series.
    let all = [
        scores(900.0, None),
        scores(100.0, Some(80.0)),
        scores(120.0, Some(20.0)),
    ];
    let p = Percentiles::from_scores(all.iter());
    assert_eq!(p.subject(&all[1]), Some((1.0, true)));
    assert_eq!(p.subject(&all[2]), Some((0.0, true)));
    assert_eq!(p.subject(&all[0]), Some((1.0, false)));
}

fn timed(name: &str, taken: Option<i64>, sharp: Option<f32>, rating: Rating) -> (PathBuf, Facts) {
    (
        PathBuf::from(name),
        Facts {
            rating,
            taken_ms: taken,
            scores: Scores {
                sharpness: sharp,
                ..Scores::default()
            },
            ..Facts::default()
        },
    )
}

#[test]
fn series_follow_capture_time_and_put_the_sharpest_first() {
    let rows = [
        timed("a", Some(0), Some(10.0), Rating::Unrated),
        timed("b", Some(1_000), Some(90.0), Rating::Unrated),
        timed("c", Some(1_800), Some(40.0), Rating::Rejected),
        timed("d", Some(5_000), Some(5.0), Rating::Unrated),
        timed("e", Some(6_000), Some(70.0), Rating::Unrated),
        timed("f", None, Some(100.0), Rating::Unrated),
    ];
    let all: Vec<_> = rows.iter().map(|(p, _)| p.clone()).collect();
    let known: HashMap<_, _> = rows.into_iter().collect();
    let lookup = |p: &Path| known.get(p).copied();
    let options = ViewOptions {
        sort: SortKey::Taken,
        ..ViewOptions::default()
    };
    let view = build(
        &all,
        options,
        lookup,
        &HashMap::new(),
        &HashMap::new(),
        |_| false,
    );
    // b is the sharpest of the first burst, c is rejected so last; e beats d; f has no time.
    assert_eq!(names(&view.paths), "bacedf");
    assert_eq!(view.series[0].unwrap().index, 1);
    assert_eq!(view.series[0].unwrap().len, 3);
    assert_eq!(view.series[2].unwrap().index, 3);
    assert_eq!(view.series[3].unwrap().len, 2);
    assert!(view.series[5].is_none());
    assert!(view.grouped);
}

#[test]
fn series_never_mix_cameras() {
    let shot = |name: &str, camera: Option<&str>, taken: i64, sharp: f32| {
        let (path, mut facts) = timed(name, Some(taken), Some(sharp), Rating::Unrated);
        facts.camera = camera.map(crate::metadata::camera_id);
        (path, facts)
    };
    // Two phones fire together; a screenshot without EXIF camera sits in between.
    let rows = [
        shot("a", Some("Pixel 7a"), 0, 10.0),
        shot("b", Some("moto g42"), 400, 20.0),
        shot("c", Some("Pixel 7a"), 1_000, 30.0),
        shot("d", Some("moto g42"), 1_300, 40.0),
        shot("e", None, 1_500, 50.0),
        shot("f", None, 2_500, 60.0),
        // Same second as a and c, other camera: whole seconds only.
        shot("g", Some("XQ-ES54"), 0, 70.0),
        shot("h", Some("XQ-ES54"), 1_000, 80.0),
    ];
    let all: Vec<_> = rows.iter().map(|(p, _)| p.clone()).collect();
    let known: HashMap<_, _> = rows.into_iter().collect();
    let view = build(
        &all,
        ViewOptions {
            sort: SortKey::Taken,
            ..ViewOptions::default()
        },
        |p: &Path| known.get(p).copied(),
        &HashMap::new(),
        &HashMap::new(),
        |_| false,
    );
    let series_of = |name: &str| {
        let index = view.iter().position(|p| p == Path::new(name)).unwrap();
        view.series[index].map(|place| (place.id, place.len))
    };
    assert_eq!(series_of("a").map(|s| s.1), Some(2));
    assert_eq!(series_of("a"), series_of("c"));
    assert_eq!(series_of("b"), series_of("d"));
    assert_eq!(
        series_of("e"),
        series_of("f"),
        "no camera: among themselves"
    );
    assert_eq!(series_of("g"), series_of("h"));
    let ids: HashSet<u32> = ["a", "b", "e", "g"]
        .iter()
        .map(|n| series_of(n).unwrap().0)
        .collect();
    assert_eq!(ids.len(), 4, "four series");
    // Each series stays together, even two that start in the same millisecond.
    let order = names(&view.paths);
    for pair in ["ca", "db", "fe", "hg"] {
        assert!(order.contains(pair), "{pair} in {order}");
    }
}

/// Windows names copies "IMG - Kopie.jpg"; they sort before "IMG.jpg" but are not the
/// original. Without a name hint, the only marked copy wins, then folder order.
#[test]
fn duplicates_pick_the_original_by_name_then_marks() {
    let refs = |names: &[&str]| -> Vec<PathBuf> { names.iter().map(PathBuf::from).collect() };
    let pick = |paths: &[PathBuf], marked: &[&str]| {
        let list: Vec<&PathBuf> = paths.iter().collect();
        let marked = |p: &Path| marked.iter().any(|m| Path::new(m) == p);
        pick_original(&list, &marked).to_string_lossy().into_owned()
    };
    let copies = refs(&["DSC_0211 Kopie.jpg", "DSC_0211.jpg"]);
    assert_eq!(pick(&copies, &[]), "DSC_0211.jpg");
    assert_eq!(
        pick(&copies, &["DSC_0211 Kopie.jpg"]),
        "DSC_0211.jpg",
        "name first"
    );
    let numbered = refs(&["IMG_1 (1).JPG", "img_1.jpg", "IMG_1 - Copy.JPG"]);
    assert_eq!(pick(&numbered, &[]), "img_1.jpg");
    let folders = refs(&["a/IMG.jpg", "b/IMG.jpg"]);
    assert_eq!(pick(&folders, &[]), "a/IMG.jpg");
    assert_eq!(pick(&folders, &["b/IMG.jpg"]), "b/IMG.jpg");
    assert_eq!(pick(&folders, &["a/IMG.jpg", "b/IMG.jpg"]), "a/IMG.jpg");
}

#[test]
fn duplicates_keep_the_first_path_as_original() {
    let all: Vec<PathBuf> = ["a", "b", "c"].map(PathBuf::from).to_vec();
    let known = HashMap::from([
        (
            all[0].clone(),
            Facts {
                fingerprint: Some(1),
                ..Facts::default()
            },
        ),
        (
            all[1].clone(),
            Facts {
                fingerprint: Some(2),
                ..Facts::default()
            },
        ),
        (
            all[2].clone(),
            Facts {
                fingerprint: Some(1),
                ..Facts::default()
            },
        ),
    ]);
    let lookup = |p: &Path| known.get(p).copied();
    let view = build(
        &all,
        ViewOptions::default(),
        lookup,
        &HashMap::new(),
        &HashMap::new(),
        |_| false,
    );
    assert_eq!(view.duplicate_of[0], None);
    assert_eq!(view.duplicate_of[1], None);
    assert_eq!(view.duplicate_of[2].as_deref(), Some(all[0].as_path()));

    let mut only = PhotoFilter::default();
    only.set(FilterKind::Duplicate, true);
    let view = build(
        &all,
        ViewOptions {
            filter: only,
            ..ViewOptions::default()
        },
        lookup,
        &HashMap::new(),
        &HashMap::new(),
        |_| false,
    );
    assert_eq!(names(&view.paths), "c");
}

/// Photos or videos only goes together with the boxes (AND), decided by the file name.
#[test]
fn media_filter_goes_with_the_boxes() {
    let all: Vec<PathBuf> = ["a.jpg", "b.mp4", "c.jpg", "d.mov"]
        .map(PathBuf::from)
        .to_vec();
    let rated = |p: &Path| {
        let name = p.to_string_lossy();
        Some(Facts {
            rating: if name.starts_with('a') || name.starts_with('b') {
                Rating::Stars(3)
            } else {
                Rating::Unrated
            },
            ..Facts::default()
        })
    };
    let shown = |options: ViewOptions| {
        names(
            &build(
                &all,
                options,
                rated,
                &HashMap::new(),
                &HashMap::new(),
                |_| false,
            )
            .paths,
        )
    };
    let mut options = ViewOptions::default();
    assert_eq!(shown(options), "a.jpgb.mp4c.jpgd.mov");
    options.media = Media::Photos;
    assert_eq!(shown(options), "a.jpgc.jpg");
    options.media = Media::Videos;
    assert_eq!(shown(options), "b.mp4d.mov");
    options.filter.set(FilterKind::Stars(3), true);
    assert_eq!(shown(options), "b.mp4");
    assert!(options.is_filtered());
    options.clear_filters();
    assert_eq!(options, ViewOptions::default());
}

/// Photos or videos only is no reason to refresh the order when scores arrive.
#[test]
fn media_does_not_depend_on_scores() {
    let options = ViewOptions {
        media: Media::Videos,
        ..ViewOptions::default()
    };
    assert!(!options.depends_on_scores());
    assert!(options.is_filtered());
    for media in Media::ALL {
        assert_eq!(Media::from_id(media.id()), Some(media));
    }
    assert_eq!(Media::from_id("something"), None);
}

#[test]
fn ids_round_trip() {
    for key in SortKey::ALL {
        assert_eq!(SortKey::from_id(key.id()), Some(key));
    }
    assert_eq!(SortKey::from_id("laion"), None);
    let mut filter = PhotoFilter::default();
    for kind in FilterKind::ALL {
        filter.set(kind, true);
    }
    assert_eq!(PhotoFilter::from_stored(&filter.id()), filter);
}
