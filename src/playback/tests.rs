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
    let _turn = test_lock();
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
    let _turn = test_lock();
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

/// The controls ask for the status every frame, also while the video starts. That must not
/// take the start's messages off the bus: a failing sound output's error then ended the video
/// instead of a start without sound (Windows CI, 1 of 10 rounds, 2026-10-11). A stall before
/// the first frame (CI, about once in 200 starts) is survived meanwhile, so each start has a
/// minute.
#[test]
fn asking_for_the_status_while_it_starts_takes_nothing_from_the_start() {
    let _turn = test_lock();
    let ctx = egui::Context::default();
    for round in 0..10 {
        let player = Player::start(&ctx, &fixture(), [320, 240], 0.5, false, Audio::Broken)
            .expect("started");
        let until = Instant::now() + Duration::from_secs(60);
        let first = loop {
            let status = player.status();
            assert_eq!(status.error, None, "round {round}");
            if status.frame_size.is_some() {
                break status;
            }
            assert!(Instant::now() < until, "round {round}: {status:?}");
            std::thread::yield_now();
        };
        assert!(first.silent, "round {round}: {first:?}");
        drop(player);
    }
}

#[test]
fn a_missing_file_reports_an_error() {
    let _turn = test_lock();
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
    let _turn = test_lock();
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
    let _turn = test_lock();
    if let Ok(path) = std::env::var("CERNO_PROBE") {
        println!("{:#?}", probe(Path::new(&path)));
    }
}

/// `CERNO_VIDEO_DIR=<folder> cargo test --features video -- --ignored --nocapture
/// first_frame_times` plays every video of a folder until its first frame, one after the
/// other, and prints how long each took (only reads the files). GStreamer is loaded before;
/// the first video of a process still pays for setting up D3D12. `CERNO_VIDEO_CPU=1` measures
/// the CPU pipeline instead.
#[test]
#[ignore = "set CERNO_VIDEO_DIR"]
fn first_frame_times() {
    let _turn = test_lock();
    let Ok(dir) = std::env::var("CERNO_VIDEO_DIR") else {
        return;
    };
    ready().expect("GStreamer");
    let mut videos: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the folder")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| crate::library::format_of(path) == Some(crate::library::Format::Video))
        .collect();
    videos.sort();
    let cpu_only = std::env::var_os("CERNO_VIDEO_CPU").is_some();
    let ctx = egui::Context::default();
    let mut times = Vec::new();
    for path in &videos {
        let started = Instant::now();
        let start = engine::Start {
            cpu_only,
            ..engine::Start::default()
        };
        let player = Player::start_with(&ctx, path, [1920, 1080], 0.0, true, Audio::Silent, start)
            .expect("started");
        let until = started + Duration::from_secs(30);
        let status = loop {
            let status = player.status();
            if status.frame_size.is_some() || status.error.is_some() || Instant::now() > until {
                break status;
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        let ms = started.elapsed().as_millis();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        match (&status.error, status.frame_size) {
            (None, Some(_)) => {
                println!("{ms:>6} ms  {}  {name}", status.pipeline);
                times.push(ms);
            }
            _ => println!("  none after {ms} ms  {name}: {status:?}"),
        }
        let release = player.stop();
        while !release.done() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    if let Some(&first) = times.first() {
        let mut sorted = times.clone();
        sorted.sort_unstable();
        let at = |share: f64| sorted[((sorted.len() - 1) as f64 * share).round() as usize];
        println!(
            "{} videos: first {first} ms, median {} ms, p90 {} ms, p99 {} ms, max {} ms",
            sorted.len(),
            at(0.5),
            at(0.9),
            at(0.99),
            at(1.0)
        );
    }
}

/// On Windows the fixture plays on D3D12, on Linux on the CPU – or, after a stall (CI), on the
/// CPU, which the report says. A plain CPU pipeline on Windows would mean D3D12 was never tried.
#[test]
fn the_self_test_plays_the_fixture() {
    let _turn = test_lock();
    let report = self_test(&fixture()).expect("self-test");
    assert!(report.starts_with("video ok: 64x64"), "{report}");
    assert!(report.contains("closed true"), "{report}");
    assert!(report.contains("MP4, H.264"), "{report}");
    let expected = if cfg!(windows) {
        "pipeline D3D12,"
    } else {
        "pipeline CPU,"
    };
    assert!(
        report.contains(expected) || report.contains("pipeline CPU after a stall,"),
        "{report}"
    );
}

/// D3D12 that shows no first frame makes way for the CPU. Nothing is remembered: the next video
/// tries D3D12 again – a stall has been a rare race on CI, not a broken GPU.
#[cfg(windows)]
#[test]
fn a_stalled_gpu_makes_way_for_the_cpu() {
    let _turn = test_lock();
    ready().expect("GStreamer");
    assert!(
        gstreamer::ElementFactory::find("d3d12convert").is_some(),
        "GStreamer without D3D12"
    );
    let stalling = || engine::Start {
        gpu_limit: Duration::from_secs(1),
        stall_gpu: true,
        ..engine::Start::default()
    };
    let ctx = egui::Context::default();
    for video in ["first", "second"] {
        let started = Instant::now();
        let player = Player::start_with(
            &ctx,
            &fixture(),
            [320, 240],
            0.0,
            true,
            Audio::Silent,
            stalling(),
        )
        .expect("started");
        let first = wait(&player, video, |s| s.frame_size.is_some());
        assert_eq!(first.pipeline, Pipeline::CpuAfterStall, "{video}");
        assert!(
            started.elapsed() >= Duration::from_secs(1),
            "{video}: D3D12 had its second"
        );
        let ended = wait(&player, "end", |s| s.ended);
        assert_eq!(ended.error, None, "{video}");
        drop(player);
    }
}
