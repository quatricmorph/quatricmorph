#!/usr/bin/env python3
"""Quatricmorph fixture generator — Artifact Plane.

Generates the small synthetic SafeTensors checkpoints used by the Rust test
suite, plus a `golden.json` of reference values read back with the *official*
Python `safetensors` library.

Why hand-roll the writer instead of calling `safetensors.numpy.save_file`?

  * numpy has no native bfloat16, so the official numpy writer cannot emit the
    BF16 tensors we need to exercise dtype handling;
  * writing the bytes here and reading them back with the official library makes
    the round-trip an *independent* cross-check rather than a tautology.

The emitted `golden.json` is what `tests/end_to_end_scalar_slice.rs` asserts
against, so the Rust test stays hermetic (no Python in CI) while the recorded
values remain traceable to a real `safetensors` read.

Usage:
    python3 fixtures/generate_fixtures.py [--out fixtures]

Requires: numpy, safetensors  (see fixtures/README.md for a venv recipe).

NOTE: this generates *metadata-and-weights at test scale* (~1.2 MB). It is not
and must never become a proxy for a real checkpoint. Trillion-parameter scaling
is exercised by the synthetic *manifest* test in q-catalog, which creates
descriptors only and never any weight payload.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path

import numpy as np

# --- model shape ------------------------------------------------------------
# Chosen so that `Q[10][100,42]` (the Section 7 vertical-slice query) is in
# bounds: q_proj is [num_heads * head_dim, hidden_size] = [128, 48], and layer
# 10 exists (12 layers, 0..11). Layer 10 deliberately lands in the *second*
# shard so the slice exercises multi-shard byte-range resolution.
CONFIG = {
    "architectures": ["LlamaForCausalLM"],
    "model_type": "llama",
    "hidden_size": 48,
    "intermediate_size": 64,
    "num_hidden_layers": 12,
    "num_attention_heads": 8,
    "num_key_value_heads": 2,
    "head_dim": 16,
    "vocab_size": 64,
    "max_position_embeddings": 128,
    "rms_norm_eps": 1e-05,
    "rope_theta": 10000.0,
    "tie_word_embeddings": False,
    "torch_dtype": "float32",
}

# Two tensors are BF16 so the dtype path is exercised. Nothing is asserted on
# their values in the Section 7 gate (bf16 has no exact float64 round-trip in
# numpy without ml_dtypes); they are covered by a bit-pattern check instead.
BF16_TENSORS = {
    "model.layers.0.mlp.gate_proj.weight",
    "model.layers.11.mlp.up_proj.weight",
}

DTYPE_SIZE = {"F32": 4, "BF16": 2, "F16": 2, "I8": 1, "U8": 1, "I32": 4, "I64": 8}


def stable_seed(name: str) -> int:
    """Deterministic per-tensor seed, stable across platforms and runs."""
    return int.from_bytes(hashlib.blake2b(name.encode(), digest_size=8).digest(), "little")


def make_f32(name: str, shape: list[int]) -> np.ndarray:
    rng = np.random.default_rng(stable_seed(name))
    return rng.standard_normal(size=shape, dtype=np.float32) * np.float32(0.02)


def f32_to_bf16_bytes(a: np.ndarray) -> bytes:
    """Truncate-toward-zero f32 -> bf16 (take the high 16 bits).

    Round-to-nearest-even would be more faithful to PyTorch, but truncation is
    simpler to reproduce and the fixture makes no numerical claim about these
    tensors beyond "the bytes are what the header says they are".
    """
    u32 = a.astype(np.float32).view(np.uint32)
    hi = (u32 >> np.uint32(16)).astype(np.uint16)
    return hi.tobytes()


def tensor_plan() -> list[tuple[str, list[int], str, int]]:
    """(name, shape, dtype, shard_index) for every tensor in the sharded model."""
    h = CONFIG["hidden_size"]
    inter = CONFIG["intermediate_size"]
    vocab = CONFIG["vocab_size"]
    n_layers = CONFIG["num_hidden_layers"]
    q_out = CONFIG["num_attention_heads"] * CONFIG["head_dim"]
    kv_out = CONFIG["num_key_value_heads"] * CONFIG["head_dim"]

    plan: list[tuple[str, list[int], str, int]] = []

    def dt(name: str) -> str:
        return "BF16" if name in BF16_TENSORS else "F32"

    plan.append(("model.embed_tokens.weight", [vocab, h], "F32", 1))
    for layer in range(n_layers):
        # Layers 0..5 -> shard 1, layers 6..11 -> shard 2.
        shard = 1 if layer < n_layers // 2 else 2
        p = f"model.layers.{layer}."
        for name, shape in [
            (p + "self_attn.q_proj.weight", [q_out, h]),
            (p + "self_attn.k_proj.weight", [kv_out, h]),
            (p + "self_attn.v_proj.weight", [kv_out, h]),
            (p + "self_attn.o_proj.weight", [h, q_out]),
            (p + "mlp.gate_proj.weight", [inter, h]),
            (p + "mlp.up_proj.weight", [inter, h]),
            (p + "mlp.down_proj.weight", [h, inter]),
            (p + "input_layernorm.weight", [h]),
            (p + "post_attention_layernorm.weight", [h]),
        ]:
            plan.append((name, shape, dt(name), shard))
    plan.append(("model.norm.weight", [h], "F32", 2))
    plan.append(("lm_head.weight", [vocab, h], "F32", 2))
    return plan


def serialize_safetensors(
    tensors: list[tuple[str, list[int], str, bytes]],
    metadata: dict[str, str] | None = None,
) -> bytes:
    """Emit a SafeTensors file.

    Layout: u64 LE header length | JSON header | data buffer.
    `data_offsets` are relative to the start of the data buffer.
    """
    header: dict[str, object] = {}
    if metadata:
        header["__metadata__"] = metadata

    buf = bytearray()
    for name, shape, dtype, payload in tensors:
        expected = DTYPE_SIZE[dtype]
        for d in shape:
            expected *= d
        if len(payload) != expected:
            raise ValueError(f"{name}: payload {len(payload)} != expected {expected}")
        start = len(buf)
        buf.extend(payload)
        header[name] = {"dtype": dtype, "shape": shape, "data_offsets": [start, len(buf)]}

    header_bytes = json.dumps(header, separators=(",", ":")).encode("utf-8")
    # SafeTensors permits trailing whitespace padding; pad to an 8-byte boundary
    # so the data buffer is aligned, matching what the reference writer does.
    pad = (8 - (len(header_bytes) % 8)) % 8
    header_bytes += b" " * pad
    return struct.pack("<Q", len(header_bytes)) + header_bytes + bytes(buf)


def build_payloads(plan):
    """name -> (shape, dtype, bytes, f32_reference_array_or_None)"""
    out = {}
    for name, shape, dtype, shard in plan:
        arr = make_f32(name, shape)
        if dtype == "F32":
            out[name] = (shape, dtype, shard, arr.tobytes(), arr)
        elif dtype == "BF16":
            out[name] = (shape, dtype, shard, f32_to_bf16_bytes(arr), None)
        else:
            raise ValueError(f"unhandled dtype {dtype}")
    return out


def write_sharded(out_dir: Path, plan, payloads) -> dict:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "config.json").write_text(json.dumps(CONFIG, indent=2) + "\n")

    n_shards = max(s for _, _, _, s in plan)
    weight_map: dict[str, str] = {}
    total_size = 0

    for shard in range(1, n_shards + 1):
        fname = f"model-{shard:05d}-of-{n_shards:05d}.safetensors"
        group = [
            (name, payloads[name][0], payloads[name][1], payloads[name][3])
            for name, _, _, s in plan
            if s == shard
        ]
        blob = serialize_safetensors(
            group,
            metadata={"format": "pt", "quatricmorph_fixture": "tiny-llama-2shard"},
        )
        (out_dir / fname).write_bytes(blob)
        for name, _, _, payload in group:
            weight_map[name] = fname
            total_size += len(payload)

    index = {
        "metadata": {"total_size": total_size},
        "weight_map": weight_map,
    }
    (out_dir / "model.safetensors.index.json").write_text(json.dumps(index, indent=2) + "\n")
    return index


def write_single(out_dir: Path, payloads) -> None:
    """A single-file checkpoint (embed + layer 0) for the single-file code path."""
    out_dir.mkdir(parents=True, exist_ok=True)
    single_cfg = dict(CONFIG)
    single_cfg["num_hidden_layers"] = 1
    (out_dir / "config.json").write_text(json.dumps(single_cfg, indent=2) + "\n")

    names = [n for n in payloads if n == "model.embed_tokens.weight" or n.startswith("model.layers.0.")]
    names.sort()
    group = [(n, payloads[n][0], payloads[n][1], payloads[n][3]) for n in names]
    blob = serialize_safetensors(
        group, metadata={"format": "pt", "quatricmorph_fixture": "tiny-llama-single"}
    )
    (out_dir / "model.safetensors").write_bytes(blob)


# --- golden reference (read back through the official library) ---------------

GOLDEN_SCALARS = [
    # The Section 7 gate: Q[10][100,42].
    ("model.layers.10.self_attn.q_proj.weight", [100, 42]),
    ("model.layers.10.self_attn.q_proj.weight", [0, 0]),
    ("model.layers.10.self_attn.q_proj.weight", [127, 47]),
    # A first-shard tensor, to prove shard selection is not hardcoded.
    ("model.layers.3.self_attn.q_proj.weight", [100, 42]),
    # A 1-D tensor.
    ("model.layers.10.input_layernorm.weight", [7]),
    # A tensor whose logical layout is [hidden, q_out] rather than [q_out, hidden].
    ("model.layers.10.self_attn.o_proj.weight", [47, 100]),
]

GOLDEN_SLICES = [
    ("model.layers.10.self_attn.q_proj.weight", [100, 104], [40, 44]),
    ("model.layers.10.self_attn.k_proj.weight", [0, 2], [0, 3]),
]


def build_golden(sharded_dir: Path, index: dict) -> dict:
    from safetensors import safe_open  # official reference reader
    import safetensors

    def read_tensor(name: str) -> np.ndarray:
        shard = index["weight_map"][name]
        with safe_open(str(sharded_dir / shard), framework="numpy") as f:
            return f.get_tensor(name)

    def bits(x: np.float32) -> str:
        return "0x%08X" % int(np.float32(x).view(np.uint32))

    scalars = []
    for name, idx in GOLDEN_SCALARS:
        arr = read_tensor(name)
        value = arr[tuple(idx)]
        scalars.append(
            {
                "tensor": name,
                "shard": index["weight_map"][name],
                "dtype": str(arr.dtype).upper().replace("FLOAT32", "F32"),
                "shape": list(arr.shape),
                "index": idx,
                "value_f32_bits": bits(value),
                "value_approx": float(value),
            }
        )

    slices = []
    for name, rows, cols in GOLDEN_SLICES:
        arr = read_tensor(name)
        sub = arr[rows[0] : rows[1], cols[0] : cols[1]]
        slices.append(
            {
                "tensor": name,
                "shard": index["weight_map"][name],
                "rows": rows,
                "columns": cols,
                "shape": list(sub.shape),
                "values_f32_bits": [bits(v) for v in sub.reshape(-1)],
            }
        )

    # BF16 tensors: assert on the raw 16-bit patterns, not on a float value.
    bf16 = []
    for name in sorted(BF16_TENSORS):
        shard = index["weight_map"][name]
        with safe_open(str(sharded_dir / shard), framework="numpy") as f:
            meta = f.metadata()  # noqa: F841 - documents that metadata is reachable
        # numpy cannot materialize BF16; read the declared byte range directly.
        raw = (sharded_dir / shard).read_bytes()
        hlen = struct.unpack("<Q", raw[:8])[0]
        header = json.loads(raw[8 : 8 + hlen])
        entry = header[name]
        start, end = entry["data_offsets"]
        data = raw[8 + hlen + start : 8 + hlen + end]
        first = struct.unpack_from("<H", data, 0)[0]
        bf16.append(
            {
                "tensor": name,
                "shard": shard,
                "dtype": entry["dtype"],
                "shape": entry["shape"],
                "byte_length": end - start,
                "first_u16_le": "0x%04X" % first,
            }
        )

    tensor_count = sum(
        len([k for k in json.loads(
            (sharded_dir / s).read_bytes()[8 : 8 + struct.unpack("<Q", (sharded_dir / s).read_bytes()[:8])[0]]
        ) if k != "__metadata__"])
        for s in sorted(set(index["weight_map"].values()))
    )

    return {
        "_comment": (
            "Generated by fixtures/generate_fixtures.py. Values were read back "
            "with the official Python `safetensors` library and are the reference "
            "the Rust suite asserts against (TILE-07 / PLAT-P0-LOOKUP / AC-005)."
        ),
        "reference_library": "safetensors==%s" % safetensors.__version__,
        "numpy": np.__version__,
        "fixture": "tiny-llama-2shard",
        "tensor_count": tensor_count,
        "shard_count": len(set(index["weight_map"].values())),
        "total_size_bytes": index["metadata"]["total_size"],
        "scalars": scalars,
        "slices": slices,
        "bf16": bf16,
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(Path(__file__).resolve().parent))
    args = ap.parse_args()
    root = Path(args.out)

    plan = tensor_plan()
    payloads = build_payloads(plan)

    sharded = root / "tiny-llama-2shard"
    index = write_sharded(sharded, plan, payloads)
    write_single(root / "tiny-llama-single", payloads)

    golden = build_golden(sharded, index)
    (sharded / "golden.json").write_text(json.dumps(golden, indent=2) + "\n")

    print(f"wrote {sharded} ({golden['tensor_count']} tensors, "
          f"{golden['shard_count']} shards, {golden['total_size_bytes']} payload bytes)")
    print(f"wrote {root / 'tiny-llama-single'}")


if __name__ == "__main__":
    main()
