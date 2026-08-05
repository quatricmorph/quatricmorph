# ADR-013 — Metal is v1's GPU compute lane, feature-gated and off by default

**Status:** Accepted
**Date:** 2026-08-04
**Promoted from:** `.plan/decisions/ADR-CANDIDATE-003-metal-build.md`

## Context

The candidate this ADR promotes was written twice. Its **original** analysis
recommended option A — extension point only, `gpu/metal/` stays a placeholder,
the `q_gpu::Backend` trait is the seam — on the reasoning that a Metal backend
was scope creep against an MVP criterion (`MVP-10`) written in terms of CUDA.
Its `## Status` section then **superseded that recommendation** and recorded a
revised v1 decision: v1 ships CPU + Metal, and CUDA is deferred until after v1.

The reversal is not this ADR's invention, and it is not confined to the
candidate file. The repository has already been rewritten around it:

* `ARCHITECTURE.md` §12.3 — *"**v1 GPU compute lane is Metal, not CUDA.**"* and
  *"**CUDA is an explicit next step, not v1 scope.**"*
* `.plan/CUDA_ARCHITECTURE.md` — titled *"RTX 3090 block compute (next step,
  post-v1)"*; §12 is *"Metal — v1's GPU compute lane"* and states that
  `ADR-CANDIDATE-003` is now `Decided`.
* `.plan/PRODUCT_SCOPE.md` §2 — the architectural-seam table lists **CUDA
  acceleration** as the capability held open behind `CUDA-001`. Metal is no
  longer listed as a seam.
* `.plan/decisions/README.md` index — candidate 003, status `Decided`, deadline
  `v1`.

Why the reversal happened: the development and target hardware for v1 is Apple
silicon with no NVIDIA GPU present. Leaving Metal an unimplemented extension
point would have left v1 with **no accelerated path testable on the only
hardware that exists**, while a CUDA-shaped acceptance criterion went unmet on
that same hardware. Option A's cost analysis was correct for a CUDA-first MVP
and became inapplicable when CUDA moved out of v1.

What remained genuinely unsettled — and is what `QM-0126` is blocked on — is the
**build and layout** question: which binding, which feature shape, how shaders
are compiled, and how a machine without a Metal toolchain still builds the
workspace.

## Decision

**Metal is v1's GPU compute lane**, implemented behind the existing
`q_gpu::Backend` trait, and its build is feature-gated and off by default.

1. **`MetalBackend` implements `q_gpu::Backend` unchanged** (`crates/q-gpu/src/lib.rs:73`).
   `CpuBackend` remains the default and the numerical reference. Nothing selects
   Metal implicitly.
2. **The binding is `objc2-metal`, not `metal` (metal-rs).** See "Why
   `objc2-metal`" below. Both it and any Objective-C runtime crate it needs are
   `optional = true` and enabled only by the `metal` feature, so the default
   dependency graph is byte-identical to today's.
3. **Feature shape:** a `metal` feature on `q-gpu` with `default = []`, mirroring
   the shape `.plan/CUDA_ARCHITECTURE.md` §9 specifies for `q-cuda`.
   `cargo build --workspace` must succeed with the feature off **and with no
   Metal toolchain installed** — that is `QM-0126` acceptance criterion 1, and it
   is the constraint that forces every other choice here.
4. **Shaders are compiled at build time.** A `build.rs` guarded by
   `cfg(feature = "metal")` compiles `gpu/metal/*.metal` to a `.metallib` using
   the Xcode command-line toolchain (`xcrun -sdk macosx metal` /
   `metallib`). With the feature off, `build.rs` does nothing and requires
   nothing. A shader that fails to compile is a **build error naming the
   shader** — never a silent fallback that ships a stub.
5. **`gpu/` keeps the ARCHITECTURE.md §16 layout** established by `ADR-007`:
   `gpu/metal/` is the home for Metal sources, and no `gpu/shaders/` directory
   is created.
6. **CUDA is deferred, not deleted.** `crates/q-cuda` and `gpu/cuda/` stay in the
   repository as the post-v1 lane behind the same trait, gated on RTX 3090
   access. No CUDA claim is made anywhere.

## Nothing in this repository has run on a Metal device

This is a build-and-layout decision. It is not a hardware result.

`gpu/metal/compute.metal` is a placeholder. `STATUS.md` `GPU-003` is **Not
Started**. No Metal device has executed anything in this repository, no kernel
has been dispatched, and no timing has been taken. `q_gpu::CpuBackend` is the
only backend that has computed a statistic.

Accordingly `MetalBackend::capabilities()` reports **`verified: false`** until
`QM-0127`'s differential test passes against `CpuBackend`, exactly as
`q_cuda::CudaBackend` reports `hardware_verified: false` today (`ADR-007`). A
backend does not claim verification it has not earned, and
`.plan/PRODUCT_SCOPE.md` §5.2 forbids claiming a metric was GPU-computed when
the CPU ran it. Any performance number for this lane is an intention until a
device produces it.

## Alternatives considered

**Option A — extension point only, `gpu/metal/` stays a placeholder.** This was
the candidate's **original recommended default, and it is superseded.** It is
recorded here rather than omitted because the reasoning behind it was sound and
its collapse is instructive. Option A's case rested on two premises: that the
MVP's accelerated-compute criterion named CUDA, and that the CPU meets every
budget in `PERFORMANCE_PLAN.md` §2.3. The second premise still holds — which is
why this lane blocks nothing on the critical path. The first premise no longer
holds: CUDA left v1 scope, so option A would leave v1 with an accelerated lane
that is *permanently* untestable on the hardware the project has, in service of
a criterion no longer being measured. Rejecting it costs a real compute backend,
its differential tests, and its memory discipline — accepted, because that cost
is now v1 scope rather than an MVP-scope violation.

**Option C — implement `wgpu` instead, covering Metal, Vulkan, and DX12.** One
implementation, three platforms, and it runs in the browser via WebGPU.
Rejected: it is strictly larger than a Metal backend, and it contradicts the
document it would be built from. `ARCHITECTURE.md` §12.3 draws a hard line
between rendering (wgpu/WebGPU/Metal/Vulkan) and large tensor compute, assigning
wgpu *"visualization, interactive reductions, filtering, culling, lightweight
compute"* — not block statistics. §12.2 places the wgpu renderer in Phase 3–4.
Building the Phase 3 answer to a Phase 11 task is the scope expansion
`RISK_REGISTER.md` R12 names.

**The `metal` crate (metal-rs) as the binding.** The obvious choice by name and
by download history. Rejected on two external facts, both recorded under
Research: it is **deprecated by its own maintainers**, who direct new work to
`objc2`/`objc2-metal`; and version 0.33.0 declares MSRV **1.82**, above this
workspace's `rust-version = "1.78"`. Adopting it would raise the whole
workspace's MSRV to serve a feature that is off by default — every crate paying
for one optional lane.

**Runtime shader compilation** (`newLibraryWithSource` at startup) instead of a
build-time `.metallib`. Genuinely tempting: it removes the toolchain dependency
from the build entirely, so acceptance criterion 1 becomes trivial. Rejected
because it moves a shader compile failure from build time to run time, which is
precisely what `QM-0126`'s error table forbids — *"Build error naming the
shader — never a silent fallback that ships a stub."* A syntax error in a kernel
should stop a build, not surface as a runtime fallback to CPU that looks like
success.

**Making Metal the default backend once it works.** Rejected, and explicitly out
of scope for `QM-0126`. `CpuBackend` is the numerical reference; a reference that
is not the default is not a reference. Selection stays explicit until
`QM-0127` has proven the two agree.

## Why `objc2-metal`

Two constraints discriminate, and they point the same way.

**MSRV.** The workspace pins `rust-version = "1.78"` (`Cargo.toml:26`).
`objc2-metal` 0.3.2 declares MSRV 1.71 — inside the floor, no workspace change.
`metal` 0.33.0 declares MSRV 1.82 — outside it.

**Maintenance.** The metal-rs README states that use of the crate is deprecated
because the `objc` ecosystem of macOS system bindings is unmaintained, and
recommends `objc2` and `objc2-metal`. Binding v1's only GPU lane to a crate its
maintainers have deprecated is a cost that arrives later and all at once.

Neither fact is available from inside this repository, which is why this is the
one part of this decision that required external research.

If `QM-0126` finds `objc2-metal` insufficient for the paired reduction, that is a
new ADR superseding this clause — not a silent substitution.

## Consequences

* `QM-0126` implements against a settled build shape: a `metal` feature with
  `default = []`, `optional = true` dependencies, a `cfg`-guarded `build.rs`, and
  `objc2-metal` as the binding. Its acceptance criteria 1, 4, and 8 follow from
  this ADR rather than needing to be re-argued.
* **`cargo build --workspace` on a machine with no Metal toolchain must keep
  working.** This is the hard invariant. The current gate is rust **318 passed /
  0 failed** with the feature off, and the feature being off by default is what
  keeps that number the meaningful one.
* The staging budget is enforced through the existing `check_workload` path,
  counting **both** blocks of a pair. Unified memory removes the discrete copy
  but not the ceiling: an Apple silicon machine shares its memory with the OS
  and the renderer, which is a *stricter* constraint than owning discrete VRAM
  outright, not a looser one (`.plan/CUDA_ARCHITECTURE.md` §12).
* Kernel reduction order must be **fixed and documented**. A tree reduction is
  acceptable; nondeterministic atomic accumulation is not — `V1-13` requires
  determinism and `QM-0127`'s tolerance is set against a stated order, not the
  other way round.
* `QM-0127` is where identity with `CpuBackend` is proven, at the tolerances
  `.plan/CUDA_ARCHITECTURE.md` §6 specifies. Until it passes, `GPU-003` is
  `Hardware-Unverified` and the task is `Implemented`, never `Verified`.
* `QM-0037` (backend selection) selects among `CpuBackend`, `MetalBackend` where
  compiled and a device exists, and `CudaBackend` (which refuses). `QM-0092`'s
  extension-point framing describes Metal as implemented in v1 rather than
  deferred.
* CI is unchanged: no GPU job. A job that "passed" without the hardware would be
  worse than none.
* **This lane blocks nothing on the critical path.** If it slips, v1 ships on
  CPU with a slower benchmark and a note saying so.

### What this does not unblock

`QM-0126`'s `## Dependencies` section reads `QM-0121`, `ADR-CANDIDATE-003
(Decided)` — not `(decision required)` — and its `## Status` says it unblocks
when `QM-0121` reaches `Complete`. Under `.plan/README.md` §"Task states", only
an `ADR-CANDIDATE-0XX (decision required)` edge holds a task at `Blocked` for a
decision. This ADR therefore **retires the ADR-decision edge and settles the
build shape**; `QM-0126` remains gated on `QM-0121`.

## Research

External research was required here and nowhere else in this promotion, because
the crate ecosystem and the macOS toolchain are facts outside the repository.

* **crates.io API — `metal`** — https://crates.io/api/v1/crates/metal, retrieved
  2026-08-04. Newest version **0.33.0**, published 2025-12-17; declared
  **MSRV 1.82**; repository `https://github.com/gfx-rs/metal-rs`.
  *Credibility: the registry's own metadata API — the authoritative record of
  what is published.* Cross-checked against the version-specific endpoint
  https://crates.io/api/v1/crates/metal/0.33.0 (same retrieval date), which
  returns `"rust_version": "1.82"`, `"edition": "2021"`,
  `"created_at": "2025-12-17T20:06:22Z"`.
* **metal-rs repository README** — https://github.com/gfx-rs/metal-rs, retrieved
  2026-08-04. Carries a deprecation notice — *"Use of this crate is deprecated
  as the `objc` ecosystem of mac system bindings are unmaintained"* — and
  directs new work to `objc2` and `objc2-metal`, with continued maintenance
  described as supporting `wgpu`'s migration. *Credibility: the maintainers'
  own statement about their own crate.*
* **crates.io API — `objc2-metal`** — https://crates.io/api/v1/crates/objc2-metal,
  retrieved 2026-08-04. Newest version **0.3.2**, published 2025-10-04;
  declared **MSRV 1.71**; repository `https://github.com/madsmtm/objc2`;
  edition 2021. *Credibility: as above.* Cross-checked against the
  version-specific endpoint https://crates.io/api/v1/crates/objc2-metal/0.3.2
  (same retrieval date), which returns `"rust_version": "1.71"`,
  `"edition": "2021"`, `"created_at": "2025-10-04T15:47:38Z"`. This figure is
  the sole discriminator that puts the chosen binding inside the workspace's
  1.78 floor, so it was confirmed from two endpoints rather than one.

These three sources **changed a binding decision**: the candidate names no
crate, and metal-rs would have been the default assumption. Nothing else in
this ADR turns on them — the Metal-is-v1's-lane decision comes from
`ARCHITECTURE.md` §12.3 and the candidate's own `## Status`, both of which
outrank any external source.

**No URL here is a build, runtime, or test dependency.** They are citations.
Version numbers and MSRVs are quoted as published metadata retrieved on the date
given, not as anything measured on this machine. The only numbers in this ADR
measured locally are the test counts, and no benchmark number appears at all,
because none has been produced.
