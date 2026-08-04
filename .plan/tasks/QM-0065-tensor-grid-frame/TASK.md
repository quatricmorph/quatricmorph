# QM-0065 — `TensorGridFrame` completion

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Give every tensor — matrix, row vector, column vector, or scalar — a complete,
labelled frame on one coordinate system.

## Repository Evidence

* `apps/web/quatricmorph-workspace/src/layout/tensor-frame.ts` — `TensorMarginFrame`,
  `buildTensorFrame`, `frameContainsPoint`. `GRID-003` is **`Implemented`**, the
  only such row in `STATUS.md`: it works but has no dedicated test.
* `grid-ruler.ts` — `labelMargin`, `framePadding`, `tensorPadding`, `axisMargin`
  all exist as config.
* `mm/util.js:140-162` `rowGuide`, ported to `util/geometry.ts`.
* `mm/viz.js` `Mat.setName`, `setLegends`, `checkLabel`, `updateLabels` — ported
  to `viz/mat.ts`.
* Task specification §17 lists ten required frame elements.

## Requirements Covered

`GRID-003`, `MVP-27`.

## Dependencies

`QM-0061`, `QM-0062`.

## Blocks

`QM-0067`.

## Parallelization

Parallel with `QM-0063`, `QM-0064` — different modules.

## Program Boundary

`apps/web/quatricmorph-workspace/src/layout`.

## Scope

* Outer boundary, inner margin, title margin.
* Shape label, canonical tensor address, alias, dtype.
* Row and column guides; axis labels from `QM-0061`'s bindings.
* Deterministic anchor and orientation.
* Camera-fit bounds.
* **Rank 0 and rank 1 framed identically to rank 2** — one coordinate system.

## Out of Scope

Cell rendering (`QM-0063`) · the grid lines themselves (`QM-0062`) · hover
metadata (`QM-0068`).

## Files Expected to Change

* `apps/web/quatricmorph-workspace/src/layout/tensor-frame.ts`
* `apps/web/quatricmorph-workspace/src/viz/mat.ts`

## Files Expected to Add

* `apps/web/quatricmorph-workspace/src/layout/__tests__/tensor-frame.test.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```text
Q[10]  [256 × 256]  F32
model.layers[10].self_attention.query_projection.weight
▣ EXACT
```

The frame shows alias, shape, and dtype on the title line; the canonical address
below; the fidelity badge from `QM-0054`'s shared vocabulary.

Anchor: `origin + tensor_anchor`, derived from the logical address, **never
stored**. Two renders of the same tensor place it identically.

## Memory and Performance Constraints

One merged geometry per frame — boundary, margins, guides — not one object per
line. Labels go into the shared text texture, never DOM nodes.

## Implementation Plan

1. Extend `buildTensorFrame` with title, address, and dtype lines.
2. Add row and column guides at `mm`'s stride, from `util/geometry.ts`.
3. Add axis labels from the `QM-0061` binding.
4. Handle rank 0 (one cell, still framed) and rank 1 (row or column) through the
   same code path — **not as special cases**.
5. Compute camera-fit bounds including label margins.
6. Write the tests `GRID-003` never had.

## Error Handling

* A missing canonical address → show the raw name and mark the role `unknown`.
  **Never fabricate an address.**
* A shape too long to label → truncate the *display* with an ellipsis; the
  tooltip carries the full shape.
* Font unavailable → frame renders; labels skipped with a warning.

## Acceptance Criteria

1. Every frame has all ten required elements.
2. Rank 0, 1, and 2 tensors are framed by the same code path.
3. A scalar is framed and labelled, not drawn bare.
4. A row vector and a column vector differ only in orientation.
5. The anchor is deterministic — two renders place a tensor identically.
6. Camera-fit bounds include label margins, so nothing is clipped.
7. Labels use the shared texture; DOM node count is unchanged.
8. An unresolved address shows the raw name and `unknown`.
9. `GRID-003` moves from `Implemented` to `Verified`.

## Verification Plan

**Automated** — the new test file covering all ranks, anchor determinism, and
bounds.
**Manual** — screenshots of scalar, row vector, column vector, and matrix frames.

## Suggested Commands

```bash
cd apps/web && npx vitest run tensor-frame              # introduced here
npm run dev --workspace quatricmorph-workspace
```

## Test Cases

| Input | Expected |
| --- | --- |
| `[256, 256]` matrix | All ten elements present |
| `[]` scalar | Framed, labelled, one cell |
| `[128]` as left operand | Row orientation |
| `[128]` as right operand | Column orientation |
| Same tensor rendered twice | Identical anchor |
| Frame bounds | Include label margins |
| Unresolved address | Raw name + `unknown` |
| Very long shape | Display truncated; tooltip full |
| DOM node count | Unchanged by labels |

## Risks

| Risk | Mitigation |
| --- | --- |
| Rank 0/1 handled as special cases that drift | One code path; asserted across ranks |
| Labels clipped by the camera fit | Bounds include margins; asserted |
| A fabricated address appears plausible | Raw name + `unknown` instead; asserted |

## Completion Evidence

* Screenshots of all four frame kinds.
* Anchor-determinism test output.
* Camera-fit bounds assertion.
* Confirmation that `GRID-003` now has a dedicated test.
