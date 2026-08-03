# QM-0037 — Backend selection, CPU fallback, determinism policy

## Status

Blocked

Unblocks when `QM-0034` reaches `Complete`.

## Phase

Phase 03 — Block runtime and compute

## Objective

Make backend choice explicit, announced, and never silently degrading.

## Repository Evidence

* `q_gpu::Backend` trait (`crates/q-gpu/src/lib.rs:73`); `ComputeCapabilities`
  (`:43`); `CpuBackend::ID = "cpu-reference"` (`:146`).
* `cpu_backend_declares_itself_verified_and_capable` (`GPU-001` Verified).
* `q_cuda::CudaBackend::ID = "cuda"` (`:75`), `is_available()` (`:93`).
* `q_statistics::TensorStatistics::finish(backend)` already records which backend
  produced a result.

## Requirements Covered

`GPU-001`, `CUDA-001`; supports `MVP-12`.

## Dependencies

`QM-0034`.

## Blocks

`QM-0041`, `QM-0070`.

## Parallelization

Lane A, parallel with `QM-0035`/`QM-0036` — it needs the build, not the hardware.

## Program Boundary

`crates/q-gpu`, `crates/q-cuda`, `crates/q-daemon`, `crates/q-cli`.

## Scope

* `select_backend(request) -> Result<Box<dyn Backend>>` with `cpu | cuda | auto`.
* `auto` prefers CUDA when available **and verified**, else CPU, and **records
  which**.
* Explicit `cuda` **never** silently falls back.
* A size threshold below which CPU is used even when CUDA is available, taken
  from `QM-0036`'s measurement.
* Every result carries `backend` and `algorithm_version` to the API.
* Document the determinism policy in code, not only in the plan.

## Out of Scope

Kernel verification (`QM-0035`, `QM-0036`) · Metal or wgpu · multi-GPU.

## Files Expected to Change

* `crates/q-gpu/src/lib.rs`
* `crates/q-cuda/src/lib.rs`
* `crates/q-daemon/src/lib.rs`
* `crates/q-cli/src/main.rs`

## Files Expected to Add

None.

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
{ "backend": { "id": "cpu-reference", "requested": "auto",
               "reason": "no CUDA device present",
               "verified": true, "device": null } }
```

`verified: false` means the backend is `Hardware-Unverified` — an
**unverified backend must never be selected by `auto`**. A user who did not ask
for unverified compute must not get it.

## Memory and Performance Constraints

Selection is one-time per job, not per block. Device query is cached for the
process lifetime.

## Implementation Plan

1. Extend `ComputeCapabilities` with `verified: bool` and `device: Option<String>`.
2. Implement `select_backend` with the three modes and the size threshold.
3. `auto` → CUDA only when `is_available()` **and** `verified`.
4. Explicit `cuda` unavailable → hard error naming the failed check.
5. Thread the selection into job records and API responses; log it at start.
6. Document the determinism policy as a doc comment on the trait: bit-identical
   per device; within tolerance across devices; fast-math off.

## Error Handling

* Explicit `cuda`, no driver → error: "CUDA requested but no driver found".
* Explicit `cuda`, capability < 8.6 → error naming both the found and required
  capability.
* Explicit `cuda`, unverified build → error: "CUDA backend is
  Hardware-Unverified; pass `--allow-unverified-backend` to proceed".
* `auto`, nothing available → CPU, logged at info.

## Acceptance Criteria

1. `auto` on a CUDA-free machine selects CPU and **says so** in the record, the
   log, and the API response.
2. Explicit `cuda` on a CUDA-free machine is a hard error naming the check.
3. `auto` never selects an unverified backend.
4. `--allow-unverified-backend` is required to use one, and the result is
   labelled.
5. Below the size threshold, `auto` selects CPU even with CUDA present.
6. Every statistics row records its producing backend.
7. The determinism policy is a doc comment on `Backend`.

## Verification Plan

**Automated** — selection tests for all three modes on a CUDA-free machine
(covers most paths); an unverified-backend refusal test.
**Manual** — on an RTX 3090, confirm `auto` selects CUDA once `QM-0035` passes.

## Suggested Commands

```bash
cargo test -p q-gpu -p q-cuda                                       # verified today
cargo run -p q-cli -- stats … --backend auto                        # introduced here
cargo run -p q-cli -- stats … --backend cuda                        # errors without a GPU
```

## Test Cases

| Input | Expected |
| --- | --- |
| `auto`, no CUDA | CPU; reason recorded |
| `cuda`, no driver | Error naming the driver check |
| `cuda`, capability 7.5 | Error naming 7.5 vs 8.6 |
| `auto`, CUDA present but unverified | **CPU**, with the reason |
| `cuda`, unverified, no flag | Error naming the flag |
| `cuda`, unverified, with the flag | Runs; result labelled unverified |
| 64×64 block, `auto`, CUDA present | CPU — below threshold |
| Statistics row | Carries `backend` |

## Risks

| Risk | Mitigation |
| --- | --- |
| A user believes a CPU run was GPU-accelerated | `auto` announces its choice in three places |
| Unverified kernels silently produce results | `auto` refuses them; an explicit flag is required and the result is labelled |
| The size threshold is guessed | It comes from `QM-0036`'s measurement; until then, `auto` uses CPU below 256×256 |

## Completion Evidence

* Selection test output for all modes.
* Log lines showing the announced choice.
* An API response containing the backend block.
* The threshold value and its source measurement.
