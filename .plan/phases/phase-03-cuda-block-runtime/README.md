# Phase 03 — Block runtime and compute

## Goal

```text
Selected tensor block
→ bounded streaming read
→ statistics and quantized visual records          [CPU: critical path]
→ persisted, cached, checkpointed, resumable
   with CUDA as a parallel accelerator lane        [RTX 3090: off critical path]
```

## The phase's central scheduling decision

**The critical path runs on the CPU.** `q_gpu::CpuBackend` is `GPU-002 Verified`
with 7 tests, and `q_gpu::block_statistics_default` already computes what the tile
pyramid needs. Routing conversion through it makes the pipeline buildable and
demonstrable on the machine this repository is developed on.

CUDA replaces a backend behind the existing `q_gpu::Backend` trait and changes no
downstream artifact. `docs/decisions/ADR-008-track-b-prerequisite-waiver.md`
already waives the RTX 3090 gate.

If CUDA were on the critical path, the MVP would be unbuildable in the
environment that must build it.

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
| `QM-0034` | CUDA build integration, feature-gated | Implementation | E 🔧 | `CUDA-007`, `MVP-11` |
| `QM-0035` | CUDA reduction and histogram differential verification | Verification | E 🔧 | `CUDA-002`, `CUDA-003`, `CUDA-008`, `MVP-10`, `MVP-12` |
| `QM-0036` | CUDA quantization, Morton, and matmul verification | Verification | E 🔧 | `CUDA-004`, `CUDA-005`, `CUDA-008` |
| `QM-0037` | Backend selection, CPU fallback, determinism policy | Implementation | A | `GPU-001`, `CUDA-001` |

🔧 = requires an RTX 3090.

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
5. `cargo build --workspace` still succeeds with **no CUDA toolkit present**.
6. With `--features cuda` on an RTX 3090: kernels compile, run, and match the CPU
   reference within documented tolerances — **or** the requirements stay
   `Hardware-Unverified` and `QM-0034`…`QM-0036` stay `Implemented`.

## Parallelization

Lane A is sequential: `QM-0030` → `QM-0031` → `QM-0032` → `QM-0033`. Each builds
on the last.

Lane E (`QM-0034` → `QM-0035` → `QM-0036`) runs **entirely in parallel** and
blocks nothing in Lane A. `QM-0037` depends on `QM-0034` for the selection logic
but not on the hardware verification.

## Risks

| Risk | Mitigation |
| --- | --- |
| R3 — no RTX 3090 ever | Already handled: MVP completes with zero CUDA; requirements stay honestly `Hardware-Unverified` |
| R7 — conversion too slow | Conversion is scoped (`model \| subsystem \| layer \| tensor \| block`); jobs resume; CPU parallelism bounded by `MAX_CONCURRENT_BLOCKS` |
| Feature-gated CUDA path rots | The refusal test stays alive when the feature is off, so both branches are covered |
