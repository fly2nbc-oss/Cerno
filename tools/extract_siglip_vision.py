"""Cuts the image tower out of the combined SigLIP so400m ONNX model.

Source: https://huggingface.co/onnx-community/siglip-so400m-patch14-384-ONNX
(`onnx/model.onnx` + `onnx/model.onnx_data`, Apache-2.0 like google/siglip-so400m-patch14-384).
The combined graph needs text input too; Cerno only needs pixel_values -> image_embeds, which
the graph already L2-normalises. The result (~1.7 GB) is the image encoder of the Aesthetic
Predictor V2.5 module.

    python tools/extract_siglip_vision.py <dir with model.onnx + model.onnx_data> <out.onnx>
"""

import os
import sys

import numpy as np
import onnx
import onnxruntime as ort
from onnx.utils import Extractor


def main(src_dir, dst):
    model = onnx.load(os.path.join(src_dir, "model.onnx"), load_external_data=True)
    vision = Extractor(model).extract_model(["pixel_values"], ["image_embeds"])
    del model
    print("nodes:", len(vision.graph.node), "inputs:", [i.name for i in vision.graph.input])
    onnx.save(vision, dst)
    print(f"wrote {dst}: {os.path.getsize(dst) / 1e9:.2f} GB")

    # Sanity check: runs, 1152 values, unit length.
    session = ort.InferenceSession(dst, providers=["CPUExecutionProvider"])
    pixels = np.random.default_rng(0).uniform(-1, 1, (1, 3, 384, 384)).astype(np.float32)
    (embeds,) = session.run(["image_embeds"], {"pixel_values": pixels})
    print("image_embeds:", embeds.shape, "norm", float(np.linalg.norm(embeds)))
    assert embeds.shape == (1, 1152) and abs(np.linalg.norm(embeds) - 1) < 1e-3


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
