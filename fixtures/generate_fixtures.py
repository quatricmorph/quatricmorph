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
    python3 fixtures/generate_fixtures.py [--out fixtures] [--llama] [--qwen]

With no selector every fixture is written, which is what CI's reproducibility
gate runs (`.github/workflows/build.yaml`: `python3 fixtures/generate_fixtures.py`
followed by `git diff --exit-code -- fixtures/`). The selectors exist only to
regenerate one fixture while iterating; they must never be needed for the gate,
or a fixture could drift without the gate noticing.

Requires: numpy, safetensors  (see fixtures/README.md for a venv recipe).
The Qwen fixture itself needs neither — it carries no weights — but this module
imports numpy at load time, so the venv is still the way to run it.

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


# --- tiny-qwen-single: a NAMING fixture, not a weights fixture ---------------
#
# QM-0010 / NSIR-006. What is under test here is *name resolution*, so this
# fixture deliberately carries **no weight payload and no `.safetensors` file**.
# ARCHITECTURE.md §4.2 forbids inferring a role from a shape, and
# `q_nsir::NsirResolver::resolve_name` takes a `&str` and nothing else — a shape
# is not merely unused, it is unavailable. A fixture of weights would therefore
# test nothing this fixture does not, while inviting exactly the shape-based
# reasoning the requirement forbids.
#
# Two files are written:
#
#   config.json  — a Qwen3-shaped declared config, so `q-architecture`'s registry
#                  can select the plugin from `model_type` / `architectures` the
#                  same way it does for `tiny-llama-2shard`.
#   golden.json  — the name-resolution contract: for every Qwen name family, the
#                  raw name and the canonical address it must produce, plus the
#                  names the plugin is deliberately NOT taught.
#
# PROVENANCE OF THE EXPECTED ADDRESSES. Every `canonical_name` below was written
# out by hand from the address rule in ARCHITECTURE.md §6.1, namely
#
#     model[.layers[N]].<component>[.experts[E]].<operation>[.<parameter>]
#
# with the component path segments and operation names taken from the declared
# manifests under `architectures/`. **No value in this file was produced by
# running the Rust resolver**, which is what makes the Rust test that asserts
# against this file a cross-check rather than a tautology. The same rule is
# applied to `architectures/llama/plugin.toml`'s tensors by
# `q_nsir::resolver::tests::llama_resolves_the_architecture_md_example`, whose
# expected string is likewise hand-written — the two agree, which is the point:
# a canonical address must not depend on which family produced the tensor.

# Layer 10 and expert 37 are the indices ARCHITECTURE.md §4.2 and QM-0010's
# §Data Contracts use, so the rows below line up with the specification text.
QWEN_LAYER = 10
QWEN_EXPERT = 37
QWEN_ROUTER_LAYER = 5

QWEN_CONFIG = {
    # `architectures` and `model_type` are the two keys the registry selects on.
    "architectures": ["Qwen3ForCausalLM"],
    "model_type": "qwen3",
    "hidden_size": 48,
    "intermediate_size": 64,
    "num_hidden_layers": 12,
    "num_attention_heads": 8,
    "num_key_value_heads": 2,
    "head_dim": 16,
    "vocab_size": 64,
    "max_position_embeddings": 128,
    "rms_norm_eps": 1e-06,
    "rope_theta": 1000000.0,
    # Qwen3 drops the Qwen2 attention biases and adds per-head q/k norms. Both
    # spellings are covered by golden.json's rows regardless of this flag; the
    # flag is recorded because it is what the checkpoint declares, and nothing
    # in the resolver reads it (a bias tensor is resolved by its *name*).
    "attention_bias": False,
    "tie_word_embeddings": False,
    "torch_dtype": "bfloat16",
}


# QM-0010 §Scope's list of name families, verbatim and in its order. Acceptance
# criterion 1 is "resolves all 15 name families", so this list is 15 entries
# long and every row below carries one of these labels — a reviewer can count.
# The last entry bundles MoE exactly as §Scope words it: "MoE `experts.N.*` plus
# `mlp.gate.weight`".
QWEN_NAME_FAMILIES = [
    "q_proj",
    "k_proj",
    "v_proj",
    "o_proj",
    "gate_proj",
    "up_proj",
    "down_proj",
    "input_layernorm",
    "post_attention_layernorm",
    "q_norm",
    "k_norm",
    "embed_tokens",
    "lm_head",
    "model.norm",
    "moe",
]


def _qwen_rows() -> list[dict]:
    """The name-resolution contract, one row per raw tensor name.

    `family` names the entry in QWEN_NAME_FAMILIES; `variant` records which Qwen
    release emits the name, so a reader can see that one fixture describes a
    family's *naming*, not one checkpoint's contents.
    """
    lay = QWEN_LAYER
    exp = QWEN_EXPERT
    rl = QWEN_ROUTER_LAYER
    wi = ["output_channel", "input_channel"]  # a 2-D projection weight
    bo = ["output_channel"]  # a 1-D projection bias
    hid = ["hidden_channel"]
    head = ["head_channel"]
    vocab = ["vocabulary", "hidden_channel"]

    def row(family, variant, raw, canonical, role, component, operation,
            parameter, axes, layer=None, expert=None):
        return {
            "family": family,
            "variant": variant,
            "raw_name": raw,
            "canonical_name": canonical,
            "role": role,
            "component": component,
            "operation": operation,
            "parameter": parameter,
            "axes": axes,
            "layer": layer,
            "expert": expert,
        }

    p = f"model.layers.{lay}."
    c = f"model.layers[{lay}]."
    return [
        # 1-4: attention projections.
        row("q_proj", "qwen2+qwen3", p + "self_attn.q_proj.weight",
            c + "self_attention.query_projection.weight",
            "attention_query_projection", "attention", "query_projection",
            "weight", wi, lay),
        row("k_proj", "qwen2+qwen3", p + "self_attn.k_proj.weight",
            c + "self_attention.key_projection.weight",
            "attention_key_projection", "attention", "key_projection",
            "weight", wi, lay),
        row("v_proj", "qwen2+qwen3", p + "self_attn.v_proj.weight",
            c + "self_attention.value_projection.weight",
            "attention_value_projection", "attention", "value_projection",
            "weight", wi, lay),
        row("o_proj", "qwen2+qwen3", p + "self_attn.o_proj.weight",
            c + "self_attention.output_projection.weight",
            "attention_output_projection", "attention", "output_projection",
            "weight", wi, lay),
        # 1-3 again, as biases: Qwen2 declares `attention_bias: true` and ships
        # q/k/v biases; o_proj has none. Qwen3 ships no attention bias at all.
        row("q_proj", "qwen2", p + "self_attn.q_proj.bias",
            c + "self_attention.query_projection.bias",
            "attention_query_projection", "attention", "query_projection",
            "bias", bo, lay),
        row("k_proj", "qwen2", p + "self_attn.k_proj.bias",
            c + "self_attention.key_projection.bias",
            "attention_key_projection", "attention", "key_projection",
            "bias", bo, lay),
        row("v_proj", "qwen2", p + "self_attn.v_proj.bias",
            c + "self_attention.value_projection.bias",
            "attention_value_projection", "attention", "value_projection",
            "bias", bo, lay),
        # 10-11: Qwen3's per-head query/key norms, which Llama checkpoints do
        # not carry (the Llama manifest declares the rule anyway).
        row("q_norm", "qwen3", p + "self_attn.q_norm.weight",
            c + "self_attention.query_normalization.weight",
            "attention_query_norm", "attention", "query_normalization",
            "weight", head, lay),
        row("k_norm", "qwen3", p + "self_attn.k_norm.weight",
            c + "self_attention.key_normalization.weight",
            "attention_key_norm", "attention", "key_normalization",
            "weight", head, lay),
        # 5-7: the dense MLP.
        row("gate_proj", "qwen2+qwen3", p + "mlp.gate_proj.weight",
            c + "mlp.gate_projection.weight",
            "mlp_gate_projection", "mlp", "gate_projection", "weight", wi, lay),
        row("up_proj", "qwen2+qwen3", p + "mlp.up_proj.weight",
            c + "mlp.up_projection.weight",
            "mlp_up_projection", "mlp", "up_projection", "weight", wi, lay),
        row("down_proj", "qwen2+qwen3", p + "mlp.down_proj.weight",
            c + "mlp.down_projection.weight",
            "mlp_down_projection", "mlp", "down_projection", "weight", wi, lay),
        # 8-9: the two per-layer norms.
        row("input_layernorm", "qwen2+qwen3", p + "input_layernorm.weight",
            c + "normalization.input_normalization.weight",
            "input_layernorm", "normalization", "input_normalization",
            "weight", hid, lay),
        row("post_attention_layernorm", "qwen2+qwen3",
            p + "post_attention_layernorm.weight",
            c + "normalization.post_attention_normalization.weight",
            "post_attention_layernorm", "normalization",
            "post_attention_normalization", "weight", hid, lay),
        # 15: MoE. `mlp.gate.weight` is the router; `mlp.gate_proj.weight`
        # above is the dense MLP's gate. The two names differ by five
        # characters and mean different things, which is why the plugin maps
        # names and not meanings.
        row("moe", "qwen3_moe", f"model.layers.{rl}.mlp.gate.weight",
            f"model.layers[{rl}].router.expert_routing.weight",
            "moe_router", "router", "expert_routing", "weight",
            ["expert", "hidden_channel"], rl),
        row("moe", "qwen3_moe",
            p + f"mlp.experts.{exp}.gate_proj.weight",
            c + f"moe.experts[{exp}].gate_projection.weight",
            "moe_expert_gate_projection", "moe", "gate_projection", "weight",
            wi, lay, exp),
        row("moe", "qwen3_moe",
            p + f"mlp.experts.{exp}.up_proj.weight",
            c + f"moe.experts[{exp}].up_projection.weight",
            "moe_expert_up_projection", "moe", "up_projection", "weight",
            wi, lay, exp),
        row("moe", "qwen3_moe",
            p + f"mlp.experts.{exp}.down_proj.weight",
            c + f"moe.experts[{exp}].down_projection.weight",
            "moe_expert_down_projection", "moe", "down_projection", "weight",
            wi, lay, exp),
        # 12-14: the three tensors outside the layer stack. These carry no
        # layer index, and the canonical address omits the `layers[N]` segment
        # rather than substituting a zero.
        row("embed_tokens", "qwen2+qwen3", "model.embed_tokens.weight",
            "model.embedding.token_embedding.weight",
            "token_embedding", "embedding", "token_embedding", "weight", vocab),
        row("model.norm", "qwen2+qwen3", "model.norm.weight",
            "model.normalization.final_normalization.weight",
            "final_norm", "normalization", "final_normalization", "weight", hid),
        row("lm_head", "qwen2+qwen3", "lm_head.weight",
            "model.output_head.output_projection.weight",
            "lm_head", "output_head", "output_projection", "weight", vocab),
    ]


# Names a Qwen checkpoint really can contain and this plugin is deliberately NOT
# taught. Each must resolve to role `unknown` with no canonical address
# (`NSIR-001`). Listing them in the fixture makes the refusal a checked contract
# rather than an absence nobody notices.
QWEN_UNTAUGHT = [
    {
        "raw_name": f"model.layers.{QWEN_LAYER}.some_future_thing.weight",
        "why": "QM-0010 §Test Cases: a name from a release this plugin predates.",
    },
    {
        "raw_name": "model.layers.0.mlp.shared_expert.up_proj.weight",
        "why": ("Qwen2-MoE's always-on shared expert. Out of scope per QM-0010 "
                "§Scope, which names `experts.N.*` and `mlp.gate.weight` only. "
                "It is not the same object as a routed expert and must not be "
                "addressed as one."),
    },
    {
        "raw_name": "model.layers.0.mlp.shared_expert_gate.weight",
        "why": "Qwen2-MoE's shared-expert gate. Out of scope, as above.",
    },
    {
        "raw_name": "visual.blocks.3.attn.qkv.weight",
        "why": ("A Qwen-VL vision-tower name. Vision and audio towers are out "
                "of scope per QM-0010 §Out of Scope; the language plugin has no "
                "business claiming them."),
    },
    {
        "raw_name": "model.layers.abc.self_attn.q_proj.weight",
        "why": ("A layer index that is not an integer. The layer must stay "
                "absent rather than default to 0, and the suffix rule must not "
                "fire without one."),
    },
    {
        "raw_name": "model.layers.4294967296.self_attn.q_proj.weight",
        "why": ("A layer index one past `u32::MAX`. Out of range is absent, "
                "never truncated or wrapped."),
    },
]


def write_qwen(out_dir: Path) -> dict:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "config.json").write_text(json.dumps(QWEN_CONFIG, indent=2) + "\n")

    rows = _qwen_rows()
    labelled = sorted({r["family"] for r in rows})
    if labelled != sorted(QWEN_NAME_FAMILIES):
        raise ValueError(
            "every row must carry one of QM-0010's 15 name families and every "
            f"family must have a row; rows cover {labelled}"
        )
    golden = {
        "_comment": (
            "Generated by fixtures/generate_fixtures.py for QM-0010 / NSIR-006. "
            "A NAMING fixture: it carries no weight payload and no .safetensors "
            "file, because name resolution is what is under test and "
            "ARCHITECTURE.md section 4.2 forbids inferring a role from a shape. "
            "Every canonical_name was hand-written from the address rule in "
            "ARCHITECTURE.md section 6.1 and NOT produced by running the Rust "
            "resolver, so the Rust test asserting against this file is a "
            "cross-check rather than a tautology. The `variant` column records "
            "which Qwen release emits a name: this file describes the family's "
            "naming, not one checkpoint's contents."
        ),
        "fixture": "tiny-qwen-single",
        "task": "QM-0010",
        "requirement": "NSIR-006",
        "carries_weight_payload": False,
        "resolver_id": "qwen",
        "canonical_address_rule": (
            "model[.layers[N]].<component>[.experts[E]].<operation>"
            "[.<parameter>]  (ARCHITECTURE.md section 6.1)"
        ),
        "fidelity": "exact",
        "name_families": QWEN_NAME_FAMILIES,
        "name_family_count": len(QWEN_NAME_FAMILIES),
        "resolved": rows,
        "untaught": QWEN_UNTAUGHT,
    }
    (out_dir / "golden.json").write_text(json.dumps(golden, indent=2) + "\n")
    return golden


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(Path(__file__).resolve().parent))
    ap.add_argument("--llama", action="store_true",
                    help="write only the Llama fixtures")
    ap.add_argument("--qwen", action="store_true",
                    help="write only the Qwen naming fixture")
    args = ap.parse_args()
    root = Path(args.out)
    # No selector means every fixture, so CI's reproducibility gate covers all
    # of them without knowing their names.
    want_llama = args.llama or not (args.llama or args.qwen)
    want_qwen = args.qwen or not (args.llama or args.qwen)

    if want_llama:
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

    if want_qwen:
        qwen_dir = root / "tiny-qwen-single"
        qg = write_qwen(qwen_dir)
        print(f"wrote {qwen_dir} ({len(qg['resolved'])} names over "
              f"{qg['name_family_count']} families, {len(qg['untaught'])} untaught, "
              f"no weight payload)")


if __name__ == "__main__":
    main()
