# QM-0004 — Shared spatial contract in `schemas/visualization`

## Status

Blocked

## Phase

Phase 00 — Repository baseline and shared contracts

## Objective

Add a `spatial_contract` definition to `schemas/visualization/schema.json`
holding the grid parameters, the LOD ladder, the geometric-error rule, the snap
tolerance, the axis binding, and the instance ceiling — **with exactly today's
values**, so nothing changes behaviourally.

## Repository Evidence

The rules are currently defined in **four** places, none of which knows about the
others:

| Concept | Where |
| --- | --- |
| LOD ladder | `q_tensor_runtime::Lod` (`crates/q-tensor-runtime/src/lib.rs:35`) |
| LOD ladder, again | `apps/web/model-viewer/src/lod-policy.ts:20` — its own enum |
| Geometric error | `q_tileset::GeometricError::for_lod` (`:46`), `ROOT_GEOMETRIC_ERROR = 1024.0` (`:34`) |
| Geometric error, again | `lod-policy.ts:102` — `1024 / 2 ** lod`, comment *"mirrors `q_tileset::GeometricError`"* |
| Distance thresholds | `lod-policy.ts:51` — `[4096, 1024, 256, 64, 16]` |
| Grid parameters | `quatricmorph-workspace/src/layout/grid-ruler.ts:63` — `DEFAULT_GRID_RULER` |
| Snap tolerance | `grid-ruler.ts:280` — `GRID_SNAP_TOLERANCE = 1e-6` |
| Instance ceiling | `q_gltf::MAX_INSTANCES_PER_TILE = 262_144` (`crates/q-gltf/src/lib.rs:77`) |

`schemas/visualization/schema.json` already exists (119 lines) and states its own
purpose: *"so the daemon, the catalog, and the viewer cannot drift while the
builders are written."* It does not yet carry any of the above.

## Requirements Covered

`GRID-006`, `SCHEMA-001`.

## Dependencies

`QM-0001`, `QM-0002`.

## Blocks

`QM-0005`, and through it every Phase 04, 05, and 06 task.

## Parallelization

**Runs alone.** It is a shared-schema change that every later lane consumes.

## Program Boundary

`schemas/visualization/schema.json`. No code changes in this task — consumption
is `QM-0005` and `QM-0060`.

## Scope

* Add the `spatial_contract` definition per
  [`SCHEMA_PLAN.md`](../../SCHEMA_PLAN.md) §2.1.
* Add `schemas/visualization/spatial-contract.json` — the **instance** holding
  the actual values.
* Add `schemas/visualization/golden-spatial.json` — the conformance vector.
* Values are **transcribed exactly** from the sources above.

## Out of Scope

Changing any value · changing any code · the conformance tests (`QM-0005`) · the
web core package (`QM-0060`).

## Files Expected to Change

* `schemas/visualization/schema.json`

## Files Expected to Add

* `schemas/visualization/spatial-contract.json`
* `schemas/visualization/golden-spatial.json`

## Files Expected to Remove or Deprecate

None yet. Duplicated constants are removed by `QM-0060`, after their consumers
exist.

## Data Contracts

```jsonc
// spatial-contract.json
{ "version": 1,
  "grid": { "cellSize": 1, "minorGridSpacing": 1, "majorGridInterval": 5,
            "tensorPadding": 1, "labelMargin": 1, "framePadding": 1,
            "operandGap": 4, "axisMargin": 1, "depthSpacing": 0,
            "origin": [0, 0, 0] },
  "lod_ladder": [
    { "level": 0, "name": "model",     "carries_exact_values": false, "distance_threshold": 4096 },
    { "level": 1, "name": "subsystem", "carries_exact_values": false, "distance_threshold": 1024 },
    { "level": 2, "name": "layer",     "carries_exact_values": false, "distance_threshold": 256 },
    { "level": 3, "name": "tensor",    "carries_exact_values": false, "distance_threshold": 64 },
    { "level": 4, "name": "block",     "carries_exact_values": false, "distance_threshold": 16 },
    { "level": 5, "name": "region",    "carries_exact_values": true,  "distance_threshold": null }
  ],
  "geometric_error": { "root": 1024.0, "falloff": "halving" },
  "snap_tolerance": 1e-6,
  "axis_binding": { "world_axes": { "X": "J", "Y": "I", "Z": "K" },
                    "max_implemented_rank": 3 },
  "instance_ceiling": 262144 }
```

`instance_ceiling` is here because **one number governs two subsystems** —
`q_gltf::MAX_INSTANCES_PER_TILE` and the workspace's `MAX_WORKSPACE_SPHERES`.
Defining it twice is how they drift apart.

`axis_binding.world_axes` records the mapping the code implements, accepted by
[`ADR-009`](../../../docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md).
`axis_binding.max_implemented_rank` is the ceiling accepted by
[`ADR-010`](../../../docs/decisions/ADR-010-tensor-rank-ceiling.md); rank above it
refuses rather than flattening.

Both values are **decisions, not transcriptions** — unlike every other field
here, they are not merely copied from an existing constant. `QM-0005` freezes
them into a cross-language golden vector at gate G1, which is why they were
promoted to real ADRs before this task became reachable.

## Memory and Performance Constraints

None. A few kilobytes of JSON.

## Implementation Plan

1. Read each source constant and transcribe it. **Do not round, adjust, or
   improve any value.**
2. Add the `spatial_contract` definition to the schema, with descriptions naming
   the Rust and TypeScript symbols each field governs.
3. Write the instance file.
4. Write the golden vector: geometric error per LOD, LOD per distance, cell
   centres, and the load-decision table.
5. Validate the instance against the schema with any JSON-Schema validator.

## Error Handling

Not applicable — this task adds data, not behaviour. The failure mode it guards
against is **transcription error**, caught by `QM-0005`'s conformance tests,
which is why those tests must pass *without* any constant changing.

## Acceptance Criteria

1. `schemas/visualization/schema.json` defines `spatial_contract` with all seven
   required properties.
2. `spatial-contract.json` validates against it.
3. Every value **exactly** matches its current source, verified by inspection
   against the eight evidence rows above.
4. `golden-spatial.json` contains ≥ 6 geometric-error cases, ≥ 3 LOD-for-distance
   cases, ≥ 4 cell-centre cases, and ≥ 3 load-decision cases.
5. Both test suites still pass at 290 + 101 — **nothing changed behaviourally**.

## Verification Plan

**Automated** — JSON-Schema validation in CI; `scripts/verify-baseline.sh`.
**Manual** — a reviewer diffs each of the eight constants against its source.

## Suggested Commands

Introduced by this task:

```bash
npx ajv-cli validate -s schemas/visualization/schema.json \
  -d schemas/visualization/spatial-contract.json
```

## Test Cases

| Input | Expected |
| --- | --- |
| `spatial-contract.json` vs schema | Valid |
| `grid.cellSize` | `1`, matching `DEFAULT_GRID_RULER` |
| `geometric_error.root` | `1024.0`, matching `ROOT_GEOMETRIC_ERROR` |
| `lod_ladder` length | Exactly 6, matching `Lod::ALL` |
| `lod_ladder[5].carries_exact_values` | `true`; every other level `false` |
| `instance_ceiling` | `262144`, matching `MAX_INSTANCES_PER_TILE` |
| Both suites after the change | 290 + 101, unchanged |

## Risks

| Risk | Mitigation |
| --- | --- |
| A transcription error introduces a silent behaviour change later | `QM-0005` asserts every constant against the schema; a mismatch is a red test |
| The contract is added and then bypassed | R2. `QM-0060` removes the duplicates; review rule forbids new spatial literals |

## Completion Evidence

* The full contents of `spatial-contract.json`.
* Schema validation output.
* A side-by-side diff of each of the eight constants against its source.
* Both suites at 290 + 101 after the change.
