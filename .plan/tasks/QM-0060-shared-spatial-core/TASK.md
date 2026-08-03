# QM-0060 — Shared spatial core package

## Status

Blocked

Unblocks when `QM-0005` reaches `Complete`.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Create `apps/web/core` and make **one** definition of the grid, the LOD ladder,
canonical addresses, and fidelity serve both web applications.

## Repository Evidence

* `apps/web/matrix-workspace/src/layout/grid-ruler.ts:63` — `DEFAULT_GRID_RULER`,
  ten parameters; `:280` `GRID_SNAP_TOLERANCE = 1e-6`; 13 tests.
* `apps/web/model-viewer/src/lod-policy.ts:20,51,102` — its own `Lod` enum,
  distance thresholds, and a hand-mirrored geometric-error formula.
* `apps/web/package.json` — npm workspaces already configured for three packages.
* `matrix-workspace` depends on `three` and `lil-gui`; `model-viewer` on neither.
* `grid-ruler.ts:346-355` already demonstrates the back-compatible alias pattern.
* `ADR-CANDIDATE-007` — a new package, not a cross-dependency.

## Requirements Covered

`GRID-006`, `MVP-26`.

## Dependencies

`QM-0005`.

## Blocks

`QM-0052`, `QM-0061`…`QM-0068`.

## Parallelization

**Runs alone.** Every Phase 06 task imports from it, and `QM-0052` depends on it.

## Program Boundary

New package `apps/web/core`; imports updated in both consumers.

## Scope

* Create the package with `spatial/`, `lod/`, `address/`, `fidelity/`.
* Import `schemas/visualization/spatial-contract.json` at build time and
  re-export typed constants — **the only definition in TypeScript**.
* Move `grid-ruler.ts` into `spatial/grid.ts`; re-export from the old path.
* Move the LOD constants out of `lod-policy.ts`; keep its functions and tests.
* Move `Fidelity` out of `block-adapter.ts`; re-export.
* Zero runtime dependencies — no `three`, no `cesium`.

## Out of Scope

Axis binding (`QM-0061`) · grid rendering (`QM-0062`) · behaviour changes of any
kind.

## Files Expected to Change

* `apps/web/package.json` — add `core` to `workspaces`
* `apps/web/matrix-workspace/package.json`, `model-viewer/package.json` — depend
  on it
* `apps/web/matrix-workspace/src/layout/grid-ruler.ts` → re-export shim
* `apps/web/model-viewer/src/lod-policy.ts` → import constants
* `apps/web/matrix-workspace/src/tensor/block-adapter.ts` → import `Fidelity`

## Files Expected to Add

* `apps/web/core/package.json`, `tsconfig.json`
* `apps/web/core/src/spatial/grid.ts`
* `apps/web/core/src/lod/ladder.ts`
* `apps/web/core/src/address/canonical.ts`
* `apps/web/core/src/fidelity/exactness.ts`
* `apps/web/core/src/__tests__/contract.test.ts`

## Files Expected to Remove or Deprecate

Local constant **declarations** in `lod-policy.ts` and `grid-ruler.ts`. The files
stay as re-export shims so existing imports keep working, exactly as the module
already does for `GridRuledLinesConfig` and `MarginGridConfig`.

## Data Contracts

Everything derives from `schemas/visualization/spatial-contract.json`. A constant
declared anywhere else in `apps/web/` is a **review failure**, and `QM-0005`'s
conformance test is what catches it in practice.

## Memory and Performance Constraints

`core` has no runtime dependencies, so it adds a few kilobytes to each bundle.
Importing JSON at build time means no runtime fetch.

## Implementation Plan

1. Create the package; add it to the workspaces array.
2. Move `grid-ruler.ts` content to `spatial/grid.ts`, replacing
   `DEFAULT_GRID_RULER`'s literals with values read from the contract.
3. Move the LOD constants to `lod/ladder.ts`; `lod-policy.ts` keeps
   `lodForDistance` and `decideLoad` but imports its constants.
4. Add `address/canonical.ts` — parse and format for canonical and alias forms.
5. Move `Fidelity` to `fidelity/exactness.ts`.
6. Turn the old paths into re-export shims.
7. **Run both suites; they must pass unchanged at 101+.**

## Error Handling

Not applicable — a pure refactor. The failure mode is an import break, caught by
`tsc --noEmit` and by both suites.

## Acceptance Criteria

1. `apps/web/core` exists with the four modules and no runtime dependencies.
2. Both apps depend on it.
3. **No spatial constant is declared outside `core`** — verified by grep.
4. Old import paths still work through re-export shims.
5. All 13 `grid-ruler` tests and all 10 `lod-policy` tests pass unchanged.
6. Both suites total 101+ with no failures.
7. `tsc --noEmit` clean in all four packages.
8. `QM-0005`'s conformance tests still pass.
9. Bundle size change measured and recorded.

## Verification Plan

**Automated** — both suites; `tsc --noEmit`; a grep-based check that no spatial
literal exists outside `core`.
**Manual** — bundle-size comparison before and after.

## Suggested Commands

```bash
cd apps/web && npx vitest run                              # verified today
npx tsc --noEmit -p matrix-workspace && npx tsc --noEmit -p model-viewer
npm run build --workspace matrix-workspace
grep -rn "1024 / 2 \*\*\|cellSize: 1" --include=*.ts . | grep -v core/    # should be empty
```

## Test Cases

| Input | Expected |
| --- | --- |
| Import `GridRuler3D` from the old path | Works via the shim |
| Import from `@quatricmorph/core` | Works |
| Grep for spatial literals outside `core` | **No matches** |
| All existing web tests | Pass unchanged |
| `tsc --noEmit` × 4 packages | Clean |
| `DEFAULT_GRID_RULER.cellSize` | `1`, from the contract |
| `core`'s `package.json` dependencies | Empty |

## Risks

| Risk | Mitigation |
| --- | --- |
| The move breaks imports subtly | Re-export shims; `tsc --noEmit`; both suites |
| `core` accumulates unrelated code | A stated scope: spatial, LOD, address, fidelity. Nothing that imports a renderer |
| A later task declares a new local constant | Grep check in CI; `QM-0005` conformance |

## Completion Evidence

* Both suites passing at ≥ 101.
* `tsc --noEmit` output for all four packages.
* The grep check returning no matches.
* Bundle sizes before and after.
