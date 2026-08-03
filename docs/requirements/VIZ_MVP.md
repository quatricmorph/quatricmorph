# Visualization MVP Requirements (Track A)

Active coding target for `quatricmorph/`. Full engineering brief: root [`prompts.md`](../../prompts.md).

## Goal

Visualize a single multiplication:

```text
A @ B = C
```

aligned to shared **3D grid ruled lines** (MarginGrid3D concept). Visualization is an interface; math and layout contracts are the product.

## Supported shapes

| Case | Result |
| --- | --- |
| Matrix @ Matrix | Matrix |
| Matrix @ Column | Column |
| Row @ Matrix | Row |
| Row @ Column | Scalar |

Types are shape-inferred (`m×n`, `n×1`, `1×n`, `1×1`). Same cell/frame/value systems for all.

## Requirement IDs

| ID | Requirement | Status |
| --- | --- | --- |
| VIZ-00 | Repo builds (`npm run build`) and unit tests pass | [x] |
| VIZ-01 | Validate `A.cols === B.rows` before multiply; clear error; no partial Three.js leaks | [x] |
| VIZ-02 | Deterministic `C[i,j] = Σ A[i,k]*B[k,j]` for fixed inputs | [x] |
| VIZ-03 | Shared GridRuledLines3D / MarginGrid3D params (`cellSize`, gaps, margins, origin) drive placement | [x] |
| VIZ-04 | World axes: X→J, Y→I, Z→K; planes A(I×K), B(K×J), C(I×J) aligned | [x] |
| VIZ-05 | Tensor margin frames for A/B/C with consistent padding/labels | [x] |
| VIZ-06 | Value→size and value→color mapping consistent across tensors | [x] |
| VIZ-07 | Orbit camera + hover/spotlight labels without breaking layout | [x] |
| VIZ-08 | URL/param serialization for reproducible scenes | [x] |
| VIZ-09 | MVP UI hides attention/LoRA/nested expr/model loading | [x] |
| VIZ-10 | Unit tests cover Array2D multiply path + genExpr/defaults used by MVP | [x] |

## Out of scope (do not surface in MVP UI)

Attention heads, QKV/softmax pipelines, LoRA viz, nested matmul trees, transformers/MLP blocks, broadcasting/batches, sparse tensors, PyTorch/model weight import, notebooks, accounts, backends.

Existing advanced `mm` code may remain internally if removal is destabilizing, but must not appear as primary MVP UX. Attention example pages remain under `quatricmorph/examples/` but are not linked from the MVP shell.

## Done when

- [x] All VIZ-01…VIZ-09 checked
- [x] `npm test` and `npm run build` green
- [ ] Manual smoke: load app, multiply small matrices, orbit, hover values, reload URL restores scene
