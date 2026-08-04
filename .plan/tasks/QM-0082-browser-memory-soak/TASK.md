# QM-0082 — Browser memory and disposal soak

## Status

Blocked

Unblocks when `QM-0152` reaches `Complete`. Scope narrows to the diagnostics surface — there is no Cesium viewer or matrix workspace in v1.

**v1 dependency rewiring.** This task's `## Dependencies` section names tasks that are now `Deferred`. For v1 it is unblocked by the tasks named above; the original edges return with the post-v1 platform release. See [`EXECUTION_ORDER.md`](../../EXECUTION_ORDER.md) §10.

## Phase

Phase 08 — Integration and performance

## Objective

Prove repeated selection and re-initialization do not leak — the defect `mm`
demonstrably had.

## Repository Evidence

* `docs/CURRENT_ARCHITECTURE.md` §8 defect 6: *"`window` event listeners are
  added and never removed."*
* Defect 4: *"`disposeAndClear` disposes geometries but **not materials or
  textures**."*
* `mm/index.html:404-412` — `renderer.info.memory` is already surfaced in the
  GUI, so the number is visible during development.
* `QM-0056` owns viewer teardown; `QM-0067` fixes workspace disposal.
* `ADR-CANDIDATE-013` — Playwright, with `page.metrics()` for heap.

## Requirements Covered

`PERF-002`, `CESIUM-013`, `MVP-41`.

## Dependencies

`QM-0080`, `QM-0056`, `QM-0067`.

## Blocks

`QM-0094`.

## Parallelization

Parallel with `QM-0081`, `QM-0083`…`QM-0085`.

## Program Boundary

Playwright suite; no source changes unless a leak is found.

## Scope

* 100 model switches in the viewer.
* 100 workspace re-initializations.
* 100 selection changes.
* 1 000 hover events.
* Measure JS heap, `renderer.info.memory` (geometries, textures, programs), and
  listener counts.

## Out of Scope

CUDA memory (`QM-0083`) · daemon RSS (`QM-0084`) · fixing a leak, which becomes
its own task unless trivial.

## Files Expected to Change

* `.github/workflows/build.yaml` — a nightly `soak` job

## Files Expected to Add

* `apps/web/e2e/memory-soak.spec.ts`
* `scripts/soak-report.js`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
{ "scenario": "model-switch", "iterations": 100,
  "baseline": { "heap_mb": 142, "geometries": 12, "textures": 4, "programs": 6 },
  "final":    { "heap_mb": 149, "geometries": 12, "textures": 4, "programs": 6 },
  "growth_pct": 4.9, "verdict": "pass" }
```

**Pass = heap within 10 % of baseline and resource counts exactly equal.** Heap
tolerates GC timing; a geometry count that grew is a leak with no ambiguity.

## Memory and Performance Constraints

* Forced GC between iterations where the browser allows it, so the measurement
  reflects retention rather than collection lag.
* Baseline taken **after** the first iteration, so one-time allocation is not
  counted as growth.
* The full soak in under 20 minutes.

## Implementation Plan

1. Playwright scenarios for the four loops.
2. Force GC and sample `page.metrics()` plus `renderer.info.memory` each
   iteration.
3. Count registered listeners via an instrumented registration path.
4. Compute growth against the post-first-iteration baseline.
5. Emit the JSON report; fail on any resource-count growth or > 10 % heap growth.
6. Nightly CI job with the report as an artifact.

## Error Handling

* GC unavailable → note it and widen the heap tolerance to 20 %; **resource
  counts remain exact**, since they do not depend on GC.
* A browser crash mid-soak → fail, reporting the iteration reached.
* A monotone growth trend within tolerance → **report it as a warning**; a slow
  leak passes a 100-iteration threshold and fails a user's afternoon.

## Acceptance Criteria

1. 100 model switches: heap within 10 % of baseline.
2. Geometry, texture, and program counts return **exactly** to baseline.
3. 100 workspace re-inits: same.
4. 100 selection changes: same.
5. 1 000 hovers: DOM node count constant.
6. Listener count after disposal equals baseline.
7. A monotone growth trend is reported even when within tolerance.
8. The soak completes in under 20 minutes.
9. The report is archived as a CI artifact.

## Verification Plan

**Automated** — the nightly soak job.
**Manual** — review the growth curves for a slow trend.

## Suggested Commands

```bash
npx playwright test apps/web/e2e/memory-soak.spec.ts     # introduced here
node scripts/soak-report.js results/soak.json
```

## Test Cases

| Scenario | Iterations | Assertion |
| --- | --- | --- |
| Model switch | 100 | Heap ≤ +10 %; resources equal |
| Workspace re-init | 100 | Heap ≤ +10 %; **textures equal** (the `mm` defect) |
| Selection change | 100 | Heap ≤ +10 % |
| Hover | 1 000 | DOM node count constant |
| Listener count post-disposal | — | Equals baseline |
| Growth trend | — | Reported even inside tolerance |

## Risks

| Risk | Mitigation |
| --- | --- |
| GC timing produces noise | Forced GC; resource counts are exact and GC-independent |
| A slow leak passes 100 iterations | The trend is reported, not just the endpoint |
| The soak is slow and gets skipped | Nightly, not per-commit; 20-minute budget |

## Completion Evidence

* The soak report JSON for all four scenarios.
* Growth curves.
* Listener-count comparison.
* Confirmation that texture counts return to baseline — the specific `mm` defect.
