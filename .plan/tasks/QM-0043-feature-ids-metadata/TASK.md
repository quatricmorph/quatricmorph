# QM-0043 — Feature IDs and structural metadata

## Status

Blocked

Unblocks when `QM-0042` reaches `Complete`.

## Phase

Phase 04 — Tensor tiles, GLB, and tileset

## Objective

Attach `EXT_mesh_features` and `EXT_structural_metadata` so a picked instance
resolves to a tensor address **without a network round trip**.

## Repository Evidence

* `QM-0042` emits `_FEATURE_ID_0` sequentially, matching `.qtile` cell order.
* `ARCHITECTURE.md` §10.2 — a tile may contain feature IDs and tile-local
  metadata; it warns support is uneven.
* `q_tensor_runtime::TileId::to_hex()`; `BlockExtent` carries row/column starts.
* `QM-0021` provides `address_for_tile(tile_id, local_index)` as the fallback
  path when metadata is unavailable.
* Picking budget: < 50 ms ([`PERFORMANCE_PLAN.md`](../../PERFORMANCE_PLAN.md) §2.5).

## Requirements Covered

`GLB-004`; enables `CESIUM-007`, `AC-004`.

## Dependencies

`QM-0042`.

## Blocks

`QM-0053`, `QM-0046`.

## Parallelization

Lane A, after `QM-0042`. Same crate — sequential.

## Program Boundary

`crates/q-gltf`.

## Scope

* `EXT_mesh_features`: one feature ID set over instances.
* `EXT_structural_metadata`: a property table with per-instance `row`, `column`,
  `value_class`, `sign`, and tile-level `tensor_id`, `canonical_address`,
  `block_extent`, `fidelity`.
* Stability: regenerating the same block yields the **same** feature IDs.

## Out of Scope

The capability probe (`QM-0057`) · the viewer's picking path (`QM-0053`) ·
per-instance exact values, which belong in the `.qtile`.

## Files Expected to Change

* `crates/q-gltf/src/instanced.rs`
* `crates/q-gltf/tests/glb_emission.rs`

## Files Expected to Add

* `crates/q-gltf/src/metadata.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

Per-instance: `row: u16`, `column: u16` (block-local), `value_class: u8`,
`sign: i8`. Tile-level: `tensor_id`, `canonical_address`, `block_extent`,
`lod`, `fidelity`.

**Per-instance stores block-local coordinates, not global ones.** Global indices
would need `u32` each and are derivable from `block_extent` — 4 bytes per
instance saved, and one source of truth for the offset.

**No exact value is stored in the GLB.** `value_class` is the quantized visual
class; the value lives in the `.qtile` (`GLB-003`).

## Memory and Performance Constraints

Metadata adds 6 B/instance → ≈ 0.4 MB at 65 536, taking a tile from ~1.8 MB to
~2.2 MB. Acceptable; it removes a round trip from every pick.

## Implementation Plan

1. Emit the `EXT_mesh_features` feature-ID set over the instance attribute.
2. Build the `EXT_structural_metadata` property table with the four per-instance
   properties.
3. Add the tile-level class with its five properties.
4. Assert feature ID *k* ↔ `.qtile` cell *k* ↔ `(row, column)`.
5. Regeneration-stability test.

## Error Handling

* A property table whose length disagrees with the instance count → refuse.
* A `value_class` outside its range → refuse.
* Missing tile-level metadata → refuse; a tile that cannot say which tensor it
  belongs to is unusable for picking.

## Acceptance Criteria

1. Feature ID *k* maps to `.qtile` cell *k* and to the correct `(row, column)`.
2. Tile-level metadata carries all five properties.
3. Regenerating produces identical feature IDs.
4. Global index = `block_extent.row_start + row`, asserted for all four corners.
5. No exact value appears anywhere in the GLB.
6. Tile size stays under 2.5 MB at 65 536 instances.
7. Profile B and C tiles omit the extensions but keep the `_FEATURE_ID_0`
   attribute or vertex equivalent.

## Verification Plan

**Automated** — emission tests plus a JSON inspection of the glTF chunk.
**Manual** — inspect the metadata in an external glTF inspector.

## Suggested Commands

```bash
cargo test -p q-gltf                                        # verified today
python3 -c "import json,struct,sys; ..."                     # dump the glTF JSON chunk
```

## Test Cases

| Input | Expected |
| --- | --- |
| Feature ID 0 | `(row 0, column 0)` |
| Feature ID 65535 on 256×256 | `(row 255, column 255)` |
| Global index for corner instances | Matches `block_extent` arithmetic |
| Regenerate a tile | Identical feature IDs |
| Search the GLB for f32 weight values | **None present** |
| Property table length ≠ instance count | Refused |
| Profile C tile | No extensions; feature IDs still present |

## Risks

| Risk | Mitigation |
| --- | --- |
| Metadata bloats the tile | Block-local `u16` coordinates; size asserted |
| Exact values leak into the GLB | An explicit test searches for them |
| Extensions unsupported | Profiles B and C keep picking working via the daemon fallback |

## Completion Evidence

* Feature-ID ↔ cell ↔ coordinate mapping test output.
* A glTF JSON chunk dump showing the property table.
* Tile size before and after metadata.
* The regeneration-stability comparison.
