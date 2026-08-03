# QM-0034 — CUDA build integration, feature-gated

## Status

Blocked

Unblocks when `QM-0005` reaches `Complete`. **Requires: CUDA toolkit** (a device is not needed to compile).

## Phase

Phase 03 — Block runtime and compute (Lane E)

## Objective

Compile and link the four `.cu` files behind a feature flag that is **off by
default**, so `cargo build --workspace` keeps working on a machine with no CUDA.

## Repository Evidence

* `gpu/cuda/{reduce,histogram,matmul,quantize}.cu` — **never compiled**.
* `gpu/cuda/README.md`: *"There is no `nvcc` step in the build, no `build.rs`,
  and no FFI binding."* Prescribes `nvcc -arch=sm_86`.
* `crates/q-cuda/src/lib.rs:51` — `KERNEL_SOURCES` lists the files as **data**.
* `:43` `RTX_3090_VRAM_BYTES`, `:47` `USABLE_VRAM_FRACTION = 0.80`, `:93`
  `is_available()`.
* `CUDA-006` Verified — `the_vram_ceiling_is_enforced_without_a_device`.
* `.github/workflows/build.yaml` — no CUDA job, deliberately.
* `ADR-CANDIDATE-002` — recommended default: `build.rs` + `nvcc`, feature off.

## Requirements Covered

`CUDA-007`, `CUDA-001`, `MVP-11`.

## Dependencies

`QM-0005`.

## Blocks

`QM-0035`, `QM-0036`, `QM-0037`, `QM-0083`.

## Parallelization

Lane E — **blocks nothing on the critical path**. Fully parallel with all of
Lane A.

## Program Boundary

`crates/q-cuda`, `gpu/cuda/`.

## Scope

* `build.rs` compiling the four `.cu` files with `-arch=sm_86`, **no fast-math**.
* `src/ffi.rs` — `extern "C"` declarations and RAII device-buffer wrappers.
* Device discovery: driver present, device count, compute capability ≥ 8.6,
  `cudaMemGetInfo`, runtime/driver compatibility.
* Implement `q_gpu::Backend` for `CudaBackend` behind the feature.
* Keep the refusal path when the feature is off.

## Out of Scope

Differential verification (`QM-0035`, `QM-0036`) · kernel optimization · a CI
GPU job · Metal.

## Files Expected to Change

* `crates/q-cuda/Cargo.toml` — the `cuda` feature
* `crates/q-cuda/src/lib.rs`
* `gpu/cuda/README.md` — update the "how to make these verified" steps
* `gpu/cuda/*.cu` — only if compilation reveals errors

## Files Expected to Add

* `crates/q-cuda/build.rs`
* `crates/q-cuda/src/ffi.rs`

## Files Expected to Remove or Deprecate

None. `every_operation_refuses_with_a_requirement_id_rather_than_faking_output`
**must keep passing with the feature off** — that is the fallback's evidence.

## Data Contracts

```rust
#[repr(C)] pub struct BlockStats {
    count: u64, min: f32, max: f32, sum: f64, sum_sq: f64,
    zeros: u64, positives: u64, negatives: u64, non_finite: u64,
}
```

`#[repr(C)]`, field order identical to the `.cu` struct, asserted by a
`size_of`/`offset_of` test — a layout mismatch produces silently wrong numbers,
which is the worst failure available here.

## Memory and Performance Constraints

Buffers per [`MEMORY_BUDGET.md`](../../MEMORY_BUDGET.md) §5:
`MAX_GPU_INPUT_BYTES` 2 GiB, `MAX_GPU_OUTPUT_BYTES` 512 MiB, `MAX_PINNED_BYTES`
128 MiB, `MAX_CONCURRENT_BLOCKS` 4. `check_workload` runs **before** any
allocation.

Device memory is freed on **every** exit path, including panic. RAII wrappers,
no `mem::forget`.

## Implementation Plan

1. Add the `cuda` feature, default off.
2. `build.rs`: locate `nvcc` (`CUDA_PATH` or `PATH`); compile each `.cu` to an
   object; archive; emit link directives. **Skip entirely when the feature is
   off.**
3. Fix any compilation errors in the `.cu` sources — they have never been
   compiled, so errors are expected.
4. `ffi.rs`: `extern "C"` declarations, RAII buffers, error-code mapping.
5. Device discovery in order, failing closed.
6. Implement `Backend` behind `#[cfg(feature = "cuda")]`.
7. Struct-layout assertion test.

## Error Handling

* `nvcc` absent with the feature on → **build error naming the tool and the
  environment variable**, not a silent skip.
* Compute capability < 8.6 → refused by name.
* `cudaErrorMemoryAllocation` → halve the block, retry, floor 64×64, then fail
  naming the budget.
* `cudaErrorIllegalAddress` → **fail the job, do not retry.** The context is
  corrupt and any further result is untrustworthy.
* Feature off → `NotImplemented` with `CUDA-001`, exactly as today.

## Acceptance Criteria

1. `cargo build --workspace` succeeds with **no CUDA toolkit installed**.
2. `cargo test --workspace` still reports 290+ with the feature off.
3. `cargo build -p q-cuda --features cuda` compiles all four `.cu` files.
4. The struct-layout test passes.
5. Device discovery correctly reports absence, capability, and free VRAM.
6. `check_workload` refuses a workload exceeding the ceiling **before**
   allocating.
7. With the feature off, every operation still refuses with `CUDA-001`.
8. `gpu/cuda/README.md` reflects reality after this task.

## Verification Plan

**Automated** — the default build and test on a CUDA-free machine; the layout
test; the feature build where a toolkit exists.
**Manual** — `nvcc --version`, `nvidia-smi`, and the build log, recorded.

## Suggested Commands

```bash
cargo build --workspace                                # verified today; must keep working
cargo test --workspace
cargo build -p q-cuda --features cuda                  # introduced here
nvcc --version && nvidia-smi
```

## Test Cases

| Input | Expected |
| --- | --- |
| Default build, no toolkit | Succeeds |
| Default test | 290+ pass; CUDA refuses with `CUDA-001` |
| `--features cuda`, toolkit present | Compiles all four kernels |
| `--features cuda`, toolkit absent | Build error naming `nvcc` and `CUDA_PATH` |
| `size_of::<BlockStats>()` vs the `.cu` struct | Equal |
| Device query, no device | Reported cleanly; `auto` selects CPU |
| `check_workload(25 GiB)` | Refused before allocation |

## Risks

| Risk | Mitigation |
| --- | --- |
| The `.cu` files do not compile | **Expected** — they never have. Fixing them is in scope |
| Struct layout mismatch produces wrong numbers silently | Layout assertion test |
| The feature-gated path rots | The refusal test keeps the off-path covered; `QM-0035` covers the on-path |

## Completion Evidence

* Default build and test output on a CUDA-free machine.
* `nvcc` build log for all four kernels.
* `nvcc --version` and `nvidia-smi` output.
* Struct-layout test output.
* The updated `gpu/cuda/README.md`.
