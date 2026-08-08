#!/usr/bin/env python3
"""
check_bias.py — prove the augmented matmuls draw GPT-2's own affine maps.

`mm` has no `+` operator, so every checkpoint view used to draw `X @ W`
where GPT-2 computes `X @ W + b`.  `gpt2_server.py` now closes that by
augmenting the operands — `[X | 1] @ [W ; b]` — and this script checks the
claim end to end, over HTTP, on the CSVs the pages actually request:

  * the appended row/column really is the checkpoint's own bias, sliced to the
    right head and strided with the right axis (or really is all ones);
  * the product mm will draw from those CSVs equals the value GPT-2 computes.

The reference is **independent of the server**: this file reads the
`.safetensors` with the `safetensors` package and runs its own forward pass in
numpy, following the HuggingFace GPT-2 definition.  Nothing here is compared
against `gpt2_server.py`'s own arithmetic — only against that reference.  The
one thing it borrows from the server is the token ids, because both sides must
see the same prompt; tokenization is not what is being verified.

The tolerance is set by the wire, not by the arithmetic: `to_csv` writes
`%.6g`, so ~6 significant digits is the most any CSV-fed product can agree to.

    ../.venv/bin/python tools/gpt2_server.py --port 8123 &
    ../.venv/bin/python tools/check_bias.py --port 8123

Needs numpy and safetensors, a running server, and the local checkpoint.
Exits nonzero on the first product that does not match.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
import urllib.parse
import urllib.request
from pathlib import Path

import numpy as np
from safetensors import safe_open

MM_ROOT = Path(__file__).resolve().parent.parent

PROMPT = "The capital of France is Paris, and the capital of Italy is Rome."
SEQ = 16

# %.6g on the wire. Products contract over ~768 terms, so the per-element error
# accumulates: compare on a relative scale against the magnitude of the operands.
RTOL = 2e-5


# ---------------------------------------------------------------------------
# the server, over HTTP — exactly what the browser page does
# ---------------------------------------------------------------------------


class Server:
    def __init__(self, base):
        self.base = base.rstrip("/")

    def json(self, route, **q):
        url = f"{self.base}/api/{route}?" + urllib.parse.urlencode(q)
        with urllib.request.urlopen(url) as r:
            return json.load(r)

    def specs(self, kinds, **q):
        d = self.json("specs.json", kinds=",".join(kinds), **q)
        if "error" in d:
            raise SystemExit(f"specs.json refused: {d['error']}")
        return d["specs"]

    def matrix(self, spec):
        """The CSV a leaf is filled from, as an array of the promised shape."""
        with urllib.request.urlopen(self.base + spec["url"]) as r:
            body = r.read().decode()
        rows = [[float(x) for x in line.split(",")] for line in body.strip().split("\n")]
        a = np.array(rows, dtype=np.float64)
        if a.shape != (spec["h"], spec["w"]):
            raise SystemExit(
                f"CSV is {a.shape}, specs.json promised ({spec['h']}, {spec['w']})"
            )
        return a


# ---------------------------------------------------------------------------
# the reference — safetensors + numpy, owing nothing to gpt2_server.py
# ---------------------------------------------------------------------------


class Reference:
    def __init__(self, model_dir: Path):
        self.cfg = json.loads((model_dir / "config.json").read_text())
        self.f = safe_open(str(model_dir / "model.safetensors"), framework="np")

    def t(self, name):
        return self.f.get_tensor(name).astype(np.float64)

    @staticmethod
    def ln(x, w, b, eps=1e-5):
        mu = x.mean(-1, keepdims=True)
        var = x.var(-1, keepdims=True)
        return (x - mu) / np.sqrt(var + eps) * w + b

    @staticmethod
    def gelu_tanh(x):
        """GPT-2's own GELU (`gelu_new`) — what the checkpoint was trained with."""
        return 0.5 * x * (1 + np.tanh(math.sqrt(2 / math.pi) * (x + 0.044715 * x**3)))

    @staticmethod
    def gelu_mm(x):
        """mm's GELU, constant for constant from `viz.ts`.

        Not `math.erf`: viz.ts computes erf with the Abramowitz & Stegun 7.1.26
        rational approximation, so the gap this measures is mm's whole GELU
        against GPT-2's, which is what the picture shows.
        """
        a = (0.254829592, -0.284496736, 1.421413741, -1.453152027, 1.061405429)
        p = 0.3275911
        z = np.abs(x / math.sqrt(2))
        t = 1.0 / (1.0 + p * z)
        y = ((((a[4] * t + a[3]) * t + a[2]) * t + a[1]) * t + a[0]) * t
        erf = np.sign(x / math.sqrt(2)) * (1 - y * np.exp(-z * z))
        return x * (1 + erf) / 2

    def forward(self, ids, upto_layer):
        """Every intermediate the views name, for layers 0..upto_layer."""
        n_head, n_embd = self.cfg["n_head"], self.cfg["n_embd"]
        head_dim = n_embd // n_head
        n = len(ids)

        x = self.t("transformer.wte.weight")[ids] + self.t("transformer.wpe.weight")[:n]
        mask = np.tril(np.ones((n, n), dtype=bool))
        out = {}

        for l in range(upto_layer + 1):
            p = f"transformer.h.{l}."
            g = {}
            g["resid"] = x.copy()
            h = self.ln(x, self.t(p + "ln_1.weight"), self.t(p + "ln_1.bias"))
            g["ln_1"] = h.copy()

            g["c_attn_w"] = self.t(p + "attn.c_attn.weight")
            g["c_attn_b"] = self.t(p + "attn.c_attn.bias")
            qkv = h @ g["c_attn_w"] + g["c_attn_b"]
            g["qkv"] = qkv
            q, k, v = np.split(qkv, 3, axis=-1)

            heads, probs_per_head = [], []
            for hd in range(n_head):
                sl = slice(hd * head_dim, (hd + 1) * head_dim)
                scores = q[:, sl] @ k[:, sl].T / math.sqrt(head_dim)
                scores = np.where(mask, scores, -np.inf)
                scores = scores - scores.max(-1, keepdims=True)
                pr = np.exp(scores)
                pr /= pr.sum(-1, keepdims=True)
                probs_per_head.append(pr)
                heads.append(pr @ v[:, sl])
            g["probs"] = probs_per_head
            g["heads"] = heads
            attn = np.concatenate(heads, axis=-1)
            g["attn_out"] = attn.copy()

            g["attn_proj_w"] = self.t(p + "attn.c_proj.weight")
            g["attn_proj_b"] = self.t(p + "attn.c_proj.bias")
            x = x + attn @ g["attn_proj_w"] + g["attn_proj_b"]

            h2 = self.ln(x, self.t(p + "ln_2.weight"), self.t(p + "ln_2.bias"))
            g["ln_2"] = h2.copy()
            g["c_fc_w"] = self.t(p + "mlp.c_fc.weight")
            g["c_fc_b"] = self.t(p + "mlp.c_fc.bias")
            g["mlp_pre"] = h2 @ g["c_fc_w"] + g["c_fc_b"]
            ff = self.gelu_tanh(g["mlp_pre"])
            g["mlp_h"] = ff.copy()
            g["mlp_proj_w"] = self.t(p + "mlp.c_proj.weight")
            g["mlp_proj_b"] = self.t(p + "mlp.c_proj.bias")
            x = x + ff @ g["mlp_proj_w"] + g["mlp_proj_b"]
            out[l] = g

        return out


# ---------------------------------------------------------------------------
# checks
# ---------------------------------------------------------------------------

FAILURES = []


def check(label, got, want, scale=None):
    """Report max relative deviation, against the magnitude of the result."""
    got, want = np.asarray(got, float), np.asarray(want, float)
    if got.shape != want.shape:
        FAILURES.append(label)
        print(f"  FAIL  {label}: shape {got.shape} vs reference {want.shape}")
        return
    denom = scale if scale is not None else max(float(np.abs(want).max()), 1e-12)
    err = float(np.abs(got - want).max()) / denom
    ok = err <= RTOL
    if not ok:
        FAILURES.append(label)
    print(f"  {'ok  ' if ok else 'FAIL'}  {label:52} {got.shape[0]:>5}x{got.shape[1]:<5}"
          f" max rel dev {err:.2e}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[1])
    ap.add_argument("--port", type=int, default=8000)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--layer", type=int, default=1)
    ap.add_argument("--head", type=int, default=3)
    ap.add_argument("--model", default=str(MM_ROOT / "models" / "distilgpt2"))
    args = ap.parse_args()

    srv = Server(f"http://{args.host}:{args.port}")
    ref = Reference(Path(args.model))
    L, H = args.layer, args.head
    n_embd = ref.cfg["n_embd"]
    head_dim = n_embd // ref.cfg["n_head"]
    hsl = slice(H * head_dim, (H + 1) * head_dim)

    ids = srv.json("tokens.json", text=PROMPT, seq=SEQ)["ids"]
    g = ref.forward(ids, L)[L]
    n = len(ids)
    print(f"layer {L}, head {H}, {n} tokens, tolerance {RTOL:g} relative "
          f"(the CSV wire is %.6g)\n")

    q = dict(layer=L, head=H, text=PROMPT, seq=SEQ)

    # -- the appended vectors are what they claim to be ----------------------
    print("appended vector is the checkpoint's own bias (or a true constant 1):")
    s = srv.specs(["ln_1:w", "wq:h", "wk_t:w", "c_attn:ch"], stride=2, **q)
    check("ones column on ln_1", srv.matrix(s["ln_1:w"])[:, -1:],
          np.ones((n, 1)), scale=1.0)
    bq, bk, bv = np.split(g["c_attn_b"], 3)
    check("bias row on wq == c_attn.bias[Q, head]",
          srv.matrix(s["wq:h"])[-1:, :], bq[hsl][None, :])
    check("bias column on wk_t == c_attn.bias[K, head]",
          srv.matrix(s["wk_t:w"])[:, -1:], bk[hsl][:, None])
    check("bias row on c_attn, strided with its columns",
          srv.matrix(s["c_attn:ch"])[-1:, :], g["c_attn_b"][None, ::2])

    # -- the products mm will draw ------------------------------------------
    print("\nthe product mm draws == the value GPT-2 computes:")

    s = srv.specs(["ln_1:w", "c_attn:ch"], stride=2, **q)
    check("qkv projection  ln_1|1 @ c_attn;b",
          srv.matrix(s["ln_1:w"]) @ srv.matrix(s["c_attn:ch"]), g["qkv"][:, ::2])

    s = srv.specs(["attn_out:w", "attn_c_proj:ch"], stride=1, **q)
    check("attention output  heads|1 @ c_proj;b",
          srv.matrix(s["attn_out:w"]) @ srv.matrix(s["attn_c_proj:ch"]),
          g["attn_out"] @ g["attn_proj_w"] + g["attn_proj_b"])

    s = srv.specs(["ln_2:w", "c_fc:ch"], stride=4, **q)
    check("mlp up, before GELU  ln_2|1 @ c_fc;b",
          srv.matrix(s["ln_2:w"]) @ srv.matrix(s["c_fc:ch"]), g["mlp_pre"][:, ::4])

    # only c_proj's *columns* are decimated — the 3072-long contraction axis is
    # whole, so this is the real product at every 4th output feature
    s = srv.specs(["mlp_h:w", "mlp_c_proj:ch"], stride=4, **q)
    check("mlp down  gelu(h)|1 @ c_proj;b",
          srv.matrix(s["mlp_h:w"]) @ srv.matrix(s["mlp_c_proj:ch"]),
          g["mlp_h"] @ g["mlp_proj_w"][:, ::4] + g["mlp_proj_b"][::4])

    # -- the attention head, conventional form (gpt2/, attngpt2/) -----------
    print("\nattention head, conventional form:")
    s = srv.specs(["ln_1:w", "ln_1:th", "wq:h", "wk_t:w", "wv:h", "wo"], stride=1, **q)
    inp, inp_t = srv.matrix(s["ln_1:w"]), srv.matrix(s["ln_1:th"])
    Q = inp @ srv.matrix(s["wq:h"])
    K_t = srv.matrix(s["wk_t:w"]) @ inp_t
    V = inp @ srv.matrix(s["wv:h"])
    check("Q  input|1 @ wQ;bq", Q, g["qkv"][:, :n_embd][:, hsl])
    check("K_t  wK_t|bk @ input_t;1", K_t, g["qkv"][:, n_embd:2 * n_embd][:, hsl].T)
    check("V  input|1 @ wV;bv", V, g["qkv"][:, 2 * n_embd:][:, hsl])
    check("out  (attn @ V) @ wO   [omits attn.c_proj.bias by design]",
          (g["probs"][H] @ V) @ srv.matrix(s["wo"]),
          g["heads"][H] @ g["attn_proj_w"][hsl])

    # -- the attention head, premultiplied QK/OV form (attnqkov/) -----------
    print("\nattention head, premultiplied QK/OV form:")
    # the premultiplied circuit absorbs bq and bk exactly: [x|1] on both sides
    # of [Wq;bq] @ [Wk;bk]^T is (x Wq + bq)(x Wk + bk)^T, with no appeal to the
    # softmax — the ones column is checkpoint-adjacent data in a leaf, not a
    # row sum computed by mm
    QK = srv.matrix(s["wq:h"]) @ srv.matrix(s["wk_t:w"])
    check("attn scores  input|1 @ QK @ input_t;1", (inp @ QK) @ inp_t, Q @ K_t)
    OV = srv.matrix(s["wv:h"]) @ srv.matrix(s["wo"])
    check("out  attn @ input|1 @ OV   [attn row sums carry bv]",
          (g["probs"][H] @ inp) @ OV, g["heads"][H] @ g["attn_proj_w"][hsl])

    # -- the one product that is still not the model's ----------------------
    # -- what the augmentation is worth --------------------------------------
    # Not a check: the size of the error that used to be drawn, measured rather
    # than quoted, so the READMEs can cite a command for the figure.
    print(f"\nhow far the bias-free product was from the model's, layer {L}:")
    for name, prod, want in [
        ("qkv projection", g["ln_1"] @ g["c_attn_w"], g["qkv"]),
        ("attention output", g["attn_out"] @ g["attn_proj_w"],
         g["attn_out"] @ g["attn_proj_w"] + g["attn_proj_b"]),
        ("mlp up, before GELU", g["ln_2"] @ g["c_fc_w"], g["mlp_pre"]),
        ("mlp down", g["mlp_h"] @ g["mlp_proj_w"],
         g["mlp_h"] @ g["mlp_proj_w"] + g["mlp_proj_b"]),
    ]:
        rel = np.abs(prod - want).max() / np.abs(want).max()
        print(f"  {name:24} {rel * 100:5.1f}% of the product's own range"
              f"  (up to {np.abs(prod - want).max():.2f} absolute)")

    print("\nremaining, stated rather than fixed:")
    dev = np.abs(ref.gelu_mm(g["mlp_pre"]) - g["mlp_h"]).max() / np.abs(g["mlp_h"]).max()
    print(f"  note  the 'mlp up' epilog is mm's erf GELU; GPT-2 trained the tanh"
          f" form:\n        max rel dev {dev:.2e} on the drawn h. The bias is in;"
          f" this is not.")
    print("  note  attn.c_proj.bias is excluded from every per-head view: GPT-2 adds"
          "\n        it once to the sum over all heads, so it is not a term of the"
          "\n        matmul those views draw. The 'attention output' view has it.")

    print()
    if FAILURES:
        print(f"{len(FAILURES)} FAILED: " + ", ".join(FAILURES))
        return 1
    print("all products match the reference forward pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
