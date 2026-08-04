# QM-0046 — External artifact validation in CI

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 04 — Tensor tiles, GLB, and tileset

## Objective

Validate generated artifacts with **external** validators, so "valid GLB" stops
meaning "valid according to the code that wrote it".

## Repository Evidence

* `.qtile` round trips are already Verified (`TILE-005`…`TILE-008`) — but a round
  trip only tests our own reader against our own writer.
* `q_tileset::TilesetNode::validate_refinement` — our own check.
* `schemas/visualization/schema.json` — our own schema.
* `.github/workflows/build.yaml` — three jobs; **no artifact validation**.
* `ARCHITECTURE.md` §10.2 warns that renderer support must be checked rather than
  assumed — the same logic applies to file validity.

## Requirements Covered

`TILE-012`, `MVP-14`, `MVP-15`.

## Dependencies

`QM-0045`, `QM-0041`, `QM-0042`, `QM-0043`, `QM-0044`.

## Blocks

**Every Phase 05 task that needs real data.** This is gate G2.

## Parallelization

Lane A, last Phase 04 task. Runs alone.

## Program Boundary

CI, plus a `q-cli validate` subcommand.

## Scope

* CI job: generate from `QM-0003`'s fixture, then validate with:
  * Khronos `gltf-validator` on every GLB;
  * `3d-tiles-validator` on `tileset.json`;
  * our JSON Schema on the tileset and on `visual_tiles` rows;
  * our `.qtile` decoder on every tile.
* `q-cli validate <dir>` running all four locally.
* Fail on **any** validator error; warnings are reported and reviewed.

## Out of Scope

Rendering validation (`QM-0051`) · performance · fixing generator bugs found —
those become their own tasks unless trivial.

## Files Expected to Change

* `.github/workflows/build.yaml`
* `crates/q-cli/src/main.rs`

## Files Expected to Add

* `scripts/validate-artifacts.sh`
* `crates/q-cli/src/validate.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

Validation report:

```jsonc
{ "qtile":   { "files": 256, "valid": 256, "errors": [] },
  "glb":     { "files": 256, "valid": 256, "errors": [], "warnings": [] },
  "tileset": { "valid": true, "errors": [], "node_count": 257 },
  "schema":  { "valid": true },
  "cross_reference": { "missing_content_uris": 0, "missing_qtile_sidecars": 0 } }
```

`cross_reference` is ours and covers what neither external validator can: that
every `content.uri` exists and every GLB has its `.qtile` sidecar.

## Memory and Performance Constraints

Validating 256 GLB files must complete in under 5 minutes in CI, or the job will
be skipped in practice. Validators run in parallel where possible.

## Implementation Plan

1. Add `gltf-validator` and `3d-tiles-validator` as CI-only npm devDependencies.
2. Write `scripts/validate-artifacts.sh` running all four checks and emitting the
   report.
3. Add `q-cli validate` for local use.
4. Add the CI job: generate the large fixture → convert → validate.
5. Fail on any error; surface warnings in the job summary.

## Error Handling

* A validator not installed → **fail the job**, never skip. A skipped validation
  that reports success is the failure mode this task exists to prevent.
* A validator crash → fail, capturing its output.
* A validation error → fail, naming the file and the error.
* A warning → reported, not fatal, but recorded in the job summary so it is seen.

## Acceptance Criteria

1. `gltf-validator` reports **zero errors** on all 256 GLB files.
2. `3d-tiles-validator` reports zero errors on `tileset.json`.
3. The tileset validates against `schemas/visualization/schema.json`.
4. Every `.qtile` decodes.
5. Every `content.uri` resolves; every GLB has a `.qtile` sidecar.
6. A deliberately corrupted GLB makes the job fail.
7. A missing validator makes the job fail, not skip.
8. The job completes in under 5 minutes.
9. Any warning is visible in the CI summary.

## Verification Plan

**Automated** — the CI job, end to end from generation.
**Manual** — inspect the report; review warnings; run `q-cli validate` locally.

## Suggested Commands

```bash
npx gltf-validator out/<model>/tiles/<tile>.glb                  # introduced here
npx 3d-tiles-validator --tilesetFile out/<model>/tileset.json
./scripts/validate-artifacts.sh out/<model>
cargo run -p q-cli -- validate out/<model>
```

## Test Cases

| Input | Expected |
| --- | --- |
| Freshly generated artifacts | All four checks pass |
| One GLB byte-corrupted | `gltf-validator` errors; job fails |
| `tileset.json` with a bad `content.uri` | Cross-reference check fails |
| A GLB with `qtile_uri` removed | Sidecar check fails |
| `gltf-validator` uninstalled | **Job fails**, not skipped |
| A validator warning | Reported; job passes; warning in the summary |
| 256 GLB files | Validated in under 5 minutes |

## Risks

| Risk | Mitigation |
| --- | --- |
| Validators are strict about things we consider fine | Warnings are non-fatal but reviewed; errors are never suppressed |
| The job is slow and gets disabled | Parallel validation; a 5-minute budget as an acceptance criterion |
| A missing validator silently skips | Explicit failure; asserted as a test case |

## Completion Evidence

* Full validation report for a generated model.
* CI job URL and duration.
* The deliberate-corruption failure output.
* The missing-validator failure output.
