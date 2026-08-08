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
npm run dev                                  # proxies /gpt2 to the above
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

## What the picture leaves out

The QK/OV factoring is exact — it is a reassociation of the same matmuls — but
it is a reassociation of the *bias-free* ones. mm has no `+`, so `c_attn.bias`
on Q/K/V and `attn.c_proj.bias` on `out` are omitted from the drawn product, as
in the conventional view. Every input matrix is the checkpoint's own. The status
bar separates the two claims.

## Cost

`QK` and `OV` are 768×768 each, which is ~1.18 M of the ~1.4 M points on screen
no matter what `Tokens` is set to — the token count only affects `input`,
`attn` and the output. Both 768 axes are contracted by the matmuls they feed, so
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

## Provenance

`mm/` is Meta's matrix-multiplication visualizer, MIT licensed — see
`mm/LICENSE`. This page drives the unmodified viewer through its existing
`?params=` and `postMessage({setParams})` interfaces.
