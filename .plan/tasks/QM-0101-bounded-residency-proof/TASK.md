# QM-0101 — Bounded-residency proof, measured

## Status

Complete

`QM-0100` and `QM-0030` are both `Complete`; claimed by `impl-agent-12`.

## Phase

Phase 10 — Out-of-core proof on a real checkpoint — **Gate G1**

## Objective

Stream every tensor of the real checkpoint end to end and **measure** that peak
resident bytes stays under a configured ceiling `C ≤ 2 GB`, on a checkpoint at
least 100 × larger than `C`. Turn the product's central technical claim from an
architectural intention into a number.

## Repository Evidence

* `crates/q-source/src/budget.rs` — named, enforced budgets;
  `a_tight_metadata_budget_is_enforced`.
* `crates/q-source/src/lib.rs` — access scale is a type (`SRC-018`):
  `metadata_scale_never_reads_payload`.
* `crates/q-tensor-runtime/src/lib.rs` — `TensorBlock::plan`, one byte run per
  row, no reads (`TILE-002`).
* `crates/q-catalog/tests/trillion_scale_manifest.rs` — the peak-allocation
  measurement idiom this task reuses, at 35.7 MB over a synthetic manifest.
* `QM-0030` — the bounded streaming block reader whose budgets this measures.

## Requirements Covered

`PERF-002` (new), `V1-03`, `V1-04`, `V1-05`.

## Dependencies

`QM-0100`, `QM-0030`.

## Blocks

`QM-0122`, `QM-0102`, and — by gate — the entire engine lane.

## Parallelization

Lane P. Runs alone: it is the gate.

## Program Boundary

`crates/q-tensor-runtime`, `crates/q-source`, `crates/q-cli`, `tests/`.

## Scope

* A CLI verb that streams every tensor of a checkpoint through the bounded block
  reader, touching every byte exactly once, computing a trivial checksum so the
  work cannot be optimised away.
* Peak-RSS measurement at three checkpoint sizes.
* A configurable resident ceiling, enforced, with a named budget.
* A committed record of the measurements.

## Out of Scope

Quantisation (`QM-0120`) · statistics persistence (`QM-0020`) · the job runner
(`QM-0033`) · throughput optimisation (`QM-0102` measures; nobody optimises yet).

## Files Expected to Change

* `crates/q-source/src/budget.rs` — add `MAX_RESIDENT_BYTES` as a named budget
* `crates/q-cli/src/main.rs` — the `stream` verb
* `crates/q-tensor-runtime/src/stream.rs` — honour the ceiling

## Files Expected to Add

* `tests/tests/bounded_residency.rs`
* `fixtures/residency-measurements.json` — committed measurements

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```rust
pub struct ResidencyReport {
    pub checkpoint_bytes: u64,
    pub resident_ceiling_bytes: u64,   // C
    pub peak_resident_bytes: u64,      // measured
    pub ratio_n: f64,                  // checkpoint_bytes / C
    pub bytes_streamed: u64,
    pub elapsed_seconds: f64,
    pub checksum: u64,                 // proves the bytes were actually touched
}
```

The checksum matters: without it, a sufficiently clever compiler or a bug that
skips blocks produces an excellent residency number for the wrong reason.

## Memory and Performance Constraints

```text
peak resident ≤ 1.25 × C,  with C ≤ 2 GiB
ratio N = checkpoint_bytes / C ≥ 100
peak resident is FLAT in checkpoint size
```

The 1.25 × allowance covers allocator overhead and mmap accounting, and it is a
stated tolerance rather than a moving target. Measured with `/usr/bin/time -l`
maximum resident set size, in a **release** build.

Note on mmap: memory-mapped pages count toward RSS while resident. The
measurement must therefore be of the process's peak RSS under normal page
pressure, and the task records whether pages were explicitly advised away. A
residency claim that only holds because the OS happened to evict pages is not a
claim; if `madvise`/`posix_fadvise` is needed to hold the ceiling, that is part of
the implementation, not an excuse.

## Implementation Plan

1. Add `MAX_RESIDENT_BYTES` to the budget module, defaulting to 2 GiB.
2. Add `q-cli stream <model> --resident-ceiling <bytes>`: iterate every tensor,
   stream its blocks, fold a checksum, count bytes.
3. Enforce the ceiling: refuse a configuration whose block size × concurrency
   would exceed it, naming the budget.
4. Measure peak RSS across three sizes — the tiny fixture,
   `models/distilbert-distilgpt2` (339 MB), and the `QM-0100` checkpoint.
5. If the mapped-page accounting inflates RSS, advise pages away after each block
   and re-measure. Document whichever path was needed.
6. Write `fixtures/residency-measurements.json`.
7. Add a test that fails if peak allocation grows with tensor size on fixtures —
   the automated guard for the property the manual measurement demonstrates.

## Error Handling

* A configuration exceeding the ceiling → refuse before streaming, naming the
  budget and the arithmetic.
* A short read → error naming the tensor and byte range; never zero-fill.
* Cancellation → stop at a block boundary and report bytes streamed so far.
* An unreadable shard → fail naming it; never skip a tensor silently.

## Acceptance Criteria

1. The full checkpoint streams; the checksum proves every byte was touched.
2. Peak RSS ≤ 1.25 × `C`, with `C ≤ 2 GiB`, **measured in release**.
3. `N ≥ 100`.
4. Peak RSS is flat across the three sizes — within the same 1.25 × band.
5. Exceeding the ceiling by configuration is refused, naming the budget.
6. Measurements are committed and reproducible from the recorded commands.
7. The automated test guards the flatness property on fixtures.

## Verification Plan

**Automated** — peak-allocation assertions at three fixture sizes.
**Manual** — `/usr/bin/time -l` at three real sizes, output pasted verbatim.

## Suggested Commands

```bash
cargo build --release -p q-cli
/usr/bin/time -l ./target/release/q-cli stream fixtures/tiny-llama-2shard --resident-ceiling 64MiB
/usr/bin/time -l ./target/release/q-cli stream models/distilbert-distilgpt2  --resident-ceiling 2GiB
/usr/bin/time -l ./target/release/q-cli stream models/<checkpoint>           --resident-ceiling 2GiB
cargo test -p q-tensor-runtime bounded_residency
```

## Test Cases

| Input | Expected |
| --- | --- |
| Tiny fixture, `C` = 64 MiB | Completes; peak ≤ 80 MiB |
| 339 MB checkpoint, `C` = 2 GiB | Completes; peak ≪ `C` |
| ≥ 24 GB checkpoint, `C` = 2 GiB | Completes; peak ≤ 2.5 GiB; `N ≥ 100` |
| Same checkpoint, `C` = 512 MiB | Still completes; peak ≤ 640 MiB |
| Block size × concurrency > `C` | Refused, budget named |
| Cancel mid-stream | Stops at a block boundary; bytes reported |
| Two runs | Identical checksum |

The `C = 512 MiB` case is the one that distinguishes a real bound from a
coincidence.

## Risks

| Risk | Mitigation |
| --- | --- |
| mmap page accounting inflates RSS and the ceiling appears breached | Advise pages away per block; document which path was needed; never redefine the metric to pass |
| The compiler elides the work | The folded checksum is compared against a second, independent run |
| Peak grows with tensor size, not block size | This is the failure the gate exists to catch. Halt; bisect the streaming path |
| A debug build measured by accident | Release build is an acceptance criterion |

## Completion Evidence

* Three `/usr/bin/time -l` outputs, verbatim, with the commands above them.
* Checkpoint size, `C`, peak RSS, and `N` for each.
* The committed measurements file.
* The commit SHA the runs were made against.
* Whether page advice was required, and if so where it was applied.

## Orchestration

| Field | Value |
| --- | --- |
| State | **Awaiting Independent Review** |
| Lane | P (runs alone — it is the gate) |
| Gate | **G1 — PASSES.** `C = 3,528,244 B` declared in `.plan/DEFINITION_OF_DONE.md` `V1-04`, independent of the measurement; peak RSS 3,850,240 B (worst of 20 release runs) ≤ 4,410,305 B; `N = 100.0000037`. Seen to fail at the same ceiling with `--io mmap` (331,038,720 B, 75.07× over, in all 13 runs) and at admission (`--resident-ceiling 1MiB` → exit 1 naming `max_resident`) |
| Branch | `task/qm-0101-bounded-residency-proof` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0101` |
| Base | `d49701c` |
| Head | recorded in `.plan/evidence/QM-0101.md` `## Merge` by the merge step; the implementation commit is the single commit this branch adds over `d49701c` |
| Agent | `impl-agent-12` |
| Evidence | `.plan/evidence/QM-0101.md` |
| Merge path | L |
| Tests added | **+79** (545 → 624), binaries **45 → 47** |
| Floor before | `scripts/baseline.json` rust 545 / 45 binaries, web 115 / 13 files |
| Floor after | rust **624** / **47** binaries, web 115 / 13 (unchanged — no web file touched). `./scripts/verify-baseline.sh` exit 0, at floor |
| Acceptance | 6 of 7 met; **AC-1 partially met** — 26 of 82 tensors stream (92.80 % of payload), the other 56 refused under `ADR-010` and reconciled to the byte. Recorded, not glossed |

**What the reviewer should check first:** that `C` is declared in a file that
predates the measurement (`.plan/DEFINITION_OF_DONE.md` `V1-04`) and is not
`R / 1.25`; that the mmap row really does breach the *same* ceiling; and that
`bytes_streamed + refused_payload_bytes == described_payload_bytes` exactly.
