# QM-0068 — Hover and selection metadata contract

## Status

Blocked

Unblocks when `QM-0067` reaches `Complete`.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Make hover and selection carry the full metadata contract, conveyed by more than
colour.

## Repository Evidence

* `apps/web/quatricmorph-workspace/src/interaction/selection.ts` — ported from `mm`;
  6 tests. Uses the `raycaster.far` toggle idiom (`0` ↔ `Infinity`) documented in
  `docs/CURRENT_ARCHITECTURE.md` §1 as *"a non-obvious idiom"*.
* `mm/index.html:455-466` `updateSpotlight`; `468-481` `requestLabelUpdate`
  coalescing into an animation frame — both ported.
* `viz/mat.ts:403` — `raycaster.params.Points.threshold` for picking.
* Task specification §18: selection must **not** rely on colour alone; the listed
  channels are scale, outline, brightness, guide thickness, opacity, frame
  emphasis, animated path.

## Requirements Covered

`GRID-012`.

## Dependencies

`QM-0067`, `QM-0054`.

## Blocks

`QM-0080`.

## Parallelization

Last Phase 06 task — it touches hover paths several earlier tasks modify.

## Program Boundary

`apps/web/quatricmorph-workspace/src/interaction`.

## Scope

* Hover tooltip with the full nine-field contract.
* Selection using **at least two** non-colour channels.
* Row, column, and block selection in addition to single cells.
* Keep the coalesced label update; keep the `raycaster.far` idiom, documented at
  its use site.

## Out of Scope

Viewer picking (`QM-0053`) · the inspector panel (`QM-0054`) · editing values.

## Files Expected to Change

* `apps/web/quatricmorph-workspace/src/interaction/selection.ts`
* `apps/web/quatricmorph-workspace/src/viz/mat.ts`

## Files Expected to Add

* `apps/web/quatricmorph-workspace/src/interaction/hover-card.ts`
* `apps/web/quatricmorph-workspace/src/interaction/__tests__/hover.test.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```text
canonical address    model.layers[10].self_attention.query_projection.weight
alias                Q[10]
logical index        [1031, 1802]
block index          block (4, 7) of (16, 16)
value                0.006408154
shape · dtype        [4096, 4096] · F32
fidelity             ▣ EXACT
source shard         model-00002-of-00002.safetensors @ 419928
```

All nine fields. `fidelity` uses the shared vocabulary from `QM-0054`;
`source shard` comes from the block's provenance, so a user can verify a value
independently with `q-cli`.

## Memory and Performance Constraints

* Hover updates coalesce into one animation frame — the `mm` behaviour, kept.
* The hover card is **one** DOM element, reused; not one per cell.
* Hover must not trigger a fetch. Values already in the block are shown; anything
  not present is shown as unavailable, not fetched.

## Implementation Plan

1. Extend the hover payload to all nine fields, sourced from the block's
   provenance and the tensor's catalog record.
2. Build the reused hover card.
3. Implement selection highlighting with outline + scale bump, and frame emphasis
   for row, column, and block selections.
4. Add row, column, and block selection modes.
5. Keep the coalesced update and the `raycaster.far` idiom, with a comment
   explaining it at the use site.
6. Tests, including a greyscale-legibility check.

## Error Handling

* Hovering an empty cell → the card shows the index and "no data", not a zero.
* A missing canonical address → the raw name and `unknown`.
* A hover during an in-flight fetch → shows what is known; **never fetches**.
* Selection cleared on Escape and on clicking empty space.

## Acceptance Criteria

1. Hover shows all nine fields.
2. Selection is conveyed by **at least two** non-colour channels.
3. Selection is legible in a **greyscale** screenshot.
4. Row, column, block, and cell selection all work.
5. Hover triggers no network request — asserted by a request log.
6. Hover updates coalesce into one frame; no update storm while moving.
7. The hover card is a single reused DOM element.
8. An empty cell shows "no data", not `0`.
9. Escape clears the selection.

## Verification Plan

**Automated** — vitest for the payload and the coalescing; Playwright for hover,
selection, greyscale legibility, and the request log.
**Manual** — hover across a block; confirm no flicker and no lag.

## Suggested Commands

```bash
cd apps/web && npx vitest run hover                                # introduced here
npx playwright test apps/web/quatricmorph-workspace/e2e/hover.spec.ts
```

## Test Cases

| Input | Expected |
| --- | --- |
| Hover a cell with data | All nine fields |
| Hover an empty cell | "No data", not `0` |
| Hover, request log | **Zero** requests |
| Select a cell | Two non-colour channels change |
| Greyscale screenshot | Selection visible |
| Select a row | Whole row emphasised |
| Select a block | Frame emphasised |
| Rapid mouse movement | One update per frame |
| DOM node count during hover | Constant |
| Escape | Selection cleared |

## Risks

| Risk | Mitigation |
| --- | --- |
| Hover triggers fetches and thrashes the daemon | Request log asserted empty during hover |
| Selection readable only in colour | Two channels; greyscale screenshot is an acceptance criterion |
| The `raycaster.far` idiom is "cleaned up" and breaks picking | Documented at the use site with the reason |

## Completion Evidence

* Screenshot of the hover card with all nine fields.
* Greyscale screenshot with a visible selection.
* Request log during a hover session, empty.
* DOM-node count during hover.
