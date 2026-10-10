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

/// Where the sound goes: the system's output, or nowhere (tests, the self-test). When the
/// system's output fails – no device (a remote session, nothing plugged in) – the video plays
/// on without sound and [`Status::silent`] says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "video"), allow(dead_code))]
pub enum Audio {
    Device,
    Silent,
    /// An output that fails as a missing device does (tests).
    #[cfg(test)]
    Broken,
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
    /// The sound output failed: the video plays without sound.
    pub silent: bool,
    pub error: Option<String>,
}

/// What a video file holds, for the details panel ([`probe`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaInfo {
    /// The container as GStreamer names it ("ISO MP4/M4A", "Matroska").
    pub container: Option<String>,
    pub duration: Option<Duration>,
    /// All streams together, in bits per second: the file size over the duration.
    pub bitrate: Option<u64>,
    pub video: Option<VideoStream>,
    pub audio: Option<AudioStream>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoStream {
    /// "H.264 (High Profile)", "H.265 (Main 10 Profile)".
    pub codec: String,
    pub width: u32,
    pub height: u32,
    /// `None` when the stream states no fixed rate: GStreamer's `0/1`, a variable frame rate,
    /// as phones record.
    pub fps: Option<f64>,
    /// "HLG" or "PQ" for HDR video.
    pub hdr: Option<&'static str>,
    /// Bits per second. Most files don't say: then it is the total minus the sound, and
    /// `bitrate_estimated` is set.
    pub bitrate: Option<u64>,
    pub bitrate_estimated: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioStream {
    /// "MPEG-4 AAC", "AC-3 (ATSC A/52)".
    pub codec: String,
    pub channels: u32,
    pub sample_rate: u32,
    /// Bits per second, when the file says.
    pub bitrate: Option<u64>,
    pub language: Option<String>,
}

/// Reads what `path` holds – container, duration, the first video and sound stream – without
/// playing it (GStreamer's discoverer; a few hundred milliseconds, so off the UI thread).
pub fn probe(path: &Path) -> Result<MediaInfo, String> {
    #[cfg(feature = "video")]
    return engine::probe(path);
    #[cfg(not(feature = "video"))]
    {
        let _ = path;
        Err("this build plays no videos (feature `video`)".to_owned())
    }
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

/// GStreamer loaded – waiting for a load in progress – before a video frame is taken
/// (`frames`, `video`).
pub fn ready() -> Result<(), String> {
    #[cfg(feature = "video")]
    return engine::init().map_err(|err| format!("{err:#}"));
    #[cfg(not(feature = "video"))]
    Err("this build plays no videos (feature `video`)".to_owned())
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

#[cfg(not(feature = "video"))]
mod stub;
#[cfg(not(feature = "video"))]
pub use stub::Player;

#[cfg(feature = "video")]
mod engine;

#[cfg(all(test, feature = "video"))]
mod tests;
