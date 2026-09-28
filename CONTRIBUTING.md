# Contributing

Thank you for helping improve **Cerno**.

## Prerequisites

1. [Rust (stable)](https://rustup.rs/)
2. A C/C++ toolchain (Windows: Visual Studio Build Tools with the C++ workload)
3. Optional: [ExifTool](https://exiftool.org/) on `PATH` for rating write tests (`winget install OliverBetz.ExifTool` on Windows; `apt install libimage-exiftool-perl` on Debian/Ubuntu)
4. Optional HEIC: libheif via vcpkg on Windows, or `libheif-dev` (≥ 1.17) on Linux — see [README.md](./README.md#heic-support)

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
cargo clippy --all-targets --features heic -- -D warnings
cargo test
cargo test --features heic
```

The ExifTool round-trip test (`rating::tests::writes_stars_and_keeps_file_dates`) skips silently when ExifTool is missing — make sure it actually ran before trusting rating-write changes. Fixture: `tests/fixtures/tiny.jpg`.

## Continuous Integration

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on pushes and pull requests to **`main`**:

- **Lint & test (Linux, HEIC)** — `cargo fmt --check`, `cargo clippy` with `--features heic`, `cargo test --features heic` on Ubuntu 24.04 (libheif + ExifTool installed)
- **Test & build (Windows)** — `cargo clippy` without HEIC, `cargo test`, `cargo build --release`

There are no installer/release workflows yet; binaries are built from source.

## Pull requests

- Keep changes focused on one concern where possible.
- Describe **what** changed and **why** in the PR description.
- For UI changes, attach screenshots when practical.
- `cargo fmt --check`, clippy (`-D warnings`, with and without `--features heic` where you can), and tests should pass.

## Code of conduct

Participants are expected to follow our [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions are licensed under the project’s [Apache-2.0 license](./LICENSE).
