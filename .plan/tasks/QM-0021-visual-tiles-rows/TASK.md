# QM-0021 — `visual_tiles` rows and tile↔tensor resolution

## Status

Blocked

Unblocks when `QM-0020` reaches `Complete`.

## Phase

Phase 02 — Catalog and NSIR completion

## Objective

Write and query `visual_tiles`, with resolution working **both ways**: tile → tensor
(for picking) and tensor → tile (for search and fly-to).

## Repository Evidence

* `crates/q-catalog/src/schema.rs:111` — `visual_tiles(tile_id, parent_tile_id,
  model_id, tensor_id, lod, bounds, geometric_error, qtile_uri, glb_uri,
  child_count)`, index `(model_id, lod)`. **Never written.**
* `q_tensor_runtime::TileId::for_block(tensor, lod, extent)` (`:264`), `to_hex()`,
  `content_hash()`; `TILE-003` verified extent- and LOD-sensitive.
* `schemas/visualization/schema.json` — `visual_tile_row` requires
  `qtile_uri` whenever `glb_uri` is set.
* `q_tileset::BoundingBox` — centre + three half-axes.

## Requirements Covered

`CAT-012`, `MVP-06`; enables `CESIUM-007`, `AC-004`.

## Dependencies

`QM-0020`, `QM-0012`.

## Blocks

`QM-0041`, `QM-0044`, `QM-0053`, `QM-0055`.

## Parallelization

**Sequential after `QM-0020`, before `QM-0022`** — shared file
`crates/q-catalog/src/lib.rs`.

## Program Boundary

`crates/q-catalog`.

## Scope

* Insert, update, and query `visual_tiles`.
* `tile_for_address(canonical, lod)` and `address_for_tile(tile_id, local_index)`.
* Subtree queries: children of a tile, tiles at a LOD for a model.
* Enforce the schema invariant: `glb_uri` set ⇒ `qtile_uri` set.
* Enforce `parent.geometric_error > child.geometric_error` at write time.

## Out of Scope

Generating tiles (`QM-0041`, `QM-0042`) · emitting `tileset.json` (`QM-0044`) ·
serving tiles over HTTP.

## Files Expected to Change

* `crates/q-catalog/src/lib.rs`

## Files Expected to Add

None.

## Files Expected to Remove or Deprecate

None.

## Data Contracts

`bounds` is stored as the 3D Tiles `box` form — six `f64` — matching
`q_tileset::BoundingBox` and `schemas/visualization/schema.json`. Storing a
different bound shape here than the tileset emits would guarantee drift.

`address_for_tile` returns:

```jsonc
{ "canonical_address": "model.layers[10].self_attention.query_projection.weight",
  "tensor_id": "…", "block_extent": { "row_start": 1024, "row_end": 1280,
                                      "column_start": 1792, "column_end": 2048 },
  "logical_index": [1031, 1802], "lod": 4 }
```

## Memory and Performance Constraints

* `tile_for_address` and `address_for_tile` must be **indexed lookups**, not
  scans — they run on every pick, with a < 50 ms budget.
* Add an index on `(model_id, tensor_id, lod)`; `(model_id, lod)` alone does not
  serve tensor → tile.
* A model with 10⁶ tiles must query in O(log n).

## Implementation Plan

1. Row struct ↔ `TilesetNode` conversion.
2. `put_visual_tile` with both invariant checks.
3. `tile_for_address`, `address_for_tile`, `children_of`, `tiles_at_lod`.
4. Add the `(model_id, tensor_id, lod)` index as a **new numbered migration** —
   never edit a shipped one.
5. Tests including the round trip tensor → tile → tensor.

## Error Handling

* `glb_uri` without `qtile_uri` → refused at write, naming
  `ARCHITECTURE.md` §10.1.
* A child whose geometric error ≥ its parent's → refused, naming the subtree it
  would make unreachable.
* `parent_tile_id` referencing a missing tile → refused.
* `address_for_tile` with an out-of-range `local_index` → error, **never a
  clamped index**, which would silently return the wrong weight.

## Acceptance Criteria

1. A tile row round-trips and survives reopen.
2. `tensor → tile → tensor` returns the original canonical address.
3. `address_for_tile(tile, local_index)` returns the correct logical index for
   every corner of a block.
4. `glb_uri` without `qtile_uri` is refused.
5. A non-refining child is refused.
6. Both lookups are indexed — asserted with `EXPLAIN QUERY PLAN`.
7. Migration is idempotent and a future schema is still refused.

## Verification Plan

**Automated** — catalog tests, including `EXPLAIN QUERY PLAN` assertions on both
lookup paths.
**Manual** — inspect the migration with `sqlite3 .schema`.

## Suggested Commands

```bash
cargo test -p q-catalog                                   # verified today
sqlite3 <catalog.db> "EXPLAIN QUERY PLAN SELECT …"        # introduced here
```

## Test Cases

| Input | Expected |
| --- | --- |
| Insert tile, read back | Identical, incl. bounds |
| Insert, close, reopen | Identical |
| `tile_for_address(addr, LOD 4)` | The tile covering that tensor |
| `address_for_tile(t, 0)` | Block's top-left logical index |
| `address_for_tile(t, 65535)` on 256×256 | Bottom-right index |
| `address_for_tile(t, 65536)` | Error, not a clamp |
| `glb_uri` set, `qtile_uri` null | Refused, citing §10.1 |
| Child error 128, parent 64 | Refused |
| `EXPLAIN QUERY PLAN` on both lookups | Uses an index |

## Risks

| Risk | Mitigation |
| --- | --- |
| Merge conflict with `QM-0020`/`QM-0022` | Strict sequence |
| A clamped index returns a plausible wrong weight | Out-of-range is an error; asserted |
| Bounds diverge from what the tileset emits | One shape, shared with `q_tileset::BoundingBox` |

## Completion Evidence

* Test output including the round trip.
* `EXPLAIN QUERY PLAN` output for both lookups.
* The migration SQL and its idempotency test.
