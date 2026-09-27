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
| Second aesthetics score: SigLIP so400m + **Aesthetic Predictor V2.5** | Newer and better on real photos than LAION. The V2.5 head is **AGPL-3.0**, so it is never embedded or committed: `tools/extract_siglip_vision.py` cuts the vision tower out of onnx-community's combined SigLIP export (1.7 GB), `tools/make_aesthetic_head.py` collapses the head (1153 floats). Distribution plan: publish both files once in the user's own Hugging Face repo (AGPL notice + source link for the head) and let Cerno download them like CLIP – the app never converts models itself. Until that repo exists there is no download button; the files are dropped into the models dir and detected at start. |
| Only the **six modules** below, all local | LAION, V2.5, personal taste, CLIP attributes, exposure, eye sharpness. They share the decoded 2048 px image and the stored embeddings; adding a module means a DB column + version constant + `is_complete` check. |
| Personal taste model = ridge regression on the stored CLIP embeddings | The user's stars (1–5) are the labels, deleted photos count as 0 (`feedback` table – the file row is gone, the embedding stays). Trained in the background 2 s after ratings/deletions change, from 15 examples; 5-fold CV picks λ and reports the typical error in stars. AI scores still never set stars. |
| Eye sharpness via YuNet (OpenCV Zoo, MIT, 230 KB, embedded) | Shallow depth of field: a sharp face in a blurred background would otherwise rank low. When a face ≥ 40 px is found, the "subject" sharpness (info bar, filmstrip marker, Hide blurry, sort) uses the eye percentile instead of the whole-frame one. |
| Design system `ui_design_system_v1.2.md` (see `../MediaFileRenamer/docs/`) | Dark tokens for all chrome, accent `#5B8EC4`, Segoe UI loaded from the system (egui's bundled font lacks ← →). Exception: the photo canvas is **neutral** dark grey, because the bluish `--bg` would bias colour judgement. Icons (flags, map pin, panel toggles) are painted in `ui/icons.rs` – egui has neither flag emoji nor Lucide. |
| UI in **German, English, French, Spanish, Italian** | User requirement. Starts in the system language (`sys-locale`, fallback English), `L` cycles (flag fades in), the choice is saved. Numbers keep the decimal point in every language ("6.1" next to "f/2.8" – a comma would mix notations). |
| **The info bar always shows**; top bar, details panel and filmstrip are optional | Buttons at the bottom right (and `B`/`P`/`T`, `I` for all three) toggle them; states are saved. The top bar also appears when a filter hides every photo, so the filter can be changed back. |
| Version **0.7.0** is the first numbered one | `Cargo.toml` is the only source; the help page and start screen show `CARGO_PKG_VERSION`. |

## Architecture

```
main.rs              eframe bootstrap (wgpu renderer), CLI path argument
app.rs               CernoApp: state, keyboard/mouse model, layout, view (sort/filter) management
view.rs              sorting + filtering of the folder list (pure, unit-tested)
ui/viewer.rs         fit / zoom / pan geometry and drawing (display texture or full-res tiles)
ui/filmstrip.rs      thumbnail strip centred on the current photo
ui/bars.rs           toolbar, two-row info bar (meters, zoom, GPS pin, panel buttons), notices, language flash
ui/details.rs        side panel with every analysis value + plain-language explanation (`P`), scrolls
ui/help.rs           help page (`H`/`F1`, modal foreground area) and the start screen (same content)
ui/icons.rs          painted flags, map pin, panel and help icons
ui/stars.rs          star shapes
i18n/                `Texts` struct (mod.rs) + one full literal per language (de/en/fr/es/it), date and coordinate formatting
library.rs           folder scan, supported extensions, natural accent-insensitive order
loader.rs            prefetch worker pool + texture cache; full-resolution tiles for zoom
decode.rs            bytes → RGB8 at a target size: JPEG (zune-jpeg) / HEIC (libheif), resize, EXIF orientation
metadata.rs          rating, orientation, camera/exposure data from the in-memory file bytes
analysis/mod.rs      background analysis (only the missing parts per image) + ScoreBoard + model download + taste trainer thread
analysis/sharpness.rs  tile-based Laplacian variance, region variance, percentile helper
analysis/aesthetic.rs  ONNX encoders (DirectML → CPU): CLIP + embedded LAION head, SigLIP + V2.5 head from the models dir
analysis/attributes.rs CLIP-IQA-style zero-shot attributes from the embedding (embedded prompt vectors)
analysis/exposure.rs   share of blown highlights / crushed shadows
analysis/faces.rs      YuNet face detection (CPU, embedded model) + sharpness around the eyes
analysis/taste.rs      ridge regression (Cholesky, k-fold CV) from embeddings to the user's stars
thumbs.rs            filmstrip textures from loader / analysis / database
deletion.rs          delayed deletion queue (countdown, undo, trash worker)
db.rs                SQLite index: files (path+stamp → fingerprint, rating), images (scores, thumbnail, embedding), feedback (deletions), settings; additive migration
tools/*.py           one-off model preparation (collapse heads, extract the SigLIP tower, CLIP prompt vectors)
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
- Fast path: `stat` + one query; if `is_complete` holds (thumbnail, sharpness/exposure/faces of their current `VERSION`, LAION/V2.5 of the current model id when that model is available) nothing is decoded.
- Otherwise decode at 2048 px once and compute **only what is missing**: thumbnail (256 px JPEG in the DB) → **fingerprint = FNV-1a over thumbnail pixels + full size** → sharpness → exposure → faces + eye sharpness → CLIP (LAION score + embedding) → SigLIP (V2.5). Bumping one module's version re-runs just that module. Each model has its own lazily loaded session (`Slot`) and `ModelState` for the UI.
- Taste trainer: a separate thread retrains from `Db::taste_examples` 2 s after `taste_changed()` (rating set, deletion carried out). Predictions come from the in-memory embeddings map, so `Analyzer::personal` is cheap per frame.
- DB migration is additive: `migrate()` reads `PRAGMA table_info(images)` and `ALTER TABLE … ADD COLUMN`s what is missing. Never drop or recreate tables – the index holds the user's ratings history and taste feedback.
- Records are keyed by fingerprint, so renamed files keep their scores. The rating writer updates the file's size in `files` after each write, so our own writes don't trigger re-fingerprinting.
- `Analyzer::preload` fills the ScoreBoard from the DB when a folder opens, so saved sort/filter settings apply immediately. `ScoreBoard::version` only changes on real changes; the toolbar offers "Refresh order" when scores arrived after the view was built.

### Start-up (measured: photo on screen ~330 ms, first frame ~230 ms after `main`)

- `main` creates the `egui::Context` and the whole `CernoApp` **before** `run_native_ext`, so the start folder is scanned and the first photo decodes while winit and wgpu set up (textures uploaded meanwhile are queued by egui). eframe's `AppCreator` only applies the theme and hands the app over.
- Before the first frame the loader decodes only the current photo (`prefetch: false`); neighbours would compete with the GPU set-up. `START_TARGET` (4K) is the decode size until the monitor is known; a larger monitor re-decodes (`Loader::set_target` drops too-small images).
- wgpu is restricted to one backend (DX12 on Windows, Vulkan + GL elsewhere) – probing all of them cost ~90 ms and opened an untitled OpenGL helper window. `WGPU_BACKEND` overrides.
- `start-up: …` log lines (info level) report app-ready, first frame and first photo; re-measure after changes that touch start-up.

### Compare mode and deletion

- `pinned` (a path) is the left photo, `current` the right one; `view::skip_pinned` keeps navigation off the pinned index. The loader keeps the pinned photo cached however far away it is and loads full resolution for both on-screen photos. Both sides share one `Zoom`, so they stay aligned.
- `A` deletes the right photo (next one moves in); `D` deletes the left one and the right photo becomes the new pinned one.
- `DeleteQueue`: deleted photos are hidden from the view immediately (`rebuild_view` filters `is_hidden`), each deletion restarts the 5 s countdown, `Esc` cancels the whole queue (Esc priority: deletions → zoom → compare → fullscreen → notice). When it runs out, a worker moves the batch to the trash (`trash` crate); failures reappear with a notice. Photos that really went to the trash are recorded as 0-star taste feedback (`Db::record_deletion`); cancelled ones are not. `on_exit` carries out a pending deletion that wasn't cancelled – after `writer.shutdown()`; keep that order, or a rating flushed on exit would hit a file that is already in the trash.

### Languages

- Every user-visible text lives in `i18n::Texts`; each language file is one full struct literal, so a forgotten text is a compile error. Texts with values are non-capturing closures coerced to `fn` pointers (word order per language, arguments type-checked). Add a text: field in `mod.rs` → all five files → `texts_show_their_values` if it takes values.
- The current language is a global atomic read via `i18n::t()` on every frame. It **starts as English** and only `CernoApp::new` applies the system/saved language, so tests never depend on the machine's locale. Tests must never call `i18n::set` (they run in parallel and `ui::details::tests` looks for "AESTHETICS"); test formatting through the `format_*` helpers that take `&Texts`.
- `metadata.rs` stays language-free (raw values: `taken` as `YYYY-MM-DD HH:MM`, `gps`, `digital_zoom`); the UI localizes them (`i18n::date`, `i18n::coordinates`, `Texts::digital_zoom`).
- Not translated: model/backend names (LAION, SigLIP, DirectML, CPU), file names, library error details (only the sentence around them).
- Wrapped text goes through `i18n::keep_together`, which glues " %", French " :" etc. with U+00A0 – egui never breaks there.

### Info bar

- Centre: stars, `AESTHETICS LAION / V2.5 / personal ★` (value only, "–" when missing) and the sharpness meter. The centre width is measured from the laid-out meters, so longer labels (French, Italian) push the side columns instead of overlapping them.
- Right: exposure line (+ EXIF digital zoom) and camera/lens; when they don't fit, whole parts are left out (focal length first, lens first) instead of clipping mid-word. Then the buttons: map pin (only with GPS; opens Google Maps via `ctx.open_url`, i.e. the default browser), language flag, help, the three panel toggles.
- Left: name, position, viewer zoom (while zoomed), capture date, size, decode time.
- The mouse wheel over the filmstrip steps through the photos (one per notch, touchpads per 50 pt); over the photo it zooms.

### Rating writes

- The UI updates immediately (session map `path → rating`); the write is queued.
- The writer debounces per file (pressing 3 then 4 quickly = one write) and skips writes that don't change the value read from the file.
- Only `XMP-xmp:Rating` is written, plus the Microsoft rating tags (`EXIF:Rating`/`RatingPercent`, `XMP-microsoft:RatingPercent`) **only if the file already has them**, so Windows Explorer never shows a stale value. `0` deletes the tags.
- `on_exit` flushes pending writes and joins the writer thread – a rating set just before closing must not be lost.

## Roadmap

1. ✅ Viewer, prefetch, star rating, timestamp-preserving writes.
2. ✅ HEIC (Windows verified with a real sample incl. rating round trip), filmstrip, 100 % zoom with tiles, SQLite index, sharpness, aesthetics on DirectML, sort/filter, camera/exposure info.
3. ✅ Analysis modules: V2.5, personal taste model, CLIP attributes, exposure, eye sharpness; details panel.
4. Host SigLIP vision + V2.5 head in the user's Hugging Face repo and add the download button (like CLIP).
5. Verify face detection / eye sharpness on real portraits (so far only unit tests and a run on photos without faces).
6. Linux verification (build, libheif, WebGPU on AMD/Vulkan).
7. Packaging with `cargo-packager` (`.msi`/NSIS, `.deb`, `.AppImage`) + updater; ship `DirectML.dll`; resolve the libde265 LGPL question first (see Gotchas).

## Gotchas

- **EXIF orientation applies to JPEG only.** For HEIC, libheif already applies the `irot`/`imir` transforms; applying EXIF orientation again would double-rotate.
- ExifTool must get `-charset filename=UTF8`, otherwise paths with umlauts fail on Windows. (Same trap when checking files by hand from PowerShell: pass the folder, not the umlaut file name; and quote `"-FNumber=2.8"` – PowerShell splits unquoted `-x=2.8`.)
- ExifTool's `-P` shifts mtime and creation time by a few **microseconds** (Perl floats). `filetimes::Snapshot::restore` puts the exact values back after every write – don't remove it just because `-P` "already preserves dates".
- **In-place writes are deliberately not atomic.** `-overwrite_original_in_place` copies the new bytes back into the original file so its identity (creation date, inode) survives – the user's "dates must not change" requirement wins over temp-file + rename. Trade-off: a crash during the copy-back can corrupt that one file.
- **Torn reads are possible:** if the loader reads a file while ExifTool copies bytes back into it (rate, navigate away and back within the debounce window), the decode fails and the slot shows an error until it is evicted and re-decoded. Rare and self-healing – keep the eviction, don't cache failures forever.
- **Fingerprints depend on the decoder and resizer.** Upgrading zune-jpeg, libheif or fast_image_resize can change them; that only means a re-analysis, no data loss. Never use `std::hash::DefaultHasher` for them (not stable across Rust versions).
- DirectML is registered with `error_on_failure()` and falls back to a CPU session explicitly, so the toolbar can show the real backend. It requires `with_memory_pattern(false)`.
- The ONNX output is picked by shape (the `[batch, 768]` / `[batch, 1152]` one), not by name. SigLIP's `image_embeds` are already L2-normalised; its preprocessing is a plain squash to 384 px with `v/127.5 − 1`, CLIP's is short side 224 + centre crop + CLIP mean/std.
- The V2.5 checkpoint stores BFloat16 tensors under `scoring_head.*`; `tools/make_aesthetic_head.py` unpickles Float/Half/BFloat16 without torch and finds the linear layers by shape.
- YuNet wants 640 × 640 **BGR, 0–255, no normalisation** (letterboxed); score = √(cls · obj). It runs on the CPU on purpose (a few ms; the GPU is busy with the big models).
- Exposure counts a pixel as blown only when **all** channels are ≥ 250 – a saturated blue sky has B = 255 and must not count (v1 did, and flagged 13 % on a normal landscape).
- Scripted smoke tests can't use the mouse: posted `WM_MOUSEMOVE`/`WM_MOUSEWHEEL` had no effect in testing (not even wheel-zoom over the photo), while posted keys work. Test pointer behaviour headless (`Context::run_ui` with `RawInput` events, see `ui::details::tests`); remember `textures_delta.clear()` or egui panics on drop.
- The index at `%LOCALAPPDATA%\Cerno\data\cerno.db` is the user's live data (ratings history, taste feedback). Never delete it to "start clean" – DB tests use in-memory databases.
- zune-jpeg decodes truncated JPEGs leniently (missing part grey) instead of failing – intended, other viewers do the same.
- `libheif[core]` is linked statically, which pulls in libde265 (**LGPL-3.0**). Before publishing binaries, switch to the dynamic triplet (`x64-windows` + `VCPKGRS_DYNAMIC=1`) and ship the DLLs, or otherwise satisfy the LGPL relinking terms. The x265 encoder (GPL) is deliberately excluded.
- `rfd` dialogs (folder picker, model download confirmation) block the UI thread while open – fine for modal dialogs.
- The help page is modal: `handle_keys` returns early while it is open (only `H`/`F1`/`Esc` and `L` work) and the photo's mouse handling is skipped. It is an `egui::Area` in `Order::Foreground`, so the details panel's scroll area below doesn't react either. Its card height is the content height remembered from the previous frame – not `ScrollArea`'s `content_size`, which with `auto_shrink(false)` is at least the visible area.
- Clicking the map pin sends the photo's coordinates to Google. Don't click it in scripted tests; `metadata::maps_url` is unit-tested.
- Linux always updates `ctime` on write; that can't be prevented and no photo tool shows it. `mtime` and (on Windows) the creation time are preserved.
- Because mtime and often the file size (XMP padding) stay the same, backup/sync tools that only compare size + date (e.g. `rsync` without `-c`) may miss rating changes.
- No colour management yet: embedded ICC profiles (e.g. Adobe RGB) are ignored.
- Smoke-testing the window from scripts: send keys with `PostMessage(WM_KEYDOWN/UP)` to Cerno's window handle and capture with `PrintWindow(PW_RENDERFULLCONTENT)`. Never use `SendKeys` – without focus it types into whatever window the user is working in. Wait for a window whose title contains "Cerno": `Process.MainWindowHandle` can briefly point at wgpu's untitled helper window.
- Deletion tests must not touch the real trash: `DeleteQueue` takes a `Remover` function, tests pass a recorder.
