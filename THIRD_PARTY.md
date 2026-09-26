# Third-party components

Cerno's own code is licensed under Apache-2.0 (see [LICENSE](./LICENSE)). It uses or embeds the following third-party work.

## Embedded in the binary

| Component | Use | License |
|---|---|---|
| [LAION improved aesthetic predictor](https://github.com/christophschuhmann/improved-aesthetic-predictor) (`sac+logos+ava1-l14-linearMSE.pth`) | Collapsed into one linear layer (`src/analysis/aesthetic_head.bin`, 768 weights + bias) by `tools/make_aesthetic_head.py` | Apache-2.0, © Christoph Schuhmann and contributors |
| [ONNX Runtime](https://github.com/microsoft/onnxruntime) via the [`ort`](https://github.com/pykeio/ort) crate | Runs the CLIP model | MIT |
| Rust crates (egui, wgpu, zune-jpeg, fast_image_resize, rusqlite/SQLite, …) | See `Cargo.lock` | MIT / Apache-2.0 / public domain (SQLite) |

## Shipped next to the binary

| Component | Use | License |
|---|---|---|
| [DirectML](https://github.com/microsoft/DirectML) (`DirectML.dll`, Windows) | GPU backend of ONNX Runtime | MIT |

## Only with `--features heic`

| Component | Use | License |
|---|---|---|
| [libheif](https://github.com/strukturag/libheif) | HEIC/HEIF container | LGPL-3.0 |
| [libde265](https://github.com/strukturag/libde265) | HEVC decoding | LGPL-3.0 |

The Windows build links these statically (vcpkg `x64-windows-static-md`). Before distributing such binaries, satisfy the LGPL (e.g. link dynamically and ship the DLLs). HEVC is patent-encumbered; check the situation for your distribution. The GPL x265 encoder is not included.

## Downloaded at runtime (not distributed with Cerno)

| Component | Use | License |
|---|---|---|
| CLIP ViT-L/14 vision model, ONNX export by [Xenova](https://huggingface.co/Xenova/clip-vit-large-patch14) of [OpenAI CLIP](https://github.com/openai/CLIP) | Image embeddings for the aesthetics score; downloaded once on request (1.2 GB) | MIT |

## Used from the system

- **Segoe UI** (Windows) or DejaVu Sans / Noto Sans (Linux) as UI font – read from the system at start-up, not distributed.
- **ExifTool** – separate program, called to write ratings; not distributed.
