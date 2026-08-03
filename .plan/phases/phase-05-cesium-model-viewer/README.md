# Phase 05 — Cesium model viewer

## Goal

```text
Open tileset.json → navigate model to tensor block → pick a block
→ resolve its canonical tensor address → show its fidelity honestly
```

## The gap

`apps/web/model-viewer/` is three files and declares **no `cesium` dependency**.
Its own `package.json` says *"app shell only; tileset rendering is not built."*

Two things **are** built and tested, and they are the two that matter most:
`lod-policy.ts` (10 tests, including
`never_reads_exact_values_from_camera_movement_alone`) and `tile-client.ts`
(`treats_a_501_as_a_declared_gap_not_a_failure_to_retry`).

## Entry conditions

* **G1** passed.
* `ADR-CANDIDATE-009` (3D Tiles version and non-geospatial placement), `010`
  (no framework), `013` (Playwright), `019` (HTTP caching) decided.
* **G2** for every task except `QM-0050`.

## The spike comes first

`QM-0050` is scheduled **early and deliberately small**: initialize Cesium with
every geospatial feature disabled, and render a **hand-authored 3-tile
tileset** — before any generator work depends on it.

This is [`RISK_REGISTER.md`](../../RISK_REGISTER.md) **R1**, the only risk whose
fallback costs a phase rather than a task. Cesium has never rendered anything in
this repository, and `ARCHITECTURE.md` §12.1 is candid that it *"still carries
many GIS and geospatial rendering assumptions."* Finding that out in Phase 05
after Phase 04 built for it would be the worst available ordering.

**If the spike fails**, the fallback is Three.js with a custom LOD traversal —
and the tile format does not change. `.qtile`, GLB, and `tileset.json` are
renderer-independent; only this package would be rewritten.

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0050` | Viewer shell and Cesium initialization spike | Implementation | `CESIUM-006`, `SEC-008`, `MVP-01` |
| `QM-0051` | Load a generated tileset from the daemon | Implementation | `CESIUM-005`, `MVP-18` |
| `QM-0052` | Camera-driven LOD wired to the shared policy | Implementation | `CESIUM-002`, `AC-006`, `MVP-19`, `MVP-20` |
| `QM-0053` | Feature pick → canonical tensor address | Implementation | `CESIUM-007`, `AC-004`, `MVP-21`, `MVP-22` |
| `QM-0054` | Inspector panel and exactness badges | Implementation | `CESIUM-008`, `AC-010`, `MVP-24` |
| `QM-0055` | Hierarchy, breadcrumbs, search by address and alias | Implementation | `CESIUM-009`, `MVP-06` |
| `QM-0056` | Camera fit, reset, URL state, full disposal | Implementation | `CESIUM-013`, `MVP-41` |
| `QM-0057` | glTF extension capability probe and fallback | Implementation | `CESIUM-010` |

## Design constraints

* **Cesium is a tile-traversal and rendering layer, not a compute engine**
  (`ARCHITECTURE.md` §19). No tensor arithmetic in this package.
* Globe, imagery, terrain, sky, atmosphere, sun, moon, and every GIS widget are
  **off**, in one place, with a comment saying why.
* `requestRenderMode: true` — render on change. A static model must not spin a
  fan.
* **Camera movement alone never triggers an exact read.** The policy is already
  tested; this phase wires it and proves it with a request log from a real
  navigation session.
* **Every panel showing a number shows a fidelity badge**, distinguished by a
  glyph as well as colour — the same accessibility reasoning §18 applies to
  selection.
* **No error state is a blank screen or a console-only message.** Every condition
  in [`CESIUM_VIEWER_ARCHITECTURE.md`](../../CESIUM_VIEWER_ARCHITECTURE.md) §9 has
  a designed presentation.
* Disposal is not optional: `mm` added `window` listeners and never removed them,
  and its `disposeAndClear` skipped materials and textures.

## Exit conditions — **integration gate G3**

1. A generated tileset from Phase 04 loads and renders in CesiumJS.
2. Approaching the camera refines LOD; a request log shows the expected sequence.
3. A full navigation session from model overview to block detail issues
   **zero exact-value requests**.
4. Clicking a cell resolves to a canonical address that **equals what
   `q-cli value` returns** for the same index.
5. Every fidelity badge state is reachable and screenshotted.
6. Search by canonical address, by alias, and by raw name all work; an ambiguous
   alias shows candidates and does not choose.
7. Switching models 100 times returns the JS heap to within 10 % of baseline.
8. The browser console is empty across the manual checklist.

## Parallelization

`QM-0050` first, alone. Then `QM-0051` → `QM-0052` → `QM-0053` is sequential.
`QM-0054`, `QM-0055`, `QM-0056`, and `QM-0057` are independent of each other and
can run in parallel once `QM-0051` lands, though all touch
`apps/web/model-viewer/src/` and should coordinate on the shell layout.

## Risks

| Risk | Mitigation |
| --- | --- |
| **R1 — Cesium cannot render this** | `QM-0050` spike, early and small; Three.js fallback with an unchanged tile format |
| R4 — extension support | `QM-0057`, three profiles, floor uses no extension |
| R8 — browser memory growth | `QM-0056` owns teardown; `QM-0082` soaks it |
| Cesium bundle size (~3 MB gzipped) | Measured in `QM-0050`; tree-shaking and route-level lazy loading |
