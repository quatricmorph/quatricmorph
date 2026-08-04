# QM-0054 — Inspector panel and exactness badges

## Status

Blocked

Unblocks when `QM-0053` reaches `Complete`.

## Phase

Phase 05 — Cesium model viewer

## Objective

Render the fidelity the data model already carries. `AC-010` is `Partial`
precisely because **no UI displays it**.

## Repository Evidence

* `STATUS.md` `AC-010`: *"The data model carries fidelity end to end and is
  Verified (`SRC-018`, `STAT-005`, `TILE-008`, API responses); no UI renders it
  yet."*
* `q_source::AccessScale` — access scale is a type
  (`metadata_scale_never_reads_payload`, `visualization_scale_is_never_exact`).
* `q_statistics` — `approximate` flag; `approximate_results_are_labelled`.
* `q_tiles` — `quantized_tiles_are_half_the_size_and_declare_themselves_lossy`.
* Task specification §14 — a sampled tile must never be displayed as if it held
  all exact values.

## Requirements Covered

`CESIUM-008`, `AC-010`, `MVP-24`, `MVP-06`.

## Dependencies

`QM-0053`, `QM-0020`.

## Blocks

`QM-0094`.

## Parallelization

Lane B, parallel with `QM-0055`…`QM-0057` after `QM-0053`. Coordinate on shell
layout.

## Program Boundary

`apps/web/model-viewer`, `apps/web/core/fidelity`.

## Scope

* Inspector showing: canonical address, alias, raw name, role, shape, dtype,
  layer, byte range, source shard, statistics.
* A **fidelity badge on every panel that shows a number**.
* A viewport indicator carrying the **coarsest** fidelity currently on screen.
* An explicit "fetch exact value" action, so exactness is a user choice.
* Glyph plus colour per badge, so it survives greyscale.

## Out of Scope

Hierarchy navigation (`QM-0055`) · chat (`QM-0074`) · editing.

## Files Expected to Change

* `apps/web/model-viewer/src/shell/layout.ts`

## Files Expected to Add

* `apps/web/core/fidelity/exactness.ts`
* `apps/web/model-viewer/src/inspector/{panel,badge,statistics}.ts`
* `apps/web/model-viewer/src/__tests__/badges.test.ts`

## Files Expected to Remove or Deprecate

* The local `Fidelity` type in
  `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts` — replaced by the
  shared one, re-exported for compatibility.

## Data Contracts

| Fidelity | Glyph | Meaning |
| --- | --- | --- |
| `metadata` | ▢ | Shape, dtype, address only — nothing read |
| `aggregate` | ▤ | A statistic over all of a region |
| `sampled` | ▨ | A statistic over a subset |
| `quantized` | ▩ | Values present, lossily encoded |
| `exact` | ▣ | Values as stored in the checkpoint |

**The glyph is not decoration.** A badge distinguished only by colour fails the
same accessibility test §18 applies to selection.

## Memory and Performance Constraints

Inspector updates on selection change, not per frame. Statistics are fetched once
and cached in component state.

## Implementation Plan

1. Move `Fidelity` into `apps/web/core/fidelity`; re-export from the workspace.
2. Build `badge.ts` rendering glyph + label + tooltip explaining what would
   produce a finer fidelity.
3. Build the inspector, consuming the selection from `QM-0053`.
4. Fetch statistics from `GET /v1/tensors/{id}/statistics`; show `approximate` as
   `sampled`.
5. Add the viewport coarsest-fidelity indicator.
6. Add the explicit "fetch exact value" action.
7. Tests for every badge state and for the coarsest-fidelity computation.

## Error Handling

* Statistics 404 → "not computed", **not zeros**, with an offer to run a
  conversion.
* Statistics 501 → the declared gap and its requirement ID.
* A response missing `fidelity` → **refuse to render the number**. A number
  without a fidelity cannot be labelled, and an unlabelled number is the thing
  this task exists to prevent.
* An unknown fidelity value → render as unknown, never as `exact`.

## Acceptance Criteria

1. Every panel showing a number shows a badge.
2. All five badge states are reachable and screenshotted.
3. The viewport indicator shows the **coarsest** fidelity on screen.
4. A quantized tile is never labelled `exact`.
5. A response without `fidelity` does not render its number.
6. Badges are distinguishable in a greyscale screenshot.
7. Statistics 404 shows "not computed", not zeros.
8. The exact value appears only after the explicit action.
9. `AC-010` can move from `Partial` to `Verified`.

## Verification Plan

**Automated** — vitest over all badge states and the coarsest-fidelity function;
Playwright screenshots of each state.
**Manual** — greyscale screenshot review; confirm no unlabelled number exists
anywhere in the UI.

## Suggested Commands

```bash
cd apps/web && npx vitest run badges                      # introduced here
npx playwright test apps/web/model-viewer/e2e/badges.spec.ts
```

## Test Cases

| Input | Expected |
| --- | --- |
| Selected tensor, no statistics | ▢ `METADATA`; "not computed" |
| Statistics present, `approximate: false` | ▤ `AGGREGATE` |
| `approximate: true` | ▨ `SAMPLED` |
| Quantized tile value | ▩ `QUANTIZED` |
| After "fetch exact value" | ▣ `EXACT` |
| Screen showing LOD 2 and LOD 4 | Indicator shows the coarser |
| Response without `fidelity` | Number not rendered |
| Greyscale screenshot | All badges distinguishable |
| Statistics 501 | Requirement ID shown |

## Risks

| Risk | Mitigation |
| --- | --- |
| A number renders somewhere without a badge | Rendering refuses without `fidelity`; a manual sweep confirms |
| Users ignore badges | Glyph + colour + a persistent viewport indicator |
| The coarsest-fidelity computation is wrong | Unit-tested over mixed-LOD scenes |

## Completion Evidence

* Screenshots of all five badge states.
* A greyscale screenshot.
* The coarsest-fidelity test output.
* Confirmation that a `fidelity`-less response renders no number.
