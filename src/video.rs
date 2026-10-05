//! Videos: one frame stands in for the picture and the thumbnail – taken by ffmpeg, a
//! separate program like ExifTool (not shipped; found on `PATH`). Playing is `playback`'s.
//! Cerno never reads a video into memory: ffmpeg gets the path.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::decode::DecodedImage;

/// A frame that takes longer than this is given up (a broken file, a slow network drive).
const TIMEOUT: Duration = Duration::from_secs(20);

/// `CERNO_FFMPEG`, then next to our executable, then `PATH` (absolute entries only, like
/// ExifTool).
pub fn locate() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CERNO_FFMPEG")
        && let Ok(path) = std::path::absolute(path)
        && path.is_file()
    {
        return Some(path);
    }
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|p| p.is_file());
    bundled.or_else(|| crate::process::find_in_path(&std::env::var_os("PATH")?, name))
}

/// One frame as JPEG bytes: at one second, or – for a clip shorter than that – the first.
/// ffmpeg applies the rotation the phone recorded.
pub fn poster(path: &Path) -> Result<Vec<u8>> {
    frame(path, None)
}

/// The same frame, scaled by ffmpeg to fit into `side` × `side` – for the filmstrip.
pub fn thumbnail(path: &Path, side: u32) -> Result<Vec<u8>> {
    frame(path, Some(side))
}

fn frame(path: &Path, side: Option<u32>) -> Result<Vec<u8>> {
    let exe = locate().context("ffmpeg not found")?;
    frame_at(&exe, path, "1", side).or_else(|_| frame_at(&exe, path, "0", side))
}

fn frame_at(exe: &Path, path: &Path, seconds: &str, side: Option<u32>) -> Result<Vec<u8>> {
    let mut command = Command::new(exe);
    command
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            seconds,
            "-i",
        ])
        .arg(path)
        .args(["-frames:v", "1", "-an"]);
    // After the rotation: ffmpeg rotates before the filters it is given.
    if let Some(side) = side {
        command.arg("-vf").arg(format!(
            "scale=w={side}:h={side}:force_original_aspect_ratio=decrease"
        ));
    }
    command
        .args(["-f", "image2pipe", "-c:v", "mjpeg"])
        .args(["-q:v", "3", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::process::hide_window(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("cannot start {}", exe.display()))?;
    // Both pipes are read on their own threads, so a full one never blocks ffmpeg.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut buf);
            }
            buf
        })
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("ffmpeg took longer than {} s", TIMEOUT.as_secs());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let jpeg = stdout.join().map_err(|_| anyhow!("ffmpeg output lost"))?;
    if jpeg.is_empty() {
        let message = String::from_utf8_lossy(&stderr.join().unwrap_or_default())
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("no frame")
            .to_owned();
        bail!("ffmpeg: {message}");
    }
    Ok(jpeg)
}

/// What a video shows without ffmpeg: a dark 16:9 frame with a play sign.
pub fn placeholder(max_size: [u32; 2]) -> DecodedImage {
    let mut image = blank(max_size);
    let (w, h) = (image.width, image.height);
    let (cx, cy, r) = (w as f32 / 2.0, h as f32 / 2.0, h as f32 / 7.0);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            // A triangle pointing right, centred on the frame.
            let inside = dx >= -r * 0.6 && dx <= r && dy.abs() <= (r - dx) * 0.62;
            if inside {
                let i = ((y * w + x) * 3) as usize;
                image.rgb[i..i + 3].copy_from_slice(&[0x70, 0x70, 0x70]);
            }
        }
    }
    image
}

/// The placeholder's dark frame without the sign – the filmstrip paints its own over every
/// video.
pub fn blank(max_size: [u32; 2]) -> DecodedImage {
    let [w, h] = crate::decode::fit_within([1280, 720], max_size);
    DecodedImage {
        width: w,
        height: h,
        rgb: vec![0x24u8; (w * h * 3) as usize],
        original_size: [w, h],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placeholder_fits_and_has_a_play_sign() {
        let image = placeholder([640, 640]);
        assert_eq!((image.width, image.height), (640, 360));
        let centre = ((180 * 640 + 330) * 3) as usize;
        assert_eq!(image.rgb[centre], 0x70);
        assert_eq!(image.rgb[0], 0x24);
    }

    /// A real clip through the installed ffmpeg: a short test pattern it makes itself. Skipped
    /// without ffmpeg.
    #[test]
    fn a_frame_comes_from_ffmpeg() {
        let Some(exe) = locate() else {
            eprintln!("ffmpeg not found – skipped");
            return;
        };
        let dir = std::env::temp_dir().join(format!("cerno-video-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Half a second only: the one-second frame is missing, the first one is taken.
        let clip = dir.join("Grüße.mp4");
        let mut make = Command::new(&exe);
        make.args(["-nostdin", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=0.5:size=320x240:rate=10")
            .args(["-pix_fmt", "yuv420p"])
            .arg(&clip);
        crate::process::hide_window(&mut make);
        assert!(make.status().unwrap().success());

        let jpeg = poster(&clip).expect("a frame");
        let image = crate::decode::decode_for_display(
            &jpeg,
            crate::library::Format::Jpeg,
            1,
            [u32::MAX; 2],
        )
        .expect("a JPEG");
        assert_eq!((image.width, image.height), (320, 240));
        let small = crate::decode::decode_for_display(
            &thumbnail(&clip, 256).expect("a small frame"),
            crate::library::Format::Jpeg,
            1,
            [u32::MAX; 2],
        )
        .expect("a JPEG");
        assert_eq!((small.width, small.height), (256, 192));
        assert!(poster(&dir.join("missing.mp4")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
