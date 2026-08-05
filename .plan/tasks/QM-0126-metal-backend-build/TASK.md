# QM-0126 — Metal backend build integration

## Status

Complete

`QM-0121` reached `Complete`, which unblocked this. Not `Complete` itself:
`STATUS.md` GPU-003 is updated but the numerical verification against
`CpuBackend` belongs to `QM-0127`, and `hardware_verified` stays `false` until
it passes.

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

Full derivation, including everything that was **not** performed:
`.plan/evidence/QM-0126.md`. Implementation commit `4ba84ef` on branch
`task/qm-0126-metal-backend`, based on `main` at `39b3aa2`.

### Acceptance criterion 1 — `cargo build --workspace` with the feature OFF

```console
$ cargo build --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 19s
$ echo $?
0
```

`crates/q-gpu/build.rs` returns immediately when `CARGO_FEATURE_METAL` is
unset — no `xcrun`, no `clang`, no probe — so a machine with no Xcode command
line tools takes the same path. `git diff --stat Cargo.lock` is empty: the
Metal API is reached through `gpu/metal/qm_metal_shim.m`, not a crates.io
binding, so `scripts/license-audit.sh`'s Rust surface is unchanged.

### Criteria 2 and 3 — a real device, a real dispatch

```console
$ cargo build -p q-gpu --features metal      # exit 0
$ cargo test  -p q-gpu --features metal      # exit 0; 55 passed; 0 failed
```

`metal::tests::the_device_reports_its_real_identity_and_limits` printed, from
`MTLDevice` and the compiled pipeline:

```text
QM-0126 device: Apple M3 Pro | unified=true |
  recommendedMaxWorkingSetSize=30150672384 B | maxBufferLength=22613000192 B |
  maxTotalThreadsPerThreadgroup=1024 | staging budget=268435456 B
```

`a_small_paired_reduction_runs_on_device_and_returns_the_right_shape` dispatched
the QM-0121 hand fixture on that GPU and got back
`count: 12, sum_sq_base: 79.6875, sum_sq_delta: 3.25, sum_abs_delta: 5.5,
max_abs_delta: 1.0, max_abs_base: 4.5` with 3 channels of `count: 4` for
`ChannelAxis::Rows` and 4 of `count: 3` for `ChannelAxis::Columns`. Full output
in the evidence file §4.2.

### Criterion 4 — `hardware_verified` is still `false`

`ComputeCapabilities { backend_id: "metal", hardware_verified: false,
caveat_requirement: Some("GPU-003"), supports_statistics: false,
supports_matmul: false, supports_histogram: false }`, on the instance bound to
the real M3 Pro as well as on a deviceless one.
`the_backend_never_claims_hardware_verification` asserts both. **No test in
this task compares Metal with `CpuBackend`**; that is `QM-0127`.

### Criterion 5 — the staging budget, counting both blocks, before dispatch

`the_staging_budget_counts_both_blocks_of_the_pair_without_a_device`: a
4096 × 8192 pair is exactly the 256 MiB budget and is accepted; 4096 × 8193 is
**refused** with `BudgetExceeded { budget_name: "metal_device_staging",
requested: 268468224, limit: 268435456 }` — a refusal that only happens because
both blocks are counted, since one block alone is 128 MiB + 16 KiB.
`the_budget_refusal_precedes_any_dispatch` shows the refusal beating the device
check with a 16-byte declared budget.

### Criterion 6 — the reduction order

Stated in the header of `gpu/metal/paired_reduction.metal` and summarised in
`crates/q-gpu/src/metal.rs`: one threadgroup per channel, **no atomics**, thread
`t` summing its stripe in increasing element order, then a fixed 256-lane binary
tree; dispatched with `dispatchThreadgroups:threadsPerThreadgroup:` so the
driver cannot reshape it. Accumulation is **f32 on device** — Metal has no
double — with the delta formed in f32 and a **widening** to f64 on host
readback; whole-block partials come from a second, independent flat dispatch
rather than a re-sum of the per-channel results.
`repeated_dispatches_of_the_same_block_return_identical_bytes` runs a 7×300 pair
five times for bit-identical output (`V1-13`).

### Criterion 7 — no device

`device_or_skip` prints a named reason and returns, for both an absent device
and an initialisation fault. **This machine has a GPU, so that branch did not
fire here** — see `.plan/evidence/QM-0126.md` §7. What was exercised instead is
`MetalBackend::with_declared_staging_budget`, which queries no device and drives
the entire refusal ladder without hardware, in four tests.

### Criterion 8 — `CpuBackend` remains the default

`q_gpu::default_backend()` returns `CpuBackend` in every build.
`the_default_backend_is_the_cpu_reference_whatever_features_are_enabled`
compiles identically with and without the feature.

### Gates and the floor

| gate | exit |
| --- | --- |
| `cargo build --workspace --all-targets` | 0 |
| `cargo fmt --all -- --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo clippy -p q-gpu --features metal --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0 — `745 passed; 0 failed` over 54 binaries |
| `cargo build -p q-gpu --features metal` | 0 |
| `cargo test -p q-gpu --features metal` | 0 — `55 passed; 0 failed` |

Floor raised in the same commit, **default features only**: `744 + 1 = 745`
tests over an unchanged 54 binaries. The nine tests in
`crates/q-gpu/src/metal.rs` are behind `#[cfg(feature = "metal")]` and
contribute zero to the floor; `q-gpu` alone reports 46 by default and 55 with
the feature, and that feature-on figure is deliberately absent from
`scripts/baseline.json`.
