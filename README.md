# Cerno

**Fast photo viewer and culling tool – instant switching, 1–5 star ratings from the keyboard, file dates untouched.**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](./LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-blue.svg)](#supported-platforms--formats)

Cerno (Latin *cerno* – "I sift, discern, see clearly") is a native Rust desktop app built on **egui + wgpu**. It decodes photos in the background at screen resolution and keeps the neighbours of the current photo ready as GPU textures, so switching feels instant. Ratings go straight into the file as `xmp:Rating`, which Lightroom, Bridge, digiKam and Windows Explorer read – without changing the file's modification or creation date.

Automatic sharpness and aesthetics scoring is the next milestone (see [Roadmap](#roadmap--known-issues)).

---

## Table of Contents

- [Features](#features)
- [Quick Start](#quick-start)
- [Usage](#usage)
- [Supported Platforms & Formats](#supported-platforms--formats)
- [Development & Build](#development--build)
- [Roadmap & Known Issues](#roadmap--known-issues)
- [License](#license)

## Features

- **Instant switching** – background workers prefetch the images around the current one (±3), decoded at monitor resolution and uploaded as GPU textures.
- **Keyboard-first rating** – `1`–`5` set stars, `0` clears; or click the stars in the info bar.
- **Safe metadata writes** – only the rating is written, in the background and debounced. File modification and creation dates stay bit-exact.
- **Windows Explorer friendly** – if a file already carries Explorer's rating tags, they are kept in sync.
- **Correct orientation** – EXIF orientation is applied (JPEG); HEIC transforms are applied by libheif.
- **Natural sort order** – `IMG_2` before `IMG_10`, umlauts next to their base letter.

## Quick Start

There are no binary releases yet – build from source (see [Development & Build](#development--build)), then:

```bash
cargo run --release -- "D:/Photos/2026-09 Trip"
```

You can also start without an argument and drop a folder or photo onto the window, or press `Ctrl+O`.

**Requirement:** [ExifTool](https://exiftool.org/) on `PATH` for writing ratings (Windows: `winget install OliverBetz.ExifTool`, Debian/Ubuntu: `apt install libimage-exiftool-perl`). Viewing works without it. `CERNO_EXIFTOOL` can point to a specific executable.

## Usage

| Key | Action |
|---|---|
| `→` `Space` `PageDown` | Next photo (hold to scroll) |
| `←` `Backspace` `PageUp` | Previous photo |
| `Home` / `End` | First / last photo |
| `1`–`5` | Set star rating |
| `0` | Remove rating |
| `F11` / `F` | Toggle fullscreen (`Esc` leaves it) |
| `I` | Toggle the info bar |
| `Ctrl+O` | Open folder |

## Supported Platforms & Formats

| | Windows | Linux |
|---|---|---|
| Rendering | wgpu (DX12 / Vulkan) | wgpu (Vulkan), X11 and Wayland |
| JPEG | ✓ | ✓ |
| HEIC / HEIF | ✓ with `--features heic` (libheif via vcpkg) | ✓ with `--features heic` (system libheif ≥ 1.17) |

## Development & Build

Prerequisites: Rust stable, a C/C++ toolchain (Windows: Visual Studio Build Tools with the C++ workload).

```bash
cargo run -- <folder>                 # debug build; dependencies are optimised, so decoding stays fast
cargo build --release
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test                            # the ExifTool round-trip test is skipped without ExifTool
```

### HEIC support

**Windows** – build libheif once with vcpkg (takes a while; the tree lands in `target/vcpkg`). [cargo-vcpkg](https://crates.io/crates/cargo-vcpkg) clones the pinned vcpkg revision; its bootstrap step currently fails, so the last two vcpkg steps run by hand:

```bash
cargo install cargo-vcpkg
cargo vcpkg build
target\vcpkg\bootstrap-vcpkg.bat -disableMetrics
target\vcpkg\vcpkg.exe install "libheif[core]:x64-windows-static-md"
cargo build --release --features heic
```

**Linux** – install the system library, then build with the feature:

```bash
sudo apt install libheif-dev
cargo build --release --features heic
```

## Roadmap & Known Issues

**Roadmap**

1. Filmstrip with cached thumbnails, 100 % zoom (tiled textures for images above 8192 px).
2. SQLite index under the app data directory.
3. Automatic scoring: tile-based sharpness and an aesthetics model via ONNX Runtime (DirectML on Windows, CPU/WebGPU on Linux) – scores stay in the database, never in the photos.
4. Installers (`.msi`, `.deb`, `.AppImage`) and an updater.

**Known issues**

- No colour management yet: embedded ICC profiles (e.g. Adobe RGB) are ignored.
- Because the modification date is preserved and XMP padding often keeps the size equal, backup/sync tools that only compare size and date (e.g. `rsync` without `-c`) may not notice a rating change.
- Truncated JPEGs are shown partially, with the missing part in grey.

## License

[Apache-2.0](./LICENSE)
