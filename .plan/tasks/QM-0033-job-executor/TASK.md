# QM-0033 — Conversion job executor

## Status

Blocked

Unblocks when `QM-0032` reaches `Complete`.

## Phase

Phase 03 — Block runtime and compute

## Objective

Build the thing that runs a job. Everything about jobs exists except execution.

## Repository Evidence

* `STATUS.md` `JOB-002` — *"Job runner (anything that actually executes a job)"*,
  **Stub**, 501.
* `crates/q-catalog/src/job.rs` (239) — the state machine;
  `legal_transitions_are_accepted`, `illegal_transitions_are_rejected`,
  `failed_and_cancelled_jobs_can_resume`. `JOB-001`, `JOB-003` Verified.
* `crates/q-daemon/src/lib.rs:659` — `conversions_501`.
* `crates/q-source/src/cancel.rs` — the token ingestion already uses;
  `cancellation_stops_at_a_shard_boundary`, `resume_skips_completed_shards`.
* `ADR-CANDIDATE-011` — HTTP + SSE.

## Requirements Covered

`JOB-002`, `API-009`, `API-010`, `MVP-16`.

## Dependencies

`QM-0032`, `QM-0022`.

## Blocks

`QM-0041`, `QM-0045`, `QM-0081`.

## Parallelization

Lane A, sequential after `QM-0032`. Touches `q-daemon` — coordinate with
`QM-0020`'s route work if overlapping.

## Program Boundary

`crates/q-daemon` (new `jobs.rs`), `crates/q-catalog`.

## Scope

* A single-executor job runner: `Pending → Inspecting → Indexing → Converting →
  Writing → Validating → Complete`, with `Paused`, `Cancelled`, `Failed`.
* Checkpoint after **each block**.
* `POST /v1/conversions`, `GET /v1/jobs/{id}`, SSE `/events`, `POST /cancel`,
  `POST /resume`.
* Scoped conversion: `model | subsystem | layer | tensor | block`.
* Graceful shutdown: an in-flight job pauses rather than being lost.

## Out of Scope

Tile or GLB generation (Phase 04) — this task runs the **statistics** pass ·
multi-job concurrency · distributed workers.

## Files Expected to Change

* `crates/q-daemon/src/lib.rs`
* `crates/q-daemon/src/main.rs`
* `crates/q-catalog/src/job.rs`

## Files Expected to Add

* `crates/q-daemon/src/jobs.rs`

## Files Expected to Remove or Deprecate

* `conversions_501` — replaced. `unbuilt_routes_return_501_with_a_requirement_id`
  is **narrowed**, not deleted.

## Data Contracts

Job record per the task specification §23: job ID, source model ID, conversion
version, configuration hash, current phase, current tensor, current block,
completed blocks, failed blocks, bytes read, bytes written, GPU time, CPU time,
cache hits, errors, started, updated.

```text
event: progress
data: {"job_id":"…","state":"Converting","phase":"blocks",
       "current_tensor":"…","blocks_done":412,"blocks_total":1024,
       "bytes_read":107374182,"bytes_written":5242880,
       "cache_hits":88,"elapsed_ms":18300}
```

`configuration_hash` covers block size, encoding, backend, and LOD range — so a
resume with different settings is **refused rather than mixed**.

## Memory and Performance Constraints

* One executor. Additional job requests are **queued, not spawned** — spawning
  would multiply every memory budget by the queue depth.
* Checkpoint write per block must be < 1 ms amortized, or it dominates.
* Progress events throttled to ≤ 10/s.

## Implementation Plan

1. `JobExecutor` owning a `tokio` task and a cancellation token.
2. Resolve scope → tensor list → block list; persist `blocks_total`.
3. Per block: cache lookup → compute → persist → checkpoint → emit progress →
   check cancellation.
4. Routes, including SSE.
5. Resume: reload the manifest, verify `content_hash`, skip verified blocks.
6. Graceful shutdown on SIGINT → `Paused`.

## Error Handling

* A block fails → recorded in `failed_blocks`; the job continues; the job ends
  `Failed` if any block failed, with the list.
* Cancellation → stop at a block boundary, `Cancelled`, manifest intact.
* Crash → on restart the job is `Converting` with a stale `updated_at`; it is
  marked resumable, never silently restarted.
* Resume with a different `configuration_hash` → **refused**, naming both.
* SSE client disconnect → the job continues; only the stream ends.
* Disk full → `Failed`; **nothing published under a final name**.

## Acceptance Criteria

1. `POST /v1/conversions` returns 202 with a job ID and an events URI.
2. The job runs a statistics pass over the scoped tensors and completes.
3. SSE emits progress at ≤ 10/s with all documented fields.
4. Cancel stops within one block; state `Cancelled`; manifest intact.
5. Resume skips verified blocks — asserted by a compute counter.
6. Resume with a changed configuration is refused naming both hashes.
7. Kill -9 mid-job, restart: the job is resumable and resumes correctly.
8. A second job request while one runs is **queued**, not run concurrently.
9. Illegal state transitions still rejected.

## Verification Plan

**Automated** — daemon tests for each route, a cancel test, a resume test with a
compute counter, and a configuration-mismatch test.
**Manual** — run a conversion over the large fixture; watch SSE; kill and resume.

## Suggested Commands

```bash
cargo test -p q-daemon -p q-catalog                              # verified today
curl -X POST localhost:PORT/v1/conversions -d '{…}'              # introduced here
curl -N localhost:PORT/v1/jobs/JOB/events
curl -X POST localhost:PORT/v1/jobs/JOB/cancel
```

## Test Cases

| Input | Expected |
| --- | --- |
| `POST /v1/conversions` scope=tensor | 202, job runs, completes |
| SSE stream | Monotone `blocks_done`; ≤ 10 events/s |
| Cancel at block 50 | `Cancelled`; 50 blocks recorded |
| Resume | Skips 50; compute counter shows only the remainder |
| Resume with a different block size | Refused, both hashes named |
| Kill -9, restart | Resumable; resumes correctly |
| Second job while one runs | Queued |
| Block fails mid-job | Recorded; job continues; ends `Failed` with the list |
| SIGINT | `Paused`, resumable |

## Risks

| Risk | Mitigation |
| --- | --- |
| Checkpoint cost dominates | Measured; batched within a bounded window while keeping one-block granularity |
| SSE holds a connection through a long job | Documented; polling fallback exists |
| A crashed job silently restarts and duplicates work | Stale `updated_at` marks it resumable; resume verifies hashes |

## Completion Evidence

* Route outputs for create, status, cancel, resume.
* An SSE transcript excerpt.
* The resume test's compute-counter comparison.
* Kill -9 and restart transcript.
