# Contributing

Thank you for helping improve **Cerno**.

## Prerequisites

1. [Rust (stable)](https://rustup.rs/)
2. A C/C++ toolchain (Windows: Visual Studio Build Tools with the C++ workload)
3. Optional: [ExifTool](https://exiftool.org/) on `PATH` for rating write tests (`winget install OliverBetz.ExifTool` on Windows; `apt install libimage-exiftool-perl` on Debian/Ubuntu)
4. Optional HEIC: libheif via vcpkg on Windows, or `libheif-dev` (≥ 1.17) on Linux — see [HEIC](#heic)

## Getting started

```bash
git clone https://github.com/fly2nbc-oss/Cerno.git
cd Cerno
cargo run --release -- path/to/photos
```

## Checking your build

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features heic,video -- -D warnings
cargo test
cargo test --features heic,video
```

The ExifTool round-trip test (`rating::tests::writes_stars_and_keeps_file_dates`) skips silently when ExifTool is missing — make sure it actually ran before trusting rating-write changes. Fixture: `tests/fixtures/tiny.jpg`.

The first build downloads ONNX Runtime. A release build puts `DirectML.dll` next to `cerno.exe`, a `--features heic` build also `heif.dll`, `libde265.dll` and `licenses/`; keep them beside the exe when you copy it elsewhere.

## HEIC

**Linux:** `sudo apt install libheif-dev` (≥ 1.17, Ubuntu 24.04 or newer).

**Windows:** libheif comes from vcpkg in `target/vcpkg`, dynamically linked (`.cargo/config.toml` sets `VCPKGRS_DYNAMIC=1`). `cargo vcpkg build` clones vcpkg but its bootstrap step fails on current Rust, so finish by hand:

```bash
cargo install cargo-vcpkg && cargo vcpkg build
target\vcpkg\bootstrap-vcpkg.bat -disableMetrics
target\vcpkg\vcpkg.exe install "libheif[core]:x64-windows"
cargo clean -p libheif-sys
cargo build --release --features heic
```

`cargo clean -p libheif-sys` is only needed once, if the tree was built against the static triplet before.

## Video playback

`--features video` plays videos through GStreamer.

**Linux:** `sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav`.

**Windows:** `pwsh packaging/windows/install-gstreamer.ps1` installs the pinned official MSVC build for your user (no admin rights). Then, in the shell you build from:

```powershell
$gst = "$env:LOCALAPPDATA\Programs\gstreamer\1.0\msvc_x86_64"
$env:PKG_CONFIG = "$gst\bin\pkg-config.exe"; $env:PKG_CONFIG_PATH = "$gst\lib\pkgconfig"
$env:PATH = "$gst\bin;$env:PATH"   # for cargo run and cargo test
```

`cerno --check-video tests/fixtures/tiny.mp4` plays a second of a clip without a window and says what decoded it.

## Packages

The scripts CI runs, after `cargo build --release --features heic,video`:

```bash
pwsh packaging/windows/build.ps1   # dist/windows: portable folder + zip, NSIS installer (needs cargo-packager)
packaging/linux/build.sh           # dist/linux: .deb (needs cargo-deb) and AppImage – on Ubuntu 24.04
```

## Continuous Integration

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on pushes and pull requests to **`main`** and on `v*` tags:

- **Lint & test (Linux, HEIC, video)** — `cargo fmt --check`, `cargo clippy` and `cargo test` with `--features heic,video` on Ubuntu 24.04 (libheif, GStreamer and ExifTool installed)
- **Test & build (Windows)** — `cargo clippy` and `cargo test` without features and with `video` (GStreamer installed; no GPU, so the player's CPU path), `cargo build --release`
- **Packages** (pushes to `main`, tags and manual runs, not pull requests) — the Windows installer and portable zip, the AppImage and the `.deb`, kept as run artifacts; a `v*` tag (equal to the `Cargo.toml` version) drafts a GitHub release with them.

## Pull requests

- Keep changes focused on one concern where possible.
- Describe **what** changed and **why** in the PR description.
- For UI changes, attach screenshots when practical.
- `cargo fmt --check`, clippy (`-D warnings`, with and without `--features heic,video` where you can), and tests should pass.

## Code of conduct

Participants are expected to follow our [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions are licensed under the project’s [Apache-2.0 license](./LICENSE).
