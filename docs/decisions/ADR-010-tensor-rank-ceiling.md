# ADR-010 — Rank ≤ 3 is implemented; higher rank refuses rather than flattens

**Status:** Accepted
**Date:** 2026-08-04
**Implements:** `GRID-007`
**Promoted from:** `.plan/decisions/ADR-CANDIDATE-016-nd-axis-binding.md`

## Context

Real checkpoints contain rank-1 tensors (biases, norms), rank-2 (projections),
rank-3 (some attention layouts, grouped experts), and occasionally rank-4. The
product requirement is that the 3D grid system be designed to support future
expansion to higher-dimensional visualizations — which is a statement about the
*design*, not about what the MVP renders.

The repository is split on rank today:

* `q_source::TensorDescriptor::shape: Vec<u64>` — **arbitrary rank already**.
* `q_tensor_runtime::BlockExtent` — **2-D only**: `row_start`, `row_end`,
  `column_start`, `column_end`.
* `q_tiles::QTileHeader` — `dimensions: u8` carries rank and `origin`/`extent`
  are `[u32; 3]`, so the format allows three; but `QTileHeader::for_block`
  hard-codes `dimensions: 2` and `extent[2] = 1`.
* `apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts` — 2-D operands on three
  planes; `depthSpacing` exists in the config and is **unused** (`0`).
* `schemas/nsir/schema.json` — records named axes such as `output_channel` and
  `input_channel`.

So the metadata layer is rank-agnostic and the block, tile, and layout layers are
2-D with a 3-D-shaped hole already cut.

## Decision

The MVP implements **rank ≤ 3**. Rank above 3 **refuses**.

```text
rank 0   one cell at the tensor anchor; still framed, still labelled
rank 1   axis 0 → row or column, by operand role
rank 2   axis 0 → Y (I), axis 1 → X (J)     row-major, matching Array2D and SafeTensors
rank 3   axis 0 → facet, axes 1,2 → Y,X     facets stacked along Z at depthSpacing
rank >3  bindAxes() returns NotImplemented carrying GRID-007
```

Axis binding follows ADR-009's world axes throughout.

## Alternatives considered

**Rank ≤ 2, refuse above.** The smallest change: it matches `BlockExtent`
exactly and touches no Rust. Rejected because rank-3 is not exotic — grouped
experts and several attention layouts produce it — and a real checkpoint
containing one would simply be unviewable.

**Arbitrary rank with automatic flattening to 2-D.** Never refuses, which is its
whole appeal. Rejected, and it is the dangerous option: a `[32, 128, 128]` tensor
shown as `[32, 16384]` produces a **confidently wrong picture**, inviting the
viewer to read adjacency between values that are not adjacent. That is precisely
the failure class `SRC-014` refuses for unknown dtypes and `NSIR-001` refuses for
unknown roles, and it would be inconsistent to admit it here.

**Arbitrary rank with a user-chosen axis binding.** The most general answer, and
the right long-term one. Rejected for the MVP: it needs a UI, a persistence
story, and a URL-state story, for a case the MVP fixture does not contain.

## Why refusing is the honest option

`bindAxes` returning `NotImplemented` with a requirement ID is the repository's
established idiom for a declared gap — the same shape as `L3BrowserCache`'s
refusal, `q-cuda`'s `hardware_verified: false`, and the daemon's 501s. A refusal
tells the user the tool does not do this yet. A flattened picture tells them
something false, and does it in a form they cannot detect.

## The designed rule above rank 3

Recorded now so that implementing it later is additive rather than a redesign.
Axes partition into a **display pair** (→ X, Y), an optional **depth axis**
(→ Z), and a **facet set**. The facet set lays out as a grid of grids whose cell
is one whole tensor frame plus `framePadding`, placed with the *same* ruler and
the *same* invariant one level up.

The grid invariant is stated over positions rather than over cells precisely so
that it holds recursively — which is what makes this extension additive.

## Consequences

* `schemas/visualization/spatial-contract.json` records
  `axis_binding.max_implemented_rank = 3` (`QM-0004`), asserted from both
  languages at gate G1 (`QM-0005`).
* `BlockExtent` and `QTileHeader::for_block` gain an **optional depth extent
  defaulting to `1`**, so 2-D behaviour and the `TILE-002` tests that verify it
  are unchanged. `QM-0040` carries this.
* `depthSpacing` stops being an unused config parameter and becomes the facet
  stride.
* `.qtile` v1 needs no format change: `dimensions: u8` and `[u32; 3]`
  origin/extent already admit rank 3. ADR-004 stands unmodified.
* Raising the ceiling later means implementing the partition rule above and
  changing one number in the spatial contract — a golden-vector change, which
  fails loudly in both languages rather than drifting.
