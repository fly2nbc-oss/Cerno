# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Cerno (Latin *cerno* – "I sift, discern, see clearly") is a **pure-Rust desktop photo viewer and culling tool** for Windows and Linux: switch between photos instantly, rate them 1–5 stars from the keyboard, zoom to 100 %, and sort/filter by automatic sharpness and aesthetics scores. UI is **egui/eframe on wgpu** – there is no WebView. The parent mono-repo `../CLAUDE.md` covers shared conventions; this file covers Cerno specifics.

## Commands

```bash
cargo run --release --features heic -- <folder-or-image>   # deps are opt-level 3 even in debug
cargo fmt --all -- --check
cargo clippy --all-targets --features heic -- -D warnings  # also run once without --features heic
cargo test --features heic
cargo test natural_order                                   # single test
CERNO_TEST_HEIC=<file.heic> cargo test --features heic -- --include-ignored   # + HEIC rating round trip
```

On Windows, `cargo` lives in `%USERPROFILE%\.cargo\bin` (rustup default location). The release build puts `DirectML.dll` next to `target\release\cerno.exe` (ort's `copy-dylibs`); the exe needs it beside it when copied elsewhere.

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
| Formats: **JPEG + HEIC** | User requirement. HEIC via `libheif-rs` (vcpkg on Windows, system libheif ≥ 1.17 on Linux), behind the `heic` feature. |
| Only the **1–5 star rating** is written into the original file (`xmp:Rating`) | Everything else (scores, thumbnails, CLIP embeddings) lives in a central SQLite DB under the app data dir – never next to or inside the photos. AI scores never set stars. |
| **File dates must never change** | ExifTool runs with `-P -overwrite_original_in_place`, and `filetimes.rs` snapshots modified/created time before each write and restores them afterwards. |
| GPU inference must work on **all vendors, AMD has priority** | Windows: ONNX Runtime + DirectML (any DX12 GPU). Linux: CPU, ORT WebGPU (Vulkan) behind the experimental `webgpu` feature. No CUDA/ROCm-only paths. Same model everywhere so scores stay comparable. |
| Aesthetics = CLIP ViT-L/14 + LAION improved aesthetic predictor | Established, local, 1–10 scale. The LAION MLP has no activations and is collapsed into one linear layer (`tools/make_aesthetic_head.py` → `src/analysis/aesthetic_head.bin`, 3 KB, embedded). The 1.2 GB ONNX vision model is downloaded on request into `%LOCALAPPDATA%\Cerno\data\models` (`~/.local/share/cerno/models`). |
| CLIP embeddings are stored per image | Basis for the planned personal taste model (learn from the user's own stars) without re-running CLIP. |
| Design system `ui_design_system_v1.2.md` (see `../MediaFileRenamer/docs/`) | Dark tokens for all chrome, accent `#5B8EC4`, Segoe UI loaded from the system (egui's bundled font lacks ← →). Exception: the photo canvas is **neutral** dark grey, because the bluish `--bg` would bias colour judgement. UI language is English, like the sibling apps. |

## Architecture

```
main.rs              eframe bootstrap (wgpu renderer), CLI path argument
app.rs               CernoApp: state, keyboard/mouse model, layout, view (sort/filter) management
view.rs              sorting + filtering of the folder list (pure, unit-tested)
ui/viewer.rs         fit / zoom / pan geometry and drawing (display texture or full-res tiles)
ui/filmstrip.rs      thumbnail strip centred on the current photo
ui/bars.rs           toolbar, two-row info bar, notices, empty state, drop hint
ui/stars.rs          star shapes
library.rs           folder scan, supported extensions, natural accent-insensitive order
loader.rs            prefetch worker pool + texture cache; full-resolution tiles for zoom
decode.rs            bytes → RGB8 at a target size: JPEG (zune-jpeg) / HEIC (libheif), resize, EXIF orientation
metadata.rs          rating, orientation, camera/exposure data from the in-memory file bytes
analysis/mod.rs      background analysis (thumbnail, fingerprint, sharpness, aesthetics) + ScoreBoard + model download
analysis/sharpness.rs  tile-based Laplacian variance, percentile helper
analysis/aesthetic.rs  ONNX session (DirectML → CPU), CLIP preprocessing, embedded LAION head
thumbs.rs            filmstrip textures from loader / analysis / database
db.rs                SQLite index: files (path+stamp → fingerprint, rating), images (scores, thumbnail, embedding), settings
rating.rs            debounced background writer, one long-lived ExifTool process (-stay_open)
exiftool.rs          ExifTool stay-open protocol
filetimes.rs         snapshot / restore of file timestamps
paths.rs             data, model and database locations
theme.rs             design tokens → egui Visuals, system UI font
```

### Loader (the speed-critical part)

- Workers (`available_parallelism - 1`, clamped 2..=4) pick the **most urgent** missing index relative to the *current* index at pick time: `0, full-res of 0 (if zoomed), +1, -1, +2, +3, -2, -3`. Jumping around re-prioritises automatically; nothing is queued ahead of time.
- Display images are decoded at **monitor resolution** (clamped to `max_texture_side`). Full resolution is loaded only for the current photo while zoomed, split into tiles ≤ 4096 px.
- Workers upload textures themselves (`egui::Context` is `Send + Sync`). They also downscale the decoded image into the filmstrip thumbnail – the neighbourhood's thumbnails are free.
- `set_library` carries cached images over by path, so re-sorting or filtering decodes nothing again. A generation counter discards results that finish after the list changed.

### Analysis

- Two workers, nearest-first over the **whole folder** (not the filtered view), paused for 0.9 s after every navigation so display decoding wins.
- Fast path: `stat` + one query; if the index has complete data (sharpness of the current `sharpness::VERSION`, thumbnail, aesthetics of the current `aesthetic::MODEL_ID` when the model is available) nothing is decoded.
- Full pass: decode at 2048 px → thumbnail (256 px JPEG in the DB) → **fingerprint = FNV-1a over thumbnail pixels + full size** → sharpness → aesthetics (one shared ORT session, loaded lazily).
- Records are keyed by fingerprint, so renamed files keep their scores. The rating writer updates the file's size in `files` after each write, so our own writes don't trigger re-fingerprinting.
- `Analyzer::preload` fills the ScoreBoard from the DB when a folder opens, so saved sort/filter settings apply immediately. `ScoreBoard::version` only changes on real changes; the toolbar offers "Refresh order" when scores arrived after the view was built.

### Rating writes

- The UI updates immediately (session map `path → rating`); the write is queued.
- The writer debounces per file (pressing 3 then 4 quickly = one write) and skips writes that don't change the value read from the file.
- Only `XMP-xmp:Rating` is written, plus the Microsoft rating tags (`EXIF:Rating`/`RatingPercent`, `XMP-microsoft:RatingPercent`) **only if the file already has them**, so Windows Explorer never shows a stale value. `0` deletes the tags.
- `on_exit` flushes pending writes and joins the writer thread – a rating set just before closing must not be lost.

## Roadmap

1. ✅ Viewer, prefetch, star rating, timestamp-preserving writes.
2. ✅ HEIC (Windows verified with a real sample incl. rating round trip), filmstrip, 100 % zoom with tiles, SQLite index, sharpness, aesthetics on DirectML, sort/filter, camera/exposure info.
3. Personal taste model: ridge regression on the stored CLIP embeddings against the user's own stars (after ~50–100 ratings), shown next to the LAION score.
4. Linux verification (build, libheif, WebGPU on AMD/Vulkan).
5. Packaging with `cargo-packager` (`.msi`/NSIS, `.deb`, `.AppImage`) + updater; ship `DirectML.dll`; resolve the libde265 LGPL question first (see Gotchas).

## Gotchas

- **EXIF orientation applies to JPEG only.** For HEIC, libheif already applies the `irot`/`imir` transforms; applying EXIF orientation again would double-rotate.
- ExifTool must get `-charset filename=UTF8`, otherwise paths with umlauts fail on Windows. (Same trap when checking files by hand from PowerShell: pass the folder, not the umlaut file name; and quote `"-FNumber=2.8"` – PowerShell splits unquoted `-x=2.8`.)
- ExifTool's `-P` shifts mtime and creation time by a few **microseconds** (Perl floats). `filetimes::Snapshot::restore` puts the exact values back after every write – don't remove it just because `-P` "already preserves dates".
- **In-place writes are deliberately not atomic.** `-overwrite_original_in_place` copies the new bytes back into the original file so its identity (creation date, inode) survives – the user's "dates must not change" requirement wins over temp-file + rename. Trade-off: a crash during the copy-back can corrupt that one file.
- **Torn reads are possible:** if the loader reads a file while ExifTool copies bytes back into it (rate, navigate away and back within the debounce window), the decode fails and the slot shows an error until it is evicted and re-decoded. Rare and self-healing – keep the eviction, don't cache failures forever.
- **Fingerprints depend on the decoder and resizer.** Upgrading zune-jpeg, libheif or fast_image_resize can change them; that only means a re-analysis, no data loss. Never use `std::hash::DefaultHasher` for them (not stable across Rust versions).
- DirectML is registered with `error_on_failure()` and falls back to a CPU session explicitly, so the toolbar can show the real backend. It requires `with_memory_pattern(false)`.
- The ONNX output is picked by shape (the `[batch, 768]` one, `image_embeds`), not by name.
- zune-jpeg decodes truncated JPEGs leniently (missing part grey) instead of failing – intended, other viewers do the same.
- `libheif[core]` is linked statically, which pulls in libde265 (**LGPL-3.0**). Before publishing binaries, switch to the dynamic triplet (`x64-windows` + `VCPKGRS_DYNAMIC=1`) and ship the DLLs, or otherwise satisfy the LGPL relinking terms. The x265 encoder (GPL) is deliberately excluded.
- `rfd` dialogs (folder picker, model download confirmation) block the UI thread while open – fine for modal dialogs.
- Linux always updates `ctime` on write; that can't be prevented and no photo tool shows it. `mtime` and (on Windows) the creation time are preserved.
- Because mtime and often the file size (XMP padding) stay the same, backup/sync tools that only compare size + date (e.g. `rsync` without `-c`) may miss rating changes.
- No colour management yet: embedded ICC profiles (e.g. Adobe RGB) are ignored.
- Smoke-testing the window from scripts: send keys with `PostMessage(WM_KEYDOWN/UP)` to Cerno's window handle and capture with `PrintWindow(PW_RENDERFULLCONTENT)`. Never use `SendKeys` – without focus it types into whatever window the user is working in.
