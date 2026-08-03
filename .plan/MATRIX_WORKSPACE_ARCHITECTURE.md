# MATRIX_WORKSPACE_ARCHITECTURE — grid-aligned tensor computation

## 0. State

`apps/web/matrix-workspace/` is the `mm` port: 40 TypeScript modules, 74 tests.
The port's central achievement is that **pure math is separated from Three.js
scene state**, which is what makes real tensor blocks possible at all.

| Already extracted, pure, tested | From `mm` |
| --- | --- |
| `math/matmul.ts` — 17 tests | `MatMul.dotprod`, `ikjmul` (`viz.js:924-1791`) |
| `math/blocking.ts` — 10 tests | `grid`, `getBlockInfo`, `scatterFromCount` (`viz.js:386-400`) |
| `math/animation-schedule.ts` — 7 tests | `getVmprodBump`, `getMvprodBump`, `getVvprodBump` cursors |
| `layout/grid-ruler.ts` — 13 tests | `getPlacementInfo`, `getLayoutInfo`, `getExtent` |
| `viz/array2d.ts` — 9 tests | `Array2D`, with the `map`-references-undefined-`n` bug fixed |
| `util/params.ts` — 3 tests | `flatten`, `unflatten`, `compress`, `uncompress` |

Deliberately **not** carried forward: `tryEvalInitExpr` (`eval` from a URL
parameter), the synchronous-XHR `config` branch, `tryLoadData`/`tryURLInit`,
`sampleSphere` (dead), and five vendored libraries. Rationale in
`docs/CURRENT_ARCHITECTURE.md` §5 and `ADR-006`.

Remaining work: the shared grid core, ruled-grid rendering, sphere-block cells,
a live block adapter, and real-block multiplication. Nine tasks,
`QM-0060`…`QM-0068`.

---

## 1. Reuse decisions

`docs/CURRENT_ARCHITECTURE.md` records a per-symbol decision for all ~78 `mm`
symbols: 4 reuse-as-is, ~45 extract, ~20 extract-and-refactor, 9 deprecate. That
document is authoritative and this plan does not repeat it. What follows is what
changes **from the ported state**, which is a different question.

| Ported module | MVP change | Task |
| --- | --- | --- |
| `layout/grid-ruler.ts` | Moves to `apps/web/core/spatial/grid.ts`, re-exported here for compatibility; parameters come from the shared schema | `QM-0060` |
| `layout/tensor-frame.ts` | Gains title/shape/address labels, axis labels, row and column guides | `QM-0065` |
| `viz/mat.ts`, `viz/material.ts` | Gains the value→opacity channel; optional `InstancedMesh` path behind a flag | `QM-0063` |
| `tensor/block-adapter.ts` | `DaemonBlockSource` stops refusing and reads from the daemon | `QM-0066` |
| `viz/matmul.ts`, `interaction/animation.ts` | Drive the animation from a real block, with the full control set | `QM-0067` |
| `interaction/selection.ts` | Hover and selection carry the full metadata contract | `QM-0068` |
| new | Ruled-grid rendering | `QM-0062` |
| new | Axis binding for rank ≤ 3; rank > 3 refuses | `QM-0061` |

---

## 2. The shared grid

Specified in [`GRID_ARCHITECTURE.md`](GRID_ARCHITECTURE.md). The workspace's
obligations:

* Every position is produced by `GridRuler3D` and is snapped within `1e-6`.
* `assertVecSnapped` is called at layout boundaries — per tensor frame, per
  operand placement — not per cell.
* No position is stored per scalar. Cell centres are computed from
  `(i, j)` on demand.
* The ten parameters come from `apps/web/core`, which reads them from
  `schemas/visualization/schema.json`. The workspace does not declare its own
  defaults after `QM-0060`.

Already true and tested: `every_operand_placement_it_produces_is_on_grid`,
`snaps_positions_to_cellSize_multiples`,
`tolerates_float_accumulation_within_the_documented_tolerance`.

---

## 3. I / J / K mapping

```text
World X → J   (output columns)
World Y → I   (output rows)
World Z → K   (contraction)

A ∈ R^(m×k) on I×K     B ∈ R^(k×n) on K×J     C ∈ R^(m×n) on I×J
C[i,j] = Σ_k A[i,k] × B[k,j]
```

Shared dimensions align physically: `A.I ≡ C.I` (same Y), `A.K ≡ B.K` (same Z),
`B.J ≡ C.J` (same X).

**Recorded divergence:** this resolves to A on YZ, B on XZ, C on XY, which is not
what `ARCHITECTURE.md` §8.2 states. The code and the task specification §16 agree
against §8.2. See [`CURRENT_ARCHITECTURE.md`](CURRENT_ARCHITECTURE.md) §6.1 and
`ADR-CANDIDATE-014`; the recommended resolution is to correct §8.2, because
changing the code would invalidate 13 passing tests and the proven `mm`
placement semantics for no gain.

---

## 4. Shape support

Required combinations (task specification §16), all already covered by
`math/__tests__/matmul.test.ts`:

| Case | Shapes | Result | Test |
| --- | --- | --- | --- |
| Matrix @ Matrix | 2×3 @ 3×2 | 2×2 | ✓ |
| Matrix @ Column | 3×3 @ 3×1 | 3×1 | ✓ |
| Row @ Matrix | 1×3 @ 3×2 | 1×2 | ✓ |
| Row @ Column | 1×3 @ 3×1 | 1×1 | ✓ |
| Scalar | 1×1 @ 1×1 | 1×1 | ✓ |
| **Invalid** | 2×3 @ 2×2 | **rejected** | ✓ |

Vectors and scalars are not special cases in the layout: a row vector is a 1×n
tensor on the same grid, framed and labelled like any other. One coordinate
system, as the task specification §27 requires.

---

## 5. Modes

### Concept mode

Generated or sampled values, using the `mm` initializers (`viz/init.ts`: rows,
cols, row/col major, linear, uniform, gaussian, tril/triu mask, eye, diff) —
**without** the `expr` initializer, which was the `eval` path.

Fidelity: **not exact**, and the badge says so. A concept-mode matrix is
explanatory, and the UI must never let it be mistaken for checkpoint data.

### Real tensor-block mode

An explicitly selected block from the checkpoint, fetched through the daemon.

```text
A[0:256, 0:256] @ B[0:256, 0:256]
```

Fidelity: **exact**, for the requested extent.

**The workspace never multiplies a whole tensor to produce an animation.** The
default region is 256×256; the ceiling is `MAX_WORKSPACE_SPHERES = 262_144`
(512×512). `assertBlockIsBounded` refuses anything larger before a request is
made (`GRID-005`).

Interactive limits, and why:

| Limit | Value | Reason |
| --- | --- | --- |
| Max operand dimension | 512 | 262 144 cells = the GLB instance ceiling; one number governs both |
| Default block | 256 × 256 | 65 536 cells, 256 KiB f32 over the wire |
| Max in-browser matmul | 512³ = 1.34×10⁸ MAC | ~1 s in JS; above this the daemon computes and returns the result |
| Max animation steps | K, capped at 512 | Beyond that stepping is not legible; the UI switches to block-level animation |

---

## 6. Tensor-block adapter

`tensor/block-adapter.ts` already defines the seam:

```ts
export interface TensorBlockSource {
  fetch(request: BlockRequest): Promise<TensorBlockData>
}
export class HandEnteredSource implements TensorBlockSource { /* works */ }
export class DaemonBlockSource  implements TensorBlockSource { /* refuses */ }
```

`GRID-004` (`QM-0066`) makes `DaemonBlockSource` real:

```text
BlockRequest { canonical_address, rows: [start,end], columns: [start,end] }
  → assertBlockIsBounded                    ← refuses BEFORE any network call
  → GET /v1/tensors/{id}/blocks?rows=…&columns=…&format=qtile
  → decode .qtile in a Web Worker
  → TensorBlockData { values, shape, dtype, fidelity, provenance }
```

Three properties the stub already enforces and the implementation must keep:

* **It refuses rather than returning plausible zeros.** The existing test is named
  `the_daemon_source_refuses_rather_than_returning_plausible_zeros`. A matrix of
  zeros that looks like data is the worst possible failure here, because it is
  indistinguishable from a genuinely sparse region.
* **A request that would pull a whole tensor is refused** before the network is
  touched.
* **Fidelity travels with the data** and reaches the badge.

Decoding happens in a Web Worker so a 256 KiB decode never stalls the animation
frame.

---

## 7. Cell rendering

Specified in [`GRID_ARCHITECTURE.md`](GRID_ARCHITECTURE.md) §5. Workspace
obligations:

| Rule | Implementation |
| --- | --- |
| One sphere per scalar, within a bounded block | Instanced or point-sprite; `ADR-CANDIDATE-015` |
| Centred on its logical cell | `cellCenterLocal(i, j, config)` |
| A zero keeps its cell | Rendered at `r_min`, `a_min` — never absent |
| Sign is distinguishable | Negative / zero / positive palettes |
| Magnitude through scale, reinforced by colour and opacity | §5.1 of the grid doc |
| Never crosses a cell boundary | `r_max ≤ 0.5 × cellSize` |
| Labels only when selected **and** the cell exceeds ~24 px | One shared texture; never one DOM node per weight |

The opacity channel is new: nothing in `viz/mat.ts` or `viz/material.ts` drives
alpha from data today. `QM-0063` adds it to the fragment shader and to
`setColorsAndSizes`'s call path.

---

## 8. Multiplication interaction

For a selected result cell `C[i,j]`, the deterministic sequence
(task specification §18):

```text
highlight row A[i, :]
→ highlight column B[:, j]
→ highlight shared K positions
→ show A[i,k] × B[k,j]
→ update running sum
→ reveal C[i,j]
→ advance k
```

This is `mm`'s proven cursor logic, already extracted as a pure state machine
(`math/animation-schedule.ts`, 7 tests). The workspace drives it; the schedule
computes it.

**Controls:** play · pause · step · previous step · reset calculation · reset
view · fit view. `previous` is why the schedule is a pure function of an index
rather than an accumulating mutation — stepping backward must produce exactly the
state that stepping forward produced, and an accumulator cannot guarantee that.

**Determinism:** the same block and the same step index always produce the same
highlight set and the same running sum. Verified by driving the schedule to step
`n`, resetting, and driving it again.

---

## 9. State domains

Task specification §18 requires these to be separate. The port has already made
most of the split; `QM-0067` finishes it.

| Domain | Owns | Module |
| --- | --- | --- |
| Tensor data | Values, shape, dtype, fidelity, provenance | `tensor/` |
| Expression | The AST, bound names, validation | `math/parse.ts`, `math/validate.ts` |
| Layout | Grid config, placements, extents | `apps/web/core/spatial` |
| Selection | Selected cells, rows, columns, blocks | `interaction/selection.ts` |
| Animation | Step index, play state, speed | `math/animation-schedule.ts` |
| Camera | Position, target, preset | `app/scene.ts` |
| Display | Palette, normalization, label visibility, grid toggles | `gui/` |
| Query | Pending plan, cost, cancellation token | `query-interface` |
| Serialized | The subset that goes in the URL | `app/url.ts` |

Why it matters concretely: `mm` had one untyped `params` bag that every subsystem
reached into (`docs/CURRENT_ARCHITECTURE.md` §1), and any parameter change
rebuilt the entire scene (`initObj`, `index.html:356-395`). With domains split, a
palette change touches display state and re-renders; it does not re-fetch a block
or reset an animation.

---

## 10. Hover and selection metadata

`GRID-012`. Hover shows, at minimum:

```text
canonical address    model.layers[10].self_attention.query_projection.weight
alias                Q[10]
logical index        [1031, 1802]
block index          block (4, 7) of (16, 16)
value                0.006408154
shape · dtype        [4096, 4096] · F32
fidelity             ▣ EXACT
source shard         model-00002-of-00002.safetensors @ 419928
```

Selection is conveyed by **at least two** of: scale bump, outline, brightness,
guide thickness, opacity, frame emphasis, animated path. Never colour alone —
task specification §18, and the same accessibility reasoning as the fidelity
badges.

---

## 11. Camera fitting and disposal

* **Fit** uses the multiplication volume's extent (`mulVolumeExtent`), not the
  scene's bounding box, so the operands stay framed as a unit.
* **Preserve scale on re-init.** `mm` compared old and new bounding-box magnitude
  to keep camera distance stable when a shape changed (`index.html:361-378`). The
  behaviour is worth keeping; the implementation is worth naming.
* **Disposal must dispose materials and textures**, not only geometries.
  `mm`'s `disposeAndClear` (`util.js:343-347`) disposes geometry only —
  a recorded defect. `QM-0067` fixes it in the port, and `QM-0082` soaks it:
  100 re-initializations must not grow `renderer.info.memory`, which the GUI
  already surfaces.

---

## 12. Requirements

| ID | Requirement | State | Task |
| --- | --- | --- | --- |
| `GRID-001`, `GRID-002` | One spatial authority; snap invariant at `1e-6` | ✓ Verified | verify only |
| `GRID-003` | `TensorGridFrame` — boundary, margins, labels, anchor | Implemented | `QM-0065` (test + complete) |
| `GRID-004` | Live tensor-block adapter | Stub | `QM-0066` |
| `GRID-005` | Block-request ceiling; no whole-tensor transfer | ✓ Verified | verify only |
| `GRID-006` | Shared spatial core consumed by both apps | New | `QM-0060` |
| `GRID-007` | Axis binding; rank ≤ 3; rank > 3 refuses | New | `QM-0061` |
| `GRID-008` | Ruled-grid rendering | New | `QM-0062` |
| `GRID-009` | Sphere-block cells with value→opacity | New | `QM-0063` |
| `GRID-010` | Sphere budget and degradation | New | `QM-0064` |
| `GRID-012` | Hover/selection metadata; never colour-only | New | `QM-0068` |
| `MATMUL-001`…`MATMUL-005` | Pure matmul, blocking, schedule, CPU reference, regression bar | ✓ Verified | verify only |
| `MATMUL-006` | Real-block `A @ B` with the full control set | New | `QM-0067` |
