# ADR-007 — `q-cuda` as a separate crate; `gpu/` keeps the ARCHITECTURE.md layout

**Status:** Accepted
**Date:** 2026-08-03
**Departs from:** ARCHITECTURE.md §16 (crate list)

## Context

Two small discrepancies between the task's required tree and ARCHITECTURE.md
§16:

1. The task lists a `crates/q-cuda/`. ARCHITECTURE.md §16 lists sixteen crates
   and `q-cuda` is not among them — CUDA appears in §12.3 as a *compute plugin*
   without a crate name.
2. The task lists `gpu/cuda/` and `gpu/shaders/`. ARCHITECTURE.md §16 lists
   `gpu/wgsl/`, `gpu/cuda/`, and `gpu/metal/`.

## Decision

**`q-cuda` is created** as a seventeenth crate, implementing the `q_gpu::Backend`
trait. **`gpu/` keeps ARCHITECTURE.md's layout** — `wgsl/`, `cuda/`, `metal/` —
and no `gpu/shaders/` directory is created.

## Why `q-cuda` is a separate crate

ARCHITECTURE.md §12.3 draws a hard line between rendering (wgpu/WebGPU/Metal/
Vulkan) and large tensor compute (CUDA, MPS, CPU SIMD/BLAS), and assigns CUDA
specific work: full matrix multiplication, quantization, spectral analysis,
large checkpoint comparison. That is a *backend*, and backends are
interchangeable behind a trait.

Putting CUDA inside `q-gpu` would mean either compiling CUDA support into every
consumer of the compute trait, or threading a `cuda` feature flag through the
whole dependency graph. A separate crate makes the dependency explicit: a binary
that wants CUDA depends on `q-cuda`; one that does not, does not.

It also isolates a future build script. When the kernels are compiled for real,
`crates/q-cuda/build.rs` will need `nvcc` — a hard requirement that must not
propagate to `q-gpu`, which has none.

## Why no `gpu/shaders/`

ARCHITECTURE.md §16 names three shader directories by language, and every one of
them holds shaders. A fourth directory called `shaders/` would either duplicate
those or be an empty box whose purpose is unclear from its name. The
language-named layout is more informative and is what the source of truth says.

## Consequences

* The workspace has seventeen crates: ARCHITECTURE.md §16's sixteen plus
  `q-cuda`.
* `q_cuda::CudaBackend` implements the same `q_gpu::Backend` trait as
  `q_gpu::CpuBackend`, so a scheduler can hold `Box<dyn Backend>` and select at
  runtime.
* **Nothing in `q-cuda` has run on a GPU.** It compiles no kernels, links no
  driver, and returns `NotImplemented` for every operation. `capabilities()`
  reports `hardware_verified: false` and `supports_*: false` throughout, so a
  scheduler cannot route work to it by mistake. See `gpu/cuda/README.md` and the
  `CUDA-*` requirements in `STATUS.md`.
* `gpu/wgsl/` and `gpu/metal/` retain the placeholder shaders inherited from the
  earlier scaffold; they are marked as such and are not claimed as implemented.
