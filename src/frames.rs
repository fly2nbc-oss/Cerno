//! One frame of a video – its still picture and its filmstrip thumbnail – taken by GStreamer
//! (feature `video`): the frame at one second, or the first one of a shorter clip, turned
//! upright (`videoflip video-direction=auto`), in RGB at the video's own size. `video` runs it
//! in a process of its own, so a decoder that crashes takes only that process down.

use std::path::Path;
use std::time::Duration;

use anyhow::Result;

/// A frame, upright, RGB8 row by row.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

/// How the frame at one second is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seek {
    /// Exactly there: decoded from the keyframe before it.
    Accurate,
    /// The keyframe at or before it – faster, the frame may be earlier.
    KeyUnit,
}

/// Takes the frame of `path`, giving up after `limit`.
pub fn grab(path: &Path, seek: Seek, limit: Duration) -> Result<Frame> {
    #[cfg(feature = "video")]
    return engine::grab(path, seek, limit);
    #[cfg(not(feature = "video"))]
    {
        let _ = (path, seek, limit);
        anyhow::bail!("this build takes no video frames (feature `video`)")
    }
}

#[cfg(feature = "video")]
mod engine {
    use std::time::Instant;

    use anyhow::{Context as _, anyhow, bail};
    use gstreamer as gst;
    use gstreamer::prelude::*;
    use gstreamer_app as gst_app;
    use gstreamer_video as gst_video;
    use gstreamer_video::prelude::*;

    use super::*;

    /// The pipeline goes down however the grab ends.
    struct Down(gst::Element);

    impl Drop for Down {
        fn drop(&mut self) {
            let _ = self.0.set_state(gst::State::Null);
        }
    }

    pub(super) fn grab(path: &Path, seek: Seek, limit: Duration) -> Result<Frame> {
        crate::playback::ready().map_err(|err| anyhow!(err))?;
        let deadline = Instant::now() + limit;
        let uri = gst::glib::filename_to_uri(path, None).context("the file name is no URI")?;
        let playbin = gst::ElementFactory::make("playbin3")
            .property("uri", uri.as_str())
            .build()?;
        // The picture only: no sound to decode, no device to open, no subtitles.
        playbin.set_property_from_str("flags", "video");
        let sink = gst::parse::bin_from_description(
            "videoflip video-direction=auto ! videoconvertscale \
             ! video/x-raw,format=RGB,pixel-aspect-ratio=1/1 \
             ! appsink name=sink sync=false max-buffers=1",
            true,
        )?;
        let appsink = sink
            .by_name("sink")
            .and_downcast::<gst_app::AppSink>()
            .context("no appsink")?;
        playbin.set_property("video-sink", &sink);
        let _down = Down(playbin.clone());
        let bus = playbin.bus().context("no bus")?;
        playbin.set_state(gst::State::Paused).map_err(|_| {
            settled(&bus, deadline)
                .err()
                .unwrap_or(anyhow!("cannot open it"))
        })?;
        settled(&bus, deadline)?;

        // At one second – unless the clip is shorter; a seek past its end finds nothing, and
        // the first frame is taken then.
        let short = playbin
            .query_duration::<gst::ClockTime>()
            .is_some_and(|d| d < gst::ClockTime::from_mseconds(1100));
        let mut moved = false;
        if !short {
            let flags = gst::SeekFlags::FLUSH
                | match seek {
                    Seek::Accurate => gst::SeekFlags::ACCURATE,
                    Seek::KeyUnit => gst::SeekFlags::KEY_UNIT | gst::SeekFlags::SNAP_BEFORE,
                };
            moved = playbin
                .seek_simple(flags, gst::ClockTime::from_seconds(1))
                .is_ok()
                && settled(&bus, deadline).is_ok();
        }
        let sample = match preroll(&appsink, deadline) {
            Some(sample) => sample,
            None if moved => {
                playbin.seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                    gst::ClockTime::ZERO,
                )?;
                settled(&bus, deadline)?;
                preroll(&appsink, deadline).context("no frame")?
            }
            None => bail!("no frame"),
        };
        frame_of(&sample)
    }

    /// Waits until the pipeline has prerolled (after the start or a seek).
    fn settled(bus: &gst::Bus, deadline: Instant) -> Result<()> {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                bail!("no frame within the time limit");
            }
            let message = bus.timed_pop_filtered(
                gst::ClockTime::from_mseconds(left.as_millis() as u64),
                &[
                    gst::MessageType::AsyncDone,
                    gst::MessageType::Error,
                    gst::MessageType::Eos,
                ],
            );
            match message.as_ref().map(|m| m.view()) {
                Some(gst::MessageView::AsyncDone(_)) => return Ok(()),
                Some(gst::MessageView::Error(err)) => bail!("{}", err.error()),
                Some(_) => bail!("the video ended before a frame"),
                None => {}
            }
        }
    }

    fn preroll(appsink: &gst_app::AppSink, deadline: Instant) -> Option<gst::Sample> {
        let left = deadline.saturating_duration_since(Instant::now());
        appsink.try_pull_preroll(gst::ClockTime::from_mseconds(left.as_millis() as u64))
    }

    /// The frame's RGB rows, without the padding the mapped frame may have.
    fn frame_of(sample: &gst::Sample) -> Result<Frame> {
        let buffer = sample.buffer().context("no buffer")?;
        let caps = sample.caps().context("no caps")?;
        let info = gst_video::VideoInfo::from_caps(caps)?;
        let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
            .map_err(|_| anyhow!("cannot read the frame"))?;
        let (w, h) = (info.width() as usize, info.height() as usize);
        let stride = frame.info().stride()[0] as usize;
        let data = frame.plane_data(0)?;
        let mut rgb = Vec::with_capacity(w * h * 3);
        for row in 0..h {
            let start = row * stride;
            rgb.extend_from_slice(data.get(start..start + w * 3).context("short frame")?);
        }
        Ok(Frame {
            width: w as u32,
            height: h as u32,
            rgb,
        })
    }
}

#[cfg(all(test, feature = "video"))]
mod tests {
    use super::*;

    #[test]
    fn a_frame_comes_from_the_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny.mp4");
        for seek in [Seek::Accurate, Seek::KeyUnit] {
            let frame = grab(&fixture, seek, Duration::from_secs(20)).expect("a frame");
            assert_eq!((frame.width, frame.height), (64, 64), "{seek:?}");
            assert_eq!(frame.rgb.len(), 64 * 64 * 3);
        }
        let missing = fixture.with_file_name("missing.mp4");
        assert!(grab(&missing, Seek::Accurate, Duration::from_secs(20)).is_err());
    }
}
