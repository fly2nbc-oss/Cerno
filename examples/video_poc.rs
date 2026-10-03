//! Proof of concept for playing videos inside Cerno (not shipped). Plays a file through
//! GStreamer into memory the way Cerno would – `playbin3`, the frame turned upright, scaled
//! and converted to RGBA, an `appsink` at the presentation clock – and reports what happened.
//!
//!     cargo run --release --features video --example video_poc -- <file> [--sw] [--size WxH]
//!         [--png frame.png] [--sound]
//!
//! `--sw` demotes every hardware decoder (what a machine without a GPU, like a CI runner, gets),
//! `--size` asks for that output size (default: the video's own), `--png` keeps frame 30.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

#[derive(Default)]
struct Frames {
    count: u64,
    first_at: Option<Instant>,
    size: Option<(u32, u32)>,
    last_pts: Option<gst::ClockTime>,
    /// The copy into an RGBA buffer the size of the frame, as a texture upload would need.
    copy: Duration,
    png: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = PathBuf::from(
        args.first()
            .context("usage: video_poc <file> [--sw] [--size WxH] [--png out.png] [--sound]")?,
    );
    let sw = args.iter().any(|a| a == "--sw");
    let sound = args.iter().any(|a| a == "--sound");
    let size = value(&args, "--size").map(|s| parse_size(&s)).transpose()?;
    let png = value(&args, "--png").map(PathBuf::from);
    let threads: u32 = value(&args, "--threads").map_or(Ok(1), |t| t.parse())?;

    let started = Instant::now();
    gst::init()?;
    let init = started.elapsed();
    let plugins = gst::Registry::get().plugins().len();
    println!(
        "init: {} ms, {plugins} plugins, {}",
        init.as_millis(),
        gst::version_string()
    );
    if sw {
        let demoted = demote_hardware_decoders();
        println!("software only: demoted {demoted}");
    }

    let uri = gst::glib::filename_to_uri(std::path::absolute(&path)?, None)?;
    let playbin = gst::ElementFactory::make("playbin3")
        .property("uri", uri.as_str())
        .build()?;
    let gpu = args.iter().any(|a| a == "--gpu");
    let size_caps = size.map_or(String::new(), |(w, h)| {
        format!(",width=[1,{w}],height=[1,{h}],pixel-aspect-ratio=1/1")
    });
    // `--gpu`: turned, converted and scaled on the GPU (D3D12), downloaded as RGBA at the
    // end; else on the CPU.
    let description = if gpu {
        format!(
            "d3d12upload ! d3d12convert video-direction=auto              ! video/x-raw(memory:D3D12Memory),format=RGBA{size_caps} ! d3d12download              ! video/x-raw,format=RGBA ! appsink name=sink"
        )
    } else {
        format!(
            "videoflip video-direction=auto ! videoconvertscale n-threads={threads}              ! video/x-raw,format=RGBA{size_caps} ! appsink name=sink"
        )
    };
    let sink_bin = gst::parse::bin_from_description(&description, true)?;
    let appsink = sink_bin
        .by_name("sink")
        .and_downcast::<gst_app::AppSink>()
        .context("appsink")?;
    appsink.set_property("sync", true);
    appsink.set_max_buffers(2);
    appsink.set_drop(true);

    let frames = Arc::new(Mutex::new(Frames {
        png,
        ..Frames::default()
    }));
    let on_sample = {
        let frames = Arc::clone(&frames);
        move |sample: gst::Sample| -> Result<(), gst::FlowError> {
            let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
            let caps = sample.caps().ok_or(gst::FlowError::Error)?;
            let info = gst_video::VideoInfo::from_caps(caps).map_err(|_| gst::FlowError::Error)?;
            let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
                .map_err(|_| gst::FlowError::Error)?;
            let copy_started = Instant::now();
            let (w, h) = (info.width() as usize, info.height() as usize);
            let stride = info.stride()[0] as usize;
            let data = frame.plane_data(0).map_err(|_| gst::FlowError::Error)?;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for row in 0..h {
                rgba.extend_from_slice(&data[row * stride..row * stride + w * 4]);
            }
            let mut f = frames.lock().unwrap();
            f.copy += copy_started.elapsed();
            f.count += 1;
            f.first_at.get_or_insert_with(Instant::now);
            f.size = Some((w as u32, h as u32));
            f.last_pts = buffer.pts();
            if f.count == 30
                && let Some(png) = f.png.take()
                && let Some(image) = image::RgbaImage::from_raw(w as u32, h as u32, rgba)
            {
                let _ = image.save(&png);
            }
            Ok(())
        }
    };
    let preroll = on_sample.clone();
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                on_sample(sample).map(|()| gst::FlowSuccess::Ok)
            })
            .new_preroll(move |sink| {
                let sample = sink.pull_preroll().map_err(|_| gst::FlowError::Eos)?;
                preroll(sample).map(|()| gst::FlowSuccess::Ok)
            })
            .build(),
    );
    playbin.set_property("video-sink", &sink_bin);
    if !sound {
        playbin.set_property("mute", true);
    }
    let bus = playbin.bus().context("bus")?;

    // Start: the first frame on screen.
    let start = Instant::now();
    playbin.set_state(gst::State::Playing)?;
    wait(&bus, Duration::from_secs(15), || {
        frames.lock().unwrap().first_at.is_some()
    })?;
    let first = start.elapsed();
    let duration = playbin
        .query_duration::<gst::ClockTime>()
        .context("no duration")?;
    let (w, h) = frames.lock().unwrap().size.unwrap_or_default();
    println!(
        "first frame: {} ms, {w}x{h}, duration {:.1} s",
        first.as_millis(),
        duration.seconds_f64()
    );
    println!("elements: {}", elements(&playbin).join(", "));

    // Play: frames per second delivered at the clock.
    let play_for = Duration::from_secs(5).min(Duration::from_secs_f64(
        (duration.seconds_f64() - 1.5).max(1.0),
    ));
    let before = frames.lock().unwrap().count;
    let played = Instant::now();
    wait(&bus, play_for, || false).ok();
    let delivered = frames.lock().unwrap().count - before;
    let fps = delivered as f64 / played.elapsed().as_secs_f64();
    let copy = {
        let f = frames.lock().unwrap();
        f.copy.as_secs_f64() * 1000.0 / f.count.max(1) as f64
    };
    println!("play: {fps:.1} fps delivered, copy {copy:.2} ms per frame");

    // Pause: no frames while paused.
    playbin.set_state(gst::State::Paused)?;
    std::thread::sleep(Duration::from_millis(200));
    let paused = frames.lock().unwrap().count;
    std::thread::sleep(Duration::from_millis(400));
    println!(
        "pause: {} frames while paused",
        frames.lock().unwrap().count - paused
    );

    // Seek while paused, keyframe then accurate: the preroll frame shows the new place.
    for (name, flags) in [
        ("keyframe", gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT),
        ("accurate", gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE),
    ] {
        let target = duration / 2 + gst::ClockTime::from_mseconds(370);
        let count = frames.lock().unwrap().count;
        let seek = Instant::now();
        playbin.seek_simple(flags, target)?;
        wait(&bus, Duration::from_secs(10), || {
            frames.lock().unwrap().count > count
        })?;
        let pts = frames.lock().unwrap().last_pts;
        println!(
            "seek {name}: {} ms, asked {:.3} s, got {:.3} s",
            seek.elapsed().as_millis(),
            target.seconds_f64(),
            pts.map_or(f64::NAN, |p| p.seconds_f64())
        );
    }

    // A playing file: can it be renamed (what deleting into .originals and moving do)?
    playbin.set_state(gst::State::Playing)?;
    std::thread::sleep(Duration::from_millis(300));
    let renamed = path.with_extension("poc-renamed");
    match std::fs::rename(&path, &renamed) {
        Ok(()) => {
            println!("rename while playing: possible");
            std::fs::rename(&renamed, &path)?;
        }
        Err(err) => println!("rename while playing: refused ({err})"),
    }

    // The end: EOS arrives.
    let end = duration.saturating_sub(gst::ClockTime::from_mseconds(600));
    playbin.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, end)?;
    let eos = Instant::now();
    match bus.timed_pop_filtered(
        gst::ClockTime::from_seconds(10),
        &[gst::MessageType::Eos, gst::MessageType::Error],
    ) {
        Some(msg) => match msg.view() {
            gst::MessageView::Eos(..) => println!("eos: after {} ms", eos.elapsed().as_millis()),
            gst::MessageView::Error(err) => println!("eos: error {}", err.error()),
            _ => {}
        },
        None => println!("eos: none within 10 s"),
    }

    let stop = Instant::now();
    playbin.set_state(gst::State::Null)?;
    println!("stop (Null): {} ms", stop.elapsed().as_millis());
    println!("total: {} ms", started.elapsed().as_millis());
    Ok(())
}

/// Pumps the bus until `done` or the time is up; an error message ends it with that error.
fn wait(bus: &gst::Bus, limit: Duration, done: impl Fn() -> bool) -> Result<()> {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if done() {
            return Ok(());
        }
        if let Some(msg) = bus.timed_pop_filtered(
            gst::ClockTime::from_mseconds(20),
            &[gst::MessageType::Error],
        ) && let gst::MessageView::Error(err) = msg.view()
        {
            bail!(
                "{} ({})",
                err.error(),
                err.debug().map(|d| d.to_string()).unwrap_or_default()
            );
        }
    }
    if done() { Ok(()) } else { bail!("timed out") }
}

/// The factory names of every element in the pipeline, decoders first.
fn elements(pipeline: &gst::Element) -> Vec<String> {
    let mut names: Vec<String> = pipeline
        .downcast_ref::<gst::Bin>()
        .map(|bin| {
            bin.iterate_recurse()
                .into_iter()
                .flatten()
                .filter_map(|element| element.factory().map(|f| f.name().to_string()))
                .collect()
        })
        .unwrap_or_default();
    names.sort_by_key(|name| (!name.contains("dec"), name.clone()));
    names.dedup();
    names
}

/// Sets every element whose class says hardware decoder to rank none.
fn demote_hardware_decoders() -> usize {
    let mut demoted = 0;
    for feature in gst::Registry::get().features(gst::ElementFactory::static_type()) {
        if let Ok(factory) = feature.clone().downcast::<gst::ElementFactory>() {
            let klass = factory.klass();
            if klass.contains("Decoder") && klass.contains("Hardware") {
                feature.set_rank(gst::Rank::NONE);
                demoted += 1;
            }
        }
    }
    demoted
}

fn value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn parse_size(text: &str) -> Result<(u32, u32)> {
    let (w, h) = text.split_once('x').context("size as WxH")?;
    Ok((w.parse()?, h.parse()?))
}

#[allow(dead_code)]
fn exists(path: &Path) -> bool {
    path.exists()
}
