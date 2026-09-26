# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added

- Native viewer (egui + wgpu) for JPEG, HEIC behind the `heic` cargo feature (verified with a real HEIC sample, incl. rating round trip).
- Background prefetch of the neighbouring photos, decoded at monitor resolution and kept as GPU textures.
- EXIF orientation is applied to JPEGs.
- 1–5 star rating with the number keys (`0` clears). Stars are written to `xmp:Rating` via ExifTool in the background (debounced), file modification and creation dates are preserved.
- Zoom: `Z` / double-click toggles 100 %, mouse wheel and `+`/`-` zoom around the cursor, drag pans. Full resolution is loaded on demand as GPU tiles; zoom and position persist across photos.
- Filmstrip with thumbnails, star counts and a "probably blurry" marker (`T` toggles it).
- SQLite index in the local app data folder: thumbnails, scores and CLIP embeddings, keyed by an image-content fingerprint so renamed or re-rated photos keep their data.
- Background analysis (nearest first, paused while browsing): tile-based sharpness (shown as percentile within the folder) and aesthetics (CLIP ViT-L/14 + LAION predictor) on the GPU via DirectML, CPU fallback. The 1.2 GB model is downloaded on request from the toolbar.
- Toolbar: sort by name, rating, aesthetics or sharpness; filter by stars; hide the blurriest 20 %; analysis progress and aesthetics backend. Settings are remembered.
- Two-row info bar with capture date, size, scores, exposure (focal length, aperture, shutter speed, ISO), camera and lens.

### Fixed

- Arrow glyphs showed as boxes: the design system's UI font (Segoe UI; DejaVu/Noto on Linux) is now loaded from the system.
