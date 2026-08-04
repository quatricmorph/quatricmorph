# QM-0102 — Scaling benchmarks

## Status

Blocked

Unblocks when `QM-0122` reaches `Complete`.

## Phase

Phase 10 — Out-of-core proof on a real checkpoint

## Objective

Record what a full diagnostic run actually costs — wall clock, throughput, bytes
read, cache effect — at three checkpoint sizes, so the product's performance
claims are quotable numbers rather than adjectives.

## Repository Evidence

* `QM-0101` — the residency measurement and its `ResidencyReport` contract.
* `crates/q-cache/src/lib.rs` — L1/L2 with hit counters (`CACHE-002`, `CACHE-003`).
* `.plan/PERFORMANCE_PLAN.md` — the existing convention: what is measured versus
  asserted.

## Requirements Covered

`PERF-003` (new). Supplies the numbers `V1-21` prints and `QM-0166` cites.

## Dependencies

`QM-0100`, `QM-0101`, `QM-0122`, `QM-0032`.

## Blocks

`QM-0165` (release audit cites these numbers).

## Parallelization

Lane P, Wave 5. Runs beside the validation tasks; touches only benchmark code.

## Program Boundary

`tests/benches/` or `crates/q-diagnostics/benches/`, `crates/q-cli`.

## Scope

* End-to-end diagnostic timing at three sizes, cold and warm cache.
* Throughput in MB/s, dominated by NVMe bandwidth if the design is right.
* Backend comparison: CPU vs. Metal, if `QM-0127` has landed.
* A committed results file and a short interpretation.

## Out of Scope

Optimisation. This task measures. A finding that something is slow becomes a new
task, not a change here.

## Files Expected to Change

* `crates/q-cli/src/main.rs` — emit timing in machine-readable form

## Files Expected to Add

* `fixtures/scaling-benchmarks.json`
* `.plan/` note appended to `PERFORMANCE_PLAN.md` with the interpretation

## Data Contracts

```jsonc
{
  "checkpoint": "…", "bytes": 0, "backend": "cpu",
  "cold": { "elapsed_s": 0, "throughput_mb_s": 0, "bytes_read": 0, "cache_hits": 0 },
  "warm": { "elapsed_s": 0, "throughput_mb_s": 0, "bytes_read": 0, "cache_hits": 0 },
  "peak_resident_bytes": 0
}
```

## Memory and Performance Constraints

Peak residency must remain within `QM-0101`'s band at every size — a benchmark
that quietly relaxes the ceiling to look fast invalidates G1.

Expected shape, stated as a prediction to be confirmed or refuted rather than a
target to hit: cold-run throughput within a small factor of sequential NVMe read
bandwidth; warm runs materially faster through the L2 cache; wall clock roughly
linear in bytes.

## Implementation Plan

1. Warm-up run, then three timed cold runs per size (drop caches between where
   the OS permits; otherwise record that the OS cache was warm and say so).
2. Three warm runs per size, reporting cache hit counts.
3. Repeat on the Metal backend if available; record the device name.
4. Write the results file and a one-paragraph interpretation.
5. If measured throughput is far below sequential read bandwidth, file a task —
   do not tune here.

## Error Handling

* A run that exceeds the residency band → the benchmark is invalid. Report it as
  a G1 regression, not as a slow result.
* High variance across repeats → report the spread; do not cherry-pick the best.

## Acceptance Criteria

1. Three sizes × (cold, warm) × available backends, all recorded.
2. Peak residency within `QM-0101`'s band at every size.
3. Warm runs show cache hits and are faster than cold.
4. Results committed with the exact commands and the machine's specification.
5. Variance reported, not hidden.

## Verification Plan

**Manual** — the runs. **Automated** — a smoke benchmark on fixtures in CI so a
catastrophic regression is caught without the large checkpoint.

## Suggested Commands

```bash
cargo build --release
/usr/bin/time -l ./target/release/q-cli diagnose models/<checkpoint> --precision int4 --out /tmp/run1
/usr/bin/time -l ./target/release/q-cli diagnose models/<checkpoint> --precision int4 --out /tmp/run2   # warm
```

## Test Cases

| Input | Expected |
| --- | --- |
| Tiny fixture, cold | Sub-second; dominated by process start |
| 339 MB, cold | Throughput within a small factor of NVMe sequential read |
| ≥ 24 GB, cold | Linear in bytes; residency flat |
| ≥ 24 GB, warm | Faster; cache hits > 0 |
| Metal vs. CPU | Both recorded; identical outputs (`V1-13`) |

## Risks

| Risk | Mitigation |
| --- | --- |
| OS page cache makes "cold" meaningless | Record whether the cache was dropped; state it plainly |
| A benchmark becomes a target and residency is relaxed to hit it | Residency is an acceptance criterion of this task, not just of `QM-0101` |
| Numbers get quoted without their machine | The machine specification is part of the results file |

## Completion Evidence

* The committed results file.
* Raw timing output for at least one run per configuration.
* Machine specification: chip, memory, disk type, free space.
* The interpretation paragraph, including anything that surprised.
