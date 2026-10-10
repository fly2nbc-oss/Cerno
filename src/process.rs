//! Starting the helper programs Cerno runs (ExifTool, the video frame helper): found by an
//! absolute path only, started without a console window, ended with Cerno. And what happens
//! when one of Cerno's own threads can't start.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Child, Command};

/// A thread Cerno can't do without did not start: the system is out of threads or memory.
/// Nothing sensible goes on from there, so this ends Cerno, with the reason in `crash.log`.
#[allow(clippy::panic)]
pub fn no_thread<T>(err: std::io::Error) -> T {
    panic!("cannot start a thread: {err}")
}

/// Starts a helper that ends with Cerno – also when Cerno is killed or crashes, which can't
/// end it itself. ExifTool's `-stay_open` waits for more commands forever otherwise: a killed
/// Cerno left one ExifTool per start behind (seen 2026-10-10). Windows: a job object that
/// kills its processes when Cerno's handle to it closes; Linux: the parent-death signal.
/// Never for a program the user works in (Edit elsewhere): that one outlives Cerno.
pub fn spawn_tied(command: &mut Command) -> std::io::Result<Child> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: only an async-signal-safe call between fork and exec.
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }
    }
    let child = command.spawn()?;
    #[cfg(windows)]
    job::assign(&child);
    Ok(child)
}

#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    /// The job's handle as a number (a `HANDLE` is not `Sync`); never closed – Windows closes
    /// it when Cerno ends, and that ends the helpers.
    static JOB: OnceLock<Option<isize>> = OnceLock::new();

    fn create() -> Option<isize> {
        // SAFETY: a new anonymous job; the struct is plain data of the documented size.
        unsafe {
            let job = CreateJobObjectW(None, None).ok()?;
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(job.0 as isize)
        }
    }

    pub fn assign(child: &std::process::Child) {
        let Some(job) = *JOB.get_or_init(create) else {
            log::warn!("no job object: a helper may outlive Cerno");
            return;
        };
        let process = HANDLE(child.as_raw_handle());
        // SAFETY: both handles are valid; the child's stays open while `child` lives.
        if let Err(err) = unsafe { AssignProcessToJobObject(HANDLE(job as *mut _), process) } {
            log::warn!("helper not tied to Cerno: {err}");
        }
    }
}

/// No console window flashes up for a helper started from the GUI.
pub fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// The first `<entry>/<name>` that is a file, from the absolute entries only: an empty entry
/// (`;;`, a trailing `;`) or a relative one would be looked up in the working directory.
pub fn find_in_path(paths: &OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// `cargo test` runs in the crate root, so relative entries would find files there.
    #[test]
    fn only_absolute_path_entries_count() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fixtures = root.join("tests").join("fixtures");
        let join = |dirs: &[&Path]| std::env::join_paths(dirs).unwrap();
        assert!(Path::new("Cargo.toml").is_file(), "runs in the crate root");

        let empty_and_relative = join(&[Path::new(""), Path::new("tests/fixtures")]);
        assert_eq!(find_in_path(&empty_and_relative, "Cargo.toml"), None);
        assert_eq!(find_in_path(&empty_and_relative, "tiny.jpg"), None);

        let absolute = join(&[Path::new(""), &fixtures]);
        assert_eq!(
            find_in_path(&absolute, "tiny.jpg"),
            Some(fixtures.join("tiny.jpg"))
        );
    }
}
