# Security

## Supported versions

| Version | Supported |
|---|---|
| The latest release | yes |
| Older releases | no – please update first |

## Reporting a vulnerability

Report it privately through GitHub: [**Report a vulnerability**](https://github.com/fly2nbc-oss/Cerno/security/advisories/new) – never as a public issue. You get a first answer within 14 days.

Say which version and system you use and how to trigger the problem. Please don't attach private photos; a generated file that shows the problem is best.

## What Cerno does

### Network connections

Cerno has no telemetry and sends no crash reports (`crash.log` stays in its data folder). It connects only here, always over HTTPS, with the system's certificate store:

- **Update check** – only after you agree at the first start (*Settings ▸ Check for updates*). At most once a day one `GET` to `https://github.com/fly2nbc-oss/Cerno/releases/latest` without following the redirect; only its `Location` header (the newest tag) is read. The User-Agent is just `Cerno`. Nothing is downloaded or installed.
- **Downloads you start** – the aesthetics models from Hugging Face (URLs pinned to a commit) and from this repository's release `models-1`, and on Windows ExifTool from SourceForge (a pinned version). Each file has a fixed size and SHA-256 and is used only after both match; an interrupted download resumes. ExifTool's archive is unpacked with limits on entries, names and sizes.
- **Links you click** – *About Cerno* opens a GitHub issue form with Cerno's version and the system filled in (no paths, no file names); Details › File opens a photo's GPS position in Google Maps or OpenStreetMap. Both open in your browser.

### What Cerno writes

- **Into a photo:** only the rating, colour label, comment and keywords, through [ExifTool](https://exiftool.org/) – for RAW, BMP and video into an `.xmp` file beside it. The file's modified and created dates are put back after every write. The write copies the new bytes back into the same file, so a crash at that moment can damage that one file. Backup tools that compare only size and date may miss a changed rating.
- **Straighten, crop and turns** rewrite a JPEG; before the first edit the original is copied into the hidden `.originals` folder beside it, and without that copy the edit does not happen.
- **Cerno's own data** – the index (scores, thumbnails, history of your ratings), settings, `crash.log`, the models and ExifTool – lives in `%LOCALAPPDATA%\Cerno\data` on Windows and `~/.local/share/cerno` on Linux, never inside your photos.

### What Cerno moves or deletes

- **Deleting** a photo moves it into `.originals` beside it, after a 5-second countdown. Cerno never deletes a photo and never empties `.originals`; do that yourself once you are sure.
- **Copy and move** go only to a folder you choose. *Delete models* removes only the downloaded model files.

### Input files

Photos and videos are treated as untrusted input. Image sizes are checked before decoding (at most 200 megapixels), and a decoder that fails or panics affects only that photo. HEIC is decoded by libheif and libde265 (C code; bundled on Windows, the distribution's on Linux), videos by GStreamer – still frames in separate helper processes, playback inside Cerno. ExifTool runs as a separate process.
