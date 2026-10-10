//! Who is reading or rewriting which photo right now.
//!
//! The rating writer, the copy/move worker, the edit render, the loader and the analysis all
//! touch the original files from their own threads. A path is held while one of them does
//! file I/O on it, so nobody reads a half-written file or copies it mid-write. Marks the
//! writer is still debouncing count as busy as well, and every write bumps the path's
//! generation: a reader can tell afterwards that the bytes it worked on are no longer current.
//!
//! Holds only cover the I/O itself (a read, one ExifTool call, one copy), never decoding.
//! The UI thread never waits here.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, MutexGuard};

use anyhow::{Context as _, Result, bail};

/// The largest photo read whole: a huge file with a photo's extension (a disk image renamed
/// `.jpg`) would otherwise end Cerno for want of memory. A 200 MP 16-bit TIFF is 1.2 GB.
pub const MAX_FILE_BYTES: u64 = 2 << 30;

#[derive(Default)]
pub struct FileLocks {
    state: Mutex<State>,
    released: Condvar,
}

#[derive(Default)]
struct State {
    /// Held paths; `true` for a write.
    held: HashMap<PathBuf, bool>,
    /// Marks waiting in the writer's debounce.
    queued: HashSet<PathBuf>,
    /// Finished writes per path.
    generation: HashMap<PathBuf, u64>,
}

/// Releases the path on drop; a write also bumps its generation then.
pub struct Held<'a> {
    locks: &'a FileLocks,
    path: PathBuf,
    write: bool,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        let mut state = self.locks.lock();
        state.held.remove(&self.path);
        if self.write {
            *state.generation.entry(self.path.clone()).or_insert(0) += 1;
        }
        drop(state);
        self.locks.released.notify_all();
    }
}

impl FileLocks {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn hold_as(&self, path: &Path, write: bool) -> Held<'_> {
        let mut state = self.lock();
        while state.held.contains_key(path) {
            state = self.released.wait(state).unwrap_or_else(|p| p.into_inner());
        }
        state.held.insert(path.to_path_buf(), write);
        Held {
            locks: self,
            path: path.to_path_buf(),
            write,
        }
    }

    /// The whole file, read while nobody writes it – never one larger than [`MAX_FILE_BYTES`].
    pub fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let _held = self.hold(path);
        let size = std::fs::metadata(path).context("cannot read file")?.len();
        if size > MAX_FILE_BYTES {
            bail!("file too large ({} MB)", size >> 20);
        }
        std::fs::read(path).context("cannot read file")
    }

    /// Waits until nobody else reads or writes `path`, then holds it for a read or a copy.
    pub fn hold(&self, path: &Path) -> Held<'_> {
        self.hold_as(path, false)
    }

    /// Like [`Self::hold`], for a write: the path counts as busy until the guard drops, and
    /// its generation goes up then.
    pub fn hold_write(&self, path: &Path) -> Held<'_> {
        self.hold_as(path, true)
    }

    /// The writer has a mark for `path` waiting (`true`) or just wrote it (`false`).
    pub fn set_queued(&self, path: &Path, queued: bool) {
        let mut state = self.lock();
        if queued {
            state.queued.insert(path.to_path_buf());
        } else {
            state.queued.remove(path);
        }
    }

    /// A write for `path` is waiting or running.
    pub fn busy(&self, path: &Path) -> bool {
        let state = self.lock();
        state.queued.contains(path) || state.held.get(path) == Some(&true)
    }

    /// Changes after every finished write of `path`.
    pub fn generation(&self, path: &Path) -> u64 {
        self.lock().generation.get(path).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn a_write_is_busy_and_bumps_the_generation() {
        let locks = FileLocks::default();
        let path = Path::new("a.jpg");
        assert!(!locks.busy(path));
        {
            let _read = locks.hold(path);
            assert!(!locks.busy(path), "a read is not a write");
        }
        let before = locks.generation(path);
        {
            let _write = locks.hold_write(path);
            assert!(locks.busy(path));
            assert_eq!(locks.generation(path), before, "bumped only when done");
        }
        assert!(!locks.busy(path));
        assert_eq!(locks.generation(path), before + 1);
        locks.set_queued(path, true);
        assert!(locks.busy(path));
        locks.set_queued(path, false);
        assert!(!locks.busy(path));
    }

    #[test]
    fn a_second_holder_waits_for_the_first() {
        let locks = Arc::new(FileLocks::default());
        let path = PathBuf::from("b.jpg");
        let held = locks.hold_write(&path);
        let (other, other_path) = (Arc::clone(&locks), path.clone());
        let waiter = std::thread::spawn(move || {
            let _held = other.hold(&other_path);
            other.generation(&other_path)
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(!waiter.is_finished(), "must wait while the write runs");
        drop(held);
        assert_eq!(waiter.join().unwrap(), 1, "sees the finished write");
        // Another path is never blocked.
        let _a = locks.hold(Path::new("c.jpg"));
        let _b = locks.hold(Path::new("d.jpg"));
    }
}
