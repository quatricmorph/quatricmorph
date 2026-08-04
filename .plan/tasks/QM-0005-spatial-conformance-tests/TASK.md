# QM-0005 — Cross-language spatial conformance tests

## Status

Blocked

Unblocks when `QM-0004` completes reaches `Complete`.

## Phase

Phase 00 — Repository baseline and shared contracts. **Integration gate G1.**

## Objective

Make spatial drift between Rust and TypeScript a red test rather than a silent
divergence.

## Repository Evidence

* `apps/web/model-viewer/src/lod-policy.ts:102` — `1024 / 2 ** lod` with the
  comment *"mirrors `q_tileset::GeometricError`"*. **Hand-mirrored by a human,
  with no mechanism to detect drift.**
* `q_tileset::GeometricError::for_lod` — `ROOT_GEOMETRIC_ERROR / 2^lod`.
* `q_tensor_runtime::Lod::ALL` — six variants.
* `apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts` — `DEFAULT_GRID_RULER`,
  `GRID_SNAP_TOLERANCE = 1e-6`, `cellCenterLocal`.
* `lod-policy.ts` tests `never_reads_exact_values_from_camera_movement_alone` and
  `reads_exact_values_only_on_an_explicit_selection` — the mechanised form of
  `AC-006`, currently asserted in one language only.

## Requirements Covered

`GRID-011`, `SCHEMA-002`; guards `GRID-006`, `AC-006`.

## Dependencies

`QM-0004`.

## Blocks

**Every Phase 04, 05, and 06 task.** This is gate G1.

## Parallelization

Runs alone. Touches both languages.

## Program Boundary

`crates/q-tileset`, `crates/q-tensor-runtime` (tests only);
`apps/web/*/src/__tests__` (tests only). No behaviour changes.

## Scope

* A Rust test loading `schemas/visualization/spatial-contract.json` and asserting
  every Rust constant equals its schema value.
* A vitest test doing the same for every TypeScript constant.
* Both languages assert against `golden-spatial.json`.
* A test that an unknown `spatial_contract.version` is **refused**, not guessed.
* A CI job running both.

## Out of Scope

Moving constants into `apps/web/core` (that is `QM-0060`) · changing any value ·
generating code from the schema.

## Files Expected to Change

* `.github/workflows/build.yaml` — a `contract` job.

## Files Expected to Add

* `crates/q-tileset/tests/spatial_conformance.rs`
* `apps/web/model-viewer/src/__tests__/spatial-conformance.test.ts`
* `apps/web/quatricmorph-workspace/src/__tests__/spatial-conformance.test.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

`golden-spatial.json`, asserted identically by both languages:

```jsonc
{ "geometric_error":  [ { "lod": 0, "expected": 1024.0 }, { "lod": 3, "expected": 128.0 },
                        { "lod": 5, "expected": 32.0 } ],
  "lod_for_distance": [ { "distance": 5000, "expected_lod": 0 },
                        { "distance": 100,  "expected_lod": 3 },
                        { "distance": 8,    "expected_lod": 5 } ],
  "cell_center":      [ { "i": 0,   "j": 0,   "cellSize": 1, "tensorPadding": 1, "expected": [1, 1, 0] },
                        { "i": 255, "j": 128, "cellSize": 1, "tensorPadding": 1, "expected": [129, 256, 0] } ],
  "load_decision":    [ { "distance": 5000, "interaction": "navigating", "reads_exact": false },
                        { "distance": 8,    "interaction": "hovering",   "reads_exact": false },
                        { "distance": 8,    "interaction": "selected",   "reads_exact": true } ] }
```

The `load_decision` block is `AC-006` mechanised, now in both languages against
one table.

## Memory and Performance Constraints

Both suites must stay fast — the additions are pure table-driven assertions.

## Implementation Plan

1. Rust: `include_str!` the contract and the golden vector, parse with
   `serde_json`, assert `ROOT_GEOMETRIC_ERROR`, `Lod::ALL.len()`,
   `carries_exact_values` per level, and `GeometricError::for_lod` against every
   golden row.
2. TypeScript: `import` both JSON files, assert `geometricErrorForLod`,
   `LOD_DISTANCE_THRESHOLDS`, `lodForDistance`, `decideLoad`,
   `DEFAULT_GRID_RULER`, `GRID_SNAP_TOLERANCE`, and `cellCenterLocal`.
3. Assert `MAX_INSTANCES_PER_TILE == instance_ceiling` in Rust.
4. Add a version-refusal test in both languages.
5. Add the CI job.

## Error Handling

* A constant mismatch fails with **both values and the field name** — a message
  that says only "assertion failed" wastes the mechanism.
* A missing or malformed contract file fails loudly; it is never defaulted.
* An unknown version **refuses**, matching `.qtile` and catalog discipline.

## Acceptance Criteria

1. Rust conformance test passes; deliberately changing `ROOT_GEOMETRIC_ERROR`
   makes it fail with both values named.
2. Both TypeScript conformance tests pass; deliberately changing
   `geometricErrorForLod` makes one fail.
3. Every golden row is asserted in **both** languages.
4. `MAX_INSTANCES_PER_TILE` is asserted against `instance_ceiling`.
5. A bumped `version` fails both suites until they are updated.
6. Total counts: 290 + N Rust, 101 + M web, both increased.
7. The CI `contract` job runs and passes.

## Verification Plan

**Automated** — the three new test files, in CI.
**Manual** — the deliberate-drift demonstration in each language, with output
captured.

## Suggested Commands

Introduced by this task:

```bash
cargo test -p q-tileset --test spatial_conformance
cd apps/web && npx vitest run spatial-conformance
```

## Test Cases

| Input | Expected |
| --- | --- |
| `for_lod(Lod::Model)` vs golden | `1024.0` |
| `geometricErrorForLod(3)` vs golden | `128.0` |
| `lodForDistance(100)` | LOD 3, both languages |
| `decideLoad(d=8, 'hovering')` | `reads_exact: false`, both languages |
| `decideLoad(d=8, 'selected')` | `reads_exact: true`, both languages |
| `cellCenterLocal(255, 128, …)` | `[129, 256, 0]` |
| `ROOT_GEOMETRIC_ERROR` set to `512.0` | Rust suite fails naming `1024.0` vs `512.0` |
| `version: 2` in the contract | Both suites refuse |

## Risks

| Risk | Mitigation |
| --- | --- |
| The tests assert the schema against itself | Assert **code constants** against the schema, never schema against schema |
| A future task adds a spatial constant without a conformance row | Review checklist; `QM-0060` removes the remaining duplicates so there is one place to add |

## Completion Evidence

* Output of all three test files passing.
* Deliberate-drift output from each language, showing the failure message.
* CI run of the `contract` job.
* Updated test counts.
