# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), versioning follows [Semantic Versioning](https://semver.org/).

---

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
