# QM-0091 — Regenerate `STATUS.md` from a real run

## Status

Blocked

Unblocks when `QM-0090` reaches `Complete`.

## Phase

Phase 09 — Documentation and release

## Objective

Rebuild `STATUS.md` from actual test output, with **no row more favourable than
its evidence**.

## Repository Evidence

* `STATUS.md` — 129 rows, generated from a `2026-08-03` run: 102 Verified,
  10 Stub, 9 Not Started, 5 Hardware-Unverified, 1 Implemented, 2 Partial.
* Its own opening line: *"Generated from a real test run, not from intent. Every
  row marked `Verified` cites a test file that exists and passed."*
* Its "What a reader should not be surprised by" section lists five honest gaps.
* `QM-0001`'s baseline script provides the counts.

## Requirements Covered

`DOC-002`, `MVP-45`.

## Dependencies

`QM-0090`, `QM-0080`, `QM-0084`.

## Blocks

`QM-0094`.

## Parallelization

Runs after `QM-0090`, `QM-0092`, `QM-0093` — it records the final state.

## Program Boundary

`STATUS.md`.

## Scope

* Run every suite at the release commit; capture output.
* Update every row's status, files, and test citations.
* Add rows for every requirement this plan introduced.
* Rewrite the summary table and the "should not be surprised by" section.
* Keep `Hardware-Unverified` for anything no RTX 3090 ran.

## Out of Scope

Changing code to improve a status · adding requirements not implemented ·
`ARCHITECTURE.md` (`QM-0090`) · limitations (`QM-0092`).

## Files Expected to Change

* `STATUS.md`

## Files Expected to Add

* `scripts/status-check.sh` — asserts every cited test exists and passed

## Files Expected to Remove or Deprecate

None.

## Data Contracts

The existing row format is kept exactly:
`ID | Description | Status | Maps to | Files | Test(s)`.

**Status rules, unchanged:**

| Status | Requires |
| --- | --- |
| `Verified` | A named test that **exists and passed in the recorded run** |
| `Implemented` | Code exists, works, no dedicated test |
| `Partial` | A working vertical slice, coverage deliberately incomplete |
| `Stub` | Real types exist; every operation returns `NotImplemented` with its ID |
| `Hardware-Unverified` | Code exists, **never executed on its target hardware** |
| `Not Started` | — |

## Memory and Performance Constraints

`status-check.sh` must run in under 2 minutes so it can gate the release.

## Implementation Plan

1. Run `cargo test --workspace`, `npx vitest run`, the Playwright suites, and
   `scripts/verify-baseline.sh`; capture all output.
2. For each existing row, verify its cited test still exists and passed; update
   its status.
3. Add rows for the ~45 new requirement IDs from
   [`REQUIREMENT_TRACEABILITY.md`](../../REQUIREMENT_TRACEABILITY.md) §3.
4. Recompute the summary table.
5. Rewrite "What a reader should not be surprised by" from what is then true.
6. Write `status-check.sh` asserting every `Verified` row's test exists and
   passed.

## Error Handling

* A cited test that no longer exists → the row **cannot be `Verified`**; downgrade
  and note it.
* A test that exists but did not run → same. A skipped test is not a passing one.
* A requirement with no test → `Implemented` at best, never `Verified`.
* A CUDA requirement with no device run → **`Hardware-Unverified`**, regardless of
  how complete the code is.

## Acceptance Criteria

1. Every row's status is justified by the recorded run.
2. Every `Verified` row cites a test that exists and passed — asserted by
   `status-check.sh`.
3. Every new requirement has a row.
4. The summary table matches the row counts.
5. `Hardware-Unverified` remains for every CUDA requirement no device ran.
6. The commands and counts at the top are from the release run.
7. "What a reader should not be surprised by" matches reality.
8. **No row is more favourable than its evidence** — reviewed row by row.

## Verification Plan

**Automated** — `status-check.sh` cross-checking every `Verified` row against the
test output.
**Manual** — a reviewer samples 20 rows and confirms each citation.

## Suggested Commands

```bash
cargo test --workspace 2>&1 | tee /tmp/rust.txt        # verified today
cd apps/web && npx vitest run 2>&1 | tee /tmp/web.txt
./scripts/status-check.sh /tmp/rust.txt /tmp/web.txt    # introduced here
```

## Test Cases

| Input | Expected |
| --- | --- |
| Every `Verified` row | Its test appears in the passing output |
| A row citing a deleted test | Flagged; status downgraded |
| A row citing a skipped test | Flagged; not `Verified` |
| A CUDA row with no device run | Stays `Hardware-Unverified` |
| Summary counts | Match the rows |
| Top-of-file counts | Match the recorded run |
| 20 sampled rows | Citations confirmed by hand |

## Risks

| Risk | Mitigation |
| --- | --- |
| Optimistic statuses at release | `status-check.sh` cross-checks mechanically |
| `Hardware-Unverified` quietly upgraded | Requires device output in the evidence; asserted |
| The document drifts again after release | The script gates every subsequent release |

## Completion Evidence

* Full output of every suite at the release commit.
* `status-check.sh` output, exit 0.
* The regenerated `STATUS.md`.
* The new summary table with counts.
* The 20-row manual sample.
