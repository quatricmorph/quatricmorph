# mm

Matmul visualizations in 3D

[Tool](https://bhosmer.github.io/mm)

[Reference](https://bhosmer.github.io/mm/ref.html)

Notes
* [Inside the Matrix: Visualizing matrix multiplications, Transformer Attention and Beyond](https://bhosmer.github.io/mm/intro/) ([slides](https://docs.google.com/presentation/d/19eaLLrANCHbPyuC26-z3GzgjCkRcYB1sFndf0jZ-lTg))

## Pages

| Route | What it is |
| --- | --- |
| `/` | the viewer itself — drives off `?params=` and `postMessage` |
| `/gpt2/` | GPT-2 explorer: several views over a real checkpoint |
| `/attngpt2/` | one attention head, conventional Q/K/V/O factoring |
| `/attnqkov/` | the same head, premultiplied into QK and OV circuits |
| `/ref.html`, `/intro/` | static docs, served from `public/` |

The three checkpoint pages iframe `/` and drive it through its public surface
only (`?params=` and `postMessage({setParams})`). They read their matrices from
the model server under `/api/`.

Those pages used to live under `/examples/`. They are the product surface rather
than samples of it, so they are top-level routes now — the old
`/examples/<name>/` URLs no longer resolve.

## Commands

```bash
npm install
npm run dev          # vite on :5173, proxying /api to the model server
npm run build        # multi-page build into dist/
npm run preview      # serve dist/, same /api proxy

npm test             # vitest, all suites
npm run test:watch
npm run typecheck    # tsc --noEmit
npm run check        # typecheck + test + build, in that order
```

The checkpoint pages need the model server as well:

```bash
python3 tools/gpt2_server.py                  # stdlib only: layer 0
../.venv/bin/python tools/gpt2_server.py      # with numpy: all layers
```

Then either `npm run dev` (which proxies `/api` to it), or `npm run build` once
and `python3 tools/gpt2_server.py --root dist`, which serves both halves from a
single origin and needs no proxy. Set `MM_MODEL_SERVER` if the server is not on
`:8000`.

`/` and the intro/reference pages need no server.

## Layout

```
index.html            the viewer page
gpt2/ attngpt2/ attnqkov/    checkpoint pages (one index.html each + README)
src/                  all application TypeScript
  main.ts             entry: renderer, camera, animation loop, messaging
  viz.ts              Mat / MatMul — geometry, layout, animation, numerics
  points.ts           instanced-quad element rendering (WebGPU) — 'spheres'
  heatmap.ts          the other render path, as pure arithmetic: mode, LOD, cells
  heatmapmesh.ts      the same path's THREE half: one textured quad per block
  colormap.ts         the heatmap ramp and the value → texel encoding
  gui.ts              lil-gui control panel
  util.ts             params (de)serialization, three.js helpers
  params.ts           the default params tree
  gpt2page.ts         shared driver for the three checkpoint pages
test/                 vitest suites (TypeScript), one per src module
tools/gpt2_server.py  reads models/distilgpt2, serves /api/
public/               copied byte-for-byte (ref.html, intro/)
assets/               fonts and the KaTeX typeface
```

See `AGENTS.md` for conventions, the state of the TypeScript migration, and what
the tests are actually pinning.
