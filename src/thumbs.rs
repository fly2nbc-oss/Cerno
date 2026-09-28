//! Filmstrip thumbnails as GPU textures.
//!
//! Three sources: the display loader (downscaled from what it just decoded – instant for the
//! neighbourhood), the analysis pass (every image, also stored in the database), and the
//! database for folders analysed before (loaded here on request).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

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
/// Textures kept at most; the filmstrip shows a few dozen.
const MAX_TEXTURES: usize = 400;

pub struct Thumbs {
    inner: Arc<Inner>,
    worker: Option<JoinHandle<()>>,
}

struct Inner {
    ctx: egui::Context,
    db: Arc<Db>,
    textures: Mutex<HashMap<PathBuf, TextureHandle>>,
    queue: Mutex<Queue>,
    wake: Condvar,
    shutdown: AtomicBool,
}

#[derive(Default)]
struct Queue {
    pending: VecDeque<PathBuf>,
    queued: HashSet<PathBuf>,
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
            return Some(texture.clone());
        }
        let mut queue = lock(&self.inner.queue);
        if !queue.misses.contains(path) && queue.queued.insert(path.to_path_buf()) {
            queue.pending.push_back(path.to_path_buf());
            self.inner.wake.notify_one();
        }
        None
    }

    pub fn contains(&self, path: &Path) -> bool {
        lock(&self.inner.textures).contains_key(path)
    }

    /// Adds a thumbnail from RGB8 pixels of at most `THUMB_SIZE`.
    pub fn insert(&self, path: &Path, width: u32, height: u32, rgb: &[u8]) {
        self.inner.insert(path, width, height, rgb);
    }

    /// Forgets the texture. A database load already running for this path is ignored, so the
    /// filmstrip doesn't flash the pre-edit thumbnail.
    pub fn invalidate(&self, path: &Path) {
        lock(&self.inner.textures).remove(path);
        let mut queue = lock(&self.inner.queue);
        *queue.fresh.entry(path.to_path_buf()).or_insert(0) += 1;
        queue.misses.remove(path);
    }

    /// Drops textures the filmstrip no longer needs.
    pub fn retain(&self, keep: impl Fn(&Path) -> bool) {
        let mut textures = lock(&self.inner.textures);
        if textures.len() > MAX_TEXTURES {
            textures.retain(|path, _| keep(path));
        }
    }

    pub fn clear(&self) {
        lock(&self.inner.textures).clear();
        let mut queue = lock(&self.inner.queue);
        queue.pending.clear();
        queue.queued.clear();
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
                if let Some(path) = queue.pending.pop_front() {
                    queue.queued.remove(&path);
                    let token = queue.fresh.get(&path).copied().unwrap_or(0);
                    break (path, token);
                }
                queue = inner.wake.wait(queue).unwrap_or_else(|p| p.into_inner());
            }
        };
        if lock(&inner.textures).contains_key(&path) {
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
    fn downscale_fits_thumb_size() {
        let rgb = vec![128u8; 1000 * 500 * 3];
        let (w, h, out) = downscale(&rgb, 1000, 500).unwrap();
        assert_eq!((w, h), (256, 128));
        assert_eq!(out.len(), 256 * 128 * 3);
    }
}
