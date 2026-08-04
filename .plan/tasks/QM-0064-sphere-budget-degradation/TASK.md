# QM-0064 — Sphere budget and degradation

## Status

Blocked

Unblocks when `QM-0063` reaches `Complete`.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Enforce a sphere ceiling and **degrade to an aggregate representation that says
so**, rather than silently truncating.

## Repository Evidence

* `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts:61` —
  `assertBlockIsBounded`; `refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser`
  (`GRID-005` Verified).
* `q_gltf::MAX_INSTANCES_PER_TILE = 262_144`.
* `schemas/visualization/spatial-contract.json` — `instance_ceiling: 262144`,
  the **one** number governing both subsystems (`QM-0004`).
* `ARCHITECTURE.md` §19 — do not send entire tensors to the browser.
* `QM-0063`'s measured render budgets.

## Requirements Covered

`GRID-010`.

## Dependencies

`QM-0063`.

## Blocks

`QM-0066`, `QM-0067`.

## Parallelization

Sequential after `QM-0063` — same rendering path.

## Program Boundary

`apps/web/quatricmorph-workspace`, `apps/web/core`.

## Scope

* `MAX_WORKSPACE_SPHERES` read from the contract's `instance_ceiling`.
* Above budget: render an **aggregate cell representation** — one cell per
  sub-block, sized and coloured from that sub-block's statistics — and set the
  fidelity badge to `aggregate`.
* Refuse the request before the network when the block would exceed the transfer
  ceiling.
* A status-bar note stating what is being shown and why.

## Out of Scope

Changing `MAX_INSTANCES_PER_TILE` · streaming a block progressively · LOD inside
the workspace beyond this one degradation step.

## Files Expected to Change

* `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts`
* `apps/web/quatricmorph-workspace/src/viz/mat.ts`

## Files Expected to Add

* `apps/web/quatricmorph-workspace/src/viz/aggregate-cells.ts`
* `apps/web/quatricmorph-workspace/src/viz/__tests__/budget.test.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```text
MAX_WORKSPACE_SPHERES  = instance_ceiling = 262_144    # from the contract
DEFAULT_BLOCK          = 256 × 256 = 65_536
MAX_BLOCK_REQUEST_BYTES = 4 MiB
```

Above budget, the aggregate representation renders `ceil(rows/f) × ceil(cols/f)`
cells for the smallest integer `f` bringing the count under budget, and the badge
becomes `aggregate`. **The user is told the factor**, so the picture is
interpretable rather than merely smaller.

## Memory and Performance Constraints

The two ceilings are deliberately equal, so **no pipeline stage can produce
something a later stage must reject**. A tile too large to render in the
workspace is also too large to emit as a GLB.

Aggregate rendering must meet the same frame budget as a full block of the same
cell count.

## Implementation Plan

1. Read `instance_ceiling` from the contract; assert it equals
   `MAX_INSTANCES_PER_TILE` in the conformance test.
2. Extend `assertBlockIsBounded` to check the sphere count as well as bytes.
3. Implement `aggregate-cells.ts` computing per-sub-block statistics.
4. Choose the smallest `f` bringing the count under budget.
5. Set the fidelity to `aggregate`; add the status-bar note with `f`.
6. Tests: at, just over, and far over budget.

## Error Handling

* A block exceeding the transfer ceiling → **refused before the network**, with
  a message suggesting a smaller extent.
* A block within transfer but over the sphere budget → aggregated, badged, and
  announced.
* Aggregation with no statistics available → refuse rather than aggregate raw
  values into a mean that is not what the badge claims.
* **Never truncate.** Showing the first 262 144 of 400 000 cells without saying so
  is the failure this task prevents.

## Acceptance Criteria

1. A 512×512 block (262 144) renders fully at the ceiling.
2. A 600×600 block (360 000) renders aggregated with `f = 2`, badged
   `aggregate`, and the factor is stated.
3. A block exceeding `MAX_BLOCK_REQUEST_BYTES` is refused **before** any network
   call.
4. **No case truncates.** Asserted by comparing the rendered cell count against
   `ceil(rows/f) × ceil(cols/f)`.
5. The badge reads `aggregate` whenever `f > 1`.
6. `MAX_WORKSPACE_SPHERES == MAX_INSTANCES_PER_TILE`, asserted.
7. Aggregate rendering meets the frame budget for its cell count.
8. `refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser` still passes.

## Verification Plan

**Automated** — vitest at, over, and far over budget; the ceiling-equality
assertion.
**Manual** — screenshots of a full block and an aggregated one, with the note
visible.

## Suggested Commands

```bash
cd apps/web && npx vitest run budget                    # introduced here
npm run dev --workspace quatricmorph-workspace
```

## Test Cases

| Input | Expected |
| --- | --- |
| 256×256 (65 536) | Full render, `exact` |
| 512×512 (262 144) | Full render at the ceiling |
| 600×600 (360 000) | `f = 2`, 90 000 cells, `aggregate` |
| 4096×4096 | Refused before the network |
| Rendered cell count vs formula | Equal — **no truncation** |
| `f > 1` | Badge reads `aggregate`; factor stated |
| Ceiling constants | Equal in both subsystems |
| Aggregate at 65 536 cells | Same frame budget as a full block |

## Risks

| Risk | Mitigation |
| --- | --- |
| Truncation creeps in as an optimization | Cell count asserted against the formula |
| An aggregate view is mistaken for exact data | Badge plus a stated factor |
| The two ceilings drift apart | One contract value; equality asserted in conformance |

## Completion Evidence

* Cell-count assertions at all four sizes.
* Screenshots of full and aggregated renders with the status note.
* The ceiling-equality assertion output.
* Frame timing for aggregate rendering.
