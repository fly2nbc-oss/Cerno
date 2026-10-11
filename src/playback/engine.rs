//! GStreamer behind the player (feature `video`).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use anyhow::{Context as _, Result, anyhow};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::*;

use super::*;

use crate::sync::lock;

static INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// How long the D3D12 pipeline gets for its first frame before the CPU one is tried. D3D12
/// has stalled without an error – on CI's software device, never yet on a GPU – and the video
/// then stayed black. Measured 2026-10-11 (`first_frame_times`): the author's 162 phone videos
/// took at most 0.76 s on the GPU (median 0.06–0.09 s, a process's first 0.34–0.43 s); CI's
/// software device once needed 8.7 s for its first, cold start.
const GPU_FIRST_FRAME: Duration = Duration::from_secs(10);

/// How long the CPU pipeline gets for its first frame; then the video fails.
const FIRST_FRAME: Duration = Duration::from_secs(20);

/// D3D12 showed no first frame in time once: the session's later videos start on the CPU.
static GPU_STALLED: AtomicBool = AtomicBool::new(false);

/// How a player starts: as Cerno does, or – in tests – with a D3D12 attempt that never shows
/// a frame, a shorter limit and a stall remembered apart from the session's.
pub(super) struct Start {
    pub(super) gpu_limit: Duration,
    pub(super) stalled: &'static AtomicBool,
    #[cfg(test)]
    pub(super) stall_gpu: bool,
}

impl Default for Start {
    fn default() -> Self {
        Self {
            gpu_limit: GPU_FIRST_FRAME,
            stalled: &GPU_STALLED,
            #[cfg(test)]
            stall_gpu: false,
        }
    }
}

/// The folder of the plugins Cerno ships, beside the exe (Windows package).
fn bundled_plugins() -> Option<PathBuf> {
    let dir = std::env::current_exe()
        .ok()?
        .parent()?
        .join("gstreamer-1.0");
    dir.is_dir().then_some(dir)
}

/// `bin` of the GStreamer `install-gstreamer.ps1` installs for the user, when it is there.
#[cfg(windows)]
pub(super) fn installed_bin(local_app_data: &Path) -> Option<PathBuf> {
    let bin = local_app_data.join(r"Programs\gstreamer\1.0\msvc_x86_64\bin");
    bin.join("gstreamer-1.0-0.dll").is_file().then_some(bin)
}

/// A build without GStreamer beside it (`cargo build`, started from Explorer) takes the
/// user's installed one, unless `PATH` already has a GStreamer: its `bin` goes in front of
/// `PATH`, where the delay-loaded DLLs, the plugins (found from the DLL's place) and the
/// frame helpers (they inherit `PATH`) find it. The packages carry their own.
#[cfg(windows)]
fn use_installed() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    if std::env::split_paths(&path).any(|dir| dir.join("gstreamer-1.0-0.dll").is_file()) {
        return;
    }
    let Some(bin) = std::env::var_os("LOCALAPPDATA").and_then(|dir| installed_bin(Path::new(&dir)))
    else {
        return;
    };
    let Ok(joined) = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path)))
    else {
        return;
    };
    // SAFETY: called at the start of `main`, before any thread exists.
    unsafe { std::env::set_var("PATH", joined) };
}

pub(super) fn configure_environment() {
    let Some(plugins) = bundled_plugins() else {
        #[cfg(windows)]
        use_installed();
        return;
    };
    let registry = crate::paths::data_dir()
        .ok()
        .map(|dir| dir.join("gstreamer-registry.bin"));
    // SAFETY: called at the start of `main`, before any thread exists.
    unsafe {
        std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", &plugins);
        std::env::remove_var("GST_PLUGIN_PATH_1_0");
        std::env::remove_var("GST_PLUGIN_PATH");
        // Scan in this process: no gst-plugin-scanner to ship.
        std::env::set_var("GST_REGISTRY_FORK", "no");
        if let Some(registry) = registry {
            std::env::set_var("GST_REGISTRY_1_0", registry);
        }
    }
}

pub(super) fn warm_up() {
    if INIT.get().is_none() {
        let _ = std::thread::Builder::new()
            .name("cerno-gst-init".into())
            .spawn(|| {
                let _ = init();
            });
    }
}

/// GStreamer, loaded once; a second caller waits for the first.
pub(super) fn init() -> Result<()> {
    INIT.get_or_init(|| {
        let started = Instant::now();
        // A GStreamer missing beside the exe would end Cerno at the first call.
        let result = crate::dlls::load(crate::dlls::GSTREAMER)
            .and_then(|()| gst::init().map_err(|err| format!("{err}")));
        match &result {
            Ok(()) => log::info!(
                "video: {} ready after {} ms, {} plugins",
                gst::version_string(),
                started.elapsed().as_millis(),
                gst::Registry::get().plugins().len()
            ),
            Err(err) => log::error!("video: GStreamer: {err}"),
        }
        result
    })
    .clone()
    .map_err(|err| anyhow!("GStreamer: {err}"))
}

#[derive(Default)]
struct Frame {
    size: Option<[u32; 2]>,
    frame_time: Option<Duration>,
}

/// Shared with the start thread and GStreamer's streaming threads.
struct Inner {
    ctx: egui::Context,
    path: PathBuf,
    audio: Audio,
    start: Start,
    pipeline: Mutex<Option<gst::Element>>,
    /// The video sink, for a new target size.
    sink: Mutex<Option<gst::Bin>>,
    texture: Mutex<Option<egui::TextureHandle>>,
    frame: Mutex<Frame>,
    target: Mutex<[u32; 2]>,
    volume: Mutex<(f32, bool)>,
    want_playing: AtomicBool,
    ended: AtomicBool,
    error: Mutex<Option<String>>,
    /// The pipeline of the current attempt.
    kind: Mutex<Pipeline>,
    /// The sound output failed; the video plays without it.
    silent: AtomicBool,
    /// Stop asked for: frames are no longer taken, a pipeline still being built is torn
    /// down as soon as it exists.
    stop: AtomicBool,
    /// The start thread is through: it will build no pipeline any more.
    start_done: AtomicBool,
    /// Held while a pipeline goes down, so the file counts as closed only after it did.
    teardown: Mutex<()>,
    released: Release,
}

pub struct Player {
    inner: Arc<Inner>,
    stopped: bool,
}

impl Player {
    /// Starts playing `path` in the background; the first frame arrives within a few
    /// hundred milliseconds (seconds when GStreamer still scans its plugins).
    pub fn start(
        ctx: &egui::Context,
        path: &Path,
        target: [u32; 2],
        volume: f32,
        muted: bool,
        audio: Audio,
    ) -> Result<Self, String> {
        Self::start_with(ctx, path, target, volume, muted, audio, Start::default())
    }

    pub(super) fn start_with(
        ctx: &egui::Context,
        path: &Path,
        target: [u32; 2],
        volume: f32,
        muted: bool,
        audio: Audio,
        start: Start,
    ) -> Result<Self, String> {
        let inner = Arc::new(Inner {
            ctx: ctx.clone(),
            path: path.to_owned(),
            audio,
            start,
            pipeline: Mutex::new(None),
            sink: Mutex::new(None),
            texture: Mutex::new(None),
            frame: Mutex::new(Frame::default()),
            target: Mutex::new(target),
            volume: Mutex::new((volume, muted)),
            want_playing: AtomicBool::new(true),
            ended: AtomicBool::new(false),
            error: Mutex::new(None),
            kind: Mutex::new(Pipeline::None),
            silent: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            start_done: AtomicBool::new(false),
            teardown: Mutex::new(()),
            released: Release::default(),
        });
        let starting = Arc::clone(&inner);
        std::thread::Builder::new()
            .name("cerno-video-start".into())
            .spawn(move || {
                if let Err(err) = run(&starting) {
                    log::error!("video {}: {err:#}", starting.path.display());
                    *lock(&starting.error) = Some(format!("{err:#}"));
                }
                starting.start_done.store(true, Ordering::Release);
                // Stopped meanwhile (or failed): nobody else takes it down.
                if starting.stop.load(Ordering::Acquire) || lock(&starting.error).is_some() {
                    teardown(&starting);
                }
                starting.ctx.request_repaint();
            })
            .map_err(|err| err.to_string())?;
        Ok(Self {
            inner,
            stopped: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// The last frame, once one arrived.
    pub fn texture(&self) -> Option<egui::TextureHandle> {
        lock(&self.inner.texture).clone()
    }

    pub fn status(&self) -> Status {
        let inner = &self.inner;
        let pipeline = lock(&inner.pipeline).clone();
        if let Some(bus) = pipeline.as_ref().and_then(|p| p.bus()) {
            while let Some(message) = bus.pop() {
                match message.view() {
                    gst::MessageView::Eos(..) => inner.ended.store(true, Ordering::Release),
                    gst::MessageView::Error(err) => {
                        log::error!("video: {} ({:?})", err.error(), err.debug());
                        *lock(&inner.error) = Some(err.error().to_string());
                    }
                    _ => {}
                }
            }
        }
        let to_duration = |t: gst::ClockTime| Duration::from_nanos(t.nseconds());
        let frame = lock(&inner.frame);
        let ended = inner.ended.load(Ordering::Acquire);
        Status {
            starting: frame.size.is_none(),
            playing: inner.want_playing.load(Ordering::Acquire) && !ended,
            ended,
            position: pipeline
                .as_ref()
                .and_then(|p| p.query_position::<gst::ClockTime>())
                .map_or(Duration::ZERO, to_duration),
            duration: pipeline
                .as_ref()
                .and_then(|p| p.query_duration::<gst::ClockTime>())
                .map(to_duration),
            frame_size: frame.size,
            frame_time: frame.frame_time,
            silent: inner.silent.load(Ordering::Acquire),
            pipeline: if frame.size.is_some() {
                *lock(&inner.kind)
            } else {
                Pipeline::None
            },
            error: lock(&inner.error).clone(),
        }
    }

    /// Play or pause; at the end, play again from the beginning.
    pub fn toggle(&self) {
        let inner = &self.inner;
        if inner.ended.swap(false, Ordering::AcqRel) {
            inner.want_playing.store(true, Ordering::Release);
            self.seek_to(Duration::ZERO, gst::SeekFlags::KEY_UNIT);
        } else {
            inner.want_playing.fetch_xor(true, Ordering::AcqRel);
        }
        apply_state(inner);
    }

    /// To `to`: exactly (slower, it decodes from the keyframe before), or to the nearest
    /// keyframe (while the timeline is dragged).
    pub fn seek(&self, to: Duration, accurate: bool) {
        self.inner.ended.store(false, Ordering::Release);
        let flags = if accurate {
            gst::SeekFlags::ACCURATE
        } else {
            gst::SeekFlags::KEY_UNIT | gst::SeekFlags::SNAP_NEAREST
        };
        self.seek_to(to, flags);
    }

    fn seek_to(&self, to: Duration, flags: gst::SeekFlags) {
        if let Some(pipeline) = lock(&self.inner.pipeline).as_ref() {
            let to = gst::ClockTime::from_nseconds(to.as_nanos() as u64);
            if let Err(err) = pipeline.seek_simple(gst::SeekFlags::FLUSH | flags, to) {
                log::warn!("video: seek: {err}");
            }
        }
    }

    /// One frame on or back; pauses first.
    pub fn step(&self, forward: bool) {
        let inner = &self.inner;
        inner.want_playing.store(false, Ordering::Release);
        apply_state(inner);
        let status = self.status();
        if forward {
            if let Some(pipeline) = lock(&inner.pipeline).as_ref() {
                let step = gst::event::Step::new(gst::format::Buffers::ONE, 1.0, true, false);
                pipeline.send_event(step);
            }
        } else {
            let back = status.frame_time.unwrap_or(Duration::from_millis(33));
            self.seek(status.position.saturating_sub(back), true);
        }
    }

    pub fn set_volume(&self, volume: f32, muted: bool) {
        *lock(&self.inner.volume) = (volume, muted);
        if let Some(pipeline) = lock(&self.inner.pipeline).as_ref() {
            apply_volume(pipeline, volume, muted);
        }
    }

    /// The photo area changed: frames come at the new size (fitted, never enlarged).
    pub fn set_target(&self, target: [u32; 2]) {
        let inner = &self.inner;
        if std::mem::replace(&mut *lock(&inner.target), target) == target {
            return;
        }
        let fit = lock(&inner.sink)
            .as_ref()
            .and_then(|sink| sink.by_name("fit"));
        let caps = sink_caps(*lock(&inner.kind) == Pipeline::Gpu, target);
        if let Some(fit) = fit
            && let Ok(caps) = caps.parse::<gst::Caps>()
        {
            fit.set_property("caps", caps);
        }
    }

    /// Stops in the background (taking the pipeline down takes ~50 ms); the file is
    /// closed once the returned [`Release`] says so.
    pub fn stop(mut self) -> Release {
        self.stopped = true;
        stop(&self.inner)
    }

    /// The elements that decode the file, for the self-test and the log.
    fn decoders(&self) -> Vec<String> {
        let Some(bin) = lock(&self.inner.pipeline)
            .clone()
            .and_then(|p| p.downcast::<gst::Bin>().ok())
        else {
            return Vec::new();
        };
        let mut names: Vec<String> = bin
            .iterate_recurse()
            .into_iter()
            .flatten()
            .filter_map(|element| element.factory())
            .filter(|factory| factory.klass().contains("Decoder"))
            .map(|factory| factory.name().to_string())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if !self.stopped {
            stop(&self.inner);
        }
    }
}

fn stop(inner: &Arc<Inner>) -> Release {
    inner.stop.store(true, Ordering::Release);
    let release = inner.released.clone();
    let inner = Arc::clone(inner);
    let spawned = std::thread::Builder::new()
        .name("cerno-video-stop".into())
        .spawn(move || teardown(&inner));
    if let Err(err) = spawned {
        log::error!("video: cannot stop in the background: {err}");
    }
    release
}

/// Takes the pipeline down (if there is one) and, once the start thread is through, says
/// the file is closed. The stop thread and the start thread both call it; the lock makes
/// the second wait until the first has the pipeline down.
fn teardown(inner: &Inner) {
    let _down = lock(&inner.teardown);
    let pipeline = lock(&inner.pipeline).take();
    *lock(&inner.sink) = None;
    if let Some(pipeline) = pipeline {
        let _ = pipeline.set_state(gst::State::Null);
    }
    if inner.start_done.load(Ordering::Acquire) {
        inner.released.set();
    }
}

fn apply_state(inner: &Inner) {
    let state = if inner.want_playing.load(Ordering::Acquire) {
        gst::State::Playing
    } else {
        gst::State::Paused
    };
    if let Some(pipeline) = lock(&inner.pipeline).as_ref()
        && let Err(err) = pipeline.set_state(state)
    {
        log::warn!("video: {state:?}: {err}");
    }
}

fn apply_volume(pipeline: &gst::Element, volume: f32, muted: bool) {
    pipeline.set_property("volume", f64::from(volume.clamp(0.0, 1.0)));
    pipeline.set_property("mute", muted);
}

/// Why an attempt failed: the sound output (the video can play on without sound), no first
/// frame in time (D3D12 makes way for the CPU), or anything else.
enum Failure {
    Audio(anyhow::Error),
    Stalled(anyhow::Error),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for Failure {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<gst::glib::BoolError> for Failure {
    fn from(err: gst::glib::BoolError) -> Self {
        Self::Other(err.into())
    }
}

impl From<gst::glib::Error> for Failure {
    fn from(err: gst::glib::Error) -> Self {
        Self::Other(err.into())
    }
}

/// Builds the pipeline – on the GPU first where it exists – and starts it. A GPU pipeline
/// that reports an error or shows no frame in time makes way for the CPU one; after such a
/// stall the session's later videos start on the CPU. A sound output that fails (no device)
/// makes way for none: the same pipeline again, without sound.
fn run(inner: &Arc<Inner>) -> Result<()> {
    init()?;
    let stalled_before = inner.start.stalled.load(Ordering::Acquire);
    let gpu_first =
        cfg!(windows) && !stalled_before && gst::ElementFactory::find("d3d12convert").is_some();
    let attempts: &[bool] = if gpu_first { &[true, false] } else { &[false] };
    let mut after_stall = stalled_before;
    let mut audio = inner.audio;
    let mut last_error = None;
    'pipelines: for &gpu in attempts {
        loop {
            if inner.stop.load(Ordering::Acquire) {
                break 'pipelines;
            }
            let kind = match (gpu, after_stall) {
                (true, _) => Pipeline::Gpu,
                (false, false) => Pipeline::Cpu,
                (false, true) => Pipeline::CpuAfterStall,
            };
            match start_pipeline(inner, kind, audio) {
                Ok(()) => return Ok(()),
                Err(Failure::Audio(err)) if audio != Audio::Silent => {
                    log::warn!("video: no sound output ({err:#}) – playing without sound");
                    audio = Audio::Silent;
                    inner.silent.store(true, Ordering::Release);
                }
                Err(Failure::Stalled(err)) if gpu => {
                    log::warn!("video: D3D12 pipeline: {err:#} – the CPU from now on");
                    inner.start.stalled.store(true, Ordering::Release);
                    after_stall = true;
                    last_error = Some(err);
                    break;
                }
                Err(Failure::Audio(err) | Failure::Stalled(err) | Failure::Other(err)) => {
                    log::warn!("video: {kind} pipeline failed: {err:#}");
                    last_error = Some(err);
                    break;
                }
            }
        }
    }
    match last_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// The sound output of an attempt: the system's (`autoaudiosink` picks the device), none
/// (`fakesink` keeps the clock), or – in tests – one that fails like a missing device.
fn audio_sink(audio: Audio) -> Result<gst::Element, gst::glib::BoolError> {
    match audio {
        Audio::Device => gst::ElementFactory::make("autoaudiosink").build(),
        Audio::Silent => gst::ElementFactory::make("fakesink")
            .property("sync", true)
            .build(),
        #[cfg(test)]
        Audio::Broken => gst::ElementFactory::make("fakesink")
            .property_from_str("state-error", "ready-to-paused")
            .build(),
    }
}

/// Whether the error came from the sound output (or an element inside it).
fn from_sound(err: &gst::message::Error, audio_sink: &gst::Element) -> bool {
    err.src().is_some_and(|src| {
        src == audio_sink.upcast_ref::<gst::Object>() || src.has_as_ancestor(audio_sink)
    })
}

/// Whether the sound output failed its own state change. A failing sink does not always post
/// the error itself: a bin around it may report "some element failed to post a proper error
/// message" instead (Linux CI, 1.12.0). Read before the attempt goes down – that resets it.
fn sound_failed(audio_sink: &gst::Element) -> bool {
    audio_sink.state(gst::ClockTime::ZERO).0.is_err()
}

fn describe(err: &gst::message::Error) -> anyhow::Error {
    anyhow!(
        "{} ({})",
        err.error(),
        err.debug().map(|d| d.to_string()).unwrap_or_default()
    )
}

/// A failure of the sound output (the error came from it, or it failed its state change) or
/// of anything else.
fn failure(
    error: anyhow::Error,
    message: Option<&gst::message::Error>,
    audio_sink: &gst::Element,
) -> Failure {
    if message.is_some_and(|err| from_sound(err, audio_sink)) || sound_failed(audio_sink) {
        Failure::Audio(error)
    } else {
        Failure::Other(error)
    }
}

/// One attempt: the pipeline is playing and has shown its first frame (or failed). It is
/// shared from the start, so the controls see it at once and a stop takes it down.
fn start_pipeline(inner: &Arc<Inner>, kind: Pipeline, audio: Audio) -> Result<(), Failure> {
    let started = Instant::now();
    let gpu = kind == Pipeline::Gpu;
    let uri = gst::glib::filename_to_uri(&inner.path, None).context("the file name is no URI")?;
    let playbin = gst::ElementFactory::make("playbin3")
        .property("uri", uri.as_str())
        .build()?;
    let target = *lock(&inner.target);
    #[cfg_attr(not(test), allow(unused_mut))]
    let mut description = sink_description(gpu, target);
    #[cfg(test)]
    if gpu && inner.start.stall_gpu {
        // A D3D12 attempt that never shows a frame, like the stalls on CI.
        description = description.replace("appsink", "valve drop=true ! appsink");
    }
    let sink = gst::parse::bin_from_description(&description, true)?;
    let appsink = sink
        .by_name("sink")
        .and_downcast::<gst_app::AppSink>()
        .context("no appsink")?;
    appsink.set_property("sync", true);
    appsink.set_max_buffers(2);
    appsink.set_drop(true);
    let on_sample = {
        let weak = Arc::downgrade(inner);
        move |sample: gst::Sample| match weak.upgrade() {
            Some(inner) => deliver(&inner, &sample),
            None => Err(gst::FlowError::Flushing),
        }
    };
    let on_preroll = on_sample.clone();
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                on_sample(sample).map(|()| gst::FlowSuccess::Ok)
            })
            .new_preroll(move |sink| {
                let sample = sink.pull_preroll().map_err(|_| gst::FlowError::Eos)?;
                on_preroll(sample).map(|()| gst::FlowSuccess::Ok)
            })
            .build(),
    );
    playbin.set_property("video-sink", &sink);
    // Our own sound output, so an error can be traced to it.
    let audio_sink = audio_sink(audio)?;
    playbin.set_property("audio-sink", &audio_sink);
    let (volume, muted) = *lock(&inner.volume);
    apply_volume(&playbin, volume, muted);
    *lock(&inner.sink) = Some(sink);
    *lock(&inner.kind) = kind;
    *lock(&inner.pipeline) = Some(playbin.clone());
    let bus = playbin.bus().context("no bus")?;
    // The error the bus holds, or `fallback`; the attempt is taken down either way.
    let fail = |fallback: anyhow::Error| {
        let message = bus.timed_pop_filtered(
            gst::ClockTime::from_mseconds(500),
            &[gst::MessageType::Error],
        );
        let err = match message.as_ref().map(|m| m.view()) {
            Some(gst::MessageView::Error(err)) => Some(err),
            _ => None,
        };
        let failed = failure(err.map_or(fallback, describe), err, &audio_sink);
        give_back(inner, &playbin);
        failed
    };
    if let Err(err) = playbin.set_state(gst::State::Playing) {
        return Err(fail(err.into()));
    }

    // Wait for the first frame, an error, or the end (a file shorter than its preroll).
    let limit = if gpu {
        inner.start.gpu_limit
    } else {
        FIRST_FRAME
    };
    let until = started + limit;
    while Instant::now() < until {
        if lock(&inner.frame).size.is_some() || inner.stop.load(Ordering::Acquire) {
            break;
        }
        if let Some(message) = bus.timed_pop_filtered(
            gst::ClockTime::from_mseconds(20),
            &[gst::MessageType::Error, gst::MessageType::Eos],
        ) {
            match message.view() {
                gst::MessageView::Error(err) => {
                    let failed = failure(describe(err), Some(err), &audio_sink);
                    give_back(inner, &playbin);
                    return Err(failed);
                }
                _ => {
                    inner.ended.store(true, Ordering::Release);
                    break;
                }
            }
        }
    }
    // No frame, no error, no end: the pipeline hangs (D3D12 did on CI's software device).
    // Before, it counted as started and the video stayed black.
    if lock(&inner.frame).size.is_none()
        && !inner.stop.load(Ordering::Acquire)
        && !inner.ended.load(Ordering::Acquire)
    {
        give_back(inner, &playbin);
        return Err(Failure::Stalled(anyhow!(
            "no first frame within {} s",
            limit.as_secs_f32()
        )));
    }
    // The sound output may fail after the first picture: playing is reached only once
    // every sink has its device. It counts only when the sound output itself failed – a
    // pause or a seek right after the first frame interrupts the pipeline's state change
    // too (CI 2026-10-03: the test's pause and seek stopped the video with "failed to
    // change its state"). Nothing else is taken from the bus here: `status` reads it, and
    // a filtered pop would drop the end of a short clip.
    if !inner.stop.load(Ordering::Acquire)
        && let (Err(_), _, _) = playbin.state(gst::ClockTime::from_seconds(5))
        && let (Err(err), _, _) = audio_sink.state(gst::ClockTime::ZERO)
    {
        give_back(inner, &playbin);
        return Err(Failure::Audio(anyhow!("the sound output: {err}")));
    }
    log::info!(
        "video: {} first frame after {} ms ({kind})",
        inner.path.display(),
        started.elapsed().as_millis(),
    );
    Ok(())
}

/// A failed attempt: down again, unless a stop took it already.
/// The next attempt waits for its own first frame, not this one's.
fn give_back(inner: &Inner, playbin: &gst::Element) {
    let _down = lock(&inner.teardown);
    let mut shared = lock(&inner.pipeline);
    if shared.as_ref() == Some(playbin) {
        *shared = None;
        *lock(&inner.sink) = None;
        drop(shared);
        let _ = playbin.set_state(gst::State::Null);
        *lock(&inner.frame) = Frame::default();
        inner.ended.store(false, Ordering::Release);
    }
}

/// Fits the frame into `target` without enlarging it; GStreamer keeps the aspect.
fn sink_caps(gpu: bool, [w, h]: [u32; 2]) -> String {
    let memory = if gpu { "(memory:D3D12Memory)" } else { "" };
    format!(
        "video/x-raw{memory},format=RGBA,width=[1,{}],height=[1,{}],pixel-aspect-ratio=1/1",
        w.max(1),
        h.max(1)
    )
}

/// GPU: upload, turn, convert and scale on D3D12, download the RGBA frame. CPU: the same
/// with `videoflip` and `videoconvertscale`.
fn sink_description(gpu: bool, target: [u32; 2]) -> String {
    let caps = sink_caps(gpu, target);
    if gpu {
        format!(
            "d3d12upload ! d3d12convert video-direction=auto ! capsfilter name=fit \
                 caps=\"{caps}\" ! d3d12download ! video/x-raw,format=RGBA ! appsink name=sink"
        )
    } else {
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
        format!(
            "videoflip video-direction=auto ! videoconvertscale n-threads={threads} \
                 ! capsfilter name=fit caps=\"{caps}\" ! appsink name=sink"
        )
    }
}

/// Puts one frame into the texture – here, on GStreamer's thread: the UI thread only
/// draws it.
fn deliver(inner: &Inner, sample: &gst::Sample) -> Result<(), gst::FlowError> {
    if inner.stop.load(Ordering::Acquire) {
        return Err(gst::FlowError::Flushing);
    }
    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
    let caps = sample.caps().ok_or(gst::FlowError::Error)?;
    let info = gst_video::VideoInfo::from_caps(caps).map_err(|_| gst::FlowError::Error)?;
    let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
        .map_err(|_| gst::FlowError::Error)?;
    let (w, h) = (info.width() as usize, info.height() as usize);
    // The mapped frame's own stride: D3D12 pads rows (an 810-px-wide portrait frame came
    // out striped with the stride the caps imply).
    let stride = frame.info().stride()[0] as usize;
    let data = frame.plane_data(0).map_err(|_| gst::FlowError::Error)?;
    let image = if stride == w * 4 {
        egui::ColorImage::from_rgba_premultiplied([w, h], &data[..w * h * 4])
    } else {
        let mut rgba = Vec::with_capacity(w * h * 4);
        for row in 0..h {
            rgba.extend_from_slice(&data[row * stride..row * stride + w * 4]);
        }
        // Opaque video: alpha is 255, so premultiplied and straight are the same.
        egui::ColorImage::from_rgba_premultiplied([w, h], &rgba)
    };
    {
        let mut texture = lock(&inner.texture);
        match texture.as_mut() {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                *texture = Some(inner.ctx.load_texture(
                    "video",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
    }
    let fps = info.fps();
    let mut frame_info = lock(&inner.frame);
    frame_info.size = Some([w as u32, h as u32]);
    frame_info.frame_time = (fps.numer() > 0)
        .then(|| Duration::from_secs_f64(f64::from(fps.denom()) / f64::from(fps.numer())));
    drop(frame_info);
    inner.ctx.request_repaint();
    Ok(())
}

pub(super) fn probe(path: &Path) -> Result<MediaInfo, String> {
    use gstreamer_pbutils as pbutils;
    use gstreamer_pbutils::prelude::*;

    init().map_err(|err| format!("{err:#}"))?;
    let uri = gst::glib::filename_to_uri(path, None).map_err(|err| err.to_string())?;
    let discoverer = pbutils::Discoverer::new(gst::ClockTime::from_seconds(10))
        .map_err(|err| err.to_string())?;
    let info = discoverer
        .discover_uri(uri.as_str())
        .map_err(|err| err.to_string())?;
    let describe = |caps: Option<gst::Caps>| {
        caps.map(|caps| pbutils::pb_utils_get_codec_description(&caps).to_string())
    };
    let duration = info
        .duration()
        .map(|d| Duration::from_nanos(d.nseconds()))
        .filter(|d| !d.is_zero());
    let bitrate = std::fs::metadata(path)
        .ok()
        .zip(duration)
        .map(|(meta, d)| (meta.len() as f64 * 8.0 / d.as_secs_f64()) as u64);
    let known = |rate: u32| (rate > 0).then_some(u64::from(rate));
    let audio = info
        .audio_streams()
        .into_iter()
        .next()
        .map(|a| AudioStream {
            codec: describe(a.caps()).unwrap_or_default(),
            channels: a.channels(),
            sample_rate: a.sample_rate(),
            bitrate: known(a.bitrate()).or_else(|| known(a.max_bitrate())),
            language: a.language().map(|l| l.to_string()),
        });
    let video = info.video_streams().into_iter().next().map(|v| {
        let caps = v.caps();
        let fps = v.framerate();
        let stated = known(v.bitrate()).or_else(|| known(v.max_bitrate()));
        // The total minus the sound, when the file states no video rate.
        let estimate = bitrate
            .map(|total| total.saturating_sub(audio.as_ref().and_then(|a| a.bitrate).unwrap_or(0)));
        VideoStream {
            codec: describe(caps.clone()).unwrap_or_default(),
            width: v.width(),
            height: v.height(),
            fps: (fps.numer() > 0 && fps.denom() > 0)
                .then(|| f64::from(fps.numer()) / f64::from(fps.denom())),
            hdr: caps.as_ref().and_then(hdr_of),
            bitrate: stated.or(estimate),
            bitrate_estimated: stated.is_none() && estimate.is_some(),
        }
    });
    let container = info
        .stream_info()
        .and_then(|s| s.downcast::<pbutils::DiscovererContainerInfo>().ok())
        .and_then(|c| c.caps())
        .map(|caps| {
            container_name(&caps).unwrap_or_else(|| describe(Some(caps)).unwrap_or_default())
        });
    Ok(MediaInfo {
        container,
        duration,
        bitrate,
        video,
        audio,
    })
}

/// GStreamer calls MP4, MOV and 3GP all "Quicktime"; its variant tells them apart.
fn container_name(caps: &gst::Caps) -> Option<String> {
    let structure = caps.structure(0)?;
    if structure.name() != "video/quicktime" {
        return None;
    }
    // A QuickTime file proper (MOV) carries no variant, or "apple".
    Some(
        match structure.get::<&str>("variant").ok() {
            Some("iso" | "iso-fragmented") => "MP4",
            Some("3gpp") => "3GP",
            _ => "QuickTime (MOV)",
        }
        .to_owned(),
    )
}

/// HLG or PQ, from the stream's colorimetry.
fn hdr_of(caps: &gst::Caps) -> Option<&'static str> {
    let colorimetry = caps.structure(0)?.get::<String>("colorimetry").ok()?;
    let colorimetry = colorimetry.parse::<gst_video::VideoColorimetry>().ok()?;
    match colorimetry.transfer() {
        gst_video::VideoTransferFunction::AribStdB67 => Some("HLG"),
        gst_video::VideoTransferFunction::Smpte2084 => Some("PQ"),
        _ => None,
    }
}

pub(super) fn self_test(path: &Path) -> Result<String, String> {
    let ctx = egui::Context::default();
    let path = std::path::absolute(path).map_err(|err| err.to_string())?;
    let player = Player::start(&ctx, &path, [640, 640], 0.0, true, Audio::Silent)?;
    let wait = |done: &dyn Fn(&Status) -> bool| {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            let status = player.status();
            if let Some(err) = &status.error {
                return Err(err.clone());
            }
            if done(&status) {
                return Ok(status);
            }
            if Instant::now() > until {
                return Err(format!("timed out: {status:?}"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let first = wait(&|s| s.frame_size.is_some())?;
    let decoders = player.decoders();
    if let Some(duration) = first.duration {
        player.seek(duration.saturating_sub(Duration::from_millis(300)), false);
    }
    wait(&|s| s.ended)?;
    let release = player.stop();
    let until = Instant::now() + Duration::from_secs(10);
    while !release.done() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    let [w, h] = first.frame_size.unwrap_or_default();
    // What Details › File reads, with the same plugins.
    let media = probe(&path)?;
    let streams = [
        media.container,
        media.video.map(|v| v.codec),
        media.audio.map(|a| a.codec),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ");
    Ok(format!(
        "video ok: {w}x{h}, {:.1} s, pipeline {}, decoders {}, closed {}; {streams}",
        first.duration.unwrap_or_default().as_secs_f64(),
        first.pipeline,
        decoders.join(" "),
        release.done()
    ))
}
