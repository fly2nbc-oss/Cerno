# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added

- Native viewer (egui + wgpu) for JPEG, HEIC behind the `heic` cargo feature.
- Background prefetch of the neighbouring images, decoded at monitor resolution and kept as GPU textures.
- EXIF orientation is applied to JPEGs.
- 1–5 star rating with the number keys (`0` clears). Stars are written to `xmp:Rating` via ExifTool in the background (debounced), file modification and creation dates are preserved.
