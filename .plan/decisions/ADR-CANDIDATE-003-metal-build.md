# ADR-CANDIDATE-003 — Metal compute for Apple GPUs

## Status

`Decided`. **Metal is the v1 GPU compute lane.** This reverses the prior
`Open`/"Post-MVP" recommendation below (kept for its evidence and rationale).
v1 ships CPU + Metal only; CUDA moves to an explicit next step, deferred until
after v1 (see `.plan/CUDA_ARCHITECTURE.md` §12 and
`docs/decisions/ADR-008-track-b-prerequisite-waiver.md`).

## Revised decision (v1)

The development and target hardware for v1 is Apple silicon with no NVIDIA GPU
present. Rather than leave Metal an unimplemented extension point while a
CUDA-shaped MVP criterion (`MVP-10`, historically written against CUDA) goes
unmet on this hardware, v1 implements Metal as the accelerated conversion
lane and states the acceptance criterion in terms of Metal for v1, with CUDA
recorded as the next step. This is **Option B** from the original analysis
below, accepted with its stated cost (a real compute backend, differential
tests, memory discipline) because that cost is now in scope for v1 rather
than being an MVP-scope violation.

CUDA (Option covered by `CUDA_ARCHITECTURE.md`) is not deleted — it is
deferred: the same `q_gpu::Backend` trait keeps the seam open, and
`crates/q-cuda`, `gpu/cuda/` remain in the repository as the next-step lane,
gated on RTX 3090 access.

## Original context (superseded)

The task specification §34 asks for a Metal build strategy covering M3 through
next-generation Apple GPUs. The development platform *is* Apple silicon, so a
Metal backend would be the only accelerated path testable here — which is
exactly why v1 now adopts it instead of treating it as scope creep.

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

## Original recommended default (superseded — see Status)

**A.** Extension point only, for the MVP as originally scoped (CUDA-first).
No longer the decision in force; kept for the strategy detail below, which
now applies to the **v1 Metal implementation** rather than to a future
extension point:

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

Lane E (Metal accelerator) in `.plan/MASTER_PLAN.md` §6 — new Metal
build/kernel tasks alongside `QM-0037-backend-selection`. `QM-0092`'s
extension-point framing is updated to describe Metal as implemented in v1
rather than deferred.

## Decision deadline

Decided for v1. CUDA (previously the MVP-10 target) is revisited as the named
next-step task once v1 ships and RTX 3090 access is available.
