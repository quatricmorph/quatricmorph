# QM-0081 — Cache reuse, resume, and failure injection

## Status

Blocked

Unblocks when `QM-0033` and `QM-0143` reach `Complete`. (Was `QM-0080`, now deferred.)

**v1 dependency rewiring.** This task's `## Dependencies` section names tasks that are now `Deferred`. For v1 it is unblocked by the tasks named above; the original edges return with the post-v1 platform release. See [`EXECUTION_ORDER.md`](../../EXECUTION_ORDER.md) §10.

## Phase

Phase 08 — Integration and performance

## Objective

Prove the pipeline survives being interrupted, corrupted, and starved — and that
a resumed run is byte-identical to an uninterrupted one.

## Repository Evidence

* `QM-0045` implements atomic writes and resume manifests.
* `QM-0032` wires the cache; `l2_is_reused_after_reopen` (`CACHE-004` Verified).
* `QM-0033`'s executor checkpoints per block.
* `resume_skips_completed_shards` (`SRC-010`) — the ingestion precedent.
* `CESIUM_VIEWER_ARCHITECTURE.md` §9 — the eleven designed error states.

## Requirements Covered

`CACHE-008`, `JOB-002`, `TILE-011`, `MVP-16`, `MVP-17`, `AC-008`.

## Dependencies

`QM-0080`.

## Blocks

`QM-0094`.

## Parallelization

Parallel with `QM-0082`…`QM-0085` after `QM-0080`.

## Program Boundary

`tests/`, plus a failure-injection harness.

## Scope

The eight-case failure table from
[`phases/phase-08`](../../phases/phase-08-integration-and-performance/README.md),
plus cache reuse and 50 kill-and-resume cycles.

## Out of Scope

Memory soaks (`QM-0082`, `QM-0083`) · performance (`QM-0084`) · new features.

## Files Expected to Change

* `.github/workflows/build.yaml`

## Files Expected to Add

* `tests/tests/failure_injection.rs`
* `scripts/inject-failure.sh`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

The decisive comparison: `sha256` of every artifact from an uninterrupted run
versus a killed-and-resumed run. **Byte-identical, or the resume logic is
wrong.**

## Memory and Performance Constraints

50 kill-and-resume cycles over a small tensor must complete in under 10 minutes,
or the test will not be run.

## Implementation Plan

1. Build the injection harness: SIGKILL at a chosen block, disk-full simulation,
   artifact corruption, artifact deletion.
2. Reference run: convert uninterrupted, hash every artifact.
3. Interrupted runs: kill at blocks 1, 50, and 255; resume; hash; compare.
4. Corruption: truncate a completed artifact, resume, assert it is redone.
5. Cache: run twice, assert hits and a compute-counter delta of zero.
6. Viewer error states: delete a tile, corrupt a tile, stop the daemon.
7. 50-cycle loop.

## Error Handling

The task **is** error handling. Every injected failure must produce the designed
state from the architecture document — never a crash, never a silent partial.

## Acceptance Criteria

1. Killing at any block leaves **no file under a final name**.
2. Resume produces byte-identical artifacts to the reference run.
3. A truncated artifact whose row claims completion is redone.
4. A second conversion reports cache hits and a zero compute delta.
5. Restarting the daemon still hits L2.
6. Disk-full fails cleanly with a consistent output directory.
7. A deleted tile shows a placeholder; siblings render.
8. A corrupted GLB fails that tile alone.
9. A stopped daemon shows the banner with the start command.
10. 50 kill/resume cycles produce identical output every time.

## Verification Plan

**Automated** — `failure_injection.rs` and a Playwright suite for the viewer
states.
**Manual** — review the hash comparison table.

## Suggested Commands

```bash
cargo test --test failure_injection                # introduced here
./scripts/inject-failure.sh kill-at-block 50        # introduced here
sha256sum -c out/reference.sha256
```

## Test Cases

| Injected failure | Expected |
| --- | --- |
| SIGKILL at block 1 / 50 / 255 | No final-named partials; resume identical |
| Truncate a completed artifact | Redone on resume |
| Second conversion | Cache hits; compute delta 0 |
| Daemon restart, then convert | L2 hit |
| Disk full | Clean failure; directory consistent |
| Delete a tile, view | Placeholder; siblings render |
| Corrupt a GLB, view | That tile alone fails |
| Stop the daemon, view | Banner with the start command |
| Cancel mid-conversion | Stops within one block; manifest intact |
| 50 kill/resume cycles | Identical artifacts every cycle |

## Risks

| Risk | Mitigation |
| --- | --- |
| Injection is unrealistic and passes trivially | SIGKILL is a real kill, not a graceful shutdown |
| Flakiness under injection | Each case is deterministic: a fixed block index, a fixed file |
| 50 cycles are slow | Small tensor; 10-minute budget |

## Completion Evidence

* Hash comparison table: reference versus each interrupted run.
* Cache hit counts and compute-counter deltas.
* Screenshots of each viewer error state.
* The 50-cycle result summary.
