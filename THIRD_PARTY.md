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

On Windows the HEIC libraries are loaded at run time (vcpkg triplet `x64-windows`, `VCPKGRS_DYNAMIC=1`). `build.rs` copies `heif.dll`, `libde265.dll` and the `licenses/` folder next to `cerno.exe`. The folder contains the GPL-3.0 and LGPL-3.0 texts and [licenses/heic.txt](licenses/heic.txt), which names the versions and how to replace the DLLs. The GPL x265 encoder is not included (`libheif[core]`). HEVC is patent-encumbered; these terms cover copyright only.

On Linux, `--features heic` links the system libheif. A package should depend on the distribution's libheif and libde265 rather than bundling static copies.

## Model files in the models folder (not distributed with Cerno)

| Component | Use | License |
|---|---|---|
| CLIP ViT-L/14 vision model, ONNX export by [Xenova](https://huggingface.co/Xenova/clip-vit-large-patch14) of [OpenAI CLIP](https://github.com/openai/CLIP) | Image embeddings for the LAION score, the attributes and the personal taste model; downloaded once on request (1.2 GB) | MIT |
| [SigLIP so400m-patch14-384](https://huggingface.co/google/siglip-so400m-patch14-384) vision tower, extracted by `tools/extract_siglip_vision.py` from the [onnx-community](https://huggingface.co/onnx-community/siglip-so400m-patch14-384) export | Image embeddings for the V2.5 score (1.7 GB) | Apache-2.0 |
| [Aesthetic Predictor V2.5](https://github.com/discus0434/aesthetic-predictor-v2-5) head, collapsed by `tools/make_aesthetic_head.py` (`aesthetic-predictor-v2.5-head.bin`) | V2.5 aesthetics score | **AGPL-3.0**, © discus0434 |

The V2.5 head is deliberately kept out of the repository and the binary; Cerno reads it as a data file at runtime. Whoever hosts the converted file must distribute it under the AGPL-3.0 with a notice and a link to the original source.

## Used from the system

- **Segoe UI** (Windows) or DejaVu Sans / Noto Sans (Linux) as UI font – read from the system at start-up, not distributed.
- **ExifTool** – separate program, called to write ratings; not distributed.
