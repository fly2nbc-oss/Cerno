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

Exactly what CI runs (`cargo audit` needs `cargo install cargo-audit`; the Linux list needs the HEIC and video packages below):

```bash
cargo fmt --all -- --check
# Linux (CI: Ubuntu 24.04)
cargo clippy --all-targets --locked --features heic,video -- -D warnings
cargo test --locked --features heic,video
cargo clippy --all-targets --locked --features webgpu -- -D warnings
cargo audit
# Windows (CI: without HEIC; video needs GStreamer, see below)
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo clippy --all-targets --locked --features video -- -D warnings
cargo test --locked --features video
```

The ExifTool round-trip test (`rating::tests::writes_stars_and_keeps_file_dates`) skips silently when ExifTool is missing — make sure it actually ran before trusting rating-write changes. Fixture: `tests/fixtures/tiny.jpg`.

What the whole app does with a key – which card, list or mode takes it, in which order a frame's keys run – is tested without a window through `src/app/harness.rs`: an index in memory, a writer without its thread, temporary copies of the fixture. Nothing there touches your photos, your index or the models.

The first build downloads ONNX Runtime. A release build puts `DirectML.dll` next to `cerno.exe`, a `--features heic` build also `heif.dll`, `libde265.dll` and `licenses/`; keep them beside the exe when you copy it elsewhere.

## HEIC

**Linux:** `sudo apt install libheif-dev` (≥ 1.17, Ubuntu 24.04 or newer).

**Windows:** libheif comes from vcpkg in `target/vcpkg`, dynamically linked (`.cargo/config.toml` sets `VCPKGRS_DYNAMIC=1`). `cargo vcpkg build` clones vcpkg but its bootstrap step fails on current Rust, so finish by hand:

```bash
cargo install cargo-vcpkg && cargo vcpkg build
target\vcpkg\bootstrap-vcpkg.bat -disableMetrics
target\vcpkg\vcpkg.exe install "libheif[core,libde265]:x64-windows"
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

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on pushes to `main`, on pull requests, on `v*` tags and by hand. Every action is pinned to a commit.

- **Lint scripts & package metadata** (`scripts`, every run, seconds) — actionlint (with shellcheck on the workflow's bash blocks) and zizmor over the workflow, shellcheck over the shell scripts, PSScriptAnalyzer over the PowerShell scripts, `desktop-file-validate` and `appstreamcli validate` over the Linux desktop file and AppStream metadata, and a check that the metadata's newest release is the version in `Cargo.toml`.
- **Lint & test (Linux, HEIC, video)** (`linux`, every run) — the Linux commands above on Ubuntu 24.04 with libheif, GStreamer and ExifTool installed, including the clippy run with the experimental `webgpu` feature and `cargo audit` (a known vulnerability fails the run).
- **Test & build (Windows)** (`windows`, every run) — the Windows commands above; GStreamer is installed, and without a GPU the player uses WARP. Pull requests also build the release.
- **Packages** (`package-windows`, `package-linux`; pushes to `main`, tags and manual runs) — the installer, the portable zip, the AppImage with its `.zsync` and the `.deb`, kept 7 days as run artifacts. The Windows job installs, starts and uninstalls the installer; the Linux packages answer `--version`.
- **Install tests** (`test-linux-packages`) — the `.deb` on bare Ubuntu 24.04 and Debian 13, the AppImage there and on Fedora 44 and Arch Linux: each opens a HEIC photo under Xvfb and plays a video without a window.
- **Signing** — on `v*` tags only, and only once the `release` environment has the SignPath organisation and token: `cerno.exe` and the installer are sent to SignPath, approved by hand and checked with `signtool verify` (see the README's *Code signing policy*). Without them the packages stay unsigned with a warning.
- **Draft release** (`release`, `v*` tags) — checks that the tag equals the `Cargo.toml` version and that `CHANGELOG.md` has its section, writes `SHA256SUMS.txt`, attests build provenance for every file (`gh attestation verify <file> --repo fly2nbc-oss/Cerno`) and drafts the GitHub release.
- **`test_rounds`** — a manual run (*Run workflow*) can repeat the Linux and Windows tests up to 99 times to catch a flaky test; the step fails when any round failed.

## Pull requests

- Keep changes focused on one concern where possible.
- Describe **what** changed and **why** in the PR description.
- For UI changes, attach screenshots when practical.
- The commands of [Checking your build](#checking-your-build) should pass where you can run them; CI runs all of them.

## Code of conduct

Participants are expected to follow our [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions are licensed under the project’s [Apache-2.0 license](./LICENSE).
