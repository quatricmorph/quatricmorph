# QM-0020 — Persist and serve tensor statistics

## Status

Complete

`QM-0012` reached `Complete` and merged at `4e0e85c`, so this is unblocked.

## Phase

Phase 02 — Catalog and NSIR completion

## Objective

Write computed statistics into `tensor_statistics` and serve them, replacing the
501 on `GET /v1/tensors/{id}/statistics`.

## Repository Evidence

* `STATUS.md`: *"`q-statistics` and `q_gpu::CpuBackend` work; nothing has run a
  statistics pass, so `tensor_statistics` is empty and the API returns 501."*
* `crates/q-statistics/src/lib.rs` (481) — `StatisticsAccumulator`, Welford
  variance, streaming-equals-batch, `mark_approximate`, `ALGORITHM_VERSION = 1`,
  `DEFAULT_HISTOGRAM_BINS = 64`. Six requirements `Verified`, all with
  hand-computed expectations.
* `crates/q-catalog/src/schema.rs:90` — the table exists, with an `approximate`
  column and `algorithm_version`.
* `crates/q-daemon/src/lib.rs:616` — `tensor_statistics_501`, requirement
  `STAT-002`.
* `crates/q-gpu/src/lib.rs:231` — `block_statistics_default`.

## Requirements Covered

`STAT-002`, `API-005`; enables `MVP-24`, `AC-010`.

## Dependencies

`QM-0012`.

## Blocks

`QM-0021`, `QM-0031`, `QM-0054`, `QM-0072`.

## Parallelization

**Sequential with `QM-0021` and `QM-0022`** — all three edit
`crates/q-catalog/src/lib.rs` (987 lines). This one runs first.

## Program Boundary

`crates/q-catalog`, `crates/q-daemon`, `crates/q-statistics`.

## Scope

* Catalog write and read for `tensor_statistics`, keyed by `StatisticsId`.
* `StatisticsId = blake3(len‖subject_id ‖ len‖algorithm_version)`
  (`ADR-CANDIDATE-018`) — so a new algorithm **mints new rows rather than
  overwriting history**.
* Serialize the 64-bin histogram.
* Replace the 501 with a real handler carrying a fidelity label.
* `q-cli stats --persist`.

## Out of Scope

Running a full-tensor pass (`QM-0031`) · GPU statistics · `GROUP BY` queries
(`QM-0072`) · UI rendering (`QM-0054`).

## Files Expected to Change

* `crates/q-catalog/src/lib.rs`
* `crates/q-daemon/src/lib.rs`
* `crates/q-cli/src/main.rs`

## Files Expected to Add

None.

## Files Expected to Remove or Deprecate

* `q_daemon::tensor_statistics_501` — replaced. The test
  `unbuilt_routes_return_501_with_a_requirement_id` is **narrowed**, not deleted:
  the other four 501s still need it.

## Data Contracts

```jsonc
// GET /v1/tensors/{id}/statistics
{ "statistics_id": "…", "subject_id": "…", "subject_kind": "tensor" | "block",
  "count": 6144, "min_value": -0.31, "max_value": 0.29,
  "mean": 0.0001, "variance": 0.0102,
  "l1_norm": 512.3, "l2_norm": 7.9,
  "zero_ratio": 0.0, "positive_ratio": 0.501, "negative_ratio": 0.499,
  "histogram": { "bins": 64, "min": -0.31, "max": 0.29, "counts": [ … ] },
  "approximate": false, "algorithm_version": 1,
  "fidelity": "aggregate", "backend": "cpu-reference" }
```

`approximate: true` ⇒ `fidelity: "sampled"`. The mapping is enforced in one place,
so the two can never disagree.

## Memory and Performance Constraints

* Histogram: 64 × `u64` = 512 bytes per row. Stored as a blob, not 64 columns.
* Writes batched in a transaction — row-per-block inserts are a measured risk
  ([`PERFORMANCE_PLAN.md`](../../PERFORMANCE_PLAN.md) §5).
* Reads are indexed by `subject_id`.

## Implementation Plan

1. Add `TensorStatistics` ↔ row conversion in `q-catalog`, with the histogram as
   a length-prefixed blob.
2. Implement `StatisticsId` per `ADR-CANDIDATE-018`.
3. `put_statistics`, `get_statistics(subject_id, algorithm_version)`, and
   `list_statistics(subject_id)`.
4. Replace the daemon 501; add the fidelity mapping.
5. Add `--persist` to `q-cli stats`.
6. Tests: round trip, reopen survival, approximate labelling, version
   coexistence.

## Error Handling

* No statistics for a subject → **404**, not an empty row. An empty row would
  read as "all zeros".
* A histogram blob of unexpected length → refused as malformed.
* Two rows with the same `algorithm_version` for one subject → the later write
  replaces it; different versions **coexist**.
* Fidelity mapping is not optional: a row without `approximate` cannot be read.

## Acceptance Criteria

1. `q-cli stats --persist` writes a row readable after reopen.
2. `GET /v1/tensors/{id}/statistics` returns data with a fidelity label.
3. `approximate: true` surfaces as `fidelity: "sampled"`.
4. Two algorithm versions coexist for one subject.
5. No statistics → 404 with an explanation, never zeros.
6. Histogram round-trips exactly.
7. The remaining four 501 routes still carry their requirement IDs.
8. Hand-computed values from `STAT-001` still match after a round trip.

## Verification Plan

**Automated** — catalog round-trip tests, daemon route tests, a reopen test.
**Manual** — `q-cli stats --persist` then `curl` the route.

## Suggested Commands

```bash
cargo test -p q-catalog -p q-daemon -p q-statistics    # verified today
cargo run -p q-cli -- stats fixtures/tiny-llama-2shard \
  'model.layers[10].self_attention.query_projection.weight' \
  --rows 100:104 --columns 40:44 --persist               # --persist is new
```

## Test Cases

| Input | Expected |
| --- | --- |
| Persist then read | Byte-identical values |
| Persist, close, reopen, read | Same values |
| `approximate: true` | `fidelity: "sampled"` |
| Statistics for an unknown subject | 404 with an explanation |
| Two `algorithm_version`s | Both retrievable |
| 64-bin histogram | Round-trips exactly |
| The other four 501 routes | Still 501 with requirement IDs |

## Risks

| Risk | Mitigation |
| --- | --- |
| Three tasks editing `q-catalog/src/lib.rs` | Strict sequence: `QM-0020` → `QM-0021` → `QM-0022` |
| An empty row read as real zeros | 404 instead; asserted |
| Histogram blob endianness | Little-endian, matching `.qtile`; asserted |

## Completion Evidence

* Round-trip and reopen test output.
* `curl` output of the statistics route with its fidelity label.
* Confirmation that the four remaining 501s still carry their IDs.

## Orchestration

**Awaiting Independent Review**

| Field | Value |
| --- | --- |
| Lane | T |
| Branch | `task/qm-0020-persist-statistics` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0020` |
| Base | `6fb593a` |
| Implementation commits | `fb042f1` — the implementation, its 42 tests, the evidence record and the raised floor · `f42e4bf` — pre-review hardening (`put_statistics_batch` refuses a row whose `statistics_id` is not derived from its subject and version, +1 test) and two evidence corrections |
| Head commit | the branch tip, which is the one metadata-only commit following `f42e4bf`: it exists only to name those two SHAs here, because a commit cannot record its own hash. `git log --oneline -3` shows all three |
| Agent | `impl-agent-10` |
| Evidence | `.plan/evidence/QM-0020.md` |
| Merge path | L |
| Tests added | **43** (q-statistics 3, q-catalog 24, q-daemon 7, q-cli 9) plus one existing 501 test narrowed and strengthened |
| Floor before | rust **434** tests over **43** binaries (measured on this branch pre-change; matches `scripts/baseline.json` at `91abade`); web 115 over 13 |
| Floor after | rust **477** tests over **43** binaries; web 115 over 13 (unchanged — no web file touched) |

`scripts/baseline.json` raised `rust_tests` 434 → 477 only. Two other branches
are raising the same floor concurrently; **the controller reconciles the final
value at merge**.

Schema note for the reviewer: `CURRENT_SCHEMA_VERSION` 1 → 2, migration 2
`statistics_subject_kind`, purely additive (`ALTER TABLE ADD COLUMN` plus a
unique index). Reasoning and the round-trip test are in
`.plan/evidence/QM-0020.md` §Summary and §Acceptance criteria.

`STATUS.md` is out of bounds for this task and is now stale in two places the
controller must update: line 114 (`STAT-002` — **Stub**) and lines 272-274
("nothing has run a statistics pass, so `tensor_statistics` is empty and the API
returns 501").
