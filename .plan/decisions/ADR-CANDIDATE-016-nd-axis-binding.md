# ADR-CANDIDATE-016 — N-D axis binding

## Status

`Open`.

## Context

The product requirement: *the 3D grid system should be designed to support future
expansion to higher-dimensional tensor visualizations.* Real checkpoints contain
rank-1 (biases, norms), rank-2 (projections), rank-3 (some attention layouts and
grouped experts), and occasionally rank-4 tensors.

## Repository evidence

* `q_source::TensorDescriptor::shape: Vec<u64>` — **arbitrary rank already**.
* `q_tensor_runtime::BlockExtent` — **2-D only**: `row_start`, `row_end`,
  `column_start`, `column_end`.
* `q_tiles::QTileHeader` — `dimensions: u8` carries rank; `origin: [u32;3]` and
  `extent: [u32;3]` allow 3; `QTileHeader::for_block` **hard-codes
  `dimensions: 2`** and `extent[2] = 1`.
* `apps/web/matrix-workspace/src/layout/grid-ruler.ts` — entirely 2-D operands on
  three planes; `depthSpacing` exists in the config and is **unused** (`0`).
* `schemas/nsir/schema.json` — records named axes such as `output_channel` and
  `input_channel`.

So: the metadata layer is rank-agnostic; the block, tile, and layout layers are
2-D with a 3-D-shaped hole already cut.

## Decision required

What rank does the MVP implement, and what happens above it?

## Options

| Option | |
| --- | --- |
| **A** | Rank ≤ 2 implemented; rank > 2 refuses |
| **B** | Rank ≤ 3 implemented (facets along Z); rank > 3 refuses |
| **C** | Arbitrary rank with automatic flattening to 2-D |
| **D** | Arbitrary rank with a user-chosen axis binding |

## Advantages

* **A** — smallest; matches `BlockExtent` exactly.
* **B** — covers real rank-3 tensors; **uses `depthSpacing`, which already
  exists**; the meta-grid rule generalises cleanly to higher ranks later.
* **C** — never refuses.
* **D** — most general; the user controls the projection.

## Disadvantages

* **A** — a rank-3 tensor in a real checkpoint becomes unviewable, and rank-3 is
  not exotic.
* **B** — `BlockExtent` and `QTileHeader::for_block` need a third dimension.
* **C** — **flattening produces a confidently wrong picture.** A `[32, 128, 128]`
  tensor shown as `[32, 16384]` invites the viewer to read adjacency that does not
  exist. This is exactly the failure class `SRC-014` and `NSIR-001` refuse
  elsewhere.
* **D** — a UI, a persistence story, and a URL-state story, for a case the MVP
  fixture does not contain.

## Risks

* **C** is the dangerous option, for the reason above.
* **B** touches `BlockExtent`, which `TILE-002` verifies. Mitigation: extend with
  an optional depth extent defaulting to `1`, so 2-D behaviour and its tests are
  unchanged.

## Recommended default

**B.**

```text
rank 0  one cell at the tensor anchor; still framed, still labelled
rank 1  axis 0 → row or column, by operand role
rank 2  axis 0 → Y (I), axis 1 → X (J)          row-major, matching Array2D and SafeTensors
rank 3  axis 0 → facet, axes 1,2 → Y,X          facets stacked along Z at depthSpacing
rank >3 bindAxes() returns NotImplemented carrying GRID-007
```

The designed rule for rank > 3, recorded so implementing it is additive: axes
partition into a **display pair** (→ X, Y), an optional **depth axis** (→ Z), and
a **facet set**; the facet set lays out as a grid of grids whose cell is one whole
tensor frame plus `framePadding`, placed with the *same* ruler and the *same*
invariant one level up. The invariant is stated over positions rather than cells
precisely so that it holds recursively.

**Rank > 3 must refuse, not flatten.** `bindAxes` returning `NotImplemented` with
a requirement ID is the repository's established idiom for a declared gap, and it
is honest in a way that a flattened picture cannot be.

## Tasks affected

`QM-0061` (implements), `QM-0004` (`max_implemented_rank` in the contract),
`QM-0040` (block planner gains a depth extent).

## Decision deadline

Before `QM-0061`.
