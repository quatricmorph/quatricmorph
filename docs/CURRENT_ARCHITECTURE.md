# Current architecture of `mm` — evidence record

**Purpose.** Every extraction into `apps/web/quatricmorph-workspace` cites this
document as evidence. It records what `mm/` **actually contains**, read from the
files, with a reuse decision per symbol. Nothing here is inferred from a name or
assumed from convention; where a file the task list expected does not exist,
that absence is recorded as an absence.

**Scope of the reading.** `mm/index.html` (839 lines), `mm/viz.js` (2105),
`mm/gui.js` (380), `mm/util.js` (365), `mm/ref.html` (1527), `mm/lib/`,
`mm/assets/`, `mm/examples/`, `mm/LICENSE`, `mm/README.md`. Line references are
to the files as they stand at the commit this document was written against;
`mm/` is read-only per `AGENTS.md` and `docs/requirements/PREREQUISITES.md`, so
they remain valid.

---

## 0. Files that do not exist

Recording these explicitly, because assuming they exist would misdescribe how
the app is built:

| Expected | Status | What is there instead |
| --- | --- | --- |
| `mm/package.json` | **Absent** | No npm package, no build step, no dependency manifest. |
| `mm/node_modules/` | **Absent** | — |
| lockfile of any kind | **Absent** | — |
| bundler config | **Absent** | — |

`mm` is served as static files. Module resolution is done in the browser by an
**import map** declared inline in `mm/index.html:230-238`, mapping `three`,
`three/addons/`, and `lil-gui` to vendored copies under `mm/lib/`, with
`mm/lib/es-module-shims.js` loaded at `index.html:228` as a polyfill for
browsers without import-map support.

**Consequence for the port:** there was no dependency graph to inherit.
`apps/web/quatricmorph-workspace/package.json` declares `three` and `lil-gui` as real
dependencies for the first time; the vendored `mm/lib/*.js` copies were not
carried over.

---

## 1. `mm/index.html` — bootstrap, scene, and input

839 lines: ~160 of CSS, ~60 of DOM, and a single inline
`<script type="module">` (line 240 onward) that is the whole application entry
point.

| Symbol / block | Line(s) | Responsibility | Depends on | Problem | Decision |
| --- | --- | --- | --- | --- | --- |
| CSS block | 8-159 | Layout for `#info`, `.lil-gui` overrides, instructions modal, responsive breakpoint at 600px | — | Global, unscoped | **Extract** → `src/style.css` |
| DOM skeleton | 162-222 | `#info`, `#container`, `#instructions`, `#minimized`, `#maximize`, `#minimize`, laptop/mobile instruction tables | — | Element ids are an implicit contract with the script | **Extract** → `index.html` + `src/app/instructions.ts` |
| import map | 230-238 | Resolves `three`, `three/addons/`, `lil-gui` to `mm/lib/` | vendored libs | No versioning, no integrity | **Deprecate** → real package.json dependencies |
| `params` object literal | 255-305 | The entire application state: expr, name, epilog, left, right, anim, block, layout, deco, viz, diag | `viz.default*()` | One untyped bag; every subsystem reaches into it | **Extract and refactor** → `src/app/default-params.ts`, split by domain |
| `default_params` / `resetParams` | 306-316 | Deep copy for reset | `util.copyTree` | — | **Extract** → `src/app/default-params.ts` |
| `url_info`, `urlPrefix`, `saveUrlInfo`, `saveUrl` | 320-355 | Serialize state to the query string, push history | `util.makeSearchParams` | Mixes serialization with `history.pushState` | **Extract and refactor** → `src/app/url.ts` |
| `initObj` | 356-395 | Rebuild the `MatMul` object, preserving camera scale by comparing old/new bounding-box magnitude (lines 361-378) | `viz.MatMul`, `util.bbhwd` | Rebuilds the whole scene on any parameter change | **Extract and refactor** → `src/app/create-app.ts` |
| camera / `aspect` / `fov` | 393-397 | `PerspectiveCamera`, fov widened for portrait aspect | THREE | — | **Extract** → `src/app/scene.ts` |
| `pointer`, `raycaster` | 398-402 | `Raycaster` with `params.Points.threshold = 0`; `raycaster.far` toggled between `0` and `Infinity` to enable/disable picking (lines 455-462) | THREE | The far-plane toggle is a non-obvious idiom | **Extract and refactor** → `src/interaction/selection.ts`, with the idiom documented |
| `scene`, `renderer`, `render_info` | 404-412 | `WebGLRenderer({antialias:true})`, `renderer.info.memory` surfaced into the GUI | THREE | — | **Extract** → `src/app/scene.ts` |
| `getContext` | 415-422 | Bundles `{raycaster, camera, ...}` and passes it down into every `Mat` | — | Ambient dependency injection via an untyped bag | **Extract and refactor** → typed context in `src/app/scene.ts` |
| `OrbitControls` + `viewState` | 425-437 | Camera control; `viewState()` snapshots position for URL serialization | `three/addons` | — | **Extract** → `src/app/scene.ts` |
| `orbit` start/change/end listeners | 439-460, 503+ | Suppress label updates during drag; debounce camera save at 250 ms (lines 485-502) | — | Debounce constant is inline | **Extract and refactor** → `src/app/scene.ts`, constant named |
| `updateSpotlight` | 455-466 | Magnifier: re-raycasts with an offset pointer and `far = Infinity` | raycaster | — | **Extract** → `src/interaction/selection.ts` |
| `requestLabelUpdate` | 468-481 | Coalesces label refreshes into an animation frame | — | — | **Extract** → `src/interaction/selection.ts` |
| `initFromParams` / `initFromSearchParams` | 517-544 | Restore camera and state from the URL; `popstate` listener | `util.updateObjectFromSearchParams` | — | **Extract** → `src/app/url.ts` |
| `resize` / `pointermove` / `pointerdown` / `pointerup` listeners | 547-595 | Viewport and hover/selection input | — | Registered on `window`, never removed | **Extract and refactor** → `src/interaction/`, with teardown |
| `key_funcs` | 596+ | Keyboard shortcuts | — | — | **Extract** → `src/interaction/` |
| animation loop | end of file | `requestAnimationFrame` driving `anim` step and `renderer.render` | — | — | **Extract** → `src/interaction/animation.ts` |

---

## 2. `mm/viz.js` — data, geometry, and the multiplication object

2105 lines. Three classes and a large body of module-level state.

### 2.1 Module-level state and initializers

| Symbol | Line(s) | Responsibility | Problem | Decision |
| --- | --- | --- | --- | --- |
| `TEXTURE`, `MATERIAL` | 10-43 | One shared `ShaderMaterial` for every point cloud; vertex shader sizes points by `mag * pointSize / -mvPosition.z` | Module-level singleton mutated at runtime | **Extract** → `src/viz/material.ts` |
| `gaussianRandom` | 51-57 | Box-Muller normal variate | — | **Extract** → `src/viz/init.ts` |
| `sampleSphere` | 60-64 | References an undefined global `sm` | **Dead code** — would throw if called; not reachable because the `sphere` entry in `INIT_FUNCS` is commented out at line 74 | **Deprecate** |
| `INIT_FUNCS`, `INITS` | 66-81 | Named initializers: rows, cols, row/col major, pt linear, uniform, gaussian, tril/triu mask, eye, diff | — | **Extract** → `src/viz/init.ts` |
| `USE_RANGE`, `USE_DROPOUT`, `useRange`, `useDropout` | 83-87 | Which initializers accept min/max and dropout | — | **Extract** → `src/viz/init.ts` |
| `DATA_CACHE`, `tryLoadData` | 89-107 | Loads CSV from a URL using a **synchronous** `XMLHttpRequest` (`req.open(..., false)`, line 99) | Blocks the main thread; deprecated API | **Deprecate** — not carried forward |
| `tryURLInit` | 109-117 | Wraps loaded CSV as an initializer | depends on the sync XHR above | **Deprecate** |
| **`tryEvalInitExpr`** | **119-126** | Builds an initializer by calling **`eval?.()`** on a user-supplied string | **Arbitrary code execution from a URL parameter.** `params.expr` is restored from the query string (`index.html:531`), so a crafted link executes attacker-chosen JavaScript in the visitor's browser. | **Deprecate — hard.** See §5 below. |
| `getInitFunc` | 128-146 | Chooses an initializer, applies range scaling and dropout | calls `tryEvalInitExpr` | **Extract and refactor** → `src/viz/init.ts` with the `eval` branch removed |
| `erf`, `gelu`, `sigmoid`, `silu`, `relu`, `pow2`, `POINTWISE` | 150-183 | Pointwise activations; `erf` is the Abramowitz-Stegun 7.1.26 rational approximation | Pure, but sat in a rendering file | **Extract** → `src/viz/epilog.ts` |
| `EPILOGS` | 188-203 | The 14 named epilogs offered in the GUI | Author's own comment at line 186: *"TODO the way epis are done is kind of messy rn"* | **Extract** → `src/viz/epilog.ts` |
| `softmax_`, `softmax_tril_`, `layernorm_` | 205-262 | In-place row softmax (max-subtracted, with an early break on non-finite denominators, line 219) and whole-array layernorm | Trailing-underscore mutation convention is undocumented | **Extract** → `src/viz/epilog.ts` |
| `IN_PLACE_EPILOGS`, `getInPlaceEpilog`, `applyInPlaceEpilog_` | 256-271 | Dispatch table | — | **Extract** → `src/viz/epilog.ts` |
| `toRange`, `initArrayData_` | 277-290 | Index-range normalization and bulk fill | — | **Extract** → `src/viz/array2d.ts` |
| `grid(info, dims, f)` | 386-400 | **Block iteration over named axes.** Skips a block whose `start >= max` (comment at line 393: *"dead final block when size * n - max > size"*) | Pure index arithmetic living in a rendering file | **Extract and refactor** → `src/math/blocking.ts` (pure, tested) |
| `elem_scale`, `elem_size`, `setElemScale`, `setElemSize` | 406-418 | Module-level point-size state, scaled by device pixel ratio | Mutable module globals | **Extract** → `src/viz/sizing.ts` |
| `ZERO_COLOR`, `COLOR_TEMP` | 420-421 | Reused `THREE.Color` scratch objects | — | **Extract** → `src/viz/material.ts` |
| `emptyPoints` | 423-441 | Allocates the position/size/color buffers for an h×w point cloud, inserting `gap` between blocks | — | **Extract** → `src/viz/sizing.ts` |

### 2.2 `class Array2D` (lines 292-380, 13 members)

`fromInit`, constructor, `reinit`, `numel`, `get`, `slice`, `addr`, `absmax`,
`absmin`, `transpose`, `map`, `map2`, `add`.

Row-major `Float32Array` with `addr(i,j) = i * w + j` (line 325) — the same
layout SafeTensors uses, which is why `q_source::TensorDescriptor::linear_index`
can share the convention.

**Bug found while reading:** `map` (lines 357-363) allocates
`new Float32Array(n)` where `n` is never defined in scope — it would throw
`ReferenceError`. `map2` (365-375) correctly computes `const n = this.h * this.w`.
`map` appears to be unused.

**Decision: Extract and refactor** → `src/viz/array2d.ts`, with `map` fixed.

### 2.3 `class Mat` (lines 443-879, 32 methods)

Constructor plus: `getBlockInfo`, `grid`, `getDispH`, `getDispW`, `initViz`,
`setColorsAndSizes`, `getExtent`, `getRangeInfo`, `sizeFromData`,
`colorFromData`, `getAbsmax`, `getGlobalAbsmax`, `reinit`, `getDataArray`,
`getData`, `getColor`, `setColor`, `getSize`, `setSize`, `show`, `hide`,
`isHidden`, `bumpColor`, `isFacing`, `isRightSideUp`, `setRowGuides`,
`setFlowGuide`, `setName`, `setLegends`, `checkLabel`, `updateLabels`.

One class holding: the data (`Array2D`), the derived statistics (`absmax`,
`absmin`), the Three.js `Points` geometry, the value→size mapping
(`sizeFromData`), the value→colour mapping (`colorFromData`, HSL with
configurable zero hue / hue gap / hue spread), the row guides, the text labels,
and the visibility state. `setFlowGuide` (line 714) is an empty method —
`MatMul` overrides it and `Mat` needs the no-op to satisfy a shared call site.

**Problem:** data, statistics, and presentation are one object. There is no way
to ask for a value without also holding its geometry.

**Decision: Extract and refactor** → `src/viz/mat.ts` for now (a faithful port),
with `sizeFromData`/`colorFromData` factored toward `src/viz/sizing.ts`. The
data/presentation split is what
`apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts` exists to finish.

### 2.4 `class MatMul` (lines 924-1791, 48 methods)

The largest symbol in the codebase. Notable members:

| Method(s) | Nature | Decision |
| --- | --- | --- |
| `getBlockInfo`, `grid` | **Pure index math** — block decomposition and iteration | **Extract** → `src/math/blocking.ts` |
| `dotprod`, `ikjmul` | **Pure math** — one output cell, one product term | **Extract** → `src/math/matmul.ts` |
| `applyPointwiseEpilog` | **Pure math** | **Extract** → `src/viz/epilog.ts` |
| `scatterFromCount`, `getLeftScatter`, `getRightScatter` | **Pure math** — layout scatter from operand count | **Extract** → `src/math/blocking.ts` |
| `getVmprodBump` (1490), `getMvprodBump` (1579), `getVvprodBump` (1668) | **Mixed** — the `curi`/`curj` cursor arithmetic is pure; it is interleaved with `bumpColor`, `setRowGuides`, and scene-graph mutation | **Extract and refactor** → cursor to `src/math/animation-schedule.ts`; the highlight calls stay in `src/viz/matmul.ts` |
| `getPlacementInfo`, `getLayoutInfo`, `getExtent`, `getBoundingBox`, `center` | **Geometry** — operand placement | **Extract and refactor** → `src/layout/grid-ruler.ts` |
| `initLeft`, `initRight`, `initResult`, `initViz`, `initLeftViz`, `initRightViz`, `initResultViz`, `prepChildParams` | **Scene construction** | **Extract** → `src/viz/matmul.ts` |
| `initAnimation`, `clearAnimMats`, `getAnimResultMats`, `getAnimIntermediateParams`, `getAnimResultParams`, `onAnimDone` | **Animation state** | **Extract** → `src/viz/matmul.ts` + `src/interaction/animation.ts` |
| `disposeAll` | **Resource management** | **Extract** → `src/viz/matmul.ts` |
| `show`, `hide`, `hideInputs`, `setColorsAndSizes`, `bumpColor`, `setRowGuides`, `setFlowGuide`, `setName`, `setLegends`, `updateLabels`, `getAbsmax`, `getGlobalAbsmax`, `getDataArray`, `getData`, `getDispH/D/W` | **Delegation to children** | **Extract** → `src/viz/matmul.ts` |

**Recursive structure:** `MatMul` holds `left` and `right`, each of which is
either a `Mat` or another `MatMul` (`prepChildParams`). That is
how `(A @ B) @ C` is represented — and it is the same tree
`q_expression::Expr::MatMul` encodes on the Rust side.

### 2.5 Layout, defaults, and expression parsing (lines 885-2105)

| Symbol | Line(s) | Responsibility | Decision |
| --- | --- | --- | --- |
| `SCHEMES`, `POLARITIES`, `LEFT_PLACEMENTS`, `RIGHT_PLACEMENTS`, `RESULT_PLACEMENTS` | 885-889 | Layout enumerations | **Extract** → `src/viz/constants.ts` |
| `layoutDesc` | 891-897 | Human-readable layout summary | **Extract** → `src/viz/constants.ts` |
| `SENSITIVITIES`, `TOP_LEVEL_ANIM_ALGS`, `ANIM_ALGS`, `FUSE_MODE` | 899-904 | Colour-sensitivity and animation enumerations | **Extract** → `src/viz/constants.ts` |
| `boolToLayout`, `LAYOUT_RULES`, `childLayout`, `setLayoutScheme` | 1804-1853 | Derive child layout from parent under a named scheme | **Extract** → `src/viz/layout.ts` |
| `default_dims`, `defaultCam`, `default_epilog`, `defaultLeft`, `defaultRight`, `defaultAnim`, `defaultBlock`, `defaultLayout` | 1855-1905 | Default parameter fragments | **Extract** → `src/viz/defaults.ts` |
| `fixBlocks`, `fixShape` | 1907-1969 | Propagate a shape or blocking change through the tree so children stay conformable | **Extract** → `src/viz/expr.ts` |
| `leftLeaf`, `rightLeaf` | 1971-1972 | Walk to the leftmost/rightmost leaf | **Extract** → `src/viz/expr.ts` |
| `parseExpr` | 1976-2004 | **Hand-written parser** for the expression string in the GUI | **Extract and refactor** → `src/math/parse.ts`; superseded for tensor queries by WeightQL (`crates/q-weightql`) |
| `syncExpr` | 2006-2094 | Rebuild the parameter tree from a parsed expression | **Extract** → `src/viz/expr.ts` |
| `genExpr` | 2096-2105 | Render the parameter tree back to a string | **Extract** → `src/viz/expr.ts` |

---

## 3. `mm/util.js` — parameters, geometry helpers, text

| Symbol | Line(s) | Responsibility | Problem | Decision |
| --- | --- | --- | --- | --- |
| `MMGUIDE_MATERIAL` | 5-31 | `RawShaderMaterial` for the flow-guide arrows | Module singleton | **Extract** → `src/util/geometry.ts` |
| `makeSearchParams` | 37-40 | State → `URLSearchParams`, compressed or raw JSON | — | **Extract** → `src/util/params.ts` |
| `updateObjectFromSearchParams` | 42-103 | Restore state from the URL. Three modes: raw `params` JSON, a `config` URL, or the compressed flat form | The `config` branch (62-78) fetches an **arbitrary URL** with a **synchronous** `XMLHttpRequest` and applies the response as state | **Extract and refactor**; the `config` branch is **Deprecated** |
| `castToType` | 107-118 | String → boolean/number/string, driven by the type of the existing default | Silently falls back to a string on an unknown type | **Extract** → `src/util/params.ts` |
| `lineSeg`, `axes` | 124-138 | Line segments; `axes()` draws 128-unit RGB axes | — | **Extract** → `src/util/geometry.ts` |
| `rowGuide` | 140-162 | Row/column guide lines at a stride of `(h-1)/denom` | — | **Extract** → `src/util/geometry.ts` |
| `LEFT_ARROW_COLOR`, `RIGHT_ARROW_COLOR`, `flowGuide` | 168-234 | The two triangles showing operand flow into the product | Mutates the shared colour attributes in place (lines 184-187), and line 184 assigns index `3` twice — the third vertex's alpha is never set | **Extract and refactor** → `src/util/geometry.ts`; the duplicate-index bug is noted |
| `bbhwd`, `gbbhwd`, `center` | 240-254 | Bounding-box dimensions | — | **Extract** → `src/util/objects.ts` |
| `updatePropRec`, `updatePropsRec`, `updateProps`, `deleteProps`, `syncProp` | 260-283 | Shallow and recursive object merge | Untyped | **Extract** → `src/util/objects.ts` |
| `flatten`, `unflatten` | 287-307 | Dotted-path flattening. The comment at 285-286 states the limits: *"only handles our nested params - nothing null or undefined, no arrays, no empty subobjects"* | Documented but unenforced | **Extract** → `src/util/params.ts` |
| `compress`, `uncompress` | 309-333 | URL shortening by replacing path segments with integer indices, storing the dictionary in the same object | Clever but opaque; index keys and name keys share a namespace | **Extract** → `src/util/params.ts`, with tests |
| `copyTree` | 335-337 | Deep copy via flatten/unflatten | Inherits `flatten`'s limits | **Extract** → `src/util/objects.ts` |
| `disposeAndClear` | 343-347 | Recursive geometry disposal | Disposes geometry but **not materials or textures** | **Extract and refactor** → `src/util/objects.ts` |
| font loading + `getText` | 349-365 | Parses `assets/droid_sans_regular.typeface.js` at module load and generates text meshes | Blocking parse at import time | **Extract** → `src/util/text.ts` |

---

## 4. `mm/gui.js` — control panel

380 lines building a `lil-gui` panel over the same `params` object.

| Symbol | Line(s) | Responsibility | Problem | Decision |
| --- | --- | --- | --- | --- |
| `gui` (module-level `let`) | 7 | The panel instance | Author's own comment: *"global! we manage reinitialization, disposal etc."* | **Extract and refactor** |
| `initGui(params, callbacks, info)` | 9-381 | The entire panel | One 370-line function | **Extract and refactor** → `src/gui/research-gui.ts` (faithful) + `src/gui/mvp-gui.ts` (reduced) |
| `set`, `addNumParam`, `addIntParam`, `addChoiceParam`, `addParam` | 13-42 | Binding helpers threading `param_path`/`obj_path` accessor pairs | Path-function plumbing is repeated at every call site | **Extract and refactor** |
| `findController`, `findFolder`, `findFolders`, `clearFolder`, `addFolder` | 44-71 | Panel tree manipulation | — | **Extract** |
| `syncLayoutSchemeAndInit` | 75-81 | Show/hide layout folders when the scheme is `custom` | — | **Extract** |
| `childInit`, `childMat`, `syncChildParams` | 85-143 | Convert a leaf into a matmul node and back | Mutates the params tree in place | **Extract and refactor** |
| `addMatParams`, `addMatmulParams` | 147-293 | Recursive panel construction mirroring the expression tree | — | **Extract** |
| `evalExpr` | 299-319 | Re-parse the expression field and rebuild the panel | Name is misleading — it calls `viz.syncExpr`, not `eval` | **Extract and refactor**, renamed |
| `addDecoParams`, `addVizParams`, `addDiagParams` | 324-359 | Decoration, colour/size, and diagnostic folders | Ten `\|\|=` "temp BC" defaults (lines 185, 190, 248, 255, 258, 264, 267, 321-323, 337) patch older saved URLs | **Extract and refactor**; back-compat defaults consolidated |

---

## 5. Security finding: `eval` reachable from a URL parameter

`mm/viz.js:119-126`:

```js
function tryEvalInitExpr(expr) {
  try {
    return eval?.(`(i, j, h, w) => { try { return (${expr}) } catch (e) { return 0 } }`)
  } ...
}
```

The chain is:

1. `mm/index.html:531` restores state from the query string via
   `util.updateObjectFromSearchParams`;
2. that sets `params.left.expr` / `params.right.expr` from the URL
   (`mm/util.js:86-102`);
3. `viz.getInitFunc` (line 132) dispatches to `tryEvalInitExpr` when
   `init == 'expr'`;
4. the string is passed to `eval`.

A crafted link therefore executes attacker-chosen JavaScript in the visitor's
browser. A second, milder instance is the `config` branch of
`updateObjectFromSearchParams` (`mm/util.js:62-78`), which fetches an arbitrary
URL synchronously and applies the response as application state.

**Decision: Deprecate.** Neither is carried into
`apps/web/quatricmorph-workspace`. More broadly, this is why WeightQL is a closed
expression language: `q_expression::Expr` is a closed enum with no `eval`, no
user-defined functions, no shell interpolation, and no raw SQL, enforced by
tests in both `crates/q-weightql/src/parser.rs` and
`apps/web/query-interface/src/__tests__/weightql.test.ts`. See
`docs/decisions/ADR-006-weightql-no-arbitrary-execution.md`.

This finding is about `mm` as it stands, not a criticism of its purpose: `mm` is
a research visualizer meant to be run locally with hand-entered expressions, and
in that setting the feature is reasonable. It stops being reasonable the moment
the same code is served as a product surface.

---

## 6. `mm/lib/`, `mm/assets/`, `mm/examples/`, `mm/ref.html`

| Path | Contents | Decision |
| --- | --- | --- |
| `mm/lib/three.module.js` | Vendored Three.js | **Deprecate** → npm `three` |
| `mm/lib/lil-gui.js` | Vendored lil-gui | **Deprecate** → npm `lil-gui` |
| `mm/lib/es-module-shims.js` | Import-map polyfill | **Deprecate** → the bundler resolves modules |
| `mm/lib/jsm/controls/OrbitControls.js` | Vendored addon | **Deprecate** → `three/addons` |
| `mm/lib/jsm/loaders/FontLoader.js` | Vendored addon | **Deprecate** → `three/addons` |
| `mm/assets/ball.png` | Point sprite for `MATERIAL` | **Reuse as-is** → `public/assets/ball.png` |
| `mm/assets/droid_sans_regular.typeface.js` | Font for `getText` | **Reuse as-is** → `src/assets/` |
| `mm/examples/attngpt2/`, `mm/examples/attnqkov/` | Two worked attention examples, each an HTML page with an inline script | **Extract** → `src/examples/*.ts` |
| `mm/ref.html` | 1527-line self-contained reference page | **Extract** → `src/ref/index.ts` |
| `mm/intro/` | Prose article plus ~30 images and videos | **Reuse as-is** → `public/intro/` |
| `mm/README.md` | Usage notes | Read; superseded by the workspace README |
| `mm/LICENSE` | MIT, Meta Platforms, Inc. | **Reuse as-is** — copied verbatim to `apps/web/quatricmorph-workspace/LICENSE`, with attribution in `NOTICE.md` |

---

## 7. Summary of decisions

| Decision | Count | Examples |
| --- | --- | --- |
| **Reuse as-is** | 4 | `ball.png`, the typeface, `intro/`, `LICENSE` |
| **Extract** | ~45 | `Array2D`, `Mat`, epilogs, initializers, defaults, layout constants, geometry helpers, examples |
| **Extract and refactor** | ~20 | `MatMul` (math split from scene), `grid` → `math/blocking.ts`, animation cursors → `math/animation-schedule.ts`, `params` bag → domain modules, `initGui` → two GUIs, `disposeAndClear` → dispose materials too |
| **Deprecate** | 9 | `tryEvalInitExpr`, `tryLoadData`/`tryURLInit`, the `config` URL branch, `sampleSphere`, the import map, five vendored libraries |

## 8. Defects found while reading

Recorded because they are evidence, and because a faithful port would otherwise
carry them forward:

1. `viz.js:357-363` — `Array2D.map` references an undefined `n`; would throw.
2. `viz.js:60-64` — `sampleSphere` references an undefined `sm`; unreachable.
3. `util.js:184` — `LEFT_ARROW_COLOR.array[3]` is assigned twice in one
   statement (`[3] = [7] = [3]`), so the third vertex's alpha is never set. The
   same pattern appears at line 186 for `RIGHT_ARROW_COLOR`.
4. `util.js:343-347` — `disposeAndClear` disposes geometries but not materials
   or textures.
5. `viz.js:186` — the author's own note: *"TODO the way epis are done is kind of
   messy rn"*.
6. `index.html` (throughout) — `window` event listeners are added and never
   removed; harmless for a single-page demo, a leak once the workspace is
   embedded.
