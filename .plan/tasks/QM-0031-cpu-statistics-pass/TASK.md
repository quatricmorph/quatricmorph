# QM-0031 — CPU statistics pass over a whole tensor

## Status

Blocked

Unblocks when `QM-0030` and `QM-0020` reach `Complete`. (`QM-0022` dropped — deferred.)

**v1 dependency rewiring.** This task's `## Dependencies` section names tasks that are now `Deferred`. For v1 it is unblocked by the tasks named above; the original edges return with the post-v1 platform release. See [`EXECUTION_ORDER.md`](../../EXECUTION_ORDER.md) §10.


**v1 role.** Retained, and distinct from `QM-0122`. This task computes **single-tensor** statistics (`STAT-008`) and persists them, which the manifest's per-tensor block and `QM-0032`'s cache path both consume. `QM-0122` computes **paired** diagnostic metrics and does not replace it. If `QM-0122` lands first and subsumes the persistence path in practice, reduce this task's scope by editing it — do not leave the overlap undecided.
## Phase

Phase 03 — Block runtime and compute

## Objective

Run block statistics over an entire tensor on the CPU backend, persist per-block
and per-tensor rows, and prove peak RSS is **independent of tensor size**.

## Repository Evidence

* `q_gpu::CpuBackend` — `GPU-002` Verified, 7 tests, declared the reference.
* `q_gpu::block_statistics_default` (`crates/q-gpu/src/lib.rs:231`).
* `q_statistics::StatisticsAccumulator` — Welford, `push_bytes(bytes, dtype)`,
  `mark_approximate`, `finish(backend)`; `streaming_in_chunks_equals_computing_at_once`.
* `q_gpu::tests::block_statistics_stream_a_real_fixture_block` — one block only.
* `QM-0020` provides persistence; `QM-0030` provides the stream.

## Requirements Covered

`STAT-008`, `PERF-001`.

## Dependencies

`QM-0030`, `QM-0020`, `QM-0022`.

## Blocks

`QM-0032`, `QM-0033`, `QM-0041`.

## Parallelization

Lane A, sequential after `QM-0030`.

## Program Boundary

`crates/q-statistics`, `crates/q-gpu`, `crates/q-catalog`, `crates/q-cli`.

## Scope

* `TensorStatisticsPass`: stream blocks → per-block statistics → accumulate a
  tensor-level roll-up → persist both.
* Tensor-level statistics computed by **merging block accumulators**, not by a
  second read.
* Histogram range from a first pass over block min/max, then a second pass to
  bin — or a single pass with a widening histogram, whichever the test shows is
  exact.
* Batch catalog writes in transactions.
* Instrument peak RSS.

## Out of Scope

GPU execution (Lane E) · caching (`QM-0032`) · job orchestration (`QM-0033`) ·
tile generation.

## Files Expected to Change

* `crates/q-statistics/src/lib.rs` — accumulator merge
* `crates/q-gpu/src/lib.rs` — pass driver
* `crates/q-cli/src/main.rs` — `stats --whole-tensor`

## Files Expected to Add

* `crates/q-gpu/tests/tensor_pass.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

Per-block rows keyed by `BlockId`; the tensor-level row keyed by `tensor_id`,
both with `algorithm_version = 1` and `approximate = false`.

**Merging must be exact.** `merge(a, b)` on two Welford accumulators must equal
computing over the concatenation — asserted, because a merge formula error would
produce plausible statistics that are quietly wrong.

## Memory and Performance Constraints

```text
peak = stream peak (1 MiB) + accumulators (256 × ~1 KiB) + write batch
     ≈ 12 MiB   for a 64 MiB tensor
budget: < 32 MiB, and FLAT across 1024², 2048², 4096²
time:   < 5 s for 4096×4096 f32 single-threaded
```

## Implementation Plan

1. Add `StatisticsAccumulator::merge`, with an exactness test against
   concatenation.
2. Drive `BlockStream`; per block, `block_statistics_default` → row.
3. Merge into the tensor accumulator.
4. Persist per-block rows in batches, then the tensor row.
5. Add `q-cli stats --whole-tensor`.
6. Measure peak RSS at three tensor sizes; assert flatness.

## Error Handling

* A block read failure → the pass fails naming the block; partial rows already
  written stay, since they are keyed by content and a resumed pass can skip them.
* A merge over zero blocks → error, not a zero-count row.
* NaN or Inf in the data → counted and reported, **not silently dropped**; the
  row carries a `non_finite_count`.
* Cancellation between blocks → completed rows persist.

## Acceptance Criteria

1. A 4096×4096 f32 tensor produces 256 block rows and 1 tensor row.
2. The tensor row equals a direct computation over all values, to `1e-9`.
3. Peak RSS < 32 MiB and **flat** across 1024², 2048², 4096².
4. Runtime < 5 s for 4096² single-threaded.
5. `merge` equals concatenation, asserted.
6. Rows survive reopen.
7. Non-finite values are counted, not dropped.
8. `STAT-001`, `STAT-003`, `STAT-004` still pass.

## Verification Plan

**Automated** — `tensor_pass.rs`, including the flat-peak assertion and the merge
exactness test.
**Manual** — `/usr/bin/time -l` on the CLI over the large fixture.

## Suggested Commands

```bash
cargo test -p q-gpu -p q-statistics                              # verified today
cargo run -p q-cli -- stats fixtures/tiny-llama-large <addr> --whole-tensor  # new
/usr/bin/time -l cargo run --release -p q-cli -- stats … --whole-tensor
```

## Test Cases

| Input | Expected |
| --- | --- |
| 4096² f32 | 256 block rows + 1 tensor row |
| Tensor row vs direct computation | Equal to `1e-9` |
| Peak RSS at 1024², 2048², 4096² | All < 32 MiB, within 10 % of each other |
| `merge(a, b)` vs concatenation | Equal to `1e-12` |
| Tensor containing NaN | `non_finite_count` > 0; mean not NaN-poisoned |
| Cancel after 100 blocks | 100 rows persisted |
| Reopen | All rows intact |

## Risks

| Risk | Mitigation |
| --- | --- |
| Merge formula subtly wrong | Asserted against concatenation, not against itself |
| Catalog writes dominate runtime | Batched; the 5 s budget includes them |
| NaN poisons a roll-up | Counted separately; asserted |

## Completion Evidence

* Peak RSS at three tensor sizes.
* Runtime for 4096².
* Merge-exactness test output.
* Row counts and a sample tensor row.
