//! One long-lived ExifTool process (`-stay_open`), so a write costs milliseconds instead of a
//! Perl start-up.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};

use anyhow::{Context as _, Result, bail};

pub struct ExifTool {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
    counter: u32,
}

pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

impl ExifTool {
    pub fn spawn() -> Result<Self> {
        let exe = locate().context(
            "ExifTool not found – install it and put it on PATH (or set CERNO_EXIFTOOL)",
        )?;
        let mut command = Command::new(&exe);
        // `-common_args` are added to every command. The UTF-8 charset is needed for non-ASCII
        // paths on Windows; `-P` and `-overwrite_original_in_place` keep the file's dates and
        // identity (same file, not a renamed copy).
        command
            .args(["-stay_open", "True", "-@", "-", "-common_args"])
            .args([
                "-charset",
                "filename=UTF8",
                "-P",
                "-overwrite_original_in_place",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("cannot start {}", exe.display()))?;
        log::info!("ExifTool started: {}", exe.display());

        let stdin = child.stdin.take().context("ExifTool stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("ExifTool stdout")?);
        let stderr = BufReader::new(child.stderr.take().context("ExifTool stderr")?);
        Ok(Self {
            child,
            stdin,
            stdout,
            stderr,
            counter: 0,
        })
    }

    /// Runs one command. Arguments go one per line; one with a line break (a comment) goes as a
    /// C string (`#[CSTR]`), so it can't split into two.
    pub fn execute(&mut self, args: &[&str]) -> Result<Output> {
        self.counter += 1;
        let sentinel = format!("{{ready{}}}", self.counter);
        for arg in args {
            writeln!(self.stdin, "{}", arg_line(arg))?;
        }
        // `-echo4` prints the sentinel to stderr after processing, `-executeN` prints `{readyN}`
        // to stdout – that way both streams can be read to their end without guessing.
        writeln!(self.stdin, "-echo4\n{sentinel}\n-execute{}", self.counter)?;
        self.stdin.flush()?;

        let stdout = read_until_sentinel(&mut self.stdout, &sentinel)?;
        let stderr = read_until_sentinel(&mut self.stderr, &sentinel)?;
        Ok(Output { stdout, stderr })
    }
}

impl Drop for ExifTool {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "-stay_open\nFalse");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

/// One argument as a line of ExifTool's argument stream. Plain unless it holds a line break;
/// then a `#[CSTR]` line with C escapes (backslashes escaped too, only there).
fn arg_line(arg: &str) -> std::borrow::Cow<'_, str> {
    if !arg.contains(['\n', '\r']) {
        return arg.into();
    }
    let mut line = String::from("#[CSTR]");
    for c in arg.chars() {
        match c {
            '\\' => line.push_str("\\\\"),
            '\n' => line.push_str("\\n"),
            '\r' => line.push_str("\\r"),
            '\t' => line.push_str("\\t"),
            c => line.push(c),
        }
    }
    line.into()
}

fn read_until_sentinel(reader: &mut impl BufRead, sentinel: &str) -> Result<String> {
    let mut text = String::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            bail!("ExifTool exited unexpectedly: {}", text.trim());
        }
        // ExifTool may print file names in the system code page; don't fail on that.
        let decoded = String::from_utf8_lossy(&line);
        if decoded.trim_end() == sentinel {
            return Ok(text);
        }
        text.push_str(&decoded);
    }
}

/// `CERNO_EXIFTOOL`, then next to our executable (bundled), then `PATH`. Always an absolute
/// path, and `spawn` starts exactly that file – with a bare name `Command` would search on
/// its own and might start another one than the file checked here.
pub fn locate() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CERNO_EXIFTOOL")
        && let Ok(path) = std::path::absolute(path)
        && path.is_file()
    {
        return Some(path);
    }
    let name = if cfg!(windows) {
        "exiftool.exe"
    } else {
        "exiftool"
    };
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|p| p.is_file());
    bundled.or_else(|| find_in_path(&std::env::var_os("PATH")?, name))
}

/// The first `<entry>/<name>` that is a file, from the absolute entries only: an empty entry
/// (`;;`, a trailing `;`) or a relative one would be looked up in the working directory.
fn find_in_path(paths: &OsStr, name: &str) -> Option<PathBuf> {
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

    #[test]
    fn a_line_break_turns_the_argument_into_a_c_string() {
        assert_eq!(
            arg_line(r"C:\Fotos\Ä.jpg"),
            r"C:\Fotos\Ä.jpg",
            "paths stay as they are"
        );
        assert_eq!(
            arg_line("-MWG:Description=eins\nzwei\\drei"),
            r"#[CSTR]-MWG:Description=eins\nzwei\\drei"
        );
    }
}
