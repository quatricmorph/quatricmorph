# Phase 06 — Grid matrix workspace

## Goal

```text
Selected tensor blocks → assign A and B → validate shapes → visualize A @ B
   on ONE shared 3D grid ruler, as sphere blocks whose size, colour, and
   opacity encode the value
```

## What is already done

The `mm` extraction succeeded. Pure math is separated from Three.js scene state,
which is the precondition for real tensor blocks:

| Module | Tests | From |
| --- | --- | --- |
| `math/matmul.ts` | 17 | `MatMul.dotprod`, `ikjmul` |
| `math/blocking.ts` | 10 | `grid`, `getBlockInfo`, `scatterFromCount` |
| `math/animation-schedule.ts` | 7 | the three `get*ProdBump` cursors |
| `layout/grid-ruler.ts` | 13 | `getPlacementInfo`, `getLayoutInfo`, `getExtent` |
| `viz/array2d.ts` | 9 | `Array2D`, with the undefined-`n` bug fixed |

The required shape matrix already passes: `2×3 @ 3×2`, `3×3 @ 3×1`, `1×3 @ 3×2`,
`1×3 @ 3×1`, `1×1 @ 1×1`, and `2×3 @ 2×2` rejected.

## The gaps

1. The grid is **local to one package**. The viewer has its own LOD constants;
   Rust has its own geometric error. → `QM-0060`
2. **No opacity channel exists.** `mat.ts` has `sizeFromData` and
   `colorFromData`; nothing drives alpha. → `QM-0063`
3. **The grid is not drawn.** `minorGridSpacing` and `majorGridInterval` are
   config values nothing renders. → `QM-0062`
4. **`DaemonBlockSource` refuses.** No real checkpoint data reaches the
   workspace. → `QM-0066`
5. **Rank > 2 is unsupported**, and `depthSpacing` is unused. → `QM-0061`

## Entry conditions

* **G1** passed — the shared contract exists.
* `ADR-CANDIDATE-014` (plane mapping), `015` (cell primitive), `016` (axis
  binding) decided.
* A running daemon for `QM-0066`; **G2** for real tile data.

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0060` | Shared spatial core package | Implementation | `GRID-006`, `MVP-26` |
| `QM-0061` | Axis binding; rank ≤ 3; rank > 3 refuses | Implementation | `GRID-007` |
| `QM-0062` | Ruled-grid rendering | Implementation | `GRID-008` |
| `QM-0063` | Sphere-block cells with value→opacity | Implementation | `GRID-009`, `PERF-003`, `MVP-27` |
| `QM-0064` | Sphere budget and degradation | Implementation | `GRID-010` |
| `QM-0065` | `TensorGridFrame` completion | Implementation | `GRID-003`, `MVP-27` |
| `QM-0066` | Live tensor-block adapter | Implementation | `GRID-004`, `MVP-25` |
| `QM-0067` | Real-block `A @ B` with full controls | Implementation | `MATMUL-006`, `MVP-28`, `MVP-30`, `MVP-31` |
| `QM-0068` | Hover and selection metadata contract | Implementation | `GRID-012` |

## Design constraints

* **Every position is derived, never stored.**
  `origin + tensor_anchor + block_origin×cellSize + index×cellSize + padding`.
  Storing would break the inverse, and the inverse is what makes a click resolve
  to an address exactly rather than approximately.
* **Snap invariant at `1e-6`**, asserted at layout boundaries, not per cell.
* **A zero keeps its cell.** `v = 0` renders at `r_min`, `a_min` — never absent.
  An absent sphere must keep meaning *no data*.
* **A sphere never crosses its cell boundary**: `r_max ≤ 0.5 × cellSize`.
* **Magnitude must survive the loss of colour or opacity.** Scale is primary;
  the other two are redundant reinforcement, because §18 forbids conveying state
  by colour alone.
* **`MAX_WORKSPACE_SPHERES = 262_144`**, equal to the GLB instance ceiling. Above
  it, **degrade to aggregate and say so in the badge** — never silently truncate.
* **`assertBlockIsBounded` refuses before the network is touched.**
* **Rank > 3 refuses.** Flattening a `[32,128,128]` tensor to `[32,16384]` invites
  the viewer to read adjacency that does not exist.
* **`previous` is exactly `forward`'s inverse**, which is why the animation
  schedule is a pure function of an index rather than an accumulator.

## Exit conditions

1. `apps/web/core` exists; neither app declares a spatial constant of its own.
2. Rank 0, 1, 2, and 3 tensors all frame and label on one coordinate system;
   rank 4 returns `NotImplemented` carrying `GRID-007`.
3. Major and minor grid lines, the origin marker, and axis labels render and
   toggle independently.
4. A 256×256 block renders as 65 536 spheres with size, colour, **and opacity**
   driven by value; `v = 0` is visibly present; no sphere overlaps a neighbour.
5. Render budgets measured and recorded at 65 536 **and** 262 144 cells.
6. A block fetched from the daemon renders with its fidelity shown, and a
   whole-tensor request is refused before any network call.
7. `A @ B` on two real blocks matches the CPU reference; play, pause, step,
   previous, and reset all work, and stepping is deterministic.
8. Hover shows canonical address, alias, logical index, block index, value,
   shape, dtype, fidelity, and source shard.
9. Selection is legible in a **greyscale** screenshot.
10. 100 re-initializations return `renderer.info.memory` to baseline —
    including materials and textures, which `mm`'s `disposeAndClear` missed.

## Parallelization

`QM-0060` first — everything else imports from it. Then `QM-0061`, `QM-0062`,
`QM-0065` are independent. `QM-0063` → `QM-0064` sequential. `QM-0066` →
`QM-0067` sequential. `QM-0068` last, since it touches hover paths several other
tasks modify.

## Risks

| Risk | Mitigation |
| --- | --- |
| R5 — 262 144 spheres too slow | Measure both counts; sprites are the default; `GRID-010` degradation designed in |
| R8 — memory growth | Explicit disposal in `QM-0067`; soaked in `QM-0082` |
| Grid invariant broken by sprite sizing | `r_max ≤ 0.5 × cellSize` clamp in `QM-0063` |
