# ADR-CANDIDATE-002 — CUDA build strategy

## Status

`Open`. **Deferred to the CUDA next step, post-v1** — v1 ships CPU + Metal
only (see `.plan/decisions/ADR-CANDIDATE-003-metal-build.md`). This ADR still
needs deciding before `QM-0034` starts, but `QM-0034` itself is scheduled
after v1, not within it.

## Context

Four `.cu` files exist and have **never been compiled**. `q-cuda` has no
`build.rs`, no FFI, and returns `NotImplemented` for every operation. Making them
run requires choosing how they get compiled and linked — and, more importantly,
how the workspace keeps building on a machine with no CUDA toolkit, which is the
machine this repository is developed and tested on.

## Repository evidence

* `gpu/cuda/{reduce,histogram,matmul,quantize}.cu` — source only.
* `gpu/cuda/README.md`: *"There is no `nvcc` step in the build, no `build.rs`,
  and no FFI binding."* It also prescribes `nvcc -arch=sm_86`.
* `crates/q-cuda/src/lib.rs:51` — `KERNEL_SOURCES` lists the four files **as
  data**, not as a build input.
* `.github/workflows/build.yaml` — no CUDA job, with a comment explaining why.
* `docs/decisions/ADR-007-q-cuda-crate-and-gpu-layout.md` — the crate/directory
  split is already decided.
* Development platform is darwin / Apple silicon.

## Decision required

How are the kernels compiled, linked, and gated?

## Options

| Option | Mechanism |
| --- | --- |
| **A** | `build.rs` + `cc`/`cudarc` invoking `nvcc`, behind a `cuda` feature, **off by default** |
| **B** | Same, but the feature is **on** by default |
| **C** | Precompiled PTX/cubin checked in, loaded via the driver API at runtime |
| **D** | A separate `q-cuda-sys` crate for the FFI, plus `q-cuda` for the safe wrapper |

## Advantages

* **A** — `cargo build --workspace` keeps working with no toolkit; the 290-test
  baseline cannot regress; the refusal path stays live and tested.
* **B** — no flag to remember on a GPU machine.
* **C** — no `nvcc` needed at build time; the artifact is reproducible.
* **D** — clean separation, conventional for `-sys` crates.

## Disadvantages

* **A** — a GPU user must pass `--features cuda`, and can silently get the CPU
  path if they forget.
* **B** — **breaks the default build for everyone without CUDA.** Disqualifying.
* **C** — checked-in binaries are opaque, unreviewable, and architecture-pinned;
  they also mean the `.cu` sources and the shipped artifact can diverge silently.
* **D** — a fifth crate for ~200 lines of FFI at MVP scale.

## Risks

* A feature-gated path is the least-exercised path. Mitigated by keeping the
  refusal test alive when the feature is off, so both branches are covered.
* `nvcc` version and driver skew. Mitigated by the compute-capability and
  runtime/driver checks in [`CUDA_ARCHITECTURE.md`](../CUDA_ARCHITECTURE.md) §4.

## Recommended default

**A.** `build.rs` + `nvcc -arch=sm_86`, no fast-math, behind a `cuda` feature
that is **off** by default. Keep `q-cuda` as one crate; split to **D** only if
the FFI exceeds a few hundred lines.

Fast-math off is not a detail: `-use_fast_math` would make the `1e-6` tolerance
against the CPU reference unachievable, and the reference is the entire
verification strategy.

## Tasks affected

`QM-0034` (implements), `QM-0035`, `QM-0036`, `QM-0083`.

## Decision deadline

Before `QM-0034` starts.
