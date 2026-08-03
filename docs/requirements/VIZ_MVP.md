# Phase 0 — Tensor Tiling Spike Requirements

Active coding target. Source: [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §17–§18. Requirement IDs for commits and agent reports: `TILE-*`.

## Goal

```text
Open one SafeTensors file
→ select one 4096 × 4096 tensor
→ create five LOD levels
→ generate tileset.json (+ GLB / .qtile as needed)
→ visualize in CesiumJS
→ click a cell and retrieve the exact value
```

Concrete MVP profile:

| Choice | Value |
| --- | --- |
| Model | 0.5B–7B SafeTensors, Qwen or Llama-like |
| Tensor | Q projection or MLP down projection |
| Viewer | CesiumJS (3D Tiles 1.1) |
| LOD | model → layer → tensor → block (→ scalar on select) |
| Query | exact scalar and tensor slice |
| Math | one `A @ B` visualization on a real block (not full-matrix animation by default) |

## LOD levels (architecture §9)

| LOD | Object | Data |
| --- | --- | --- |
| 0 | Model | parameter count, bytes, global distributions |
| 1 | Subsystem | layer ranges, aggregate norms |
| 2 | Layer | tensor count, mean norm, anomaly score |
| 3 | Tensor | shape, dtype, histogram, spectrum summary |
| 4 | Block | block statistics, quantized samples |
| 5 | Scalar region | exact or sampled weight values |

## Requirement IDs

| ID | Requirement | Status |
| --- | --- | --- |
| TILE-00 | Architecture docs defer to root `ARCHITECTURE.md`; conflicting Three.js-as-product docs removed/redirected | [x] |
| TILE-01 | Do not load the entire checkpoint into RAM | [ ] |
| TILE-02 | Parse SafeTensors header / shape / byte ranges (single file first; shards in Phase 1) | [ ] |
| TILE-03 | Metadata import can be cancelled and resumed | [ ] |
| TILE-04 | Build multiresolution tiles (summaries → blocks); GLB holds geometry/instances only; tensor payload in `.qtile` | [ ] |
| TILE-05 | Emit `tileset.json` and load it in CesiumJS with view-based LOD | [ ] |
| TILE-06 | Clicking a visual cell returns the correct canonical tensor address | [ ] |
| TILE-07 | Exact scalar matches a Python SafeTensors reference for the same index | [ ] |
| TILE-08 | Zooming out does not load exact values; zooming in only range-reads needed bytes | [ ] |
| TILE-09 | Cache reused after reopening (content-addressed key per architecture §13) | [ ] |
| TILE-10 | Shape-mismatched expression rejected before GPU execution; UI labels exact / sampled / approximate | [ ] |
| TILE-11 | One block-mode `A @ B` visualization (planes A:XY, B:YZ, C:XZ) without multiplying entire matrices by default | [ ] |

## Out of scope (Phase 0)

- Full-model / trillion-scale ingestion
- One cube GLB per weight
- Cesium as tensor compute engine
- Chat freely executing large expressions
- Morph/export platform features (`PLAT-P0-MIR`, export)
- Replacing Cesium with custom WebGPU (Phase 3)
- Treating `mm/` or legacy `quatricmorph/` Three.js UI as the product surface

## Done when

- [ ] TILE-01…TILE-11 checked
- [ ] Automated tests for deterministic addressing / scalar equality vs fixture
- [ ] Manual smoke: open fixture → zoom LOD → click cell → exact value matches reference
