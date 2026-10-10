use super::*;

const STAMP: FileStamp = FileStamp {
    size: 1000,
    mtime_ns: 42,
};

#[test]
fn lookup_requires_matching_stamp() {
    let db = Db::open_in_memory().unwrap();
    db.put_file(
        "a.jpg",
        STAMP,
        u64::MAX - 7,
        Rating::Stars(3),
        Some(Label::Red),
    )
    .unwrap();
    let record = db.lookup("a.jpg", STAMP).unwrap().unwrap();
    assert_eq!(
        record.fingerprint,
        u64::MAX - 7,
        "u64 survives the i64 column"
    );
    assert_eq!(record.rating, Rating::Stars(3));
    assert_eq!(record.label, Some(Label::Red));
    assert_eq!(record.image, ImageRecord::default());

    let changed = FileStamp {
        size: 1001,
        ..STAMP
    };
    assert!(db.lookup("a.jpg", changed).unwrap().is_none());
    db.update_after_write("a.jpg", changed, Rating::Rejected, None)
        .unwrap();
    let updated = db.lookup("a.jpg", changed).unwrap().unwrap();
    assert_eq!(updated.rating, Rating::Rejected);
    assert_eq!(updated.label, None);
}

#[test]
fn scores_follow_the_fingerprint() {
    let db = Db::open_in_memory().unwrap();
    db.put_sharpness(7, 123.5, 1).unwrap();
    db.put_aesthetic(7, 6.25, "m1", &[0.5; 768]).unwrap();
    db.put_aesthetic25(7, 5.5, "v25").unwrap();
    db.put_exposure(7, 0.01, 0.2, 1).unwrap();
    db.put_faces(7, Some(88.0), 2, 1).unwrap();
    db.put_thumbnail(7, &[1, 2, 3]).unwrap();
    // A renamed file pointing at the same pixels sees the same scores.
    db.put_file("renamed.jpg", STAMP, 7, Rating::Unrated, None)
        .unwrap();
    let image = db.lookup("renamed.jpg", STAMP).unwrap().unwrap().image;
    assert_eq!(image.scores.sharpness, Some(123.5));
    assert_eq!(image.scores.aesthetic, Some(6.25));
    assert_eq!(image.scores.aesthetic25, Some(5.5));
    assert_eq!(image.scores.highlights, Some(0.01));
    assert_eq!(image.scores.shadows, Some(0.2));
    assert_eq!(image.scores.eyes, Some(88.0));
    assert_eq!(image.scores.faces, Some(2));
    assert_eq!(image.sharpness_version, 1);
    assert_eq!(image.exposure_version, 1);
    assert_eq!(image.faces_version, 1);
    assert_eq!(image.aesthetic_model.as_deref(), Some("m1"));
    assert_eq!(image.aesthetic25_model.as_deref(), Some("v25"));
    assert_eq!(image.embedding.as_deref(), Some(&[0.5f32; 768][..]));
    assert!(image.has_thumbnail);
    assert_eq!(db.thumbnail(7).unwrap(), Some(vec![1, 2, 3]));
    assert_eq!(db.image(7).unwrap(), image);
    assert_eq!(db.image(8).unwrap(), ImageRecord::default());
}

#[test]
fn settings_round_trip() {
    let db = Db::open_in_memory().unwrap();
    assert_eq!(db.setting("sort"), None);
    db.put_setting("sort", "aesthetics");
    assert_eq!(db.setting("sort").as_deref(), Some("aesthetics"));
}

/// An index written by the first release (without the newer columns) must open and work.
#[test]
fn migrates_an_index_from_the_first_release() {
    const FIRST_RELEASE: &str = "
            CREATE TABLE files (path TEXT PRIMARY KEY, size INTEGER NOT NULL,
                mtime_ns INTEGER NOT NULL, fingerprint INTEGER NOT NULL, rating INTEGER);
            CREATE TABLE images (fingerprint INTEGER PRIMARY KEY, sharpness REAL,
                sharpness_version INTEGER NOT NULL DEFAULT 0, aesthetic REAL,
                aesthetic_model TEXT, embedding BLOB, thumbnail BLOB);
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO images (fingerprint, sharpness, sharpness_version) VALUES (9, 50.0, 1);
            INSERT INTO files VALUES ('old.jpg', 1000, 42, 9, 4);";
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(FIRST_RELEASE).unwrap();
    let db = Db::init(conn, true).unwrap();

    let record = db.lookup("old.jpg", STAMP).unwrap().unwrap();
    assert_eq!(record.rating, Rating::Stars(4));
    assert_eq!(record.image.scores.sharpness, Some(50.0));
    assert_eq!(record.image.exposure_version, 0);
    assert_eq!(record.image.metadata_version, 0);
    assert_eq!(record.label, None);
    assert_eq!(record.image.camera, None);
    db.put_metadata(9, Some(1_000), Some("Pixel 7a"), 2)
        .unwrap();
    assert_eq!(db.image(9).unwrap().taken_ms, Some(1_000));
    assert_eq!(db.image(9).unwrap().camera.as_deref(), Some("Pixel 7a"));
    assert_eq!(db.image(9).unwrap().metadata_version, 2);
    db.put_faces(9, None, 0, 1).unwrap();
    assert_eq!(db.image(9).unwrap().scores.faces, Some(0));
    assert_eq!(record.image.scores.truncated, None, "not checked yet");
    db.put_truncated(9, true).unwrap();
    assert_eq!(db.image(9).unwrap().scores.truncated, Some(true));
    db.put_truncated(9, false).unwrap();
    let record = db.lookup("old.jpg", STAMP).unwrap().unwrap();
    assert_eq!(record.image.scores.truncated, Some(false));
}

/// Stars count with their number, rejections as 0; a deleted photo is no example.
#[test]
fn rejections_teach_and_deletions_do_not() {
    let db = Db::open_in_memory().unwrap();
    for (fp, path, rating) in [
        (1, "liked.jpg", Rating::Stars(5)),
        (2, "binned.jpg", Rating::Unrated),
        (3, "rejected.jpg", Rating::Rejected),
        (4, "unrated.jpg", Rating::Unrated),
    ] {
        db.put_aesthetic(fp, 5.0, "m", &[fp as f32; 768]).unwrap();
        db.put_file(path, STAMP, fp, rating, None).unwrap();
    }
    db.record_deletion("binned.jpg", "/.originals/binned.jpg")
        .unwrap();
    assert!(db.lookup("binned.jpg", STAMP).unwrap().is_none());

    let (examples, sources) = db.taste_examples().unwrap();
    let mut examples: Vec<(f32, f32)> = examples
        .into_iter()
        .map(|(embedding, label)| (embedding[0], label))
        .collect();
    examples.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(examples, [(1.0, 5.0), (3.0, 0.0)]);
    assert_eq!(
        sources,
        TasteSources {
            stars: 1,
            rejected: 1,
        }
    );
}

/// An index of 1.8.0 or before: its deletion examples are cleared when it opens.
#[test]
fn old_deletion_examples_are_cleared() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn.execute(
        "INSERT INTO feedback (fingerprint, label, at) VALUES (1, 0.0, 0), (2, 0.0, 0)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO restored (path) VALUES ('/p/a.jpg')", [])
        .unwrap();
    let db = Db::init(conn, true).unwrap();
    let count = |table: &str| -> i64 {
        db.conn()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    assert_eq!((count("feedback"), count("restored")), (0, 0));
}

fn sources(db: &Db) -> TasteSources {
    db.taste_examples().unwrap().1
}

/// A deleted photo's row lies in `.originals` with it (scores and thumbnail are found there
/// at once) and teaches nothing, not even its stars – on both kinds of separator. Put back,
/// it counts with its stars again.
#[test]
fn a_deleted_photo_counts_again_once_it_is_back() {
    for (dir, sep) in [("/p", "/"), (r"C:\p", r"\")] {
        let db = Db::open_in_memory().unwrap();
        let (photo, aside) = (
            format!("{dir}{sep}IMG.jpg"),
            format!("{dir}{sep}.originals{sep}IMG.jpg"),
        );
        db.put_aesthetic(5, 5.0, "m", &[5.0; 768]).unwrap();
        db.put_file(&photo, STAMP, 5, Rating::Stars(5), None)
            .unwrap();
        db.record_deletion(&photo, &aside).unwrap();
        assert!(db.lookup(&photo, STAMP).unwrap().is_none());
        assert_eq!(db.lookup(&aside, STAMP).unwrap().unwrap().fingerprint, 5);
        assert_eq!(sources(&db), TasteSources::default(), "{dir}");
        assert_eq!(
            db.set_aside_in(&format!("{dir}{sep}.originals")).unwrap(),
            [(aside.clone(), photo.clone())]
        );

        let back = format!("{dir}{sep}IMG (2).jpg");
        db.record_restore(&aside, &back, &photo).unwrap();
        assert_eq!(db.lookup(&back, STAMP).unwrap().unwrap().fingerprint, 5);
        assert!(
            db.set_aside_in(&format!("{dir}{sep}.originals"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            sources(&db),
            TasteSources {
                stars: 1,
                ..TasteSources::default()
            }
        );
    }
}

/// Back under another name, the photo takes the kept originals from before its deletion
/// along; a later one belongs to the photo that has the name now.
#[test]
fn kept_originals_follow_a_photo_put_back_under_another_name() {
    let db = Db::open_in_memory().unwrap();
    db.put_file("/p/IMG.jpg", STAMP, 7, Rating::Unrated, None)
        .unwrap();
    db.push_backup("/p/IMG.jpg", "/p/.originals/IMG.jpg", 1)
        .unwrap();
    db.record_deletion("/p/IMG.jpg", "/p/.originals/IMG (2).jpg")
        .unwrap();
    db.push_backup("/p/IMG.jpg", "/p/.originals/IMG (3).jpg", i64::MAX)
        .unwrap();
    db.record_restore("/p/.originals/IMG (2).jpg", "/p/IMG (2).jpg", "/p/IMG.jpg")
        .unwrap();
    let backups = |path| -> Vec<String> {
        db.backups_of(path)
            .unwrap()
            .into_iter()
            .map(|(_, b)| b)
            .collect()
    };
    assert_eq!(backups("/p/IMG (2).jpg"), ["/p/.originals/IMG.jpg"]);
    assert_eq!(backups("/p/IMG.jpg"), ["/p/.originals/IMG (3).jpg"]);
}

/// Faces round-trip with their points and eye measure; storing again replaces them.
#[test]
fn faces_are_stored_per_photo() {
    let db = Db::open_in_memory().unwrap();
    assert_eq!(db.faces_of(3).unwrap(), None);
    let face = |x: f32, eyes| FaceRow {
        score: 0.9,
        bbox: [x, 0.2, 0.1, 0.15],
        landmarks: [
            [x + 0.02, 0.25],
            [x + 0.07, 0.25],
            [x + 0.05, 0.28],
            [x + 0.03, 0.31],
            [x + 0.07, 0.31],
        ],
        eyes,
    };
    db.put_face_rows(3, &[face(0.1, Some(120.0)), face(0.5, None)])
        .unwrap();
    assert_eq!(
        db.faces_of(3).unwrap(),
        Some(vec![face(0.1, Some(120.0)), face(0.5, None)])
    );
    db.put_face_rows(3, &[face(0.7, None)]).unwrap();
    assert_eq!(db.faces_of(3).unwrap(), Some(vec![face(0.7, None)]));
    assert_eq!(db.faces_of(4).unwrap(), None);
}

#[test]
fn taste_reset_skips_old_ratings_until_they_change() {
    let db = Db::open_in_memory().unwrap();
    db.put_aesthetic(1, 5.0, "m", &[1.0; 768]).unwrap();
    db.put_file("a.jpg", STAMP, 1, Rating::Stars(4), None)
        .unwrap();
    assert_eq!(db.taste_examples().unwrap().0.len(), 1);
    db.reset_taste_learning().unwrap();
    assert!(db.taste_examples().unwrap().0.is_empty());
    db.allow_taste_for(1).unwrap();
    db.put_file("a.jpg", STAMP, 1, Rating::Stars(3), None)
        .unwrap();
    assert_eq!(db.taste_examples().unwrap().0.len(), 1);
}

#[test]
fn retarget_path_keeps_the_fingerprint() {
    let db = Db::open_in_memory().unwrap();
    db.put_file("old.jpg", STAMP, 9, Rating::Stars(4), Some(Label::Red))
        .unwrap();
    db.put_file("stale.jpg", STAMP, 1, Rating::Unrated, None)
        .unwrap();
    db.retarget_path("old.jpg", "stale.jpg").unwrap();
    assert!(db.lookup("old.jpg", STAMP).unwrap().is_none());
    let moved = db.lookup("stale.jpg", STAMP).unwrap().unwrap();
    assert_eq!(moved.fingerprint, 9);
    assert_eq!(moved.rating, Rating::Stars(4));
    assert_eq!(moved.label, Some(Label::Red));
    db.retarget_path("missing.jpg", "other.jpg").unwrap();
}

/// Offsets are per folder and camera; 0 removes one. A camera id above `i64::MAX` comes
/// back as it went in.
#[test]
fn camera_offsets_per_folder() {
    let db = Db::open_in_memory().unwrap();
    let big = u64::MAX - 7;
    db.set_camera_offset("/trip", 1, -7_777_000).unwrap();
    db.set_camera_offset("/trip", big, 3_000).unwrap();
    db.set_camera_offset("/other", 1, 60_000).unwrap();
    let trip = db.camera_offsets("/trip").unwrap();
    assert_eq!(trip, HashMap::from([(1, -7_777_000), (big, 3_000)]));
    db.set_camera_offset("/trip", 1, 0).unwrap();
    assert_eq!(
        db.camera_offsets("/trip").unwrap(),
        HashMap::from([(big, 3_000)])
    );
    assert_eq!(db.camera_offsets("/other").unwrap()[&1], 60_000);
}
