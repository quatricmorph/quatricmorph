### Attention head explorer — conventional form

One attention head of `mm/models/distilgpt2`, drawn in the usual factoring:

```
input = ln_1(x)              [n_tokens, n_embd]   the model's own residual stream
wQ, wV                       [n_embd, head_dim]   columns of c_attn.weight
wK_t                         [head_dim, n_embd]
wO                           [head_dim, n_embd]   rows of attn.c_proj.weight

Q      = input @ wQ
K_t    = wK_t @ input_t
attn   = softmax(tril(Q @ K_t / sqrt(head_dim)))
V      = input @ wV
out    = (attn @ V) @ wO
```

The premultiplied QK/OV factoring of the same head is [`../attnqkov`](../attnqkov/).
Both are the same function; they differ in which intermediate you get to look at.

## Run

Data comes from `tools/gpt2_server.py`, which reads the checkpoint by byte
range. The page itself is a Vite entry, so it needs Vite (or a built tree):

```bash
cd mm
../.venv/bin/python tools/gpt2_server.py &   # numpy present -> all 6 layers
npm run dev                                  # proxies /api to the above
```

Then open the URL Vite prints, at `/attngpt2/`. Without node:

```bash
npm run build
../.venv/bin/python tools/gpt2_server.py --root dist
```

## What changed from the original

This example previously loaded fixed 256×768 CSVs of Karpathy
[NanoGPT](https://github.com/karpathy/nanoGPT) `gpt2` weights from a public
bucket, with 12 layers, 12 heads and 10 canned sample inputs.

It now reads a local checkpoint instead, which changes four things:

* **6 layers, not 12** — distilgpt2 is the distilled model. `n_head` is 12 in
  both, and `head_dim` is 64 in both, but the layer count comes from
  `/api/meta.json` rather than a literal.
* **The prompt is yours.** `input` is the real residual stream after `ln_1` for
  the text you type, computed by a real forward pass, not one of ten samples.
* **Shapes come from `/api/specs.json`.** Nothing here writes an `h` or a `w`.
  mm's `tryURLInit` wraps out-of-range indices (`data[i % data.length]`), so a
  hand-written shape that disagrees with the data is silently *tiled* into a
  plausible, wrong picture. The server refuses to emit a matrix that disagrees
  with the spec it published.
* **Nothing touches the network.**

## The biases, and the one that is left out

GPT-2 computes `x @ W + b` and mm's `EPILOGS` has no `+`, so `c_attn.bias` is
drawn the only way a matmul-only grammar can draw a bias — by augmenting the
operands, `[input | 1] @ [wQ ; bq]`. That is `input @ wQ + bq` exactly, with the
bias as one more index along the contraction axis, so `Q`, `K_t` and `V` are the
model's own. The extra index lives *inside* those three; `attn` still contracts
over `head_dim`, so the scale below is unaffected.

`attn.c_proj.bias` is **not** drawn, and that is the honest choice rather than a
missing feature: GPT-2 adds it once to the sum over all twelve heads, so it is
not a term of this matmul at all. Putting it on one head's `wO` slice would
produce a matrix that is neither the head's contribution nor the layer's output.
`gpt2_server.py` refuses `wo:h` for that reason; [`../gpt2`](../gpt2/)'s
*attention output* view concatenates every head and draws it there, where it
does belong.

Every *input* matrix is the checkpoint's own, apart from the appended all-ones
column that carries the bias. The status bar states all of this separately,
`data:` and `product:`.

`softmax(tril(x/sqrt(k)))` is the correct scale in this factoring: `attn` is
`Q @ K_t`, which contracts over `head_dim`, and mm binds `k` in an epilog to the
contracted dimension of the matmul it sits on. This is *not* true of the
premultiplied form — see [`../attnqkov`](../attnqkov/).

## Controls

`Layer`, `Head`, `Tokens` and `Prompt` are all reflected in the page URL, so a
particular head is a shareable link:

```
?layer=3&head=5&seq=32&anim=Q+%40+K+%40+V+%40+wO
```

`Animate` walks the product outward from the innermost matmul: `attn @ V @ wO`
animates the last two, `Q @ K @ V @ wO` adds the attention scores,
`input @ wQ @ K @ V @ wO` adds the projections.

The one panel is the viewer's own outliner, top left: the scene as a tree of
matrices and matmuls, where a row selects, the eye hides and double-click
frames. It lists what is drawn, not the checkpoint — the tensors this page's
single view does not reach are not in it. See [`../gpt2`](../gpt2/) for what
each kind of tensor in distilgpt2 is and why most of them are undrawn here.

## Provenance

`mm/` is Meta's matrix-multiplication visualizer, MIT licensed — see
`mm/LICENSE`. This page drives the unmodified viewer through its existing
`?params=` and `postMessage({setParams})` interfaces.
