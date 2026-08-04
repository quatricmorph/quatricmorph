# QM-0083 — CUDA device-memory soak

## Status

Deferred

Not in v1 — post-v1 **CUDA accelerator lane**, requiring an RTX 3090. v1's GPU lane is Metal (`QM-0126`, `QM-0127`). See [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4.

## Phase

Phase 08 — Integration and performance (Lane E)

## Objective

Prove 10 000 block jobs return device memory to its starting value.

## Repository Evidence

* `crates/q-cuda/src/lib.rs:43,47` — `RTX_3090_VRAM_BYTES`,
  `USABLE_VRAM_FRACTION = 0.80`; `check_workload` enforces the ceiling.
* `the_vram_ceiling_is_enforced_without_a_device` (`CUDA-006` Verified) — the
  arithmetic only.
* `QM-0034` introduces RAII device buffers; `QM-0036` verifies OOM adaptation.
* [`CUDA_ARCHITECTURE.md`](../../CUDA_ARCHITECTURE.md) §7: *"Device memory is
  freed on every exit path, including panic."*

## Requirements Covered

`CUDA-009`, `MVP-42`.

## Dependencies

`QM-0036`.

## Blocks

`QM-0094` (or a waiver).

## Parallelization

Lane E. Blocks nothing on the critical path. **Requires: RTX 3090.**

## Program Boundary

`crates/q-cuda` — tests only.

## Scope

* 10 000 sequential block jobs, sampling `cudaMemGetInfo` every 100.
* Cancellation soak: 1 000 jobs cancelled mid-flight.
* Panic soak: 100 jobs panicking inside the kernel launch path.
* OOM soak: 100 jobs hitting the ceiling and adapting.

## Out of Scope

Browser memory (`QM-0082`) · host RSS (`QM-0084`) · kernel correctness
(`QM-0035`, `QM-0036`).

## Files Expected to Change

None.

## Files Expected to Add

* `crates/q-cuda/tests/memory_soak.rs` — gated on the `cuda` feature

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
{ "scenario": "sequential-blocks", "iterations": 10000,
  "free_bytes_start": 25165824000, "free_bytes_end": 25165824000,
  "delta_bytes": 0, "peak_used_bytes": 1572864, "verdict": "pass" }
```

**`delta_bytes` must be exactly 0.** Device memory is not garbage-collected;
there is no timing excuse for a non-zero delta, so unlike the browser soak this
one admits no tolerance.

## Memory and Performance Constraints

* Peak device use ≤ `MAX_GPU_INPUT_BYTES + MAX_GPU_OUTPUT_BYTES`.
* 10 000 jobs in under 30 minutes; if slower, the block size is reduced and the
  count kept.

## Implementation Plan

1. Sample `cudaMemGetInfo` before the loop.
2. Run 10 000 block statistics jobs, sampling every 100.
3. Sample after; assert `delta_bytes == 0`.
4. Cancellation soak: cancel at a random point (deterministically seeded) 1 000
   times.
5. Panic soak: inject a panic after allocation, before free; assert RAII frees.
6. OOM soak: constrain the budget so adaptation triggers; assert no accumulation.
7. Emit the report.

## Error Handling

* Non-zero `delta_bytes` → **fail, naming the scenario and the delta**. A leak
  here eventually kills a long conversion.
* `cudaErrorIllegalAddress` → fail immediately; the context is corrupt and later
  measurements are meaningless.
* Device lost → fail, reporting the iteration reached.
* No device → **skip with a message**, never a false pass.

## Acceptance Criteria

1. 10 000 sequential jobs: `delta_bytes == 0`.
2. 1 000 cancelled jobs: `delta_bytes == 0`.
3. 100 panicking jobs: `delta_bytes == 0` — RAII frees on the panic path.
4. 100 OOM-adapting jobs: `delta_bytes == 0`.
5. Peak device use stays within the configured budget throughout.
6. Sampling every 100 shows **no monotone growth**.
7. Under 30 minutes.
8. Skips cleanly with a message when no device is present.

## Verification Plan

**Automated** — `memory_soak.rs` on the RTX 3090.
**Manual** — `nvidia-smi` before and after; review the sample curve.

## Suggested Commands

```bash
cargo test -p q-cuda --features cuda --test memory_soak -- --nocapture   # new
nvidia-smi --query-gpu=memory.used,memory.free --format=csv -l 5
```

## Test Cases

| Scenario | Iterations | Assertion |
| --- | --- | --- |
| Sequential blocks | 10 000 | `delta_bytes == 0` |
| Cancelled mid-flight | 1 000 | `delta_bytes == 0` |
| Panic after allocation | 100 | `delta_bytes == 0` |
| OOM adaptation | 100 | `delta_bytes == 0` |
| Sample curve | every 100 | No monotone growth |
| Peak device use | — | Within budget |
| No device present | — | Skipped with a message |

## Risks

| Risk | Mitigation |
| --- | --- |
| No RTX 3090 is available | `MVP-42` takes a written waiver; `STATUS.md` keeps `CUDA-009` unverified |
| Another process on the GPU perturbs the measurement | Record `nvidia-smi` before and after; require an otherwise idle device |
| A leak appears only after far more than 10 000 jobs | The sample curve detects a trend, not just the endpoint |

## Completion Evidence

* The soak report for all four scenarios.
* `nvidia-smi` before and after.
* The sample curve across 10 000 iterations.
* Device name and driver version.
