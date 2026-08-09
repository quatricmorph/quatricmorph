### Attention head explorer — premultiplied QK/OV

The same attention head as [`../attngpt2`](../attngpt2/), reassociated so the
head's two `n_embd × n_embd` circuits are on screen instead of its per-token
projections:

```
QK   = wQ @ wK_t              [n_embd, n_embd]
OV   = wV @ wO                [n_embd, n_embd]

out  = softmax(tril(input @ QK @ input_t / sqrt(head_dim))) @ input @ OV
```

rather than the conventional:

```
out  = softmax(tril((input @ wQ) @ (wK_t @ input_t) / sqrt(head_dim))) @ (input @ wV) @ wO
```

Matrix multiplication is associative, so these are the same function of the same
weights. What differs is the intermediate you can look at: here `QK` and `OV`
are the head's whole read and write circuits, drawn in full.

## Run

Data comes from `tools/gpt2_server.py`, which reads `mm/models/distilgpt2` by
byte range. The page is a Vite entry, so it needs Vite (or a built tree):

```bash
cd mm
../.venv/bin/python tools/gpt2_server.py &   # numpy present -> all 6 layers
npm run dev                                  # proxies /api to the above
```

Then open the URL Vite prints, at `/attnqkov/`. Without node:

```bash
npm run build
../.venv/bin/python tools/gpt2_server.py --root dist
```

## What changed from the original

This example previously loaded fixed 256×768 CSVs of Karpathy
[NanoGPT](https://github.com/karpathy/nanoGPT) `gpt2` weights from a public
bucket. It now reads a local checkpoint: 6 layers rather than 12, `input` is the
real residual stream after `ln_1` for the prompt you type, and every leaf's
shape comes from `/api/specs.json` rather than a literal — mm's `tryURLInit`
wraps out-of-range indices (`data[i % data.length]`), so a hand-written shape
that disagrees with the data tiles silently instead of failing.

## The attention scale is a literal 8 here, and that is deliberate

mm binds `k` in an epilog to the contracted dimension of the matmul the epilog
sits on (`viz.js` `applyPointwiseEpilog` returns `x / Math.sqrt(this.D)`).

In the conventional factoring `attn` is `Q @ K_t`, which contracts over
`head_dim`, so `softmax(tril(x/sqrt(k)))` *is* the attention scale. Premultiplied,
`attn` is `inputQK @ input_t`, which contracts over **`n_embd`** — `sqrt(k)`
would be `sqrt(768) ≈ 27.7` where the model divides by `sqrt(64) = 8`. Using it
would draw a real attention pattern at the wrong temperature, with nothing on
screen to say so.

mm's `EPILOGS` is a fixed list, and the only other scaled softmax in it is the
literal `softmax(tril(x/8))`. That is exactly right for `head_dim` 64 and wrong
for anything else, so the page checks `head_dim` against 64 on load and
**refuses with the reason** if a checkpoint ever disagrees, rather than drawing
it. distilgpt2 is `n_embd` 768 / `n_head` 12 = 64, so it draws.

## The biases, and what carries them

The QK/OV factoring is exact — it is a reassociation of the same matmuls — and
it reassociates the biased ones. mm has no `+`, so the operands are augmented
instead, and premultiplication absorbs `c_attn.bias` without a special case:

```
QK  =  [wQ ; bq] @ [wK_t | bk]              769×769
       [input | 1] @ QK @ [input_t ; 1]  ==  (x wQ + bq)(x wK + bk)ᵀ
```

which is the same score matrix the conventional form draws. `bv` rides the same
ones column: `attn @ [input | 1] @ [[wV ; bv] @ wO]` is `attn @ V @ wO`.

Two things the picture does not give you, both in the status bar:

* **`attn.c_proj.bias` is not drawn.** GPT-2 adds it once to the sum over all
  heads, so it is not a term of a per-head matmul — the same call as in
  [`../attngpt2`](../attngpt2/), for the same reason.
* **`bv` arrives through `attn`'s row sums**, which softmax makes 1. That is
  exact in arithmetic, but `viz.ts` deliberately yields a row of zeros rather
  than NaN when a softmax row underflows, and such a row would carry 0 there and
  drop `bv` instead of adding it. The conventional factoring in
  [`../attngpt2`](../attngpt2/) does not depend on this: it forms `V` as a leaf
  matmul before `attn` ever touches it.

Every input matrix is the checkpoint's own, apart from the appended all-ones
column that carries the bias. The status bar separates the claims.

## Cost

`QK` is 769×769 and `OV` is 769×768 once augmented, which is ~1.18 M of the
~1.4 M points on screen no matter what `Tokens` is set to — the token count only
affects `input`, `attn` and the output. (The bias costs one index on a 768 axis:
0.3 % more points, for the difference between the model's product and a
bias-free stand-in.) Both long axes are contracted by the matmuls they feed, so
`Stride` cannot decimate them without turning the rendered product into a
partial sum; the control is disabled here for that reason. Expect this page to
be the slowest of the three, and pick a small `Tokens` value.

## Controls

`Layer`, `Head`, `Tokens` and `Prompt` are reflected in the page URL:

```
?layer=2&head=7&seq=16&anim=%2B+input+%40+QK
```

`Animate` walks outward from the innermost matmul: `attn @ input @ OV` animates
the two outer products, `+ inputQK @ input_t` adds the attention scores,
`+ input @ QK` adds the read circuit.

The one panel is the viewer's own outliner, top left: the scene as a tree of
matrices and matmuls, where a row selects, the eye hides and double-click
frames. It lists what is drawn, not the checkpoint — the tensors this page's
single view does not reach are not in it. See [`../gpt2`](../gpt2/) for what
each kind of tensor in distilgpt2 is and why most of them are undrawn here.

## Provenance

`mm/` is Meta's matrix-multiplication visualizer, MIT licensed — see
`mm/LICENSE`. This page drives the unmodified viewer through its existing
`?params=` and `postMessage({setParams})` interfaces.
