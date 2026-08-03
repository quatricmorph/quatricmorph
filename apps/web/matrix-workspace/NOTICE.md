# Attribution

`apps/web/matrix-workspace` is derived from **mm**, the matrix-multiplication
visualizer by Meta Platforms, Inc. and affiliates. The original lives in this
repository at [`mm/`](../../../mm), kept read-only as a historical reference.

The original MIT license is reproduced verbatim in [`LICENSE`](LICENSE) and
applies to the derived work. Copyright notices must be preserved in any further
redistribution.

## What was carried over

| From `mm/` | Now at | Change |
| --- | --- | --- |
| `viz.js` `Array2D` | `src/viz/array2d.ts` | ported to TypeScript |
| `viz.js` `Mat` | `src/viz/mat.ts` | ported |
| `viz.js` `MatMul` | `src/viz/matmul.ts` | ported; block/animation/scatter math extracted to `src/math/` |
| `viz.js` layout constants and placement | `src/layout/grid-ruler.ts` | reorganized into `GridRuler3D` |
| `util.js` params/URL/compression | `src/util/params.ts` | ported |
| `util.js` line, guide, and text helpers | `src/util/geometry.ts`, `src/util/text.ts` | ported |
| `gui.js` | `src/gui/research-gui.ts`, `src/gui/mvp-gui.ts` | ported and split |
| `index.html` bootstrap | `src/app/` | split into scene, camera, URL, and instruction modules |
| `examples/` | `src/examples/` | ported |

## What was deliberately not carried over

`mm/viz.js:119-126` (`tryEvalInitExpr`) builds an initializer function with
`eval?.()`. Quatricmorph does not carry that forward at any layer — WeightQL is
a closed expression language with no `eval`, no user-defined functions, and no
code loading. See `docs/decisions/ADR-006-weightql-no-arbitrary-execution.md`
and the `Deprecate` decision recorded in `docs/CURRENT_ARCHITECTURE.md`.
