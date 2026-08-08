#!/usr/bin/env python3
"""
gpt2_server.py — serve a real GPT-2 checkpoint to the `mm` matrix-multiplication viewer.

`mm` renders a tree of matmuls whose leaf matrices are filled from CSV over HTTP
(`viz.js` `INIT_FUNCS` / `tryURLInit`).  This server is the other end of that
wire for `models/distilgpt2`:

  * it reads `model.safetensors` by **byte range** — a column slice of
    `c_attn.weight` is 768 reads of 256 bytes, not a 352 MB load — so no weight
    file is ever materialised on disk as CSV and resident memory stays bounded;
  * it derives the per-head projections `mm` wants (wQ, wK_t, wV, wO) from the
    fused GPT-2 Conv1D tensors;
  * it runs a real forward pass so the `input` leaf is the model's own residual
    stream for a prompt you type, not noise;
  * it serves `mm/` itself, so the viewer and the data are same-origin.

Fidelity labelling (this repo never presents a sampled figure as exact):

  * `rs=1,cs=1` -> **exact**: every element of the slice is the checkpoint's own.
  * `rs>1`/`cs>1` -> **sampled**: every n-th row/column, no interpolation.  Used
    to keep 768x3072 MLP and 768x50257 vocabulary matrices inside a browser
    point budget.  The driver page only ever decimates an axis that is *not* a
    contraction axis of the matmul it feeds, so a rendered product is never a
    partial sum wearing the full result's name.
  * activations -> **exact** for the prompt given, when numpy is present.
    Without numpy only layer 0 is available (embedding gather + LayerNorm in
    pure stdlib); deeper layers return 501 rather than a plausible-looking fake.

Nothing here writes to the checkpoint, and nothing here needs the network.

    python3 tools/gpt2_server.py                 # stdlib only: layer 0 activations
    ../.venv/bin/python tools/gpt2_server.py     # with numpy: all layers

then open http://127.0.0.1:8000/examples/gpt2/
"""

from __future__ import annotations

import argparse
import json
import math
import re
import struct
import sys
import threading
import urllib.parse
from array import array
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

try:
    import numpy as np
except ImportError:  # optional — see module docstring
    np = None

MM_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MODEL = MM_ROOT / "models" / "distilgpt2"

# safetensors dtype -> (array typecode, itemsize, numpy dtype string)
DTYPES = {
    "F32": ("f", 4, "<f4"),
    "F64": ("d", 8, "<f8"),
}


# ---------------------------------------------------------------------------
# safetensors: header parse + byte-range block reads
# ---------------------------------------------------------------------------


def as_2d(shape):
    """Collapse a safetensors shape to (h, w), dropping leading unit axes."""
    dims = [d for i, d in enumerate(shape) if d != 1 or i >= len(shape) - 2]
    if len(dims) == 1:
        return 1, dims[0]
    if len(dims) == 2:
        return dims[0], dims[1]
    raise ValueError(f"cannot view shape {shape} as a matrix")


class SafeTensorsStore:
    """Random access into a .safetensors file without loading it."""

    def __init__(self, path: Path):
        self.path = Path(path)
        with self.path.open("rb") as f:
            (header_len,) = struct.unpack("<Q", f.read(8))
            self.header = json.loads(f.read(header_len))
        self.data_start = 8 + header_len
        self.meta = {k: v for k, v in self.header.items() if k != "__metadata__"}
        self._fh = self.path.open("rb")
        self._lock = threading.Lock()

    def close(self):
        self._fh.close()

    def shape(self, name):
        return as_2d(self.meta[name]["shape"])

    def _read(self, offset: int, nbytes: int) -> bytes:
        with self._lock:
            self._fh.seek(offset)
            return self._fh.read(nbytes)

    def block(self, name, r0=0, r1=None, c0=0, c1=None, rstride=1, cstride=1):
        """A strided 2-D block of `name`, read by byte range.

        Returns (flat array of floats, nrows, ncols).
        """
        m = self.meta[name]
        code, isize, _ = DTYPES[m["dtype"]]
        h, w = as_2d(m["shape"])
        r1 = h if r1 is None else min(r1, h)
        c1 = w if c1 is None else min(c1, w)
        base = self.data_start + m["data_offsets"][0]

        rows = range(r0, r1, rstride)
        cols = range(c0, c1, cstride)
        out = array(code)

        if rstride == 1 and cstride == 1 and c0 == 0 and c1 == w:
            # whole rows, contiguous: one read
            out.frombytes(self._read(base + r0 * w * isize, (r1 - r0) * w * isize))
        else:
            span = c1 - c0
            for i in rows:
                chunk = array(code)
                chunk.frombytes(self._read(base + (i * w + c0) * isize, span * isize))
                out.extend(chunk[::cstride] if cstride > 1 else chunk)

        if sys.byteorder == "big":
            out.byteswap()
        return out, len(rows), len(cols)

    def gather_rows(self, name, indices):
        """Rows at arbitrary indices — one byte-range read per row."""
        m = self.meta[name]
        code, isize, _ = DTYPES[m["dtype"]]
        _, w = as_2d(m["shape"])
        base = self.data_start + m["data_offsets"][0]
        out = array(code)
        for i in indices:
            chunk = array(code)
            chunk.frombytes(self._read(base + i * w * isize, w * isize))
            out.extend(chunk)
        if sys.byteorder == "big":
            out.byteswap()
        return out, len(indices), w

    def numpy(self, name):
        """Whole tensor as a numpy array (used only by the forward pass)."""
        m = self.meta[name]
        _, isize, npdt = DTYPES[m["dtype"]]
        start, end = m["data_offsets"]
        buf = self._read(self.data_start + start, end - start)
        return np.frombuffer(buf, dtype=npdt).reshape(m["shape"])


def transpose(flat, h, w):
    if np is not None:
        a = np.frombuffer(memoryview(flat), dtype=flat.typecode).reshape(h, w)
        return array(flat.typecode, a.T.copy().tobytes()), w, h
    out = array(flat.typecode, bytes(len(flat) * flat.itemsize))
    for i in range(h):
        base = i * w
        for j in range(w):
            out[j * h + i] = flat[base + j]
    return out, w, h


def to_csv(flat, h, w, fmt="%.6g"):
    lines = []
    for i in range(h):
        row = flat[i * w : (i + 1) * w]
        lines.append(",".join([fmt % x for x in row]))
    return ("\n".join(lines) + "\n").encode("ascii")


# ---------------------------------------------------------------------------
# GPT-2 byte-level BPE (vocab.json + merges.txt, stdlib only)
# ---------------------------------------------------------------------------


def bytes_to_unicode():
    bs = (
        list(range(ord("!"), ord("~") + 1))
        + list(range(ord("\xa1"), ord("\xac") + 1))
        + list(range(ord("\xae"), ord("\xff") + 1))
    )
    cs = bs[:]
    n = 0
    for b in range(256):
        if b not in bs:
            bs.append(b)
            cs.append(256 + n)
            n += 1
    return dict(zip(bs, (chr(c) for c in cs)))


# stdlib stand-in for GPT-2's \p{L} / \p{N} pattern: [^\W\d_] is "word char that
# is neither digit nor underscore", i.e. a letter; underscore rejoins punctuation.
TOKEN_PAT = re.compile(
    r"'s|'t|'re|'ve|'m|'ll|'d| ?[^\W\d_]+| ?\d+| ?(?:[^\s\w]|_)+|\s+(?!\S)|\s+",
    re.UNICODE,
)


class BPETokenizer:
    def __init__(self, model_dir: Path):
        self.encoder = json.loads((model_dir / "vocab.json").read_text(encoding="utf-8"))
        merges = (model_dir / "merges.txt").read_text(encoding="utf-8").split("\n")
        merges = [m for m in merges if m and not m.startswith("#version")]
        self.ranks = {tuple(m.split()): i for i, m in enumerate(merges)}
        self.byte_encoder = bytes_to_unicode()
        self.decoder = {v: k for k, v in self.encoder.items()}
        self._cache = {}

    def _bpe(self, token: str) -> list[str]:
        if token in self._cache:
            return self._cache[token]
        word = tuple(token)
        while len(word) > 1:
            pairs = set(zip(word[:-1], word[1:]))
            bigram = min(pairs, key=lambda p: self.ranks.get(p, math.inf))
            if bigram not in self.ranks:
                break
            first, second = bigram
            new_word, i = [], 0
            while i < len(word):
                try:
                    j = word.index(first, i)
                except ValueError:
                    new_word.extend(word[i:])
                    break
                new_word.extend(word[i:j])
                i = j
                if i < len(word) - 1 and word[i + 1] == second:
                    new_word.append(first + second)
                    i += 2
                else:
                    new_word.append(word[i])
                    i += 1
            word = tuple(new_word)
        result = list(word)
        self._cache[token] = result
        return result

    def encode(self, text: str):
        ids, pieces = [], []
        for match in TOKEN_PAT.findall(text):
            token = "".join(self.byte_encoder[b] for b in match.encode("utf-8"))
            for piece in self._bpe(token):
                if piece in self.encoder:
                    ids.append(self.encoder[piece])
                    pieces.append(piece)
        return ids, pieces


# ---------------------------------------------------------------------------
# activations
# ---------------------------------------------------------------------------


def layernorm_rows(flat, h, w, weight, bias, eps=1e-5):
    """GPT-2 LayerNorm: per row over the last axis. In place, stdlib."""
    for i in range(h):
        base = i * w
        row = flat[base : base + w]
        mean = sum(row) / w
        var = sum((x - mean) ** 2 for x in row) / w
        denom = math.sqrt(var + eps)
        for j in range(w):
            flat[base + j] = (row[j] - mean) / denom * weight[j] + bias[j]
    return flat


class Activations:
    """Residual-stream states for a prompt.

    Layer 0 is an embedding gather plus a LayerNorm — cheap and exact in pure
    stdlib.  Deeper layers need the actual forward pass, hence numpy.
    """

    def __init__(self, store: SafeTensorsStore, cfg: dict):
        self.store = store
        self.cfg = cfg
        self._cache = {}
        self._lock = threading.Lock()

    # -- stdlib path: layer 0 only ------------------------------------------

    def _embed(self, ids):
        n_embd = self.cfg["n_embd"]
        emb, n, _ = self.store.gather_rows("transformer.wte.weight", ids)
        pos, _, _ = self.store.block("transformer.wpe.weight", 0, len(ids))
        for k in range(n * n_embd):
            emb[k] += pos[k]
        return emb, n

    def _vec(self, name):
        flat, _, _ = self.store.block(name)
        return flat

    def _layer0_stdlib(self, ids):
        n_embd = self.cfg["n_embd"]
        resid, n = self._embed(ids)
        ln1 = array(resid.typecode, resid)
        layernorm_rows(
            ln1, n, n_embd,
            self._vec("transformer.h.0.ln_1.weight"),
            self._vec("transformer.h.0.ln_1.bias"),
        )
        return {"resid": (resid, n), "ln_1": (ln1, n)}

    # -- numpy path: full forward -------------------------------------------

    @staticmethod
    def _ln(x, w, b, eps=1e-5):
        mu = x.mean(axis=-1, keepdims=True)
        var = x.var(axis=-1, keepdims=True)
        return (x - mu) / np.sqrt(var + eps) * w + b

    @staticmethod
    def _gelu_new(x):
        return 0.5 * x * (1.0 + np.tanh(math.sqrt(2.0 / math.pi) * (x + 0.044715 * x**3)))

    def _forward(self, ids):
        s = self.store
        n_head, n_embd = self.cfg["n_head"], self.cfg["n_embd"]
        head_dim = n_embd // n_head
        n = len(ids)

        wte = s.gather_rows("transformer.wte.weight", ids)[0]
        x = np.array(wte, dtype=np.float32).reshape(n, n_embd)
        x = x + s.numpy("transformer.wpe.weight")[:n].astype(np.float32)

        mask = np.tril(np.ones((n, n), dtype=bool))
        out = {"resid": {}, "ln_1": {}, "ln_2": {}, "attn_out": {}, "mlp_h": {}}

        for l in range(self.cfg["n_layer"]):
            p = f"transformer.h.{l}."
            out["resid"][l] = x.copy()

            h = self._ln(x, s.numpy(p + "ln_1.weight"), s.numpy(p + "ln_1.bias"))
            out["ln_1"][l] = h.copy()

            qkv = h @ s.numpy(p + "attn.c_attn.weight") + s.numpy(p + "attn.c_attn.bias")
            q, k, v = np.split(qkv, 3, axis=-1)
            heads = []
            for hd in range(n_head):
                sl = slice(hd * head_dim, (hd + 1) * head_dim)
                scores = q[:, sl] @ k[:, sl].T / math.sqrt(head_dim)
                scores = np.where(mask, scores, -np.inf)
                scores = scores - scores.max(axis=-1, keepdims=True)
                probs = np.exp(scores)
                probs /= probs.sum(axis=-1, keepdims=True)
                heads.append(probs @ v[:, sl])
            attn = np.concatenate(heads, axis=-1)
            out["attn_out"][l] = attn.copy()
            x = x + attn @ s.numpy(p + "attn.c_proj.weight") + s.numpy(p + "attn.c_proj.bias")

            h2 = self._ln(x, s.numpy(p + "ln_2.weight"), s.numpy(p + "ln_2.bias"))
            out["ln_2"][l] = h2.copy()

            ff = self._gelu_new(h2 @ s.numpy(p + "mlp.c_fc.weight") + s.numpy(p + "mlp.c_fc.bias"))
            out["mlp_h"][l] = ff.copy()
            x = x + ff @ s.numpy(p + "mlp.c_proj.weight") + s.numpy(p + "mlp.c_proj.bias")

        out["final"] = self._ln(
            x, s.numpy("transformer.ln_f.weight"), s.numpy("transformer.ln_f.bias")
        )
        return out

    # -- public --------------------------------------------------------------

    def get(self, ids, kind, layer):
        """(flat, h, w) for an activation matrix, or raise NotImplementedError."""
        key = tuple(ids)
        n_embd = self.cfg["n_embd"]
        with self._lock:
            if np is None:
                if layer != 0 or kind not in ("resid", "ln_1"):
                    raise NotImplementedError(
                        f"activation '{kind}' at layer {layer} needs a forward pass; "
                        "run this server with numpy available "
                        "(e.g. ../.venv/bin/python tools/gpt2_server.py). "
                        "Without it only layer 0 'resid' and 'ln_1' are exact."
                    )
                cache = self._cache.get(("stdlib", key))
                if cache is None:
                    cache = self._cache[("stdlib", key)] = self._layer0_stdlib(ids)
                flat, n = cache[kind]
                return array(flat.typecode, flat), n, n_embd

            cache = self._cache.get(key)
            if cache is None:
                cache = self._forward(ids)
                self._cache[key] = cache
            mat = cache["final"] if kind == "final" else cache[kind][layer]
            flat = array("f", mat.astype(np.float32).ravel().tobytes())
            return flat, mat.shape[0], mat.shape[1]


# ---------------------------------------------------------------------------
# logical matrix resolution
# ---------------------------------------------------------------------------

WEIGHT_KINDS = {
    # kind: (tensor template, needs_head, transpose)
    "wq": ("transformer.h.{l}.attn.c_attn.weight", True, False),
    "wk": ("transformer.h.{l}.attn.c_attn.weight", True, False),
    "wk_t": ("transformer.h.{l}.attn.c_attn.weight", True, True),
    "wv": ("transformer.h.{l}.attn.c_attn.weight", True, False),
    "wo": ("transformer.h.{l}.attn.c_proj.weight", True, False),
    "c_attn": ("transformer.h.{l}.attn.c_attn.weight", False, False),
    "attn_c_proj": ("transformer.h.{l}.attn.c_proj.weight", False, False),
    "c_fc": ("transformer.h.{l}.mlp.c_fc.weight", False, False),
    "mlp_c_proj": ("transformer.h.{l}.mlp.c_proj.weight", False, False),
    "wte": ("transformer.wte.weight", False, False),
    "wte_t": ("transformer.wte.weight", False, True),
    "wpe": ("transformer.wpe.weight", False, False),
}

# activation kind -> width in units of n_embd (mlp_h is the 4x GELU hidden state)
ACT_KINDS = {"resid": 1, "ln_1": 1, "ln_2": 1, "attn_out": 1, "mlp_h": 4, "final": 1}


class Model:
    def __init__(self, model_dir: Path):
        self.dir = Path(model_dir)
        self.config = json.loads((self.dir / "config.json").read_text())
        self.store = SafeTensorsStore(self.dir / "model.safetensors")
        self.tokenizer = BPETokenizer(self.dir)
        self.cfg = {
            "n_layer": self.config["n_layer"],
            "n_head": self.config["n_head"],
            "n_embd": self.config["n_embd"],
            "n_ctx": self.config["n_ctx"],
            "vocab_size": self.config["vocab_size"],
        }
        self.cfg["head_dim"] = self.cfg["n_embd"] // self.cfg["n_head"]
        self.acts = Activations(self.store, self.cfg)

    # -- weight slices -------------------------------------------------------

    def weight_extent(self, kind, layer, head):
        """Full (unstrided) shape of a logical weight, and its slice bounds."""
        tmpl, needs_head, want_t = WEIGHT_KINDS[kind]
        name = tmpl.format(l=layer)
        h, w = self.store.shape(name)
        d = self.cfg["head_dim"]
        r0, r1, c0, c1 = 0, h, 0, w
        if needs_head:
            if kind == "wo":
                r0, r1 = head * d, (head + 1) * d  # rows of attn.c_proj
            else:
                base = {"wq": 0, "wk": 1, "wk_t": 1, "wv": 2}[kind] * self.cfg["n_embd"]
                c0, c1 = base + head * d, base + (head + 1) * d
        return name, (r0, r1, c0, c1), want_t

    def weight(self, kind, layer, head, rstride=1, cstride=1):
        """Strides address the *stored* tensor's axes, before any built-in
        transpose (so `wte_t` decimates vocabulary with `rs`, not `cs`)."""
        name, (r0, r1, c0, c1), builtin_t = self.weight_extent(kind, layer, head)
        flat, h, w = self.store.block(name, r0, r1, c0, c1, rstride, cstride)
        if builtin_t:
            flat, h, w = transpose(flat, h, w)
        return flat, h, w

    # -- activations ---------------------------------------------------------

    def token_ids(self, text, seq):
        ids, pieces = self.tokenizer.encode(text)
        if not ids:
            ids, pieces = [self.config["bos_token_id"]], ["<|endoftext|>"]
        limit = min(seq, self.cfg["n_ctx"]) if seq else self.cfg["n_ctx"]
        return ids[:limit], pieces[:limit]

    def activation(self, kind, layer, ids, rstride=1, cstride=1):
        flat, h, w = self.acts.get(ids, kind, layer)
        if rstride > 1 or cstride > 1:
            rows = range(0, h, rstride)
            out = array(flat.typecode)
            for i in rows:
                out.extend(flat[i * w : (i + 1) * w][::cstride])
            flat, h, w = out, len(rows), len(range(0, w, cstride))
        return flat, h, w

    # -- the single source of truth for shapes -------------------------------

    def activation_available(self, kind, layer):
        """Whether an activation can be served honestly, and why not if it can't.

        Reported through specs.json so the driver page can refuse up front. If it
        only found out at CSV-fetch time it would receive a 501 JSON body, and
        mm's `+s` coercion would silently turn that into a matrix of NaN.
        """
        if np is not None:
            return True, None
        if layer == 0 and kind in ("resid", "ln_1"):
            return True, None
        return False, (
            f"'{kind}' at layer {layer} needs the forward pass, which needs numpy. "
            "Restart the server with it available (e.g. ../.venv/bin/python "
            "tools/gpt2_server.py); without it only layer 0 resid and ln_1 exist."
        )

    def spec(self, kind, layer, head, rstride, cstride, want_t, ids):
        """Shape + fidelity of a logical matrix, without reading its data.

        `route_matrix` asserts the data it emits matches this, so a shape
        disagreement fails loudly instead of being silently tiled by mm's
        `tryURLInit` (which wraps with `i % data.length`).
        """
        available, reason = True, None
        if kind in ACT_KINDS:
            r0, r1 = 0, len(ids)
            c0, c1 = 0, self.cfg["n_embd"] * ACT_KINDS[kind]
            builtin_t = False
            available, reason = self.activation_available(kind, layer)
        else:
            _, (r0, r1, c0, c1), builtin_t = self.weight_extent(kind, layer, head)
        h = len(range(r0, r1, rstride))
        w = len(range(c0, c1, cstride))
        if builtin_t:
            h, w = w, h
        if want_t:
            h, w = w, h
        return {
            "h": h,
            "w": w,
            "fidelity": "exact" if rstride == 1 and cstride == 1 else "sampled",
            "available": available,
            "reason": reason,
        }


# ---------------------------------------------------------------------------
# HTTP
# ---------------------------------------------------------------------------


class Handler(SimpleHTTPRequestHandler):
    model: Model = None  # set in main()

    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(MM_ROOT), **kw)

    def log_message(self, fmt, *args):
        sys.stderr.write("  %s\n" % (fmt % args))

    # -- helpers -------------------------------------------------------------

    def _send(self, body: bytes, ctype: str, extra=None):
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Cache-Control", "public, max-age=3600")
        for k, v in (extra or {}).items():
            self.send_header(k, str(v))
        self.end_headers()
        self.wfile.write(body)

    def _json(self, obj, code=200):
        body = json.dumps(obj).encode()
        if code == 200:
            self._send(body, "application/json")
        else:
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(body)

    @staticmethod
    def _q(qs, key, default=None, cast=str):
        vals = qs.get(key)
        if not vals:
            return default
        return cast(vals[0])

    def _ids(self, qs):
        text = self._q(qs, "text", "")
        seq = self._q(qs, "seq", 0, int)
        return self.model.token_ids(text, seq)[0]

    # -- routes --------------------------------------------------------------

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        if not parsed.path.startswith("/gpt2/"):
            return super().do_GET()
        qs = urllib.parse.parse_qs(parsed.query)
        route = parsed.path[len("/gpt2/") :]
        try:
            handler = {
                "meta.json": self.route_meta,
                "tokens.json": self.route_tokens,
                "specs.json": self.route_specs,
                "matrix.csv": self.route_matrix,
            }.get(route)
            if handler is None:
                return self._json({"error": f"unknown route '{route}'"}, 404)
            return handler(qs)
        except NotImplementedError as e:
            return self._json({"error": str(e), "kind": "NotImplemented"}, 501)
        except Exception as e:  # surface the real reason, never a fake result
            return self._json({"error": f"{type(e).__name__}: {e}"}, 400)

    def route_meta(self, qs):
        m = self.model
        tensors = [
            {"name": n, "dtype": v["dtype"], "shape": v["shape"],
             "bytes": v["data_offsets"][1] - v["data_offsets"][0]}
            for n, v in sorted(m.store.meta.items())
        ]
        return self._json({
            "model_dir": str(m.dir),
            "config": m.config,
            "dims": m.cfg,
            "numpy": np is not None,
            "deep_layers": np is not None,
            "checkpoint_bytes": m.store.path.stat().st_size,
            "tensors": tensors,
        })

    def route_tokens(self, qs):
        ids, pieces = self.model.token_ids(
            self._q(qs, "text", ""), self._q(qs, "seq", 0, int)
        )
        return self._json({
            "ids": ids,
            "tokens": [p.replace("Ġ", " ").replace("Ċ", "\\n") for p in pieces],
            "n": len(ids),
        })

    def route_specs(self, qs):
        """Shapes + URLs for a batch of logical matrices.

        The driver page builds its mm params tree from exactly these numbers, so
        a leaf's declared `h`/`w` always come from the same code that will emit
        the CSV.  Request items are `kind[:flags]`; flags are `t` (transpose),
        `r` (apply the stride to rows), `c` (apply it to columns).
        """
        m = self.model
        layer = self._q(qs, "layer", 0, int)
        head = self._q(qs, "head", 0, int)
        stride = max(1, self._q(qs, "stride", 1, int))
        text = self._q(qs, "text", "")
        seq = self._q(qs, "seq", 0, int)
        ids = m.token_ids(text, seq)[0]
        out = {}
        for item in (self._q(qs, "kinds", "") or "").split(","):
            if not item:
                continue
            kind, _, flags = item.partition(":")
            if kind not in WEIGHT_KINDS and kind not in ACT_KINDS:
                return self._json({"error": f"unknown matrix kind '{kind}'"}, 400)
            want_t = "t" in flags
            rs = stride if "r" in flags else 1
            cs = stride if "c" in flags else 1
            spec = m.spec(kind, layer, head, rs, cs, want_t, ids)
            params = {"kind": kind, "layer": layer, "head": head,
                      "rs": rs, "cs": cs, "t": 1 if want_t else 0}
            if kind in ACT_KINDS:
                params.update({"text": text, "seq": seq})
            spec["url"] = "/gpt2/matrix.csv?" + urllib.parse.urlencode(params)
            spec["kind"] = kind
            out[item] = spec
        return self._json({"specs": out, "n_tokens": len(ids)})

    def route_matrix(self, qs):
        m = self.model
        kind = self._q(qs, "kind")
        layer = self._q(qs, "layer", 0, int)
        head = self._q(qs, "head", 0, int)
        rs = max(1, self._q(qs, "rs", 1, int))
        cs = max(1, self._q(qs, "cs", 1, int))
        want_t = self._q(qs, "t", 0, int) == 1

        if kind in ACT_KINDS:
            ids = self._ids(qs)
            flat, h, w = m.activation(kind, layer, ids, rs, cs)
        elif kind in WEIGHT_KINDS:
            ids = []
            flat, h, w = m.weight(kind, layer, head, rs, cs)
        else:
            return self._json({"error": f"unknown matrix kind '{kind}'"}, 400)
        if want_t:
            flat, h, w = transpose(flat, h, w)

        expected = m.spec(kind, layer, head, rs, cs, want_t, ids)
        if (h, w) != (expected["h"], expected["w"]):
            raise RuntimeError(
                f"shape disagreement for {kind}: emitted {h}x{w}, "
                f"specs.json promised {expected['h']}x{expected['w']}"
            )

        return self._send(
            to_csv(flat, h, w),
            "text/csv",
            {"X-Matrix-Shape": f"{h},{w}",
             "X-Matrix-Fidelity": expected["fidelity"]},
        )


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[1])
    ap.add_argument("--model", default=str(DEFAULT_MODEL), help="model directory")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8000)
    args = ap.parse_args()

    model_dir = Path(args.model)
    weights = model_dir / "model.safetensors"
    if not weights.exists():
        sys.exit(
            f"no checkpoint at {weights}\n"
            "models/ holds large local-only files; see CLAUDE.md 'Gotchas'."
        )

    Handler.model = Model(model_dir)
    dims = Handler.model.cfg
    print(f"model    {model_dir}")
    print(f"         {len(Handler.model.store.meta)} tensors, "
          f"{weights.stat().st_size:,} bytes, read by byte range (never fully loaded)")
    print(f"         n_layer={dims['n_layer']} n_head={dims['n_head']} "
          f"n_embd={dims['n_embd']} head_dim={dims['head_dim']}")
    print(f"activations  {'exact, all layers (numpy)' if np else 'layer 0 only (no numpy)'}")
    print(f"\n  http://{args.host}:{args.port}/examples/gpt2/\n")

    ThreadingHTTPServer((args.host, args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
