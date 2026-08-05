# QM-0153 — Rendering ceiling and labelled degradation

## Status

Blocked

Unblocks when `QM-0150` reaches `Complete`.

## Phase

Phase 13 — Diagnostic surface

## Objective

Make the surface's aggregation honest: above a defined cell ceiling it aggregates
rather than truncating, and it **says so where the user is looking** — not only in
a footnote.

## Repository Evidence

* `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts` — `assertBlockIsBounded`
  and `refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser`
  (`GRID-005`): the ceiling-and-refuse idiom.
* `.plan/PRODUCT_SCOPE.md` §6 — degrade to an aggregate representation and say so
  in the fidelity label; never silently truncate.
* `crates/q-tiles/src/lib.rs` — `quantized_tiles_are_half_the_size_and_declare_themselves_lossy`
  (`TILE-008`): a lossy representation declares itself.

## Requirements Covered

`SURF-002`, `V1-26`.

## Dependencies

`QM-0150`.

## Blocks

`QM-0165`.

## Parallelization

Lane S. Edits the same files as `QM-0150`, so it follows it.

## Program Boundary

`apps/web/diagnostics`.

## Scope

* `MAX_HEATMAP_CELLS`, defined, documented, and enforced.
* Column aggregation above it, with the aggregation factor visible.
* A per-cell indication that a cell is aggregated, legible without hovering.
* The same treatment for `sampled` fidelity arriving from the manifest.

## Out of Scope

Level-of-detail streaming · a tile pyramid (deferred with the platform) ·
changing the aggregation arithmetic (that is the engine's, `QM-0123`).

## Files Expected to Change

* `apps/web/diagnostics/src/heatmap.ts`
* `apps/web/diagnostics/src/app.ts`

## Data Contracts

```ts
const MAX_HEATMAP_CELLS = 250_000;

type CellFidelity =
  | { kind: 'exact' }
  | { kind: 'aggregated'; channelsPerCell: number }
  | { kind: 'sampled' };            // propagated from the manifest's fidelity field
```

Three states, not two. `sampled` comes from the engine and means something
different from `aggregated`, which comes from the renderer; conflating them would
tell a user that the data is coarse when in fact the display is, or vice versa.

## Memory and Performance Constraints

Rendered cells ≤ `MAX_HEATMAP_CELLS`, always. A 100-layer × 8 192-channel model
is 819 200 cells and aggregates to a factor of 4 or more.

Aggregation is by **maximum**, not mean, and the choice is stated in the UI. A
mean hides a single catastrophic channel inside a healthy group — which is the
exact finding a compression engineer opened the tool for. This is a product
decision, not a rendering detail, which is why it appears here as a contract.

## Implementation Plan

1. Compute the required aggregation factor from layer count × channel count
   against the ceiling.
2. Aggregate columns by maximum; record `channelsPerCell`.
3. Render aggregated cells with a persistent visual marker — a border treatment
   or hatch — legible without interaction and in greyscale.
4. State the aggregation factor and the "maximum, not mean" rule in the legend.
5. Propagate `sampled` fidelity from the manifest as a distinct marker.
6. Tests: the ceiling holds at extreme dimensions; markers are present; the
   legend states the rule.

## Error Handling

| Case | Behaviour |
| --- | --- |
| Cell count exceeds the ceiling | Aggregate; never truncate; never render off-screen cells |
| A layer with a single channel | No aggregation; not a special-cased blank |
| Manifest reports `sampled` | Distinct marker, distinct legend entry |
| Aggregation factor of 1 | No marker; identical to the exact case |

## Acceptance Criteria

1. Cell count never exceeds `MAX_HEATMAP_CELLS`, at any input dimension.
2. Aggregated cells carry a persistent marker, legible without hover and in
   greyscale.
3. The legend states the aggregation factor and that aggregation is by maximum.
4. `sampled` is visually distinct from `aggregated`.
5. No truncation path exists — a test asserts that every channel is represented in
   some cell.
6. Aggregation factor 1 renders identically to the unaggregated case.

Criterion 5 is the important one: truncation is the failure mode that produces a
confidently wrong screenshot.

## Verification Plan

**Automated** — cell-count bounds at extreme dimensions; a coverage assertion
that every channel maps into a cell; marker presence.
**Manual** — a screenshot of the degraded state, in colour and greyscale.

## Suggested Commands

```bash
cd apps/web && npx vitest run diagnostics
```

## Test Cases

| Input | Expected |
| --- | --- |
| 12 × 512 | No aggregation, no markers |
| 100 × 8 192 | Aggregated; ≤ 250 000 cells; markers present |
| 1 000 × 65 536 | Still ≤ ceiling; every channel covered |
| Layer with 1 channel | Rendered; no marker |
| Manifest with `fidelity: sampled` | Distinct marker and legend entry |
| Channel-coverage assertion | Every channel index appears in exactly one cell |

## Risks

| Risk | Mitigation |
| --- | --- |
| Aggregation by mean hides a catastrophic channel | Maximum is the contract, stated in the legend and tested |
| Markers invisible in greyscale | Greyscale screenshot is part of the evidence |
| A truncation path is added later for performance | Criterion 5's coverage assertion fails if one appears |
| `aggregated` and `sampled` conflated | Separate variants in the type; separate legend entries |

## Completion Evidence

* Screenshots of the degraded state, colour and greyscale.
* Test output including the coverage assertion.
* The legend text as rendered.
