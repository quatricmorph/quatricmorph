# distilgpt2 in the mm viewer

Drives the `mm` matrix-multiplication viewer from a real GPT-2 checkpoint —
`mm/models/distilgpt2` (6 layers, 12 heads, `n_embd` 768, 352,824,413 bytes).
Every matrix you see is read out of `model.safetensors`; every `input` matrix is
the model's own residual stream for the prompt you type.

## Run

```bash
cd mm
../.venv/bin/python tools/gpt2_server.py     # numpy present -> all 6 layers
# or:  python3 tools/gpt2_server.py          # stdlib only  -> layer 0 only
```

Then open <http://127.0.0.1:8000/examples/gpt2/>.

The server serves `mm/` as well as the data, so the CSV URLs the viewer loads
are same-origin. Nothing here touches the network, and the checkpoint is opened
read-only.

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

Layer and head selectors reach all 6 layers and all 12 heads. The `tensors` link
lists all 82 tensors with shapes and byte sizes. Every control is reflected in
the page URL, so a particular view is a shareable link:

```
?view=mlp+down&layer=4&stride=4&prompt=The+capital+of+France+is+Paris
```

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

## The drawn products omit biases

The same missing `+` has a second consequence, and it is not cosmetic. GPT-2
computes `x @ W + b`; `mm` can only draw `x @ W`. So while every *input* matrix
is exact, the *product* rendered in four of the six views is the bias-free one:

| View | Omitted term | Effect on layer 2 |
| --- | --- | --- |
| attention head | `c_attn.bias` on Q/K/V, `attn.c_proj.bias` on out | inherited from `examples/attngpt2`, which has the same gap |
| qkv projection | `c_attn.bias` | rendered product is **24 %** off the model's |
| attention output | `attn.c_proj.bias` | bias up to 0.63 absolute |
| mlp up (gelu) | `mlp.c_fc.bias` | **14 %**; and since GELU is nonlinear, the `h` drawn here is *not* the `h` that `mlp down` consumes |
| mlp down | `mlp.c_proj.bias` | |
| logits (tied wte) | none — GPT-2's LM head is the tied embedding with no bias | product is the model's |

The status bar states this per view, and separates the two claims: `data:` is
about the leaf matrices, `product:` is about what the multiplication shows. The
`input` activations are unaffected — the forward pass includes every bias, so a
layer-4 `ln_2` really is the model's layer-4 `ln_2`.

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
| `/gpt2/meta.json` | config, derived dims, tensor inventory, numpy availability |
| `/gpt2/tokens.json?text=…` | GPT-2 BPE token ids and pieces |
| `/gpt2/specs.json?kinds=…&layer=…&head=…&stride=…&text=…` | `{h, w, fidelity, url}` per matrix |
| `/gpt2/matrix.csv?kind=…&layer=…&head=…&rs=…&cs=…&t=…` | the matrix as CSV |

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
`mm/LICENSE`. These two files (`tools/gpt2_server.py`, `examples/gpt2/`) are
additions that drive the unmodified viewer through its existing `?params=` and
`postMessage({setParams})` interfaces; `index.html`, `viz.js`, `util.js` and
`gui.js` are untouched by this example.
