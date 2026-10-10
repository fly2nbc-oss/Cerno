//! Writing the marks of one file: what is on disk, what is wanted, one ExifTool call.

use super::*;

pub(super) struct Written {
    pub(super) rating: Rating,
    pub(super) label: Option<Label>,
}

/// Writes the rating, colour label, comment and keywords – whatever changed – in one ExifTool
/// call. `Ok(None)` when the file already matches, so the dates are not touched.
pub(super) fn write_marks(
    exiftool: &mut Option<ExifTool>,
    path: &Path,
    rating: Option<Rating>,
    label: Option<Option<Label>>,
    description: Option<&Description>,
) -> Result<Option<Written>> {
    // Where the marks live: in the file, or in its XMP sidecar (RAW, BMP, video).
    let sidecar = crate::sidecar::applies(path);
    let target = if sidecar {
        crate::sidecar::path_of(path)
    } else {
        path.to_path_buf()
    };
    let path_str = target.to_str().context("path is not valid Unicode")?;
    // Read what is there right now: skips no-op writes and tells which extra rating tags
    // (Windows Explorer's) need to be kept in sync.
    let on_disk = marks_on_disk(path, sidecar)?;
    // A new sidecar starts empty and from then on replaces the file's own marks, so the first
    // write into it carries all of them – a colour set on a RAW keeps its in-camera stars.
    // Whether anything changes at all is still judged against the file.
    let fresh = sidecar && !target.is_file();
    let (meta, rating, label, description) = if fresh {
        let changes = rating.is_some_and(|r| r != on_disk.rating.value)
            || label.is_some_and(|l| label_needs_write(on_disk.label, l))
            || description.is_some_and(|d| !description_args(d, &on_disk.description).is_empty());
        if !changes {
            return Ok(None);
        }
        let carried = description.map_or_else(|| on_disk.description.clone(), Clone::clone);
        (
            metadata::read(&[]),
            Some(rating.unwrap_or(on_disk.rating.value)),
            Some(label.unwrap_or(on_disk.label.known())),
            Some(carried),
        )
    } else {
        (on_disk, rating, label, description.cloned())
    };

    let mut args = Vec::new();
    let mut written_rating = meta.rating.value;
    let mut written_label = meta.label.known();
    if let Some(rating) = rating
        && rating != meta.rating.value
    {
        args.extend(rating_args(rating, &meta.rating));
        written_rating = rating;
    }
    if let Some(label) = label
        && label_needs_write(meta.label, label)
    {
        args.push(label_arg(label));
        written_label = label;
    }
    if let Some(description) = &description {
        args.extend(description_args(description, &meta.description));
    }
    if args.is_empty() {
        if !fresh {
            return Ok(None);
        }
        // All the file's marks are cleared: an empty sidecar says so.
        crate::sidecar::ensure(&target)?;
        return Ok(Some(Written {
            rating: written_rating,
            label: written_label,
        }));
    }
    // ExifTool first: without it an empty sidecar would be left behind, and from then on it
    // would hide the RAW's own (in-camera) marks.
    let tool = match exiftool {
        Some(tool) => tool,
        None => exiftool.insert(ExifTool::spawn()?),
    };
    if sidecar {
        crate::sidecar::ensure(&target)?;
    }

    let snapshot = filetimes::Snapshot::capture(&target).context("cannot read file times")?;
    let mut command: Vec<&str> = args.iter().map(String::as_str).collect();
    command.push(path_str);
    let output = match tool.execute(&command) {
        Ok(output) => output,
        Err(err) => {
            // The process is gone or out of sync – start a fresh one next time.
            *exiftool = None;
            return Err(err);
        }
    };

    // ExifTool's `-P` goes through Perl floats and shifts the times by a few microseconds;
    // this puts back the exact values.
    if snapshot
        .restore(&target)
        .context("cannot restore file times")?
    {
        log::debug!("restored exact file times of {}", target.display());
    }
    let updated = ["1 image files updated", "1 image files unchanged"]
        .iter()
        .any(|ok| output.stdout.contains(ok));
    if !updated {
        let message = output
            .stderr
            .lines()
            .chain(output.stdout.lines())
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("ExifTool did not update the file");
        bail!("{message}");
    }
    log::info!(
        "marks {:?} {:?} written to {}",
        written_rating,
        written_label,
        target.display()
    );
    Ok(Some(Written {
        rating: written_rating,
        label: written_label,
    }))
}

/// The marks as stored now. With a sidecar: only it (a video is never read whole). Before the
/// first write a RAW or BMP shows its own marks – a rating given in the camera – so those are
/// compared with.
pub(super) fn marks_on_disk(path: &Path, sidecar: bool) -> Result<metadata::FileMetadata> {
    let video = crate::library::format_of(path) == Some(crate::library::Format::Video);
    if sidecar && (video || crate::sidecar::path_of(path).is_file()) {
        return Ok(metadata::read_sidecar(path));
    }
    let bytes = std::fs::read(path).context("cannot read file")?;
    parsed(|| {
        if sidecar {
            metadata::read_for(path, &bytes)
        } else {
            metadata::read(&bytes)
        }
    })
}

/// A file's metadata, parsed without ending the writer thread when a parser panics on a
/// broken file: that is an error for this photo, and every later mark is still written.
pub(super) fn parsed(
    read: impl FnOnce() -> metadata::FileMetadata,
) -> Result<metadata::FileMetadata> {
    crate::decode::catch_panic(|| Ok(read()))
}
