# Phase 03 — Block runtime and compute

Directory name is historical (`phase-03-cuda-block-runtime`); **v1's GPU lane
implemented in this phase is Metal, not CUDA** — see the scheduling decision
below. CUDA work in this phase is the deferred next step, post-v1.

## Goal

```text
Selected tensor block
→ bounded streaming read
→ statistics and quantized visual records          [CPU: critical path]
→ persisted, cached, checkpointed, resumable
   with Metal as a parallel accelerator lane        [v1: off critical path]
   (CUDA as a further accelerator lane              [next step, post-v1, RTX 3090])
```

## The phase's central scheduling decision

**The critical path runs on the CPU.** `q_gpu::CpuBackend` is `GPU-002 Verified`
with 7 tests, and `q_gpu::block_statistics_default` already computes what the tile
pyramid needs. Routing conversion through it makes the pipeline buildable and
demonstrable on the machine this repository is developed on.

**Metal is v1's accelerator lane**, implementing the same `q_gpu::Backend`
trait and changing no downstream artifact (`ADR-CANDIDATE-003`, `Decided`).
**CUDA is deferred to the next step, post-v1** — it will implement the same
trait when RTX 3090 access is available.
`docs/decisions/ADR-008-track-b-prerequisite-waiver.md` already waives the
RTX 3090 gate, which is why v1 can ship complete without it.

If either GPU lane were on the critical path, the MVP would be unbuildable in
an environment lacking that hardware — which is why both stay accelerator
lanes beside the CPU critical path.

## Entry conditions

* Phase 00 complete; **G1** passed.
* `QM-0003`'s larger fixture available.
* `QM-0020` complete, so statistics have somewhere to go.

## Tasks

| ID | Title | Kind | Lane | Requirements |
| --- | --- | --- | --- | --- |
| `QM-0030` | Bounded streaming block reader | Implementation | A | `TILE-009`, `MVP-09` |
| `QM-0031` | CPU statistics pass over a whole tensor | Implementation | A | `STAT-008`, `PERF-001` |
| `QM-0032` | Wire the cache into the block and statistics paths | Implementation | A | `CACHE-008`, `API-012`, `MVP-17` |
| `QM-0033` | Conversion job executor | Implementation | A | `JOB-002`, `API-009`, `API-010`, `MVP-16` |
| *(new)* | Metal build integration, feature-gated | Implementation | E-Metal 🍎 | `GPU-003`, `MVP-11` (v1) |
| *(new)* | Metal reduction, histogram, quantization, Morton, matmul differential verification | Verification | E-Metal 🍎 | `GPU-003`, `MVP-10`, `MVP-12` (v1) |
| `QM-0037` | Backend selection, CPU fallback, determinism policy | Implementation | A | `GPU-001`, `GPU-003` (v1 Metal); `CUDA-001` (post-v1) |
| `QM-0034` | CUDA build integration, feature-gated | Implementation | F 🔧 (next step, post-v1) | `CUDA-007`, `MVP-11` |
| `QM-0035` | CUDA reduction and histogram differential verification | Verification | F 🔧 (next step, post-v1) | `CUDA-002`, `CUDA-003`, `CUDA-008`, `MVP-10`, `MVP-12` |
| `QM-0036` | CUDA quantization, Morton, and matmul verification | Verification | F 🔧 (next step, post-v1) | `CUDA-004`, `CUDA-005`, `CUDA-008` |

🍎 = Apple GPU (v1). 🔧 = requires an RTX 3090 (next step, post-v1). *(new)*
task IDs are to be minted when Metal implementation tasks are created; they
are not yet in `.plan/tasks/`.

## Design constraints

* **Every buffer is bounded and named.** Formulas in
  [`MEMORY_BUDGET.md`](../../MEMORY_BUDGET.md); defaults `MAX_HOST_STAGING_BYTES`
  512 MiB, `MAX_CONCURRENT_BLOCKS` 4, `MAX_OUTPUT_QUEUE_DEPTH` 64.
* **Backpressure, not buffering.** A full output queue blocks the reader.
  Growing the queue converts a throughput problem into an out-of-memory crash,
  which destroys the completed work a stalled pipeline preserves.
* **Peak RSS must not grow with tensor size** — only with block count.
  `PERF-001` asserts this as a test, not a benchmark, because a regression here
  breaks the architecture's premise.
* **Blocks are clamped, never padded.** A padded block puts fabricated zeros into
  a statistic.
* **CPU is the numerical reference.** Any other backend is diffed against it, and
  a divergence beyond tolerance fails the job rather than being recorded.
* **`auto` backend selection announces its choice** in the job record and the API
  response. A user who believes a run was GPU-accelerated and reads CPU timings
  has been misled by silence.

## Exit conditions

1. A 4096×4096 f32 tensor converts fully on CPU with peak RSS **< 32 MB**,
   measured and recorded.
2. Statistics rows exist for every block and survive a reopen.
3. A second conversion of the same tensor reports cache hits and skips compute.
4. A job killed mid-conversion resumes and produces **byte-identical** output.
5. `cargo build --workspace` still succeeds with **no CUDA toolkit present**
   and with no Metal-capable GPU present (CPU-only fallback).
6. With `--features metal` on Apple GPU hardware (v1): kernels compile, run,
   and match the CPU reference within documented tolerances.
7. With `--features cuda` on an RTX 3090 (next step, post-v1): kernels
   compile, run, and match the CPU reference within documented tolerances —
   **or** the requirements stay `Hardware-Unverified` and `QM-0034`…`QM-0036`
   stay `Implemented`. Not required for v1.

## Parallelization

Lane A is sequential: `QM-0030` → `QM-0031` → `QM-0032` → `QM-0033`. Each builds
on the last.

Lane E-Metal (the new Metal build/verification tasks) runs **entirely in
parallel** with Lane A and is part of v1. `QM-0037` depends on the Metal build
task for the v1 selection logic.

Lane F (`QM-0034` → `QM-0035` → `QM-0036`, CUDA) is the deferred next step,
post-v1. It runs entirely in parallel with Lanes A and E-Metal whenever it is
taken up and blocks nothing in either.

## Risks

| Risk | Mitigation |
| --- | --- |
| R3 — no RTX 3090 ever | Already handled: v1 completes with zero CUDA via the Metal lane; CUDA requirements stay honestly `Hardware-Unverified` until the next step lands |
| R7 — conversion too slow | Conversion is scoped (`model \| subsystem \| layer \| tensor \| block`); jobs resume; CPU parallelism bounded by `MAX_CONCURRENT_BLOCKS` |
| Feature-gated CUDA path rots | The refusal test stays alive when the feature is off, so both branches are covered |
| Feature-gated Metal path rots | Same discipline: the refusal test stays alive when the `metal` feature is off |
