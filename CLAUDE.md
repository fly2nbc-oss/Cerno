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

On Windows, `cargo` lives in `%USERPROFILE%\.cargo\bin` (rustup default location). The release build puts `DirectML.dll` next to `target\release\cerno.exe` (ort's `copy-dylibs`). A `--features heic` build also puts `heif.dll`, `libde265.dll` and a `licenses/` folder there (`build.rs`). The exe needs those files beside it when copied elsewhere. On Windows `build.rs` also embeds `assets/icon.ico` and the version info into the exe (`winresource`, which runs the Windows SDK's `rc.exe`).

Unit tests live in-module (`#[cfg(test)]`). `rating::tests::writes_stars_and_keeps_file_dates` is a real round trip through ExifTool (umlaut file name, mtime + creation time compared) and silently skips when ExifTool isn't on `PATH` – make sure it actually ran before trusting a change to the write path. Fixture: `tests/fixtures/tiny.jpg`.

HEIC on Windows needs libheif from vcpkg in `target/vcpkg` (dynamic triplet `x64-windows`). `.cargo/config.toml` sets `VCPKGRS_DYNAMIC=1`; without it the `vcpkg` crate links statically and `build.rs` aborts. libheif-sys does not notice that variable on its own: a tree already built against `x64-windows-static-md` keeps the static link until `cargo clean -p libheif-sys`. `cargo vcpkg build` clones the pinned vcpkg revision but its bootstrap step fails on current Rust (it spawns `bootstrap-vcpkg.bat` by relative name), so finish by hand:

```bash
cargo install cargo-vcpkg && cargo vcpkg build     # clones target/vcpkg, then fails at bootstrap
target\vcpkg\bootstrap-vcpkg.bat -disableMetrics
target\vcpkg\vcpkg.exe install "libheif[core]:x64-windows"
cargo clean -p libheif-sys
cargo build --release --features heic
```

Packages (what CI's `package-windows` / `package-linux` jobs run, after the HEIC release build):

```bash
pwsh packaging/windows/build.ps1   # dist/windows: Cerno\ folder, cerno_<v>_x64-portable.zip, cerno_<v>_x64-setup.exe
packaging/linux/build.sh           # dist/linux: cerno_<v>_amd64.deb, cerno_<v>_x86_64.AppImage (Ubuntu 24.04 only)
```

`build.ps1` needs `cargo install cargo-packager --version 0.11.8 --locked` and a Visual Studio with the C++ tools; Build Tools installs have no `VC\Redist` folder, so pass `-CrtDir` with a folder holding `msvcp140*.dll` / `vcruntime140*.dll`. `build.sh` needs `cargo-deb`, `patchelf`, `libheif-plugin-libde265`. `packaging/windows/test-installer.ps1` installs over and uninstalls `%LOCALAPPDATA%\Cerno` – CI only, never on the author's machine.

## Product decisions (settled – build on them, don't re-propose alternatives)

| Decision | Why |
|---|---|
| Pure Rust + egui/eframe (wgpu renderer) instead of Tauri | WebViews can't display HEIC (Rust would have to decode *and* re-encode), WebKitGTK is slow on Linux. Decode once → GPU texture. |
| Formats: **JPEG + HEIC** | User requirement. HEIC via `libheif-rs` (vcpkg on Windows, system libheif ≥ 1.17 on Linux), behind the `heic` feature. |
| Only the **rating** and the **colour label** are written into the original file (`xmp:Rating`: 1–5 stars, or **-1 = rejected**; `xmp:Label`: the English names `Red` / `Yellow` / `Green` / `Blue` / `Purple`) | Everything else (scores, thumbnails, CLIP embeddings, capture time) lives in a central SQLite DB under the app data dir – never next to or inside the photos. AI scores never set stars or labels. An unknown label string already in the file is left alone until the user sets a Cerno colour. |
| The one exception: **straighten, crop and quarter turns** change the file – and **keep the original** | The writer first copies the file into `data/backups` (index table `backups`); without that copy the edit does not happen. `Ctrl+Z` writes the newest copy of the current photo back in place (same file, same dates), then the rating and label the file has now, so later marks stay; pressed again, the copy before. Copies are deleted after 30 days (`backup::KEEP`) or when the photo really goes to the trash (`backup::discard` – no private copies left where nobody expects them), and follow a moved photo. Only files directly in `data/backups` are read, written back or deleted (`backup::inside`); a row pointing anywhere else is dropped. Quarter turns only change the EXIF orientation; straighten and crop re-encode at quality 95, 4:4:4, in the file's own colour space with its ICC profile. |
| **File dates must never change** | ExifTool runs with `-P -overwrite_original_in_place`, and `filetimes.rs` snapshots modified/created time before each write and restores them afterwards. |
| GPU inference must work on **all vendors, AMD has priority** | Windows: ONNX Runtime + DirectML (any DX12 GPU). Linux: CPU, ORT WebGPU (Vulkan) behind the experimental `webgpu` feature. No CUDA/ROCm-only paths. Same model everywhere so scores stay comparable. |
| Aesthetics = CLIP ViT-L/14 + LAION improved aesthetic predictor | Established, local, 1–10 scale. The LAION MLP has no activations and is collapsed into one linear layer (`tools/make_aesthetic_head.py` → `src/analysis/aesthetic_head.bin`, 3 KB, embedded). The 1.2 GB ONNX vision model is downloaded on request into `%LOCALAPPDATA%\Cerno\data\models` (`~/.local/share/cerno/models`), from a URL pinned to a commit (not `main`) and checked by size and SHA-256. |
| Second aesthetics score: SigLIP so400m + **Aesthetic Predictor V2.5** | Newer and better on real photos than LAION. The V2.5 head is **AGPL-3.0**, so it is never embedded or committed: `tools/extract_siglip_vision.py` cuts the vision tower out of onnx-community's combined SigLIP export (1.7 GB), `tools/make_aesthetic_head.py` collapses the head (1153 floats). Distribution plan: publish both files once in the user's own Hugging Face repo (AGPL notice + source link for the head) and let Cerno download them like CLIP – the app never converts models itself. Until that repo exists there is no download button; the files are dropped into the models dir and detected at start. |
| Only the **six modules** below, all local | LAION, V2.5, personal taste, CLIP attributes, exposure, eye sharpness. They share the decoded 2048 px image and the stored embeddings; adding a module means a DB column + version constant + `is_complete` check. |
| For you = ridge regression on the stored CLIP embeddings | Shown as „Für dich“ / “For you” / „Pour vous“ / „Para ti“ / „Per te“, never as taste or aesthetics. The user's stars (1–5) are the labels, deleted photos count as 0 (`feedback` table – the file row is gone, the embedding stays). Trained in the background 2 s after ratings/deletions change, from 15 examples; 5-fold CV picks λ and reports the typical error in stars. AI scores still never set stars. |
| Eye sharpness via YuNet (OpenCV Zoo, MIT, 230 KB, embedded) | Shallow depth of field: a sharp face in a blurred background would otherwise rank low. When a face ≥ 40 px is found, the "subject" sharpness (info bar, filmstrip marker, Hide blurry, sort) uses the eye percentile instead of the whole-frame one. |
| Design system `ui_design_system_v1.2.md` (see `../MediaFileRenamer/docs/`) | Dark token *structure*, accent `#5B8EC4`, status colours, Segoe UI loaded from the system (egui's bundled font lacks ← →). **Deliberate deviation: all surfaces are neutral grey** (`#141414`/`#202020`/`#2A2A2A`, canvas `#161616`) instead of the system's blue-grey – tinted chrome around a photo biases colour judgement. Only accent and status colours carry hue, **plus the five label colours** (a dot in the info bar and a stripe on the filmstrip cell – the photo's own mark, not chrome). Icons (flags, menu ticks and boxes, the blurry mark, the outline star) are painted in `ui/icons.rs` / in place – egui has no flag emoji, no Lucide, and Segoe UI lacks ✓. Font sizes only from `theme::text` (six roles: 11 / 12 / 13 / 15 / 17 / 22). The accent marks selection, interaction and progress only – section titles and the series line are muted. Label blue is `#3366E6` (CIEDE2000 13.5 from the accent). |
| UI in **German, English, French, Spanish, Italian** | User requirement. Starts in the system language (`sys-locale`, fallback English), `Ctrl+L` cycles, the choice is saved. The flag appears **only while switching** (fade-in); the help header shows the language as a word. Numbers keep the decimal point in every language ("6.1" next to "f/2.8" – a comma would mix notations). |
| **The info bar always shows**; filter bar, details panel and filmstrip are optional | Default: photo, filmstrip and info bar only. The menu (button at the bottom right, `Ctrl+K`) and `T` / `Tab` / `F6` (`Shift+Tab` all three) toggle them; states are saved (`top_bar`, `details_mode`, `filmstrip` – the pre-0.8 keys `toolbar`/`details` are ignored so the new default applied once). The filter bar also appears when a filter hides every photo, so the filter can be changed back. |
| **Keyboard-first, Cerno's own key layout** – **not a Lightroom clone** | The user does not want to rebuild Lightroom. Keys and features are chosen for culling in Cerno, never named after Lightroom or justified by "Lightroom has it", and user-facing texts (README, help, UI, new changelog entries) don't advertise "Lightroom-style" anything. Lightroom is named only for compatibility (it reads `xmp:Rating` / `xmp:Label`; its localised label names are recognised). The layout: `Tab` details panel, `Shift+Tab` all panels, `T` filter bar, `F` and `F11` full screen, `F6` filmstrip, `I` expands or collapses every explanation, `X` = reject (toggles; `Shift+X` rejects and moves on), `6`–`9` = red/yellow/green/blue label (again clears; `Shift+6`–`9` sets and moves on; purple is menu-only), `Delete` = delete, `0`–`5` stars (`Shift+0…5` rate and move on), `Z` zoom, `C` compare; `S` straighten, `R` crop, `Ctrl+←/→` quarter turn, `Ctrl+Z` undo an edit; `?`/`H`/`F1` help; `Ctrl+K` opens the menu (view, sort, filter, edit, photo, labels, models & data, language, help) with the shortcut on each row – arrows, Enter, `→`/`←` for submenus, a letter jumps, Esc; `Ctrl+M` the action menu (copy, move, delete what the filter shows); `Ctrl+L` language. `Space` next, `Backspace` previous. Auto-advance is a saved menu switch, not Caps Lock – egui does not expose that key reliably. |
| Three scores in the info bar, **all on the star scale** | `L 3.4 / V 3.8 / ☆ 2.4`, no group label – L and V are aesthetics (LAION / V2.5), the **outline** star (painted, not a glyph) is **For you** (German „Für dich“: the stars Cerno thinks the user would give); filled stars are only ever the user's own rating. LAION and V2.5 are *displayed* via `aesthetic::as_stars` (2–8 → 0–5, clamped – the range the bars always used); the DB keeps the raw 1–10 scores and sorting is unchanged. For you is its own details section, not grouped under Ästhetik. |
| **Subfolders stay off** until the user turns them on | A card or a year folder must not be scanned and analysed by accident. The palette command reopens the current folder. Hidden directories (name starts with `.`) are skipped and directory symlinks are not followed. |
| **Series** are time groups, not collapsed stacks | Consecutive photos at most 2 s apart (`SERIES_GAP_MS`) form a series. Capture-time sort puts the sharpest non-rejected photo first inside the series. **Nothing is folded away**: "Best of each series" (0.9.0, one photo per series with `+n`) was removed – the next frame slipped into a rejected photo's place unnoticed, so after "Delete rejected photos" the look-alike frames seemed to have come back; the stale `best_of_series` setting is ignored. Photos without a capture time are never in a series. **One camera per series**: only photos of the same camera model (`CameraInfo::model` – EXIF model, else make; `images.camera`, `metadata::camera_id` in memory) form a series, so two cameras firing together make two; photos without camera data only group with each other. The model alone, not make + model: some DJI files lack the make. Two bodies of the same model still share series – phones write no serial number. |
| **Duplicates are exact and only marked** | Same fingerprint (FNV-1a of the 256 px thumbnail pixels plus the original size). The original is the file whose name the others only extend (`IMG_1.jpg` for `IMG_1 - Kopie.jpg` / `IMG_1 (1).jpg`, which sort *before* it), else the only one with stars, a rejection or a label, else the first in folder order (`view::pick_original`); the others are "duplicate of …". Nothing is rejected or deleted automatically. Near-duplicates (CLIP similarity) are not duplicates. |
| **"Probably blurry"** needs the folder's blurriest 20 % **and** an absolute ceiling | Frame 250, eyes 60 (`view::BLURRY_FRAME_MAX` / `BLURRY_EYES_MAX`): the 10th percentile of the author's index on 2026-09-28. A rank alone warned on every fifth photo even in a folder of sharp ones; the same rule drives the filmstrip mark, the info bar, the details and the "Blurry" filter. |
| **No system dialogs** except the folder picker | Confirmations (model download, reset For you, delete models) are an in-app card (`ui/confirm.rs`, Enter/Esc). Copy/move of the selection ask nothing (choosing the folder confirms), deleting uses the countdown like `Delete`. The CLIP download is offered as a one-time hint once a folder is open, never as a dialog at start. Hints fade after ~5 s (longer texts a little longer), errors stay until Esc or a click. |
| Version **0.7.0** is the first numbered one | `Cargo.toml` is the only source; the help page and start screen show `CARGO_PKG_VERSION`. |
| **Four packages**, all with HEIC: Windows NSIS installer + portable zip, Linux AppImage + `.deb` | Built by CI on every push to `main` (artifacts) and drafted as a GitHub release for a `v*` tag (tag must equal the `Cargo.toml` version; `SHA256SUMS.txt`, notes from the CHANGELOG section). The installer is per user (`currentUser`, no UAC) in five languages; the zip is the same folder. Linux is built on Ubuntu 24.04 – the oldest base with libheif ≥ 1.17 – so the AppImage needs glibc 2.39+. Tools: cargo-packager (NSIS only), cargo-deb (`$auto` dependencies, `Recommends`), linuxdeploy + appimagetool with our own `AppRun`. Not signed yet; no updater. |

## Architecture

```
main.rs              eframe bootstrap (wgpu renderer), CLI path argument
app/mod.rs           CernoApp: the state, start-up, one frame (`ui`), exit; each submodule adds the methods for one part:
app/browse.rs        opening a folder, building the view (`rebuild_view` / `set_view`, `refresh_marks`), navigation
app/marks.rs         stars, rejection, colour labels (session maps, then the writer)
app/files.rs         copy / move of the view, deletion with the countdown
app/editing.rs       straighten / crop sessions, quarter turns, Ctrl+Z, the edit overlay
app/gate.rs          one action at a time on a photo (`blocked`, `allowed`, `menu_block`)
app/keys.rs          `read_keys` (one frame's input → `KeyInput`, pure, tested) and `handle_keys`
app/photos.rs        photo slots, compare mode, mouse zoom / pan, drawing the photos
app/frame.rs         `Layout` of the bars and the photo area, filmstrip, info bar, filter bar
app/menu.rs          burger and action menu entries, help, models card, confirmations (`draw_overlays`)
app/panels.rs        which bars show, language switch and flag
app/notice.rs        messages over the photo and the deletion countdown
view.rs              sorting + filtering, time series and exact duplicates (`View`, pure, unit-tested)
ui/viewer.rs         fit / zoom / pan geometry and drawing (display texture or full-res tiles)
ui/filmstrip.rs      thumbnail strip centred on the current photo; marks explained in one tooltip
ui/filter_bar.rs     filter bar (sort, filter boxes, colour squares, status, Action)
ui/info_bar.rs       two-row info bar (meters, zoom, help and menu)
ui/overlays.rs       over the photo area: compare labels, deletion countdown, notices, drop hint, language flash
ui/details.rs        side panel with the current photo's values (and its size / load time / GPS position with Google Maps and OpenStreetMap links); rows fold open, `I` all; scrolls
ui/help.rs           help page (`H`/`F1`/`?`, modal foreground area) and the small start screen (five keys, H for the rest)
ui/palette.rs        the menus: burger (`Ctrl+K`) and action menu (`Ctrl+M`) – groups, switches/choices, keyboard
ui/modal.rs          card over the dimmed window (height remembered from the last frame)
ui/confirm.rs        confirmation card (Enter / Esc) instead of system message boxes
ui/models.rs         "Models & data": model states, models folder, download / reset For you / delete models
ui/edit.rs           straighten grid, crop frame, banners
ui/icons.rs          painted flags, menu, help, copy and warning icons
ui/stars.rs          star shapes
i18n/                `Texts` struct (mod.rs) + one full literal per language (de/en/fr/es/it), date and coordinate formatting
library.rs           folder scan (optional subfolders), supported extensions, natural accent-insensitive order
loader.rs            prefetch worker pool + texture cache; full-resolution tiles for zoom
decode.rs            bytes → RGB8 at a target size: JPEG (zune-jpeg) / HEIC (libheif), resize, EXIF orientation
metadata.rs          rating, colour label, capture time (`taken_ms`), orientation, camera model (series), camera/exposure data from the in-memory file bytes
analysis/mod.rs      background analysis (only the missing parts per image) + ScoreBoard + taste trainer thread
analysis/models.rs   `ModelState`, lazily loaded model slots (`run_model`), CLIP download (stall timeout), "Delete models"
analysis/sharpness.rs  tile-based Laplacian variance, region variance, percentile helper
analysis/aesthetic.rs  ONNX encoders (DirectML → CPU): CLIP + embedded LAION head, SigLIP + V2.5 head from the models dir
analysis/attributes.rs CLIP-IQA-style zero-shot attributes from the embedding (embedded prompt vectors)
analysis/exposure.rs   share of blown highlights / crushed shadows
analysis/faces.rs      YuNet face detection (CPU, embedded model) + sharpness around the eyes
analysis/taste.rs      ridge regression (Cholesky, k-fold CV) from embeddings to the user's stars
thumbs.rs            filmstrip textures from loader / analysis / database; at most 400, the least recently drawn go first
histogram.rs         RGB histogram of the display image
edit.rs              straighten / crop geometry and the pixel work that writes a new JPEG
backup.rs            originals kept before an edit (data/backups, 30 days or until the photo is deleted) for Ctrl+Z
transfer.rs          copy / move of the photos the filter shows (background thread)
deletion.rs          delayed deletion queue (countdown, undo, trash worker)
db.rs                SQLite index: files (path+stamp → fingerprint, rating, label), images (scores, thumbnail, embedding, taken_ms, camera, metadata_version), feedback (deletions), backups (kept originals), settings; additive migration
tools/*.py           one-off model preparation (collapse heads, extract the SigLIP tower, CLIP prompt vectors); `i18n_edit.py` for texts; `make_icon.py` for the app icon
assets/              app icon: `icon-source.jpg` (image-model output) → `tools/make_icon.py` → `icon.png` (256 px, window and taskbar, `main.rs`) and `icon.ico` (16–256 px, the exe, `build.rs`)
rating.rs            debounced background writer for marks (rating and colour label), quarter turns, pixel edits and Ctrl+Z restores; one long-lived ExifTool process (-stay_open)
exiftool.rs          ExifTool stay-open protocol
filelock.rs          who reads or writes which photo right now (holds around file I/O, write generations)
filetimes.rs         snapshot / restore of file timestamps
paths.rs             data, model and database locations
theme.rs             design tokens → egui Visuals, `text` font sizes, system UI font
packaging/windows/   build.ps1 (portable folder + VC++ runtime + import check, zip, NSIS), test-installer.ps1, German/Italian installer texts
packaging/linux/     build.sh (.deb, AppImage), AppRun, cerno.desktop
```

### Loader (the speed-critical part)

- Workers (`available_parallelism - 1`, clamped 2..=4) pick the **most urgent** missing index relative to the *current* index at pick time: `0, full-res of 0 (if zoomed), +1, -1, +2, +3, -2, -3`. Jumping around re-prioritises automatically; nothing is queued ahead of time.
- Display images are decoded at **monitor resolution** (clamped to `max_texture_side`). Full resolution is loaded only for the current photo while zoomed, split into tiles ≤ 4096 px.
- Workers upload textures themselves (`egui::Context` is `Send + Sync`). They also downscale the decoded image into the filmstrip thumbnail – the neighbourhood's thumbnails are free.
- `set_library` carries cached images over by path, so re-sorting or filtering decodes nothing again. A generation counter discards results that finish after the list changed; the same list again (compare mode on or off) keeps the decodes in flight (`State::switch`).

### Analysis

- Two workers, nearest-first over the **whole folder** (not the filtered view), paused for 0.9 s after every navigation so display decoding wins.
- Fast path: `stat` + one query; if `is_complete` holds (thumbnail, sharpness/exposure of their current `VERSION`, faces when the detector loads, LAION/V2.5 of the current model id when that model is available, **and** `metadata_version == metadata::VERSION`) nothing is decoded. A model that failed to load is never waited for (`Capabilities`), or every photo would decode on every visit.
- Otherwise decode at 2048 px once and compute **only what is missing**: thumbnail (256 px JPEG in the DB) → **fingerprint = FNV-1a over thumbnail pixels + full size** → sharpness → exposure → faces + eye sharpness → CLIP (LAION score + embedding) → SigLIP (V2.5). Bumping one module's version re-runs just that module. Each model has its own lazily loaded session (`Slot`) and `ModelState` for the UI.
- Metadata fast path: when the scores are already complete and only `metadata_version` is stale, the file is read and `metadata::read` runs – no 2048 px decode. That fills `taken_ms` and `camera` and refreshes the file's rating and label (`metadata::VERSION` 2 added the camera: every photo is read once more after the update, without a decode). `put_metadata` updates the existing images row.
- Taste trainer: a separate thread retrains from `Db::taste_examples` 2 s after `taste_changed()` (rating set, deletion carried out). Predictions come from the in-memory embeddings map, so `Analyzer::personal` is cheap per frame.
- DB migration is additive: `migrate()` reads `PRAGMA table_info` for `images` and `files` and `ALTER TABLE … ADD COLUMN`s what is missing. Never drop or recreate tables – the index holds the user's ratings history and taste feedback.
- Records are keyed by fingerprint, so renamed files keep their scores. The mark writer updates the file's size in `files` after each write, so our own writes don't trigger re-fingerprinting.
- `Analyzer::preload` fills the ScoreBoard from the DB when a folder opens, so saved sort/filter settings apply immediately. `ScoreBoard::version` only changes on real changes; with a score-dependent sort or filter the filter bar offers "Refresh order" when scores arrived after the view was built. In name order without filters the marks refresh themselves, at most every 2 s (`refresh_marks`) – nothing moves, but duplicate marks and series need the new fingerprints and capture times. With the same photos in the same order only `series` and `duplicate_of` are swapped: a full `rebuild_view` restarts the loader (decodes in flight are discarded) and pauses the analysis.

### Start-up (measured: photo on screen ~330 ms, first frame ~230 ms after `main`)

- `main` creates the `egui::Context` and the whole `CernoApp` **before** `run_native_ext`, so the start folder is scanned and the first photo decodes while winit and wgpu set up (textures uploaded meanwhile are queued by egui). eframe's `AppCreator` only applies the theme and hands the app over.
- Before the first frame the loader decodes only the current photo (`prefetch: false`); neighbours would compete with the GPU set-up. `START_TARGET` (4K) is the decode size until the monitor is known; a larger monitor re-decodes (`Loader::set_target` drops too-small images).
- wgpu is restricted to one backend (DX12 on Windows, Vulkan + GL elsewhere) – probing all of them cost ~90 ms and opened an untitled OpenGL helper window. `WGPU_BACKEND` overrides.
- `start-up: …` log lines (info level) report app-ready, first frame and first photo; re-measure after changes that touch start-up.

### Compare mode and deletion

- `pinned` (a path) is the left photo, `current` the right one; `view::skip_pinned` keeps navigation off the pinned index. The loader keeps the pinned photo cached however far away it is and loads full resolution for both on-screen photos. Both sides share one `Zoom`, so they stay aligned.
- `A` **rejects** the right photo, `D` the left one; then compare mode ends and the kept photo is shown alone (`photos::choose` clears `pinned` before rebuilding on the winner; a loser that takes no mark right now – `allowed` – keeps the comparison open). Nothing is deleted in compare mode – rejects stay visible (dimmed in the filmstrip) until the menu's "Delete rejected photos (n)" (Photo) sends them through the normal `DeleteQueue`.
- `metadata::Rating` (`Unrated` / `Rejected` / `Stars(1..=5)`) is the one rating type everywhere (file, index, session map, view, UI). The index stores the file's value (NULL, -1, 1–5); rejects are taste examples with label 0, like deletions. Sorting by rating puts rejects after unrated photos; filter "Rejected" shows only them.
- `DeleteQueue`: deleted photos are hidden from the view immediately (`rebuild_view` filters `is_hidden`), each deletion restarts the 5 s countdown, `Esc` cancels the whole queue (Esc priority: deletions → zoom → compare → fullscreen → notice). When it runs out, a worker moves the batch to the trash (`trash` crate); failures reappear with a notice. Photos that really went to the trash are recorded as 0-star taste feedback (`Db::record_deletion`); cancelled ones are not. `on_exit` carries out a pending deletion that wasn't cancelled – after `writer.shutdown()`; keep that order, or a rating flushed on exit would hit a file that is already in the trash.

### One action at a time

- `app::gate::blocked(change, activity)` is pure and unit-tested. Every action that changes a photo asks `allowed()` first (a hint says why not); menu rows ask `menu_block()` and are greyed out with the reason as tooltip. A new action that changes a photo must go through `allowed`.
- Rules: a photo being moved takes nothing; one being copied still takes marks (the file lock keeps the rating write and the copy apart); an edit in the writer (`edit_busy`) blocks everything but marks; an open straighten/crop session blocks quarter turns, `Ctrl+Z` and copy/move until `Enter` or `Esc`.
- An `EditSession` remembers its path. `set_view` ends the session (hint) when another photo becomes current, and `confirm_edit` checks the path again.
- `filelock::FileLocks` is shared by the writer, the loader, the analysis, the edit render and the copy/move worker. Each holds a path only around the I/O itself (one read, one ExifTool call, one copy – never a decode) and never two paths at once. The UI thread never waits there.
- The analysis puts a photo back (`Outcome::Retry`, again after 1 s, at most 10 times) when a mark for it is queued or being written, or when its write generation changed while it ran – size and dates can stay the same after a rating, so the stamp alone doesn't tell.

### Languages

- Every user-visible text lives in `i18n::Texts`; each language file is one full struct literal, so a forgotten text is a compile error. Texts with values are non-capturing closures coerced to `fn` pointers (word order per language, arguments type-checked). Add a text: field in `mod.rs` → all five files → `texts_show_their_values` if it takes values. `tools/i18n_edit.py spec.json` adds, replaces or removes a text in all six files at once (the spec format is in its docstring).
- The current language is a global atomic read via `i18n::t()` on every frame. It **starts as English** and only `CernoApp::new` applies the system/saved language, so tests never depend on the machine's locale. Tests must never call `i18n::set` (they run in parallel and `ui::details::tests` looks for "AESTHETICS"); test formatting through the `format_*` helpers that take `&Texts`.
- `metadata.rs` stays language-free (raw values: `taken` as `YYYY-MM-DD HH:MM`, `gps`, `digital_zoom`); the UI localizes them (`i18n::date`, `i18n::coordinates`, `Texts::digital_zoom`).
- Not translated: model/backend names (LAION, SigLIP, DirectML, CPU), file names, library error details (only the sentence around them).
- Wrapped text goes through `i18n::keep_together`, which glues " %", French " :" etc. with U+00A0 – egui never breaks there.

### Info bar

- Centre: stars, `L 6.1 / V 6.5 / ☆ 2.4` (LAION / V2.5 / For you; the letters are muted `Piece::Prefix`es of one `LayoutJob`, "–" when missing; the outline star is painted over a `Piece::Star` placeholder of non-breaking spaces, found with `Galley::pos_from_cursor`) and the sharpness meter. The centre is as wide as its widest variant in the language (`widest_centre`: blurry note, "Eyes"), so the side columns never move while browsing and longer labels (French, Italian) push them instead of overlapping.
- Right: exposure line (+ EXIF digital zoom) and camera/lens; when they don't fit, whole parts are left out (focal length first, lens first) instead of clipping mid-word. Then the 32 px buttons: help, menu (tooltips carry the keys). The GPS position is not here but in Details › File (`details::map_links`: `egui::Hyperlink`s to Google Maps and OpenStreetMap, opened in the default browser).
- Left: name (relative, `100CANON/IMG_0001.JPG`, when the file is in a subfolder), then position, "Auto advance" while on, viewer zoom (while zoomed), series position (`Series 3 / 7`), "duplicate of …" and the capture date – whole parts are left out from the end when they don't fit (`fit_parts`). Size and load time are in the details panel. A colour dot sits just left of the stars.
- The mouse wheel over the filmstrip steps through the photos (one per notch, touchpads per 50 pt); over the photo it zooms. In capture-time order neighbouring series get a wider gap; the current series has a muted line under its cells (the accent frame is the current photo alone). A duplicate draws the copy icon; one tooltip per cell names every mark (blurry, rejected, label, duplicate of).

### View

- `view::build` returns a `View`: the visible paths, a `SeriesPlace` per path (`None` when the photo is not in a series), the original path of each duplicate, and `grouped` (true only for capture-time sort, which is the only order that keeps a series together).
- Capture time is ascending; photos without a time come last. Inside a series the order is subject sharpness descending, rejected last, not-yet-measured last. Other sorts stay descending with missing values last.
- Session maps (`session_ratings`, `session_labels`) override the index until the writer has flushed. A mark remembers the next path *before* the rebuild, then steps there when the photo leaves the view or when advance was requested – index+1 after a capture-time reshuffle would skip the wrong photo.

### Keyboard details

- `Tab` never reaches egui: `raw_input_hook` removes it and queues it for `handle_keys`. egui would otherwise move keyboard focus to the next widget with `Tab`, and `Space` ("next photo") would then also click that widget.
- Key tests run `read_keys` in a headless frame (`app::keys::tests::read`). egui takes `InputState::modifiers` from `Event::ModifiersChanged`, not from the key event, so the helper sends both.
- Digits – plain and with Shift – are recognised by the **physical** key (`app::keys::digit_key`, `Event::Key::physical_key`; the logical key only when there is none): with Shift the logical key is `!`, `"`, `§` … depending on the layout, and on AZERTY even the plain keys type `&`, `é`, `'`, `-` …. On German layouts `Shift+0` types `=`, which is also a zoom key – zoom-in is suppressed in a frame with a shifted digit; on AZERTY the 6 key types `-`, so zoom-out is suppressed in a frame with a colour digit. Key repeats don't rate.
- `Ctrl+←/→` turn the photo; next/previous ignore Ctrl (they used to step as well, so a following `Ctrl+Z` looked at the neighbour).
- Texts show modifiers with the language's key names (`i18n::with_ctrl("K")` → `Strg+K` / `Ctrl+K`); help rows are literal per language.

### Mark writes

- The UI updates immediately (session maps `path → rating` and `path → label`); the write is queued.
- The writer debounces per file and merges a pending rating and a pending label into **one** ExifTool call. A value that already matches the file is left out of that call; if nothing changed, the dates are not touched.
- `XMP-xmp:Label` is written with the English name, or deleted (empty value) when the colour is cleared. Localised Lightroom names (`Rot`, `Rouge`, `Púrpura`, …) are recognised on read. Unknown text is not a Cerno colour and is overwritten only when the user sets one.
- Only `XMP-xmp:Rating` is written for stars, plus the Microsoft rating tags (`EXIF:Rating`/`RatingPercent`, `XMP-microsoft:RatingPercent`) **only if the file already has them**, so Windows Explorer never shows a stale value. `0` deletes the tags. Rejected writes `-1` into `xmp:Rating` and clears the Microsoft tags – they have no "rejected", and Explorer then shows no stars. Explorer ignores `xmp:Label`.
- `on_exit` flushes pending writes and joins the writer thread – a rating or label set just before closing must not be lost.

## Roadmap

1. ✅ Viewer, prefetch, star rating, timestamp-preserving writes.
2. ✅ HEIC (Windows verified with a real sample incl. rating round trip), filmstrip, 100 % zoom with tiles, SQLite index, sharpness, aesthetics on DirectML, sort/filter, camera/exposure info.
3. ✅ Analysis modules: V2.5, personal taste model, CLIP attributes, exposure, eye sharpness; details panel.
4. ✅ Selection: sort by capture time, colour labels, time series with the sharpest first, optional subfolders, exact duplicate marks, auto-advance.
5. Host SigLIP vision + V2.5 head in the user's own Hugging Face repo and add the download button (like CLIP: URL pinned to a commit, size and SHA-256 checked before the file is used).
6. Verify face detection / eye sharpness on real portraits (so far only unit tests and a run on photos without faces).
7. Linux verification (build, libheif, WebGPU on AMD/Vulkan).
8. ✅ Packages in CI: NSIS installer, portable zip, AppImage, `.deb` (see `packaging/`); the icons are in `assets/` (`icon.ico` for the installer, `icon.png` for the `.desktop` entry – on Wayland the window icon comes from there via the app id `cerno`, not from `with_icon`). Open: updater, code signing. Before each release: `cargo audit` (on 2026-09-28 only `ttf-parser` "unmaintained" via egui, reads just the system font), check the vcpkg pin for libheif/libde265 fixes (they parse HEIC from untrusted sources, C code), and record which ONNX Runtime binary `ort`'s `download-binaries` fetched.

## Gotchas

- **EXIF orientation applies to JPEG only.** For HEIC, libheif already applies the `irot`/`imir` transforms; applying EXIF orientation again would double-rotate.
- `exiftool::locate` returns an absolute path (`CERNO_EXIFTOOL`, next to the exe, then only the *absolute* `PATH` entries) and `spawn` starts exactly that file. An empty `PATH` entry (`;;`, a trailing `;`) means the working directory – with a bare name `Command` would search on its own and could start another file than the one checked.
- ExifTool must get `-charset filename=UTF8`, otherwise paths with umlauts fail on Windows. (Same trap when checking files by hand from PowerShell: pass the folder, not the umlaut file name; and quote `"-FNumber=2.8"` – PowerShell splits unquoted `-x=2.8`.)
- ExifTool's `-P` shifts mtime and creation time by a few **microseconds** (Perl floats). `filetimes::Snapshot::restore` puts the exact values back after every write – don't remove it just because `-P` "already preserves dates".
- **In-place writes are deliberately not atomic.** `-overwrite_original_in_place` copies the new bytes back into the original file so its identity (creation date, inode) survives – the user's "dates must not change" requirement wins over temp-file + rename. Trade-off: a crash during the copy-back can corrupt that one file.
- **No torn reads:** the loader, the analysis, the edit render and the copy worker read a file only while holding it in `FileLocks`, and every write holds it too, so nobody sees a file while ExifTool copies bytes back into it. Another program can still rewrite a file under Cerno; a failed decode is evicted and decoded again – don't cache failures forever.
- **Fingerprints depend on the decoder and resizer.** Upgrading zune-jpeg, libheif or fast_image_resize can change them; that only means a re-analysis, no data loss. Never use `std::hash::DefaultHasher` for them (not stable across Rust versions).
- "Delete models" runs in the background (`remove_models`): the states go to `Removing` first, then both model slots are taken (waiting for a running inference) and the files deleted. `run_model` never loads a model whose state is `Missing` or `Removing`, so a worker with stale capabilities can't reopen the file.
- Model download: ureq 3 has no idle timeout (`timeout_recv_body` is the total). The socket is read on a helper thread and `receive` gives up after 60 s without data (`DOWNLOAD_STALL`); connect 30 s, response headers 60 s. A failed download removes its `.part`. `MODEL_URL` names a commit, never `main`: `download` checks `MODEL_BYTES` and `MODEL_SHA256` (hashed while streaming, `Hashing`) before the rename, since ONNX Runtime parses the file next. Another model file means new constants – and a new `MODEL_ID` if the scores change.
- DirectML is registered with `error_on_failure()` and falls back to a CPU session explicitly, so the toolbar can show the real backend. It requires `with_memory_pattern(false)`.
- The ONNX output is picked by shape (the `[batch, 768]` / `[batch, 1152]` one), not by name. SigLIP's `image_embeds` are already L2-normalised; its preprocessing is a plain squash to 384 px with `v/127.5 − 1`, CLIP's is short side 224 + centre crop + CLIP mean/std.
- The V2.5 checkpoint stores BFloat16 tensors under `scoring_head.*`; `tools/make_aesthetic_head.py` unpickles Float/Half/BFloat16 without torch and finds the linear layers by shape.
- YuNet wants 640 × 640 **BGR, 0–255, no normalisation** (letterboxed); score = √(cls · obj). It runs on the CPU on purpose (a few ms; the GPU is busy with the big models).
- Exposure counts a pixel as blown only when **all** channels are ≥ 250 – a saturated blue sky has B = 255 and must not count (v1 did, and flagged 13 % on a normal landscape).
- Scripted smoke tests can't use the mouse: posted `WM_MOUSEMOVE`/`WM_MOUSEWHEEL` had no effect in testing (not even wheel-zoom over the photo), while posted keys work. Test pointer behaviour headless (`Context::run_ui` with `RawInput` events, see `ui::details::tests`); remember `textures_delta.clear()` or egui panics on drop.
- The index at `%LOCALAPPDATA%\Cerno\data\cerno.db` is the user's live data (ratings history, taste feedback). Never delete it to "start clean" – DB tests use in-memory databases.
- When the index can't be opened, Cerno runs on an in-memory database (`Db::is_in_memory`): the error notice stays (`open` doesn't replace an error), and straighten, crop, quarter turns and `Ctrl+Z` are blocked (`Blocked::NoIndex`) – a kept original would have no row to be found or pruned by.
- `Library::open` makes the path absolute first; the index keys files by their full path.
- zune-jpeg decodes truncated JPEGs leniently (missing part grey) instead of failing – intended, other viewers do the same.
- **The decoders allocate the whole frame before reading a pixel**, so a few hundred bytes with a patched header can ask for 12.9 GB (65 535²). `decode::check_size` reads the size first (JPEG `decode_headers`, HEIC the image handle) and refuses more than `MAX_PIXELS` (200 MP) – keep that check in front of any new decode path. With leniency the grey fill would otherwise "succeed" after gigabytes.
- Decode jobs run inside `decode::catch_panic` (loader display and full size, `analyze`, the straighten/crop render): a panicking decoder crate becomes an error for that photo instead of ending the worker thread for the session. This relies on `panic = "unwind"` – never set `panic = "abort"` in a profile.
- **HEIC on Windows is dynamically linked** (vcpkg triplet `x64-windows`, `VCPKGRS_DYNAMIC=1` in `.cargo/config.toml`). `libheif` and `libde265` are LGPL-3.0; `build.rs` copies their DLLs and `licenses/` next to the executable. Do not switch the triplet back to `x64-windows-static-md`. The x265 encoder (GPL) stays excluded via `libheif[core]`. Linux links the system libheif; a package depends on the distro library instead of bundling a static copy. ExifTool stays a separate program and is not distributed. The V2.5 head (AGPL-3.0) stays out of the binary.
- The only `rfd` dialogs left are the folder pickers (open, copy / move target); they block the UI thread while open – fine for modal dialogs. Confirmations are `ui/confirm.rs`.
- Help page, menus, the models card and confirmations are modal: `handle_keys` returns early while one is open (help: only `H`/`F1`/`?`/`Esc`, `Ctrl+L`, `Ctrl+K`; the menus take arrows, Enter, Esc and typed letters out of the input in `palette::keyboard`; the cards consume Enter/Esc) and the photo's mouse handling is skipped. It is an `egui::Area` in `Order::Foreground`, so the details panel's scroll area below doesn't react either. Its card height is the content height remembered from the previous frame – not `ScrollArea`'s `content_size`, which with `auto_shrink(false)` is at least the visible area.
- Clicking a map link in Details › File sends the photo's coordinates to Google or OpenStreetMap. Don't click them in scripted tests; `metadata::maps_url` / `osm_url` are unit-tested, and `ui::details::tests` clicks headless (egui only reports the URL).
- Linux always updates `ctime` on write; that can't be prevented and no photo tool shows it. `mtime` and (on Windows) the creation time are preserved.
- Because mtime and often the file size (XMP padding) stay the same, backup/sync tools that only compare size + date (e.g. `rsync` without `-c`) may miss rating changes.
- JPEG decode converts to sRGB once (`decode::to_srgb`): untagged and sRGB stay, Adobe RGB and Display P3 use fixed matrices, anything else goes through moxcms. HEIC is left to libheif. The Windows monitor profile is not applied. Fingerprints change for converted files, so those photos are analysed again; ratings stay. Straighten and crop decode **without** the conversion (`decode::decode_for_edit`) and ExifTool copies `-ICC_Profile` explicitly – it is a protected tag and not part of `-all:all` (`rating::tests::reencode_keeps_the_colour_profile`).
- Smoke-testing the window from scripts: send keys with `PostMessage(WM_KEYDOWN/UP)` to Cerno's window handle and capture with `PrintWindow(PW_RENDERFULLCONTENT)`. Posted modifier keys are ignored – winit reads the thread's key state – so for `Ctrl`/`Shift` combos attach to Cerno's UI thread (`AttachThreadInput`), set the modifier in `SetKeyboardState`, post the key, restore the state and detach; this touches only Cerno's input queue, never the user's foreground window. Never use `SendKeys` – without focus it types into whatever window the user is working in. Wait for a window whose title contains "Cerno": `Process.MainWindowHandle` can briefly point at wgpu's untitled helper window.
- Deletion tests must not touch the real trash: `DeleteQueue` takes a `Remover` function, tests pass a recorder.
- An `egui::Area` is invisible in its first frame (egui measures it): headless tests of overlays (menus, cards) run two frames before looking at shapes.
- A label inside a `left_to_right` layout never wraps in egui, whatever the `LayoutJob` says – it widens the parent and pushes right-aligned values out of view. Wrapped text goes into a `top_down` block with `Label::wrap()` (`details::explanation`).
- `ViewportBuilder::with_maximized(true)` on a scaled Windows display leaves the window flagged as maximized at its restored size, and the first frame's `Maximized(true)` is then a no-op. Only the command is used; the restored window shows for ~13 ms.
- Posted `WM_KEYDOWN`s also produce text input, so scripted tests can use the menus' letter jump.
- Posted arrows, `Home`/`End`, `PageUp`/`PageDown`, `Insert` and `Delete` need the extended-key bit (bit 24 of `lParam`, next to the `MapVirtualKey` scancode): without it winit reads the numpad key with that scancode, and since digits count by physical key, `→` sets the red label and `←` gives 4 stars. Smoke-test on generated photos, never on real ones – the data folder can't be redirected from outside (`directories` asks Windows, not `%LOCALAPPDATA%`), so a run writes into the live index, and a mark on a copy of a real photo becomes a For you example for its fingerprint.
- `Ctrl+Z` restores with `write_in_place` plus a file-time snapshot (like a pixel edit), then writes the marks the file had just before, so a star given after the edit survives the undo.
- The icon source is a JPEG with the "transparent" checkerboard baked into the pixels; `tools/make_icon.py` cuts the blue tile out by colour (blue minus red > 45), so a new JPEG source must keep a blue tile on a light border (a PNG with real transparency keeps its alpha). Change the icon there and regenerate both files – never edit `icon.png` / `icon.ico` by hand. Explorer caches exe icons: after a change it may show the old one until `ie4uinit.exe -show` or a new sign-in.
- **The Windows install directory is Cerno's data root.** The per-user installer puts the exe into `%LOCALAPPDATA%\Cerno`; the index, models and kept originals are in `%LOCALAPPDATA%\Cerno\data`. The uninstaller deletes only the files it lists plus a non-recursive `RMDir`, so the data goes only with the opt-in "Delete the application data" box (`appdata-paths`). `test-installer.ps1` asserts that – keep it when changing the installer.
- **The VC++ runtime is not part of Windows.** `cerno.exe` (ONNX Runtime), `heif.dll` and `libde265.dll` import `MSVCP140`/`VCRUNTIME140`. `build.ps1` copies them app-locally and fails when any import of the folder is neither in it nor in System32 – a VC++ runtime found in System32 does not count (runners and dev machines have one, a fresh Windows does not). `crt-static` is no option: ort's prebuilt library and the vcpkg triplet are /MD.
- **HEVC decoding on Linux is a libheif plugin**, dlopen'd from a compiled-in directory, so `ldd` never shows it missing. Ubuntu 24.04's `libheif1` only *suggests* `libheif-plugin-libde265`; the `.deb` depends on it. The AppImage carries it in `usr/lib/libheif/plugins`, `AppRun` sets `LIBHEIF_PLUGIN_PATH` (it replaces the default directory) and `patchelf` gives it an rpath to the bundled libde265. CI's install tests open a HEIC under Xvfb in clean containers and wait for `start-up: first photo drawn`.
- The AppImage uses its own `AppRun`: AppImageKit's (what cargo-packager ships) changes into `$APPDIR/usr` – a relative path argument then points elsewhere – and sets `LD_LIBRARY_PATH` for ExifTool and the browser too. The binary finds the bundled libraries through linuxdeploy's rpath instead.
- winit, wgpu and xkbcommon load their libraries with dlopen, so cargo-deb's `$auto` misses them; the `.deb` lists them by hand. The desktop file must stay `cerno.desktop` (= the app id).
- cargo-packager's config is camelCase with `deny_unknown_fields` and only a few kebab aliases: `installer-mode` (not `install-mode`), `custom-language-file` (singular). It has no German or Italian texts; `packaging/windows/*.nsh` must stay UTF-8 **with BOM** (NSIS reads BOM-less files in the ANSI code page).
- In a workflow `run:` block (bash `-e`), a negated command – `! grep -q "not found" ldd.txt` – never fails the step; bash exempts `!` pipelines from `-e`. Write `if grep -q …; then exit 1; fi`.
