# QM-0051 — Load a generated tileset from the daemon

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 05 — Cesium model viewer

## Objective

Render a tileset produced by the Phase 04 pipeline, with every error state
designed rather than silent.

## Repository Evidence

* `apps/web/model-viewer/src/tile-client.ts` —
  `treats_a_501_as_a_declared_gap_not_a_failure_to_retry` (`CESIUM-003` Verified).
* `QM-0044` serves `GET /v1/visualizations/{modelId}/tileset.json`.
* `QM-0042`/`QM-0043` serve GLB tiles with feature IDs.
* `ADR-CANDIDATE-019` — immutable content-addressed tile URLs,
  `Cache-Control: immutable`; `tileset.json` gets `no-cache` + `ETag`.
* `CESIUM_VIEWER_ARCHITECTURE.md` §9 — the eleven error states.

## Requirements Covered

`CESIUM-005`, `MVP-18`.

## Dependencies

`QM-0050`, `QM-0046`.

## Blocks

`QM-0052`, `QM-0053`, `QM-0054`, `QM-0055`.

## Parallelization

Lane B, sequential after `QM-0050`.

## Program Boundary

`apps/web/model-viewer`.

## Scope

* Model list from `GET /v1/models`; select and load its tileset.
* `Cesium3DTileset.fromUrl` against the daemon.
* All eleven error states from the architecture document.
* Tile request instrumentation for the dev panel and for tests.
* Rely on HTTP caching; no client cache layer.

## Out of Scope

LOD tuning (`QM-0052`) · picking (`QM-0053`) · inspector (`QM-0054`) ·
hierarchy (`QM-0055`).

## Files Expected to Change

* `apps/web/model-viewer/src/index.ts`
* `apps/web/model-viewer/src/tile-client.ts`

## Files Expected to Add

* `apps/web/model-viewer/src/cesium/tileset.ts`
* `apps/web/model-viewer/src/shell/errors.ts`
* `apps/web/model-viewer/src/__tests__/tileset-lifecycle.test.ts`
* `apps/web/model-viewer/e2e/load-tileset.spec.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

Consumes the tileset and tile routes from
[`API_CONTRACTS.md`](../../API_CONTRACTS.md). A `501` is **a declared gap, not a
retryable failure** — the client shows the requirement ID and stops.

## Memory and Performance Constraints

* Time to first render for a 1 000-tile tileset: < 2 s.
* Tile request latency against the local daemon: < 20 ms.
* `MAX_LOADED_TILES = 256`; `CESIUM_CACHE_BYTES = 512 MiB`.
* JS heap at 256 loaded tiles: < 600 MB.

## Implementation Plan

1. Fetch and render the model list.
2. `loadModel(modelId)`: fetch the tileset URL, `fromUrl`, add to the scene, zoom
   to root bounds.
3. Wire `tileFailed`, `tileLoad`, and `allTilesLoaded` events to the status bar.
4. Implement each error state with a visible presentation.
5. Instrument requests: count, bytes, latency, and **whether any exact-value
   route was called** — the last one is what `QM-0052` asserts.
6. Playwright test loading a generated tileset with a screenshot.

## Error Handling

Per the architecture document's table: daemon unreachable (banner + start
command + backoff retry) · `501` (declared gap, no retry) · conversion incomplete
(greyed subtree) · missing tile (placeholder box at its bounds) · corrupted GLB
(tile fails alone) · missing `.qtile` ("values unavailable") · incompatible
version (refuse, name both) · cache miss (status-bar counter only).

## Acceptance Criteria

1. A generated tileset renders; screenshot captured.
2. Time to first render < 2 s for 1 000 tiles.
3. A stopped daemon shows a banner with the start command.
4. A `501` shows the requirement ID and is **not retried**.
5. A deleted tile file shows a placeholder; siblings keep rendering.
6. A corrupted GLB fails that tile alone.
7. Request instrumentation is visible in the dev panel.
8. JS heap at 256 tiles < 600 MB.
9. No error state is blank or console-only.

## Verification Plan

**Automated** — vitest for lifecycle and error handling; Playwright for render
and the error states, with screenshots.
**Manual** — stop the daemon mid-session; delete a tile; corrupt a tile.

## Suggested Commands

```bash
cargo run -p q-daemon -- --model-root fixtures/tiny-llama-large   # verified today
npm run dev --workspace model-viewer
npx playwright test apps/web/model-viewer/e2e/load-tileset.spec.ts   # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| Generated tileset | Renders; screenshot |
| 1 000 tiles | First render < 2 s |
| Daemon stopped | Banner with the start command |
| Route returns 501 | Requirement ID shown; **no retry** |
| Tile file deleted | Placeholder box; siblings render |
| Tile bytes corrupted | That tile fails alone |
| `.qtile` deleted | Geometry renders; "values unavailable" |
| `asset.version` bumped | Refused, both versions named |
| 256 tiles loaded | Heap < 600 MB |

## Risks

| Risk | Mitigation |
| --- | --- |
| Cesium's error events do not surface enough detail | Supplement with client-side fetch instrumentation |
| A partially converted model looks broken | The greyed-subtree state, with a link to the job |
| Retry storms against a 501 | `CESIUM-003` already prevents it; asserted again here |

## Completion Evidence

* Screenshot of a generated tileset rendering.
* Time-to-first-render measurement.
* Screenshots of each error state.
* Request-instrumentation output.
* Heap measurement at 256 tiles.
