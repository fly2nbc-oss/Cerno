# Third-party components

Cerno's own code is licensed under Apache-2.0 (see [LICENSE](./LICENSE)). It uses or embeds the following third-party work.

## Embedded in the binary

| Component | Use | License |
|---|---|---|
| [LAION improved aesthetic predictor](https://github.com/christophschuhmann/improved-aesthetic-predictor) (`sac+logos+ava1-l14-linearMSE.pth`) | Collapsed into one linear layer (`src/analysis/aesthetic_head.bin`, 768 weights + bias) by `tools/make_aesthetic_head.py` | Apache-2.0, © Christoph Schuhmann and contributors |
| [YuNet face detector](https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet) (`face_detection_yunet_2023mar.onnx`) | Finds faces and eyes for the eye-sharpness measure | MIT, © Shiqi Yu and contributors (OpenCV Zoo) |
| CLIP prompt vectors (`src/analysis/clip_prompts.bin`) | Text embeddings of 12 prompts for the zero-shot attributes, computed once by `tools/make_clip_prompts.py` with the [CLIP ViT-L/14](https://github.com/openai/CLIP) text model ([Xenova](https://huggingface.co/Xenova/clip-vit-large-patch14) ONNX export) | MIT (OpenAI CLIP) |
| [ONNX Runtime](https://github.com/microsoft/onnxruntime) via the [`ort`](https://github.com/pykeio/ort) crate | Runs all models | MIT |
| Rust crates (egui, wgpu, zune-jpeg, fast_image_resize, rusqlite/SQLite, …) | See `Cargo.lock` | MIT / Apache-2.0 / public domain (SQLite) |

## Shipped next to the binary

| Component | Use | License |
|---|---|---|
| [DirectML](https://github.com/microsoft/DirectML) (`DirectML.dll`, Windows) | GPU backend of ONNX Runtime | MIT |
| [libheif](https://github.com/strukturag/libheif) (`heif.dll`, Windows, `--features heic`) | HEIC/HEIF container | LGPL-3.0 |
| [libde265](https://github.com/strukturag/libde265) (`libde265.dll`, Windows, `--features heic`) | HEVC decoding | LGPL-3.0 |
| Microsoft Visual C++ runtime (`MSVCP140.dll`, `MSVCP140_1.dll`, `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll`; Windows installer and portable zip) | C++ runtime of ONNX Runtime, libheif and libde265 | Microsoft Visual Studio license terms – "Distributable Code", copied from Visual Studio's redist folder |
| libheif, its libde265 plugin, libde265 and libsharpyuv of Ubuntu 24.04 (`usr/lib` in the AppImage) | HEIC decoding in the Linux AppImage | LGPL-3.0 (libheif, libde265), BSD-3-Clause (libsharpyuv) |
| [GStreamer](https://gstreamer.freedesktop.org/) 1.28.7 (official MSVC build): core, plugins base / good / bad / ugly, gst-libav; 23 plugins in `gstreamer-1.0\` and their libraries beside `cerno.exe` (Windows, `--features video`) | Video playback | LGPL-2.0-or-later |
| [FFmpeg](https://ffmpeg.org/) libraries of that build (`avcodec`, `avformat`, `avutil`, `avfilter`, `swscale`, `swresample`) | Video and audio decoding through gst-libav | LGPL-2.1-or-later (built without `--enable-gpl` / `--enable-nonfree`) |
| GLib, proxy-libintl, orc, libffi, PCRE2, zlib, bzip2, libvpx, dav1d, gst-plugins-rs (dav1d plugin) of that build | GStreamer's own dependencies; VP8/VP9 and AV1 decoding | LGPL-2.0-or-later (GLib, proxy-libintl), BSD (orc, PCRE2, libvpx, dav1d), MIT (libffi), zlib, bzip2, MIT / Apache-2.0 (gst-plugins-rs) |

On Windows the HEIC libraries are loaded at run time (vcpkg triplet `x64-windows`, `VCPKGRS_DYNAMIC=1`). `build.rs` copies `heif.dll`, `libde265.dll` and the `licenses/` folder next to `cerno.exe`. The folder contains the GPL-3.0 and LGPL-3.0 texts and [licenses/heic.txt](licenses/heic.txt), which names the versions and how to replace the DLLs. The GPL x265 encoder is not included (`libheif[core]`). HEVC is patent-encumbered; these terms cover copyright only.

On Windows, GStreamer comes from its official installer, unchanged; `packaging/windows/build.ps1` copies the plugins Cerno uses, the DLLs they import and their licence texts (`licenses/gstreamer/`) into the package. [licenses/gstreamer.txt](licenses/gstreamer.txt) names the version, the source of every part and how to replace the DLLs. On Linux, `--features video` links the system's GStreamer: the `.deb` depends on its plugin packages, and the AppImage bundles no GStreamer, GLib or FFmpeg – it uses the system's. H.264, H.265 and AAC are patent-encumbered; these terms cover copyright only.

On Linux, `--features heic` links the system libheif. The `.deb` depends on the distribution's libheif and its libde265 plugin. The AppImage bundles them as separate shared libraries; `usr/share/doc/cerno/bundled-libraries.txt` inside it lists the Ubuntu package versions, whose source is in Ubuntu's archive.

## Model files in the models folder (not distributed with Cerno)

| Component | Use | License |
|---|---|---|
| CLIP ViT-L/14 vision model, ONNX export by [Xenova](https://huggingface.co/Xenova/clip-vit-large-patch14) of [OpenAI CLIP](https://github.com/openai/CLIP) | Image embeddings for the LAION score, the attributes and the personal taste model; downloaded once on request (1.2 GB) | MIT |
| [SigLIP so400m-patch14-384](https://huggingface.co/google/siglip-so400m-patch14-384) vision tower, extracted by `tools/extract_siglip_vision.py` from the [onnx-community](https://huggingface.co/onnx-community/siglip-so400m-patch14-384) export | Image embeddings for the V2.5 score; downloaded once on request (1.7 GB) | Apache-2.0 |
| [Aesthetic Predictor V2.5](https://github.com/discus0434/aesthetic-predictor-v2-5) head, collapsed by `tools/make_aesthetic_head.py` (`aesthetic-predictor-v2.5-head.bin`) | V2.5 aesthetics score; downloaded together with SigLIP (4.6 KB) | **AGPL-3.0**, © discus0434 |

The V2.5 head is deliberately kept out of the repository and the binary; Cerno reads it as a data file at runtime. CLIP comes from Hugging Face at a pinned commit. SigLIP and the V2.5 head come from the release [`models-1`](https://github.com/fly2nbc-oss/Cerno/releases/tag/models-1) of this repository, which carries the AGPL-3.0 and Apache-2.0 texts and a `NOTICE.md` with the sources, commits and conversion scripts. Cerno uses a downloaded file only when its size and SHA-256 match the values in `src/analysis/manifest.rs`.

## Used from the system

- **Segoe UI** (Windows) or DejaVu Sans / Noto Sans (Linux) as UI font – read from the system at start-up, not distributed.
- **ExifTool** – separate program, called to write ratings; not distributed.
- **ffmpeg** (optional) – separate program, called for one frame of each video (still frame and thumbnail); not distributed. Without it videos show a placeholder and still play.
- **GStreamer** (Linux) – the distribution's GStreamer and its plugins play videos; not distributed.
