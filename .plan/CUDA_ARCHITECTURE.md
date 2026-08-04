# CUDA_ARCHITECTURE — RTX 3090 block compute (next step, post-v1)

**Scope note:** v1's GPU compute lane is Metal, not CUDA — see
`.plan/decisions/ADR-CANDIDATE-003-metal-build.md` (`Decided`) and
`.plan/MASTER_PLAN.md` §6 Lane E. Everything in this document describes the
**deferred next-step CUDA lane**, scheduled after v1 ships, kept ready behind
the same `q_gpu::Backend` trait Metal implements in v1.

## 0. Honest starting position

**No code in this repository has ever run on a GPU.**

`gpu/cuda/README.md` states it: *"Every `.cu` file in this directory is source
only. None has been compiled, linked, or executed. There is no `nvcc` step in the
build, no `build.rs`, and no FFI binding in `crates/q-cuda`."* `STATUS.md` marks
`CUDA-002` … `CUDA-005` **Hardware-Unverified** with the test column reading
*"none — never compiled or executed"*. CI has no CUDA job, deliberately.

This document plans how that changes, and — equally important — how the MVP
completes if it does not.

**The MVP does not require CUDA.** `q_gpu::CpuBackend` is the numerical reference
(`GPU-002`, 7 tests) and `q_gpu::block_statistics_default` already computes what
the tile pyramid needs. The entire critical path in
[`MASTER_PLAN.md`](MASTER_PLAN.md) §5 runs on CPU.
`docs/decisions/ADR-008-track-b-prerequisite-waiver.md` already waives the RTX
3090 gate. CUDA is **Lane E**: an accelerator that replaces a backend behind an
existing trait, changing no downstream artifact.

---

## 1. Target hardware

| Property | Value | Source |
| --- | --- | --- |
| Device | NVIDIA GeForce RTX 3090 | Task specification §10 |
| Architecture | Ampere, `sm_86` | `gpu/cuda/README.md` |
| VRAM | 24 GB = `q_cuda::RTX_3090_VRAM_BYTES` | `crates/q-cuda/src/lib.rs:43` |
| Usable fraction | `0.80` = `USABLE_VRAM_FRACTION` | `crates/q-cuda/src/lib.rs:47` |
| Effective ceiling | ≈ 19.2 GB | Derived |

### What 24 GB is, in parameters

| dtype | Parameters that fit in 19.2 GB | Fraction of 10¹² |
| --- | --- | --- |
| f32 | 4.8 × 10⁹ | 0.48 % |
| f16 / bf16 | 9.6 × 10⁹ | 0.96 % |
| int8 | 19.2 × 10⁹ | 1.92 % |

— before a single working buffer, output buffer, or kernel's scratch space.

**A trillion-parameter model does not fit, cannot be made to fit, and this
architecture never pretends otherwise.** The GPU is a block processor. It sees
one bounded block at a time, streamed in by the host, and it never holds a
tensor, let alone a model.

---

## 2. What CUDA is and is not for

### Uses CUDA

| Workload | Kernel | State |
| --- | --- | --- |
| FP16 / BF16 → F32 conversion | in `reduce.cu` / `quantize.cu` | Source only |
| Min / max reduction | `gpu/cuda/reduce.cu` | Source only |
| Mean and variance | `reduce.cu` | Source only |
| L1 and L2 norms | `reduce.cu` (sum, sum-of-squares) | Source only |
| Positive / negative / zero ratios | `reduce.cu` | Source only |
| Histogram | `gpu/cuda/histogram.cu` | Source only |
| Quantization to i16 | `gpu/cuda/quantize.cu` | Source only |
| Morton-order encoding | `quantize.cu` | Source only |
| Value normalization and visual classification | `quantize.cu` | Source only |
| Block sampling | `quantize.cu` | Source only |
| Block matrix multiplication (optional) | `gpu/cuda/matmul.cu`, tiled | Source only |
| Tensor comparison (optional) | not written | — |

### Does not use CUDA

SafeTensors header parsing · catalog queries · file path handling · GLB container
writing · `tileset.json` generation · Cesium tile traversal · browser UI state.

These are I/O-bound, branch-heavy, or allocation-heavy. Moving them to a GPU
would add a transfer for every operation and remove nothing from the CPU's
critical path. `ARCHITECTURE.md` §12.3 draws the same line.

---

## 3. Data flow and memory budgets

```text
  SafeTensors on disk
        │  mmap / pread, one byte run per row  (TensorBlock::plan)
        ▼
  ┌─ bounded host staging buffer ────────────┐   MAX_HOST_STAGING_BYTES
  │  raw dtype bytes for N blocks in flight  │
  └────────────┬─────────────────────────────┘
               │ dtype decode (CPU) or pass raw to the kernel
        ▼
  ┌─ pinned host buffer ─────────────────────┐   MAX_PINNED_BYTES
  │  page-locked, enables async DMA          │
  └────────────┬─────────────────────────────┘
               │ cudaMemcpyAsync on a per-block stream
        ▼
  ┌─ GPU input staging ──────────────────────┐   MAX_GPU_INPUT_BYTES
  │  one block per in-flight slot            │
  └────────────┬─────────────────────────────┘
               │ kernel launch
        ▼
  ┌─ GPU output buffer ──────────────────────┐   MAX_GPU_OUTPUT_BYTES
  │  statistics struct + quantized cells     │   ≪ input
  └────────────┬─────────────────────────────┘
               │ cudaMemcpyAsync back
        ▼
  ┌─ compact host output ────────────────────┐   MAX_OUTPUT_QUEUE_DEPTH
  └────────────┬─────────────────────────────┘
               ▼
        .qtile writer  →  GLB writer  →  catalog rows  →  cache
```

Every arrow is bounded, and every bound is a named configuration variable, not a
literal. Formulas in [`MEMORY_BUDGET.md`](MEMORY_BUDGET.md).

| Budget | Default | Rationale |
| --- | --- | --- |
| `MAX_HOST_STAGING_BYTES` | 512 MiB | 2 048 blocks of 256×256 f32 |
| `MAX_PINNED_BYTES` | 128 MiB | Pinned memory is scarce; over-pinning degrades the whole system, not just this process |
| `MAX_GPU_INPUT_BYTES` | 2 GiB | ≈ 10 % of the usable ceiling, leaving room for other processes |
| `MAX_GPU_OUTPUT_BYTES` | 512 MiB | Outputs are ~2 orders smaller than inputs |
| `MAX_CONCURRENT_BLOCKS` | 4 | One CUDA stream each; enough to overlap transfer with compute |
| `MAX_OUTPUT_QUEUE_DEPTH` | 64 | Backpressure onto the reader when the writer falls behind |

`q_cuda::CudaBackend::check_workload` already enforces the ceiling **before a
launch would be attempted** — verified today by arithmetic on a declared limit,
without a device (`the_vram_ceiling_is_enforced_without_a_device`).

### 3.1 Adaptive block sizing

On `cudaErrorMemoryAllocation`, or when a budget check fails, the scheduler
**halves the block dimension and retries**, down to a floor of 64 × 64. Below
that it fails with a clear message naming the budget that could not be met. It
does not fall back to processing the whole tensor, and it does not silently
succeed with a smaller sample.

---

## 4. Device discovery and validation

Before any launch, in this order, failing closed:

1. **Driver and runtime present.** `cudaGetDeviceCount` succeeds.
2. **At least one device.** Zero devices is not an error — it selects the CPU
   backend.
3. **Compute capability ≥ 8.6** for kernels compiled `-arch=sm_86`. A lower
   device is refused by name, not run with wrong assumptions.
4. **Free VRAM query.** `cudaMemGetInfo`. The usable ceiling is
   `min(free × USABLE_VRAM_FRACTION, MAX_GPU_INPUT_BYTES + MAX_GPU_OUTPUT_BYTES)`
   — the *actual* free memory, because another process may already hold most of
   the card.
5. **Runtime/driver version compatibility.** A mismatch is reported with both
   versions.

`q_cuda::CudaBackend::is_available()` is the entry point and already exists as a
stub.

---

## 5. Backend selection and fallback

```text
requested backend
  ├─ "cpu"   → CpuBackend                       always available
  ├─ "cuda"  → CudaBackend if all checks pass
  │            else → hard error naming the failed check
  └─ "auto"  → CudaBackend if available, else CpuBackend, and SAY WHICH
```

Rules:

* **`auto` announces its choice** in the job record, the API response, and the
  log. A user who thinks a run was GPU-accelerated and reads CPU timings has been
  misled by silence.
* **Explicit `cuda` never silently falls back.** If a user asked for the GPU, an
  unavailable GPU is an error, not a slow success.
* CPU covers every MVP operation. There is no operation the pipeline needs that
  only CUDA can do — by design, so the fallback is complete rather than partial.

---

## 6. Determinism and numerical tolerance

**The CPU backend is the ground truth.** `gpu/cuda/README.md` already says a
divergence is a bug in the kernel.

| Operation | Tolerance vs CPU f64 reference | Why |
| --- | --- | --- |
| min / max | **exact** | Comparison, not arithmetic. Any difference is a bug |
| Zero / positive / negative ratios | **exact** | Counting |
| Histogram bin counts | **exact** | Counting, given identical bin edges |
| Sum, L1 | rel. `1e-6` (f32 accumulate) | Tree reduction reorders addition |
| Mean, variance | rel. `1e-6` | Welford on CPU vs parallel reduction on GPU |
| L2 norm | rel. `1e-6` | Sum-of-squares, then one sqrt |
| Quantized i16 | **exact**, given identical min/max | Integer arithmetic after normalization |
| Morton codes | **exact** | Bit interleaving |
| Block matmul | rel. `1e-5` | Tiled accumulation order differs; f32 FMA |

Determinism requirements:

* **Run-to-run on the same device: bit-identical.** Fixed block and grid
  dimensions, no atomics into float accumulators, no `atomicAdd(float*)` in a
  reduction. Where a histogram needs atomics they are integer atomics, which are
  order-independent.
* **Across devices: within tolerance, not bit-identical.** Different SM counts
  change the reduction tree. This is documented rather than fought.
* Fast-math is **off**. `-use_fast_math` would make `1e-6` unachievable and would
  make the CPU reference meaningless as a check.

---

## 7. Cancellation and resumability

* **Cancellation is checked between blocks, never inside a kernel.** A launched
  kernel runs to completion; blocks are small enough that the latency is bounded
  by one block's compute — milliseconds, not seconds.
* On cancel: stop issuing launches, `cudaStreamSynchronize` the in-flight
  streams, free device buffers, mark the job `Cancelled`, and leave the completed
  block manifest intact.
* On resume: re-read the manifest, verify content hashes, skip verified blocks.
  Identical in shape to the verified ingestion resume
  (`resume_skips_completed_shards`).
* **Device memory is freed on every exit path**, including panic and error.
  Buffers are RAII wrappers; nothing frees in a `Drop` that can be skipped by
  `mem::forget`. `QM-0083` soaks this: 10 000 block jobs must end at the same
  `cudaMemGetInfo` free value they started at.

---

## 8. Error handling

| Failure | Response |
| --- | --- |
| No driver / no device | Select CPU under `auto`; hard error under explicit `cuda` |
| Compute capability too low | Named refusal; never run a `sm_86` binary on older hardware |
| `cudaErrorMemoryAllocation` | Halve the block, retry, down to 64×64; then fail naming the budget |
| Kernel launch failure | Propagate with kernel name, launch geometry, and block ID |
| `cudaErrorIllegalAddress` | Fail the job. Do **not** retry — the context is corrupt and any further result is untrustworthy |
| Result diverges from CPU beyond tolerance | Fail the job, record both values and the block ID. A silent bad statistic is worse than a stopped job |
| Device lost / ECC error | Fail the job, mark it resumable, free everything |

Every error carries the block ID, so a resumed job knows exactly what to redo.

---

## 9. Build integration

`QM-0034`. Deliberately **feature-gated and off by default**, so that a machine
without CUDA still builds the workspace — which is the machine this repository is
developed on.

```toml
# crates/q-cuda/Cargo.toml
[features]
default = []
cuda    = []          # enables build.rs nvcc compilation and FFI linkage
```

```bash
cargo build --workspace                       # no CUDA. must keep working.
cargo build -p q-cuda --features cuda         # requires nvcc + driver
cargo test  -p q-cuda --features cuda         # requires an actual RTX 3090
```

`build.rs` compiles `gpu/cuda/*.cu` with `-arch=sm_86`, no fast-math, and links
the CUDA runtime. When the `cuda` feature is off, `CudaBackend` keeps returning
`NotImplemented` exactly as it does today — so the default build's behaviour is
unchanged and the 290-test baseline cannot regress.

CI stays as it is: **no CUDA job**. The comment in `.github/workflows/build.yaml`
already explains why a job that "passed" without the hardware would be worse than
none. A GPU job is added only when a self-hosted RTX 3090 runner exists.

---

## 10. Testing

| Test | Hardware | Task |
| --- | --- | --- |
| VRAM ceiling arithmetic | none — already passing | — |
| Every operation refuses without the feature | none — already passing | — |
| `nvcc` compiles all four `.cu` files | CUDA toolkit only, no device | `QM-0034` |
| Reduction kernels vs `CpuBackend`, f32/f16/bf16 | **RTX 3090** | `QM-0035` |
| Histogram vs CPU, exact bin counts | **RTX 3090** | `QM-0035` |
| Quantization and Morton vs CPU, exact | **RTX 3090** | `QM-0036` |
| Block matmul vs CPU, rel `1e-5` | **RTX 3090** | `QM-0036` |
| Multiple block dimensions: 64, 128, 256, 512 | **RTX 3090** | `QM-0035` |
| Out-of-memory adaptation halves and retries | **RTX 3090** | `QM-0036` |
| Cancellation between blocks frees everything | **RTX 3090** | `QM-0083` |
| 10 000-job device-memory soak | **RTX 3090** | `QM-0083` |
| Kernel benchmarks vs CPU on identical blocks | **RTX 3090** | `QM-0036` |

**Until an RTX 3090 executes these, `CUDA-002`…`CUDA-005` stay
`Hardware-Unverified` in `STATUS.md` and the corresponding tasks stay
`Implemented`, never `Verified`.** That is the rule in
[`README.md`](README.md) §"Status vocabulary", and it exists so that no one
reads this repository and believes a kernel works because someone wrote it.

---

## 11. Benchmarking

Reported per kernel, per block size, on identical input, against the CPU backend:

```text
block dimensions · dtype · bytes in · bytes out
host→device ms · kernel ms · device→host ms · total ms
GB/s effective · CPU baseline ms · speedup
peak device bytes · blocks in flight
```

**No performance number appears in any document until it has been measured on the
device.** The current `.cu` files carry launch geometry and intent; the README
already warns that every performance claim in them is *"an intention, not a
measurement"*. That warning stands until `QM-0036` produces numbers.

---

## 12. Metal — v1's GPU compute lane

Task specification §34 asks for a Metal build strategy for M3 and later Apple
GPUs. `gpu/metal/compute.metal` is the target for v1's implementation.

**Scope:** Metal is **v1's GPU compute lane**, not an extension point held for
later (this inverts the prior `PRODUCT_SCOPE.md` §2 framing —
`ADR-CANDIDATE-003` is now `Decided`). It implements the same
`q_gpu::Backend` trait CUDA would, is diffed against the same CPU reference,
and inherits the same block-at-a-time discipline and budgets described
throughout this document (§§3–8) — read those sections as applying to Metal
in v1, with CUDA reusing the same discipline when its turn comes post-v1.
Unified memory changes the transfer story — there is no discrete copy — but
not the ceiling story: an M3 Max with 128 GB of unified memory still cannot
hold a trillion parameters, and competing with the OS and the renderer for
that memory is a stricter constraint than owning 24 GB outright.

Note that §26 excludes a *native Metal renderer*. A Metal **compute** backend
is a different thing and is v1 work. `ADR-CANDIDATE-003` records the decision
and strategy; v1 tasks implement it (`.plan/MASTER_PLAN.md` §6 Lane E).
