# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added

- Straighten (`S`, fine grid, wheel or arrows, `Shift` finer) and crop (`R`: Original, 3:2, 4:3, 16:9, 1:1; `A` changes the ratio, `X` flips) for JPEGs, `Ctrl+←` / `Ctrl+→` for lossless quarter turns via the EXIF orientation. `Enter` applies, `Esc` cancels.
- **Originals are kept**: before any of these edits the file is copied into Cerno's data folder (`backups`), and `Ctrl+Z` (also Edit › Undo) writes the newest copy of the current photo back – same file, same dates, with the rating and colour label given since. Copies are deleted after 30 days.
- Action menu (`Ctrl+M`, "Action" in the filter bar): copy or move every photo the filter shows to another folder, or delete them with the usual countdown.
- "Models & data" in the menu: where each model runs, the models folder, download, reset For you and delete the models – no longer part of the details panel.
- Colour labels are part of the filter (small squares in the filter bar, rows with a colour dot in the menu); "Show all" resets them too.
- The details panel shows the photo's size and load time.
- JPEGs are converted to sRGB on decode (Adobe RGB, Display P3 by a fixed matrix, other profiles through a colour engine).

### Changed

- The burger menu is grouped (view, sort, filter, edit, photo, colour labels, models & data, language, help) and fully keyboard-operated: arrows, `Enter`, `→`/`←` for submenus, a letter jumps, `Esc` closes the submenu first. Switches show a box, choices a tick; long rows end in "…".
- Keys: `F` and `F11` full screen, `T` the filter bar (Lightroom: F full screen, T toolbar). "Sort and filter" is called "Filter bar" everywhere.
- Info bar: `L / V / ☆` without the "Aesthetics" label – For you has an outline star, filled stars are only your rating. Size and load time left for the details panel; parts that don't fit are left out whole, and the side columns no longer move while browsing. Buttons are 32 px.
- Filter bar: one line; the status only shows while something needs attention (analysis running, model missing, loading or failed); hidden boxes fade at the edge.
- "Probably blurry" needs the folder's blurriest 20 % **and** an absolute ceiling, so a folder of sharp photos gets no warnings. The filmstrip mark is a painted icon; every mark of a cell is named in its tooltip.
- Duplicates: the original is the file whose name the others extend ("IMG_1.jpg" for "IMG_1 - Kopie.jpg"), else the only marked one, else the first in folder order. Before, the copy often counted as the original.
- No system dialogs apart from the folder picker: an in-app card confirms the model download, resetting For you and deleting the models; copy, move and delete of the selection ask nothing (the folder choice or the countdown is the confirmation). No download dialog at start – a one-time hint once a folder is open.
- Hints fade after about 5 s; errors stay until `Esc` or a click.
- The start screen is a small card with the five keys to begin with; `H` shows the full help (with its own "Edit" section), also over the start screen.
- Six font sizes instead of fifteen; section titles and the series line are muted, the accent marks the current photo and interaction only; label blue is clearly different from the accent.

### Fixed

- One action at a time on a photo: a photo being moved takes no stars, labels, edits or deletion, one being copied no edit or deletion; while a straighten or crop is open, quarter turns, `Ctrl+Z` and copy/move wait for `Enter` or `Esc`. A short hint says why, and the menu greys those rows out. Before, stars given during a move could be lost and an edit could land on the neighbouring photo.
- Straighten and crop keep the photo's colour profile: an Adobe RGB or Display P3 JPEG was written back as untagged sRGB, with its strong colours clipped for good.
- Digits rate and set colours on French and Belgian keyboards (AZERTY) too: the keys count by their place, not by the character they type. On AZERTY the 6 key no longer also zooms out. A held digit rates once instead of on every key repeat.
- Deletions carried out when the window closes count for For you, like the ones during the session.
- If the index can't be opened, its error stays on screen instead of being replaced by the next hint, and editing is off (its kept originals would be lost track of).
- `cerno IMG_0042.jpg` started inside the folder opens that photo instead of the first one; paths are stored absolute.
- "Delete models" no longer freezes the window while an analysis runs and no longer fails because the file is in use; the models card says "removing…" meanwhile.
- A model download that stalls gives up after a minute with an error instead of hanging at the same percentage until a restart; a failed download leaves no partial file behind.
- If the face detector can't be loaded, photos are no longer decoded again on every visit.
- A straighten or crop is cancelled (with a hint) when another photo becomes current underneath it, e.g. because a copy finished.
- No torn reads or copies: reading, copying and writing a photo take turns, so the analysis, the viewer and a copy never see a file halfway through a rating write.
- A photo rated while it was being analysed is analysed again shortly afterwards instead of staying without values until the folder is reopened, and the index no longer keeps the rating it had before.
- The explanations in the details panel (`I`) wrap inside the panel; before, they ran out of it and pushed every value out of view.
- The window fills the screen at start (it stayed at its restored size while flagged as maximized on scaled displays).
- `Ctrl+←` / `Ctrl+→` no longer also step to the neighbouring photo.
- In name order without filters, duplicate marks and series appear as the analysis finds them, not only after sorting or filtering.
- Compare mode (`C`, `A`, `D`) no longer restarts the decoding of the photos on screen when the list itself stays the same; both photos appear clearly sooner.
- That refresh no longer restarts decoding every 2 s: a 100 % zoom on a large photo finishes again, and the analysis no longer pauses each time.
- The menu no longer crashes when it gets shorter while it is open (e.g. a deletion runs out) and a key is pressed.
- Filmstrip thumbnails stay at 400 textures while the analysis runs through a large folder; before, every analysed photo kept one in graphics memory.

### Removed

- "Best of each series" and its `+n` count: a series always shows all its photos. Hiding them let the next frame of a burst slip into a rejected photo's place unnoticed, so after "Delete rejected photos" the look-alike frames seemed to be rejected photos that had come back. Series stay marked in the filmstrip and the info bar, and capture-time order still puts the sharpest first.

## [0.9.0] – 2026-09-28

### Added

- Sort by capture time (ascending). The time comes from `DateTimeOriginal` plus sub-seconds, or `DateTimeDigitized` when the original is missing. Photos without a time stay at the end.
- Colour labels Red, Yellow, Green, Blue and Purple, written as the English `xmp:Label` (`6`–`9`, purple from the command palette; the same key again clears the label, `Shift+6`–`9` sets it and moves on). Names that a German, French, Spanish or Italian Lightroom wrote are recognised. A dot in the info bar and a stripe on the filmstrip show the colour; the top bar can filter by it.
- Series: photos at most two seconds apart belong together. Capture-time order puts the sharpest non-rejected photo first. The filmstrip separates series, and the info bar shows the position inside the current one. "Best of each series" (command palette) keeps only that photo, with a `+n` count of the rest.
- Optional subfolders (command palette, off by default). Hidden folders are skipped, directory shortcuts are not followed. A photo in a subfolder is shown as `100CANON/IMG_0001.JPG`.
- Exact duplicates (same pixels and size) are marked: a badge on the filmstrip, "Duplicate of …" in the info bar, and a "Only duplicates" filter. Nothing is rejected or deleted on its own.
- Auto-advance (command palette, off by default): `0`–`9` and `X` then move to the next photo, the same way their `Shift` variants already did. A short hint in the info bar shows while it is on.

### Changed

- An existing index picks up capture times by re-reading the file only – photos that were already analysed are not decoded again.

## [0.8.1] – 2026-09-27

### Changed

- Aesthetics scores (LAION, V2.5) are shown on the star scale 0–5 (2 → 0, 8 → 5), so they compare directly with your stars and the personal taste value. Stored scores and sort order are unchanged.

## [0.8.0] – 2026-09-27

### Added

- Command palette (`Ctrl+K`): type a few letters to find and run any command – sorting, filters, panels, compare, zoom, language, help – with arrows and Enter.
- `Shift+1`–`5` / `Shift+0`: rate and go to the next photo (like Lightroom). `?` opens the help.
- "Rejected" as a rating (`X`, `Shift+X` rejects and moves on): written into the file as `xmp:Rating = -1` (the XMP standard's value), shown as a red cross, dimmed in the filmstrip, filter "Rejected", sorted last. Rejects count as 0 stars for the taste model. The palette offers "Delete rejected photos (n)" with the usual countdown.
- Details panel in three stages: `I` steps through values → values with explanations → off.

### Changed

- Neutral grey surfaces instead of blue-grey, so no tint around the photo influences colour judgement (deliberate deviation from the shared design system).
- Compare mode rejects the losing photo instead of deleting it (`A` / `D`).
- Lightroom key layout: `Tab` details panel, `Shift+Tab` all panels, `T` top bar, `F6` filmstrip, `Ctrl+L` language (was `P`, `I`, `B`, `T`, `L`).
- By default only the photo, the filmstrip and the info bar are shown; top bar and details panel are one key away.
- Info bar: the three aesthetics values carry small labels (`L 6.1 / V 6.5 / ★ 2.4`); the language flag only appears while switching, and the panel buttons' tooltips name their keys.

## [0.7.0] – 2026-09-27

First numbered version.

### Added

- Five UI languages: German, English, French, Spanish, Italian. Starts in the system language, `L` switches (a flag fades in), the choice is remembered.
- Help page (`H` / `F1`) with all shortcuts and a short introduction; the start screen shows the same page with an "Open folder" button. Version number on both.
- Plain-language explanation under every value in the details panel.
- Info bar: the three aesthetics scores side by side (`LAION / V2.5 / personal ★`), the current zoom level while zoomed, EXIF digital zoom, and a map pin for photos with GPS coordinates that opens Google Maps.
- Buttons in the info bar show or hide the top bar, details panel and filmstrip (also `B`, `P`, `T`; `I` for all three). The info bar always stays; all states are remembered.
- Mouse wheel over the filmstrip steps through the photos.
- Native viewer (egui + wgpu) for JPEG, HEIC behind the `heic` cargo feature (verified with a real HEIC sample, incl. rating round trip).
- Background prefetch of the neighbouring photos, decoded at monitor resolution and kept as GPU textures.
- EXIF orientation is applied to JPEGs.
- 1–5 star rating with the number keys (`0` clears). Stars are written to `xmp:Rating` via ExifTool in the background (debounced), file modification and creation dates are preserved.
- Zoom: `Z` / double-click toggles 100 %, mouse wheel and `+`/`-` zoom around the cursor, drag pans. Full resolution is loaded on demand as GPU tiles; zoom and position persist across photos.
- Filmstrip with thumbnails, star counts and a "probably blurry" marker (`T` toggles it).
- SQLite index in the local app data folder: thumbnails, scores and CLIP embeddings, keyed by an image-content fingerprint so renamed or re-rated photos keep their data.
- Background analysis (nearest first, paused while browsing): tile-based sharpness (shown as percentile within the folder) and aesthetics (CLIP ViT-L/14 + LAION predictor) on the GPU via DirectML, CPU fallback. The 1.2 GB model is downloaded on request from the toolbar.
- Toolbar: sort by name, rating, aesthetics or sharpness; filter by stars; hide the blurriest 20 %; analysis progress and aesthetics backend. Settings are remembered.
- Two-row info bar with capture date, size, scores, exposure (focal length, aperture, shutter speed, ISO), camera and lens.

- Compare mode: `C` pins a photo on the left, `A`/`D` keep left/right and delete the other; the winner stays on the left, the next photo moves in on the right; zoom is shared.
- Deleting without confirmation: `Delete` hides the photo, a 5-second countdown (restarted by every further deletion) then moves the queue to the trash in the background; `Esc` restores everything waiting.
- Larger score display in the info bar: values with bars.
- Start-up log lines (`start-up: … ms`).
- Second aesthetics score: SigLIP so400m + Aesthetic Predictor V2.5 (DirectML), used in the info bar when installed; sortable. Model files go into the models folder (no download yet; the AGPL head is not bundled).
- Personal taste model: ridge regression on the stored CLIP embeddings, trained in the background on your star ratings plus deleted photos as 0 stars (from 15 examples); predicted stars in the info bar, details panel and as a sort option.
- Eye sharpness: built-in YuNet face detector, sharpness measured around the eyes; for photos with a face it decides "probably blurry", Hide blurry and the sharpness sort.
- Exposure check: share of blown highlights and crushed shadows, with warning colours.
- CLIP attributes (zero-shot, CLIP-IQA style): quality, sharpness, lighting, composition, noise, colourfulness.
- Details panel (`P`) with all values and the model states; scrolls when the window is low.
- The index database migrates in place; existing ratings and scores are kept and only the new modules are computed.

### Changed

- `I` no longer hides the info bar.
- Capture dates follow the language (e.g. 12.09.2026 in German).
- Info bar text that doesn't fit leaves out whole parts (focal length, lens) instead of being cut off; the centre adapts to the width of the scores.
- Faster start: one graphics backend (DX12 on Windows, Vulkan + GL fallback elsewhere; `WGPU_BACKEND` overrides), and the first photo decodes while the window and GPU start up. Photo on screen after ~330 ms instead of ~470 ms (24 MP JPEG, measured on an RX 6700 XT system).

### Fixed

- Arrow glyphs showed as boxes: the design system's UI font (Segoe UI; DejaVu/Noto on Linux) is now loaded from the system.
