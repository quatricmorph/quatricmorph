# distilgpt2 in the mm viewer

**The page this describes is now `/`, mm's home page.** It used to be `/gpt2/`;
this directory keeps the notes and no longer holds the page. Its source is
`../index.html`, and the viewer it iframes moved down to `../viewer/index.html`.

Drives the `mm` matrix-multiplication viewer from a real GPT-2 checkpoint —
`mm/models/distilgpt2` (6 layers, 12 heads, `n_embd` 768, 352,824,413 bytes).
Every matrix you see is read out of `model.safetensors`; every `input` matrix is
the model's own residual stream for the prompt you type.

## Run

```bash
cd mm
../.venv/bin/python tools/gpt2_server.py &   # numpy present -> all 6 layers
# or:  python3 tools/gpt2_server.py          # stdlib only  -> layer 0 only
npm run dev                                  # proxies /api to the above
```

Then open the URL Vite prints — this page is the root of it.

This page is a Vite entry — it imports `src/gpt2page.ts` and that module's
stylesheet — so the static half has to come from Vite or from a built tree. To
run without node, build once and point the python server at the result:

```bash
npm run build
../.venv/bin/python tools/gpt2_server.py --root dist
# then http://127.0.0.1:8000/
```

Either way the CSV URLs the viewer loads are same-origin: under `npm run dev`
`vite.config.js` proxies `/api` to the python server, and under `--root dist`
one process serves both halves. Nothing here touches the network, and the
checkpoint is opened read-only.

With the server up, this checks that what the page draws is what GPT-2 computes:

```bash
../.venv/bin/python tools/check_bias.py     # --port to match the server
```

`npm run check` cannot: `models/` is local-only, so the test suite runs on
synthetic specs and never sees the checkpoint. Anything that changes a served
matrix, a `kinds` entry or an augmentation flag needs this too.

Two sibling pages share the same driver and the same server:
[`../attngpt2`](../attngpt2/) (one attention head, conventional factoring, with
progressive animation) and [`../attnqkov`](../attnqkov/) (the same head with the
QK and OV circuits premultiplied).

If `numpy` is missing, install it into the repo venv:

```bash
python3 -m venv .venv && .venv/bin/pip install numpy
```

## What you can see

| View | Expression | Weights it shows |
| --- | --- | --- |
| attention head | `out = ((attn = (Q = input @ wQ) @ (K_t = wK_t @ input_t)) @ (V = input @ wV)) @ wO` | one head's slice of `c_attn` and `attn.c_proj` |
| qkv projection | `qkv = ln_1(x) @ c_attn` | the whole fused `[Q \| K \| V]` projection, all 12 heads |
| attention output | `attn_y = heads @ c_proj` | `attn.c_proj` in full |
| mlp up (gelu) | `h = ln_2(x) @ c_fc` | `mlp.c_fc`, 768 → 3072 |
| mlp down | `mlp_y = gelu(h) @ c_proj` | `mlp.c_proj`, 3072 → 768 |
| logits (tied wte) | `logits = ln_f(x) @ wte_t` | the tied embedding, 50257 rows |

Layer and head selectors reach all 6 layers and all 12 heads. Every control is
reflected in the page URL, so a particular view is a shareable link:

```
?view=mlp+down&layer=4&stride=4&prompt=The+capital+of+France+is+Paris
```

## The scene tree

There is one panel over the scene and it lives inside the viewer: the outliner
(`src/outliner.ts`), top left. It lists what is actually drawn — every matrix
and matmul in the current view, by scene path — and it is a control surface, not
a legend: a row selects, shift-click adds, the eye hides, double-click frames.
Selection is shared with the viewport, so clicking a matrix in 3D reveals its
row and vice versa.

What it does not list is the rest of the checkpoint. The panel that used to sit
to the left of the viewer showed all 82 tensors including the ones no view
draws, and said why each was undrawn. That panel is gone, and with it the
click-a-tensor-to-open-the-view-that-draws-it navigation. What each kind of
tensor is, and why 33 of distilgpt2's 82 are drawn by no view on this page, is
recorded below rather than in a panel:

| | |
| --- | --- |
| **drawn** | 49 of 82. `c_attn.weight` is reachable three ways — `attention head`, `qkv projection`, and inside the staged model — and they are three different pictures of it. |
| **a bias** | drawn as the augmenting row or column of its weight, in the views that *ask* for it. `attn.c_proj.bias` is a term of `attention output` and not of `attention head` — that is the `NO_BIAS` refusal below, not an oversight. |
| **a LayerNorm gain or shift** | not drawn: the forward pass folds it into the activation, so it is already inside every matrix built from that activation. 26 tensors. |
| **a registered buffer** | `h.N.attn.bias` is the causal mask — not a learned parameter, and not read: `_forward` builds its own with `np.tril`. Six copies of a 1024×1024 float32 buffer are 25,165,824 of the checkpoint's bytes that nothing here touches. |
| **`wpe`** | the positional embedding, added to the residual stream before the first block rather than multiplied by anything. |

`/api/meta.json` still serves the full inventory — `Model.tensor_roles` builds
it by inverting `WEIGHT_KINDS`, `BIAS_KINDS` and `NO_BIAS`, the same tables the
reads go through — so the facts above are checkable against the server without
a panel to draw them.

## Why there is no single "whole model" scene

`mm`'s expression grammar is matmul-only — `viz.js` `parseExpr` maps `@` and
nothing else, and `EPILOGS` has no addition. GPT-2's two residual connections
(`x + attn(x)`, `x + mlp(x)`) therefore cannot be drawn as edges in the graph,
so the six blocks cannot be fused into one scene without drawing something that
is not GPT-2.

The residual stream lives in the **data** instead: `tools/gpt2_server.py` runs
the real forward pass, and each `input` leaf is the actual activation at that
point in the network. Every layer and every head is reachable; what you give up
is seeing all six blocks at once.

## The biases are drawn, by augmenting

GPT-2 computes `x @ W + b`; mm draws matmuls and only matmuls. Leaving the `+`
out was not cosmetic. Measured on layer 2 of distilgpt2 with this page's default
prompt, the bias-free products sat this far from the model's own, as a fraction
of each product's own range:

| qkv projection | attention output | mlp up (pre-GELU) | mlp down |
| --- | --- | --- | --- |
| 19.2 % | 9.3 % | 12.0 % | 1.9 % |

Those are outputs, not claims — `tools/check_bias.py --layer 2` prints them, and
they move with the prompt and the layer. Rather than draw that, the operands are
augmented:

```
x @ W + b  ==  [x | 1] @ [W ; b]
```

The bias becomes one more index along the contraction axis. `tools/gpt2_server.py`
appends it: a column of the constant 1 on the activation, a row of the
checkpoint's own bias vector on the weight, sliced to the same head and strided
with the same output axis. Nothing in `viz.ts` changed, and nothing needed to —
because the bias is an ordinary `k` index, every animation algorithm, block size
and partial sum stays correct by construction.

| View | Bias drawn | Product |
| --- | --- | --- |
| attention head | `c_attn.bias` on Q, K and V | the model's, less `attn.c_proj.bias` — see below |
| qkv projection | `c_attn.bias` | the model's |
| attention output | `attn.c_proj.bias` | the model's |
| mlp up (gelu) | `mlp.c_fc.bias` | the model's affine map; mm's GELU is the erf form where GPT-2 trained the tanh form, ~1e-5 relative |
| mlp down | `mlp.c_proj.bias` | the model's |
| logits (tied wte) | none — GPT-2's LM head is the tied embedding, which has no bias | the model's |

Two things this does **not** do, both stated in the status bar rather than
quietly absorbed:

* **`attn.c_proj.bias` is not drawn on a single head.** GPT-2 adds it once to
  the sum over all twelve head outputs, so it is not a term of the matmul a
  per-head view draws; adding it there would give a matrix that is neither the
  head's contribution nor the layer's output. The server refuses `wo:h` outright
  for that reason. The *attention output* view concatenates every head, and
  there the bias does belong to the matmul drawn — so that is where it appears.
* **The ones column is synthetic.** It is the only number in an augmented leaf
  the checkpoint did not supply, and the `data:` line says so; "exact" is never
  left standing over it unqualified.

Because `mlp up`'s bias is now inside the matmul and in front of the GELU, the
`h` drawn there **is** the `h` that `mlp down` consumes — which was not true
while the bias was missing, GELU being nonlinear.

`tools/check_bias.py` checks all of this end to end against an independent
numpy + `safetensors` forward pass, over the same CSVs the page fetches.

## Fidelity

Results are labelled in the status bar, and the labels mean what they say. This
section is about the leaf **data**; what the rendered *product* leaves out is the
section above.

* **exact** — every element rendered is the checkpoint's own value, or an
  activation computed from it in float32. This is the default everywhere.
* **sampled** — every n-th row or column, chosen by the `Stride` control, with
  no interpolation. Needed because `c_fc` is 768×3072 and the tied embedding is
  50257×768, which exceed a comfortable browser point budget.

Stride is only ever applied to an axis that the matmul does **not** contract
over. A sampled view therefore shows fewer columns of a genuine product, never a
partial sum presented as the whole one. `specs.json` is the single source of
truth for shapes and `matrix.csv` refuses to emit anything that disagrees with
it — `mm`'s `tryURLInit` wraps out-of-range indices (`data[i % data.length]`), so
a mis-shaped CSV would otherwise tile silently rather than fail.

Without `numpy`, only layer 0 activations exist (an embedding gather plus a
LayerNorm, both exact in pure stdlib). Deeper layers return HTTP 501 with the
reason, never a placeholder.

## HTTP API

| Route | Returns |
| --- | --- |
| `/api/meta.json` | config, derived dims, tensor inventory, numpy availability |
| `/api/tokens.json?text=…` | GPT-2 BPE token ids and pieces |
| `/api/specs.json?kinds=…&layer=…&head=…&stride=…&text=…` | `{h, w, fidelity, url}` per matrix |
| `/api/matrix.csv?kind=…&layer=…&head=…&rs=…&cs=…&t=…` | the matrix as CSV |

`kind` is one of the weight kinds (`wq`, `wk`, `wk_t`, `wv`, `wo`, `c_attn`,
`attn_c_proj`, `c_fc`, `mlp_c_proj`, `wte`, `wte_t`, `wpe`) or the activation
kinds (`resid`, `ln_1`, `ln_2`, `attn_out`, `mlp_h`, `final`). In `specs.json`,
`kind:flags` requests `t` (transpose), `r`/`c` (apply stride to rows/columns).

`rs`/`cs` address the **stored** tensor's axes, before any built-in transpose —
so `wte_t` decimates vocabulary with `rs`, keeping the contracted 768 axis whole.

## Memory

The checkpoint is never loaded whole. A per-head projection is a column slice of
`c_attn.weight` — 768 reads of 256 bytes. Token embeddings are gathered a row at
a time. Only the forward pass holds full tensors, and only one layer's worth at
a time (~28 MB), plus the per-prompt activations it caches.

## Provenance

`mm/` is Meta's matrix-multiplication visualizer, MIT licensed — see
`mm/LICENSE`. `tools/gpt2_server.py`, `src/gpt2page.{ts,css}` and the three
The checkpoint pages are additions that drive the viewer through its existing
`?params=` and `postMessage({setParams})` interfaces; `index.html`, `viz.js`,
`util.js` and `gui.js` are untouched by this example.
