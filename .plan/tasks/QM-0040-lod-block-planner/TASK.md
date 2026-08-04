# QM-0040 — LOD ladder and block-layout planner

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 04 — Tensor tiles, GLB, and tileset

## Objective

Plan the complete tile pyramid for a model — every tile's LOD, extent, bounds,
geometric error, parent, and children — **without writing anything**.

## Repository Evidence

* `q_tensor_runtime::Lod` — 6-level closed enum, `parent()`, `child()`,
  `carries_exact_values()` true only at level 5.
* `BlockExtent::clamped_to(shape)` — clamped, never padded.
* `TileId::for_block(tensor, lod, extent)` — stable, extent- and LOD-sensitive.
* `q_tileset::GeometricError::for_lod` — `1024 / 2^lod`;
  `TilesetNode::validate_refinement`.
* `q_gltf::MAX_INSTANCES_PER_TILE = 262_144`.
* `schemas/visualization/spatial-contract.json` (from `QM-0004`) — the grid
  parameters and the ladder.

## Requirements Covered

`TILE-010`; enables `TILE-004`, `GLB-001`, `CESIUM-001`.

## Dependencies

`QM-0031`, `QM-0021`, `QM-0005`.

## Blocks

`QM-0041`, `QM-0042`, `QM-0044`.

## Parallelization

Lane A, first Phase 04 task. Runs alone.

## Program Boundary

`crates/q-tensor-runtime`, `crates/q-tileset`.

## Scope

* `PyramidPlan`: model → subsystems → layers → tensors → blocks, with a node per
  level.
* Block size selection per [`TILING_ARCHITECTURE.md`](../../TILING_ARCHITECTURE.md)
  §2.1, honouring dtype width, tensor dimensions, memory budget, and the instance
  ceiling.
* Bounds from the shared grid layout ([`GRID_ARCHITECTURE.md`](../../GRID_ARCHITECTURE.md)
  §7), **derived from logical addresses**.
* Assert parent bounds contain children's.
* Assert geometric error decreases strictly with depth.
* Report total tiles, total bytes, and estimated output size.

## Out of Scope

Writing `.qtile`, GLB, or `tileset.json` · reading any weight byte · rank > 3
(`GRID-007` refuses).

## Files Expected to Change

* `crates/q-tensor-runtime/src/lib.rs`
* `crates/q-tileset/src/lib.rs`

## Files Expected to Add

* `crates/q-tensor-runtime/src/pyramid.rs`
* `crates/q-tensor-runtime/tests/pyramid_plan.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```rust
pub struct PyramidNode {
    pub tile_id: TileId, pub parent: Option<TileId>,
    pub lod: Lod, pub subject: Subject,       // Model|Subsystem|Layer|Tensor|Block
    pub extent: Option<BlockExtent>,
    pub bounds: BoundingBox, pub geometric_error: f64,
    pub instance_count: u32, pub children: Vec<TileId>,
}
```

Bounds use the 3D Tiles `box` form, identical to `q_tileset::BoundingBox` and to
what `QM-0021` persists — one shape end to end, or drift is guaranteed.

## Memory and Performance Constraints

* Planning is **pure metadata arithmetic**; it reads no payload.
* A 7 B model plans ~100 000 block nodes. Plan memory ≈ 100 000 × ~200 B = 20 MB.
* `MAX_TILESET_NODES = 1_000_000` — above it, planning refuses and names implicit
  tiling as the extension point.
* Planning a 4096×4096 tensor under 10 ms.

## Implementation Plan

1. Read the spatial contract for grid parameters and the ladder.
2. Walk model → subsystem → layer → tensor from the catalog.
3. Per tensor, choose the block size against the four constraints; grid the
   tensor with `clamped_to`.
4. Compute each node's bounds from its logical address and the grid rule.
5. Assign geometric errors from the contract.
6. Assert containment and strict refinement; fail planning if either breaks.
7. Report totals.

## Error Handling

* Rank > 3 → `NotImplemented` carrying `GRID-007`. **Never flattened.**
* A tensor whose block count would exceed `MAX_TILESET_NODES` → refuse, naming
  the limit and implicit tiling.
* Parent bounds not containing a child → **planning error**, not a warning: a
  contained child would be culled and the failure would look like a rendering
  glitch.
* Instance count above the ceiling → subdivide further; if impossible, refuse.

## Acceptance Criteria

1. A 4096×4096 f32 tensor plans 256 LOD-4 nodes at 256×256, plus its LOD-3 node.
2. A 4000×4000 tensor produces clamped edge blocks; the last is 160×160.
3. Geometric error strictly decreases at every parent/child pair.
4. Parent bounds contain children's bounds at every level.
5. `instance_count ≤ 262_144` for every node.
6. Rank-4 tensor → `NotImplemented` with `GRID-007`.
7. Planning reads **no** payload byte.
8. A 4096² tensor plans in under 10 ms.
9. Two runs produce identical plans, including tile IDs.

## Verification Plan

**Automated** — `pyramid_plan.rs` with containment, refinement, and determinism
assertions.
**Manual** — dump a plan for the large fixture and inspect the node counts.

## Suggested Commands

```bash
cargo test -p q-tensor-runtime --test pyramid_plan          # introduced here
cargo run -p q-cli -- plan-pyramid fixtures/tiny-llama-large --dry-run   # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| 4096² f32 | 256 block nodes + 1 tensor node |
| 4000² f32 | Edge blocks clamped; last 160×160 |
| 128×48 (small fixture) | One clamped block, not a padded 256×256 |
| Every parent/child pair | Error strictly decreases; bounds contain |
| A tensor forcing > 262 144 instances | Subdivided further |
| Rank-4 tensor | `NotImplemented` / `GRID-007` |
| Plan twice | Identical, incl. tile IDs |
| Planning with a payload-read guard | No payload read |

## Risks

| Risk | Mitigation |
| --- | --- |
| Bounds computed from geometry rather than the grid rule | Derived from logical addresses; containment asserted |
| Block size chosen by one constraint only | All four constraints are asserted in the tests |
| Plan memory grows unmanageably | `MAX_TILESET_NODES` refuses, naming the extension point |

## Completion Evidence

* Plan dump for the large fixture with node counts per LOD.
* Containment and refinement assertion output.
* Planning time for a 4096² tensor.
* Determinism comparison across two runs.
