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
- **Compare** – `C` pins the current photo on the left, the right side browses the rest. `A` keeps the left one, `D` the right one; the other is deleted and the next photo moves in, so a burst is culled in a few keystrokes. Both sides zoom together.
- **Delete without dialogs** – `Delete` hides the photo at once and moves it to the trash after a 5-second countdown; every further deletion restarts it, `Esc` brings all waiting photos back. Nothing blocks meanwhile.
- **Filmstrip** – thumbnails with your stars and a marker for probably blurry shots.
- **Sharpness, aesthetics and your own taste** – see [Scores](#scores). Sort by rating, either aesthetics score, personal taste or sharpness; filter by stars; hide the blurriest shots.
- **Details panel** – `P` shows every value Cerno measured for the current photo, each with a short explanation in plain words: both aesthetics scores, personal taste, whole-frame and eye sharpness, clipped highlights and shadows, CLIP attributes and the state of each model.
- **Info bar** – always visible: stars, the three aesthetics scores side by side (`LAION / V2.5 / personal ★`), sharpness, capture data incl. digital zoom, the current zoom level, and a map pin for photos with GPS coordinates (click opens Google Maps). Buttons at its right end show or hide the top bar, details panel and filmstrip.
- **Five languages** – German, English, French, Spanish, Italian. Cerno starts in your system language; `L` switches, a flag shows the new one.
- **Help** – `H` or `F1` lists all shortcuts with a short explanation; the start screen shows the same page.
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
| Mouse wheel over the filmstrip | Step through the photos |
| `1`–`5` / `0` | Set star rating / remove it |
| `Delete` | Delete (to the trash after 5 s; `Esc` undoes) |
| `C` | Compare: pin the current photo on the left / leave compare mode |
| `A` / `D` | Compare: keep left / keep right – the other one is deleted |
| `Z`, double-click | Toggle fit ↔ 100 % |
| `+` / `-`, mouse wheel | Zoom in / out |
| Drag | Pan while zoomed |
| `B` | Toggle the top bar |
| `T` | Toggle the filmstrip |
| `P` | Toggle the details panel (mouse wheel scrolls it) |
| `I` | Top bar, details panel and filmstrip together (the info bar always stays) |
| `F11` / `F` | Toggle fullscreen |
| `L` | Switch language (DE → EN → FR → ES → IT) |
| `H` / `F1` | Help page with all shortcuts |
| `Esc` | Close help, undo pending deletions, then leave zoom, compare mode, fullscreen |
| `Ctrl+O` | Open folder |

## Scores

Cerno analyses every photo of the open folder in the background (nearest first, paused while you browse). Results are stored in `%LOCALAPPDATA%\Cerno\data\cerno.db` (Linux: `~/.local/share/cerno/cerno.db`), keyed by the image content, so renamed photos keep their scores.

- **Sharpness** – variance of the Laplacian on the sharpest tiles, so a blurred background doesn't penalise a sharp subject. Shown as a percentile *within the folder*: "Sharpness 87 %" means sharper than 87 % of this series. The blurriest 20 % are marked "probably blurry".
- **Eye sharpness** – for portraits, a small face detector ([YuNet](https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet), built in) finds the eyes and Cerno measures sharpness right there. When a face is found, this value decides "blurry or not" instead of the whole frame – a sharp face in front of a soft background is not penalised.
- **Aesthetics (LAION)** – CLIP ViT-L/14 image embedding scored by the [LAION improved aesthetic predictor](https://github.com/christophschuhmann/improved-aesthetic-predictor), roughly 1–10 (ordinary photos around 4–6). It judges the overall impression, not technical quality. Click **Enable aesthetics…** in the toolbar once to download the model (1.2 GB, Hugging Face). It runs on the GPU via DirectML on Windows (any DX12 GPU incl. AMD), otherwise on the CPU.
- **Aesthetics (V2.5)** – SigLIP so400m scored by [Aesthetic Predictor V2.5](https://github.com/discus0434/aesthetic-predictor-v2-5), same 1–10 scale, noticeably better on real-world photos. When present, the info bar shows it instead of LAION. There is no download button yet: put `siglip-so400m-patch14-384-vision.onnx` and `aesthetic-predictor-v2.5-head.bin` (made with the scripts in `tools/`) into the models folder next to the CLIP model and restart. The V2.5 head is AGPL-3.0 and therefore not part of Cerno.
- **Personal taste** – learns from your own decisions: every rated photo (1–5 stars) and every photo you deleted (0 stars) is an example. From 15 examples on, Cerno predicts stars for the rest of your photos, retrained a few seconds after every change. The details panel shows how many photos it learned from and how far off it typically is (e.g. "±0.7 ★").
- **Exposure** – share of blown highlights (all channels ≥ 250) and crushed shadows (all ≤ 2); highlighted in orange above 1 % and 5 %.
- **CLIP attributes** – quality, sharpness, lighting, composition, noise and colourfulness judged zero-shot from the same CLIP embedding (CLIP-IQA style), 0–100 %. Rough indicators, useful for spotting outliers.

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

1. Download button for the V2.5 model files.
2. Linux verification (build, HEIC, WebGPU on AMD).
3. Installers (`.msi`, `.deb`, `.AppImage`) and an updater.

**Known issues**

- No colour management yet: embedded ICC profiles (e.g. Adobe RGB) are ignored.
- Because the modification date is preserved and XMP padding often keeps the size equal, backup/sync tools that only compare size and date (e.g. `rsync` without `-c`) may not notice a rating change.
- Truncated JPEGs are shown partially, with the missing part in grey.
- Deleted photos go to the system trash (Recycle Bin / freedesktop trash), not straight to oblivion. Closing Cerno during the countdown carries the deletion out.

## License

[Apache-2.0](./LICENSE). Third-party components and model weights: [THIRD_PARTY.md](./THIRD_PARTY.md).
