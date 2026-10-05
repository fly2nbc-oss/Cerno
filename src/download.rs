//! Downloading a file whose size and SHA-256 are known before the first byte: the models and
//! ExifTool. A broken download keeps its `.part` and continues there next time; only bytes of
//! the expected size and hash are renamed into place.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use sha2::{Digest as _, Sha256};

/// A download that gets no data for this long gives up (ureq has no idle timeout).
const STALL: Duration = Duration::from_secs(60);

/// A file Cerno downloads, with where it comes from, its size and SHA-256.
pub struct RemoteFile {
    /// File name in the target folder.
    pub name: &'static str,
    /// Pinned: a commit, a release tag that is never re-used, or a versioned file name.
    pub url: &'static str,
    /// Tried once when `url` fails; it must serve the same bytes.
    pub mirror: Option<&'static str>,
    pub bytes: u64,
    /// Lower-case hex.
    pub sha256: &'static str,
}

/// Where an unfinished download of `file` waits to be continued.
pub fn part_of(dir: &Path, file: &RemoteFile) -> PathBuf {
    dir.join(format!("{}.part", file.name))
}

/// Fetches `file` into `dir` unless it is there. A `.part` left by an earlier attempt is
/// continued. Only bytes of the expected size and SHA-256 are renamed into place; wrong bytes
/// are deleted, an interrupted download keeps its `.part` for the next attempt.
pub fn download_file(
    dir: &Path,
    file: &RemoteFile,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let dest = dir.join(file.name);
    if dest.is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    let part = part_of(dir, file);
    let (received, sha256) = fetch(file, &part, progress, cancelled)?;
    if let Err(err) = verify(file, received, &sha256) {
        let _ = std::fs::remove_file(&part);
        return Err(err);
    }
    std::fs::rename(&part, &dest)?;
    Ok(())
}

/// The file is the one expected: ONNX Runtime parses a model next, a tool is unpacked and run,
/// so nothing else may get through.
fn verify(file: &RemoteFile, received: u64, sha256: &str) -> Result<()> {
    if received != file.bytes {
        bail!(
            "{}: unexpected size {received} bytes (expected {})",
            file.name,
            file.bytes
        );
    }
    if sha256 != file.sha256 {
        bail!(
            "{}: unexpected SHA-256 {sha256} (expected {})",
            file.name,
            file.sha256
        );
    }
    Ok(())
}

/// What to do with the bytes an earlier attempt left in the `.part`.
#[derive(Debug, PartialEq, Eq)]
enum Resume {
    /// The server sends exactly the rest.
    Append,
    /// The server sends the whole file.
    Restart,
}

/// An earlier attempt's bytes are kept only when the server answers the range request with
/// the rest (206 starting where the `.part` ends); a 200 is the whole file again. A 206 for
/// another range fits neither.
fn resume_plan(have: u64, status: u16, content_range: Option<&str>) -> Result<Resume> {
    if status != 206 {
        return Ok(Resume::Restart);
    }
    let start = content_range
        .and_then(|range| range.strip_prefix("bytes "))
        .and_then(|range| range.split('-').next())
        .and_then(|start| start.trim().parse::<u64>().ok());
    if have > 0 && start == Some(have) {
        Ok(Resume::Append)
    } else {
        bail!("the server sent another part ({content_range:?}, asked from byte {have})")
    }
}

/// Passes the bytes on and hashes them on the way, so the 1.7 GB are not read a second time.
struct Hashing<W> {
    out: W,
    sha256: Sha256,
}

impl<W> Hashing<W> {
    fn new(out: W) -> Self {
        Self {
            out,
            sha256: Sha256::new(),
        }
    }

    /// Feeds bytes that are in the output already (an earlier attempt's) into the hash only.
    fn absorb(&mut self, mut reader: impl Read) -> io::Result<u64> {
        let mut buffer = vec![0u8; 1 << 20];
        let mut total = 0;
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                return Ok(total);
            }
            self.sha256.update(&buffer[..n]);
            total += n as u64;
        }
    }

    /// The writer back, and the SHA-256 in lower-case hex (as the manifests list it).
    fn finish(self) -> (W, String) {
        let digest = self.sha256.finalize();
        let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        (self.out, hex)
    }
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.out.write(buf)?;
        self.sha256.update(&buf[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

/// Asks for `url` from byte `from` on (the whole file for 0). Redirects keep the `Range`
/// header (ureq drops only credentials and cookies), so GitHub's hop to its storage host
/// still asks for the rest. Every hop must be HTTPS.
///
/// Certificates are checked against the system's store, like a browser does: an antivirus
/// that scans HTTPS (Kaspersky did for huggingface.co) or a company proxy presents its own
/// root, which Mozilla's built-in list does not know. The SHA-256 check guards the bytes.
fn request(url: &str, from: u64) -> Result<ureq::http::Response<ureq::Body>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .user_agent(concat!("Cerno/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let mut request = agent.get(url);
    if from > 0 {
        request = request.header("Range", &format!("bytes={from}-"));
    }
    Ok(request.call()?)
}

/// The file's URL, else its mirror once.
pub(crate) fn open(file: &RemoteFile, from: u64) -> Result<ureq::http::Response<ureq::Body>> {
    request(file.url, from).or_else(|err| match file.mirror {
        Some(mirror) => {
            log::warn!("download from {}: {err:#} – trying the mirror", file.url);
            request(mirror, from)
        }
        None => Err(err),
    })
}

/// The bytes the `.part` holds in the end and their SHA-256.
fn fetch(
    file: &RemoteFile,
    part: &Path,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<(u64, String)> {
    let mut have = std::fs::metadata(part).map_or(0, |meta| meta.len());
    if have > file.bytes {
        std::fs::remove_file(part)?;
        have = 0;
    }
    if have == file.bytes && have > 0 {
        // Complete already: an earlier attempt ended before its check.
        let mut hashing = Hashing::new(io::sink());
        hashing.absorb(File::open(part)?)?;
        progress(have);
        return Ok((have, hashing.finish().1));
    }
    let response = match open(file, have) {
        // The `.part` does not fit what the server has: start over.
        Err(err)
            if have > 0
                && matches!(
                    err.downcast_ref::<ureq::Error>(),
                    Some(ureq::Error::StatusCode(416))
                ) =>
        {
            have = 0;
            open(file, 0)?
        }
        other => other?,
    };
    let content_range = response
        .headers()
        .get("content-range")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let resume = resume_plan(have, response.status().as_u16(), content_range.as_deref())
        .inspect_err(|_| {
            let _ = std::fs::remove_file(part);
        })?;
    let mut out = match resume {
        Resume::Append => {
            let mut out = Hashing::new(OpenOptions::new().append(true).open(part)?);
            // The bytes from before go into the hash first, read once more from disk.
            out.absorb(File::open(part)?.take(have))?;
            log::info!("download: {} continues at byte {have}", file.name);
            out
        }
        Resume::Restart => {
            have = 0;
            Hashing::new(File::create(part)?)
        }
    };
    let mut reader = response.into_body().into_reader();
    // The socket is read on a helper thread, so a connection that stalls without closing
    // ends the download after `STALL` instead of hanging it forever. The helper ends with its
    // next read once nobody listens any more.
    let (tx, rx) = mpsc::sync_channel(4);
    std::thread::Builder::new()
        .name("cerno-download-read".into())
        .spawn(move || {
            loop {
                let mut buffer = vec![0u8; 1 << 20];
                let chunk = reader.read(&mut buffer).map(|n| {
                    buffer.truncate(n);
                    buffer
                });
                let last = !matches!(&chunk, Ok(bytes) if !bytes.is_empty());
                if tx.send(chunk).is_err() || last {
                    break;
                }
            }
        })?;
    let received = receive(
        &rx,
        &mut out,
        STALL,
        file.bytes - have,
        |received| progress(have + received),
        cancelled,
    );
    // What arrived stays on disk for the next attempt, even when the connection broke – unless
    // there was more of it than the file has.
    let (part_file, sha256) = out.finish();
    part_file.sync_all()?;
    drop(part_file);
    let received = received.inspect_err(|_| {
        if std::fs::metadata(part).is_ok_and(|meta| meta.len() > file.bytes) {
            let _ = std::fs::remove_file(part);
        }
    })?;
    Ok((have + received, sha256))
}

/// Writes the chunks from `rx` to `out` until the empty one that marks the end. Gives up after
/// `stall` without data, as soon as `cancelled` says so, or when more than `limit` bytes come –
/// a host that sends on and on must not fill the disk. Returns the bytes written.
fn receive(
    rx: &mpsc::Receiver<io::Result<Vec<u8>>>,
    out: &mut impl Write,
    stall: Duration,
    limit: u64,
    mut progress: impl FnMut(u64),
    cancelled: impl Fn() -> bool,
) -> Result<u64> {
    let tick = stall.min(Duration::from_millis(500));
    let mut received = 0u64;
    let mut last_data = Instant::now();
    loop {
        if cancelled() {
            bail!("cancelled");
        }
        let chunk = match rx.recv_timeout(tick) {
            Ok(chunk) => chunk?,
            Err(RecvTimeoutError::Timeout) if last_data.elapsed() >= stall => {
                bail!("no data for {} s", stall.as_secs())
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => bail!("the download ended early"),
        };
        if chunk.is_empty() {
            return Ok(received);
        }
        if received + chunk.len() as u64 > limit {
            bail!("the server sent more than the expected {limit} bytes");
        }
        out.write_all(&chunk)
            .context("cannot write the downloaded bytes")?;
        received += chunk.len() as u64;
        last_data = Instant::now();
        progress(received);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "abc" and its SHA-256.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cerno-download-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A file no test ever asks a server for: its `.part` is complete already.
    fn local(name: &'static str, sha256: &'static str) -> RemoteFile {
        RemoteFile {
            name,
            url: "https://127.0.0.1:9/never",
            mirror: None,
            bytes: 3,
            sha256,
        }
    }

    #[test]
    fn the_download_is_hashed_on_the_way_and_checked() {
        let mut out = Hashing::new(Vec::new());
        out.write_all(b"ab").unwrap();
        out.write_all(b"c").unwrap();
        let (bytes, sha256) = out.finish();
        assert_eq!(bytes, b"abc");
        assert_eq!(sha256, ABC);

        let file = local("abc.bin", ABC);
        assert!(verify(&file, 3, ABC).is_ok());
        let other = "0".repeat(64);
        let err = verify(&file, 3, &other).unwrap_err();
        assert!(err.to_string().contains("SHA-256"), "{err}");
        let err = verify(&file, 2, ABC).unwrap_err();
        assert!(err.to_string().contains("size"), "{err}");
    }

    /// A continued download hashes the bytes from before first, then the rest: the same hash
    /// as one piece.
    #[test]
    fn a_continued_download_hashes_like_one_piece() {
        let mut out = Hashing::new(b"ab".to_vec());
        assert_eq!(out.absorb(&b"ab"[..]).unwrap(), 2);
        out.write_all(b"c").unwrap();
        let (bytes, sha256) = out.finish();
        assert_eq!((bytes.as_slice(), sha256.as_str()), (&b"abc"[..], ABC));
    }

    #[test]
    fn only_the_rest_continues_a_part() {
        let rest = Some("bytes 100-199/200");
        assert_eq!(resume_plan(100, 206, rest).unwrap(), Resume::Append);
        // The whole file again (the server ignores ranges, or there was nothing yet).
        assert_eq!(resume_plan(100, 200, None).unwrap(), Resume::Restart);
        assert_eq!(resume_plan(0, 200, None).unwrap(), Resume::Restart);
        // Another part than the one asked for fits nothing.
        assert!(resume_plan(50, 206, rest).is_err());
        assert!(resume_plan(100, 206, None).is_err());
        assert!(resume_plan(0, 206, Some("bytes 0-9/200")).is_err());
    }

    /// A `.part` an earlier attempt finished is checked and renamed without asking a server;
    /// one with wrong bytes is deleted.
    #[test]
    fn a_complete_part_is_checked_and_wrong_bytes_go() {
        let dir = temp_dir("part");
        let good = local("good.bin", ABC);
        std::fs::write(part_of(&dir, &good), b"abc").unwrap();
        let mut seen = 0;
        download_file(&dir, &good, &mut |n| seen = n, &|| false).unwrap();
        assert_eq!(std::fs::read(dir.join("good.bin")).unwrap(), b"abc");
        assert!(!part_of(&dir, &good).exists());
        assert_eq!(seen, 3);
        // There already: nothing to do.
        download_file(&dir, &good, &mut |_| {}, &|| false).unwrap();

        let bad = local("bad.bin", ABC);
        std::fs::write(part_of(&dir, &bad), b"abd").unwrap();
        let err = download_file(&dir, &bad, &mut |_| {}, &|| false).unwrap_err();
        assert!(err.to_string().contains("SHA-256"), "{err}");
        assert!(!part_of(&dir, &bad).exists() && !dir.join("bad.bin").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_stalled_download_gives_up_and_a_finished_one_counts_its_bytes() {
        let (tx, rx) = mpsc::sync_channel(4);
        tx.send(Ok(vec![1, 2, 3])).unwrap();
        tx.send(Ok(vec![4])).unwrap();
        tx.send(Ok(Vec::new())).unwrap();
        let mut out = Vec::new();
        let mut seen = Vec::new();
        let received = receive(
            &rx,
            &mut out,
            Duration::from_secs(5),
            4,
            |n| seen.push(n),
            || false,
        )
        .unwrap();
        assert_eq!((received, out, seen), (4, vec![1, 2, 3, 4], vec![3, 4]));

        // The sender stays alive but sends nothing: a stalled connection.
        let (tx, rx) = mpsc::sync_channel(4);
        tx.send(Ok(vec![9])).unwrap();
        let started = Instant::now();
        let err = receive(
            &rx,
            &mut Vec::new(),
            Duration::from_millis(60),
            10,
            |_| {},
            || false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("no data"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(tx);

        let (_tx, rx) = mpsc::sync_channel::<io::Result<Vec<u8>>>(1);
        let err = receive(
            &rx,
            &mut Vec::new(),
            Duration::from_secs(5),
            10,
            |_| {},
            || true,
        )
        .unwrap_err();
        assert!(err.to_string().contains("cancelled"));
    }

    /// A host that sends more than the file has is cut off before the extra bytes are written.
    #[test]
    fn no_more_than_the_expected_bytes_are_written() {
        let (tx, rx) = mpsc::sync_channel(4);
        tx.send(Ok(vec![1, 2])).unwrap();
        tx.send(Ok(vec![3, 4])).unwrap();
        tx.send(Ok(Vec::new())).unwrap();
        let mut out = Vec::new();
        let err = receive(&rx, &mut out, Duration::from_secs(5), 3, |_| {}, || false).unwrap_err();
        assert!(err.to_string().contains("more than"), "{err}");
        assert_eq!(out, vec![1, 2]);
    }

    /// Plain HTTP is refused before any connection – also for a redirect's target.
    #[test]
    fn plain_http_is_refused() {
        let err = request("http://127.0.0.1:9/never", 0).unwrap_err();
        assert!(
            matches!(
                err.downcast_ref::<ureq::Error>(),
                Some(ureq::Error::RequireHttpsOnly(_))
            ),
            "{err:#}"
        );
    }
}
