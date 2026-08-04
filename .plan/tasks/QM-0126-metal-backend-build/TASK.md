# QM-0126 — Metal backend build integration

## Status

Blocked

Unblocks when `QM-0121` reaches `Complete`.

## Phase

Phase 11 — Quantisation-error diagnostic engine

## Objective

Implement `q_gpu::Backend` on Apple GPU hardware, with the paired reduction as its
first real kernel, and make it build and skip cleanly on machines without Metal.

## Repository Evidence

* `crates/q-gpu/src/lib.rs:73` — the `Backend` trait; `CpuBackend` is `GPU-002
  Verified` with 7 tests and is the numerical reference.
* `gpu/metal/` — placeholder shaders; `gpu/wgsl/compute.wgsl` remains a
  placeholder and stays one.
* `crates/q-cuda/src/lib.rs` — the refusal idiom and `the_vram_ceiling_is_enforced_without_a_device`.
* `.plan/decisions/ADR-CANDIDATE-003-metal-build.md` — `Decided`: Metal is v1's
  GPU lane.
* `docs/decisions/ADR-008-track-b-prerequisite-waiver.md` — the RTX 3090 gate is
  already waived.

## Requirements Covered

`GPU-003`.

## Dependencies

`QM-0121`, `ADR-CANDIDATE-003` (Decided).

## Blocks

`QM-0127`.

## Parallelization

Lane U. **Blocks nothing on the critical path** — v1 ships on CPU if this slips.
Shares `crates/q-gpu/src/lib.rs` with `QM-0121`, so it starts after it.

## Program Boundary

`crates/q-gpu`, `gpu/metal/`.

## Scope

* A `MetalBackend` implementing `Backend`, feature-gated.
* The paired reduction as a Metal compute kernel.
* Device discovery, capability reporting, and a bounded staging buffer.
* Clean behaviour where Metal is unavailable: the crate builds, tests skip with a
  named reason, and `CpuBackend` remains the default.

## Out of Scope

Differential verification (`QM-0127`) · matmul kernels · CUDA · optimisation
beyond a straightforward correct kernel · making Metal the default backend.

## Files Expected to Change

* `crates/q-gpu/Cargo.toml` — a `metal` feature, default off
* `crates/q-gpu/src/lib.rs` — backend selection

## Files Expected to Add

* `crates/q-gpu/src/metal.rs`
* `gpu/metal/paired_reduction.metal`
* `crates/q-gpu/build.rs` — shader compilation, guarded

## Data Contracts

No new public types. `MetalBackend` implements the existing trait and returns the
same `PairedPartials` `CpuBackend` does — that identity is the whole point, and
`QM-0127` is where it is proven.

`ComputeCapabilities` reports the real device: name, unified-memory size, maximum
buffer length, and `verified: false` until `QM-0127` passes.

## Memory and Performance Constraints

```text
device staging = max_concurrent_blocks × 2 × block_bytes
               ≤ MAX_DEVICE_STAGING_BYTES     (named budget, default 256 MiB)
```

Enforced through the existing `check_workload` path, counting **both** blocks of
the pair. On unified memory the distinction between host and device staging is
softer than on discrete GPUs; the budget is still named and enforced, because
`V1-03`'s residency ceiling covers the whole process.

Reduction order inside the kernel must be **fixed and documented**. A tree
reduction is fine; a nondeterministic atomic accumulation is not — `V1-13`
requires determinism, and `QM-0127`'s tolerance assumes a stated order.

## Implementation Plan

1. Add the `metal` feature, off by default, enabled automatically on
   `target_os = "macos"` where the toolchain is present.
2. `build.rs`: compile `.metal` sources to a metallib when the feature is on;
   emit a clear skip when it is not. `cargo build --workspace` must keep working
   with the feature off.
3. Device discovery; capabilities reported from the real device.
4. The paired-reduction kernel: per-threadgroup partial sums in a fixed tree
   order, f32 inputs accumulated in f32 with a documented final f64 fold on the
   host — or full f64 where the device supports it, stated either way.
5. Per-channel partials via a second dispatch or a segmented reduction; whichever
   is chosen, the segmentation must be documented for `QM-0127`'s tolerance.
6. Enforce the staging budget before dispatch.
7. Tests that skip with a named reason where no device is present.

## Error Handling

| Case | Behaviour |
| --- | --- |
| No Metal device | `MetalBackend::new` returns `None`; selection falls back to CPU and says so |
| Shader compilation fails at build time | Build error naming the shader — never a silent fallback that ships a stub |
| Workload exceeds the staging budget | `BudgetExceeded` naming the budget, before dispatch |
| Device loss mid-dispatch | Error naming the device; the caller may retry on CPU. Never partial results presented as complete |
| Feature off | Trait method absent from selection; CPU used; no runtime cost |

## Acceptance Criteria

1. `cargo build --workspace` succeeds **without** the Metal feature and without a
   Metal toolchain.
2. With the feature on, `MetalBackend` builds and reports real device
   capabilities.
3. The paired reduction runs on device and returns `PairedPartials` of the right
   shape for a small block.
4. `capabilities().verified` is `false` until `QM-0127` passes — the backend does
   not claim verification it has not earned.
5. The staging budget is enforced before dispatch, counting both blocks.
6. Kernel reduction order is fixed and documented.
7. Where no device exists, tests skip with a named reason rather than failing or
   silently passing.
8. `CpuBackend` remains the default; nothing selects Metal implicitly.

## Verification Plan

**Automated** — build with and without the feature; a device test that skips
cleanly.
**Manual** — run on the M3 Pro; record the device name and capabilities.

## Suggested Commands

```bash
cargo build --workspace                       # feature off — must succeed
cargo build -p q-gpu --features metal
cargo test  -p q-gpu --features metal
system_profiler SPDisplaysDataType | head -20 # device identity for the evidence
```

## Test Cases

| Input | Expected |
| --- | --- |
| Build without the feature | Succeeds; no Metal symbols |
| Build with the feature, toolchain present | Succeeds; metallib produced |
| Small paired reduction on device | Correct shape; plausible values |
| Workload over the staging budget | Refused, budget named |
| No device (feature on, e.g. CI) | Test skips with a reason |
| `capabilities().verified` | `false` |

## Risks

| Risk | Mitigation |
| --- | --- |
| The Metal lane slips and drags v1 | It blocks nothing; CPU ships v1 with a slower benchmark and a note |
| Nondeterministic reduction breaks `V1-13` | Fixed tree order, documented; determinism tested in `QM-0127` |
| f32 accumulation on device drifts from f64 on host | Documented accumulation strategy; `QM-0127`'s tolerance is set against it, not the other way round |
| A stub ships claiming GPU execution | `verified: false` until `QM-0127`; `PRODUCT_SCOPE.md` §5.2 forbids the claim |

## Completion Evidence

* Build output with and without the feature.
* Device name and reported capabilities from the M3 Pro.
* A small paired reduction's device output.
* The documented reduction order.
* Confirmation that `verified` is still `false`.
