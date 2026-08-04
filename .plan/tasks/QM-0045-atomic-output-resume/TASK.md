# QM-0045 — Atomic output and resume manifests

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 04 — Tensor tiles, GLB, and tileset

## Objective

Guarantee that a partially written artifact is **never** visible under its final
name, and that a resumed conversion produces byte-identical output.

## Repository Evidence

* `q_catalog::job` — `failed_and_cancelled_jobs_can_resume` (`JOB-001` Verified).
* `crates/q-source/src/cancel.rs` — `resume_skips_completed_shards` for
  ingestion; the same shape applies here.
* `tensor_blocks.content_hash` (from `QM-0022`) — what makes resume **verify**
  rather than trust.
* `Cargo.toml` — `tempfile` is already a dependency.
* `QM-0033`'s executor checkpoints per block.

## Requirements Covered

`TILE-011`, `MVP-16`.

## Dependencies

`QM-0044`, `QM-0033`, `QM-0022`.

## Blocks

`QM-0046`, `QM-0081`.

## Parallelization

Lane A, after `QM-0044`. Touches `q-tiles`, `q-gltf`, `q-tileset`, `q-daemon`.

## Program Boundary

All three artifact crates plus the job executor.

## Scope

* Every write: temp file → `fsync` → atomic `rename`.
* `tileset.json` written **last**, after every referenced file exists.
* Resume: reload the block manifest, verify `content_hash` against the file on
  disk, skip only what matches.
* Sweep orphaned `*.tmp.<job_id>` files on resume and on cancel.

## Out of Scope

Cross-filesystem atomicity (out of scope: outputs go under one root) ·
transactional multi-file commit · external validation (`QM-0046`).

## Files Expected to Change

* `crates/q-tiles/src/pyramid.rs`
* `crates/q-gltf/src/instanced.rs`
* `crates/q-tileset/src/builder.rs`
* `crates/q-daemon/src/jobs.rs`

## Files Expected to Add

* `crates/q-daemon/src/atomic.rs`
* `tests/tests/resume_atomicity.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```text
write   → <name>.tmp.<job_id>
fsync   → the file, then the directory
rename  → <name>            atomic on the same filesystem
```

The manifest is `tensor_blocks` rows plus the job record. A resumed job compares
each row's `content_hash` against the artifact on disk; **a mismatch is redone,
not trusted** — a truncated file whose row says "done" is exactly the case this
guards.

## Memory and Performance Constraints

`fsync` per file costs milliseconds. At 256 tiles that is a bounded, acceptable
cost, and it is the difference between "probably complete" and "complete".
Directory `fsync` is required for rename durability on ext4 and APFS.

## Implementation Plan

1. `AtomicWriter`: temp path, write, `fsync` file, `rename`, `fsync` directory.
2. Route all three artifact writers through it.
3. Order the job's `Writing` phase so `tileset.json` is last.
4. Resume: for each completed row, stat and hash the artifact; matching rows are
   skipped, others requeued.
5. Sweep `*.tmp.<job_id>` on resume and on cancel.
6. Failure-injection tests: kill during write, kill between files, kill before
   the tileset.

## Error Handling

* Rename failure → the temp file is removed; the block is marked failed.
* Disk full mid-write → fail before rename; **nothing is published**.
* A hash mismatch on resume → redo that block; log it.
* An orphaned temp from a *different* job → left alone, since another job may own
  it.
* Crash between `fsync` and `rename` → the temp file remains and is swept.

## Acceptance Criteria

1. Killing the process mid-write leaves **no file under a final name**.
2. A resumed job produces byte-identical output to an uninterrupted run.
3. Resume skips blocks whose `content_hash` matches, verified by a compute
   counter.
4. A truncated artifact whose row claims completion is **redone**.
5. `tileset.json` never references a missing file, in any interruption case.
6. Orphaned temps for this job are swept; other jobs' temps are not.
7. Disk-full leaves the output directory consistent.
8. 50 kill-and-resume cycles produce identical final output.

## Verification Plan

**Automated** — `resume_atomicity.rs` with SIGKILL injection at three points, and
a byte-identity comparison.
**Manual** — kill during a large-fixture conversion; inspect the directory; resume.

## Suggested Commands

```bash
cargo test --test resume_atomicity                              # introduced here
cargo run -p q-cli -- convert fixtures/tiny-llama-large &  kill -9 %1
cargo run -p q-cli -- convert fixtures/tiny-llama-large --resume
find out -name '*.tmp.*'
sha256sum -c out/manifest.sha256
```

## Test Cases

| Input | Expected |
| --- | --- |
| SIGKILL during a `.qtile` write | No final-named file; one temp remains |
| SIGKILL between two GLB writes | Completed files intact; no partial finals |
| SIGKILL before `tileset.json` | No tileset; tiles intact; resume completes it |
| Resume after each | Byte-identical to an uninterrupted run |
| Truncate a completed artifact, resume | That block redone |
| Another job's temp present | Untouched |
| Disk full | Fails; no final-named partial |
| 50 kill/resume cycles | Identical output every time |

## Risks

| Risk | Mitigation |
| --- | --- |
| Rename is not atomic across filesystems | Outputs are confined to one root; asserted at start |
| `fsync` cost dominates | Measured; bounded at 256 tiles per tensor |
| A resumed job trusts a corrupt artifact | Hash verified, not assumed |

## Completion Evidence

* Kill-injection transcripts at all three points.
* Byte-identity comparison (`sha256sum`) between interrupted-then-resumed and
  uninterrupted runs.
* The compute-counter comparison proving skipping.
* `find out -name '*.tmp.*'` empty after resume.
