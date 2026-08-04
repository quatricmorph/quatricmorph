# QM-0052 — Camera-driven LOD wired to the shared policy

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 05 — Cesium model viewer

## Objective

Make Cesium's refinement follow the shared LOD policy, and **prove that camera
movement alone never reads an exact value**.

## Repository Evidence

* `apps/web/model-viewer/src/lod-policy.ts` —
  `never_reads_exact_values_from_camera_movement_alone` and
  `reads_exact_values_only_on_an_explicit_selection` (`CESIUM-002` Verified).
  `LOD_DISTANCE_THRESHOLDS = [4096, 1024, 256, 64, 16]`; `decideLoad(camera,
  interaction)` where interaction ∈ `idle | navigating | hovering | selected`.
* `QM-0004`/`QM-0060` move these constants into the shared contract.
* `q_tensor_runtime::Lod::carries_exact_values()` — true **only** at level 5.
* `ARCHITECTURE.md` §13.3 — prefetch children and sibling metadata; **do not
  fetch exact values**.

## Requirements Covered

`CESIUM-002`, `AC-006`, `AC-007`, `MVP-19`, `MVP-20`, `PERF-002`.

## Dependencies

`QM-0051`, `QM-0060`.

## Blocks

`QM-0053`, `QM-0080`.

## Parallelization

Lane B, sequential after `QM-0051`.

## Program Boundary

`apps/web/model-viewer`.

## Scope

* Drive `maximumScreenSpaceError` from the shared geometric-error rule.
* Map camera distance → expected LOD via `lodForDistance`.
* Prefetch: current tile → children → sibling metadata; **stop there**.
* An assertion harness recording every request URL during a navigation session,
  so "no exact reads" is measured rather than asserted.

## Out of Scope

Picking (`QM-0053`) · the inspector (`QM-0054`) · prefetch tuning beyond the
documented policy.

## Files Expected to Change

* `apps/web/model-viewer/src/lod-policy.ts` — import constants from
  `apps/web/core`
* `apps/web/model-viewer/src/cesium/tileset.ts`

## Files Expected to Add

* `apps/web/model-viewer/src/cesium/lod.ts`
* `apps/web/model-viewer/e2e/lod-navigation.spec.ts`

## Files Expected to Remove or Deprecate

* The local `enum Lod` and `geometricErrorForLod` in `lod-policy.ts` — replaced
  by imports from `apps/web/core`. **The tests move with them, they are not
  deleted.**

## Data Contracts

Request log entry: `{ url, method, status, bytes, timestamp, route_class }`
where `route_class ∈ tileset | glb | qtile | value | blocks | statistics`.

**`route_class ∈ {value, blocks}` during navigation is a test failure.** That is
`AC-006` made observable.

## Memory and Performance Constraints

* Frame time < 16 ms with 256 loaded tiles.
* `MAX_LOADED_TILES = 256`; Cesium's own `cacheBytes` set from the memory budget.
* Prefetch must not exceed the loaded-tile ceiling.

## Implementation Plan

1. Replace the local constants with imports from `apps/web/core`.
2. Compute `maximumScreenSpaceError` from the contract's root error and the
   viewport, so refinement distance matches `lodForDistance`.
3. Implement the prefetch policy on `tileVisible`.
4. Build the request-log harness in the dev panel and expose it to Playwright.
5. Playwright: fly from overview to block detail, assert LOD progression and
   **zero** exact-route requests.

## Error Handling

* A prefetch failure → logged, never fatal; the tile loads on demand instead.
* Exceeding `MAX_LOADED_TILES` → Cesium unloads by its own policy; the ceiling is
  a budget, not a hard error.
* An exact-route request during navigation → in development, **throw**; in
  production, log an error. Silence would defeat the purpose.

## Acceptance Criteria

1. Flying from overview to block detail loads LOD 0 → 1 → 2 → 3 → 4 in order.
2. The request log contains **zero** `value` or `blocks` requests across the
   whole flight.
3. Hovering a tile loads no exact value.
4. Selecting one **does**.
5. Frame time < 16 ms at 256 loaded tiles.
6. Prefetch loads children and sibling metadata, and nothing beyond.
7. `never_reads_exact_values_from_camera_movement_alone` still passes, now
   importing from `apps/web/core`.
8. LOD for a given distance matches the golden vector in both languages.

## Verification Plan

**Automated** — vitest for the policy; Playwright for the navigation session with
the request-log assertion.
**Manual** — fly through the model with the dev panel open; watch the log.

## Suggested Commands

```bash
cd apps/web && npx vitest run lod-policy                          # verified today
npx playwright test apps/web/model-viewer/e2e/lod-navigation.spec.ts   # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| Fly overview → block | LOD 0→1→2→3→4 in order |
| Request log for that flight | **Zero** `value`/`blocks` requests |
| Hover a tile | No exact request |
| Click a tile | Exactly one exact request |
| 256 tiles loaded | Frame time < 16 ms |
| `lodForDistance(100)` | LOD 3, matching the golden vector |
| Prefetch at LOD 3 | Children + sibling metadata only |
| Injected exact request in dev | Throws |

## Risks

| Risk | Mitigation |
| --- | --- |
| Screen-space error and distance thresholds disagree | Derived from one contract; the golden vector asserts both |
| Prefetch becomes aggressive and pulls exact data | The request log is asserted, not the intent |
| Removing local constants breaks existing tests | Tests move with them; both suites must stay green |

## Completion Evidence

* Request log from a full navigation session, showing zero exact reads.
* LOD progression capture.
* Frame-time measurement at 256 tiles.
* Both suites still passing after the constant move.
