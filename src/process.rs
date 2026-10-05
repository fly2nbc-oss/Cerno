//! Starting the helper programs Cerno runs (ExifTool, the video frame helper): found by an
//! absolute path only, started without a console window.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

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
