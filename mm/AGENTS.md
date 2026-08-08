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

## What the tests actually pin

`test/` has one suite per `src/` module. They were written against the
JavaScript that preceded the TypeScript port and passed unchanged after it,
which is the only reason the port can be called behaviour-preserving.

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
by the build and by `defaultParams` (extracted into `src/params.ts` precisely so
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
`src/` import each other with `.js` (`import * as viz from './viz.js'`) — the
extension the emitted code would use, which the bundler maps to the `.ts` file.
The inline `<script type="module">` in each page HTML imports `.ts` directly
(`'../src/gpt2page.ts'`), because it is not itself TypeScript.

## Routing

Pages are top-level routes; the model server owns `/api/`. Keep those namespaces
apart — they used to collide, and the symptom was the `/gpt2/` page being
answered by the data router with `{"error": "unknown route ''"}`.

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
* `q-`style honesty applies to the status bar, and it now makes three claims,
  not two: what the *data* is (exact vs sampled), which part of it is
  *synthetic* (the all-ones vector — the only number in an augmented leaf the
  checkpoint did not supply), and what the *product* is (`bias` drawn, `gap`
  remaining). Do not let a sampled figure be presented as exact, and do not let
  "complete" print beside a known gap — `productClaim` enforces the second.

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
wire format need a browser load of `/gpt2/` to verify.
