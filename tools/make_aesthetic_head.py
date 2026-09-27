"""Collapses a linear-only aesthetic predictor head into a single linear layer.

Works for both heads Cerno uses:

* LAION improved aesthetic predictor (Apache-2.0),
  https://github.com/christophschuhmann/improved-aesthetic-predictor,
  `sac+logos+ava1-l14-linearMSE.pth` – CLIP ViT-L/14 embedding, 768 inputs.
  The result is embedded in Cerno (`src/analysis/aesthetic_head.bin`).
* Aesthetic Predictor V2.5 (AGPL-3.0), https://github.com/discus0434/aesthetic-predictor-v2-5,
  `models/aesthetic_predictor_v2_5.pth` – SigLIP so400m embedding, 1152 inputs.
  Because of the AGPL the result is NOT embedded; it is distributed as a separate model file
  (`aesthetic-predictor-v2.5-head.bin`), and this script is its corresponding source.

Both heads are Linear/Dropout stacks without activation functions, so at inference time they
are exactly one affine map: score = w · x + b, with x the L2-normalised image embedding. The
output is (inputs + 1) little-endian f32: the weights, then the bias.

No torch needed: the .pth zip is unpickled with numpy (float32, float16 and bfloat16 storages).

    python tools/make_aesthetic_head.py <head.pth> <out.bin>
"""

import collections
import pickle
import re
import sys
import zipfile

import numpy as np


def bfloat16(raw):
    halves = np.frombuffer(raw, dtype="<u2").astype(np.uint32) << 16
    return halves.view(np.float32)


STORAGES = {
    "FloatStorage": lambda raw: np.frombuffer(raw, dtype="<f4"),
    "HalfStorage": lambda raw: np.frombuffer(raw, dtype="<f2").astype(np.float32),
    "BFloat16Storage": bfloat16,
}


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
            if storage_type not in STORAGES:
                raise ValueError(f"unsupported storage {storage_type}")
            return STORAGES[storage_type](archive.read(f"{prefix}/data/{key}"))

    with archive.open(f"{prefix}/data.pkl") as f:
        return Unpickler(f).load()


def linear_layers(state):
    """(weight, bias) pairs in network order, found by their `<prefix>.<index>.weight` keys."""
    indices = {}
    for key in state:
        match = re.fullmatch(r"(.*)\.(\d+)\.weight", key)
        if match:
            indices.setdefault(match.group(1), []).append(int(match.group(2)))
    if len(indices) != 1:
        raise ValueError(f"expected one layer stack, found {sorted(indices)}")
    prefix, numbers = next(iter(indices.items()))
    return [
        (
            state[f"{prefix}.{n}.weight"].astype(np.float64),
            state[f"{prefix}.{n}.bias"].astype(np.float64),
        )
        for n in sorted(numbers)
    ]


def main(src, dst):
    layers = linear_layers(load_state_dict(src))
    print("layers:", [w.shape for w, _ in layers])
    inputs = layers[0][0].shape[1]

    w, b = np.eye(inputs), np.zeros(inputs)
    for wi, bi in layers:
        w, b = wi @ w, wi @ b + bi

    # Verify against a plain layer-by-layer forward pass.
    rng = np.random.default_rng(0)
    for _ in range(5):
        x = rng.standard_normal(inputs)
        x /= np.linalg.norm(x)
        reference = x
        for wi, bi in layers:
            reference = wi @ reference + bi
        assert np.allclose(w @ x + b, reference, atol=1e-9), (w @ x + b, reference)

    head = np.concatenate([w.reshape(-1), b]).astype("<f4")
    assert head.shape == (inputs + 1,)
    head.tofile(dst)
    print(f"wrote {dst}: {inputs} weights + bias {b[0]:.4f}, |w| = {np.linalg.norm(w):.4f}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
