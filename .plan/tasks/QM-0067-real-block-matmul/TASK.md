# QM-0067 — Real-block `A @ B` with full controls

## Status

Blocked

Unblocks when `QM-0066` reaches `Complete`.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Multiply two real tensor blocks on the shared grid, animate it deterministically,
and fix `mm`'s disposal defect while doing so.

## Repository Evidence

* `apps/web/quatricmorph-workspace/src/math/matmul.ts` — 17 tests covering every
  required shape combination including the `2×3 @ 2×2` rejection (`MATMUL-001`).
* `math/animation-schedule.ts` — the cursor logic as a **pure state machine**,
  7 tests (`MATMUL-003`).
* `math/blocking.ts` — 10 tests (`MATMUL-002`).
* `q_gpu::CpuBackend` matmul — `hand_computed_matmul_2x3_by_3x2` (`MATMUL-004`).
* `grid-ruler.ts:140` `placeOperands` — `mm`'s proven placement, all on-grid.
* `docs/CURRENT_ARCHITECTURE.md` §8 defect 4: *"`disposeAndClear` disposes
  geometries but **not materials or textures**."*

## Requirements Covered

`MATMUL-006`, `MVP-28`, `MVP-30`, `MVP-31`.

## Dependencies

`QM-0066`, `QM-0065`, `QM-0064`.

## Blocks

`QM-0080`.

## Parallelization

Sequential after `QM-0066`.

## Program Boundary

`apps/web/quatricmorph-workspace/src/{viz,interaction,gui}`.

## Scope

* Assign fetched blocks to A and B; validate shapes **before** any compute.
* Compute `C = A @ B` in the browser below 512³; delegate to the daemon above.
* Drive the animation from `animation-schedule.ts`.
* Controls: play, pause, step, previous, reset calculation, reset view, fit.
* **Fix disposal** to release materials and textures, not only geometries.

## Out of Scope

Server-side matmul execution (`QM-0070`) · CUDA · expressions beyond `A @ B` ·
hover metadata (`QM-0068`).

## Files Expected to Change

* `apps/web/quatricmorph-workspace/src/viz/matmul.ts`
* `apps/web/quatricmorph-workspace/src/interaction/animation.ts`
* `apps/web/quatricmorph-workspace/src/gui/mvp-gui.ts`
* `apps/web/quatricmorph-workspace/src/util/objects.ts` — `disposeAndClear`

## Files Expected to Add

* `apps/web/quatricmorph-workspace/src/tensor/operands.ts`
* `apps/web/quatricmorph-workspace/src/__tests__/real-matmul.test.ts`
* `apps/web/quatricmorph-workspace/e2e/matmul-controls.spec.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```text
A ∈ R^(m×k) on I×K     B ∈ R^(k×n) on K×J     C ∈ R^(m×n) on I×J
C[i,j] = Σ_k A[i,k] × B[k,j]
X → J,  Y → I,  Z → K
```

Animation sequence per selected `C[i,j]`: highlight `A[i,:]` → highlight
`B[:,j]` → highlight shared `k` → show `A[i,k] × B[k,j]` → update the running sum
→ reveal `C[i,j]` → advance.

**`previous` must be exactly `forward`'s inverse** — which is why the schedule is
a pure function of a step index rather than an accumulating mutation.

## Memory and Performance Constraints

* In-browser matmul up to 512³ (~1.34×10⁸ MAC), budget < 1.5 s. Above that,
  delegate.
* Animation step < 16 ms.
* Disposal must return `renderer.info.memory` — **geometries, textures, and
  programs** — to baseline over 100 re-initializations.

## Implementation Plan

1. `operands.ts`: assign, transpose, and validate; reject incompatible shapes
   **before** any compute.
2. Place A, B, C with `placeOperands`; assert every position is snapped.
3. Compute with `math/matmul.ts` below the threshold; call the daemon above.
4. Wire the schedule to the highlight calls; `previous` recomputes from the index.
5. Build the seven controls.
6. Rewrite `disposeAndClear` to dispose materials and textures; track every
   created resource.
7. Tests, including a 100-re-init memory soak.

## Error Handling

* Shape mismatch → **rejected before any compute**, naming both shapes.
* A block fetch failing → the operand slot stays empty; **no zeros**.
* A matmul above the delegation threshold with the daemon unavailable → refuse
  with the reason, and offer a smaller region.
* Cancellation mid-compute → operands remain; C is cleared and marked incomplete.

## Acceptance Criteria

1. Two fetched 256×256 blocks multiply; C matches the CPU reference to `1e-5`.
2. All six required shape combinations work; `2×3 @ 2×2` is rejected **before
   compute**.
3. Every operand and cell position is snapped within `1e-6`.
4. Play, pause, step, previous, reset calculation, reset view, and fit all work.
5. Stepping to *n*, resetting, and stepping to *n* again yields an identical
   state.
6. `previous` from *n* yields exactly the state at *n−1*.
7. 512³ completes in < 1.5 s; above that it delegates.
8. **100 re-initializations return geometries, textures, and programs to
   baseline.**
9. Animation steps in < 16 ms.

## Verification Plan

**Automated** — vitest for operand validation and determinism; Playwright for the
controls and the memory soak.
**Manual** — compare a computed `C[i,j]` against a hand calculation from
`golden.json` values.

## Suggested Commands

```bash
cd apps/web && npx vitest run real-matmul                                  # new
npx playwright test apps/web/quatricmorph-workspace/e2e/matmul-controls.spec.ts   # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| Two real 256×256 blocks | C matches the CPU reference to `1e-5` |
| `2×3 @ 3×2` | 2×2 result |
| `1×3 @ 3×1` | 1×1 result |
| `2×3 @ 2×2` | Rejected before compute, both shapes named |
| Step to 50, reset, step to 50 | Identical state |
| Step to 50, then `previous` | Exactly state 49 |
| 512³ | < 1.5 s |
| 1024³ | Delegated to the daemon |
| Daemon unavailable above threshold | Refused with a suggestion |
| 100 re-inits | Geometries, textures, programs at baseline |
| Every operand position | Snapped within `1e-6` |

## Risks

| Risk | Mitigation |
| --- | --- |
| `previous` diverges from `forward` | The schedule is a pure function of an index; asserted |
| Textures leak (the `mm` defect) | Explicitly fixed; asserted by the soak |
| Browser matmul blocks the UI | Threshold at 512³; above it, delegate |

## Completion Evidence

* C versus CPU-reference comparison.
* Determinism output for step/reset/step and for `previous`.
* `renderer.info.memory` across 100 re-inits.
* Timing at 256³ and 512³.
* A screen recording or frame sequence of the animation.
