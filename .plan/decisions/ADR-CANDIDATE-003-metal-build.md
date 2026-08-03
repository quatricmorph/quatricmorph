# ADR-CANDIDATE-003 — Metal compute for Apple GPUs

## Status

`Open`. Post-MVP.

## Context

The task specification §34 asks for a Metal build strategy covering M3 through
next-generation Apple GPUs. The development platform *is* Apple silicon, so a
Metal backend would be the only accelerated path testable here — which makes it
tempting to pull into the MVP. It should not be.

## Repository evidence

* `gpu/metal/compute.metal` — placeholder.
* `gpu/wgsl/compute.wgsl` — placeholder.
* `STATUS.md` `GPU-003` — wgpu / Metal backends, **Not Started**.
* `q_gpu::Backend` (`crates/q-gpu/src/lib.rs:73`) — the trait both would
  implement; `CpuBackend` is the declared reference.
* Task specification §26 excludes a *native Metal renderer*. It does not exclude
  Metal **compute**.

## Decision required

Does the MVP ship a Metal compute backend, and if not, what shape does the
extension point take?

## Options

| Option | Scope |
| --- | --- |
| **A** | Extension point only. `gpu/metal/` stays a placeholder; the trait is the seam |
| **B** | Implement a Metal backend in the MVP, since the hardware is present |
| **C** | Implement `wgpu` instead — one backend covering Metal, Vulkan, and DX12 |

## Advantages

* **A** — zero MVP cost; nothing to verify; the seam already exists.
* **B** — the only accelerated path testable on this machine; would move
  statistics off the CPU.
* **C** — one implementation, three platforms, and it runs in the browser via
  WebGPU. `ARCHITECTURE.md` §12.2 already proposes `wgpu`.

## Disadvantages

* **A** — no acceleration in the MVP. Acceptable: the CPU meets every budget in
  [`PERFORMANCE_PLAN.md`](../PERFORMANCE_PLAN.md) §2.3.
* **B** — a whole compute backend, its differential tests, and its memory
  discipline, for a criterion (`MVP-10`) that names CUDA, not Metal. It would not
  satisfy the acceptance criterion it appears to address.
* **C** — larger than **B**; `ARCHITECTURE.md` places `wgpu` in Phase 3–4, well
  past the MVP.

## Risks

Scope expansion ([`RISK_REGISTER.md`](../RISK_REGISTER.md) R12). "The hardware is
right here" is exactly the reasoning that turns an MVP into a platform.

## Recommended default

**A.** Extension point only.

The strategy, recorded now so implementing it later is additive:

* Implements `q_gpu::Backend`, unchanged.
* Diffed against `CpuBackend` at the same tolerances as CUDA
  ([`CUDA_ARCHITECTURE.md`](../CUDA_ARCHITECTURE.md) §6).
* Same block-at-a-time discipline. **Unified memory removes the discrete copy but
  not the ceiling** — an M3 Max with 128 GB shares that memory with the OS and the
  renderer, which is a stricter constraint than owning 24 GB outright, not a
  looser one.
* Metal Performance Shaders for reductions and GEMM where they beat a hand-written
  kernel; custom kernels for quantization and Morton packing.
* Feature-gated `metal`, off by default, exactly as CUDA is.

## Tasks affected

None in the MVP. Documented as an extension point in `QM-0092`.

## Decision deadline

Post-MVP. Revisit if `PERF-001`'s conversion budget proves unmeetable on CPU.
