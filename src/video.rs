//! Videos: one frame stands in for the picture and the thumbnail – the frame at one second,
//! taken by GStreamer (`frames`) in helper processes (`cerno --frames`) that stay open for the
//! next frame. A decoder that crashes or hangs takes only a helper down, and the video shows
//! its placeholder. Playing is `playback`'s. Cerno never reads a video into memory: GStreamer
//! gets the path.

use std::io::{self, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::decode::DecodedImage;
use crate::frames::Seek;
use crate::sync::lock;

/// Exactly the frame at one second, like ffmpeg's up to 1.8.0: the frame at the keyframe
/// before it was faster but often visibly another one (measured 2026-10-05 on 162 phone and
/// drone videos: a mean difference of 7.6 against ffmpeg's frame, 19.2 for the keyframe).
const SEEK: Seek = Seek::Accurate;

/// The helper's own limit for one frame (a broken file, a slow network drive).
const TIMEOUT: Duration = Duration::from_secs(20);

/// Cerno waits a little longer, so the helper can say itself why there is no frame; then the
/// helper is ended.
const ANSWER: Duration = Duration::from_secs(25);

/// A helper that got no request for this long ends itself; Cerno drops it a little earlier,
/// so a request never reaches one that is just ending.
const IDLE: Duration = Duration::from_secs(60);
const KEEP_IDLE: Duration = Duration::from_secs(50);

/// Helpers kept waiting for the next frame: the loader's workers (up to four) and the
/// thumbnail thread. With three, a busy moment started and ended one per round.
const KEPT: usize = 4;

/// A frame larger than this is no answer of the helper's (a broken stream).
const MAX_FRAME_BYTES: u64 = 512 << 20;

/// The still picture: the frame fitted into the photo area, scaled like a photo on screen.
pub fn poster(path: &Path, target: [u32; 2]) -> Result<DecodedImage> {
    frame(path, target, true)
}

/// The same frame for the filmstrip and the grid: fitted into `side` × `side`.
pub fn thumbnail(path: &Path, side: u32) -> Result<DecodedImage> {
    frame(path, [side; 2], false)
}

/// What a helper is asked: the frame of `path`, fitted into `max` – with Lanczos3 for the
/// screen (`sharp`), else CatmullRom.
#[derive(Debug, Clone, PartialEq)]
struct Request {
    max: [u32; 2],
    sharp: bool,
    seek: Seek,
    path: String,
}

enum Answer {
    Frame(DecodedImage),
    Failed(String),
}

static IDLE_HELPERS: Mutex<Vec<Helper>> = Mutex::new(Vec::new());

fn frame(path: &Path, max: [u32; 2], sharp: bool) -> Result<DecodedImage> {
    // Loaded here first, so a helper finds the plugin registry up to date.
    crate::playback::ready().map_err(|err| anyhow!(err))?;
    let request = Request {
        max,
        sharp,
        seek: SEEK,
        path: path
            .to_str()
            .context("the path is not valid Unicode")?
            .to_owned(),
    };
    let idle = {
        let mut idle = lock(&IDLE_HELPERS);
        idle.retain(|helper| helper.since.elapsed() < KEEP_IDLE);
        idle.pop()
    };
    let mut helper = match idle {
        Some(helper) => helper,
        None => Helper::start()?,
    };
    // A helper that fails to answer is dropped, which ends it.
    let answer = helper.ask(&request)?;
    let mut idle = lock(&IDLE_HELPERS);
    if idle.len() < KEPT {
        idle.push(helper);
    }
    drop(idle);
    match answer {
        Answer::Frame(image) => Ok(image),
        Answer::Failed(message) => bail!("{message}"),
    }
}

/// A `cerno --frames` process and the thread that reads its answers.
struct Helper {
    child: Child,
    stdin: ChildStdin,
    answers: mpsc::Receiver<io::Result<Answer>>,
    /// Its last answer (or its start).
    since: Instant,
}

impl Helper {
    fn start() -> Result<Self> {
        let exe = std::env::current_exe().context("cannot find Cerno itself")?;
        let mut command = Command::new(exe);
        command
            .arg("--frames")
            .env("GST_REGISTRY_UPDATE", "no")
            .env("RUST_LOG", "error")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        crate::process::hide_window(&mut command);
        let mut child =
            crate::process::spawn_tied(&mut command).context("cannot start the frame helper")?;
        let stdin = child.stdin.take().context("frame helper stdin")?;
        let stdout = child.stdout.take().context("frame helper stdout")?;
        let (tx, answers) = mpsc::channel();
        std::thread::Builder::new()
            .name("cerno-frame-answers".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    let answer = read_answer(&mut reader);
                    let ended = answer.is_err();
                    if tx.send(answer).is_err() || ended {
                        break;
                    }
                }
            })
            .context("cannot read the frame helper")?;
        Ok(Self {
            child,
            stdin,
            answers,
            since: Instant::now(),
        })
    }

    fn ask(&mut self, request: &Request) -> Result<Answer> {
        self.stdin
            .write_all(&request_bytes(request))
            .and_then(|()| self.stdin.flush())
            .context("the frame helper is gone")?;
        let answer = match self.answers.recv_timeout(ANSWER) {
            Ok(Ok(answer)) => answer,
            Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => {
                let status = self.child.wait().map(|s| s.to_string()).unwrap_or_default();
                bail!("the frame helper ended ({status})");
            }
            Err(RecvTimeoutError::Timeout) => bail!("no frame within {} s", ANSWER.as_secs()),
        };
        self.since = Instant::now();
        Ok(answer)
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `cerno --frames`: the helper's side. Reads requests from stdin and answers each on stdout,
/// until stdin closes – Cerno ended – or no request came for [`IDLE`]. Returns the exit code.
pub fn serve_frames() -> i32 {
    let started = Instant::now();
    let last = Arc::new(AtomicU64::new(0));
    let busy = Arc::new(AtomicBool::new(false));
    {
        let (last, busy) = (Arc::clone(&last), Arc::clone(&busy));
        let _ = std::thread::Builder::new()
            .name("cerno-frames-idle".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let quiet = started.elapsed().as_millis() as u64 - last.load(Ordering::Relaxed);
                    if !busy.load(Ordering::Relaxed) && quiet > IDLE.as_millis() as u64 {
                        std::process::exit(0);
                    }
                }
            });
    }
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let request = match read_request(&mut input) {
            Ok(Some(request)) => request,
            Ok(None) => return 0,
            Err(err) => {
                eprintln!("{err}");
                return 1;
            }
        };
        busy.store(true, Ordering::Relaxed);
        let answer = answer_for(&request);
        busy.store(false, Ordering::Relaxed);
        last.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
        if write_answer(&mut output, &answer)
            .and_then(|()| output.flush())
            .is_err()
        {
            return 1;
        }
    }
}

fn answer_for(request: &Request) -> Answer {
    let image =
        crate::frames::grab(Path::new(&request.path), request.seek, TIMEOUT).and_then(|frame| {
            crate::decode::fit_rgb(
                frame.width,
                frame.height,
                frame.rgb,
                request.max,
                request.sharp,
            )
        });
    match image {
        Ok(image) => Answer::Frame(image),
        Err(err) => Answer::Failed(format!("{err:#}")),
    }
}

/// `[width u32][height u32][sharp u8][seek u8][length u32][path, UTF-8]`, little-endian.
fn request_bytes(request: &Request) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(14 + request.path.len());
    bytes.extend_from_slice(&request.max[0].to_le_bytes());
    bytes.extend_from_slice(&request.max[1].to_le_bytes());
    bytes.push(u8::from(request.sharp));
    bytes.push(u8::from(request.seek == Seek::KeyUnit));
    bytes.extend_from_slice(&(request.path.len() as u32).to_le_bytes());
    bytes.extend_from_slice(request.path.as_bytes());
    bytes
}

/// The next request; `None` when the input ends before one starts.
fn read_request(input: &mut impl Read) -> io::Result<Option<Request>> {
    let mut head = [0u8; 14];
    match input.read_exact(&mut head[..1]) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    }
    input.read_exact(&mut head[1..])?;
    let number =
        |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    let length = number(10) as usize;
    if length > 32 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "path too long"));
    }
    let mut path = vec![0u8; length];
    input.read_exact(&mut path)?;
    Ok(Some(Request {
        max: [number(0), number(4)],
        sharp: head[8] != 0,
        seek: if head[9] != 0 {
            Seek::KeyUnit
        } else {
            Seek::Accurate
        },
        path: String::from_utf8(path)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "path not UTF-8"))?,
    }))
}

/// `0`, then width, height, original width and height (u32) and the RGB rows – or `1`, the
/// message's length (u32) and the message.
fn write_answer(output: &mut impl Write, answer: &Answer) -> io::Result<()> {
    match answer {
        Answer::Frame(image) => {
            output.write_all(&[0])?;
            for value in [
                image.width,
                image.height,
                image.original_size[0],
                image.original_size[1],
            ] {
                output.write_all(&value.to_le_bytes())?;
            }
            output.write_all(&image.rgb)
        }
        Answer::Failed(message) => {
            output.write_all(&[1])?;
            output.write_all(&(message.len() as u32).to_le_bytes())?;
            output.write_all(message.as_bytes())
        }
    }
}

fn read_answer(input: &mut impl Read) -> io::Result<Answer> {
    let mut kind = [0u8];
    input.read_exact(&mut kind)?;
    let mut number = || -> io::Result<u32> {
        let mut bytes = [0u8; 4];
        input.read_exact(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    };
    let broken = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_owned());
    match kind[0] {
        0 => {
            let (width, height) = (number()?, number()?);
            let original_size = [number()?, number()?];
            let length = u64::from(width) * u64::from(height) * 3;
            if width == 0 || height == 0 || length > MAX_FRAME_BYTES {
                return Err(broken("no frame size"));
            }
            let mut rgb = vec![0u8; length as usize];
            input.read_exact(&mut rgb)?;
            Ok(Answer::Frame(DecodedImage {
                width,
                height,
                rgb,
                original_size,
            }))
        }
        1 => {
            let length = number()? as usize;
            if length > 64 * 1024 {
                return Err(broken("message too long"));
            }
            let mut message = vec![0u8; length];
            input.read_exact(&mut message)?;
            Ok(Answer::Failed(
                String::from_utf8_lossy(&message).into_owned(),
            ))
        }
        _ => Err(broken("unknown answer")),
    }
}

/// What a video shows without a frame (none could be taken): a dark 16:9 frame with a play
/// sign.
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

    /// Requests and answers read back as they were sent; a cut-off or absurd answer is an
    /// error, not a huge allocation.
    #[test]
    fn requests_and_answers_read_back() {
        let request = Request {
            max: [2560, 1440],
            sharp: true,
            seek: Seek::Accurate,
            path: "D:\\Fotos\\Grüße.mp4".into(),
        };
        let bytes = request_bytes(&request);
        let mut input = bytes.as_slice();
        assert_eq!(read_request(&mut input).unwrap(), Some(request));
        assert_eq!(read_request(&mut input).unwrap(), None, "the input ended");

        let image = DecodedImage {
            width: 2,
            height: 1,
            rgb: vec![1, 2, 3, 4, 5, 6],
            original_size: [1920, 1080],
        };
        let mut bytes = Vec::new();
        write_answer(&mut bytes, &Answer::Frame(image)).unwrap();
        write_answer(&mut bytes, &Answer::Failed("no frame".into())).unwrap();
        let mut input = bytes.as_slice();
        let Answer::Frame(back) = read_answer(&mut input).unwrap() else {
            panic!("a frame");
        };
        assert_eq!(
            (back.width, back.height, back.original_size, back.rgb),
            (2, 1, [1920, 1080], vec![1, 2, 3, 4, 5, 6])
        );
        let Answer::Failed(message) = read_answer(&mut input).unwrap() else {
            panic!("a failure");
        };
        assert_eq!(message, "no frame");
        assert!(read_answer(&mut input).is_err(), "the helper ended");

        let mut huge = vec![0u8];
        for value in [100_000u32, 100_000, 1, 1] {
            huge.extend_from_slice(&value.to_le_bytes());
        }
        assert!(read_answer(&mut huge.as_slice()).is_err());
    }

    /// The helper's answer for the fixture: fitted, and a missing file says so.
    #[cfg(feature = "video")]
    #[test]
    fn the_helper_answers_with_a_fitted_frame() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny.mp4");
        let request = |max, path: &Path| Request {
            max,
            sharp: true,
            seek: SEEK,
            path: path.to_str().unwrap().to_owned(),
        };
        let Answer::Frame(full) = answer_for(&request([256, 256], &fixture)) else {
            panic!("a frame");
        };
        assert_eq!(
            (full.width, full.height, full.original_size),
            (64, 64, [64, 64])
        );
        let Answer::Frame(small) = answer_for(&request([32, 20], &fixture)) else {
            panic!("a frame");
        };
        assert_eq!((small.width, small.height), (20, 20));
        let missing = fixture.with_file_name("missing.mp4");
        assert!(matches!(
            answer_for(&request([32, 32], &missing)),
            Answer::Failed(_)
        ));
    }

    #[test]
    fn the_placeholder_fits_and_has_a_play_sign() {
        let image = placeholder([640, 640]);
        assert_eq!((image.width, image.height), (640, 360));
        let centre = ((180 * 640 + 330) * 3) as usize;
        assert_eq!(image.rgb[centre], 0x70);
        assert_eq!(image.rgb[0], 0x24);
    }
}
