# AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, Cursor, Copilot …) and for people working on this repository. `CLAUDE.md` only imports this file; personal notes go into the untracked `CLAUDE.local.md`.

Cerno (Latin *cerno* – "I sift, discern, see clearly") is a **pure-Rust desktop photo viewer and culling tool** for Windows and Linux: switch between photos instantly, rate them from the keyboard, zoom to 100 %, sort/filter by automatic sharpness and aesthetics scores. UI is **egui/eframe on wgpu** – no WebView. The parent mono-repo `../CLAUDE.md` covers shared conventions; this file covers Cerno specifics. Details and history of each decision are in `CHANGELOG.md` and the git history of this file.

## Standards

This project follows the latest versions of two documents in the parent folder
(outside this repository): `repo_standard_v*.md` (setup, docs, CI, release) and
`ui_design_system_v*.md` (UI, where the project has one). Read them before changing
those areas; where this file says something different, this file wins.

Cerno's deliberate differences from the repo standard: no Dependabot (there is no pull-request flow; actions are bumped by hand, see *Actions pinned by commit*), `cargo audit` instead of `cargo deny`, and the `stable` toolchain instead of a `rust-toolchain.toml`.

## Commands

```bash
cargo run --release --features heic,video -- <folder-or-image>   # deps are opt-level 3 even in debug
cargo fmt --all -- --check
cargo clippy --all-targets --features heic,video -- -D warnings  # also run once without features
cargo test --features heic,video
cargo test natural_order                                   # single test
CERNO_TEST_HEIC=<file.heic> cargo test --features heic -- --ignored --nocapture heic   # HEIC decode + rating round trip
```

- On Windows `cargo` lives in `%USERPROFILE%\.cargo\bin`.
- `--features video` needs GStreamer. Windows: `pwsh packaging/windows/install-gstreamer.ps1` once (pinned MSVC build, per user), then set `PKG_CONFIG`, `PKG_CONFIG_PATH` – and `PATH` for `cargo test` – as its help text shows. Linux: `apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav`.
- The release build puts `DirectML.dll` next to `target\release\cerno.exe`; a `heic` build also `heif.dll`, `libde265.dll` and `licenses/` (`build.rs`). Copy them along with the exe. `build.rs` also embeds `assets/icon.ico` and version info (`winresource`, needs the Windows SDK's `rc.exe`).
- Unit tests live in-module (`#[cfg(test)]`); fixture `tests/fixtures/tiny.jpg`. `rating::tests::writes_stars_and_keeps_file_dates` is a real ExifTool round trip and **silently skips without ExifTool on `PATH`** – make sure it ran before trusting a write-path change.
- `pwsh tools/measure_startup.ps1` / `tools/measure_exit.ps1` time start and exit (never on the live data folder).

**HEIC on Windows** needs libheif from vcpkg in `target/vcpkg`, dynamic triplet `x64-windows`; `.cargo/config.toml` sets `VCPKGRS_DYNAMIC=1` (without it `build.rs` aborts). A tree once built against `x64-windows-static-md` needs `cargo clean -p libheif-sys`. `cargo vcpkg build` fails at bootstrap on current Rust, so finish by hand:

```bash
cargo install cargo-vcpkg && cargo vcpkg build     # clones target/vcpkg, then fails at bootstrap
target\vcpkg\bootstrap-vcpkg.bat -disableMetrics
target\vcpkg\vcpkg.exe install "libheif[core,libde265]:x64-windows"
cargo clean -p libheif-sys
cargo build --release --features heic
```

- The pin is `[package.metadata.vcpkg] rev` in `Cargo.toml` (libheif 1.23.7, libde265 1.1.3). Since libheif 1.23.6 the HEVC decoder is the port feature `libde265`: `libheif[core]` alone opens every HEIC and decodes none, so `build.rs` stops when `libde265.dll` is missing.
- Raising the pin: update its comment and `licenses/heic.txt`; CI's cache key follows the rev. Locally: `git -C target/vcpkg fetch --depth 1 https://github.com/microsoft/vcpkg <rev>`, `git -C target/vcpkg checkout FETCH_HEAD`, bootstrap, `vcpkg.exe remove --outdated --recurse`, install, `cargo clean -p libheif-sys`, build. `heif.dll` has no version resource – the HEIC test prints the loaded version.

**Packages** (what CI's `package-windows` / `package-linux` run after the HEIC release build):

```bash
pwsh packaging/windows/build.ps1   # dist/windows: Cerno\ folder, cerno_<v>_x64-portable.zip, cerno_<v>_x64-setup.exe
packaging/linux/build.sh           # dist/linux: cerno_<v>_amd64.deb, cerno_<v>_x86_64.AppImage (Ubuntu 24.04 only)
```

`build.ps1` needs `cargo install cargo-packager --version 0.11.8 --locked` and Visual Studio C++ tools (Build Tools have no `VC\Redist`: pass `-CrtDir` with `msvcp140*.dll` / `vcruntime140*.dll`). `build.sh` needs `cargo-deb`, `patchelf`, `libheif-plugin-libde265`, `zsync`. `packaging/windows/test-installer.ps1` installs over `%LOCALAPPDATA%\Cerno` – **CI only, never on the author's machine.**

## Product decisions (settled – build on them, don't re-propose alternatives)

### Platform, formats, files
- **Pure Rust + egui/eframe (wgpu)** instead of Tauri: WebViews can't show HEIC, WebKitGTK is slow. Decode once → GPU texture.
- **Formats**: JPEG + HEIC (`libheif-rs`, feature `heic`; vcpkg on Windows, system libheif ≥ 1.17 on Linux); PNG/TIFF/WebP/BMP/GIF (first frame) via `image`; **RAW** shows its largest embedded JPEG preview (`raw::preview`, no development – the UI says *RAW preview*, warns below `raw::SMALL_PREVIEW_PX`, decided by `Format::is_raw`); **video** shows the frame at 1 s from GStreamer helper processes that stay open (`cerno --frames`, `video.rs` → `frames::grab`) and plays inside Cerno with sound (`playback/`, `Space`). No ffmpeg. Videos get no analysis and no index row. **Only JPEG can be edited** (`Blocked::NotJpeg`).
- **Marks of proprietary RAW (not DNG), BMP and video go into an XMP sidecar** (`IMG_1.xmp`, or `IMG_1.MOV.xmp` when a RAW shares the stem – `sidecar::path_of`, `sidecar::moved`), created on first write, never overwritten; once it exists it replaces the file's own marks (`metadata::with_sidecar`), so the first write carries all marks (`write_marks`, `fresh`). Delete/copy/move take the sidecar along.
- **RAW + JPG are one photo** (setting, on by default): exactly one RAW/DNG and one JPEG of the same stem pair (`pairs::pair_up`); the JPEG shows, marks go into both (`app/pairs.rs`), copy/move/delete/`Ctrl+Z` take both, edits change the JPEG only. Differences in the RAW sidecar are shown (`pair_note`), never aligned silently. The analysis never sees the RAW. A Lightroom sidecar keeps its development and foreign namespaces.
- **Live Photos are one photo** (*Settings ▸ Live Photos as one photo*, on by default, `live_photo_pairs`; CER-73, the user's decisions of 2026-11-22): a HEIC or JPEG and a `.MOV` of the same stem in one folder pair when the clip is a Live Photo's – Apple's content identifier in `moov/meta`, or at most `quicktime::LIVE_MAX` (4 s) – read from box headers only (`quicktime.rs`, no crate); a real video never pairs, and beside a RAW pair the clip stays apart (one companion per photo). `pairs::Pairs` knows the kind (`raw` / `motion`): the clip rides along like a RAW for copy, move, delete, put back, reveal and rename, but **marks go into the picture only** (`rate_companion` and its siblings use `Pairs::raw`). Badge `LIVE`; `Enter` in the single view plays the clip once over the picture (`current_live`, `play_live`; the picture shows again at its end), `Space` still moves on – the user's choice, so browsing never stops at every iPhone photo. No conversion, no merging, no Android motion photos.
- **Only rating, colour label, comment and keywords are written into the photo** (`xmp:Rating` 1–5 or −1 = rejected; `xmp:Label` English names; comment/keywords via ExifTool MWG tags). Scores, thumbnails, embeddings, capture time live in the SQLite index under the app data dir – never in or next to photos, except the hidden `.originals` folder. AI scores never set stars or labels. Unknown label strings are left alone.
- **Pins** (UI: German *Vormerken*, English *Pin* – never "mark", which means stars/labels/reject; `V` toggles, `Shift+V` pins and moves on; CER-74, the user's decisions of 2026-11-22): a pre-selection never written into a file – table `pins` in the index only (`db/pins.rs`), so it survives a restart; `retarget_path`, `record_deletion` and `record_restore` carry it along, and `CernoApp::pins` is reloaded after moves, renames, deletions and putting back (`reload_pins`). Gated by `Change::Pin` (no ExifTool needed; a photo moved, renamed or deleted takes none). Lucide `pin` on the cell (`CellInfo::marked` – `pinned` is compare mode's left photo) and in the info bar; filter box `FilterKind::Pinned`, a group of its own first in the bar (`V` in the filter mode); `Ctrl+Z` (`undo::Entry::Pin`); *Visible photos ▸ Unpin all (n)* with a confirmation card, no key. **Collection** (*Visible photos ▸ Pinned photos of every folder*, `open_pins`): the pinned files still there go through `take_library` like a folder (`Folder::collection`, `dir` = their common folder or `None` across drives, then names show whole); RAWs/clips of the same stem are looked for beside them (`pins::companions`) and paired as usual. Camera time, the 🗑 scan and `Ctrl+U` are greyed out (`pins_folder_only`); unpinning removes the photo at once (`collect_member`), a move gathers the collection again; opening a folder ends it, nothing is saved.
- **Straighten, crop, quarter turns change the file and keep the original**: before the first edit the file is copied into `.originals` beside it (`originals::keep`, table `backups`); no copy → no edit. Only the first original is kept; `Ctrl+Z` writes it back in place, then the current marks. **Deleting moves the photo into `.originals`** (`originals::set_aside`, no trash). Originals follow moves (`originals::follow`). Only files directly in the photo's own `.originals` (or legacy `data/backups`) are touched (`originals::allowed`). Turns change EXIF orientation only; straighten/crop re-encode at q95, 4:4:4, with the ICC profile.
- **ExifTool writes every mark**; no package ships it. Windows downloads the pinned official package on request (`tools::EXIFTOOL`, via `download.rs`, into `data/tools/exiftool`); Linux takes it from the distribution (`system::exiftool_install_command`). **Without ExifTool or older than 12.24, every mark and edit is greyed out with the reason** (`Blocked::NoExifTool` / `ExifToolOld` / `ExifToolLoading`, `app/exiftool.rs`); viewing, delete, copy, move still work.
- **Marks not written are kept, not lost**: a failed write keeps its marks in `rating::unsaved` with the error, retries after 5 s / 30 s / 2 min, then on folder open, the card's *Try again*, before copy/move/delete of that photo (`retry_before`) and at exit. The info bar shows `⚠ n marks not saved` (`ui/unsaved.rs`). A failed write releases its hold with `Held::unchanged()` (no generation bump). Not kept across restarts.
- **File dates must never change**: ExifTool with `-P -overwrite_original_in_place`, and `filetimes.rs` snapshots and restores mtime/creation time around every write.
- **Renaming** (`F2` the current photo, `Shift+F2` every photo shown; the user's decisions of 2026-11-22): a card offers four patterns – *Standard* `2026_08_23__191546` (the author's own, ≈ 54 000 files), *Dropbox / OneDrive* `2026-08-23 19.15.46`, *Android / Samsung* `20260823_191546`, *Date, text, counter* `2026-08-23_Roadtrip_001` (text = folder name to start with, counter in capture order) – chosen with `↑`/`↓` or `Alt+1`–`Alt+4`; one photo's name can be typed over (`rename::check`: Windows' forbidden characters, reserved names, a dot or space at the end), several show *old → new*. The time is the corrected capture time (`Facts::taken_ms`, camera offsets added); no time: the date patterns are greyed out, a batch leaves the photo as it is. A taken name gets `_1`, `_2` … (as in the author's archive), the extension stays, a RAW riding along takes the same new name, sidecars go along. Pure planning in `rename.rs` (`plan`, `rename_one`, `order` – a chain from its free end, a ring through a hidden `.cerno-rename-…` name); the files are renamed by the transfer worker (`transfer::Mode::Rename`, `run_renames`), gated by `Change::Rename` (`Blocked::Renaming` while it runs). `finish_rename` re-keys what knows a photo by path (marks, pairs, `ScoreBoard::rename`, `folder.all` sorted again, pinned, journal) after `retarget_moved`; `Ctrl+Z` gives every photo of the last rename its old name back (`undo::Entry::Rename`, not recorded again).
- **Subfolders stay off** until the user turns them on (`Ctrl+U`); hidden dirs skipped, dir symlinks not followed, `.originals` never opened as a folder.
- **No system dialogs** except the folder pickers and Edit elsewhere's program picker / "Open with". Confirmations are in-app cards (`ui/confirm.rs`); bulk rows name their count instead of asking; deleting uses the 5 s countdown. Hints fade, errors stay until `Esc`/click.
- **Version**: `Cargo.toml` is the only source (help, About, start screen show `CARGO_PKG_VERSION`).
- **Four packages**, all with HEIC and video: NSIS installer (per user, five languages) + portable zip, AppImage + `.deb` (built on Ubuntu 24.04 → glibc 2.39+). CI builds on every push to `main`, drafts a release for a `v*` tag (must equal `Cargo.toml`, notes from the CHANGELOG section, `SHA256SUMS.txt`, attestation). **Not code-signed, no signing planned**; no updater – the update check only names a version; the AppImage carries zsync update info. AppStream metadata in `packaging/linux/io.github.fly2nbc_oss.cerno.metainfo.xml`.

### Analysis and scores
- **GPU inference on all vendors, AMD first**: Windows ONNX Runtime + DirectML; Linux CPU, experimental `webgpu` feature (kept; CI clippies it). No CUDA/ROCm-only paths; same model everywhere.
- **Only six modules, all local**: LAION, V2.5, prediction (taste), CLIP attributes, exposure, eye sharpness. A new module needs a DB column + version constant + `is_complete` check.
- **Aesthetics = CLIP ViT-L/14 + LAION head** (collapsed into one linear layer, `src/analysis/aesthetic_head.bin`, embedded) and **SigLIP so400m + Aesthetic Predictor V2.5** (head is **AGPL-3.0 – never embedded or committed**; distributed in release **`models-1`**). Models are downloaded on request, pinned by URL/size/SHA-256 in **`analysis/manifest.rs`, the one place naming every downloaded file**. **Never replace a file under `models-1`**: new bytes need a new tag, new manifest values and, if scores change, a new `MODEL_ID` / `V25_MODEL_ID`. The app never converts models itself.
- **One aesthetics value**: the mean of LAION and V2.5 (`aesthetic::combined`), the only aesthetics sort (`SortKey::Aesthetics`); LAION and V2.5 appear separately only in the details panel. The DB keeps raw 1–10 scores.
- **Two values under the photo, both in percent**: Aesthetics (`aesthetic::as_percent`, fixed scale 2–8 → 0–100 %) and Sharpness (a folder percentile).
- **The prediction** (UI: "Prediction"; code: taste / `personal`) = ridge regression on CLIP embeddings (`analysis/taste.rs`, dual/primal form give the same weights). Labels: stars 1–5, rejected = 0; **deleted photos don't count**. Trains in the background from 15 examples. In the info bar it only lightly fills the empty stars of an unrated photo (`info_bar::hint_stars`); never sets stars.
- **Eye sharpness via YuNet** (embedded, CPU): with a face ≥ 40 px, "subject" sharpness uses the eye percentile.
- **"Probably blurry"** = folder's blurriest 20 % **and** below an absolute ceiling (`view::BLURRY_FRAME_MAX` / `BLURRY_EYES_MAX`).
- **Duplicates are exact and only marked**: same fingerprint; the original by `view::pick_original`. Nothing rejected or deleted automatically; CLIP near-duplicates are not duplicates.
- **Incomplete JPEGs are only marked** (`jpeg_info::is_complete`, `images.truncated`, filter `FilterKind::Incomplete`); never repaired, never in Top N.
- **Check overlay** (`O`: sharp edges → clipping → off), computed in the loader from the decode (`overlay.rs`), never saved; thresholds `overlay::PEAK_SHARE` / `PEAK_MIN`, exposure uses `exposure::HIGHLIGHT` / `SHADOW`.

### Views and navigation
- **Keyboard-first, Cerno's own key layout – not a Lightroom clone.** Keys and features are never named after or justified by Lightroom; user-facing texts don't say "Lightroom-style". Lightroom is named only for compatibility. The full key list is the help page (`Texts::help_keys`) and `app/keys.rs`. Notable (reviewed with the user for 1.16): views `F5`–`F8` in switcher order (photo, grid, scenes, faces), bars `F10`/`Alt+1` menu, `Tab`/`Alt+2` details, `F`/`Alt+3` filter, `Ctrl+F` filter mode, `Shift+F` filter aside (every filter key on F), `F9`/`Alt+4` filmstrip, `Shift+Tab`/`Alt+0` all; full screen `F11` only; `0`–`5` stars, `6`–`9` labels, `X` reject; `V` pin; **Shift has two meanings**, both kept and named in the help's tips: with a marking key (`0`–`9`, `X`, `V`, `A`, `D`, `Space`) "and move on", with a view or action key (`C`, `Tab`, `F`) "more / all"; `Del` only ever deletes; free: `T`, `G`, `F3`, `F4`, `F12`; `C` compare, `Shift+C` four-up, `Alt+C` split view; `M` similar ("more like this"); help `F1`/`H` (no `?`); `Ctrl+L` language. `Alt` digits only with `Alt` alone (`AltGr` = `Ctrl+Alt`). Auto-advance is a menu switch, not Caps Lock.
- **The info bar always shows**; menu bar, filter bar, details panel, filmstrip are optional and saved (`top_bar`, `details_mode`, `filmstrip`, `side_bar`). The filter bar also shows when a filter hides every photo.
- **The info bar's block** (right end, `info_bar::block`): view switcher (photo · grid · scenes · faces) │ four bar buttons in window-edge order │ help. A part never vanishes – greyed out with the reason. The faces part shows the faces grid's count (`face_button`, `app::faces::judged`).
- **Menu bar on the left** (`ui/side_bar.rs`): only what has no other home (open folder; *This photo*, *Visible photos*, *View*, *Settings*), folding groups, fixed width. **Mouse only, takes no key**; rows sense `Sense::CLICK`, never egui focus. Rows are greyed out, never missing. `E` without a remembered program opens the program list over the photo.
- **Edit elsewhere** (`E`): programs the system registers (`external.rs`: Windows shell COM, Linux `.desktop`), choice remembered; goes through `allowed(Change::External)`, keeps the first original before starting; the photo is watched and reloaded after the program saves, restoring marks the program dropped (`app/external.rs`).
- **Browse arrows** over the single photo while the pointer moves (`overlays::browse_arrows`), fading; single view only.
- **Navigator** while zoomed in the single view (`ui/navigator.rs`): press/drag centres the view (`Zoom::centre_on`, clamped by `Zoom::reachable`).
- **Split view** (`Alt+C`): the same photo whole (left, never zooms) and at 100 % (right, shared zoom), aimed at the largest face's eyes, else the sharpest spot (`overlay::sharpest_point`), else the middle; the frame on the whole side drags the 100 % side. Not for videos, not saved. Only stage 1 (no second window).
- **Four-up view** (`Shift+C`) and **Grid** (`F6`): the frame/cursor **is** `current`, so marks, delete, turns, `Ctrl+Z` need no code of their own. Grid draws only visible rows; while it shows `frames` is empty and only the cursor photo decodes. Floating date pill only in capture-time order.
- **Scene view** (`F7`): the grid grouped into scenes (`view::scenes`: series/single photos joined within 2 min, span ≤ 10 min, CLIP ≥ 0.85 to the scene's **first** block) with header lines; runs of single photos collected (`view::sections`); always capture-time order while shown (sort box greyed out). **The only place similarity groups photos.**
- **Faces grid** (`F8`, `ui/faces.rs`): all faces large over the photo; boxes between two heads dropped (`faces::bridges`); faces table filled by the analysis, older photos searched again only when shown. Details panel tabs: Values | Description, `Ctrl+Tab`.
- **Compare mode** (`C`): see *Compare mode and deletion*.
- **Series** are time groups (≤ 2 s apart, same camera model), never collapsed. **Nothing is folded away** anywhere except the scene view.
- **Show in Explorer** (`Command::Reveal`, `external::reveal`); no key.
- **About Cerno** is the help's third tab; problems go to GitHub issue forms (link fills only `version` and `os`, never paths). No telemetry.
- **Update check**: **off until the user says yes** (one question card), then at most once a day: one GET to `/releases/latest`, reads only the redirect's `Location`, User-Agent `Cerno`. Nothing downloaded or installed (`update.rs`, `app/update.rs`).

### Filters
- **Filter bar order**: scope box (photos / videos / Top N) → camera box → "without ✕" → chips by group → ≈ similar → file list / keyword chips; right: count with × (*Show all*), sort, *Refresh order*, status. **Groups AND, boxes within a group OR** (`PhotoFilter::accepts`). Stored filter ids keep `FilterKind::ALL`'s order. **Nothing in the filter bar changes width while used.**
- **Photos / videos / both** is a filter (`view::Media`), AND with the boxes.
- **Top N** (10/25/50/100/250, in the scope box, no key): `view::pick_top`; the pick is frozen until a filter changes or *Refresh order*; never saved; bulk delete greyed out while on; never writes a mark.
- **Similar photos** (`M`): cosine ≥ `view::SIMILAR_MIN` (0.85) to the current/pinned photo, an AND filter, not a group; never saved.
- **Deleted photos** come back through the 🗑 box (`originals::deleted_in` on a thread, `app/deleted.rs`); they only look – every change but `Change::Restore` is `Blocked::Deleted`; `Ctrl+Z` or *Put back* restores (`originals::restore`).
- **Camera filter**: model, or body by serial (`ViewOptions::camera`, `view/cameras.rs`), never saved; fixed-width box.
- **Camera clocks set right**: offset per folder + camera model in table `camera_offsets`, only in the index; added in `CernoApp::facts`, so sort/series/Top N follow.
- **File list** (menu › *Visible photos*): pasted names → `name_list::match_list`, an AND filter, never saved.
- **Keyword filter and suggestions**: keywords in the index (`files.keywords`); card `ui/keywords.rs`, pick OR within, AND with the rest, never saved; a keyword changed while filtering moves nothing until the next build. Suggestions while typing from the folder's, then the index's vocabulary (`keywords.rs`); one word in any case.
- **A hidden filter shows in the info bar** (accent `3 / 12 of 340`) while the filter bar is off; nothing is reset.
- **Filter aside** (`Shift+F`, `app/filter_aside.rs`): sets the whole filter aside and back; never saved; lapses with another folder. A field added to `ViewOptions` is a filter automatically (`view::tests::the_whole_filter_goes_aside_and_comes_back` won't compile without naming it).
- **Filter mode** (`Ctrl+F`, `app/filter_mode.rs`): the filter bar from the keyboard – a frame Cerno paints on one box (never egui focus), arrows/`Home`/`End` move, `Space`/`Enter` act, marking keys tick their box, Backspace = *Show all*, `Esc`/`Ctrl+F` end. Photo keys rest meanwhile. Top-centre overlays stack by one rule (`overlays::stack_top`).

### Look and language
- **Design system v2.0**, values in `theme.rs`, **dark only**: neutral grey surfaces (`#141414` / `#202020` / `#262626`), accent `#5B8EC4` only for on/selection/progress (hover neutral), status colours and the five label colours carry hue. Font sizes only from `theme::text`, radii only `theme::radius`. Buttons via `theme::primary_button` / `secondary_button` / `danger_button`. `LINE_STRONG` only where the border is the element. **Icons are Lucide** (embedded subset, `assets/lucide`, `tools/make_lucide_subset.py`, `icons::glyph`); stars, flags, labels and the video play button stay painted. Deliberate: `spacing.interact_size` stays 18 (`theme::keep_spacing`).
- **UI in German, English, French, Spanish, Italian**; starts in the system language, `Ctrl+L` cycles, saved. Numbers keep the decimal point in every language.

## Architecture

```
main.rs              eframe bootstrap (wgpu); `--version` first (no window/log/data dir), then `--frames`, `--check-video`
crashlog.rs          panic hook → `crash.log` in the data folder (≤ 50 KB)
app/mod.rs           CernoApp: services, `view` / `current` / `options`, one part struct per submodule; `new` → `assemble`, `run_frame`, exit
app/pairs.rs         RAW + JPG as one photo
app/camera_time.rs   camera offsets: `facts`, compare row, camera time card
app/browse.rs        opening a folder, building the view (`rebuild_view` / `set_view`, `refresh_marks`), navigation
app/marks.rs         stars, rejection, colour labels (session maps → writer)
app/files.rs         copy / move of the view, deletion countdown
app/deleted.rs       deleted photos in `.originals`: listed, shown via 🗑, put back
app/faces.rs         current photo's faces, face zoom, the switcher's face count, split view eyes
app/editing.rs       straighten / crop sessions, turns, `Ctrl+Z` target, edit overlay
app/undo.rs          session journal for `Ctrl+Z` (≤ 200, not saved)
app/external.rs      Edit elsewhere: start, watch, reload after save
app/gate.rs          one action at a time (`blocked`, `allowed`, `menu_block`)
app/exiftool.rs      ExifTool found / download / Models & data row / install command
app/keys.rs          `read_keys` (pure), `KeyInput::commands`, `read_filter_keys`, `handle_keys`, the `Esc` chain
app/filter_mode.rs   the filter mode (`Ctrl+F`)
app/command.rs       `Command` from keys, menu rows, clicks; `execute_all`, guarded `run` / `click`
app/layer.rs         `Layer`: help, a list, or a card – at most one
app/target.rs        decode size (`Target::settle`, `update_target`)
app/harness.rs       tests only: the whole app headless (`frame` / `press` / `settle`)
app/photos.rs        slots, compare, four-up, split view, mouse zoom / pan, drawing photos
app/scenes.rs        scene view: capture-time order, `SceneState`, its navigation
app/frame.rs         `Layout` of bars and photo area
app/menu.rs          menu bar content, lists, bulk counts, help, models card, confirmations
app/panels.rs        which bars show (`Bars`), language switch
app/name_list.rs     file-list filter
app/keywords.rs      keyword filter and vocabulary
app/notice.rs        notices and the deletion countdown
app/filter_aside.rs  `Shift+F`
app/camera_filter.rs camera box entries, Details' camera button
app/unsaved.rs       marks not written: count, card, `retry_before`, `exit_step`
app/pins.rs          marks in Cerno alone (`V`): toggle, undo, reload, clear
app/rename.rs        renaming (`F2`, `Shift+F2`): the card's data, the job for the worker, `finish_rename`
app/update.rs        update check from the app's side
app/video.rs         the playing video: start / pause, keys, bar, volume
view/                sorting, filtering, series, duplicates, Top N, scenes, sections, nav, cameras (pure, unit-tested)
ui/viewer.rs         fit / zoom / pan geometry, drawing (display texture or tiles)
ui/filmstrip.rs      thumbnail strip
ui/grid.rs           the grid (pure `Layout`, `row_step`, `top_cell`), floating date
ui/scenes.rs         scene view layout and headers
ui/cells.rs          one thumbnail cell (filmstrip, grid, scenes)
ui/filter_bar.rs     filter bar; filter-mode slots (`Slot`, `step`, `settle`)
ui/info_bar.rs       info bar: stars, meters, the right-end block (`block`, `corner_buttons`)
ui/video_controls.rs bar under a playing video
ui/overlays.rs       browse arrows, compare labels, countdown, notices, filter-mode pill, flags
ui/details.rs        side panel: values, file info, map links; tabs
ui/tabs.rs           tab strip
ui/faces.rs          faces grid
ui/description.rs    description tab: comment, keyword chips, suggestions, IIM length warnings
ui/help.rs           help (shortcuts | tips | About) and start screen
ui/palette.rs        list beside a menu row, over the photo or below a filter box; `bar_row`
ui/side_bar.rs       menu bar (clicks only)
ui/modal.rs          card over the dimmed window
ui/navigator.rs      navigator while zoomed
ui/confirm.rs        confirmation card
ui/camera_time.rs    camera time card
ui/rename.rs         rename card: four patterns, the name field or the *old → new* list
ui/name_list.rs      file-list card
ui/keywords.rs       keyword filter card
ui/unsaved.rs        unsaved-marks card
ui/models.rs         Models & data
ui/edit.rs           straighten grid, crop frame
ui/icons.rs          Lucide glyphs, painted flags / play button
ui/stars.rs          star shapes
i18n/                `Texts` + one full literal per language, date / coordinate formatting
pairs.rs             RAW ↔ JPEG and Live Photo picture ↔ clip pairing (pure)
camera_time.rs       offsets `±h:mm:ss`, compare alignment (pure)
name_list.rs         `match_list` (pure)
rename.rs            rename patterns, name check, `plan`, `rename_one`, `order` (pure)
keywords.rs          fold, `Vocabulary`, `Pick` (pure)
library.rs           folder scan, `Format`, natural order, stamps
loader.rs            prefetch workers + texture cache, full-res tiles, overlay textures, `LoadedImage::focus`
decode.rs            bytes → RGB8: JPEG (zune-jpeg), HEIC, `image` formats, RAW preview; resize, orientation, sRGB
raw.rs               largest JPEG preview in a RAW
quicktime.rs         a QuickTime / MP4 file's duration, Apple's content identifier and creation date, from box headers
sidecar.rs           XMP sidecar paths
video.rs             frame helpers (`cerno --frames`), protocol, timeouts, placeholder
frames.rs            one frame through GStreamer (what a helper runs)
playback/            playback (`engine.rs` with `video`, `stub.rs` without), `probe`, `configure_environment`, `self_test`
metadata/            rating, label, comment, keywords, hierarchy, capture time, orientation, camera; IIM limits
analysis/mod.rs      background analysis + ScoreBoard + taste trainer thread
analysis/manifest.rs every downloaded model file (URL, mirror, size, SHA-256); `Pack`
analysis/models.rs   `ModelState`, lazy model slots, downloads, delete models
analysis/sharpness.rs, aesthetic.rs, attributes.rs, exposure.rs, faces.rs, taste.rs   the modules
thumbs.rs            filmstrip / grid textures, video thumbnails
histogram.rs         RGB histogram
overlay.rs           check overlay, `sharpest_point`
edit.rs              straighten / crop geometry and re-encode
jpeg_info.rs         JPEG quality estimate, subsampling, completeness
originals.rs         `.originals`: keep, set aside, list, restore, follow, migrate
update.rs            update check (one GET, version parse, `due`)
transfer.rs          copy / move / rename worker with `Progress`
deletion.rs          delayed deletion queue
dlls.rs              delay-loaded DLLs checked before first call
db/                  SQLite index (files, images, backups, set_aside, faces, camera_offsets, taste, settings, pins); additive `migrate`
rating/              debounced mark writer, edits, restores, unsaved marks; one stay-open ExifTool
exiftool.rs          stay-open protocol, `locate`, version check
download.rs          HTTPS download with size / SHA-256, resume, stall timeout
tools.rs             ExifTool download and safe unpack (Windows)
process.rs           `spawn_tied`, `hide_window`, `find_in_path`
sync.rs              poison-tolerant `lock`
system.rs            Linux os-release, ExifTool install command
external.rs          registered programs, system chooser, `reveal`
filelock.rs          who reads / writes which photo; write generations
filetimes.rs         file timestamp snapshot / restore
paths.rs             data, model, tool, DB locations (`CERNO_DATA_DIR`)
theme.rs             design tokens, fonts, buttons, `test_context`
tools/*.py           one-off model prep, `i18n_edit.py`, `make_icon.py`, `make_lucide_subset.py`, `benchmark_excire.py`
assets/              icon source → `icon.png` / `icon.ico` (via `tools/make_icon.py`), `lucide/`
packaging/windows/   build.ps1, test-installer.ps1, installer texts
packaging/linux/     build.sh, AppRun, cerno.desktop, metainfo
```

### Loader (speed-critical)
- Workers (2..=4) pick the most urgent missing index at pick time: `0, full-res of 0 (if zoomed), +1, -1, +2, +3, -2, -3`. Nothing is queued ahead.
- Display images are decoded at the photo area's physical size with **Lanczos3** (`decode::decode_for_screen`) and drawn **pixel for pixel** – egui-wgpu builds no mipmaps, so never let the GPU shrink a texture. Analysis and thumbnails keep `decode_for_display` (CatmullRom): the 256 px thumbnail feeds the fingerprint.
- A new area is taken after `TARGET_SETTLE` (200 ms); stale decodes stay on screen meanwhile (`loader::stale`).
- Full resolution only for the current photo when zoomed beyond the display image (`viewer::needs_full`, no tolerance), in tiles ≤ 4096 px.
- Workers upload textures themselves and make filmstrip thumbnails. `set_library` carries cached images over by path; a generation counter discards stale results.

### Analysis
- Two workers, nearest-first over the whole folder, paused 0.9 s after each navigation.
- Fast path: `stat` + one query; if `is_complete` holds, nothing is decoded. A model that failed to load is never waited for (`Capabilities`).
- Otherwise decode at 2048 px once and compute only what is missing: thumbnail → **fingerprint (FNV-1a over thumbnail pixels + full size)** → sharpness → exposure → faces → CLIP → SigLIP. Bumping one module's `VERSION` re-runs just that module.
- Metadata fast path: when only `metadata_version` is stale (or `files.keywords` is NULL), read metadata without decoding.
- **DB migration is additive only** (`PRAGMA table_info` + `ADD COLUMN`). Never drop or recreate tables – the index holds the user's rating history.
- Records are keyed by fingerprint (renames keep scores). The writer updates the file size in `files` after each write so our own writes don't trigger re-fingerprinting.
- `Analyzer::preload` fills the ScoreBoard when a folder opens (only when `depends_on_scores`). With score-dependent sort/filter the bar offers *Refresh order*; in name order marks refresh themselves (`refresh_marks`).

### Start-up and exit

**Reference values** (release, warm, measured 2026-10/11 – re-measure after touching these paths and compare):

| What | Value | How |
|---|---|---|
| Start to first photo on screen | ~300 ms | `start-up:` log line, `pwsh tools/measure_startup.ps1` |
| Next photo (neighbour prefetched) / a jump (`End`, fresh decode) | 0 ms / median ~45–50 ms | `browse: photo drawn N ms after the step` |
| Opening a folder, subfolders on, score sort: 1246 / 96 245 files | 20 ms / 0.63 s | `open:` log line |
| Exit (window frozen meanwhile) | ~50 ms, 0.3–0.4 s while the analysis runs | `exit:` log lines, `pwsh tools/measure_exit.ps1` |
| Video to its first frame | ≤ ~0.8 s on D3D12, ~1 s on the CPU | `cargo test --features video -- --ignored first_frame_times` (`CERNO_VIDEO_CPU=1`) |
| Full resolution when zoomed to 100 % (generated 20 MP JPEG) | ~165 ms | `cargo test --release -- --ignored split_view_costs` |
| Video thumbnail (256 px) from a frame helper | median ~140 ms | – |
| Training the prediction after a rating | ~30 ms | – |

- `main` builds the `egui::Context` and the whole `CernoApp` **before** `run_native_ext`, so the first photo decodes while wgpu sets up. Before the first frame only the current photo decodes (`START_TARGET` 4K); prefetch starts after the first area-based `set_target`. wgpu uses one backend (DX12 / Vulkan+GL; `WGPU_BACKEND` overrides). `start-up:`, `open:`, `browse:` and `exit:` log lines (info) measure – re-measure after touching these paths.
- eframe calls `on_exit`, then drops `CernoApp`, then the window: everything slow there is a visible hang. The taste trainer and analysis check a stop flag; `Analyzer::drop` leaks the ORT sessions to the process end.
- **A close with marks pending or failed is held** (`CancelClose`, retry, then `Close`; else the exit card after `EXIT_WAIT`).
- `on_exit` flushes and joins the writer, **then** carries out a pending deletion – keep that order.

### Compare mode and deletion
- `pinned` is the left photo, `current` the right; both share one `Zoom`; the loader keeps the pinned photo cached.
- `A` rejects the right photo, `D` the left, then compare ends (`photos::choose`); `Shift+A` / `Shift+D` keep comparing with the next photo (`next_contender`, path taken before the mark; already rejected passed over; forward only). Nothing is deleted in compare mode.
- `metadata::Rating` (`Unrated` / `Rejected` / `Stars`) is the one rating type everywhere; index stores NULL / −1 / 1–5.
- `DeleteQueue`: hidden immediately, 5 s countdown restarted per deletion, `Esc` / `Ctrl+Z` cancel all. Then a worker sets photos aside into `.originals`; `Db::record_deletion` moves their `files` rows. `Esc` priority: edit session → deletions → split view → zoom → four-up → compare → grid/scenes → fullscreen → notice.

### One action at a time
- `app::gate::blocked(change, activity)` is pure and tested. **Every action that changes a photo asks `allowed()` first**; menu rows use `menu_block()` and are greyed out with the reason.
- A photo being moved takes nothing; one being copied still takes marks; an edit in the writer blocks everything but marks; an open straighten/crop session blocks turns, `Ctrl+Z`, Edit elsewhere, copy/move.
- `filelock::FileLocks` is shared by writer, loader, analysis, edit render, copy/move: hold a path only around the I/O itself, never two at once; the UI thread never waits there. The analysis retries a photo (`Outcome::Retry`) when a mark is queued or its write generation changed.

### Languages
- Every user-visible text lives in `i18n::Texts`; each language file is one full struct literal (a missing text is a compile error). Texts with values are non-capturing closures as `fn` pointers. Add a text: field in `mod.rs` → all five files → `texts_show_their_values` if it takes values; `tools/i18n_edit.py spec.json` does all files at once.
- The language is a global atomic (`i18n::t()`), **English until `CernoApp::new`** applies the saved/system one. **Tests must never call `i18n::set`**; test formatting through the `format_*` helpers that take `&Texts`.
- `metadata` stays language-free; the UI localises. Model/backend names and file names are not translated. Wrapped text goes through `i18n::keep_together`.

### Info bar, view
- Info bar centre: stars (with the prediction hint), Aesthetics and Sharpness meters; the centre is as wide as its widest variant (`widest_centre`) so nothing moves while browsing. Left and right parts drop whole parts when they don't fit (`fit_parts`), never clip mid-word.
- `view::build` returns visible paths, `SeriesPlace`s, duplicate originals and `grouped` (capture-time sort only). Capture time ascending, no-time last; inside a series sharpest first, rejected last.
- Session maps (`Marks::ratings`, `labels`, `descriptions`) override the index until the writer flushed. A mark remembers the next path before the rebuild.

### Keyboard details
- `Tab` never reaches egui: `raw_input_hook` removes it (else `Space` would click a focused widget).
- Key tests run `read_keys` headless (`app::keys::tests::read`); send `Event::ModifiersChanged` along with the key.
- Digits (plain, Shift, Alt) are read by **physical** key (`keys::digit_key`, `alt_digit`) – layouts differ (AZERTY, German `Shift+0` = `=`). Key repeats don't rate.
- While a text field has the cursor, `handle_keys` does nothing.

### Mark writes
- UI updates at once; the writer debounces per file and merges rating, label and description into **one** ExifTool call, leaving out values that already match (nothing changed → dates untouched).
- `XMP-xmp:Label` English name or deleted. Stars: `XMP-xmp:Rating`, plus Microsoft rating tags **only if the file already has them**; `0` deletes; rejected = −1 and clears the Microsoft tags.
- Comment/keywords: read from XMP, else IPTC IIM; written with `-use MWG` (`MWG:Keywords`, `MWG:Description`) plus `-charset iptc=UTF8 -IPTC:CodedCharacterSet=UTF8` – which deliberately **creates IIM** where there was none.
- Lightroom's `lr:hierarchicalSubject` is only kept in step: a removed keyword takes paths that **end** in it along (`rating::args::kept_hierarchy`); a parent inside a longer path stays.
- IIM length limits (keyword 64 B, comment 2000 B) are **shown, never enforced** (`metadata::exceeds_iim`). No rights fields (creator, copyright, location …) are written.
- `Ctrl+Z` restores with `write_in_place` + file-time snapshot, then writes the marks the file had just before.

## Roadmap

Done: viewer, HEIC, index, analysis modules, culling features, model downloads, packages in CI. Open:
- Verify face detection / eye sharpness on real portraits.
- Linux verification (build, libheif, WebGPU on AMD/Vulkan).
- Before each release: `cargo audit`, check the vcpkg pin for libheif/libde265 fixes, record which ONNX Runtime binary `ort`'s `download-binaries` fetched (`ort-sys`'s `build/download/dist.tsv`).

## Release

1. Raise the version in `Cargo.toml` (the only source).
2. In the same commit: `CHANGELOG.md`'s `[Unreleased]` becomes `## [X.Y.Z] – YYYY-MM-DD` under a new empty `[Unreleased]`, and the AppStream metainfo gets `<release version="X.Y.Z" date="…">` (with its release page as `details` URL) at the top of `<releases>` – the `scripts` job checks both. On a `release/X.Y.Z` branch the metainfo line may come with the bump with a placeholder date (accepted only off `main` and off tags) and the heading later. A visible UI change also renews `screenshots/cerno-main.jpg`.
3. Push `main`, wait for a green CI.
4. Only when the owner says so: `git tag -a vX.Y.Z -m "Cerno X.Y.Z"`, `git push origin vX.Y.Z`.
5. CI builds, attests and drafts the release (stops when the tag differs from `Cargo.toml` or the CHANGELOG lacks the section). Check notes, files, `SHA256SUMS.txt`. Before re-running a release job that got that far: `gh release delete vX.Y.Z --yes` (the tag stays).
6. Publish: `gh release edit vX.Y.Z --draft=false --latest` (the AppImage's update info follows *latest*).

## Gotchas

### Data safety and tests
- **The index at `%LOCALAPPDATA%\Cerno\data\cerno.db` is the user's live data.** Never delete it; DB tests use in-memory databases. **Smoke tests always set `CERNO_DATA_DIR`** to a temp folder (`directories` ignores `%LOCALAPPDATA%`; without it marks land in the live index and train the prediction), and run on generated photos, never real ones.
- Deletion tests pass a recorder as `DeleteQueue`'s `Remover`; `originals` tests use their own temp folders.
- Scripted smoke tests: post keys with `PostMessage(WM_KEYDOWN/UP)` to Cerno's window, capture with `PrintWindow(PW_RENDERFULLCONTENT)`. Modifiers: `AttachThreadInput` + `SetKeyboardState`. Arrows/Home/End/PgUp/PgDn/Ins/Del need the extended-key bit, or they arrive as numpad digits (→ marks!). **Never `SendKeys`.** Posted mouse messages don't work – test pointer behaviour headless (`Context::run_ui`, call `textures_delta.clear()`). Wait for a window titled "Cerno".
- **UI tests build their context with `theme::test_context()`** (the Semibold family must be bound). Its widths aren't Segoe UI's: check fixed-room labels on screen in all five languages.
- An `egui::Area` is invisible in its first frame: run two frames before looking at shapes. Tooltips need the pointer to rest.
- **Tests that build GStreamer pipelines take `playback::test_lock()`.**
- Don't click map links in scripted tests (they send coordinates out); `ui::details::tests` clicks headless.

### Files, metadata, ExifTool
- **EXIF orientation applies to everything but HEIC** (libheif applies `irot`/`imir`); video frames are turned by GStreamer.
- **Never `fs::read` a video** – it goes by path to a frame helper and by URI to GStreamer. Sidecar-format metadata through `metadata::read_for` / `read_sidecar`.
- `exiftool::locate` returns an absolute path and `spawn` starts exactly that file (only absolute `PATH` entries – an empty entry means the working directory).
- ExifTool needs `-charset filename=UTF8` (umlaut paths). `-P` shifts times by microseconds – `filetimes::Snapshot::restore` stays.
- **In-place writes are deliberately not atomic** (`-overwrite_original_in_place` keeps file identity and dates); a crash mid-copy can corrupt that one file.
- No torn reads: every read and write holds the file in `FileLocks`. Another program can still rewrite a file – don't cache decode failures forever.
- A downloaded ExifTool older than the pin is offered the pinned one (`tools::EXIFTOOL_VERSION` must match `EXIFTOOL.name`); the swap pauses the writer (`rating::Pauser`). Bumping needs version, size, SHA-256 and `EXIFTOOL_VERSION`.
- **Fingerprints depend on decoder and resizer** (zune-jpeg, libheif, fast_image_resize, sRGB conversion): a change only means re-analysis. Never use `DefaultHasher` for them.
- JPEG decode converts to sRGB once (`decode::to_srgb`); straighten/crop decode without it (`decode_for_edit`) and copy `-ICC_Profile` explicitly.
- **`decode::check_size` (≤ `MAX_PIXELS`, 200 MP) must stay in front of every decode path** – decoders allocate the whole frame from the header first.
- zune-jpeg decodes truncated JPEGs leniently (intended).
- Kept originals look deleted to an index without their `backups` rows (fresh `CERNO_DATA_DIR`).
- Linux always updates `ctime`; size-and-date sync tools (`rsync` without `-c`) may miss rating changes.
- When the index can't be opened Cerno runs on an in-memory DB and blocks edits (`Blocked::NoIndex`).
- `Library::open` makes the path absolute first; the index keys files by full path.

### Models and downloads
- Downloads (`download.rs`): HTTPS only on every hop, 60 s stall timeout (`download::STALL`), byte cap, `.part` resume via `Range` (`resume_plan`), system certificate store (`platform-verifier` – antivirus HTTPS scanning). Only bytes with the right size and SHA-256 are renamed into place. URLs name a commit or `models-1`, never `main` (`manifest::tests`). `CERNO_TEST_DOWNLOAD=1|full|exiftool|update cargo test -- --ignored …` checks the hosts.
- "Delete models" runs in the background and is refused while a download runs; `run_model` never loads a `Missing` / `Removing` model.
- DirectML is registered with `error_on_failure()` and falls back to CPU explicitly; needs `with_memory_pattern(false)`. **All sessions share one ORT environment** (`analysis::models::session_builder`) – a second `ort::init().build()` fails.
- ONNX outputs are picked by shape, not name. SigLIP: squash to 384, `v/127.5 − 1`, embeds already normalised. CLIP: short side 224 + centre crop + mean/std. YuNet: 640², BGR 0–255, letterboxed, CPU.
- Exposure counts a pixel as blown only when **all** channels ≥ 250.

### UI and egui
- **`run_frame` lays out twice** (before and after `handle_keys`): a key can empty the view, so nothing may index `view.paths` unchecked.
- **Panics**: `decode::catch_panic` around decode jobs and `CernoApp::guarded` around keys and commands rely on `panic = "unwind"` – never set `abort`. `main.rs` denies `unwrap_used`, `expect_used`, `panic` outside tests.
- **Nothing in the filter bar or menu bar changes width or appears/disappears while used** – greyed out instead (fixed combo widths, count slot, `Bars::toolbar_held`).
- A label in a `left_to_right` layout never wraps – wrapped text goes into a `top_down` block with `Label::wrap()`. Selectable labels eat tooltips: details rows use `.selectable(false)`.
- **egui ≥ 0.37 needs `monochrome_emoji_fonts`** for ★ (`Cargo.toml` names `egui` for that).
- The crop gesture senses drags only (`ui::edit::pointer_area`), so the drag starts at the press.
- Modal help/list/cards: `handle_keys` returns early; they are `Order::Foreground` areas; card height is remembered from the last frame.
- `with_maximized(true)` is unreliable on scaled Windows displays; only the command is used.
- German layout: `Shift` with `+` / `−` reports `CloseBracket` / `Slash` (crop's `size_keys`).
- The only `rfd` dialogs are the folder pickers and the program picker.
- Icon: change `assets/icon-source.jpg` and regenerate with `tools/make_icon.py` – never edit `icon.png` / `icon.ico` by hand.

### Build, packaging, video
- **A git worktree builds `--features heic` only with `target/vcpkg` linked** (junction to the main checkout's).
- **HEIC on Windows is dynamically linked** (LGPL libheif/libde265 DLLs + licences beside the exe); never switch back to `x64-windows-static-md`; x265 (GPL) stays out. Linux links the system libheif. ExifTool is never distributed; the V2.5 head stays out of the binary.
- **The Windows install directory is the data root** (`%LOCALAPPDATA%\Cerno`, data in `data\`); the uninstaller deletes data only with the opt-in box – `test-installer.ps1` asserts it.
- **The VC++ runtime is not part of Windows**: `build.ps1` copies it app-locally and fails on imports found only in System32. `crt-static` is no option.
- **The Windows package carries its own GStreamer** (pinned in `install-gstreamer.ps1`, CI caches the installer – bump the key with the version); licence-checked plugins only (`mpegpsdemux` left out). `playback::configure_environment` (first in `main`) sets the plugin path, registry in the data folder, `GST_REGISTRY_FORK=no`. A dev exe uses the per-user GStreamer (`engine::use_installed`). FFmpeg in GStreamer's MSVC build must stay LGPL – check with every new GStreamer version.
- **Delay-loaded DLLs** (GStreamer, GLib, DirectML, libheif, OpenGL, dbghelp): a missing one kills Cerno at its first call (0xC06D007E, no crash log), so `dlls::load` checks first. A new delay-loaded DLL needs the same.
- **Linux**: HEVC decoding is a dlopen'd libheif plugin – the `.deb` depends on `libheif-plugin-libde265`, the AppImage bundles it (`LIBHEIF_PLUGIN_PATH` in our own `AppRun`, patchelf rpath). The AppImage bundles no GStreamer/GLib/FFmpeg (`build.sh` fails if one slips in). winit/wgpu/xkbcommon are dlopen'd, so the `.deb` lists them by hand. The desktop file stays `cerno.desktop` (= app id) and lists no video types. CI install tests: `.deb` on Ubuntu 24.04 + Debian 13, AppImage also on Fedora 44 and Arch.
- The `.zsync` lands in appimagetool's working directory – `build.sh` runs it inside `dist/linux`; keep the file name pattern.
- `cerno --version` on Windows prints only through a redirect (GUI subsystem); `build.ps1`'s PE check refuses a non-release `cerno.exe`.
- cargo-packager config is camelCase with `deny_unknown_fields` (`installer-mode`, `custom-language-file`); `packaging/windows/*.nsh` stay **UTF-8 with BOM**.
- **Video playback**: Windows refuses to move a playing file – copy/move/delete/exit wait for `Player::stop`'s `Release`. Take the stride from the mapped frame (D3D12 pads rows). No sound device is no error (restart with `fakesink`). `Player::status` reads the bus only after the start thread is through (`start_done`); never `timed_pop_filtered` on that bus. A pipeline may stall before its first frame: `FIRST_FRAME` timeout, D3D12 → CPU, CPU retries once; nothing is remembered. Videos show only Details › File (`playback::probe`).

### CI
- **Actions pinned by commit**: every `uses:` in `ci.yml` names a 40-hex commit with its version as a comment; `grep -n '@v[0-9]' .github/workflows/ci.yml` must find nothing. To bump: `gh api repos/<owner>/<repo>/commits/<tag> --jq .sha`, check the exact tag with `gh api "repos/<owner>/<repo>/tags?per_page=100" --jq '.[] | select(.commit.sha=="<sha>") | .name'`, replace every line of that action, run the workflow by hand. `dtolnay/rust-toolchain` is a `master` commit with `toolchain: stable` – never the `stable` branch's commit. zizmor fails the run on any finding; its ignore comment goes after the version comment (`# v6.1.0 # zizmor: ignore[cache-poisoning]`).
- **The `scripts` job** lints what isn't Rust: actionlint (+ shellcheck on bash `run:` blocks), zizmor, shellcheck over `*.sh` and `AppRun`, PSScriptAnalyzer over `*.ps1`, `desktop-file-validate`, `appstreamcli validate --no-net --strict`, and the CHANGELOG/metainfo/version check. Silence a finding where it occurs with its reason, never in a settings file.
- In a bash `run:` block a negated command (`! grep …`) never fails the step – write `if grep …; then exit 1; fi`.
