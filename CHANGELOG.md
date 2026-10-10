# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

## [1.10.0] – 2026-10-10

A faster start, `Ctrl+Z` for marks, a warning for cut-off JPEGs, a shorter help, arrows to browse with the mouse and an update check.

### Added

- **`Ctrl+Z` takes back marks** – stars, rejections and colours of this session, newest first, on whichever photo they were: Cerno shows that photo, gives it back what it had and says so (`Undone – IMG_0012: 3 stars`). Straighten, crop and turns of the session take their turn in the same order. In a RAW + JPG pair both files go back. The menu's *Undo* row names what it would take back. A deletion that is still counting down and a deleted photo on screen come first, as before; an original kept in an earlier session comes last.
- **Incomplete JPEGs are marked** – a JPEG cut off while it was copied or downloaded shows its missing part grey. Cerno now says so: *File incomplete* beside the stars, a torn page on its cell and a row in Details › File, and the filter bar has a box for them next to *Blurry* and *Duplicates*. Such a photo is never among the *Top N*. Nothing is repaired or rejected. Data after the image's end (a motion photo's video) is fine. Photos analysed before are checked once more in the background, without decoding them again.
- **Cerno says when there is a new version** – once a day it asks GitHub which release is the latest (one request, nothing about you or your photos is sent) and names a newer one once, with a link in the menu and on *About Cerno*. Nothing is downloaded or installed. *Settings ▸ Check for updates* turns it off; *About Cerno* shows where it stands and checks on request. The installer's first page and a one-time hint say so too. See *Privacy* in the README.
- **Arrows to browse with the mouse** – while the pointer moves over the photo, a round, see-through arrow at each side steps to the previous or next photo; they fade a moment after the pointer rests. Only in the single view, not while straightening or cropping.
- **The start screen says what is still to set up** – while ExifTool or the aesthetics models are missing, one line names them and where to get them (*Ctrl+K › Settings › Models & data*).

### Changed

- **Faster start** – the first photo appears at once in the size of the last session's photo area, instead of being decoded twice. The aesthetics models, the video and HEIC libraries and ExifTool load only when they are needed, and the analysis and the prediction start a moment after the first photo, so they never compete with it. The screen redraws less often while the analysis runs.
- **A shorter, clearer help** – every shortcut is still there, each with a few words; one *Shift+…* row replaces the Shift row of every key. What the rows used to explain in detail – the grid, straighten and crop, the mouse wheel, the four-up view, the file list – is in the tips, under *Good to know* and where it belongs.
- **No faces tab in the details panel any more** – `G` still shows all faces of a photo large; `Ctrl+Tab` steps between the values and the description.
- **Signed Windows packages, prepared** – once the SignPath Foundation has accepted Cerno, `cerno.exe` and the installer of each release are signed (free code signing by SignPath.io), and Windows stops calling the publisher unknown. Until then they stay unsigned. The README has the code signing policy.

### Fixed

- **A video and a RAW of the same name no longer share their marks.** Both kept them in `IMG_1.xmp`, so a star on `IMG_1.MOV` landed on `IMG_1.CR3` too. The RAW keeps `IMG_1.xmp`; the video (or a BMP) now uses `IMG_1.MOV.xmp` whenever another file of the folder would want the short name, also after copying, moving, deleting and putting back. A sidecar the two shared until now belongs to the RAW, so the video's marks may seem to have moved to it.
- ExifTool and the video helpers end with Cerno, also when Cerno is ended by force – each start used to leave an ExifTool behind.
- When the background writer for marks stops, Cerno says so instead of losing the following marks without a word.
- An unexpected error in a key or menu command no longer ends Cerno: it is written to `crash.log`, a message says so, and Cerno goes on.
- A deleted photo's colour label no longer lingered for the session.
- Files over 2 GB are no longer read as photos; a cross-drive move writes the copy to disk before the original goes; a photo put back goes only into the folder above its `.originals`.

## [1.9.1] – 2026-10-06

Group photos without doubled faces, and a photo's path with one click.

### Added

- **Copy a photo's path** – a small button in the details panel's *File* title puts the photo's full path on the clipboard; its tooltip shows the path.

### Fixed

- In a group photo, the faces (`G` and the Faces tab) no longer show two people twice: the face detection sometimes found a "face" between two heads, with an eye of each. Such boxes are now left out – also for photos analysed before, without analysing them again.

## [1.9.0] – 2026-10-05

No more ffmpeg, ExifTool with one click, an About page, and the details panel's tabs on `Ctrl+Tab`.

### Added

- **About Cerno** – the help (`H`) has a third tab with the version, the licence and the source code, and links to report a problem or suggest an idea on GitHub (version and system filled in) and to open the data folder with `crash.log`.
- **ExifTool with one click (Windows)** – Cerno writes stars, colours, comments and keywords with ExifTool. When it is missing, Cerno now offers to download it (11 MB, from the official source, checked before it is used): once when a folder opens, at the first star, and in *Models & data*. Nothing to install by hand, no `winget`.

### Changed

- **`Ctrl+Tab` steps through the details panel's tabs** – values, description, faces (`Ctrl+Shift+Tab` back) – instead of `B` and `G` for two of them. `G` now shows all faces large over the photo (it was `Shift+G`); on the description tab `Enter` puts the cursor into the keyword field.
- **The prediction no longer learns from deleted photos** – a good photo is often deleted only because there are too many alike. Only your stars and rejected photos teach it now; what it had learned from deletions is forgotten.
- **Another photo starts whole** – zooming in on one photo no longer carries over to the next. Compare mode and the four-up view keep their shared zoom.
- **Faces at full width** – the details panel's Faces tab shows each face as wide as the panel, without a label.
- **No ffmpeg needed any more** – a video's still frame and filmstrip picture now come from GStreamer, which Cerno already uses to play videos – faster than before (a filmstrip picture in about 140 instead of 230 ms). Cerno takes them in separate helper processes, so a broken video can't take Cerno down.
- **Without ExifTool, stars and colours are greyed out** with the reason, instead of showing and then being lost. On Linux the hint names the command that installs it. An ExifTool older than 12.24 is not used: it can run code hidden in a photo.

### Fixed

- `Ctrl+Z` right after deleting brings the photo back, like `Esc`. It used to act on the next photo – and could put that photo's original over its edit.
- Rating a RAW without ExifTool no longer leaves an empty XMP file behind that hid the camera's own stars.

## [1.8.0] – 2026-10-05

Four photos at once, and a RAW with its JPG as one photo.

### Added

- **Four photos at once** – `Shift+C` shows four photos of the view side by side, from the current one. A frame marks the current photo: stars, labels, `X` and `Delete` act on it, `←`/`→` move the frame a photo, `↑`/`↓` a row, a click puts it on a photo. Zooming and `Ctrl+1` show all four at the same place. `Shift+C` again or `Esc` shows the single photo; comparing two with `C`, `A` and `D` stays as it is.
- **RAW + JPG as one photo** – a RAW (or DNG) and a JPG of the same name appear once, as the JPG, with a `RAW+JPG` mark. Stars, colour, comment and keywords go into both files; copy, move and delete take both, and a deleted pair comes back together. Straightening, cropping and turning change the JPG only. Where the RAW's XMP file holds other marks, the info bar says so (`RAW+JPG – RAW: 5 stars, Red`) – nothing is changed until you set a mark yourself. *Settings ▸ RAW+JPG as one photo* turns it off.

## [1.7.0] – 2026-10-05

Faces at a glance, the photos a client chose, and cameras whose clocks differ.

### Added

- **Faces at a glance** – `G` opens a *Faces* tab in the details panel with every face of the photo, left to right, and a note where the eyes are probably blurry; `Shift+G` shows them all large over the photo, for group photos. A click (or the face's number in the grid) zooms to it.
- **The photos a client chose** – *Filter ▸ By file list …* takes a pasted list of file names or numbers (one per line, or separated by commas or semicolons; case and extension don't matter, `345` finds `IMG_0345`) and shows just those photos. While you type it says how many it finds and which names it can't; the filter bar then shows `List 23/25 ×`, and a click shows every photo again.
- **Cameras whose clocks differ** – when two cameras were off by a few minutes or hours, their photos stood apart in capture-time order and formed no series together. Compare two photos taken at the same moment and choose *This photo ▸ Match right camera to left*: every photo of the right camera in this folder moves by that offset. *Visible photos ▸ Camera time …* shows each camera with its offset to adjust or reset. The info bar shows the corrected time with the offset, its tooltip the file's own; the files are not changed.

## [1.6.0] – 2026-10-05

Deleted photos come back, the keyboard crops, and the help explains how to work with Cerno.

### Added

- **Deleted photos can come back** – a bin box beside ✕ in the filter bar (and *Filter ▸ Deleted*) shows the photos deleted in this folder; they still lie in the hidden `.originals` folder. `Ctrl+Z` puts the photo shown back into its folder, *Put back (n photos)* in the action menu every one the filter shows. When its name is taken, a photo comes back as `name (2).jpg` – nothing is overwritten. Deleted photos only look: no stars, labels or edits until they are back. With ✕ the box shows everything sorted out; it is greyed out in a folder without deleted photos and never saved.
- **"without ✕"** in the filter bar (and *Filter ▸ Hide rejected*) hides the rejected photos and keeps every other one – the other way round from ✕, which shows only them.
- **Crop with the keyboard** – the arrows slide the frame, `+` and `−` make it larger or smaller about its centre; Shift makes each step one pixel, as in straighten.
- **Tips in the help** – the help page has two tabs, the shortcuts and tips on working with Cerno (two passes, series, the best photos, colour labels, deleted photos, the prediction); `←`/`→` or a click switches.
- **Details › File shows the file size**, for photos and videos, and for a JPEG its quality and colour subsampling (≈ 92 · 4:2:0) – estimated from the file, which does not store the quality itself.

### Changed

- **A RAW file says that its preview shows** – what Cerno shows of a RAW is the JPEG the camera embedded: the info bar says *RAW preview* (and *Zoom 100 % of the preview*), Details › File marks the size as the preview's and explains it, the histogram and exposure say *(preview)*.
- **The menus show what a row sets** – stars before the star ratings, a dot before the colours, ✕ and the bin before rejected and deleted, as in the filter bar. "No colour" sits on top like "No stars" (German: „Ohne Farbe“).

## [1.5.2] – 2026-10-03

A people filter, tidier video details, a few common keys and a help page that fits on one screen.

### Added

- **People filter** – two small icons in the filter bar (and *Filter ▸*) keep the photos with or without people. Cerno finds them by their faces, which it already looks for, so nothing is analysed again; people seen from behind or very small don't count.
- **`Ctrl+U` turns subfolders on and off** (as *Settings ▸ Include subfolders*); a hint says which.
- **`Ctrl+0` shows the whole photo, `Ctrl+1` 100 %**, and `Ctrl+Plus` / `Ctrl+Minus` zoom like `+` / `−` (they used to change the size of the whole window's text).
- **The models card says what the prediction learns from** – how many photos have stars, how many were rejected and how many deleted (those two count as 0 stars), so the number no longer looks like the count of rated photos.
- `CERNO_DATA_DIR` puts the index, settings and crash log into another folder (the models stay shared) – for tests that must not touch your index.

### Changed

- **The help page is sorted anew** – eight short sections (browse, rate, sort out, video, view, panels, edit, more) in columns of about the same length – three on a wide window, so nothing scrolls – with the new keys.
- **"For you" is now called "Prediction"** („Vorhersage“, « Prédiction », «Predicción», «Previsione»).

### Fixed

- **A video's details show only what applies to it** – the aesthetics, prediction, sharpness and exposure rows waited forever ("analysing…"), since videos are never analysed. A video recorded at a variable frame rate (as phones do) shows "variable" instead of no frame rate at all.
- **A video paused or moved right after it started no longer stops** with "Cannot play the video" (since 1.5.1, rare).

## [1.5.1] – 2026-10-03

Videos play without a sound device, and their details show what is inside.

### Added

- **Details › File of a video** – container, duration, the video's codec (and HDR, if it is), size, frame rate and bitrate, the sound's codec, channels, sample rate and bitrate, and the total bitrate. Most MP4 files state no video bitrate; Cerno then takes the total minus the sound and marks it with ≈.

### Changed

- **Jump 5 seconds with `Alt+←` / `Alt+→`** (was `J` / `L`); plain `←` / `→` still go to the previous and next photo.
- **Only `Space` plays a video** – `Enter` no longer does.
- **The play button is a round button with a larger play sign**, without text (its tooltip names the key).

### Fixed

- **Videos play without a sound device** – on a remote desktop or with nothing to play sound on, a video did not start at all ("Cannot play the video"). It now plays without sound, and a hint says so once.

## [1.5.0] – 2026-10-03

Videos play inside Cerno with sound, the second aesthetics model downloads with one click, and copying shows how far it is.

### Added

- **Videos play inside Cerno, with sound** – `Space` or `Enter` plays and pauses the video on screen (`Shift+Space` moves on), `J` / `L` jump 5 seconds back or on, `,` / `.` step one frame, `↑` / `↓` set the volume. A bar under the video shows the time, seeks with a click or a drag and has a speaker and a volume slider; it fades while the video plays. Phone videos stand the right way up, and 4K HEVC from a phone plays smoothly – on Windows the graphics card converts the frames. Another photo, the grid, compare mode, *Edit elsewhere*, copying, moving and deleting stop the video first. The Windows downloads bring their own GStreamer (about 45 MB); on Linux the `.deb` installs it and the AppImage uses the system's. Videos are no longer handed to the system's player.
- **V2.5 by download** – *Menu → Models & data* now downloads every missing aesthetics model with one button: CLIP (1.2 GB, from Hugging Face) and SigLIP + V2.5 (1.7 GB, from this project's release `models-1`). If only V2.5 is missing, the button says *Load V2.5…*. Before, V2.5 had to be put into the models folder by hand, and *Delete models* removed it for good. Each file is checked by size and SHA-256 before it is used. An interrupted download keeps what arrived and continues next time. A folder opened while V2.5 is missing shows a one-time hint, and a finished or failed download says so.
- **Progress while copying or moving** – copying or moving what the filter shows puts a bar at the bottom of the photo area: which photo of how many, how much of the size is done, and the name of a large file (a video) while it takes its time. It sits above the deletion countdown and a video's play button.

### Changed

- `Space` on a video plays and pauses it; `Shift+Space` or `→` move on, as `Space` does on photos.
- Videos are no longer zoomed or compared – the zoom showed a still frame that ffmpeg had to make a second time.
- *Delete models* is greyed out while a model downloads, and it also removes unfinished downloads.

### Fixed

- **Model downloads behind HTTPS scanning** – an antivirus that inspects HTTPS (seen with Kaspersky) or a company proxy made the CLIP download fail with "invalid peer certificate". Cerno now checks certificates against the system's store, like a browser; the SHA-256 check still decides whether a file is used.

## [1.4.0] – 2026-10-02

The best photos at a click, a tidier filter bar, and two fixes: no more crash when a mark empties the filter, and a crop frame that moves.

### Added

- **Top 10 … Top 250** – the first box of the filter bar (and *Filter ▸ Best photos ▸*) shows only the best photos of what the other filters leave: 10 highlights, 25 for a preview, 50 for a slideshow, 100 for a photo book, 250 for a gallery. A photo counts with the mean of its stars (or the *For you* guess while it has none), its aesthetics and its sharpness; the best of each burst comes first, then the second best, so a burst can't fill the list. Rejected and blurry photos, duplicates and videos never count. The choice stays while you rate and delete – nothing slips into a rejected photo's place – until a filter changes or *Refresh order* is chosen. It is not saved, never changes a photo, and *Delete* in the action menu is greyed out while it is on.
- **`crash.log`** – a crash is written to `crash.log` in the data folder (thread, place, version; at most 50 KB), since the installed program has no console where the message would show.

### Changed

- **The filter bar reads left to right**: what is shown (photos, videos, Top N), the filters by group, then the sort, the count and *Action*. The filters are compact chips with a line between groups; rejected (✕) and no stars (☆) look like the keys that set them. *Show all* is the × in the count.
- **Filters of different kinds work together** – *4★* and *Blurry* now show the blurry 4-star photos, *5★* and red the red 5-star ones. Boxes of the same kind still add up (*4★* and *5★*). Before, every ticked box added photos.
- The README screenshot shows the current build with the filter bar and the details panel.

### Fixed

- **No more crash when a mark empties the filter** – rating, rejecting, labelling or deleting the last photo a filter showed (for example `0` on the last 2-star photo with *2★* ticked, or `Delete` on the only photo of a folder) ended Cerno. The bars were laid out for the photo that had just left the view. Grid view and stars clicked in the info bar were not affected.
- **The crop frame can be moved and resized** – the first frame is the whole photo, so it had nowhere to move; a drag inside it now draws a new frame. A quick pull from a corner was taken for a move (the drag was decided only after 6 pt of movement); it is now decided where the button goes down, and the corners take the pointer from 16 pt away.
- *For you* could stop updating for the rest of the session when the retraining delay ran out at an unlucky moment.

## [1.3.1] – 2026-10-01

Sharper photos: what you see in the window and at 100 % is now exactly the photo, pixel for pixel.

### Fixed

- **Sharper fitted photos** – a photo that fits the window is decoded for the photo area itself and drawn pixel for pixel, scaled with Lanczos3. Before, it was decoded for the monitor and shrunk a second time by the graphics card, which cost about a quarter of the finest detail. When a panel opens or the window changes size, the photos are decoded again; until then the previous picture stays.
- **100 % really is 100 %** – the photo opened at start and its nearest neighbours were decoded for a 4K screen, and on 4000 px photos `Z` then stretched that picture by 4 % instead of loading the full resolution, which halved the finest detail. Full resolution now loads as soon as a photo is shown larger than its picture, and the neighbours are decoded only once the window's photo area is known.

## [1.3.0] – 2026-10-01

Simpler to read: one aesthetics value, two numbers under the photo, explanations on hover, an action menu that says how many photos it takes. Videos play in a player again, and a filter shows photos or videos only.

### Added

- **Photos, videos or both** – a box after the sort in the filter bar (*Photos and videos*, *Photos only*, *Videos only*) and the same choices at the top of *Filter ▸*. It works together with the other filters, is saved like them, and *Show all* resets it. In a folder without videos it is greyed out.
- **The filter bar counts** – left of *Action* it shows how many photos the filter leaves (*12 of 340 photos*, on a light accent while a filter is on; *340 photos* otherwise). That is what *Action* works on.
- A click on *Play (Enter)* over a video plays it, like `Enter`.

### Changed

- **One aesthetics value** – the mean of LAION and V2.5 (LAION alone without V2.5) is *Aesthetics*. In a benchmark against a commercial culling tool's ratings (1934 photos in two folders, 2026-09-30) the mean agreed better than either model alone (Spearman 0.64 / 0.66 against 0.55–0.60). It is the only aesthetics sort now; a saved V2.5 sort becomes it.
- **Two numbers under the photo** – the info bar shows *Aesthetics* and *Sharpness*, both in percent with a bar. Aesthetics is a fixed scale (the 2–8 range the star scale used), so a photo reads the same in every folder; sharpness still compares with the folder. Compare mode shows the same two.
- **For you in the stars** – instead of a third number, For you lightly fills the empty stars of an unrated photo (its prediction rounded; none below half a star). The tooltip names the exact value; your own stars stay the bright ones.
- **Details panel** – the explanations no longer fold open under each value: resting the pointer on a row shows it. The aesthetics section lists the mean, then LAION and V2.5 in percent, with what each model likes. The CLIP attributes fold open under their own row. `I` (all explanations) is gone.
- **Action menu** – each row says how many photos it takes (*Copy to … (12 photos)*, *Move to …*, *Delete …*, *Delete rejected …*), and its tooltip says which ones and where deleted photos go. The menu grows with its longest row. The countdown names the hidden `.originals` folder.
- German: *Abgesoffene Schatten* is now *Verlorene Tiefen*, and the exposure overlay's hint uses the same words.
- The README is short and written for users; building and packaging moved to `CONTRIBUTING.md`.

### Fixed

- `Enter` on a video could start a second Cerno instead of a player: when *Open with › Cerno* had once been used for `.mp4`, Windows could hand videos back to Cerno. Cerno now asks Windows which program opens the type; if that is Cerno, the next program Windows offers plays the video (or the *Open with* dialog opens), and a hint says how to change the default.

## [1.2.0] – 2026-09-30

Checking and overview: an overlay for sharp edges and clipping, a filter for similar photos and a grid of the whole view. The filter bar holds still, and videos get thumbnails of their own.

### Added

- **Check overlay** – `O` marks on the photo what the details panel measures:
  - First the sharpest edges in purple (the photo's strongest 2 %, none on a blurred photo). Then blown highlights in red and crushed shadows in blue. Then it is off again.
  - It works at 100 % and in compare mode.
  - It is also in *View ▸ Overlay* and behind the eye next to *Sharpness* and *Exposure* in the details panel.
- **Similar photos** – `M` shows only the photos that look like the current one: the cosine of their CLIP embeddings at 85 % or more. The threshold was measured on real folders: it keeps 90 % of the pairs within a two-second series and 0.4 % of photos taken an hour apart.
  - It combines with the other filters, and in compare mode it is about the pinned photo.
  - `M` again, *Show all* or the filter bar's last box brings every photo back; the info bar shows how alike the current photo is.
  - It needs the aesthetics model (CLIP), and nothing is marked or hidden for good.
- **Grid** – `F7` shows every photo of the view as a thumbnail.
  - The cursor is the current photo, so stars, rejecting, colours, deleting and *move on* work as usual. `↑`/`↓` go a row, Page Up/Down a screen, `+`/`−` or Ctrl + wheel change the size.
  - `Enter`, a double click, `F7` or `Esc` open the photo. Straighten, crop, compare, zoom and the check overlay open it first.
  - In capture-time order a line separates the series and the current one is underlined; nothing is folded away.
  - Only the rows on screen are drawn. While the grid shows, only its cursor photo is decoded and the analysis doesn't pause for it.

### Changed

- The `.deb` recommends ffmpeg, so videos show a frame after a plain `apt install`.
- Videos carry a see-through play button in the filmstrip, so they stand apart from photos at a glance – also before their frame has arrived.

### Fixed

- Videos further than three places from the current photo stayed blank in the filmstrip: their thumbnail only came from the photo view. They now get a small frame of their own as soon as the strip shows them (nearest first; without ffmpeg a dark frame with the play button).
- The filter bar holds still:
  - *Show all* has a fixed place, greyed out while nothing is filtered, so ticking the first filter no longer pushes every box away from the pointer.
  - The sort box is as wide as its longest entry.
  - *Action* stays at the right edge while the analysis count ticks up.
  - In the menu, *Show all* is always the first filter row, so the highlighted row stays on the filter just ticked.
  - A filter bar that appeared because a filter hid every photo stays while the pointer is on it.
- Linux: the last row of *Edit elsewhere* promised the system's *Open with* dialog, but Linux opens the default program without asking; it now says *Open with the default program*.

## [1.1.0] – 2026-09-29

More than JPEG and HEIC: PNG, TIFF, WebP, BMP, GIF, RAW and videos. Comments and keywords, editing in another program, and originals that are never deleted.

### Added

- **More formats**, kept simple: PNG, TIFF, WebP, BMP and GIF (first frame); **RAW** files (CR2, CR3, NEF, ARW, RAF, ORF, RW2, PEF, SRW, DNG, …) by the full-size preview they carry, with the camera's orientation; **videos** (MP4, MOV, M4V, AVI, MKV, MTS, …) by one frame when [ffmpeg](https://ffmpeg.org) is installed – `Enter` plays them in the default player. Straighten, crop, turns and `Ctrl+Z` stay JPEG-only and are greyed out in the menu for everything else.
- Stars and colour labels of proprietary RAW, BMP and video are written into an **XMP sidecar** beside the file (`IMG_1.xmp`, as Lightroom names it) – a RAW is never rewritten, a video of several gigabytes not copied for a star. The first write into a new sidecar takes the RAW's own stars, colour and keywords along, so none of them is lost. Delete, copy and move take the sidecar along.
- **Comment and keywords**: the details panel has a second tab, *Description* (`B`, also This photo › Comment and keywords). Keywords are added with `Enter` (several separated by commas) and removed with their ×; the comment is taken when its field is left. Cerno reads XMP and IPTC and writes both (IPTC as UTF-8), in the same background write as stars, with the file dates unchanged. While a field has the cursor, Cerno's keys stay out of it.
- **Edit elsewhere** (`E`, or *This photo › Edit elsewhere* in the menu): opens the current photo in a program the system registers for its type, in one picked by hand, or through the system's *Open with* dialog. The choice is remembered, and the same submenu switches to another program. The first original goes to `.originals` before; when the program saves, Cerno reloads the photo, analyses it again and writes back the stars, colour label, comment and keywords the save dropped.

### Changed

- The menu (`Ctrl+K`) is grouped by what a command acts on: **This photo** (stars, reject, colour label, compare, straighten, crop, turns, undo, delete), **Photos on screen** (sort, filter, refresh order, copy, move, delete, delete rejected), **View** (panels, zoom, full screen), **Settings** (auto advance, subfolders, language, models & data) and Help. Stars and deleting the current photo are in the menu now; colours appear once for marking and once, clearly apart, as a filter; no row appears or disappears at the top level. Submenus can hold submenus. The action menu (`Ctrl+M`) offers "Delete rejected photos" too.
- A series only holds photos of one camera model: two phones or a phone and a drone firing at the same moment make two series instead of one. Photos without camera data only form series with each other. After the update Cerno reads every photo's metadata once more (no new analysis).
- **Originals are never deleted.** Deleting a photo (`Delete`, the action menu, "Delete rejected photos") moves it into a hidden `.originals` folder beside it instead of the trash. Before a photo's first straighten, crop or quarter turn, its original is copied there under the same name, and only that first original is kept – `Ctrl+Z` brings it back in one step and the copy stays. When Cerno moves a photo to another folder, its original moves along. Cerno never shows `.originals`, not even when that folder is opened.
- The originals Cerno 1.0 kept in its data folder move into the `.originals` folder beside their photos at the first start.

### Removed

- The 30-day clean-up of kept originals, and deleting their copies together with the photo.
- "Best of each series" and its `+n` count: a series always shows all its photos. Hiding them let the next frame of a burst slip into a rejected photo's place unnoticed, so after "Delete rejected photos" the look-alike frames seemed to be rejected photos that had come back. Series stay marked in the filmstrip and the info bar, and capture-time order still puts the sharpest first.

## [1.0.0] – 2026-09-29

The first release with packages. It also contains the changes of 0.9.0 (capture-time sort, colour labels, series, subfolders, duplicate marks, auto-advance), which had no release of its own.

### Added

- **Packages**, built by CI with HEIC support: a Windows installer (per user, no admin rights, in the five UI languages; uninstalling keeps Cerno's data unless asked), a Windows portable zip (VC++ runtime included), a Linux AppImage (HEIC libraries included, glibc 2.39+) and a `.deb` for Ubuntu 24.04+ / Debian 13+. A `v*` tag drafts a GitHub release with all four and `SHA256SUMS.txt`.
- Straighten (`S`, fine grid, wheel or arrows, `Shift` finer) and crop (`R`: Original, 3:2, 4:3, 16:9, 1:1; `A` changes the ratio, `X` flips) for JPEGs, `Ctrl+←` / `Ctrl+→` for lossless quarter turns via the EXIF orientation. `Enter` applies, `Esc` cancels.
- **Originals are kept**: before any of these edits the file is copied into Cerno's data folder (`backups`), and `Ctrl+Z` (also Edit › Undo) writes the newest copy of the current photo back – same file, same dates, with the rating and colour label given since. Copies are deleted after 30 days, or with the photo when it is deleted in Cerno.
- Action menu (`Ctrl+M`, "Action" in the filter bar): copy or move every photo the filter shows to another folder, or delete them with the usual countdown.
- "Models & data" in the menu: where each model runs, the models folder, download, reset For you and delete the models – no longer part of the details panel.
- Colour labels are part of the filter (small squares in the filter bar, rows with a colour dot in the menu); "Show all" resets them too.
- The details panel shows the photo's size and load time.
- Details › File shows the GPS position with a link to OpenStreetMap next to Google Maps.
- JPEGs are converted to sRGB on decode (Adobe RGB, Display P3 by a fixed matrix, other profiles through a colour engine).

### Changed

- The burger menu is grouped (view, sort, filter, edit, photo, colour labels, models & data, language, help) and fully keyboard-operated: arrows, `Enter`, `→`/`←` for submenus, a letter jumps, `Esc` closes the submenu first. Switches show a box, choices a tick; long rows end in "…".
- Keys: `F` and `F11` full screen, `T` the filter bar. "Sort and filter" is called "Filter bar" everywhere.
- Info bar: `L / V / ☆` without the "Aesthetics" label – For you has an outline star, filled stars are only your rating. Size and load time left for the details panel; parts that don't fit are left out whole, and the side columns no longer move while browsing. Buttons are 32 px. The map pin is gone – the position and its map links are in Details › File.
- Compare mode: after `A` or `D` the comparison ends and the kept photo is shown alone. Before, the winner stayed pinned and the next photo moved in.
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
- A tiny defective file whose header claims a gigantic size no longer crashes or freezes Cerno each time its folder opens: photos over 200 megapixels are refused with "photo too large" before anything is allocated for them.
- A file that crashes a decoder shows an error instead of "Loading…" forever, and the viewer, the analysis and straighten/crop keep working. Before, a few such files stopped all loading until a restart.
- A photo rated while it was being analysed is analysed again shortly afterwards instead of staying without values until the folder is reopened, and the index no longer keeps the rating it had before.
- The explanations in the details panel (`I`) wrap inside the panel; before, they ran out of it and pushed every value out of view.
- The window fills the screen at start (it stayed at its restored size while flagged as maximized on scaled displays).
- `Ctrl+←` / `Ctrl+→` no longer also step to the neighbouring photo.
- In name order without filters, duplicate marks and series appear as the analysis finds them, not only after sorting or filtering.
- Compare mode (`C`, `A`, `D`) no longer restarts the decoding of the photos on screen when the list itself stays the same; both photos appear clearly sooner.
- That refresh no longer restarts decoding every 2 s: a 100 % zoom on a large photo finishes again, and the analysis no longer pauses each time.
- The menu no longer crashes when it gets shorter while it is open (e.g. a deletion runs out) and a key is pressed.
- Filmstrip thumbnails stay at 400 textures while the analysis runs through a large folder; before, every analysed photo kept one in graphics memory.

### Security

- On Windows, HEIC photos are decoded with libheif 1.23.5 and libde265 1.1.3 instead of 1.23.1 and 1.1.1. They fix flaws a crafted HEIC file could trigger, among them heap overflows that allow running code (CVE-2026-84383). The Linux packages use the distribution's libheif.
- The CLIP model is downloaded from a fixed commit of its Hugging Face repository instead of the moving `main` branch, and checked by SHA-256 as well as by size before it is used. A changed or swapped file is rejected and removed.
- ExifTool is only looked up in absolute `PATH` entries, and Cerno starts exactly the file it found – an empty entry no longer points into the working folder.
- The temporary JPEG that straighten and crop write is always a new file; a file or link already there under its name is never written through.
- `Ctrl+Z` and the 30-day clean-up only read, restore or delete kept originals in Cerno's own backup folder; an index entry pointing at any other file is ignored and dropped.

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
