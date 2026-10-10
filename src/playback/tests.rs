
use super::*;
use std::path::PathBuf;
use std::time::Instant;

fn fixture() -> PathBuf {
    std::path::absolute(Path::new("tests/fixtures/tiny.mp4"))
        .unwrap_or_else(|_| PathBuf::from("tests/fixtures/tiny.mp4"))
}

/// A development build finds the user's GStreamer by its core DLL, not by the folder alone.
#[cfg(windows)]
#[test]
fn the_installed_gstreamer_is_found_by_its_dll() {
    let root = std::env::temp_dir().join(format!("cerno-gst-{}", std::process::id()));
    let bin = root.join(r"Programs\gstreamer\1.0\msvc_x86_64\bin");
    std::fs::create_dir_all(&bin).unwrap();
    assert_eq!(engine::installed_bin(&root), None);
    std::fs::write(bin.join("gstreamer-1.0-0.dll"), b"").unwrap();
    assert_eq!(engine::installed_bin(&root), Some(bin));
    std::fs::remove_dir_all(&root).unwrap();
}

fn wait(player: &Player, what: &str, done: impl Fn(&Status) -> bool) -> Status {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let status = player.status();
        assert_eq!(status.error, None, "{what}");
        if done(&status) {
            return status;
        }
        assert!(Instant::now() < until, "{what}: {status:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 1 s, 64×64, H.264 + AAC: plays, fits (never enlarged), pauses, jumps, ends, closes.
#[test]
fn a_video_plays_pauses_seeks_ends_and_closes() {
    let ctx = egui::Context::default();
    let player =
        Player::start(&ctx, &fixture(), [320, 240], 0.5, false, Audio::Silent).expect("started");
    let first = wait(&player, "first frame", |s| s.frame_size.is_some());
    assert_eq!(first.frame_size, Some([64, 64]), "not enlarged");
    assert!(player.texture().is_some());
    let duration = first.duration.expect("duration");
    assert!((900..=1100).contains(&duration.as_millis()), "{duration:?}");
    assert!(first.frame_time.is_some());

    player.toggle();
    assert!(!player.status().playing);
    player.seek(Duration::from_millis(500), true);
    wait(&player, "seek", |s| {
        s.position >= Duration::from_millis(400) && s.position <= Duration::from_millis(600)
    });
    player.toggle();
    let ended = wait(&player, "end", |s| s.ended);
    assert!(!ended.playing);

    let release = player.stop();
    let until = Instant::now() + Duration::from_secs(10);
    while !release.done() {
        assert!(Instant::now() < until, "the file stays open");
        std::thread::sleep(Duration::from_millis(10));
    }
    // Closed: the file can be renamed (Windows refuses while it plays).
    let copy = std::env::temp_dir().join(format!("cerno-video-{}.mp4", std::process::id()));
    std::fs::copy(fixture(), &copy).unwrap();
    let player = Player::start(&ctx, &copy, [320, 240], 0.0, true, Audio::Silent).unwrap();
    wait(&player, "first frame", |s| s.frame_size.is_some());
    let release = player.stop();
    while !release.done() {
        std::thread::sleep(Duration::from_millis(10));
    }
    let renamed = copy.with_extension("renamed.mp4");
    std::fs::rename(&copy, &renamed).expect("closed after the release");
    std::fs::remove_file(&renamed).unwrap();
}

/// No sound device (a remote session, nothing plugged in): the video plays on without
/// sound instead of failing, and the status says so.
#[test]
fn a_failing_sound_output_plays_on_without_sound() {
    let ctx = egui::Context::default();
    let player =
        Player::start(&ctx, &fixture(), [320, 240], 0.5, false, Audio::Broken).expect("started");
    let first = wait(&player, "first frame without sound", |s| {
        s.frame_size.is_some()
    });
    assert!(first.silent, "{first:?}");
    assert_eq!(first.error, None);
    let ended = wait(&player, "end", |s| s.ended);
    assert!(ended.silent);
    let release = player.stop();
    let until = Instant::now() + Duration::from_secs(10);
    while !release.done() {
        assert!(Instant::now() < until, "the file stays open");
        std::thread::sleep(Duration::from_millis(10));
    }
    // A working output stays a working one.
    let player =
        Player::start(&ctx, &fixture(), [320, 240], 0.0, true, Audio::Silent).expect("started");
    let first = wait(&player, "first frame", |s| s.frame_size.is_some());
    assert!(!first.silent);
    drop(player);
}

#[test]
fn a_missing_file_reports_an_error() {
    let ctx = egui::Context::default();
    let player = Player::start(
        &ctx,
        Path::new("C:/does/not/exist.mp4"),
        [320, 240],
        0.0,
        true,
        Audio::Silent,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    while player.status().error.is_none() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    let release = player.stop();
    let until = Instant::now() + Duration::from_secs(10);
    while !release.done() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 1 s, 64×64 at 15 fps, H.264 + AAC (a 440 Hz sine, mono, 48 kHz) in MP4.
#[test]
fn the_probe_reads_streams_and_rates() {
    let info = probe(&fixture()).expect("probed");
    assert!(
        info.duration
            .is_some_and(|d| (900..=1100).contains(&d.as_millis())),
        "{info:?}"
    );
    assert!(info.bitrate.is_some_and(|b| b > 0), "{info:?}");
    assert_eq!(info.container.as_deref(), Some("MP4"), "{info:?}");
    let video = info.video.as_ref().expect("video stream");
    assert!(video.codec.contains("H.264"), "{video:?}");
    assert_eq!((video.width, video.height), (64, 64));
    // MP4 states no rate: it is counted from the frames and the length – 14 or 15 for a
    // 1-s clip of 15 frames.
    assert!(
        video.fps.is_some_and(|f| (13.5..=15.5).contains(&f)),
        "{video:?}"
    );
    assert_eq!(video.hdr, None);
    assert!(video.bitrate.is_some_and(|b| b > 0), "{video:?}");
    let audio = info.audio.as_ref().expect("audio stream");
    assert!(audio.codec.contains("AAC"), "{audio:?}");
    assert_eq!((audio.channels, audio.sample_rate), (1, 48_000));
    assert!(
        probe(Path::new("tests/fixtures/tiny.jpg")).is_err() || {
            // An image is no video: no video stream with a duration.
            let jpeg = probe(Path::new("tests/fixtures/tiny.jpg")).unwrap();
            jpeg.duration.is_none() && jpeg.audio.is_none()
        }
    );
}

/// `CERNO_PROBE=<file> cargo test --features video -- --ignored --nocapture probe_a_file`
/// prints what the details panel would show for any video.
#[test]
#[ignore = "set CERNO_PROBE"]
fn probe_a_file() {
    if let Ok(path) = std::env::var("CERNO_PROBE") {
        println!("{:#?}", probe(Path::new(&path)));
    }
}

#[test]
fn the_self_test_plays_the_fixture() {
    let report = self_test(&fixture()).expect("self-test");
    assert!(report.starts_with("video ok: 64x64"), "{report}");
    assert!(report.contains("closed true"), "{report}");
    assert!(report.contains("MP4, H.264"), "{report}");
}
