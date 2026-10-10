//! Turns, pixel edits and Ctrl+Z restores: the file changes, its dates stay.

use super::*;

/// `Ctrl+Z`: writes the first original (`.originals`) back into the file – in place, so the
/// file keeps its identity and dates – then the rating and colour label the file has now, so
/// marks set after the edit stay. The original stays where it is.
pub(super) fn restore_original(
    exiftool: &mut Option<ExifTool>,
    path: &Path,
    db: &Db,
) -> Result<()> {
    let key = path.to_string_lossy();
    let copy = crate::originals::original(db, path)?.context("no original kept for this photo")?;
    let original = std::fs::read(&copy).context("cannot read the kept original")?;
    let bytes = std::fs::read(path).context("cannot read file")?;
    let now = parsed(|| metadata::read(&bytes))?;
    let label = match now.label {
        LabelInfo::Known(label) => Some(Some(label)),
        LabelInfo::None => Some(None),
        // Text Cerno does not know is left as the original had it.
        LabelInfo::Other => None,
    };
    let snapshot = filetimes::Snapshot::capture(path).context("cannot read file times")?;
    write_in_place(path, &original)?;
    snapshot
        .restore(path)
        .context("cannot restore file times")?;
    write_marks(
        exiftool,
        path,
        Some(now.rating.value),
        label,
        Some(&now.description),
    )?;
    db.forget_file(&key).context("cannot drop the index row")?;
    log::info!("original restored: {}", path.display());
    Ok(())
}

pub(super) fn apply_quarter_turn(
    exiftool: &mut Option<ExifTool>,
    path: &Path,
    clockwise: bool,
    db: &Db,
) -> Result<()> {
    let path_str = path.to_str().context("path is not valid Unicode")?;
    let bytes = std::fs::read(path).context("cannot read file")?;
    let current = parsed(|| metadata::read(&bytes))?.orientation;
    drop(bytes);
    let next = crate::edit::rotate_orientation(current, if clockwise { 1 } else { -1 });
    if next == current {
        return Ok(());
    }
    let snapshot = filetimes::Snapshot::capture(path).context("cannot read file times")?;
    run_exiftool(
        exiftool,
        &[format!("-Orientation#={next}"), path_str.to_owned()],
    )?;
    if snapshot
        .restore(path)
        .context("cannot restore file times")?
    {
        log::debug!("restored exact file times of {}", path.display());
    }
    db.forget_file(&path.to_string_lossy())
        .context("cannot drop the index row")?;
    log::info!("orientation {current} → {next} on {}", path.display());
    Ok(())
}

pub(super) fn apply_pixels(
    exiftool: &mut Option<ExifTool>,
    path: &Path,
    jpeg: &[u8],
    db: &Db,
) -> Result<()> {
    let path_str = path.to_str().context("path is not valid Unicode")?;
    let snapshot = filetimes::Snapshot::capture(path).context("cannot read file times")?;
    let temp = TempJpeg::write(jpeg)?;
    let temp_str = temp
        .path
        .to_str()
        .context("temp path is not valid Unicode")?
        .to_owned();
    run_exiftool(
        exiftool,
        &[
            "-TagsFromFile".to_owned(),
            path_str.to_owned(),
            "-all:all".to_owned(),
            "-unsafe".to_owned(),
            // Not part of `-all:all`: the pixels stay in the file's colour space
            // (`decode::decode_for_edit`), so its profile must come along.
            "-ICC_Profile".to_owned(),
            "-Orientation#=1".to_owned(),
            "-ThumbnailImage=".to_owned(),
            "-PreviewImage=".to_owned(),
            "-MPF:all=".to_owned(),
            "-IFD1:all=".to_owned(),
            "-EXIF:ImageWidth=".to_owned(),
            "-EXIF:ImageHeight=".to_owned(),
            "-EXIF:ExifImageWidth=".to_owned(),
            "-EXIF:ExifImageHeight=".to_owned(),
            temp_str,
        ],
    )?;
    let prepared = std::fs::read(&temp.path).context("cannot read prepared JPEG")?;
    write_in_place(path, &prepared)?;
    if snapshot
        .restore(path)
        .context("cannot restore file times")?
    {
        log::debug!("restored exact file times of {}", path.display());
    }
    db.forget_file(&path.to_string_lossy())
        .context("cannot drop the index row")?;
    log::info!("pixels rewritten in {}", path.display());
    Ok(())
}

pub(super) fn run_exiftool(exiftool: &mut Option<ExifTool>, args: &[String]) -> Result<()> {
    if args.iter().any(|arg| arg.contains(['\n', '\r'])) {
        bail!("argument contains a line break");
    }
    let tool = match exiftool {
        Some(tool) => tool,
        None => exiftool.insert(ExifTool::spawn()?),
    };
    let command: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = match tool.execute(&command) {
        Ok(output) => output,
        Err(err) => {
            *exiftool = None;
            return Err(err);
        }
    };
    let updated = ["1 image files updated", "1 image files unchanged"]
        .iter()
        .any(|ok| output.stdout.contains(ok));
    if !updated {
        let message = output
            .stderr
            .lines()
            .chain(output.stdout.lines())
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("ExifTool did not update the file");
        bail!("{message}");
    }
    Ok(())
}

pub(super) fn write_in_place(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .context("cannot open file")?;
    file.write_all(bytes).context("cannot write file")?;
    file.set_len(bytes.len() as u64)
        .context("cannot resize file")?;
    Ok(())
}

/// Removed on drop, including when the metadata copy fails.
pub(super) struct TempJpeg {
    path: PathBuf,
}

impl TempJpeg {
    /// A new file with `bytes` in the temp folder. `create_new` fails when the name is taken
    /// instead of following a link another user planted there (a shared `/tmp` on Linux).
    fn write(bytes: &[u8]) -> Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("cerno-edit-{}-{nanos}.jpg", std::process::id()));
        Self::write_at(path, bytes)
    }

    pub(super) fn write_at(path: PathBuf, bytes: &[u8]) -> Result<Self> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .context("cannot create temporary JPEG")?;
        // Ours from here on: removed on drop, also when the write fails.
        let temp = Self { path };
        file.write_all(bytes)
            .context("cannot write temporary JPEG")?;
        Ok(temp)
    }
}

impl Drop for TempJpeg {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
