# QM-0030 — Bounded streaming block reader

## Status

Complete

`QM-0100` is `Complete` and merged, so the v1 edge is satisfied. (Was `QM-0003`, now deferred — v1 streams the **real** checkpoint, not an LOD fixture.)

**v1 dependency rewiring.** This task's `## Dependencies` section names tasks that are now `Deferred`. For v1 it is unblocked by the tasks named above; the original edges return with the post-v1 platform release. See [`EXECUTION_ORDER.md`](../../EXECUTION_ORDER.md) §10.

## Phase

Phase 03 — Block runtime and compute

## Objective

Stream a tensor's blocks through bounded, named buffers with backpressure, so
peak memory depends on block size and concurrency — **never on tensor size**.

## Repository Evidence

* `q_tensor_runtime::TensorBlock::plan` — one byte run per row, no reads
  (`TILE-002`).
* `q_tensor_runtime::SourceByteRanges::total_bytes()` / `run_count()`.
* `crates/q-source/src/local.rs` — mmap range reads; `range_read_returns_exactly_the_window`,
  `range_past_end_of_file_is_rejected`, `path_traversal_is_refused`.
* `crates/q-source/src/budget.rs` — named budgets; `a_tight_metadata_budget_is_enforced`.
* `crates/q-source/src/dtype.rs` — exact f32/bf16/f16 decode incl. subnormals.
* `crates/q-source/src/cancel.rs` — the cancellation token used by ingestion.
* `crates/q-gpu/src/lib.rs:105` — `BlockData::new(rows, columns, values)`.

## Requirements Covered

`TILE-009`, `MVP-09`; enables `STAT-008`, `PERF-001`.

## Dependencies

`QM-0003`.

## Blocks

`QM-0031`, `QM-0032`, `QM-0033`, `QM-0041`.

## Parallelization

First task in Lane A; runs alone until `QM-0031`.

## Program Boundary

`crates/q-tensor-runtime` (new `stream.rs`), `crates/q-source` (budgets).

## Scope

* `BlockStream`: iterate a tensor's blocks in a deterministic order, reading each
  through `ModelSource::read_range`, decoding to `BlockData`.
* Bounded buffers per [`MEMORY_BUDGET.md`](../../MEMORY_BUDGET.md) §4:
  `MAX_HOST_STAGING_BYTES`, `MAX_CONCURRENT_BLOCKS`, `MAX_OUTPUT_QUEUE_DEPTH`.
* **Backpressure**: a full output queue blocks the reader.
* Cancellation checked **between blocks**.
* Adaptive halving on allocation failure, floor 64×64.

## Out of Scope

Statistics (`QM-0031`) · caching (`QM-0032`) · jobs (`QM-0033`) · GPU transfer ·
writing artifacts.

## Files Expected to Change

* `crates/q-tensor-runtime/src/lib.rs` — export the module
* `crates/q-source/src/budget.rs` — add the three budgets

## Files Expected to Add

* `crates/q-tensor-runtime/src/stream.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

> **CONTROLLER CORRECTION, 2026-08-05.** This section specified
> `StreamedBlock.data: BlockData` with `BlockData` living in `q-gpu`. **That is
> unsatisfiable: `crates/q-gpu/Cargo.toml` declares `q-tensor-runtime` under
> `[dependencies]`, so the contract as written is a dependency cycle.**
> `QM-0030` moved `BlockData` into `q-tensor-runtime` — character-for-character
> identical — and left `pub use q_tensor_runtime::BlockData;` at
> `q-gpu/src/lib.rs:109`, keeping `q_gpu::BlockData` valid for `CpuBackend`, the
> `Backend` trait, nine unit tests and `q-cuda:35,138,167`. `review-agent-10`
> confirmed the cycle is real and ruled this a **plan defect correctly handled**,
> not an out-of-scope edit. Recorded in `.plan/PLAN_CHANGELOG.md`.


```rust
pub struct BlockStreamConfig {
    pub block_rows: u64, pub block_columns: u64,
    pub max_host_staging_bytes: u64,   // default 512 MiB
    pub max_concurrent_blocks: usize,  // default 4
    pub max_output_queue_depth: usize, // default 64
    pub min_block_dimension: u64,      // floor 64
}
pub struct StreamedBlock {
    pub extent: BlockExtent, pub block_id: TileId,
    pub data: BlockData, pub bytes_read: u64,
}
```

Block order is **row-major over the block grid**, deterministic, so a resumed job
and a fresh job visit blocks identically.

## Memory and Performance Constraints

```text
peak = max_concurrent_blocks × block_rows × block_columns × 4
     = 4 × 256 × 256 × 4 = 1 MiB   at defaults
```

**Independent of tensor size.** This is `PERF-001`, and it is asserted as a test,
not a benchmark — a regression here breaks the architecture's premise.

I/O: 256 runs of 1 KiB at a 16 KiB stride for a 256-column window of a
4096-column f32 tensor — 256 KiB read, not 4 MiB.

## Implementation Plan

1. `BlockStream::new(source, descriptor, config)`; validate the config against
   the budgets.
2. Generate the block grid via `BlockExtent::clamped_to` — **clamped, never
   padded**.
3. For each block: `TensorBlock::plan` → `read_range` per run → decode → emit.
4. Bounded channel for output; the reader blocks when full.
5. Check the cancellation token between blocks.
6. On allocation failure, halve both dimensions and retry; below the floor, fail
   naming the budget.
7. Instrument peak allocation for the test.

## Error Handling

* A read past the tensor's extent → refused (`descriptor.rs` already does).
* A short read → error naming the block and the byte range; never zero-filled.
* Unknown dtype → refused, never guessed.
* Cancellation → stop at the block boundary, return blocks completed so far.
* Allocation failure → halve and retry; then fail naming the budget.
* **A block is never silently skipped.**

## Acceptance Criteria

1. A 4096×4096 f32 tensor streams as 256 blocks of 256×256.
2. Peak allocation ≤ 2 MiB at defaults, **measured**, and unchanged when the
   tensor is 1024², 2048², or 4096².
3. Edge blocks on a non-multiple shape are clamped, not padded — asserted by
   element count.
4. Decoded values match `golden.json` for known indices.
5. Cancellation stops at a block boundary; completed blocks are returned.
6. `MAX_OUTPUT_QUEUE_DEPTH` produces observable backpressure.
7. Allocation failure halves the block and completes.
8. Block order is identical across runs.
9. bf16 and f16 tensors stream with exact decoding.

## Verification Plan

**Automated** — unit tests plus a peak-allocation assertion at three tensor
sizes.
**Manual** — `/usr/bin/time -l` on a CLI stream of the large fixture.

## Suggested Commands

```bash
cargo test -p q-tensor-runtime -p q-source          # verified today
cargo run -p q-cli -- stream fixtures/tiny-llama-large <address> --block 256   # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| 4096×4096 f32, 256×256 blocks | 256 blocks, peak ≤ 2 MiB |
| 1024², 2048², 4096² | **Same peak** |
| 4000×4000, 256×256 | Edge blocks clamped; last is 160×160 |
| Cancel after 10 blocks | 10 returned, stops at a boundary |
| `max_output_queue_depth = 1` | Reader blocks; no unbounded growth |
| Simulated allocation failure at 256 | Retries at 128, completes |
| Failure down to 64 | Fails naming the budget |
| bf16 tensor | Exact decode vs `golden.json` |
| Two runs | Identical block order |

## Risks

| Risk | Mitigation |
| --- | --- |
| Peak memory grows with tensor size | Asserted at three sizes; the test is the guard |
| Backpressure deadlocks | Bounded channel with a documented ordering; a timeout test |
| A short read silently zero-fills | Explicit error naming block and range |

## Completion Evidence

* Peak-allocation measurements at 1024², 2048², 4096².
* Block-count and clamping assertions.
* Cancellation and backpressure test output.
* `/usr/bin/time -l` output from the CLI run.

---

## Orchestration

| Field | Value |
| --- | --- |
| Controller state | `Awaiting Independent Review` |
| Lane | P |
| Branch | `task/qm-0030-streaming-block-reader` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0030` |
| Base commit | `6fb593a` |
| Commits on the branch | `f4fba1e` implementation · `e4061cd` plan-only (records the SHA) · `eea5950` test-setup fix + `baseline.json` commit repin. A trailing plan-only commit records this list. **`git log --oneline main..HEAD` is authoritative for the tip** — a commit cannot contain its own hash, so no field here can name it. |
| Agent | `impl-agent-8` |
| Evidence | [`.plan/evidence/QM-0030.md`](../../evidence/QM-0030.md) |
| Merge path | L |
| Tests added | 43 (30 in `q-tensor-runtime` `stream.rs`, 4 in `q-source` `budget.rs`, 1 in `tests/bounded_residency.rs`, 8 in `tests/real_fixture_blocks.rs`) |
| Floor before | rust 434 tests over 43 binaries |
| Floor after | rust 477 tests over 45 binaries (`scripts/baseline.json` raised; web 115/13 untouched) |

**Deviation a reviewer must check.** `TASK.md` *Data Contracts* specifies
`StreamedBlock.data: BlockData` and *Repository Evidence* points at
`crates/q-gpu/src/lib.rs:105`, but `q-gpu` depends on `q-tensor-runtime`, so that
import would be a dependency cycle. `BlockData` was moved into
`q-tensor-runtime` and re-exported from `q-gpu` in one line — the only edit
outside the stated program boundary. Rationale and verification in
`.plan/evidence/QM-0030.md`.

**Not claimed:** gate `G1` (that is `QM-0101`'s, and needs a configured residency
ceiling that does not exist yet), `GRID-007` as `Verified` (`QM-0061` /
`QM-0040`), and both `.plan/PERFORMANCE_PLAN.md` §2.2 latency budgets, which were
not measured.
