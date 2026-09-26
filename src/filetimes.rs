//! Keeps file dates untouched across metadata writes.

use std::fs::{self, File, FileTimes};
use std::io;
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    modified: SystemTime,
    /// Only restorable on Windows; Linux has no API to set the birth time.
    created: Option<SystemTime>,
}

impl Snapshot {
    pub fn capture(path: &Path) -> io::Result<Self> {
        let meta = fs::metadata(path)?;
        Ok(Self {
            modified: meta.modified()?,
            created: meta.created().ok(),
        })
    }

    /// Puts the captured times back if they changed. Returns whether anything had to be restored.
    pub fn restore(&self, path: &Path) -> io::Result<bool> {
        let now = Self::capture(path)?;
        let created_changed =
            cfg!(windows) && self.created.is_some() && now.created != self.created;
        if now.modified == self.modified && !created_changed {
            return Ok(false);
        }

        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut times = FileTimes::new().set_modified(self.modified);
        #[cfg(windows)]
        if let Some(created) = self.created {
            use std::os::windows::fs::FileTimesExt;
            times = times.set_created(created);
        }
        File::options().write(true).open(path)?.set_times(times)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn restores_modified_time_after_write() {
        let path = std::env::temp_dir().join(format!("cerno-filetimes-{}.bin", std::process::id()));
        fs::write(&path, b"original").unwrap();
        let old = SystemTime::now() - Duration::from_secs(86_400 * 30);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(old))
            .unwrap();

        let snapshot = Snapshot::capture(&path).unwrap();
        File::options()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b" changed")
            .unwrap();
        assert_ne!(fs::metadata(&path).unwrap().modified().unwrap(), old);

        assert!(snapshot.restore(&path).unwrap());
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
        assert!(
            !snapshot.restore(&path).unwrap(),
            "second restore is a no-op"
        );
        fs::remove_file(&path).unwrap();
    }
}
