# GRID_ARCHITECTURE — the shared 3D grid ruler

> *"All scalar, tensor, and matrix visualizations and everything
> multiple-dimension tensor should arrange and align inside a 3D rule grid
> system. The 3D grid system should be shared across all visualizations and
> mathematical operations."*

This document specifies that system. It is the reason
[`REPOSITORY_ANALYSIS.md`](REPOSITORY_ANALYSIS.md) §5 identifies the
three-spatial-authority problem as the plan's first structural blocker: a grid
that is not shared is not a grid, it is a coincidence.

---

## 1. What the grid is for

The grid is **not decoration**. It is the coordinate system that makes four
otherwise unrelated guarantees possible:

1. **A click resolves to an address.** If a cell's world position is derived from
   its logical index by one invertible rule, the inverse is exact and
   `AC-004` is satisfiable. If positions are stored, they drift, and the inverse
   becomes a nearest-neighbour guess.
2. **Two visualizations of the same tensor agree.** The Cesium viewer's tile for
   block `(4,7)` of tensor `T` and the matrix workspace's rendering of the same
   block must place scalar `[1031, 1802]` in the same place relative to the
   tensor's own anchor.
3. **Nothing is stored per scalar.** `ARCHITECTURE.md` §19 forbids storing
   absolute positions for every scalar. Derivation is the only alternative.
4. **Alignment survives operations.** `A @ B = C` is legible only because `A.I`
   lines up with `C.I`, `A.K` with `B.K`, and `B.J` with `C.J` — which is a
   statement about a shared coordinate system, not about rendering.

---

## 2. The single contract

**One definition, in `schemas/visualization/schema.json`, consumed by everything.**
(`GRID-006`, task `QM-0004`.)

### 2.1 Grid parameters

The ten parameters already present in
`apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts:25-37`, promoted to the
schema and given Rust and TypeScript consumers.

| Parameter | Default | Meaning |
| --- | --- | --- |
| `cellSize` | `1` | Edge length of one logical cell. The quantum of every position |
| `minorGridSpacing` | `1` | Ruled-line interval, in cells |
| `majorGridInterval` | `5` | Every Nth minor line is drawn heavy and labelled |
| `tensorPadding` | `1` | Gap between a tensor's frame and its first cell centre |
| `labelMargin` | `1` | Reserved band outside the frame for titles and shapes |
| `framePadding` | `1` | Gap between the frame and neighbouring content |
| `operandGap` | `4` | Separation between operand planes (`mm`'s `layout.gap`) |
| `axisMargin` | `1` | Reserved band for axis ticks and labels |
| `depthSpacing` | `0` | Separation along the stacking axis for rank > 2 facets |
| `origin` | `{0,0,0}` | Workspace origin |

### 2.2 The invariant

```text
position.x % cellSize ≈ 0
position.y % cellSize ≈ 0
position.z % cellSize ≈ 0
```

**Tolerance: `1e-6` of a cell**, already defined as `GRID_SNAP_TOLERANCE` and
justified in place: positions are built by repeated addition, so f64 rounding
leaves a residue on the order of `1e-15` per operation; `1e-6` is far above that
and far below anything visible at any zoom.

Enforcement is `GridRuler3D.assertSnapped` / `assertVecSnapped`, called **at
layout boundaries, not per cell** — per-cell assertion would cost more than the
rendering. `GRID-002` covers this today with
`tolerates_float_accumulation_within_the_documented_tolerance`.

### 2.3 Position derivation

Positions are **computed, never stored**:

```text
world_position =
      workspace_origin
    + tensor_anchor            (from the tensor's place in the model layout)
    + block_origin  × cellSize (block coordinates within the tensor)
    + logical_index × cellSize (scalar coordinates within the block)
    + tensorPadding
```

Every term is derived from a logical address. Inverting the sum recovers the
address exactly, which is what makes picking correct rather than approximate.

The GPU-side form is the same statement in `ARCHITECTURE.md` §11.1:
`position = tile_origin + decode_morton(morton_coordinate) × cell_spacing`.
The `.qtile` payload stores a Morton coordinate, not a position — 4 bytes for a
coordinate instead of 12 for a float triple, and no drift.

---

## 3. Axis binding, and the path to rank > 3

The grid must serve rank-1 (vectors), rank-2 (matrices), rank-3 (blocked or
batched tensors), and be *designed* for higher ranks (`GRID-007`,
[`ADR-010`](../docs/decisions/ADR-010-tensor-rank-ceiling.md) — **accepted**:
rank ≤ 3 implemented, rank > 3 refuses).

### 3.1 The binding table

An **axis binding** maps each tensor axis to a world axis or a facet rule:

```text
AxisBinding = {
  tensor_axis: u8,             // index into TensorDescriptor::shape
  role: "row" | "column" | "depth" | "facet",
  world_axis: "X" | "Y" | "Z" | null,   // null for facet
  facet: { stride: u32, wrap: u32 } | null
}
```

### 3.2 MVP bindings, implemented

| Rank | Binding | Layout |
| --- | --- | --- |
| **0** (scalar) | — | One cell at the tensor anchor. Still framed, still labelled |
| **1** (vector) | axis 0 → row **or** column, chosen by operand role | Row vector runs along X; column vector along Y. Matches the required `1×3 @ 3×2` and `1×3 @ 3×1` cases |
| **2** (matrix) | axis 0 → Y (I), axis 1 → X (J) | The plane. Row-major, matching `Array2D.addr(i,j) = i*w + j` and SafeTensors |
| **3** | axis 0 → facet, axes 1,2 → Y,X | Facets stacked along Z at `depthSpacing`, so a `[H, m, n]` attention tensor reads as H aligned planes |

### 3.3 Rank > 3 — the extension point

**Not implemented in the MVP.** The designed rule, recorded so that implementing
it later is additive:

* Axes are partitioned into a **display pair** (→ X, Y), an optional **depth
  axis** (→ Z), and a **facet set**.
* The facet set is laid out as a grid of grids: facet index → `(row, column)` in
  a meta-grid whose cell is one whole tensor frame plus `framePadding`.
* Meta-grid placement uses the *same* ruler and the same invariant, one level up.
  This is why the invariant is stated over positions and not over cells: it holds
  recursively.

Until implemented, `bindAxes(shape)` **returns `NotImplemented` carrying
`GRID-007`** for `shape.len() > 3`. It does not silently flatten, reshape, or
pick the first three axes — that would produce a confidently wrong picture, which
is the failure mode this repository has consistently refused (`SRC-014`,
`NSIR-001`, `dtype::tests::unknown_dtype_is_rejected_not_guessed`).

---

## 4. Operand placement for `A @ B = C`

Preserved from the working `mm` semantics, now stated as a grid rule.

```text
World X → J   (output columns)
World Y → I   (output rows)
World Z → K   (contraction)

A ∈ R^(m×k)  on I×K     B ∈ R^(k×n)  on K×J     C = A@B ∈ R^(m×n)  on I×J
C[i,j] = Σ_k A[i,k] × B[k,j]
```

Shared-dimension alignment is a placement constraint, not a rendering choice:

| Constraint | Consequence |
| --- | --- |
| `A.I` ≡ `C.I` | A and C share Y. A row of A is at the same height as its result row |
| `A.K` ≡ `B.K` | A and B share Z. The contraction index is one physical direction |
| `B.J` ≡ `C.J` | B and C share X. A column of B sits above its result column |

`placeOperands` (`grid-ruler.ts:140`) already implements this with `mm`'s
polarity, left/right, and result placement options, and every position it emits
is snapped — `every_operand_placement_it_produces_is_on_grid`.

**Note the recorded divergence:** this mapping resolves to A on the YZ plane, B
on XZ, C on XY, which is *not* what `ARCHITECTURE.md` §8.2 says. See
[`CURRENT_ARCHITECTURE.md`](CURRENT_ARCHITECTURE.md) §6.1 and
[`ADR-009`](../docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md),
which **accepted** the code's mapping. §8.2 is corrected by `QM-0090`.

---

## 5. Sphere-block cells

The product requirement: *each matrix visualizes as sphere blocks; each sphere is
one scalar; size, colour, and opacity are determined by the value.*

### 5.1 Value → channel encoding

Let `v` be the scalar, `absmax` the block's maximum magnitude (or a
user-selected normalization range), and `s = clamp(|v| / absmax, 0, 1)`.

| Channel | Mapping | Purpose | Status |
| --- | --- | --- | --- |
| **Scale** | `radius = cellSize × (r_min + (r_max − r_min) × f(s))`, `f` selectable: linear, `log(1+s)/log 2`, or signed-percentile | Magnitude. The channel that must always work | `sizeFromData` exists (`mat.ts:110`) |
| **Colour** | Sign → palette: negative / zero / positive, with HSL zero-hue, hue-gap, hue-spread as today | Sign, and magnitude as a secondary cue | `colorFromData` exists (`mat.ts:144`) |
| **Opacity** | `alpha = a_min + (1 − a_min) × s`, `a_min ≥ 0.15` | De-emphasise near-zero weights so structure is visible through a dense block | **New — `QM-0063`** |

Three rules constrain this:

1. **A zero keeps its cell.** `v = 0` renders at `r_min` and `a_min`, never
   nothing. An absent sphere means *no data*, and that distinction must survive.
2. **Magnitude must survive the loss of any one channel.** Task specification §18
   forbids conveying selection by colour alone; the same logic applies to
   opacity, which is unreliable on projectors, in screenshots, and for viewers
   with low-contrast displays. Scale is the primary channel; colour and opacity
   are redundant reinforcement.
3. **A sphere never crosses its cell boundary.** `r_max ≤ 0.5 × cellSize` by
   construction, so `radius ≤ cellSize/2` and neighbours cannot overlap. Without
   this, a dense positive region becomes a solid blob and the grid stops being
   readable.

### 5.2 Numerical labels

Labels are rendered **only** when both hold: the block is the current selection,
and the projected cell size exceeds a threshold in pixels (default 24 px).
`ARCHITECTURE.md` §19's warning about data explosion applies to DOM nodes too —
a label per weight is a per-scalar object by another name. Labels are drawn into
a single canvas texture or a shared instanced-text mesh, never as one DOM element
per cell.

### 5.3 Rendering strategy — an open decision

`ADR-CANDIDATE-015`. Two viable options, to be decided by measurement in
`QM-0063`, not by preference:

| | Point sprites (today) | `InstancedMesh` spheres |
| --- | --- | --- |
| What exists | `THREE.Points` + `ball.png` + `ShaderMaterial` (`viz/material.ts`) | Nothing |
| Cost per cell | 1 vertex | ~80–320 triangles at low LOD |
| Screen size | Distance-derived (`mag * pointSize / -mvPosition.z`) — **not** grid-derived | Grid-derived; a cell is a cell |
| Occlusion / lighting | None; always camera-facing | Real |
| Opacity | Straightforward in the fragment shader | Needs sorting or order-independent blending |
| Picking | `raycaster.params.Points.threshold` (already used) | Standard instanced raycast |

**Recommended default: keep sprites for the MVP**, add the opacity channel, and
add an `InstancedMesh` path behind a flag with a measured comparison at 65 536
cells. Sprites already pass 13 grid tests and the whole `mm` port; replacing the
renderer to satisfy a word in the requirement would risk the alignment guarantees
that matter more. If the measurement shows sprites break the grid invariant
visually at close range, the flag flips — and that is a finding, not a rewrite.

### 5.4 Budget and degradation

```text
MAX_WORKSPACE_SPHERES = 262_144        # 512 × 512; matches q_gltf::MAX_INSTANCES_PER_TILE
DEFAULT_BLOCK          = 256 × 256     #  65 536 spheres
```

Above the budget the workspace **degrades to an aggregate cell representation and
says so in the exactness badge**. It never silently truncates, and it never
requests the block: `assertBlockIsBounded` refuses first (`GRID-005`,
`refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser`).

The two ceilings are deliberately equal. A tile that would be too big to render
in the workspace is also too big to emit as a GLB, so one number governs both and
there is no regime where the pipeline produces something the viewer cannot show.

---

## 6. Ruled-grid rendering

`GRID-008`, task `QM-0062`. The grid must be *visible*, or "aligned to the grid"
is an unverifiable claim.

| Element | Rule |
| --- | --- |
| Minor lines | Every `minorGridSpacing` cells, thin, low contrast |
| Major lines | Every `majorGridInterval` minor lines, heavier, labelled with the logical index |
| Origin marker | At `origin`, distinct from both |
| Axis labels | `I`, `J`, `K` for operands; tensor axis names from NSIR (`output_channel`, `input_channel`) where known, positional indices where `unknown` |
| Tensor frame | Outer boundary + inner margin + title margin, from `TensorGridFrame` (`layout/tensor-frame.ts`) |
| Frame labels | Canonical address, alias, shape, dtype — e.g. `Q[10] [256 × 256] f32` |
| Toggles | Show major / show minor / show labels / show frames, independently, per the required UI controls |

Grid geometry is drawn as line segments in **one** buffer per plane, not one
object per line. `mm`'s `util.js:124-162` `lineSeg` / `rowGuide` helpers are the
starting point and already carry the row/column guide idiom.

---

## 7. Model-scale layout in the viewer

The same ruler governs the Cesium scene. Spatial placement is deterministic and
derived from logical addresses (`ARCHITECTURE.md` §13's requirement that layout
must not use arbitrary scattered offsets).

```text
layer_index          → primary model axis (Z), spaced by layerSpacing
module role          → secondary grouping axis (X), ordered by a fixed role order
tensor index in role → local tensor grid (X,Y) within the module cell
block coordinates    → local block grid within the tensor frame
scalar coordinates   → procedural cell coordinates within the block
```

Every level is the same rule applied one scale down, with its own padding drawn
from the same parameter set. Two properties follow:

* **Zoom is continuous in meaning.** Descending from model to layer to tensor to
  block to scalar never changes the coordinate convention, so the breadcrumb and
  the camera agree.
* **`tensor_anchor` is a pure function of the canonical address.** It can be
  computed in the browser without a round trip, which is what makes "fit
  selection" and "search by address" instant.

Bounding volumes for `tileset.json` are derived from the same layout, so a tile's
3D Tiles `box` and the workspace's frame for the same block describe the same
region of space.

---

## 8. Consistency enforcement

Without a test, "shared" degrades to "copied" within one release. `QM-0005`
adds:

1. **Schema conformance (Rust).** `q-tileset` and `q-tensor-runtime` load the
   spatial contract from `schemas/visualization/schema.json` at build or test
   time; a constant that disagrees fails the test.
2. **Schema conformance (TypeScript).** `apps/web/core` does the same, and both
   web apps import from it rather than declaring constants.
3. **Cross-language golden vector.** A fixed table of
   `(lod, distance, expected_geometric_error, expected_decision)` and
   `(i, j, block, expected_position)` is checked into `schemas/` and asserted by
   both a Rust test and a vitest test. If either language changes a rule, one
   suite goes red.

This is the mechanism that the current hand-mirrored comment
(*"mirrors `q_tileset::GeometricError`"*) lacks.

---

## 9. Requirements introduced here

| ID | Requirement | Task |
| --- | --- | --- |
| `GRID-006` | One spatial contract, consumed by Rust and both web apps | `QM-0004`, `QM-0060` |
| `GRID-007` | Axis binding; rank ≤ 3 implemented, rank > 3 refuses with this ID | `QM-0061` |
| `GRID-008` | Ruled-grid rendering: minor, major, origin, axis labels, toggles | `QM-0062` |
| `GRID-009` | Sphere-block cells; value → scale, colour, **opacity** | `QM-0063` |
| `GRID-010` | Sphere budget and documented degradation to aggregate | `QM-0064` |
| `GRID-011` | Cross-language spatial conformance test | `QM-0005` |
| `GRID-012` | Selection and hover metadata contract; never colour-only | `QM-0068` |
