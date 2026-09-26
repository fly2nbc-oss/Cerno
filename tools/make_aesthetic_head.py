"""Collapses the LAION improved-aesthetic-predictor MLP into a single linear layer.

Source: https://github.com/christophschuhmann/improved-aesthetic-predictor (Apache-2.0),
weights `sac+logos+ava1-l14-linearMSE.pth` (e.g. huggingface.co/camenduru/improved-aesthetic-predictor).

The MLP is Linear/Dropout only – no activation functions – so at inference time the whole
network is exactly one affine map: score = w · x + b, with x the L2-normalised CLIP ViT-L/14
image embedding. The result is written as 769 little-endian f32 (768 weights, then the bias)
and embedded into Cerno with `include_bytes!`.

No torch needed: the .pth zip is unpickled with numpy.

    python tools/make_aesthetic_head.py sac+logos+ava1-l14-linearMSE.pth src/analysis/aesthetic_head.bin
"""

import collections
import pickle
import sys
import zipfile

import numpy as np

LAYERS = (0, 2, 4, 6, 7)  # nn.Sequential indices of the Linear layers (the rest is Dropout)


def load_state_dict(path):
    archive = zipfile.ZipFile(path)
    prefix = archive.namelist()[0].split("/")[0]

    class Unpickler(pickle.Unpickler):
        def find_class(self, module, name):
            if module == "torch._utils" and name == "_rebuild_tensor_v2":
                def rebuild(storage, offset, size, stride, *_):
                    item = storage.itemsize
                    view = np.lib.stride_tricks.as_strided(
                        storage[offset:], shape=size, strides=[s * item for s in stride]
                    )
                    return np.array(view)
                return rebuild
            if module == "collections" and name == "OrderedDict":
                return collections.OrderedDict
            if module == "torch" and name.endswith("Storage"):
                return name
            return super().find_class(module, name)

        def persistent_load(self, pid):
            _, storage_type, key, _location, _numel = pid
            if storage_type != "FloatStorage":
                raise ValueError(f"unexpected storage {storage_type}")
            return np.frombuffer(archive.read(f"{prefix}/data/{key}"), dtype="<f4")

    with archive.open(f"{prefix}/data.pkl") as f:
        return Unpickler(f).load()


def main(src, dst):
    state = load_state_dict(src)
    weights = [state[f"layers.{i}.weight"].astype(np.float64) for i in LAYERS]
    biases = [state[f"layers.{i}.bias"].astype(np.float64) for i in LAYERS]
    print("layers:", [w.shape for w in weights])

    w, b = np.eye(768), np.zeros(768)
    for wi, bi in zip(weights, biases):
        w, b = wi @ w, wi @ b + bi

    # Verify against a plain layer-by-layer forward pass.
    rng = np.random.default_rng(0)
    for _ in range(5):
        x = rng.standard_normal(768)
        x /= np.linalg.norm(x)
        reference = x
        for wi, bi in zip(weights, biases):
            reference = wi @ reference + bi
        assert np.allclose(w @ x + b, reference, atol=1e-9), (w @ x + b, reference)

    head = np.concatenate([w.reshape(-1), b]).astype("<f4")
    assert head.shape == (769,)
    head.tofile(dst)
    print(f"wrote {dst}: 768 weights + bias {b[0]:.4f}, |w| = {np.linalg.norm(w):.4f}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
