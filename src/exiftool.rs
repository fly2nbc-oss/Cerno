//! One long-lived ExifTool process (`-stay_open`), so a write costs milliseconds instead of a
//! Perl start-up.

use std::ffi::OsStr;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};

use anyhow::{Context as _, Result, bail};

/// ExifTool before 12.24 runs code hidden in a crafted file (CVE-2021-22204) – and Cerno hands
/// it every photo that gets a mark.
const MIN_VERSION: (u32, u32) = (12, 24);

pub const NAME: &str = if cfg!(windows) {
    "exiftool.exe"
} else {
    "exiftool"
};

pub struct ExifTool {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
    counter: u32,
    version: String,
}

pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

/// Why no ExifTool runs: the UI says it in the user's language and blocks the marks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    Missing,
    /// The version found.
    TooOld(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(f, "ExifTool not found"),
            Self::TooOld(version) => write!(
                f,
                "ExifTool {version} is too old ({}.{} or newer needed)",
                MIN_VERSION.0, MIN_VERSION.1
            ),
        }
    }
}

impl std::error::Error for ToolError {}

impl ExifTool {
    pub fn spawn() -> Result<Self> {
        let found = locate().ok_or(ToolError::Missing)?;
        let mut command = Command::new(&found.path);
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
        // The ExifTool Cerno downloaded brings its own Perl: modules from elsewhere stay out.
        if found.origin == Origin::Downloaded {
            for variable in ["PERL5LIB", "PERL5OPT", "PERLLIB"] {
                command.env_remove(variable);
            }
        }
        crate::process::hide_window(&mut command);
        let mut child = crate::process::spawn_tied(&mut command)
            .with_context(|| format!("cannot start {}", found.path.display()))?;

        let stdin = child.stdin.take().context("ExifTool stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("ExifTool stdout")?);
        let stderr = BufReader::new(child.stderr.take().context("ExifTool stderr")?);
        let mut tool = Self {
            child,
            stdin,
            stdout,
            stderr,
            counter: 0,
            version: String::new(),
        };
        let version = tool.execute(&["-ver"])?.stdout.trim().to_owned();
        if !new_enough(&version) {
            // Dropping `tool` ends the process.
            return Err(ToolError::TooOld(version).into());
        }
        log::info!("ExifTool {version} started: {}", found.path.display());
        tool.version = version;
        Ok(tool)
    }

    pub fn version(&self) -> &str {
        &self.version
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

/// `12.24` and later; anything that is no version is too old.
fn new_enough(version: &str) -> bool {
    let mut parts = version.trim().split('.');
    let number = |part: Option<&str>| part.and_then(|p| p.trim().parse::<u32>().ok());
    match (number(parts.next()), number(parts.next())) {
        (Some(major), Some(minor)) => (major, minor) >= MIN_VERSION,
        _ => false,
    }
}

/// Where an ExifTool was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `CERNO_EXIFTOOL`.
    Variable,
    /// Next to `cerno.exe`.
    BesideCerno,
    /// The one Cerno downloaded into its data folder.
    Downloaded,
    /// Installed on the system, found on `PATH`.
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub path: PathBuf,
    pub origin: Origin,
}

/// Where Cerno unpacks the ExifTool it downloads (`exiftool.exe` and `exiftool_files`).
pub fn download_dir() -> Option<PathBuf> {
    crate::paths::tools_dir()
        .ok()
        .map(|dir| dir.join("exiftool"))
}

/// `CERNO_EXIFTOOL`, then next to our executable, then the one Cerno downloaded, then `PATH`.
/// Always an absolute path, and `spawn` starts exactly that file – with a bare name `Command`
/// would search on its own and might start another one than the file checked here.
pub fn locate() -> Option<Located> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    locate_in(
        std::env::var_os("CERNO_EXIFTOOL").as_deref(),
        exe_dir.as_deref(),
        download_dir().as_deref(),
        std::env::var_os("PATH").as_deref(),
    )
}

fn locate_in(
    variable: Option<&OsStr>,
    exe_dir: Option<&Path>,
    downloaded: Option<&Path>,
    path_var: Option<&OsStr>,
) -> Option<Located> {
    let found = |path: PathBuf, origin| path.is_file().then_some(Located { path, origin });
    variable
        .filter(|v| !v.is_empty())
        .and_then(|v| std::path::absolute(v).ok())
        .and_then(|path| found(path, Origin::Variable))
        .or_else(|| found(exe_dir?.join(NAME), Origin::BesideCerno))
        .or_else(|| found(downloaded?.join(NAME), Origin::Downloaded))
        .or_else(|| {
            let path = crate::process::find_in_path(path_var?, NAME)?;
            Some(Located {
                path,
                origin: Origin::Path,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The order: the variable, beside Cerno, the downloaded one, `PATH` – each only where
    /// the file is.
    #[test]
    fn exiftool_is_looked_for_in_order() {
        let root = std::env::temp_dir().join(format!("cerno-locate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let [own, beside, downloaded, on_path] =
            ["own", "beside", "downloaded", "path"].map(|name| root.join(name));
        for dir in [&own, &beside, &downloaded, &on_path] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(NAME), b"").unwrap();
        }
        let path_var = std::env::join_paths([Path::new(""), &on_path]).unwrap();
        let variable = own.join(NAME).into_os_string();
        let origin = |variable: Option<&OsStr>, beside: Option<&Path>, downloaded| {
            locate_in(variable, beside, downloaded, Some(&path_var)).map(|l| l.origin)
        };
        let all = origin(Some(&variable), Some(&beside), Some(&downloaded));
        assert_eq!(all, Some(Origin::Variable));
        assert_eq!(
            origin(None, Some(&beside), Some(&downloaded)),
            Some(Origin::BesideCerno)
        );
        assert_eq!(
            origin(None, Some(&own.join("x")), Some(&downloaded)),
            Some(Origin::Downloaded)
        );
        assert_eq!(origin(None, None, None), Some(Origin::Path));
        // A variable pointing nowhere falls through to the next place.
        let nowhere = root.join("missing").into_os_string();
        assert_eq!(
            origin(Some(&nowhere), None, Some(&downloaded)),
            Some(Origin::Downloaded)
        );
        assert_eq!(locate_in(None, None, None, None), None);
        let found = locate_in(None, None, Some(&downloaded), None).unwrap();
        assert_eq!(found.path, downloaded.join(NAME));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn only_a_new_enough_exiftool_runs() {
        assert!(new_enough("13.59"));
        assert!(new_enough("12.24\n"));
        assert!(new_enough("12.76"));
        assert!(!new_enough("12.23"));
        assert!(!new_enough("11.88"));
        assert!(!new_enough(""));
        assert!(!new_enough("Error"));
        assert_eq!(
            ToolError::TooOld("12.10".into()).to_string(),
            "ExifTool 12.10 is too old (12.24 or newer needed)"
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
