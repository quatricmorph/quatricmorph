# QM-0153 — Rendering ceiling and labelled degradation

## Status

Complete

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

Full derivation, including the failing-first output and the rendered legend
text: `.plan/evidence/QM-0153.md`.

**Failing first** — the 24 tests of
`apps/web/diagnostics/src/__tests__/degradation.test.ts` were written and run
before any source change and committed in that state as `9950ec7`:

```bash
git checkout 9950ec7
cd apps/web && npx vitest run diagnostics/src/__tests__/degradation.test.ts
```

```
 Test Files  1 failed (1)
      Tests  13 failed | 11 passed (24)
```

Three lines of that file changed afterwards, in two assertions, when the legend
entry kind became `engine-coarse` rather than `sampled`; nothing else moved, and
the 13/11 split is reproducible at that SHA.

The 11 that passed are the anti-truncation coverage assertions — they hold on
`main` today, and exist to fail if a truncation path is ever added.

**Passing** — implementation commit `9401dc8`:

```bash
cd apps/web && npx vitest run
```

```
 Test Files  22 passed (22)
      Tests  361 passed (361)
```

Floor raised in the same commit: `336 + 24 + 1 = 361` tests over `21 + 1 = 22`
files (24 new in `degradation.test.ts`, 1 new committed image in
`artifacts.test.ts`). Rust untouched at 744/54.

**The legend as rendered**, read out of the committed
`apps/web/diagnostics/artifacts/sampled-greyscale.svg` and
`aggregated-colour.svg`:

```
Columns are merged 2 to a cell, by maximum rather than mean, so one bad channel is not averaged away.
The numbers on this map are sampled, which describes how they were obtained. Column merging describes only how they are
drawn, and never changes the label a number carries.
- -  cells with a dashed border aggregate more than one channel, by maximum (factor 2)
◣  a corner wedge marks sampled values from the engine; a dashed border marks columns the renderer merged
```

**Not performed** — no screenshot was taken: no browser and no headless
renderer is available here. The committed SVGs are renderings from the same
draw plan the browser canvas consumes, asserted byte-for-byte by
`artifacts.test.ts`; they are not screenshots and are not offered as such. The
2-D canvas painter still draws neither mark (it never drew the aggregation dash
either); `present.ts` writes the marked SVG onto the page beside it, so the
marks are on screen, but the canvas element itself is unmarked. Both are set
out in the evidence file.

**Gates** — `./scripts/verify-baseline.sh` exit 0 on `253e559`:

```
  ok    rust tests: measured 744, floor 744 — at floor
  ok    rust test binaries: measured 54, floor 54 — at floor
  ok    web tests: measured 361, floor 361 — at floor
  ok    web test files: measured 22, floor 22 — at floor
  ...all 13 CLI goldens ok
elapsed: 134s (budget: 300s)
verify-baseline: OK
```

**Departures from the plan**, both recorded in the evidence file:
`render.ts` was changed although it is not in `Files Expected to Change` —
criteria 2 and 4 are about a mark a reader can see and nothing else draws one;
and `CellFidelity`'s `aggregated` variant carries `channelsPerCell: number |
null` rather than `number`, because manifest v1's summary projection publishes
no channel extent and `number` would force a claim of one channel.
