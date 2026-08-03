# QM-0084 — Scaling benchmarks and the benchmark harness

## Status

Blocked

Unblocks when `QM-0080` reaches `Complete`.

## Phase

Phase 08 — Integration and performance

## Objective

Establish reproducible measurements, and prove nothing scales as O(model bytes)
in memory.

## Repository Evidence

* `CAT-006` Verified — the only measured number in the repository: 47 278
  tensors, 1.048×10¹² parameters, 2.10 TB described, **35.7 MB peak** (56 040:1).
* `gpu/cuda/README.md`: *"Treat every performance or numerical claim below as an
  intention, not a measurement."*
* `QM-0013` promotes the manifest generator to a parameterized tool.
* [`PERFORMANCE_PLAN.md`](../../PERFORMANCE_PLAN.md) §4 — the scaling invariants.

## Requirements Covered

`PERF-004`, `CAT-006`, `MVP-05`.

## Dependencies

`QM-0080`, `QM-0013`.

## Blocks

`QM-0092`, `QM-0094`.

## Parallelization

Parallel with `QM-0081`…`QM-0085`.

## Program Boundary

`benchmarks/`, `criterion` benches, CI.

## Scope

* `criterion` benches behind a `bench` feature, so the default suite stays fast.
* Scaling runs: import at 10³, 10⁴, 10⁵ tensors; conversion at 1024², 2048²,
  4096².
* Browser measurements via `performance.mark`/`measure` in Playwright.
* Reports in `benchmarks/` with commit SHA, hardware, and **every configuration
  variable**.

## Out of Scope

Optimization · CUDA benchmarks (`QM-0035`, `QM-0036`) · setting new budgets, as
opposed to measuring against existing ones.

## Files Expected to Change

* `.github/workflows/build.yaml`
* Various `Cargo.toml` — a `bench` feature

## Files Expected to Add

* `benchmarks/README.md`
* `crates/q-catalog/benches/import_scaling.rs`
* `crates/q-gpu/benches/conversion_scaling.rs`
* `apps/web/e2e/perf-marks.spec.ts`
* `scripts/bench-report.js`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
{ "commit": "…", "hardware": { "cpu": "…", "cores": 10, "ram_gb": 32, "os": "darwin 25.5.0" },
  "config": { "MAX_HOST_STAGING_BYTES": 536870912, "MAX_CONCURRENT_BLOCKS": 4, "…": "…" },
  "results": [ { "name": "import", "tensors": 100000,
                 "mean_ms": 4210, "median_ms": 4180, "p95_ms": 4400,
                 "peak_rss_mb": 71.2, "iterations": 5 } ] }
```

**A single mean is not a measurement.** Median, p95, iteration count, hardware,
and configuration are all required — a number without its budgets is not
reproducible.

## Memory and Performance Constraints

The benchmark suite must not run per-commit. Nightly, or on demand.

## Implementation Plan

1. Add the `bench` feature and `criterion` benches for import and conversion.
2. Parameterize import at 10³, 10⁴, 10⁵ using `QM-0013`'s generator.
3. Parameterize conversion at 1024², 2048², 4096².
4. Add browser `performance.mark`s around render, fetch, decode, and matmul.
5. Emit reports; check them into `benchmarks/` with full context.
6. Assert the scaling invariants: import memory linear in tensor count;
   **conversion peak RSS flat in tensor size**.

## Error Handling

* A budget missed → **report it and record the number**; the budget is corrected
  in the plan with the measurement attached, never quietly dropped.
* High variance (p95 > 2× median) → flag the run as noisy and re-run.
* Missing hardware info → the report is invalid; a report without context cannot
  be compared to anything.

## Acceptance Criteria

1. Import at 10³, 10⁴, 10⁵ shows time and memory **linear in tensor count**.
2. Conversion peak RSS is **flat** across 1024², 2048², 4096² — within 10 %.
3. Conversion time is linear in bytes converted.
4. `CAT-006`'s 35.7 MB result is reproduced within 20 %.
5. Browser marks cover render, fetch, decode, and matmul.
6. Every report carries commit, hardware, and configuration.
7. Median, p95, and iteration count are reported, not just a mean.
8. A missed budget is recorded with its number.
9. The suite does not run per-commit.

## Verification Plan

**Automated** — the nightly benchmark job with the scaling assertions.
**Manual** — review the curves for the expected shapes.

## Suggested Commands

```bash
cargo bench -p q-catalog --features bench                     # introduced here
cargo bench -p q-gpu --features bench
npx playwright test apps/web/e2e/perf-marks.spec.ts
node scripts/bench-report.js
```

## Test Cases

| Measurement | Assertion |
| --- | --- |
| Import 10³ → 10⁵ tensors | Time and memory linear in count |
| Conversion 1024² → 4096² | **Peak RSS flat within 10 %** |
| Conversion time | Linear in bytes |
| 47 278-tensor manifest | 35.7 MB ± 20 % |
| Viewer first render, 1 000 tiles | Recorded against the 2 s budget |
| Block fetch + decode 256×256 | Recorded against the 200 ms budget |
| Workspace matmul 256³ | Recorded against the 200 ms budget |
| Report completeness | Commit, hardware, config all present |

## Risks

| Risk | Mitigation |
| --- | --- |
| Benchmarks are noisy and ignored | p95 reported; noisy runs flagged and re-run |
| A missed budget is quietly dropped | Recorded with its number; the plan is corrected, not the expectation |
| Numbers are quoted without context | Hardware and configuration are required fields |

## Completion Evidence

* Benchmark reports for every parameterization.
* Scaling curves for import and conversion.
* The `CAT-006` reproduction.
* Browser performance marks.
* A list of any missed budgets with their measured values.
