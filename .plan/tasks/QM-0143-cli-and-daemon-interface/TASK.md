# QM-0143 — CLI exit codes and daemon diagnostics routes

## Status

Blocked

Unblocks when `QM-0141` reaches `Complete`.

## Phase

Phase 12 — Report, manifest, and the machine interface

## Objective

Make the diagnostic usable without a human: a CLI whose exit codes gate a CI
pipeline, and daemon routes that return exactly what the CLI writes.

## Repository Evidence

* `crates/q-daemon/src/lib.rs` — 8 working routes; `unbuilt_routes_return_501_with_a_requirement_id`
  (`API-005`); `a_shape_mismatch_is_a_400_before_any_read` (`API-006`);
  `the_model_root_boundary_is_enforced` (`SEC-001`).
* `crates/q-cli/src/main.rs` — `inspect`, `value`, `query`, `stats`.
* `crates/q-catalog/src/job.rs` — the job state machine (`JOB-001`).
* `.plan/API_CONTRACTS.md` — route, payload, and status-code conventions.

## Requirements Covered

`REP-004`, `API-012`, `V1-23`.

## Dependencies

`QM-0141`, `QM-0033`.

## Blocks

`QM-0161` (a partner runs the CLI, not a library).

## Parallelization

Lane R. Touches `q-daemon` and `q-cli`, which no other v1 task edits
concurrently.

## Program Boundary

`crates/q-cli`, `crates/q-daemon`.

## Scope

* `quatricmorph diagnose` and `quatricmorph report` verbs.
* Documented, stable exit codes.
* `--fail-above` threshold gating for CI.
* Daemon routes returning the identical manifest bytes.
* Progress for long runs.

## Out of Scope

An MCP server (`API-013`, a seam) · authentication · remote checkpoints
(`SRC-008`) · a web UI (`QM-0150`).

## Files Expected to Change

* `crates/q-cli/src/main.rs`
* `crates/q-daemon/src/lib.rs`
* `.plan/API_CONTRACTS.md` — append the new routes

## Data Contracts

### CLI

```bash
quatricmorph diagnose <model-path> \
    --precision int4|int8 \
    --granularity tensor|channel|group:<n> \
    --symmetric|--asymmetric \
    --resident-ceiling <bytes> \
    --backend cpu|metal|auto \
    --out <dir>                        # manifest.json + report.md
    [--fail-above <relative-error>]
    [--summary]                        # summary projection only

quatricmorph report <run-dir> [--diff <other-run-dir>]
```

### Exit codes — stable, documented, part of the contract

| Code | Meaning |
| --- | --- |
| `0` | Completed; no threshold exceeded |
| `1` | Completed; `--fail-above` threshold exceeded by at least one layer |
| `2` | Refused — invalid config, unsupported dtype, misaligned group, non-finite weights |
| `3` | Cancelled |
| `4` | I/O or environment failure (missing shard, disk, permissions) |

A CI pipeline distinguishes "the model regressed" (1) from "the tool could not
run" (2, 4). Collapsing those is the difference between a useful gate and one
teams disable.

### Daemon

```http
POST /v1/diagnostics                  → 202 { "runId": "…", "jobId": "…" }
GET  /v1/diagnostics/{runId}          → 200 manifest.json  (identical bytes to the CLI's)
GET  /v1/diagnostics/{runId}/report   → 200 text/markdown
GET  /v1/diagnostics/{runId}/summary  → 200 summary projection
GET  /v1/jobs/{jobId}                 → existing route, unchanged
POST /v1/jobs/{jobId}/cancel          → existing route, unchanged
```

Progress over the existing job route and its SSE transport — this task adds no
new transport.

## Memory and Performance Constraints

The daemon must not materialise a full per-tensor manifest in memory for a large
run; stream it from disk. `/summary` exists so that a browser never needs the
full document, mirroring the `assertBlockIsBounded` discipline.

## Implementation Plan

1. Add the CLI verbs and argument parsing; validate the config **before** opening
   the checkpoint.
2. Wire the diagnostic run through the job runner (`QM-0033`) so cancellation and
   resume work identically from CLI and API.
3. Write `manifest.json` and `report.md` atomically — temp file plus rename, the
   convention already required for artifacts.
4. Implement `--fail-above` against the maximum layer relative error; print which
   layer tripped it.
5. Add the daemon routes, serving the same files, enforcing the model-root
   boundary.
6. Document exit codes in `--help` and in `docs/`.

## Error Handling

| Case | Behaviour |
| --- | --- |
| Invalid config | Exit 2 / HTTP 400, before any read — the `API-006` pattern |
| Path outside the model root | Refused (`SEC-001`); HTTP 403 |
| Unknown `runId` | HTTP 404 |
| Run still in progress | HTTP 409 naming the job, or 202 with job status; never a partial manifest |
| Cancellation | Exit 3; partial results retained and marked cancelled, never presented as complete |
| Disk full while writing | Exit 4; the temp file is removed; no half-written manifest is published |

## Acceptance Criteria

1. `diagnose` produces `manifest.json` and `report.md` in `--out`.
2. Exit codes match the table, each covered by a test.
3. `--fail-above` returns 1 and names the offending layer; below the threshold
   returns 0.
4. `GET /v1/diagnostics/{runId}` returns **byte-identical** content to the CLI's
   manifest.
5. A run in progress never returns a partial manifest.
6. Cancellation from the API and from the CLI behave identically.
7. Output is written atomically; a killed process leaves no partial artifact.
8. `--help` documents every exit code.
9. Path traversal is refused (`SEC-001` regression check).

## Verification Plan

**Automated** — exit-code tests; a byte-comparison between CLI output and API
response; an atomicity test using a kill signal.
**Manual** — a CI-shaped invocation in a shell script, exercised end to end.

## Suggested Commands

```bash
q-cli diagnose models/<ckpt> --precision int4 --granularity group:128 --asymmetric --out runs/a
echo $?
q-cli diagnose models/<ckpt> --precision int4 --out runs/b --fail-above 0.05 ; echo $?
cargo run -p q-daemon -- --model-root models/ &
curl -s localhost:PORT/v1/diagnostics/<runId> | cmp - runs/a/manifest.json
```

## Test Cases

| Input | Expected |
| --- | --- |
| Valid run | Exit 0; both files present |
| `--fail-above` exceeded | Exit 1; the layer named |
| `--granularity group:100` with 256-column blocks | Exit 2, misalignment named, before any read |
| Cancel mid-run | Exit 3; partial marked cancelled |
| Unreadable shard | Exit 4 |
| API manifest vs. CLI manifest | Byte-identical |
| Kill during write | No partial `manifest.json` |
| `../` in a path | Refused |
| Run in progress | No partial manifest served |

## Risks

| Risk | Mitigation |
| --- | --- |
| Exit codes drift and break someone's CI | Documented in `--help`, tested individually, and treated as a public contract |
| A partial manifest is served or published | Atomic write plus the in-progress test |
| CLI and API diverge | The byte-comparison test is an acceptance criterion |
| `--fail-above` compares the wrong statistic | It compares maximum **layer** relative error; the semantics are stated in `--help` and tested |

## Completion Evidence

* Exit-code test output, one case per code.
* The CLI-vs-API byte comparison.
* The atomicity test.
* A CI-shaped shell script and its output on both a passing and a failing config.
