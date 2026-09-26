# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Cerno (Latin *cerno* – "I sift, discern, see clearly") is a **pure-Rust desktop photo viewer and culling tool** for Windows and Linux: switch between photos instantly, rate them 1–5 stars from the keyboard, and (later) get automatic sharpness and aesthetics scores. UI is **egui/eframe on wgpu** – there is no WebView. The parent mono-repo `../CLAUDE.md` covers shared conventions; this file covers Cerno specifics.

## Commands

```bash
cargo run --release -- <folder-or-image>   # debug builds decode fine too: deps are built with opt-level 3
cargo run --release --features heic -- <folder>
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test natural_order                    # single test
```

On Windows, `cargo` lives in `%USERPROFILE%\.cargo\bin` (rustup default location).

Unit tests live in-module (`#[cfg(test)]`). `rating::tests::writes_stars_and_keeps_file_dates` is a real round trip through ExifTool (umlaut file name, mtime + creation time compared) and silently skips when ExifTool isn't on `PATH` – make sure it actually ran before trusting a change to the write path. Fixture: `tests/fixtures/tiny.jpg`.

HEIC on Windows needs libheif from vcpkg in `target/vcpkg` (static triplet `x64-windows-static-md`, found automatically by the `vcpkg` crate). `cargo vcpkg build` clones the pinned vcpkg revision but its bootstrap step fails on current Rust (it spawns `bootstrap-vcpkg.bat` by relative name), so finish by hand:

```bash
cargo install cargo-vcpkg && cargo vcpkg build     # clones target/vcpkg, then fails at bootstrap
target\vcpkg\bootstrap-vcpkg.bat -disableMetrics
target\vcpkg\vcpkg.exe install "libheif[core]:x64-windows-static-md"
cargo build --release --features heic
```

## Product decisions (settled – build on them, don't re-propose alternatives)

| Decision | Why |
|---|---|
| Pure Rust + egui/eframe (wgpu renderer) instead of Tauri | WebViews can't display HEIC (Rust would have to decode *and* re-encode), WebKitGTK is slow on Linux. Decode once → GPU texture. |
| Formats: **JPEG + HEIC** | User requirement. HEIC via `libheif-rs` (vcpkg on Windows, system libheif on Linux), behind the `heic` feature. |
| Only the **1–5 star rating** is written into the original file (`xmp:Rating`) | Everything else (AI scores, analysis state) goes into a central SQLite DB under the app data dir – not next to the photos. |
| **File dates must never change** | ExifTool runs with `-P -overwrite_original_in_place`, and `filetimes.rs` snapshots modified/created time before each write and restores them afterwards. |
| GPU inference must work on **all vendors, AMD has priority** | Windows: ONNX Runtime + DirectML. Linux: CPU baseline, ORT WebGPU (Vulkan) as experimental opt-in. No CUDA/ROCm-only paths. Same models on every platform so scores stay comparable. |
| Design system `ui_design_system_v1.2.md` (see `../MediaFileRenamer/docs/`) | Dark tokens for all chrome, accent `#5B8EC4`. Exception: the photo canvas is **neutral** dark grey, because the bluish `--bg` would bias colour judgement. |

## Architecture

```
main.rs        eframe bootstrap (wgpu renderer), CLI path argument
app.rs         CernoApp: keyboard model, drawing (canvas, overlay, stars, empty state), drag & drop
library.rs     folder scan, supported extensions, natural filename order
loader.rs      prefetch worker pool + texture cache (window around the current index)
decode.rs      bytes → display-sized RGBA: JPEG (zune-jpeg) / HEIC (libheif, feature), resize (fast_image_resize), EXIF orientation
metadata.rs    rating + orientation read from the in-memory file bytes (XMP packet scan, EXIF fallback)
rating.rs      debounced background writer, one long-lived ExifTool process (-stay_open)
exiftool.rs    ExifTool stay-open protocol
filetimes.rs   snapshot / restore of file timestamps
theme.rs       design tokens → egui Visuals
```

### Loader (the speed-critical part)

- Workers (`available_parallelism - 1`, clamped 2..=4) pick the **most urgent** missing index relative to the *current* index at pick time: `0, +1, -1, +2, +3, -2, -3`. Jumping around re-prioritises automatically; nothing is queued ahead of time.
- Images are decoded at **monitor resolution** (clamped to `max_texture_side`), never full resolution, so a window resize doesn't invalidate the cache.
- Workers upload the texture themselves (`egui::Context` is `Send + Sync`) and call `request_repaint()`. The cache holds `TextureHandle`s; evicting a handle frees the GPU memory.
- A **library generation** counter discards results that finish after the folder changed.
- The file bytes read for decoding are reused for metadata (rating, orientation) – no second read.

### Rating writes

- The UI updates immediately (session map `path → rating`); the write is queued.
- The writer debounces per file (pressing 3 then 4 quickly = one write) and skips writes that don't change the value read from the file.
- Only `XMP-xmp:Rating` is written, plus the Microsoft rating tags (`EXIF:Rating`/`RatingPercent`, `XMP-microsoft:RatingPercent`) **only if the file already has them**, so Windows Explorer never shows a stale value. `0` deletes the tags.
- `on_exit` flushes pending writes and joins the writer thread – a rating set just before closing must not be lost.

## Roadmap

1. ✅ Milestone 1: fast viewer, prefetch, star rating, timestamp-preserving writes (smoke-tested on Windows with an AMD RX 6700 XT).
2. HEIC: the Windows build with `--features heic` works (vcpkg, see Commands) and libheif is called, but decoding a **real** HEIC photo and the Linux build are still unverified. Then: filmstrip with cached thumbnails; 100 % zoom (tiled textures above 8192 px).
3. SQLite index (`rusqlite`, app data dir), keyed by a fingerprint of the **decoded pixels** – not path/size/mtime, because the rating write changes the size while the mtime stays fixed.
4. Analysis module: tile-based sharpness (Laplacian variance / Tenengrad, max over tiles, percentile within the folder), aesthetics via ONNX Runtime (`ort`), filters/sorting by score. Analysis pauses while the user navigates fast.
5. Packaging with `cargo-packager` (`.msi`/NSIS, `.deb`, `.AppImage`) + updater, replacing the Tauri `latest.json` convention.

## Gotchas

- **EXIF orientation applies to JPEG only.** For HEIC, libheif already applies the `irot`/`imir` transforms; applying EXIF orientation again would double-rotate.
- ExifTool must get `-charset filename=UTF8`, otherwise paths with umlauts fail on Windows. (Same trap when checking files by hand from PowerShell: pass the folder, not the umlaut file name.)
- ExifTool's `-P` shifts mtime and creation time by a few **microseconds** (Perl floats). `filetimes::Snapshot::restore` puts the exact values back after every write – don't remove it just because `-P` "already preserves dates".
- **In-place writes are deliberately not atomic.** `-overwrite_original_in_place` copies the new bytes back into the original file so its identity (creation date, inode) survives – the user's "dates must not change" requirement wins over temp-file + rename. Trade-off: a crash during the copy-back can corrupt that one file.
- **Torn reads are possible:** if the loader reads a file while ExifTool copies bytes back into it (rate, navigate away and back within the debounce window), the decode fails and the slot shows an error until it is evicted and re-decoded. Rare and self-healing – keep the eviction, don't cache failures forever.
- zune-jpeg decodes truncated JPEGs leniently (missing part grey) instead of failing – intended, other viewers do the same.
- `libheif[core]` is linked statically, which pulls in libde265 (**LGPL-3.0**). Before publishing binaries, switch to the dynamic triplet (`x64-windows` + `VCPKGRS_DYNAMIC=1`) and ship the DLLs, or otherwise satisfy the LGPL relinking terms. The x265 encoder (GPL) is deliberately excluded.
- `rfd`'s folder dialog blocks the UI thread while open – fine for a modal dialog, don't call it from elsewhere.
- Linux always updates `ctime` on write; that can't be prevented and no photo tool shows it. `mtime` and (on Windows) the creation time are preserved.
- Because mtime and often the file size (XMP padding) stay the same, backup/sync tools that only compare size + date (e.g. `rsync` without `-c`) may miss rating changes.
- No colour management yet: embedded ICC profiles (e.g. Adobe RGB) are ignored.
