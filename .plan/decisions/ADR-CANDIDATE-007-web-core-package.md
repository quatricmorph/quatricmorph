# ADR-CANDIDATE-007 — Shared web core package

## Status

`Open`.

## Context

The product requirement is that one 3D grid system be shared across all
visualizations and mathematical operations. Today the grid lives in one web
package and the LOD policy in another, each unaware of the other, and both
hand-mirror Rust constants.

## Repository evidence

* `apps/web/matrix-workspace/src/layout/grid-ruler.ts:63` — `DEFAULT_GRID_RULER`,
  ten parameters, `GRID_SNAP_TOLERANCE = 1e-6`.
* `apps/web/model-viewer/src/lod-policy.ts:20,51,102` — its **own** `enum Lod`,
  `LOD_DISTANCE_THRESHOLDS`, and `geometricErrorForLod = 1024 / 2 ** lod` with
  the comment *"mirrors `q_tileset::GeometricError`"*.
* `crates/q-tileset/src/lib.rs:34,46` — `ROOT_GEOMETRIC_ERROR = 1024.0`,
  `GeometricError::for_lod`.
* `apps/web/package.json` — npm workspaces already configured for three packages.
* `matrix-workspace` depends on `three` (^0.185) and `lil-gui`; `model-viewer`
  depends on neither.

## Decision required

Where do the shared spatial, LOD, address, and fidelity types live?

## Options

| Option | |
| --- | --- |
| **A** | A new `apps/web/core` package; both apps depend on it |
| **B** | `model-viewer` depends on `matrix-workspace` |
| **C** | Duplicate and rely on the conformance tests to catch drift |
| **D** | Generate TypeScript from the JSON schema at build time |

## Advantages

* **A** — one definition; no renderer dependency crosses packages; npm workspaces
  already support it; the package is small and dependency-free.
* **B** — no new package.
* **C** — no restructuring at all.
* **D** — generation makes drift structurally impossible.

## Disadvantages

* **A** — one more package to configure.
* **B** — **drags `three` and `lil-gui` (~2 MB) into a package that renders with
  Cesium.** Disqualifying.
* **C** — duplication with a test is still duplication; a test catches drift after
  someone writes it, and the current hand-mirrored comment shows how easily that
  happens.
* **D** — a codegen step in the web build, and generated code is harder to read at
  its point of use than a constant with a comment.

## Risks

* **A** — the package becomes a dumping ground. Mitigation: a stated scope —
  spatial, LOD, address, fidelity. Nothing that imports a renderer.
* Circular dependencies. Mitigation: `core` depends on nothing in the repository.

## Recommended default

**A**, with the schema **imported** (not generated from) at build time:

```text
apps/web/core/
├── spatial/grid.ts        the ten parameters, snap, cell centres
├── spatial/axes.ts        axis binding; rank > 3 refuses (GRID-007)
├── lod/ladder.ts          6 levels, distance thresholds, geometric error
├── address/canonical.ts   canonical address parse/format, alias forms
└── fidelity/exactness.ts  metadata | aggregate | sampled | quantized | exact
```

Importing the JSON directly gives **D**'s guarantee — one source — without a
codegen step, because TypeScript can `import` JSON and infer its types.

Back-compatible re-exports stay at the old paths. `grid-ruler.ts` already
demonstrates the pattern with `GridRuledLinesConfig` and `MarginGridConfig`,
kept *"so existing imports keep working"*.

## Tasks affected

`QM-0004`, `QM-0005`, `QM-0060`; then every Phase 05 and 06 task.

## Decision deadline

Before `QM-0060`.

`QM-0004` and `QM-0005` appear earlier in `Tasks affected` but **do not commit**,
so they do not set the deadline (`README.md` §"How a deadline is derived").
`QM-0004` writes JSON only; `QM-0005` asserts constants in place and puts
*"Moving constants into `apps/web/core`"* explicitly Out of Scope. `QM-0060` is
the first task that must know where the shared types live.
