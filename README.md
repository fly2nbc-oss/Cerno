# Cerno

**Fast photo viewer and culling tool – switch instantly, rate from the keyboard, keep the file dates.**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](./LICENSE)
[![CI](https://github.com/fly2nbc-oss/Cerno/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/fly2nbc-oss/Cerno/actions/workflows/ci.yml)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-blue.svg)](#install)

Cerno (Latin *cerno* – "I sift, see clearly") helps you go through a folder of photos quickly and keep the good ones. Stars, colour labels, comments and keywords go straight into the file, where Lightroom, Bridge, digiKam and Windows Explorer find them – the file's dates stay exactly as they were. Everything runs on your computer.

<p align="center">
  <img src="./screenshots/cerno-main.jpg" alt="Cerno showing a photo with the filter bar, the details panel, the filmstrip and the info bar" width="720" />
</p>

<sub>Sample photo: CC0 (public domain), [Images from Unsplash](https://commons.wikimedia.org/wiki/Category:Images_from_Unsplash) on Wikimedia Commons.</sub>

## What it does

- **Instant switching** – the photos around the current one are loaded in advance.
- **Rate from the keyboard** – stars, reject, colour labels, comment and keywords.
- **Two helpers under every photo** – *Aesthetics* and *Sharpness*, both in percent (see [the values](#the-values)). Empty stars fill lightly with Cerno's guess of your rating.
- **Sort and filter** – by capture time, rating, aesthetics, sharpness; photos or videos only; blurry shots, duplicates, similar photos, photos with or without people. Filters of different kinds work together: *4★* and *Blurry* are the blurry 4-star photos.
- **Top 10 … Top 250** – the best photos of what the filters leave, for a slideshow or a photo book: by your stars (else Cerno's guess), aesthetics and sharpness, the best of each burst first. Rejected and blurry shots and duplicates never count, and nothing in the photos changes.
- **Compare** two photos side by side, or see the whole folder as a grid.
- **Straighten, crop and rotate** JPEGs – the original is kept and `Ctrl+Z` brings it back.
- **Nothing is lost** – deleting moves a photo into a hidden `.originals` folder beside it, after a 5-second countdown you can undo with `Esc`.
- **Videos play right here** – with sound: `Space` plays and pauses, `Alt+←`/`Alt+→` jump 5 seconds, the bar under the video seeks and sets the volume.
- **Formats** – JPEG, HEIC, PNG, TIFF, WebP, BMP, GIF, RAW (its embedded preview) and videos (MP4, MOV, MKV, WebM, AVI, MTS …).
- **Five languages** – German, English, French, Spanish, Italian.

## Install

Download from [Releases](https://github.com/fly2nbc-oss/Cerno/releases):

| File | For |
|---|---|
| `cerno_<version>_x64-setup.exe` | Windows 10/11 – installs for your user, no admin rights |
| `cerno_<version>_x64-portable.zip` | Windows – unzip anywhere and run `Cerno\cerno.exe` |
| `cerno_<version>_x86_64.AppImage` | Linux (Ubuntu 24.04, Debian 13, Fedora 40 or newer) |
| `cerno_<version>_amd64.deb` | Ubuntu 24.04+ / Debian 13+ – `sudo apt install ./cerno_<version>_amd64.deb` |

The Windows files are not signed yet: SmartScreen asks once (*More info* → *Run anyway*).

**Needed for writing stars and labels:** [ExifTool](https://exiftool.org/) (Windows: `winget install OliverBetz.ExifTool`, Linux: `apt install libimage-exiftool-perl`). Viewing works without it.
**Videos** play with GStreamer: the Windows downloads bring it along; on Linux the `.deb` installs it, and the AppImage uses the one every desktop has. **Optional:** [ffmpeg](https://ffmpeg.org/) shows a still frame and a filmstrip picture of each video (Windows: `winget install Gyan.FFmpeg`, Linux: `apt install ffmpeg`).

## The most important keys

| Key | Does |
|---|---|
| `→` / `←`, `Space` | Next / previous photo |
| `Space` on a video | Play / pause (`Shift+Space`: next, `Alt+←`/`Alt+→`: 5 seconds back / on) |
| `1`–`5`, `0` | Stars / no stars (`Shift` + digit: rate and go on) |
| `X` | Reject (again: undo) |
| `6`–`9` | Colour label red, yellow, green, blue |
| `Delete` | Delete – `Esc` within 5 s brings it back |
| `Z`, double click | 100 % zoom (`Ctrl+1` 100 %, `Ctrl+0` the whole photo) |
| `C` | Compare: this photo stays on the left, browse on the right |
| `T` / `Tab` / `F6` / `F7` | Filter bar / details / filmstrip / grid |
| `Ctrl+U` | Include subfolders (on / off) |
| `Ctrl+K` | Menu with every function |
| `Ctrl+M` | Copy, move or delete all photos the filter shows |
| `H` | Help with all keys |

## The values

- **Aesthetics** – how appealing two AIs find the photo (LAION and V2.5, averaged). 0 % is unappealing, 100 % very appealing; most photos land at 40–60 %. A fixed scale: a photo has the same value in every folder. Needs two image models (CLIP 1.2 GB and V2.5 1.7 GB). Cerno offers to download them once (*Menu → Models & data*), and an interrupted download continues where it stopped.
- **Sharpness** – how sharp the photo is compared with the other photos in the folder, measured at the eyes when there is a face. "Probably blurry" marks the blurriest ones.
- **Prediction** – the stars Cerno thinks you would give, learnt from your own ratings (and from the photos you reject or delete, as 0 stars) once there are 15 of them. It shows as lightly filled stars and never sets a rating.

The details panel (`Tab`) shows more – both AIs separately, exposure, histogram, camera data – and explains each value when you rest the pointer on it.

## Where your data is

- **In the photo:** only stars, colour label, comment and keywords (RAW, BMP and video: in an `.xmp` file beside it). File dates are not changed.
- **In Cerno's index:** all scores – `%LOCALAPPDATA%\Cerno\data` on Windows, `~/.local/share/cerno` on Linux.
- **Beside your photos:** the hidden `.originals` folder with deleted photos and the originals of edited ones. Empty it yourself once you are sure – it is not the Recycle Bin.

Backup tools that only compare size and date (e.g. `rsync` without `-c`) may miss a changed rating, because the date stays the same.

## More

- Building from source and checks: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Changes per version: [`CHANGELOG.md`](./CHANGELOG.md)
- License: Apache 2.0 ([`LICENSE`](./LICENSE)), third-party components in [`THIRD_PARTY.md`](./THIRD_PARTY.md)
