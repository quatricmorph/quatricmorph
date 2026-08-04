# QM-0053 — Feature pick → canonical tensor address

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 05 — Cesium model viewer

## Objective

Make a click resolve to the correct canonical tensor address — **exactly**, not
approximately.

## Repository Evidence

* `QM-0043` emits `EXT_mesh_features` and `EXT_structural_metadata` with
  block-local `(row, column)` per instance and tile-level `tensor_id`,
  `canonical_address`, `block_extent`.
* `QM-0021` provides `address_for_tile(tile_id, local_index)` as the fallback for
  profiles B and C.
* `q_tensor_runtime::TileId` — extent- and LOD-sensitive (`TILE-003`).
* Positions are **derived, never stored**
  ([`GRID_ARCHITECTURE.md`](../../GRID_ARCHITECTURE.md) §2.3), so the inverse is
  exact.
* `AC-004` is `Partial`: addressing is Verified; there is no viewer to click.
* Budget: pick → address < 50 ms.

## Requirements Covered

`CESIUM-007`, `AC-004`, `MVP-21`, `MVP-22`.

## Dependencies

`QM-0052`, `QM-0043`, `QM-0021`.

## Blocks

`QM-0054`, `QM-0080`.

## Parallelization

Lane B, sequential after `QM-0052`. **This is gate G3.**

## Program Boundary

`apps/web/model-viewer`.

## Scope

* `scene.pick` → `Cesium3DTileFeature` → feature ID.
* Resolve via in-tile metadata (profile A) or a daemon lookup (B/C).
* Compose the global index: `block_extent.row_start + row`, likewise for column.
* Emit a selection event carrying the full address, and fetch the exact value
  **only on selection**.
* Selection presentation using **at least two** non-colour channels.

## Out of Scope

The inspector panel (`QM-0054`) · opening the workspace (`QM-0066`) · hover
metadata detail (`QM-0068`).

## Files Expected to Change

* `apps/web/model-viewer/src/index.ts`

## Files Expected to Add

* `apps/web/model-viewer/src/cesium/picking.ts`
* `apps/web/model-viewer/src/state/selection.ts`
* `apps/web/model-viewer/src/__tests__/picking.test.ts`
* `apps/web/model-viewer/e2e/pick-address.spec.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
{ "tile_id": "…", "feature_id": 1031,
  "tensor_id": "…",
  "canonical_address": "model.layers[10].self_attention.query_projection.weight",
  "block_extent": { "row_start": 1024, "column_start": 1792, … },
  "local": { "row": 7, "column": 10 },
  "logical_index": [1031, 1802],
  "lod": 4, "fidelity": "quantized",
  "resolution_path": "in-tile" | "daemon-lookup" }
```

`fidelity: "quantized"` at pick time is **correct and important**: the tile's
value is quantized. Only after an explicit exact fetch does it become `exact`.

## Memory and Performance Constraints

* Pick → address < 50 ms, including a daemon lookup in profile B/C.
* No exact value is fetched on hover.
* Selection state is a single object, not per-instance data.

## Implementation Plan

1. `pickFeature(x, y)` returning the feature and its tile.
2. Profile A: read `EXT_structural_metadata` directly.
3. Profiles B/C: `GET /v1/visualizations/{m}/tiles/{t}` metadata, cached.
4. Compose the global index from `block_extent` + local coordinates.
5. Emit the selection; fetch the exact value **only** for a selection.
6. Apply the two-channel highlight: outline + scale bump.
7. Playwright: click a known cell; assert the address equals `q-cli value`'s.

## Error Handling

* Pick returning nothing → clear the selection; not an error.
* Missing in-tile metadata → fall back to the daemon lookup; log the profile.
* A daemon lookup failing → show the tile ID and feature ID and say the address
  is unresolved. **Never guess an address.**
* A feature ID outside the instance count → error; never clamped.

## Acceptance Criteria

1. Clicking a cell yields the correct canonical address and logical index.
2. The address **equals** what `q-cli value` reports for the same index —
   asserted across all four corners of a block and the centre.
3. Pick → address < 50 ms.
4. Hover fetches no exact value; selection fetches exactly one.
5. Profile B/C fall back to the daemon and still resolve correctly.
6. An unresolvable feature reports so; no guessed address.
7. Selection is legible in a **greyscale** screenshot.
8. Selecting a different tile replaces the selection cleanly.

## Verification Plan

**Automated** — vitest for index composition; Playwright for click → address,
compared against a value fetched through the API.
**Manual** — click cells at known indices; compare with `q-cli value`.

## Suggested Commands

```bash
cargo run -p q-cli -- value fixtures/tiny-llama-large <addr> --index 1031,1802  # today
npx playwright test apps/web/model-viewer/e2e/pick-address.spec.ts               # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| Click block `(4,7)` local `(0,0)` | `[1024, 1792]` |
| Click local `(255,255)` | `[1279, 2047]` |
| Click all four corners + centre | All match `q-cli value` |
| Pick timing | < 50 ms |
| Hover | No exact request |
| Select | Exactly one exact request |
| Profile C tile | Daemon fallback resolves correctly |
| Feature ID 999999 | Error, not a clamp |
| Greyscale screenshot | Selection still visible |

## Risks

| Risk | Mitigation |
| --- | --- |
| Off-by-one in index composition | All four corners plus centre are test cases |
| A guessed address on metadata failure | Explicitly unresolved instead; asserted |
| Selection conveyed by colour alone | Two channels; greyscale screenshot is an acceptance criterion |

## Completion Evidence

* Address comparison table: clicked cell versus `q-cli value`, five positions.
* Pick timing measurements.
* Request log showing hover versus selection behaviour.
* Greyscale screenshot with a visible selection.
