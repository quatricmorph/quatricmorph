# QM-0061 — Axis binding: rank ≤ 3 implemented, rank > 3 refuses

## Status

Blocked

Unblocks when `QM-0060` reaches `Complete`.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Map tensor axes to world axes for rank 0–3, and **refuse rank > 3 rather than
flattening it**.

## Repository Evidence

* `q_source::TensorDescriptor::shape: Vec<u64>` — arbitrary rank already.
* `q_tensor_runtime::BlockExtent` — **2-D only**: row and column bounds.
* `q_tiles::QTileHeader` — `dimensions: u8` and `[u32;3]` origin/extent allow 3,
  but `for_block` hard-codes `dimensions: 2` and `extent[2] = 1`.
* `grid-ruler.ts` — `depthSpacing` exists in the config and is **unused** (`0`).
* `schemas/nsir/schema.json` — records named axes (`output_channel`,
  `input_channel`).
* [`ADR-010`](../../../docs/decisions/ADR-010-tensor-rank-ceiling.md) — **accepted**,
  not a recommendation: rank ≤ 3 implemented, rank > 3 returns `NotImplemented`
  carrying `GRID-007`. The ceiling is already frozen in the spatial contract by
  `QM-0004` and asserted at gate G1; this task implements it.

## Requirements Covered

`GRID-007`.

## Dependencies

`QM-0060`.

## Blocks

`QM-0065`, `QM-0040` (depth extent).

## Parallelization

Parallel with `QM-0062`, `QM-0065` after `QM-0060`.

## Program Boundary

`apps/web/core/spatial/axes.ts`; `q_tensor_runtime::BlockExtent` gains an
optional depth extent.

## Scope

* `bindAxes(shape, role) -> Result<AxisBinding[]>` for rank 0, 1, 2, 3.
* Rank 3: axis 0 → facet along Z at `depthSpacing`; axes 1, 2 → Y, X.
* **Rank > 3 returns `NotImplemented` carrying `GRID-007`.**
* Extend `BlockExtent` with an optional depth extent defaulting to `1`, so all
  2-D behaviour and its tests are unchanged.
* Use NSIR axis names as labels where known; positional indices where `unknown`.

## Out of Scope

Implementing rank > 3 · `.qtile` v2 · the meta-grid facet layout, which is
designed but not built.

## Files Expected to Change

* `crates/q-tensor-runtime/src/lib.rs` — `BlockExtent` depth
* `apps/web/matrix-workspace/src/layout/tensor-frame.ts`

## Files Expected to Add

* `apps/web/core/src/spatial/axes.ts`
* `apps/web/core/src/__tests__/axes.test.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```ts
type AxisBinding = {
  tensorAxis: number
  role: 'row' | 'column' | 'depth' | 'facet'
  worldAxis: 'X' | 'Y' | 'Z' | null
  facet: { stride: number; wrap: number } | null
  label: string          // NSIR axis name, or "axis N"
}
```

| Rank | Binding |
| --- | --- |
| 0 | One cell at the anchor; framed and labelled |
| 1 | Axis 0 → row **or** column by operand role |
| 2 | Axis 0 → Y (I), axis 1 → X (J); row-major |
| 3 | Axis 0 → facet along Z; axes 1, 2 → Y, X |
| > 3 | **`NotImplemented` / `GRID-007`** |

The designed rank > 3 rule — display pair, optional depth, facet set laid out as
a grid of grids using the same ruler one level up — is documented in
[`GRID_ARCHITECTURE.md`](../../GRID_ARCHITECTURE.md) §3.3 and **not implemented**.

## Memory and Performance Constraints

Pure index arithmetic, no allocation per cell. A rank-3 tensor with H facets
renders H aligned planes; the sphere budget applies to the **total**, not per
facet.

## Implementation Plan

1. Define the types in `core/spatial/axes.ts`.
2. Implement rank 0, 1, 2, 3.
3. Return `NotImplemented` with `GRID-007` above rank 3 — **no flattening, no
   reshaping, no taking the first three axes.**
4. Add the optional depth extent to `BlockExtent`, defaulting to `1`.
5. Use NSIR axis names for labels; fall back to positional.
6. Tests for every rank including the refusal.

## Error Handling

* Rank > 3 → `NotImplemented` naming `GRID-007` and stating that flattening is
  deliberately not done.
* A shape containing 0 → error; a zero-extent tensor has nothing to render.
* A rank-1 tensor with no operand role → defaults to a column vector, documented.

## Acceptance Criteria

1. Rank 0, 1, 2, 3 all produce valid bindings.
2. A rank-3 `[H, m, n]` tensor renders H aligned planes separated by
   `depthSpacing`.
3. A rank-4 tensor returns `NotImplemented` carrying `GRID-007`.
4. **No code path flattens, reshapes, or truncates a rank > 3 shape.**
5. `BlockExtent` with default depth behaves identically to today; all
   `q-tensor-runtime` tests pass unchanged.
6. Axis labels use NSIR names where known.
7. Rank-1 renders as a row or column by operand role.

## Verification Plan

**Automated** — vitest for every rank and the refusal; Rust tests confirming
`BlockExtent` compatibility.
**Manual** — render a synthetic rank-3 tensor and confirm the facets align.

## Suggested Commands

```bash
cd apps/web && npx vitest run axes                # introduced here
cargo test -p q-tensor-runtime                     # verified today
```

## Test Cases

| Input | Expected |
| --- | --- |
| `[]` (scalar) | One cell at the anchor |
| `[128]`, role left operand | Row vector along X |
| `[128]`, role right operand | Column vector along Y |
| `[256, 256]` | Axis 0 → Y, axis 1 → X |
| `[8, 256, 256]` | 8 facets along Z at `depthSpacing` |
| `[2, 8, 256, 256]` | **`NotImplemented` / `GRID-007`** |
| `[0, 256]` | Error |
| NSIR axes known | Labels are `output_channel`, `input_channel` |
| NSIR `unknown` | Labels are `axis 0`, `axis 1` |
| All existing `BlockExtent` tests | Pass unchanged |

## Risks

| Risk | Mitigation |
| --- | --- |
| Someone "helpfully" flattens rank > 3 later | An explicit test asserts the refusal, and the reason is documented at the call site |
| Adding depth to `BlockExtent` breaks 2-D behaviour | Optional, defaults to 1; all existing tests must pass unchanged |
| Rank-3 facets blow the sphere budget | Budget applies to the total; `QM-0064` enforces it |

## Completion Evidence

* Test output for every rank including the refusal message.
* A screenshot of a rank-3 tensor rendering as aligned facets.
* Confirmation that all `q-tensor-runtime` tests pass unchanged.
