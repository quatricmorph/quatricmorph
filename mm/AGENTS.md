# Working in `mm/`

Conventions for anyone — human or agent — changing this codebase.

## The gate

```bash
npm run check        # typecheck, then tests, then build
```

All three must pass before a change is done. They catch different things and
none of them subsumes another:

* `typecheck` catches renames, arity drift, and typos in property names.
* `test` catches behaviour changes in the numerics and the params plumbing.
* `build` catches broken module resolution and unregistered pages — Rollup will
  not discover an HTML page just because another page iframes it.

**None of the three renders a frame.** The shaders, the WebGPU renderer, the
animation loop and the camera are not covered by anything automated. A change
that touches them has to be looked at in a browser: `npm run dev`, open `/`, and
for the checkpoint pages start `tools/gpt2_server.py` first.

Nor does any of the three see the checkpoint — `models/` is local-only, so the
suite runs entirely on synthetic specs. A change to the served matrices or to
what the pages ask for needs the numeric check, which does see it:

```bash
../.venv/bin/python tools/gpt2_server.py --port 8123 &
../.venv/bin/python tools/check_bias.py --port 8123
```

It fetches the CSVs the pages fetch, reassembles the products mm will draw, and
compares them against its own numpy + `safetensors` forward pass — a reference
that owes nothing to `gpt2_server.py`'s arithmetic. Tolerance is set by the wire
(`to_csv` writes `%.6g`), not by the maths.

## Module layout

`src/` is five library modules plus the two entry points. The entries stay at
the root because that is what they are — the four HTML pages name them by path
(`/src/main.ts` from `viewer/index.html`, `./src/gpt2page.ts` from the three
checkpoint pages), and nothing but `npm run build` checks those strings.

```text
src/
  main.ts, gpt2page.ts, gpt2page.css   entries — not library, referenced by HTML
  common/    util.ts                            params codec, object-tree helpers, THREE guides + text
  render/    points.ts colormap.ts              a matrix becomes pixels: instanced-quad elements,
             heatmap.ts heatmapmesh.ts          the colour ramp, the heatmap arithmetic and its mesh
  scene/     viz.ts params.ts                   the scene built from a params tree, and its defaults
  editor/    address.ts scenetree.ts            the tensor editor: addressing, selection, picking,
             selection.ts picking.ts            edit stack, highlights, camera, controller, and its
             editops.ts highlight.ts            own two DOM panels
             cameractl.ts interaction.ts
             inspector.ts outliner.ts
  gui/       gui.ts                             the lil-gui params panel (unrelated to editor panels)
```

The module graph is a DAG and must stay one:

```text
common ← scene ← editor          render ← scene, editor, gpt2page
       ← gui   ← main            common ← main, gui, scene
```

`inspector.ts` and `outliner.ts` live in `editor/` rather than beside `gui.ts`
for exactly this reason: they import `editops`/`selection` while `interaction`
imports them back. As files that is acyclic; split across two modules it would
be a module cycle. Keep the editor's own panels inside the editor.

**There are no `index.ts` barrels, on purpose.** No module here is uniformly
pure: `render/` holds THREE-free arithmetic (`colormap`, `heatmap`) beside TSL
shader graphs (`points`, `heatmapmesh`), and `editor/` holds pure index algebra
(`address`, `scenetree`, `selection`) beside modules that build materials at
import. A barrel re-exporting both halves would pull THREE into every consumer
of the pure half — silently growing the page bundle and defeating the very
property `test/imports.smoke.test.ts` was written to pin. Import deep paths.
If a module ever becomes uniformly pure or uniformly THREE, a barrel is fine.

`common/util.ts` is the one file that straddles a boundary: it holds the pure
params codec (`flatten`/`compress`/`makeSearchParams`) *and* the THREE guides,
axes and text geometry. It moved whole rather than being split, because it
builds a `NodeMaterial` and parses a typeface at module scope — splitting it
changes what gets imported when, which is a behaviour change, not a move. Do
that split as its own change with `imports.smoke.test.ts` watched.

## What the tests actually pin

`test/` mirrors `src/` **folder for folder**, and that is the granularity —
not file for file. `test/editor/` is genuinely one suite per file (ten and
ten), and so is `test/common/` and `test/gui/`; the other two are not, and it
matters which:

* `test/render/points.test.ts` is the single suite for all four of
  `points`, `colormap`, `heatmap` and `heatmapmesh` — it imports all four,
  and the `points` bullet below describes the contract it pins.
* `test/scene/viz.test.ts` covers `viz.ts`. **`scene/params.ts` has no suite
  of its own**: it is reached only through `gui.test.ts` and
  `interaction.test.ts`, which build against the real `defaultParams`. Say
  that rather than implying the mirror means per-file coverage.

`test/modules.test.ts` enforces the mirror at that same folder granularity —
it asserts each suite imports the module it sits under, not that each file has
a suite.

Four files stay at `test/` root because they are cross-cutting rather than
per-module: `setup.ts`; `imports.smoke.test.ts`, which asserts every `src/`
module imports headless and is the precondition every other suite depends on;
`modules.test.ts`, which is about the layout rather than any one module; and
`gpt2page.test.ts`, matching its entry, which has no folder either.

`vite.config.ts` needs no change to find any of it: `test.include` is already
`test/**/*.test.{js,ts}`.

The suites were written against the JavaScript that preceded the TypeScript
port and passed unchanged after it, which is the only reason the port can be
called behaviour-preserving.

They are deliberately weighted toward properties over line coverage:

* `util` — round-trip identities. `unflatten(flatten(x)) === x`,
  `uncompress(compress(...))`, and a full params tree through
  `makeSearchParams` → `updateObjectFromSearchParams` with types intact. A
  break here does not throw, it produces a URL that loads a slightly different
  scene.
* `viz` — the numerics, against hand-computed values: softmax, causal (tril)
  softmax, layernorm, the initializers, `Array2D`. A wrong softmax still draws a
  smooth colour ramp, so only arithmetic catches it.
* `gpt2page` — the params-tree builders, `countPoints`, `bbox`, `merge`'s
  refusal to address a node the tree does not have, `checkShapes`' refusal of a
  tree mm would tile, and the two status-bar claims: that "exact" never stands
  alone over the synthetic ones vector, and that "complete" never prints beside
  a gap.
* `points` — the contract with `viz`/`main`: attribute names, and that
  `raycast` returns the *element* index so `index / W` and `index % W` recover
  row and column.
* `gui` — that the panel builds against the real default params and that its
  controls are bound to the param paths the rest of the app reads.

Two conventions worth keeping:

* **Expected values are hand-computed, never captured from the code under
  test.** A golden produced by the implementation only pins that it has not
  changed, not that it is right. Where a value is non-obvious, the derivation is
  in a comment above it.
* **Surprising behaviour is pinned deliberately, with a note saying why.**
  `layernorm` normalises over the whole matrix rather than per row; softmax
  yields zeros rather than NaN when a row underflows. Both look like bugs and
  are load-bearing. Do not "fix" one without deciding to change the pictures.

`main.ts` has no exports and builds a renderer at import, so it is covered only
by the build and by `defaultParams` (extracted into `src/scene/params.ts` precisely so
it could be reached). Say so rather than implying the suite covers it.

## TypeScript

The port is deliberately **not** `strict`. This is ~4,600 lines of previously
untyped code whose central data structure is a heterogeneous params tree
addressed by string paths; turning strict on would produce hundreds of errors at
once, and a permanently red typecheck is one everybody learns to ignore.

So `tsconfig.json` is green from day one with the checks that catch real
mistakes on, and the ones needing annotations everywhere off. To tighten it,
turn on **one** flag (`noImplicitAny`, then `strictNullChecks`, then `strict`),
fix what it finds, and leave it on.

Two settings there are load-bearing and commented in place:

* `paths` maps `three` → the webgpu typings, mirroring the alias in
  `vite.config.ts`. If the two disagree, the typecheck checks a program that
  never runs.
* `useDefineForClassFields: false`. With it on, a bare `foo: any` declaration
  emits a real field that shadows an inherited member with `undefined` — which
  broke `PointCloudGeometry` by shadowing `setIndex`.

`any` appears in three places on purpose, each commented: TSL node types (the
generics cannot express what the node graph carries at runtime), by-id DOM
lookups in `gpt2page.ts`, and the `Params` tree.

Import specifiers differ by where you are, and both are correct: modules under
`src/` import each other with `.js` (`import * as viz from '../scene/viz.js'`)
— the extension the emitted code would use, which the bundler maps to the `.ts`
file. They are plain relative paths, with no `@mm/*` alias, deliberately: an
alias would have to be declared twice, in `tsconfig.json`'s `paths` and in
`vite.config.ts`'s `resolve.alias`, and those two drifting apart is the failure
the `three` entry in both already exists to prevent.
The inline `<script type="module">` in each page HTML imports `.ts` directly
(`'./src/gpt2page.ts'` from the home page, `'../src/gpt2page.ts'` from the two
under a directory), because it is not itself TypeScript.

## Routing

Pages are top-level routes; the model server owns `/api/`. Keep those namespaces
apart — they used to collide, and the symptom was the GPT-2 page being answered
by the data router with `{"error": "unknown route ''"}`.

`/` is the GPT-2 explorer and `/viewer/` is the viewer it iframes. That is the
one asymmetry in `viewerUrl`: the two pages under a directory take the
`'../viewer/index.html'` default, and the home page passes `'viewer/index.html'`
because the viewer is one level *down* from it, not up.

Adding a page means adding it to `rollupOptions.input` in `vite.config.ts`. It
will work in `dev` without that and silently vanish from `build`.

`public/ref.html` and `public/intro/` are in `public/` on purpose: they embed
zero-md `<script type="text/markdown">` blocks whose bodies are raw markdown,
and Vite's HTML pipeline would rewrite script tags in an entry.

## Things that are true and easy to get wrong

* Every leaf's `h`/`w` comes from `/api/specs.json`, never a literal. mm wraps
  out-of-range indices (`data[i % data.length]`), so a wrong shape is *tiled*
  into a plausible, wrong picture with no error anywhere. `checkShapes` and the
  throw in `leaf()` catch the two ways that happens; neither is decoration.
* The **root** params node must not carry `matmul: true` — `ensureChildCounts`
  recognises the root by `matmul === undefined`.
* Biases are drawn by **augmenting**, not by an epilog: `X @ W + b` is served as
  `[X | 1] @ [W ; b]`, one more index along the contraction axis. That is why
  `viz.ts` needs no `+` and why the animation algorithms did not have to change
  — a bias added in `applyPointwiseEpilog` would be added once per accumulated
  chunk, since it is affine and the partial sums are not. If you are tempted to
  put one there, that is the reason not to.
* `q-`style honesty applies to the three claims `showStatus` builds, and there
  are three, not two: what the *data* is (exact vs sampled), which part of it is
  *synthetic* (the all-ones vector — the only number in an augmented leaf the
  checkpoint did not supply), and what the *product* is (`bias` drawn, `gap`
  remaining). Do not let a sampled figure be presented as exact, and do not let
  "complete" print beside a known gap — `productClaim` enforces the second.
  **These no longer have a status bar to print in**: that row was removed so the
  viewer gets the height, and `claims()` writes them to a hidden overlay while
  `fail()` is the only thing that shows it. What is still stated on screen is the
  render fidelity — the viewer's own `#colorbar` prints encoding, texels and LOD
  in-frame, and `#timeline` repeats it per stage. If you put the data/product
  claims back in front of the user, that is a UI decision, not a licence to
  weaken them.
* Navigate the embedded viewer with `contentWindow.location.replace`, never by
  assigning `$('mm').src`, and do not `pushState` from inside the frame
  (`saveUrl` returns early when framed). Both push joint-history entries whose
  top-level URL is the page's, so a back navigation — the trackpad swipe that is
  also this viewer's zoom gesture — reloads the frame at a *previous* view while
  the chrome goes on describing the current one, or, for a staged scene whose URL
  carries no params, drops it into `resetParams()` and mm's default demo scene.
  Verified by a browser A/B: `history.length` 2→3→4 across two view changes
  before, constant after.

## Known, deliberately not changed

`gui.ts` `syncChildParams` calls `layoutProto(pp)`, passing the parent params
object where the parameter is named `left_child`. It is always truthy, so
`childLayout` always sees a left child. It looks wrong, nothing covers it, and
it was left exactly as found rather than changed during a type migration. If you
touch it, do it as its own change with a picture to compare against.

There are two lockfiles (`bun.lock` and `package-lock.json`). Pick one — this
was left alone rather than decided unilaterally.

The cross-frame protocol (page → iframe `?params=` → `postMessage({setParams})`
→ `getUrlInfo`) has no automated coverage and cannot get any while `main.ts`
builds a renderer at import. Changes to `viewerUrl`, `RESPONDERS`, or the params
wire format need a browser load of `/` to verify.
