# QM-0035 — CUDA reduction and histogram differential verification

## Status

Blocked

Unblocks when `QM-0034` reaches `Complete`. **Requires: RTX 3090.**

## Phase

Phase 03 — Block runtime and compute (Lane E)

## Objective

Prove `reduce.cu` and `histogram.cu` produce the same numbers as
`q_gpu::CpuBackend`, on real hardware, within documented tolerances.

## Repository Evidence

* `gpu/cuda/reduce.cu`, `gpu/cuda/histogram.cu` — **never compiled or executed**;
  `STATUS.md` `CUDA-002`, `CUDA-003` test column: *"none — never compiled or
  executed."*
* `gpu/cuda/README.md`: *"Diff every kernel's output against `q_gpu::CpuBackend`
  … The CPU backend is the reference; a divergence is a bug in the kernel."*
* `q_gpu::CpuBackend` — `GPU-002` Verified, 7 tests, hand-computed expectations.
* `q_statistics` — Welford; `welford_stays_accurate_where_the_naive_formula_collapses`.

## Requirements Covered

`CUDA-002`, `CUDA-003`, `CUDA-008`, `MVP-10`, `MVP-12`.

## Dependencies

`QM-0034`, `QM-0003`.

## Blocks

`QM-0036`.

## Parallelization

Lane E. Blocks nothing on the critical path. **Requires: RTX 3090.**

## Program Boundary

`crates/q-cuda`, `gpu/cuda/`.

## Scope

* Differential tests over the large fixture: min, max, sum, sum-of-squares, mean,
  variance, L1, L2, zero/positive/negative ratios, 64-bin histogram.
* dtypes f32, f16, bf16.
* Block dimensions 64, 128, 256, 512.
* Determinism: the same block run 100 times must be **bit-identical**.
* Kernel benchmarks against the CPU on identical input.

## Out of Scope

Quantization, Morton, matmul (`QM-0036`) · leak soak (`QM-0083`) · optimization
beyond correctness.

## Files Expected to Change

* `gpu/cuda/reduce.cu`, `gpu/cuda/histogram.cu` — fixes found by running them
* `crates/q-cuda/src/lib.rs`

## Files Expected to Add

* `crates/q-cuda/tests/differential_reduction.rs` — gated on the `cuda` feature

## Files Expected to Remove or Deprecate

None.

## Data Contracts

Tolerances from [`CUDA_ARCHITECTURE.md`](../../CUDA_ARCHITECTURE.md) §6:

| Operation | Tolerance |
| --- | --- |
| min, max | **exact** — comparison, not arithmetic |
| zero/positive/negative ratios | **exact** — counting |
| histogram bin counts | **exact** — counting, given identical edges |
| sum, L1, mean, variance, L2 | relative `1e-6` |

**Exact means exact.** A difference in min or max is a bug, not a rounding
artifact, and treating it as tolerable would hide a real defect.

## Memory and Performance Constraints

Buffers per §5 of the memory budget. Fast-math **off** — `-use_fast_math` would
make `1e-6` unachievable and render the CPU reference useless as a check.

No `atomicAdd(float*)` in any reduction: it makes results order-dependent and
therefore non-deterministic. Histogram atomics are **integer**, which are
order-independent.

## Implementation Plan

1. Wire the kernels to `CudaBackend`'s `Backend` implementation.
2. Write the differential harness: read a block via `BlockStream`, run both
   backends, compare per tolerance.
3. Parameterize over dtype × block dimension.
4. Add the 100-run determinism test.
5. Add benchmarks recording host→device, kernel, device→host, total, GB/s, and
   the CPU baseline.
6. Fix every divergence **in the kernel**, per the README's rule.

## Error Handling

* A divergence beyond tolerance → **fail the test naming both values, the block
  ID, the dtype, and the block dimension**. Never widen a tolerance to pass.
* A launch failure → propagate with kernel name and launch geometry.
* `cudaErrorIllegalAddress` → fail, do not retry.
* No device → the test **skips with a message**, never a false pass.

## Acceptance Criteria

1. All 11 statistics match the CPU reference for f32 at all four block
   dimensions.
2. f16 and bf16 match within tolerance.
3. min, max, ratios, and histogram counts match **exactly**.
4. 100 runs of one block are bit-identical.
5. A deliberately perturbed kernel makes the test fail.
6. Benchmarks recorded with device name and driver version.
7. `STATUS.md` `CUDA-002` and `CUDA-003` may move off `Hardware-Unverified` —
   **only after this passes on real hardware.**

## Verification Plan

**Automated** — `cargo test -p q-cuda --features cuda --test differential_reduction`
on the RTX 3090.
**Manual** — `nvidia-smi` capture; benchmark table reviewed.

## Suggested Commands

```bash
cargo test -p q-cuda --features cuda --test differential_reduction   # new, RTX 3090
cargo bench -p q-cuda --features cuda                                 # new
nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv
```

## Test Cases

| Input | Expected |
| --- | --- |
| 256×256 f32 block | All 11 statistics match |
| Same at 64, 128, 512 | Match at every dimension |
| f16 block | Match within `1e-6` |
| bf16 block | Match within `1e-6` |
| Block of all zeros | `zero_ratio = 1.0`, exactly |
| Block with NaN | `non_finite` counted, both backends agree |
| 100 runs, same block | Bit-identical |
| Perturbed kernel | Test fails naming both values |
| No device | Skipped with a message |

## Risks

| Risk | Mitigation |
| --- | --- |
| A tolerance is widened to make a test pass | Tolerances are stated in the architecture document and reviewed; exact means exact |
| Kernels have never run and may be substantially wrong | Expected; fixing them is the task |
| No hardware is ever available | The requirement stays `Hardware-Unverified`; the MVP is unaffected |

## Completion Evidence

* Full differential test output, all parameterizations.
* `nvidia-smi` output naming the card and driver.
* The 100-run determinism result.
* Benchmark table: host→device, kernel, device→host, GB/s, CPU baseline, speedup.
* The perturbed-kernel failure output.
