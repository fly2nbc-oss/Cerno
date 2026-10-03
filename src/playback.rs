//! Playing a video inside Cerno (feature `video`). GStreamer's `playbin3` decodes it – on the
//! GPU where it can – and its sink turns each frame upright, converts it to RGBA and scales it
//! to fit the photo area (`width=[1,W],height=[1,H]`: GStreamer keeps the aspect itself and
//! never enlarges). An `appsink` at the presentation clock puts every frame into one egui
//! texture; the sound goes to the system's output. The file goes by URI and is never read
//! whole.
//!
//! On Windows the conversion runs on D3D12 (`d3d12convert`): on the CPU a 4K 10-bit HEVC clip
//! from a phone reached only 21–26 fps (PoC 2026-10-03), on the GPU 30 fps at a quarter of a
//! core. When that pipeline fails to start, the CPU one is tried.
//!
//! While a file plays Windows refuses to rename or move it, so a player is stopped before
//! anything moves the file; [`Release`] says when it is closed. Without the feature
//! [`Player::start`] fails and nothing else changes.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use eframe::egui;

/// Where the sound goes: the system's output, or nowhere (tests, the self-test).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "video"), allow(dead_code))]
pub enum Audio {
    Device,
    Silent,
}

/// What the controls show, read once per frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Status {
    /// No frame yet: the pipeline is being built or prerolls.
    pub starting: bool,
    pub playing: bool,
    /// The end is reached; playing again starts from the beginning.
    pub ended: bool,
    pub position: Duration,
    pub duration: Option<Duration>,
    /// The size of the frames in the texture.
    pub frame_size: Option<[u32; 2]>,
    /// One frame's length, from the frame rate.
    pub frame_time: Option<Duration>,
    pub error: Option<String>,
}

/// Says when a stopped player has closed its file.
#[derive(Clone, Debug, Default)]
pub struct Release(Arc<AtomicBool>);

impl Release {
    pub fn done(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    #[cfg_attr(not(feature = "video"), allow(dead_code))]
    fn set(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Where Cerno's own GStreamer lives (the Windows package carries it beside the exe): its
/// plugins only, and a registry in Cerno's data folder – a GStreamer installed on the system
/// is left alone. Call before any thread starts (it sets environment variables).
pub fn configure_environment() {
    #[cfg(feature = "video")]
    engine::configure_environment();
}

/// Loads GStreamer in the background (the first plugin scan takes seconds), so `Enter` on a
/// video finds it ready.
pub fn warm_up() {
    #[cfg(feature = "video")]
    engine::warm_up();
}

/// `cerno --check-video <file>`: plays a second of the file without a window and sound, jumps
/// to its end and reports what decoded it. For the package tests.
pub fn self_test(path: &Path) -> Result<String, String> {
    #[cfg(feature = "video")]
    return engine::self_test(path);
    #[cfg(not(feature = "video"))]
    {
        let _ = path;
        Err("this build plays no videos (feature `video`)".to_owned())
    }
}

#[cfg(feature = "video")]
pub use engine::Player;

/// Without the feature there is no player; [`Player::start`] says so.
#[cfg(not(feature = "video"))]
pub enum Player {}

#[cfg(not(feature = "video"))]
impl Player {
    pub fn start(
        _ctx: &egui::Context,
        _path: &Path,
        _target: [u32; 2],
        _volume: f32,
        _muted: bool,
        _audio: Audio,
    ) -> Result<Self, String> {
        Err("this build plays no videos (feature `video`)".to_owned())
    }

    pub fn path(&self) -> &Path {
        match *self {}
    }

    pub fn texture(&self) -> Option<egui::TextureHandle> {
        match *self {}
    }

    pub fn status(&self) -> Status {
        match *self {}
    }

    pub fn toggle(&self) {
        match *self {}
    }

    pub fn seek(&self, _to: Duration, _accurate: bool) {
        match *self {}
    }

    pub fn step(&self, _forward: bool) {
        match *self {}
    }

    pub fn set_volume(&self, _volume: f32, _muted: bool) {
        match *self {}
    }

    pub fn set_target(&self, _target: [u32; 2]) {
        match *self {}
    }

    pub fn stop(self) -> Release {
        match self {}
    }
}

#[cfg(feature = "video")]
mod engine {
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use std::time::Instant;

    use anyhow::{Context as _, Result, anyhow};
    use gstreamer as gst;
    use gstreamer::prelude::*;
    use gstreamer_app as gst_app;
    use gstreamer_video as gst_video;
    use gstreamer_video::prelude::*;

    use super::*;

    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|p| p.into_inner())
    }

    static INIT: OnceLock<Result<(), String>> = OnceLock::new();

    /// The folder of the plugins Cerno ships, beside the exe (Windows package).
    fn bundled_plugins() -> Option<PathBuf> {
        let dir = std::env::current_exe()
            .ok()?
            .parent()?
            .join("gstreamer-1.0");
        dir.is_dir().then_some(dir)
    }

    pub(super) fn configure_environment() {
        let Some(plugins) = bundled_plugins() else {
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
    fn init() -> Result<()> {
        INIT.get_or_init(|| {
            let started = Instant::now();
            let result = gst::init().map_err(|err| format!("{err}"));
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
        /// The pipeline runs on D3D12 (else on the CPU).
        gpu: AtomicBool,
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
            let inner = Arc::new(Inner {
                ctx: ctx.clone(),
                path: path.to_owned(),
                audio,
                pipeline: Mutex::new(None),
                sink: Mutex::new(None),
                texture: Mutex::new(None),
                frame: Mutex::new(Frame::default()),
                target: Mutex::new(target),
                volume: Mutex::new((volume, muted)),
                want_playing: AtomicBool::new(true),
                ended: AtomicBool::new(false),
                error: Mutex::new(None),
                gpu: AtomicBool::new(false),
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
            let caps = sink_caps(inner.gpu.load(Ordering::Acquire), target);
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

    /// Builds the pipeline – on the GPU first where it exists – and starts it. A GPU pipeline
    /// that reports an error before its first frame makes way for the CPU one.
    fn run(inner: &Arc<Inner>) -> Result<()> {
        init()?;
        let gpu_first = cfg!(windows) && gst::ElementFactory::find("d3d12convert").is_some();
        let attempts: &[bool] = if gpu_first { &[true, false] } else { &[false] };
        let mut last_error = None;
        for &gpu in attempts {
            if inner.stop.load(Ordering::Acquire) {
                break;
            }
            match start_pipeline(inner, gpu) {
                Ok(()) => return Ok(()),
                Err(err) => {
                    log::warn!(
                        "video: {} pipeline failed: {err:#}",
                        if gpu { "D3D12" } else { "CPU" }
                    );
                    last_error = Some(err);
                }
            }
        }
        match last_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// One attempt: the pipeline is playing and has shown its first frame (or failed). It is
    /// shared from the start, so the controls see it at once and a stop takes it down.
    fn start_pipeline(inner: &Arc<Inner>, gpu: bool) -> Result<()> {
        let started = Instant::now();
        let uri =
            gst::glib::filename_to_uri(&inner.path, None).context("the file name is no URI")?;
        let playbin = gst::ElementFactory::make("playbin3")
            .property("uri", uri.as_str())
            .build()?;
        let target = *lock(&inner.target);
        let sink = gst::parse::bin_from_description(&sink_description(gpu, target), true)?;
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
        if inner.audio == Audio::Silent {
            let fake = gst::ElementFactory::make("fakesink")
                .property("sync", true)
                .build()?;
            playbin.set_property("audio-sink", &fake);
        }
        let (volume, muted) = *lock(&inner.volume);
        apply_volume(&playbin, volume, muted);
        *lock(&inner.sink) = Some(sink);
        inner.gpu.store(gpu, Ordering::Release);
        *lock(&inner.pipeline) = Some(playbin.clone());
        if let Err(err) = playbin.set_state(gst::State::Playing) {
            give_back(inner, &playbin);
            return Err(err.into());
        }

        // Wait for the first frame, an error, or the end (a file shorter than its preroll).
        let bus = playbin.bus().context("no bus")?;
        let until = Instant::now() + Duration::from_secs(20);
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
                        give_back(inner, &playbin);
                        return Err(anyhow!(
                            "{} ({})",
                            err.error(),
                            err.debug().map(|d| d.to_string()).unwrap_or_default()
                        ));
                    }
                    _ => {
                        inner.ended.store(true, Ordering::Release);
                        break;
                    }
                }
            }
        }
        log::info!(
            "video: {} first frame after {} ms ({})",
            inner.path.display(),
            started.elapsed().as_millis(),
            if gpu { "D3D12" } else { "CPU" }
        );
        Ok(())
    }

    /// A failed attempt: down again, unless a stop took it already.
    fn give_back(inner: &Inner, playbin: &gst::Element) {
        let _down = lock(&inner.teardown);
        let mut shared = lock(&inner.pipeline);
        if shared.as_ref() == Some(playbin) {
            *shared = None;
            *lock(&inner.sink) = None;
            drop(shared);
            let _ = playbin.set_state(gst::State::Null);
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
        Ok(format!(
            "video ok: {w}x{h}, {:.1} s, decoders {}, closed {}",
            first.duration.unwrap_or_default().as_secs_f64(),
            decoders.join(" "),
            release.done()
        ))
    }
}

#[cfg(all(test, feature = "video"))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    fn fixture() -> PathBuf {
        std::path::absolute(Path::new("tests/fixtures/tiny.mp4")).unwrap()
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
        let player = Player::start(&ctx, &fixture(), [320, 240], 0.5, false, Audio::Silent)
            .expect("started");
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

    #[test]
    fn the_self_test_plays_the_fixture() {
        let report = self_test(&fixture()).expect("self-test");
        assert!(report.starts_with("video ok: 64x64"), "{report}");
        assert!(report.contains("closed true"), "{report}");
    }
}
