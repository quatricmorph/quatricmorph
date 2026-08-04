# QM-0152 — Surface reads a real manifest end to end

## Status

Blocked

Unblocks when `QM-0150` and `QM-0143` reach `Complete`.

## Phase

Phase 13 — Diagnostic surface

## Objective

Connect the surface to the daemon and render a diagnosis of the real `QM-0100`
checkpoint — closing the loop from checkpoint bytes to a pixel a human reads.

## Repository Evidence

* `QM-0150` — the surface, built and tested against synthetic manifests.
* `QM-0143` — `GET /v1/diagnostics/{runId}/summary` and the full manifest route.
* `apps/web/model-viewer/src/tile-client.ts` — the daemon-client idiom, including
  `treats_a_501_as_a_declared_gap_not_a_failure_to_retry`.

## Requirements Covered

`V1-24`.

## Dependencies

`QM-0150`, `QM-0143`, `QM-0125`.

## Blocks

`QM-0151`, `QM-0161`.

## Parallelization

Lane S.

## Program Boundary

`apps/web/diagnostics`.

## Scope

* A daemon-backed manifest client with the existing error conventions.
* Run selection where more than one diagnosis exists.
* An end-to-end browser test against a running daemon.
* Screenshots of the real checkpoint's diagnosis.

## Out of Scope

Starting a run from the browser · authentication · remote daemons.

## Files Expected to Change

* `apps/web/diagnostics/src/manifest-client.ts`
* `apps/web/diagnostics/src/app.ts`

## Data Contracts

Consumes the routes `QM-0143` defines. A `501` is a declared gap carrying a
requirement ID and is shown as such, never retried — matching `CESIUM-003`.

## Memory and Performance Constraints

Initial load fetches the **summary** projection only. For a checkpoint with tens
of thousands of tensors, the full manifest is tens of MB and must never be the
initial payload. Detail is fetched per layer, on an explicit click.

Browser heap after rendering the real checkpoint's summary must stay within a
documented bound; a soak of repeated run-switching is `QM-0085`'s scope.

## Implementation Plan

1. Manifest client against the daemon, with schema validation and version
   refusal preserved from `QM-0150`.
2. Run selection listing available `runId`s.
3. Render the real diagnosis; capture screenshots.
4. A browser test (Playwright or the existing harness) asserting that tiles of
   the map render and that only the summary was fetched initially.
5. Error states exercised: daemon down, unknown `runId`, run in progress, 501.

## Error Handling

| Case | Behaviour |
| --- | --- |
| Daemon unreachable | Named error with retry, distinct from a declared gap |
| Unknown `runId` | 404 surfaced as "no such run", with the list of runs |
| Run in progress | Progress state from the job route; never a partial manifest |
| 501 | Declared gap with its requirement ID; no retry |
| Schema validation failure | Refuse to render; show the failing field |

## Acceptance Criteria

1. The surface renders a diagnosis of the real `QM-0100` checkpoint from the
   daemon.
2. Only the summary projection is fetched for the initial view — asserted from a
   request log.
3. Per-layer detail is fetched on click and only then.
4. Every error state above renders distinguishably.
5. Screenshots of the real diagnosis are captured for `QM-0151` and the release.
6. Browser heap after initial render is within the documented bound.
7. The web suite stays green.

## Verification Plan

**Automated** — a browser test with request-log assertions.
**Manual** — screenshots, and one error state exercised by hand.

## Suggested Commands

```bash
cargo run -p q-daemon -- --model-root models/ &
cd apps/web/diagnostics && npx vite dev
cd apps/web && npx vitest run
```

## Test Cases

| Input | Expected |
| --- | --- |
| Real checkpoint run | Renders; screenshot captured |
| Initial load request log | Summary only |
| Click a layer | One detail request, that layer only |
| Daemon stopped | Named error with retry |
| Unknown `runId` | "No such run" plus the run list |
| Run in progress | Progress, no partial manifest |
| 501 route | Declared gap with requirement ID |

## Risks

| Risk | Mitigation |
| --- | --- |
| The full manifest is fetched and the tab stalls | Acceptance criterion 2, asserted from the request log |
| A partial manifest renders as complete | The in-progress state is an explicit error case |
| Screenshots taken from synthetic data | Criterion 1 names the real checkpoint |

## Completion Evidence

* Screenshots of the real diagnosis.
* The request log for the initial load.
* Browser test output.
* One error state, captured.
