# QM-0050 — Viewer shell and Cesium initialization spike

## Status

Blocked

Unblocks when `QM-0005` reaches `Complete`.

## Phase

Phase 05 — Cesium model viewer. **Risk spike for R1.**

## Objective

Prove CesiumJS can render a non-geospatial tensor tileset at all, using a
hand-authored 3-tile tileset, **before** any generator work depends on it.

## Repository Evidence

* `apps/web/model-viewer/package.json` — devDeps only; **no `cesium`**;
  description: *"app shell only; tileset rendering is not built (CESIUM-001)"*.
* `apps/web/model-viewer/src/` — `index.ts`, `lod-policy.ts`, `tile-client.ts`.
* `ARCHITECTURE.md` §12.1 — Cesium *"still carries many GIS and geospatial
  rendering assumptions."* §11.3 — `CustomShader` is experimental and must not
  become a core dependency.
* `ADR-CANDIDATE-009` (local ENU frame), `010` (no framework), `013`
  (Playwright).
* **Nothing in this repository has ever rendered.**

## Requirements Covered

`CESIUM-006`, `SEC-008`, `MVP-01`.

## Dependencies

`QM-0005`.

## Blocks

`QM-0051`…`QM-0057`.

## Parallelization

Lane B, first task. **Runs early and in parallel with all of Lane A** — that is
the point of a spike.

## Program Boundary

`apps/web/model-viewer`.

## Scope

* Add `cesium`; measure the bundle.
* Initialize with globe, imagery, terrain, sky, atmosphere, sun, moon, and every
  GIS widget **off**, in one documented place.
* `requestRenderMode: true`.
* Place a hand-authored 3-tile tileset in a local ENU frame; confirm it renders,
  refines, and picks.
* The application shell: header reading "Quatricmorph — Trillion-Scale Tensor
  Visualization", panels, status bar.
* A CSP with no `unsafe-eval` and no `unsafe-inline` for scripts.

## Out of Scope

Loading a generated tileset (`QM-0051`) · picking to addresses (`QM-0053`) ·
inspector (`QM-0054`) · any framework.

## Files Expected to Change

* `apps/web/model-viewer/package.json`
* `apps/web/model-viewer/src/index.ts`

## Files Expected to Add

* `apps/web/model-viewer/src/cesium/viewer.ts`
* `apps/web/model-viewer/src/shell/{header,layout,status}.ts`
* `apps/web/model-viewer/vite.config.ts`
* `apps/web/model-viewer/index.html`
* `apps/web/model-viewer/fixtures/hand-tileset/` — 3 tiles, authored by hand
* `apps/web/model-viewer/src/__tests__/viewer-init.test.ts`
* `apps/web/model-viewer/e2e/spike.spec.ts` — Playwright

## Files Expected to Remove or Deprecate

None.

## Data Contracts

The hand-authored tileset is a **minimal, valid** 3D Tiles 1.1 document: a root
plus two children, each with a small instanced GLB. It is checked in, because
it must not depend on the generator that it exists to de-risk.

## Memory and Performance Constraints

* Bundle measured and recorded; Cesium is expected to be the largest dependency
  (~3 MB gzipped). Tree-shaking and lazy loading applied and re-measured.
* Time to first render for 3 tiles: < 2 s.
* `requestRenderMode` means a static scene should cost ≈ 0 CPU — asserted by
  observing no continuous rAF loop.

## Implementation Plan

1. Add `cesium` and `vite`; configure asset copying for Cesium's static files.
2. `viewer.ts`: one `createViewer()` with every GIS feature disabled and a
   comment per line explaining why.
3. Hand-author the 3-tile tileset and its GLBs.
4. Render it; verify refinement by moving the camera.
5. Verify picking returns a feature.
6. Build the shell and the CSP.
7. Playwright spike test with a screenshot.

## Error Handling

* Cesium failing to initialize → a visible error panel naming the cause, never a
  blank canvas.
* A tileset failing to load → visible error with the URL.
* WebGL unavailable → a message explaining the requirement.
* **No error state may be console-only.**

## Acceptance Criteria

1. The hand-authored tileset **renders**, with a screenshot as evidence.
2. Moving the camera closer loads the children.
3. Clicking a tile returns a `Cesium3DTileFeature`.
4. No globe, imagery, terrain, sky, sun, or GIS widget is visible.
5. A static scene issues no continuous render — `requestRenderMode` verified.
6. Bundle size measured before and after tree-shaking.
7. CSP present with no `unsafe-eval`.
8. The header reads "Quatricmorph — Trillion-Scale Tensor Visualization".
9. `npm run build --workspace model-viewer` succeeds.

## Verification Plan

**Automated** — vitest for the init configuration; Playwright for render, refine,
and pick, with a screenshot artifact.
**Manual** — visual inspection; check for precision artifacts at close range.

## Suggested Commands

```bash
cd apps/web && npm install                                   # verified today
npm run dev --workspace model-viewer                          # introduced here
npx playwright test apps/web/model-viewer/e2e/spike.spec.ts
npx vite build --workspace model-viewer && du -sh dist
```

## Test Cases

| Input | Expected |
| --- | --- |
| Load the hand tileset | Renders; screenshot captured |
| Camera moved closer | Children load |
| Click a tile | Returns a feature |
| Scene idle 10 s | No continuous render |
| Malformed tileset | Visible error panel, not a blank canvas |
| WebGL disabled | Explanatory message |
| Bundle | Size recorded |
| Close-range camera | **Precision artifacts assessed and recorded** |

## Risks

| Risk | Mitigation |
| --- | --- |
| **R1 — Cesium cannot render this** | This spike is the mitigation. Failure triggers `ADR-CANDIDATE-009`'s Three.js fallback, with the tile format unchanged |
| Precision loss in a local ENU frame | Explicitly assessed here, at close range, before anything depends on it |
| Bundle size | Measured; tree-shaking and lazy loading applied |

## Completion Evidence

* **A screenshot of the rendered tileset.** This is the single most important
  artifact in Phase 05.
* Bundle size before and after optimization.
* The refinement and pick test output.
* A written assessment of precision at close range.
* A go/no-go recommendation for `ADR-CANDIDATE-009`.
