# ADR-CANDIDATE-010 — CesiumJS framework shell

## Status

`Open`.

## Context

`ARCHITECTURE.md` §12.1 lists "React or Svelte" in the prototype stack. The
existing web packages use none.

## Repository evidence

* `apps/web/quatricmorph-workspace/package.json` — `three`, `lil-gui`; dev:
  `typescript`, `vite`, `vitest`. **No framework.**
* `apps/web/model-viewer/package.json` — dev only: `typescript`, `vite`,
  `vitest`. **No `cesium` yet.**
* `apps/web/query-interface/package.json` — no dependencies at all.
* `apps/web/quatricmorph-workspace/src/gui/` — `research-gui.ts` (faithful `mm` port)
  and `mvp-gui.ts` (reduced), both built on `lil-gui`.
* `docs/CURRENT_ARCHITECTURE.md` §4 — `mm`'s `initGui` was one 370-line function
  over an untyped `params` bag; the port split it in two.

## Decision required

Does the viewer adopt a UI framework?

## Options

| Option | |
| --- | --- |
| **A** | No framework; plain TypeScript + Vite, as today |
| **B** | React |
| **C** | Svelte |
| **D** | Lit / web components |

## Advantages

* **A** — zero new dependencies; consistent with all three existing packages;
  nothing between the code and the Cesium/Three imperative APIs, which are
  imperative anyway.
* **B** — the largest ecosystem; the panel-heavy layout is a natural fit; the
  team may know it.
* **C** — smaller bundle than React; compiles away.
* **D** — standards-based; no framework runtime.

## Disadvantages

* **A** — panel and tree state is written by hand. The hierarchy tree, inspector,
  and chat history are the parts where a framework would actually help.
* **B** — **React plus Cesium is already ~3.5 MB**; two rendering models
  (declarative React, imperative Cesium) meeting at a `useRef` boundary is a
  well-known source of lifecycle bugs.
* **C** — a third build model in the repository for one package.
* **D** — verbose for panels; weakest tooling of the four.

## Risks

* **A** — hand-written state management grows into the untyped `params` bag that
  `mm` had and the port deliberately dismantled. Mitigation: the state domains in
  [`MATRIX_WORKSPACE_ARCHITECTURE.md`](../MATRIX_WORKSPACE_ARCHITECTURE.md) §9 are
  already separated by design; the viewer adopts the same split.
* **B/C** — bundle size, which is already `PERF`-relevant given Cesium.

## Recommended default

**A.** No framework.

Three reasons, in order of weight:

1. **The viewer's state is small.** A selection, a camera, a breadcrumb, a set of
   toggles. That does not need a reconciler.
2. **Cesium is imperative.** Every framework integration ends up as an escape
   hatch around the imperative API, and the escape hatch is where the bugs live.
3. **Consistency.** Three packages already do it this way, and their 101 tests
   run in vitest with no DOM framework harness.

Revisit **only** if the hierarchy tree and inspector exceed roughly 1 500 lines
of hand-written DOM code, which would be the point at which the framework pays
for itself.

The Cesium dependency itself is unavoidable under
`ADR-CANDIDATE-009`'s recommendation. `QM-0050` measures the bundle and applies
tree-shaking and route-level lazy loading; see
[`PERFORMANCE_PLAN.md`](../PERFORMANCE_PLAN.md) §5.

## Tasks affected

`QM-0050` (establishes the shell), then every Phase 05 task.

## Decision deadline

Before `QM-0050`.
