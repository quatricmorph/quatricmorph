# QM-0044 — `tileset.json` generation

## Status

Blocked

Unblocks when `QM-0042` reaches `Complete`.

## Phase

Phase 04 — Tensor tiles, GLB, and tileset

## Objective

Emit a 3D Tiles 1.1 `tileset.json` describing the pyramid, written **last**, so
a tileset on disk is always complete.

## Repository Evidence

* `crates/q-tileset/src/lib.rs:111` — `UnimplementedTilesetBuilder`;
  `the_builder_refuses_rather_than_emitting_a_fake_tileset`.
* `:30` `TILES_VERSION = "1.1"`; `:34` `ROOT_GEOMETRIC_ERROR = 1024.0`;
  `:60` `TilesetNode`; `:87` `validate_refinement`.
* `geometric_error_halves_down_the_ladder`, `a_child_that_never_refines_is_rejected`
  — `CESIUM-004` Verified.
* `schemas/visualization/schema.json` — `tileset_node` requires strictly positive
  geometric error.
* `ADR-CANDIDATE-008` explicit tiling; `009` 3D Tiles 1.1 + local ENU frame.

## Requirements Covered

`CESIUM-001`, `CESIUM-011`, `MVP-15`.

## Dependencies

`QM-0042`, `QM-0021`.

## Blocks

`QM-0045`, `QM-0046`, `QM-0051`.

## Parallelization

Lane A, after `QM-0042`.

## Program Boundary

`crates/q-tileset`, `crates/q-daemon`.

## Scope

* `ExplicitTilesetBuilder` walking the pyramid plan into a `TilesetNode` tree and
  serializing it.
* `refine: "REPLACE"`; root transform placing the model in a local ENU frame.
* Content URIs pointing at generated GLB and `.qtile` files.
* Serve `GET /v1/visualizations/{modelId}/tileset.json` with
  `Cache-Control: no-cache` and an `ETag` (`ADR-CANDIDATE-019`).
* Preserve the implicit-tiling seam: node fields stay sufficient for it.

## Out of Scope

Implicit tiling · atomic write ordering (`QM-0045`) · external validation
(`QM-0046`) · the viewer (`QM-0051`).

## Files Expected to Change

* `crates/q-tileset/src/lib.rs`
* `crates/q-daemon/src/lib.rs`

## Files Expected to Add

* `crates/q-tileset/src/builder.rs`
* `crates/q-tileset/tests/tileset_generation.rs`

## Files Expected to Remove or Deprecate

* `q_daemon::tileset_501` — replaced. `UnimplementedTilesetBuilder` **stays** for
  the not-yet-converted case and keeps its refusal test.

## Data Contracts

```jsonc
{ "asset": { "version": "1.1" },
  "geometricError": 1024.0,
  "root": { "transform": [ /* local ENU placement, 16 f64 */ ],
            "boundingVolume": { "box": [ /* centre + 3 half-axes */ ] },
            "geometricError": 1024.0, "refine": "REPLACE",
            "content": { "uri": "tiles/<tile_id>.glb" },
            "extras": { "qtile_uri": "tiles/<tile_id>.qtile",
                        "canonical_address": "…", "lod": 0, "fidelity": "aggregate" },
            "children": [ … ] } }
```

`geometricError` values come from the **spatial contract**, not a local constant
— the whole point of `QM-0004`.

## Memory and Performance Constraints

* A 1 000-node tileset serializes in < 100 ms.
* `MAX_TILESET_NODES = 1_000_000`; above it, refuse and name implicit tiling.
* The tree is built in memory; at 10⁶ nodes that is ~200 MB, which is why the
  ceiling exists.

## Implementation Plan

1. Convert `PyramidNode` → `TilesetNode`, bottom-up.
2. Compute the root transform placing the model's bounds at the chosen origin.
3. Set content URIs; put `qtile_uri`, `canonical_address`, and `fidelity` in
   `extras`.
4. Run `validate_refinement()` **before** serializing.
5. Serialize; serve the route with `ETag`.
6. Tests: schema validity, refinement, containment, node counts.

## Error Handling

* `validate_refinement` failing → refuse to emit, naming the offending pair. An
  invalid tileset that loads is worse than none.
* A content URI pointing at a missing file → refuse. This is why the tileset is
  written last.
* Node count above the ceiling → refuse naming implicit tiling.
* No conversion for the model → the daemon returns 404, not an empty tileset.

## Acceptance Criteria

1. A converted tensor produces a `tileset.json` validating against the 3D Tiles
   1.1 schema.
2. Geometric error decreases strictly at every parent/child pair.
3. Every `content.uri` resolves to a file that exists.
4. Every node's `extras.qtile_uri` is present.
5. `refine` is `REPLACE` throughout.
6. Root bounds contain all children.
7. `GET …/tileset.json` returns it with an `ETag`; an unconverted model → 404.
8. 1 000 nodes serialize in < 100 ms.
9. Regenerating produces byte-identical JSON.

## Verification Plan

**Automated** — `tileset_generation.rs`; JSON-Schema validation; URI existence
checks.
**Manual** — open the tileset in an external 3D Tiles inspector.

## Suggested Commands

```bash
cargo test -p q-tileset                                     # verified today
cargo run -p q-cli -- convert … --emit tileset               # introduced here
npx 3d-tiles-validator --tilesetFile out/<model>/tileset.json
curl -sI localhost:PORT/v1/visualizations/<m>/tileset.json
```

## Test Cases

| Input | Expected |
| --- | --- |
| Converted tensor | Valid `tileset.json`, `asset.version 1.1` |
| Every parent/child pair | Error strictly decreases |
| Every `content.uri` | File exists |
| Root bounds | Contain all descendants |
| Unconverted model | 404, not an empty tileset |
| Node count 1 000 001 | Refused naming implicit tiling |
| Regenerate | Byte-identical |
| Deliberate non-refining child | Emission refused, pair named |

## Risks

| Risk | Mitigation |
| --- | --- |
| A tileset references files that do not exist | Written last; URIs checked at emission |
| Geometric errors drift from the Rust constants | Taken from the spatial contract; `QM-0005` guards it |
| Local ENU placement causes precision loss | Measured in `QM-0051`; `ADR-CANDIDATE-009` records the fallback |

## Completion Evidence

* `3d-tiles-validator` output.
* Node counts per LOD.
* URI-existence check output.
* Serialization timing.
* `curl -I` showing the `ETag`.
