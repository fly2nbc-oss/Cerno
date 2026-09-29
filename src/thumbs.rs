//! Filmstrip thumbnails as GPU textures.
//!
//! Three sources: the display loader (downscaled from what it just decoded – instant for the
//! neighbourhood), the analysis pass (every image, also stored in the database), and the
//! database for folders analysed before (loaded here on request).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow};
use eframe::egui::{self, ColorImage, TextureFilter, TextureHandle, TextureOptions};
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use crate::db::{Db, FileStamp};
use crate::decode;

/// Longest side of a thumbnail in pixels.
pub const THUMB_SIZE: u32 = 256;
/// Textures kept at least; the filmstrip shows a few dozen. The analysis adds one for every
/// photo of the folder, so the least recently drawn ones go once there are more. The grid
/// raises the limit to what it shows (`Thumbs::set_visible`).
const MAX_TEXTURES: usize = 400;
/// Room kept above the visible cells, so scrolling a little doesn't reload.
const VISIBLE_RESERVE: usize = 100;
/// A request not repeated for this long is dropped: its cell has scrolled out of view.
const STALE: Duration = Duration::from_secs(1);

pub struct Thumbs {
    inner: Arc<Inner>,
    worker: Option<JoinHandle<()>>,
}

struct Inner {
    ctx: egui::Context,
    db: Arc<Db>,
    textures: Mutex<Textures>,
    queue: Mutex<Queue>,
    wake: Condvar,
    shutdown: AtomicBool,
}

/// Textures with the time they were last asked for (a counter, not a clock).
struct Textures {
    map: HashMap<PathBuf, (TextureHandle, u64)>,
    tick: u64,
    /// How many are kept; a quarter more may pile up before the oldest go in one batch, which
    /// keeps the sort off the per-photo path.
    capacity: usize,
}

impl Default for Textures {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            tick: 0,
            capacity: MAX_TEXTURES,
        }
    }
}

impl Textures {
    fn get(&mut self, path: &Path) -> Option<TextureHandle> {
        self.tick += 1;
        let tick = self.tick;
        self.map.get_mut(path).map(|(texture, used)| {
            *used = tick;
            texture.clone()
        })
    }

    fn insert(&mut self, path: PathBuf, texture: TextureHandle) {
        self.tick += 1;
        self.map.insert(path, (texture, self.tick));
        if self.map.len() > self.capacity + self.capacity / 4 {
            let mut ticks: Vec<u64> = self.map.values().map(|(_, used)| *used).collect();
            let cut = ticks.len() - self.capacity;
            let (_, oldest_kept, _) = ticks.select_nth_unstable(cut);
            let oldest_kept = *oldest_kept;
            self.map.retain(|_, (_, used)| *used >= oldest_kept);
        }
    }
}

#[derive(Default)]
struct Queue {
    /// Waiting for a database load, with when each was last asked for: the newest goes first,
    /// so the cells on screen fill before the ones scrolled past.
    wanted: HashMap<PathBuf, Instant>,
    /// Not in the database (yet); the analysis pass will insert them.
    misses: HashSet<PathBuf>,
    /// Bumped when a photo is edited, so a database load that started earlier is dropped.
    fresh: HashMap<PathBuf, u64>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Thumbs {
    pub fn new(ctx: egui::Context, db: Arc<Db>) -> Self {
        let inner = Arc::new(Inner {
            ctx,
            db,
            textures: Mutex::default(),
            queue: Mutex::default(),
            wake: Condvar::new(),
            shutdown: AtomicBool::new(false),
        });
        let worker_inner = Arc::clone(&inner);
        let worker = std::thread::Builder::new()
            .name("cerno-thumbs".into())
            .spawn(move || worker(&worker_inner))
            .expect("failed to spawn thumbnail worker");
        Self {
            inner,
            worker: Some(worker),
        }
    }

    /// The texture, or `None` after queueing a database lookup.
    pub fn get_or_request(&self, path: &Path) -> Option<TextureHandle> {
        if let Some(texture) = lock(&self.inner.textures).get(path) {
            return Some(texture);
        }
        let mut queue = lock(&self.inner.queue);
        if !queue.misses.contains(path)
            && queue
                .wanted
                .insert(path.to_path_buf(), Instant::now())
                .is_none()
        {
            self.inner.wake.notify_one();
        }
        None
    }

    /// The grid shows this many cells: keep at least that many textures (with some reserve),
    /// or they would be dropped and reloaded every frame. `0` when it closes.
    pub fn set_visible(&self, cells: usize) {
        lock(&self.inner.textures).capacity = (cells + VISIBLE_RESERVE).max(MAX_TEXTURES);
    }

    pub fn contains(&self, path: &Path) -> bool {
        lock(&self.inner.textures).map.contains_key(path)
    }

    /// Adds a thumbnail from RGB8 pixels of at most `THUMB_SIZE`.
    pub fn insert(&self, path: &Path, width: u32, height: u32, rgb: &[u8]) {
        self.inner.insert(path, width, height, rgb);
    }

    /// Forgets the texture. A database load already running for this path is ignored, so the
    /// filmstrip doesn't flash the pre-edit thumbnail.
    pub fn invalidate(&self, path: &Path) {
        lock(&self.inner.textures).map.remove(path);
        let mut queue = lock(&self.inner.queue);
        *queue.fresh.entry(path.to_path_buf()).or_insert(0) += 1;
        queue.misses.remove(path);
    }

    pub fn clear(&self) {
        lock(&self.inner.textures).map.clear();
        let mut queue = lock(&self.inner.queue);
        queue.wanted.clear();
        queue.misses.clear();
    }
}

impl Drop for Thumbs {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::Relaxed);
        self.inner.wake.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Inner {
    fn insert(&self, path: &Path, width: u32, height: u32, rgb: &[u8]) {
        let image = ColorImage::from_rgb([width as usize, height as usize], rgb);
        let options = TextureOptions {
            mipmap_mode: Some(TextureFilter::Linear),
            ..TextureOptions::LINEAR
        };
        let texture = self
            .ctx
            .load_texture(format!("thumb:{}", path.display()), image, options);
        lock(&self.textures).insert(path.to_path_buf(), texture);
        lock(&self.queue).misses.remove(path);
        self.ctx.request_repaint();
    }
}

fn worker(inner: &Inner) {
    loop {
        let (path, token) = {
            let mut queue = lock(&inner.queue);
            loop {
                if inner.shutdown.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(path) = next_wanted(&mut queue.wanted, Instant::now()) {
                    let token = queue.fresh.get(&path).copied().unwrap_or(0);
                    break (path, token);
                }
                queue = inner.wake.wait(queue).unwrap_or_else(|p| p.into_inner());
            }
        };
        if lock(&inner.textures).map.contains_key(&path) {
            continue;
        }
        let loaded = load_from_db(&inner.db, &path);
        if lock(&inner.queue).fresh.get(&path).copied().unwrap_or(0) != token {
            continue;
        }
        match loaded {
            Ok(Some((w, h, rgb))) => inner.insert(&path, w, h, &rgb),
            Ok(None) => {
                lock(&inner.queue).misses.insert(path);
            }
            Err(err) => {
                log::debug!("thumbnail for {}: {err:#}", path.display());
                lock(&inner.queue).misses.insert(path);
            }
        }
    }
}

/// The most recently asked-for path; requests that went stale are dropped first.
fn next_wanted(wanted: &mut HashMap<PathBuf, Instant>, now: Instant) -> Option<PathBuf> {
    wanted.retain(|_, asked| now.saturating_duration_since(*asked) < STALE);
    let newest = wanted
        .iter()
        .max_by_key(|(_, asked)| **asked)
        .map(|(path, _)| path.clone())?;
    wanted.remove(&newest);
    Some(newest)
}

fn load_from_db(db: &Db, path: &Path) -> Result<Option<(u32, u32, Vec<u8>)>> {
    let stamp = FileStamp::of(path)?;
    let Some(record) = db.lookup(&path.to_string_lossy(), stamp)? else {
        return Ok(None);
    };
    if !record.image.has_thumbnail {
        return Ok(None);
    }
    let Some(jpeg) = db.thumbnail(record.fingerprint)? else {
        return Ok(None);
    };
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&jpeg), options);
    let rgb = decoder
        .decode()
        .map_err(|e| anyhow!("thumbnail decoding failed: {e:?}"))?;
    let info = decoder.info().context("thumbnail header missing")?;
    Ok(Some((u32::from(info.width), u32::from(info.height), rgb)))
}

/// Scales RGB8 down to thumbnail size.
pub fn downscale(rgb: &[u8], width: u32, height: u32) -> Result<(u32, u32, Vec<u8>)> {
    let [w, h] = decode::fit_within([width, height], [THUMB_SIZE, THUMB_SIZE]);
    if (w, h) == (width, height) {
        return Ok((w, h, rgb.to_vec()));
    }
    Ok((w, h, decode::resize_rgb(rgb.to_vec(), width, height, w, h)?))
}

pub fn encode_jpeg(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>> {
    let mut jpeg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpeg, 82).encode(
        rgb,
        u16::try_from(width)?,
        u16::try_from(height)?,
        jpeg_encoder::ColorType::Rgb,
    )?;
    Ok(jpeg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_round_trip_keeps_size() {
        let rgb: Vec<u8> = (0..64 * 40)
            .flat_map(|i| [(i % 255) as u8, 90, 200])
            .collect();
        let jpeg = encode_jpeg(64, 40, &rgb).unwrap();
        let db = Db::open_in_memory().unwrap();
        db.put_thumbnail(1, &jpeg).unwrap();
        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&jpeg), options);
        assert_eq!(decoder.decode().unwrap().len(), 64 * 40 * 3);
    }

    #[test]
    fn textures_stay_bounded_and_keep_the_ones_in_use() {
        let ctx = egui::Context::default();
        let texture = |i: usize| {
            ctx.load_texture(
                format!("t{i}"),
                ColorImage::from_rgb([1, 1], &[0, 0, 0]),
                TextureOptions::LINEAR,
            )
        };
        let mut textures = Textures::default();
        let visible = PathBuf::from("visible.jpg");
        textures.insert(visible.clone(), texture(0));
        for i in 1..=2_000 {
            textures.insert(PathBuf::from(format!("{i}.jpg")), texture(i));
            // The filmstrip asks for the photo on screen every frame.
            assert!(textures.get(&visible).is_some());
            assert!(textures.map.len() <= MAX_TEXTURES + MAX_TEXTURES / 4);
        }
        assert!(textures.map.contains_key(Path::new("2000.jpg")));
        assert!(!textures.map.contains_key(Path::new("1.jpg")));
    }

    /// The grid keeps as many textures as it shows.
    #[test]
    fn a_larger_capacity_keeps_more() {
        let ctx = egui::Context::default();
        let mut textures = Textures {
            capacity: 900,
            ..Textures::default()
        };
        for i in 0..1_000 {
            let texture = ctx.load_texture(
                format!("t{i}"),
                ColorImage::from_rgb([1, 1], &[0, 0, 0]),
                TextureOptions::LINEAR,
            );
            textures.insert(PathBuf::from(format!("{i}.jpg")), texture);
        }
        assert_eq!(textures.map.len(), 1_000, "below capacity plus a quarter");
    }

    /// Scrolling asks for new cells: they come first, and cells no longer on screen are dropped.
    #[test]
    fn newest_requests_first_stale_ones_dropped() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut wanted = HashMap::from([
            (PathBuf::from("old.jpg"), at(0)),
            (PathBuf::from("a.jpg"), at(900)),
            (PathBuf::from("b.jpg"), at(1_100)),
        ]);
        let now = at(1_200);
        assert_eq!(next_wanted(&mut wanted, now), Some(PathBuf::from("b.jpg")));
        assert!(
            !wanted.contains_key(Path::new("old.jpg")),
            "asked for 1.2 s ago"
        );
        assert_eq!(next_wanted(&mut wanted, now), Some(PathBuf::from("a.jpg")));
        assert_eq!(next_wanted(&mut wanted, now), None);
    }

    #[test]
    fn downscale_fits_thumb_size() {
        let rgb = vec![128u8; 1000 * 500 * 3];
        let (w, h, out) = downscale(&rgb, 1000, 500).unwrap();
        assert_eq!((w, h), (256, 128));
        assert_eq!(out.len(), 256 * 128 * 3);
    }
}
