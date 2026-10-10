use super::*;

/// The temp JPEG never reuses a name: whatever is there already stays untouched – it is
/// neither written through nor deleted on drop.
#[test]
fn temp_jpeg_refuses_a_taken_name() {
    use std::fs;
    let dir = std::env::temp_dir().join(format!("cerno-temp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let taken = dir.join("taken.jpg");
    fs::write(&taken, b"someone else's").unwrap();
    assert!(TempJpeg::write_at(taken.clone(), b"edit").is_err());
    assert_eq!(fs::read(&taken).unwrap(), b"someone else's");

    let fresh = dir.join("fresh.jpg");
    let temp = TempJpeg::write_at(fresh.clone(), b"edit").unwrap();
    assert_eq!(fs::read(&fresh).unwrap(), b"edit");
    drop(temp);
    assert!(!fresh.exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn only_marks_a_save_dropped_come_back() {
    let red = Some(Label::Red);
    // Paint and the like drop all metadata: both come back.
    assert_eq!(
        lost_marks(Rating::Unrated, LabelInfo::None, Rating::Stars(4), red),
        (Some(Rating::Stars(4)), Some(red))
    );
    assert_eq!(
        lost_marks(Rating::Unrated, LabelInfo::None, Rating::Rejected, None),
        (Some(Rating::Rejected), None)
    );
    // What the other program set stays, an unknown label text too.
    let blue = LabelInfo::Known(Label::Blue);
    assert_eq!(
        lost_marks(Rating::Stars(2), blue, Rating::Stars(4), red),
        (None, None)
    );
    assert_eq!(
        lost_marks(Rating::Unrated, LabelInfo::Other, Rating::Unrated, red),
        (None, None)
    );
    // Nothing known, nothing to write.
    assert_eq!(
        lost_marks(Rating::Unrated, LabelInfo::None, Rating::Unrated, None),
        (None, None)
    );
}

#[test]
fn only_xmp_unless_microsoft_tags_exist() {
    assert_eq!(
        rating_args(Rating::Stars(4), &RatingInfo::default()),
        ["-XMP-xmp:Rating=4"]
    );
    let windows = RatingInfo {
        value: Rating::Stars(2),
        has_exif_rating: true,
        has_ms_photo_rating: true,
    };
    assert_eq!(
        rating_args(Rating::Stars(5), &windows),
        [
            "-XMP-xmp:Rating=5",
            "-EXIF:Rating=5",
            "-EXIF:RatingPercent=99",
            "-XMP-microsoft:RatingPercent=99",
        ]
    );
}

#[test]
fn clearing_deletes_the_tags() {
    let windows = RatingInfo {
        value: Rating::Stars(3),
        has_exif_rating: true,
        has_ms_photo_rating: false,
    };
    assert_eq!(
        rating_args(Rating::Unrated, &windows),
        ["-XMP-xmp:Rating=", "-EXIF:Rating=", "-EXIF:RatingPercent="]
    );
}

#[test]
fn rejected_is_minus_one_and_clears_the_windows_stars() {
    let windows = RatingInfo {
        value: Rating::Stars(3),
        has_exif_rating: true,
        has_ms_photo_rating: true,
    };
    assert_eq!(
        rating_args(Rating::Rejected, &windows),
        [
            "-XMP-xmp:Rating=-1",
            "-EXIF:Rating=",
            "-EXIF:RatingPercent=",
            "-XMP-microsoft:RatingPercent=",
        ]
    );
}

/// A RAW's marks go into `IMG_9.xmp` beside it – created on the first write – and the RAW
/// itself is never touched. Through a real ExifTool; skipped without one.
#[test]
fn raw_marks_go_into_the_sidecar() {
    if crate::exiftool::locate().is_none() {
        eprintln!("ExifTool not found – skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cerno-sidecar-w-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let raw = dir.join("Blüte IMG_9.NEF");
    let untouched = b"II*\0 proprietary raw data".to_vec();
    std::fs::write(&raw, &untouched).unwrap();
    let mut exiftool = None;

    write_marks(
        &mut exiftool,
        &raw,
        Some(Rating::Stars(3)),
        Some(Some(Label::Green)),
        None,
    )
    .unwrap();
    assert_eq!(std::fs::read(&raw).unwrap(), untouched, "RAW untouched");
    let meta = metadata::read_for(&raw, &untouched);
    assert_eq!(meta.rating.value, Rating::Stars(3));
    assert_eq!(meta.label, LabelInfo::Known(Label::Green));
    // Back to no stars: the sidecar says so, not the camera.
    write_marks(&mut exiftool, &raw, Some(Rating::Unrated), None, None).unwrap();
    assert_eq!(metadata::read_sidecar(&raw).rating.value, Rating::Unrated);
    drop(exiftool);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A star on a video beside a RAW of its name goes into `IMG_4.MOV.xmp`; the RAW's
/// `IMG_4.xmp` keeps its own marks, and each reads back its own. Skipped without ExifTool.
#[test]
fn a_video_beside_a_raw_marks_its_own_sidecar() {
    if crate::exiftool::locate().is_none() {
        eprintln!("ExifTool not found – skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cerno-sidecar-v-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (video, raw) = (dir.join("IMG_4.MOV"), dir.join("IMG_4.CR3"));
    std::fs::write(&video, b"not read").unwrap();
    std::fs::write(&raw, b"II*\0 raw").unwrap();
    let mut exiftool = None;

    write_marks(&mut exiftool, &raw, Some(Rating::Stars(2)), None, None).unwrap();
    write_marks(
        &mut exiftool,
        &video,
        Some(Rating::Stars(5)),
        Some(Some(Label::Red)),
        None,
    )
    .unwrap();
    assert!(dir.join("IMG_4.MOV.xmp").is_file());
    let (video_meta, raw_meta) = (metadata::read_sidecar(&video), metadata::read_sidecar(&raw));
    assert_eq!(video_meta.rating.value, Rating::Stars(5));
    assert_eq!(video_meta.label, LabelInfo::Known(Label::Red));
    assert_eq!(raw_meta.rating.value, Rating::Stars(2));
    assert_eq!(raw_meta.label.known(), None);
    drop(exiftool);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The first write into a new sidecar carries the RAW's own marks: a colour set on a RAW
/// with in-camera stars and a keyword keeps both, and a comment is read back from the
/// sidecar. Clearing a RAW's only mark leaves an empty sidecar that says so.
#[test]
fn a_new_sidecar_keeps_the_raws_own_marks() {
    if crate::exiftool::locate().is_none() {
        eprintln!("ExifTool not found – skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cerno-sidecar-c-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let in_camera = "II*\0<x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF \
            xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><rdf:Description \
            rdf:about='' xmlns:xmp='http://ns.adobe.com/xap/1.0/' \
            xmlns:dc='http://purl.org/dc/elements/1.1/' xmp:Rating='3'><dc:subject><rdf:Bag>\
            <rdf:li>Berg</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF>\
            </x:xmpmeta>";
    let raw = dir.join("IMG_7.NEF");
    std::fs::write(&raw, in_camera).unwrap();
    let mut exiftool = None;

    write_marks(&mut exiftool, &raw, None, Some(Some(Label::Green)), None).unwrap();
    let side = metadata::read_sidecar(&raw);
    assert_eq!(
        side.rating.value,
        Rating::Stars(3),
        "in-camera stars carried"
    );
    assert_eq!(side.label, LabelInfo::Known(Label::Green));
    assert_eq!(
        side.description.keywords,
        ["Berg"],
        "in-camera keyword carried"
    );
    let comment = Description {
        comment: "Gipfel".into(),
        keywords: vec!["Berg".into()],
    };
    write_marks(&mut exiftool, &raw, None, None, Some(&comment)).unwrap();
    let meta = metadata::read_for(&raw, in_camera.as_bytes());
    assert_eq!(
        meta.description, comment,
        "the description comes from the sidecar"
    );
    assert_eq!(
        std::fs::read(&raw).unwrap(),
        in_camera.as_bytes(),
        "RAW untouched"
    );

    // A RAW with only in-camera stars, cleared: nothing to write, but the sidecar must say
    // "no stars" from now on.
    let stars_only = in_camera.replace(
        "<dc:subject><rdf:Bag><rdf:li>Berg</rdf:li></rdf:Bag></dc:subject>",
        "",
    );
    let cleared = dir.join("IMG_8.NEF");
    std::fs::write(&cleared, &stars_only).unwrap();
    assert_eq!(
        metadata::read_for(&cleared, stars_only.as_bytes())
            .rating
            .value,
        Rating::Stars(3)
    );
    write_marks(&mut exiftool, &cleared, Some(Rating::Unrated), None, None).unwrap();
    assert!(
        crate::sidecar::path_of(&cleared).is_file(),
        "the empty sidecar is written"
    );
    let meta = metadata::read_for(&cleared, stars_only.as_bytes());
    assert_eq!(meta.rating.value, Rating::Unrated);
    drop(exiftool);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A sidecar another program wrote – Lightroom's development settings, a mask, a namespace
/// Cerno doesn't know, keywords – keeps all of it when Cerno sets stars, a colour and a
/// comment: only those tags change. ExifTool writes the packet anew (attributes may become
/// elements), so the test looks for names and values, not bytes. Skipped without ExifTool.
#[test]
fn a_lightroom_sidecar_keeps_its_development() {
    if crate::exiftool::locate().is_none() {
        eprintln!("ExifTool not found – skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cerno-lr-xmp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let raw = dir.join("IMG_3.CR3");
    let untouched = b"proprietary raw data".to_vec();
    std::fs::write(&raw, &untouched).unwrap();
    let lightroom = r#"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 7.0">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:foo="http://example.com/foo/1.0/"
    xmp:Rating="2"
    crs:Version="16.0"
    crs:ProcessVersion="15.4"
    crs:Exposure2012="+0.50"
    crs:Contrast2012="+12"
    crs:HasSettings="True"
    foo:Kept="keep me">
   <crs:MaskGroupBasedCorrections>
    <rdf:Seq>
     <rdf:li crs:What="Correction" crs:LocalExposure2012="-0.35"/>
    </rdf:Seq>
   </crs:MaskGroupBasedCorrections>
   <dc:subject>
    <rdf:Bag>
     <rdf:li>Berg</rdf:li>
    </rdf:Bag>
   </dc:subject>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end='w'?>
"#;
    let sidecar = crate::sidecar::path_of(&raw);
    std::fs::write(&sidecar, lightroom).unwrap();
    let mut exiftool = None;
    let comment = Description {
        comment: "Gipfel".into(),
        keywords: vec!["Berg".into()],
    };
    write_marks(
        &mut exiftool,
        &raw,
        Some(Rating::Stars(5)),
        Some(Some(Label::Red)),
        Some(&comment),
    )
    .unwrap();
    drop(exiftool);
    let written = std::fs::read_to_string(&sidecar).unwrap();
    for kept in [
        "Exposure2012",
        "+0.50",
        "Contrast2012",
        "ProcessVersion",
        "MaskGroupBasedCorrections",
        "LocalExposure2012",
        "-0.35",
        "keep me",
        "Berg",
    ] {
        assert!(written.contains(kept), "{kept} lost:\n{written}");
    }
    let meta = metadata::read_sidecar(&raw);
    assert_eq!(meta.rating.value, Rating::Stars(5));
    assert_eq!(meta.label, LabelInfo::Known(Label::Red));
    assert_eq!(meta.description.comment, "Gipfel");
    assert_eq!(std::fs::read(&raw).unwrap(), untouched, "RAW untouched");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_description_comes_back_only_when_the_save_dropped_it() {
    let known = Description {
        comment: "Gipfel".into(),
        keywords: vec!["Berg".into()],
    };
    let none = Description::default();
    assert_eq!(lost_description(&none, &known), Some(known.clone()));
    // The program kept (or set) something of its own: that stays.
    let own = Description {
        comment: String::new(),
        keywords: vec!["Tal".into()],
    };
    assert_eq!(lost_description(&own, &known), None);
    // Nothing known, nothing to write.
    assert_eq!(lost_description(&none, &none), None);
}

/// End-to-end through a real ExifTool; skipped when ExifTool isn't installed.
#[test]
fn writes_stars_and_keeps_file_dates() {
    // Non-ASCII on purpose: needs `-charset filename=UTF8` on Windows.
    round_trip(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/tiny.jpg"
        )),
        "Überprüfung ä.jpg",
    );
}

/// The same on a real HEIC (XMP lives in a metadata item there, not in APP1):
/// `CERNO_TEST_HEIC=<file.heic> cargo test -- --ignored heic`
#[test]
#[ignore = "needs a HEIC sample in CERNO_TEST_HEIC"]
fn heic_rating_round_trip() {
    let source = std::env::var_os("CERNO_TEST_HEIC").expect("set CERNO_TEST_HEIC");
    round_trip(Path::new(&source), "Überprüfung ä.heic");
}

fn round_trip(source: &Path, name: &str) {
    use std::fs::{self, File, FileTimes};
    use std::time::SystemTime;

    if crate::exiftool::locate().is_none() {
        eprintln!("ExifTool not found – skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!(
        "cerno-rating-{}-{}",
        std::process::id(),
        name.len()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::copy(source, &path).unwrap();
    let old = SystemTime::now() - Duration::from_secs(86_400 * 400);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(old))
        .unwrap();
    let before = fs::metadata(&path).unwrap();
    let mut exiftool = None;

    let read = || metadata::read(&fs::read(&path).unwrap());
    let on_disk = || read().rating.value;
    let on_label = || read().label;
    write_marks(
        &mut exiftool,
        &path,
        Some(Rating::Stars(4)),
        Some(Some(Label::Red)),
        None,
    )
    .unwrap();
    assert_eq!(on_disk(), Rating::Stars(4));
    assert_eq!(on_label(), LabelInfo::Known(Label::Red));
    write_marks(&mut exiftool, &path, Some(Rating::Stars(4)), None, None).unwrap(); // no-op
    write_marks(&mut exiftool, &path, Some(Rating::Rejected), None, None).unwrap();
    assert_eq!(on_disk(), Rating::Rejected);
    assert_eq!(on_label(), LabelInfo::Known(Label::Red));
    write_marks(
        &mut exiftool,
        &path,
        Some(Rating::Unrated),
        Some(None),
        None,
    )
    .unwrap();
    assert_eq!(on_disk(), Rating::Unrated);
    assert_eq!(on_label(), LabelInfo::None);

    // Comment (two lines, through ExifTool's C-string arguments) and umlaut keywords.
    let description = Description {
        comment: "Grüße aus Köln\nzweite Zeile".to_owned(),
        keywords: vec!["Straße".to_owned(), "Äpfel".to_owned()],
    };
    write_marks(&mut exiftool, &path, None, None, Some(&description)).unwrap();
    assert_eq!(read().description, description);
    assert_eq!(on_label(), LabelInfo::None, "other marks untouched");
    if name.ends_with(".jpg") {
        // IPTC too (HEIC has no IPTC block), as UTF-8.
        assert_eq!(
            metadata::iptc_description(&fs::read(&path).unwrap()),
            description
        );
    }
    let unchanged = fs::read(&path).unwrap();
    write_marks(&mut exiftool, &path, None, None, Some(&description)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), unchanged, "same values: no write");
    write_marks(
        &mut exiftool,
        &path,
        None,
        None,
        Some(&Description::default()),
    )
    .unwrap();
    assert_eq!(read().description, Description::default());

    let after = fs::metadata(&path).unwrap();
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    #[cfg(windows)]
    assert_eq!(after.created().unwrap(), before.created().unwrap());
    drop(exiftool);
    fs::remove_dir_all(&dir).unwrap();
}

/// A quarter turn with its original kept, a rating given afterwards, then Ctrl+Z: the
/// orientation comes back, the rating stays, the dates never move.
#[test]
fn restore_brings_back_the_original_and_keeps_later_marks() {
    use std::fs::{self, File, FileTimes};
    use std::time::SystemTime;

    if crate::exiftool::locate().is_none() {
        eprintln!("ExifTool not found – skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cerno-restore-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Ärger.jpg");
    fs::copy(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/tiny.jpg"
        )),
        &path,
    )
    .unwrap();
    let old = SystemTime::now() - Duration::from_secs(86_400 * 400);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(old))
        .unwrap();
    let db = Db::open_in_memory().unwrap();
    let mut exiftool = None;
    let read = || metadata::read(&fs::read(&path).unwrap());
    let before = read().orientation;

    let untouched = fs::read(&path).unwrap();
    // Two turns: only the first original is kept.
    for _ in 0..2 {
        crate::originals::keep(&db, &path).unwrap();
        apply_quarter_turn(&mut exiftool, &path, true, &db).unwrap();
    }
    assert_ne!(read().orientation, before);
    let kept = dir.join(crate::originals::FOLDER).join("Ärger.jpg");
    assert_eq!(fs::read(&kept).unwrap(), untouched);
    write_marks(&mut exiftool, &path, Some(Rating::Stars(4)), None, None).unwrap();

    restore_original(&mut exiftool, &path, &db).unwrap();
    let after = read();
    assert_eq!(after.orientation, before);
    assert_eq!(after.rating.value, Rating::Stars(4));
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
    assert_eq!(fs::read(&kept).unwrap(), untouched, "the original stays");
    restore_original(&mut exiftool, &path, &db).expect("again: same result");
    assert_eq!(read().orientation, before);
    drop(exiftool);
    fs::remove_dir_all(&dir).unwrap();
}

/// A re-encode keeps the ICC profile and the stored colours: an Adobe RGB photo must not
/// come back as untagged sRGB.
#[test]
fn reencode_keeps_the_colour_profile() {
    use crate::decode::fixtures;
    use zune_core::bytestream::ZCursor;
    use zune_jpeg::JpegDecoder;

    if crate::exiftool::locate().is_none() {
        eprintln!("ExifTool not found – skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cerno-icc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("adobe.jpg");
    let rgb: Vec<u8> = (0..64 * 48).flat_map(|_| [30, 200, 60]).collect();
    let mut jpeg = fixtures::jpeg(64, 48, &rgb);
    let profile = fixtures::adobe_rgb_profile();
    fixtures::insert_icc(&mut jpeg, &profile);
    std::fs::write(&path, &jpeg).unwrap();
    let before = crate::decode::decode_for_edit(&jpeg, 1).unwrap();

    let db = Db::open_in_memory().unwrap();
    let mut exiftool = None;
    let rect = crate::edit::PixelRect {
        x: 8,
        y: 8,
        w: 40,
        h: 30,
    };
    let cropped = crate::edit::render_crop(&path, rect, &FileLocks::default()).unwrap();
    apply_pixels(&mut exiftool, &path, &cropped, &db).unwrap();

    let after = std::fs::read(&path).unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(&after));
    decoder.decode_headers().unwrap();
    assert_eq!(decoder.icc_profile().as_deref(), Some(profile.as_slice()));
    let pixels = crate::decode::decode_for_edit(&after, 1).unwrap();
    let (centre_before, centre_after) = (
        &before.rgb[((24 * 64 + 32) * 3)..][..3],
        &pixels.rgb[((15 * 40 + 20) * 3)..][..3],
    );
    for (a, b) in centre_before.iter().zip(centre_after) {
        assert!(a.abs_diff(*b) <= 3, "{centre_before:?} → {centre_after:?}");
    }
    drop(exiftool);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Quarter turn via the orientation tag, then a real re-encode. Dates stay put either way.
#[test]
fn quarter_turn_and_reencode_keep_file_dates() {
    use std::fs::{self, File, FileTimes};
    use std::time::SystemTime;

    if crate::exiftool::locate().is_none() {
        eprintln!("ExifTool not found – skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cerno-edit-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tiny.jpg");
    fs::copy(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/tiny.jpg"
        )),
        &path,
    )
    .unwrap();
    let old = SystemTime::now() - Duration::from_secs(86_400 * 400);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(old))
        .unwrap();
    let created = fs::metadata(&path).unwrap().created().ok();
    let db = Db::open_in_memory().unwrap();
    let mut exiftool = None;

    let orientation = || metadata::read(&fs::read(&path).unwrap()).orientation;
    let before = orientation();
    apply_quarter_turn(&mut exiftool, &path, true, &db).unwrap();
    assert_eq!(orientation(), crate::edit::rotate_orientation(before, 1));
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);

    let bytes = fs::read(&path).unwrap();
    let meta = metadata::read(&bytes);
    let oriented = crate::decode::decode_for_display(
        &bytes,
        crate::library::Format::Jpeg,
        meta.orientation,
        [u32::MAX; 2],
    )
    .unwrap();
    let jpeg =
        crate::edit::render_rotation(&path, 2.0_f64.to_radians(), &FileLocks::default()).unwrap();
    apply_pixels(&mut exiftool, &path, &jpeg, &db).unwrap();
    let after = fs::read(&path).unwrap();
    let after_meta = metadata::read(&after);
    assert_eq!(after_meta.orientation, 1);
    let decoded = crate::decode::decode_for_display(
        &after,
        crate::library::Format::Jpeg,
        after_meta.orientation,
        [u32::MAX; 2],
    )
    .unwrap();
    assert_eq!(
        (decoded.width, decoded.height),
        (oriented.width, oriented.height)
    );
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
    #[cfg(windows)]
    assert_eq!(fs::metadata(&path).unwrap().created().ok(), created);
    #[cfg(not(windows))]
    let _ = created;

    drop(exiftool);
    fs::remove_dir_all(&dir).unwrap();
}

/// While ExifTool's folder is replaced the writer holds the marks – past their debounce – and
/// writes them after `Resume`; a pause is acknowledged once ExifTool has ended.
#[test]
fn a_pause_holds_marks_until_resume() {
    let dir = std::env::temp_dir().join(format!("cerno-pause-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pause.jpg");
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tiny.jpg"),
        &path,
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let status = Mutex::new(WriterStatus::default());
    let outcomes = Mutex::new(Vec::new());
    let ctx = egui::Context::default();
    let db = Db::open_in_memory().unwrap();
    let files = FileLocks::default();
    let pending = || status.lock().unwrap().pending;
    let shared = Shared {
        status: &status,
        outcomes: &outcomes,
        ctx: &ctx,
        db: &db,
        files: &files,
    };
    std::thread::scope(|scope| {
        // The receiver goes to the writer's thread; the rest is borrowed.
        let shared = &shared;
        scope.spawn(move || run(&rx, shared));
        let pauser = Pauser { tx: tx.clone() };
        assert!(pauser.pause(Duration::from_secs(10)), "paused");
        tx.send(Message::SetRating {
            path: path.clone(),
            rating: Rating::Stars(3),
        })
        .unwrap();
        std::thread::sleep(DEBOUNCE + Duration::from_millis(400));
        assert_eq!(pending(), 1, "held while paused");
        pauser.resume();
        let deadline = Instant::now() + Duration::from_secs(20);
        while pending() > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(pending(), 0, "written after the pause");
        tx.send(Message::Shutdown).unwrap();
    });
    let _ = std::fs::remove_dir_all(&dir);
}
