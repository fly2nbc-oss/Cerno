"""Precomputes the CLIP text embeddings for Cerno's zero-shot attributes.

Each attribute is a pair of prompts (positive, negative), scored like CLIP-IQA: softmax over
the cosine similarities (x 100) of the image embedding with both prompts. The text side never
changes, so it is computed once here and embedded (`src/analysis/clip_prompts.bin`, N x 768
little-endian f32, L2-normalised, in the order of PROMPTS below – keep `analysis/attributes.rs`
in sync).

Model: CLIP ViT-L/14 text tower, ONNX export by Xenova
(https://huggingface.co/Xenova/clip-vit-large-patch14, `onnx/text_model.onnx` and
`tokenizer.json`), MIT like OpenAI CLIP.

    python tools/make_clip_prompts.py <text_model.onnx> <tokenizer.json> <out.bin>
"""

import sys

import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

# (positive, negative) – CLIP-IQA style antonym prompts.
PROMPTS = [
    ("Good photo.", "Bad photo."),
    ("A sharp photo.", "A blurry photo."),
    ("A photo with good lighting.", "A photo with bad lighting."),
    ("A well composed photo.", "A badly composed photo."),
    ("A clean photo.", "A noisy photo."),
    ("A colorful photo.", "A dull photo."),
]


def main(model_path, tokenizer_path, dst):
    tokenizer = Tokenizer.from_file(tokenizer_path)
    session = ort.InferenceSession(model_path, providers=["CPUExecutionProvider"])
    vectors = []
    for pair in PROMPTS:
        for text in pair:
            ids = np.array([tokenizer.encode(text).ids], dtype=np.int64)
            (embeds,) = session.run(["text_embeds"], {"input_ids": ids})
            v = embeds[0] / np.linalg.norm(embeds[0])
            vectors.append(v)
            print(f"{text!r}: {len(ids[0])} tokens")
    out = np.stack(vectors).astype("<f4")
    assert out.shape == (2 * len(PROMPTS), 768)
    out.tofile(dst)
    print(f"wrote {dst}: {out.shape}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3])
