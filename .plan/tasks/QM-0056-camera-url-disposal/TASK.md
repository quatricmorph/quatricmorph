# QM-0056 — Camera fit, reset, URL state, and full disposal

## Status

Blocked

Unblocks when `QM-0051` reaches `Complete`.

## Phase

Phase 05 — Cesium model viewer

## Objective

Camera controls, shareable URL state, and a teardown path that actually releases
everything.

## Repository Evidence

* `docs/CURRENT_ARCHITECTURE.md` §8, defect 6: *"`window` event listeners are
  added and never removed; harmless for a single-page demo, a leak once the
  workspace is embedded."*
* Defect 4: *"`disposeAndClear` disposes geometries but **not materials or
  textures**."*
* `apps/web/quatricmorph-workspace/src/util/params.ts` — `flatten`, `unflatten`,
  `compress`, `uncompress`, 3 tests.
* `apps/web/quatricmorph-workspace/src/app/url.ts` — URL serialization, ported.
* `docs/CURRENT_ARCHITECTURE.md` §3 — the `config`-URL branch fetched an
  arbitrary URL synchronously and applied it as state. **Deprecated.**
* `cameraPresetPose` in `grid-ruler.ts` — four presets already defined.

## Requirements Covered

`CESIUM-013`, `MVP-41`.

## Dependencies

`QM-0051`.

## Blocks

`QM-0082`.

## Parallelization

Lane B, parallel with `QM-0054`, `QM-0055`, `QM-0057`.

## Program Boundary

`apps/web/model-viewer`.

## Scope

* Fit model, fit selection, reset view, and the four camera presets.
* URL state: model, selection, camera, LOD flags — debounced.
* Restore on load and on `popstate`.
* **Full disposal**: tileset destroyed, primitives removed, listeners removed,
  render handles released, tile cache cleared, materials and textures disposed.

## Out of Scope

Workspace camera (`QM-0067`) · the `config`-URL branch, permanently deprecated ·
session persistence beyond the URL.

## Files Expected to Change

* `apps/web/model-viewer/src/cesium/viewer.ts`
* `apps/web/model-viewer/src/index.ts`

## Files Expected to Add

* `apps/web/model-viewer/src/cesium/camera.ts`
* `apps/web/model-viewer/src/state/url.ts`
* `apps/web/model-viewer/src/__tests__/url-state.test.ts`
* `apps/web/model-viewer/e2e/disposal.spec.ts`

## Files Expected to Remove or Deprecate

None — but the `config`-URL branch is **not** reimplemented here, and a test
asserts no state is applied from a fetched URL.

## Data Contracts

```text
?model=<id>&sel=<canonical>&cam=<compressed>&flags=<bitfield>
```

Camera uses the existing `compress`/`uncompress` helpers. Restored state is
validated against a schema and `castToType`d against defaults; **unknown keys are
dropped, not applied**.

## Memory and Performance Constraints

* Camera writes to history are debounced (`mm` used 250 ms; reused, and this time
  the constant is **named**).
* Disposal must return the JS heap to within 10 % of baseline over 100 model
  switches — the `MVP-41` criterion.

## Implementation Plan

1. `camera.ts`: fit model, fit selection, reset, four presets, all deriving
   bounds from the grid layout.
2. `url.ts`: serialize and deserialize with the ported helpers; validate on
   restore.
3. Debounced history writes with a named constant.
4. `dispose()`: destroy the tileset, remove primitives, **remove every listener
   registered**, release render handles, clear the tile cache, dispose materials
   and textures.
5. Track every listener registration so removal is complete by construction, not
   by memory.
6. Playwright soak: 100 model switches with heap measurement.

## Error Handling

* Malformed URL state → defaults are used; a warning is logged; the app still
  loads.
* An unknown key in the URL → dropped silently, which is correct: it may be from
  a newer version.
* Fit with no selection → falls back to fit model.
* Disposal during an in-flight tile request → the request is aborted, not
  awaited.

## Acceptance Criteria

1. Fit model, fit selection, reset, and all four presets work.
2. URL round-trips camera and selection; reload restores both.
3. `popstate` restores correctly.
4. History writes are debounced with a named constant.
5. **100 model switches return the heap to within 10 % of baseline.**
6. Every registered listener is removed on disposal — asserted by counting.
7. No state is ever applied from a fetched URL.
8. Malformed URL state does not break loading.

## Verification Plan

**Automated** — vitest for URL round trips and validation; Playwright for the
100-switch heap soak and the listener count.
**Manual** — share a URL, open it in a fresh tab, confirm the view matches.

## Suggested Commands

```bash
cd apps/web && npx vitest run url-state                            # introduced here
npx playwright test apps/web/model-viewer/e2e/disposal.spec.ts
```

## Test Cases

| Input | Expected |
| --- | --- |
| Fit selection | Camera frames the selected object |
| Reset | Returns to the model overview |
| Each of four presets | Distinct, deterministic poses |
| Serialize then deserialize | Camera and selection identical |
| Reload with the URL | Same view |
| `popstate` | Restores |
| 100 model switches | Heap within 10 % of baseline |
| Listener count after disposal | Zero added listeners remain |
| `?config=http://evil/` | **Ignored**; nothing fetched |
| Malformed `?cam=` | Defaults used; app loads |

## Risks

| Risk | Mitigation |
| --- | --- |
| A listener is missed and leaks | Registrations are tracked in one place; the count is asserted |
| Cesium holds internal references after `destroy()` | Measured by heap soak, not assumed |
| The `config`-URL branch is reintroduced by habit | An explicit negative test |

## Completion Evidence

* Heap measurements across 100 model switches.
* Listener-count assertion output.
* URL round-trip test output.
* A screenshot pair: original view and the same URL opened fresh.
