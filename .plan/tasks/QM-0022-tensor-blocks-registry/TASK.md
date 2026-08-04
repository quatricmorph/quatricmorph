# QM-0022 — `tensor_blocks` registry wired to conversion

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 02 — Catalog and NSIR completion

## Objective

Populate `tensor_blocks` so a conversion can record what it has done, and a
resumed job can skip verified work.

## Repository Evidence

* `crates/q-catalog/src/schema.rs:75` — `tensor_blocks(block_id, tensor_id, lod,
  row_start, row_end, column_start, column_end, source_byte_ranges,
  statistics_id, content_hash)`, index `(tensor_id, lod)`. **Never written.**
* `q_tensor_runtime::TensorBlock::plan` — derives one byte run per row without
  reading (`TILE-002` verified).
* `q_tensor_runtime::SourceByteRanges` — `total_bytes()`, `run_count()`.
* `TileId::content_hash()` exists.
* `q_catalog::job` — `JOB-001`, `JOB-003` verified; `failed_and_cancelled_jobs_can_resume`.

## Requirements Covered

`CAT-013`; enables `JOB-002`, `TILE-011`, `MVP-16`, `MVP-17`.

## Dependencies

`QM-0021`.

## Blocks

`QM-0033`, `QM-0041`, `QM-0045`.

## Parallelization

**Sequential after `QM-0021`** — shared file `crates/q-catalog/src/lib.rs`.

## Program Boundary

`crates/q-catalog`, `crates/q-tensor-runtime`.

## Scope

* Insert and query block rows, keyed by `BlockId = TileId::for_block(...)`.
* Serialize `SourceByteRanges` compactly.
* `blocks_for_tensor(tensor_id, lod)`, `completed_blocks(job_id)`,
  `block_by_id`.
* `content_hash` is what makes resume able to **verify** rather than trust.

## Out of Scope

Running a conversion (`QM-0031`, `QM-0033`) · generating tiles · GPU work.

## Files Expected to Change

* `crates/q-catalog/src/lib.rs`
* `crates/q-tensor-runtime/src/lib.rs` — serialization helper for
  `SourceByteRanges`

## Files Expected to Add

None.

## Files Expected to Remove or Deprecate

None.

## Data Contracts

`source_byte_ranges` is stored as a length-prefixed little-endian blob of
`(u64 start, u64 end)` pairs — **not JSON**. A 256-row block has 256 runs; JSON
would be ~8 KB per row against 4 KB binary, times millions of rows.

```text
[u32 run_count][u64 start][u64 end]…
```

`content_hash` covers the **produced artifact**, not the source bytes: it is what
a resumed job compares against the file on disk.

## Memory and Performance Constraints

* Block rows are the highest-volume table: a 4096×4096 tensor at 256×256 is 256
  rows; a 7 B model is ~100 000.
* **Inserts must be batched in a transaction.**
* `blocks_for_tensor` is indexed by `(tensor_id, lod)` — the existing index
  serves it.
* Storage: ~4 KB per row at 256 runs. 100 000 rows ≈ 400 MB — which is why the
  blob format matters, and why `QM-0031` measures it.

## Implementation Plan

1. `SourceByteRanges` binary encode/decode with a round-trip test.
2. Block row struct ↔ `TensorBlock`.
3. `put_blocks(&[TensorBlock])` — batched, one transaction.
4. `blocks_for_tensor`, `completed_blocks`, `block_by_id`.
5. Link `statistics_id` to the rows `QM-0020` writes.
6. Tests: round trip, batch insert timing, reopen, resume-shaped query.

## Error Handling

* A block extent outside the tensor's shape → refused
  (`BlockExtent::clamped_to` already exists for the legal case).
* `source_byte_ranges` decoding to a different run count than declared →
  malformed.
* A duplicate `block_id` → replaces; the ID is content-derived, so the same
  inputs mean the same block.
* `statistics_id` referencing a missing row → refused.

## Acceptance Criteria

1. 256 block rows for one 4096×4096 tensor insert in a single transaction.
2. `SourceByteRanges` round-trips byte-exactly.
3. `blocks_for_tensor` returns them in a deterministic order.
4. `completed_blocks(job_id)` supports resume.
5. Rows survive reopen.
6. Inserting 100 000 rows completes under 10 s and is measured.
7. An out-of-shape extent is refused.
8. Storage per row is measured and recorded.

## Verification Plan

**Automated** — catalog tests; a timed 100 000-row insert.
**Manual** — `sqlite3` page-count check for storage per row.

## Suggested Commands

```bash
cargo test -p q-catalog -p q-tensor-runtime          # verified today
sqlite3 <catalog.db> "SELECT count(*) FROM tensor_blocks;"   # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| 256 blocks of a 4096×4096 tensor | All inserted, one transaction |
| `SourceByteRanges` with 256 runs | Round-trips byte-exactly |
| `blocks_for_tensor(t, LOD 4)` | 256 rows, deterministic order |
| Extent `[5000:5256]` on a 4096-row tensor | Refused |
| Duplicate `block_id` | Replaces, no duplicate row |
| `statistics_id` pointing nowhere | Refused |
| 100 000 rows | Under 10 s; storage measured |
| Reopen | Rows intact |

## Risks

| Risk | Mitigation |
| --- | --- |
| Merge conflict with `QM-0020`/`QM-0021` | Strict sequence; this is last |
| Block rows dominate catalog size | Binary blob, not JSON; storage measured here and in `QM-0031` |
| Insert throughput dominates conversion | Batched transactions; measured |

## Completion Evidence

* Round-trip and batch-insert test output with timing.
* Measured storage per row and total for 100 000 rows.
* Reopen test output.
