# Cerno

**Fast photo viewer and culling tool – instant switching, 1–5 star ratings from the keyboard, automatic sharpness and aesthetics scores, file dates untouched.**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](./LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-blue.svg)](#supported-platforms--formats)

Cerno (Latin *cerno* – "I sift, discern, see clearly") is a native Rust desktop app built on **egui + wgpu**. It decodes photos in the background at screen resolution and keeps the neighbours of the current photo ready as GPU textures, so switching feels instant. Ratings go straight into the file as `xmp:Rating`, which Lightroom, Bridge, digiKam and Windows Explorer read – without changing the file's modification or creation date. Sharpness and aesthetics are computed locally (GPU via DirectML on Windows) and kept in Cerno's own database, never in your photos.

---

## Table of Contents

- [Features](#features)
- [Quick Start](#quick-start)
- [Usage](#usage)
- [Scores](#scores)
- [Supported Platforms & Formats](#supported-platforms--formats)
- [Development & Build](#development--build)
- [Roadmap & Known Issues](#roadmap--known-issues)
- [License](#license)

## Features

- **Instant switching** – background workers prefetch the photos around the current one, decoded at monitor resolution and uploaded as GPU textures.
- **Keyboard-first rating** – `1`–`5` set stars, `0` clears; or click the stars in the info bar.
- **Safe metadata writes** – only the rating is written, in the background and debounced. File modification and creation dates stay bit-exact. Windows Explorer's own rating tags are kept in sync if present.
- **Zoom** – `Z` or double-click toggles 100 %, mouse wheel zooms around the cursor, drag pans. Full resolution is loaded on demand; zoom and position stay when you switch photos, so a series can be compared at the same spot.
- **Filmstrip** – thumbnails with your stars and a marker for probably blurry shots.
- **Sharpness and aesthetics** – see [Scores](#scores). Sort by rating, aesthetics or sharpness; filter by stars; hide the blurriest shots.
- **Capture info** – camera, lens, focal length, aperture, shutter speed, ISO and capture date.
- **Correct orientation**, natural sort order (`IMG_2` before `IMG_10`, umlauts next to their base letter).

## Quick Start

There are no binary releases yet – build from source (see [Development & Build](#development--build)), then:

```bash
cargo run --release --features heic -- "D:/Photos/2026-09 Trip"
```

You can also start without an argument and drop a folder or photo onto the window, or press `Ctrl+O`. When copying `target/release/cerno.exe` elsewhere, copy `DirectML.dll` from the same folder along with it.

**Requirement:** [ExifTool](https://exiftool.org/) on `PATH` for writing ratings (Windows: `winget install OliverBetz.ExifTool`, Debian/Ubuntu: `apt install libimage-exiftool-perl`). Viewing works without it. `CERNO_EXIFTOOL` can point to a specific executable.

## Usage

| Input | Action |
|---|---|
| `→` `Space` `PageDown` | Next photo (hold to scroll) |
| `←` `Backspace` `PageUp` | Previous photo |
| `Home` / `End` | First / last photo |
| `1`–`5` / `0` | Set star rating / remove it |
| `Z`, double-click | Toggle fit ↔ 100 % |
| `+` / `-`, mouse wheel | Zoom in / out |
| Drag | Pan while zoomed |
| `T` | Toggle the filmstrip |
| `I` | Toggle toolbar, filmstrip and info bar |
| `F11` / `F` | Toggle fullscreen |
| `Esc` | Leave zoom, then fullscreen |
| `Ctrl+O` | Open folder |

## Scores

Cerno analyses every photo of the open folder in the background (nearest first, paused while you browse). Results are stored in `%LOCALAPPDATA%\Cerno\data\cerno.db` (Linux: `~/.local/share/cerno/cerno.db`), keyed by the image content, so renamed photos keep their scores.

- **Sharpness** – variance of the Laplacian on the sharpest tiles, so a blurred background doesn't penalise a sharp subject. Shown as a percentile *within the folder*: "Sharpness 87 %" means sharper than 87 % of this series. The blurriest 20 % are marked "probably blurry".
- **Aesthetics** – CLIP ViT-L/14 image embedding scored by the [LAION improved aesthetic predictor](https://github.com/christophschuhmann/improved-aesthetic-predictor), roughly 1–10 (ordinary photos around 4–6). It judges the overall impression, not technical quality. Click **Enable aesthetics…** in the toolbar once to download the model (1.2 GB, Hugging Face). It runs on the GPU via DirectML on Windows (any DX12 GPU incl. AMD), otherwise on the CPU.

Everything runs locally. Scores never change your star ratings.

## Supported Platforms & Formats

| | Windows | Linux |
|---|---|---|
| Rendering | wgpu (DX12 / Vulkan) | wgpu (Vulkan), X11 and Wayland |
| JPEG | ✓ | ✓ |
| HEIC / HEIF | ✓ with `--features heic` (libheif via vcpkg) | ✓ with `--features heic` (system libheif ≥ 1.17) |
| Aesthetics model | DirectML (GPU), CPU fallback | CPU; WebGPU with `--features webgpu` (experimental) |

## Development & Build

Prerequisites: Rust stable, a C/C++ toolchain (Windows: Visual Studio Build Tools with the C++ workload). The first build downloads prebuilt ONNX Runtime binaries (`ort` crate).

```bash
cargo run -- <folder>                 # debug build; dependencies are optimised, so decoding stays fast
cargo build --release --features heic
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

1. Personal taste score learned from your own star ratings (uses the stored CLIP embeddings).
2. Linux verification (build, HEIC, WebGPU on AMD).
3. Installers (`.msi`, `.deb`, `.AppImage`) and an updater.

**Known issues**

- No colour management yet: embedded ICC profiles (e.g. Adobe RGB) are ignored.
- Because the modification date is preserved and XMP padding often keeps the size equal, backup/sync tools that only compare size and date (e.g. `rsync` without `-c`) may not notice a rating change.
- Truncated JPEGs are shown partially, with the missing part in grey.

## License

[Apache-2.0](./LICENSE). Third-party components and model weights: [THIRD_PARTY.md](./THIRD_PARTY.md).
