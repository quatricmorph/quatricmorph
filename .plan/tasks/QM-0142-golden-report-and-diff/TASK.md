# QM-0142 — Golden report and config-diff test

## Status

Blocked

Unblocks when `QM-0141` reaches `Complete`.

## Phase

Phase 12 — Report, manifest, and the machine interface

## Objective

Lock the report's determinism and diffability behind tests, so that "Git-diffable"
is a property the build enforces rather than an aspiration.

## Repository Evidence

* `QM-0141` — rendering is pure: manifest in, string out, no clock, no I/O.
* `crates/q-tiles/src/lib.rs` — `round_trip_preserves_header_and_payload_byte_for_byte`:
  the byte-exactness discipline this mirrors.
* `fixtures/` — the convention of checked-in goldens with a generator.

## Requirements Covered

`REP-003`, `V1-19`.

## Dependencies

`QM-0141`.

## Blocks

`QM-0165`.

## Parallelization

Lane R.

## Program Boundary

`crates/q-report`, `fixtures/reports/`.

## Scope

* A checked-in golden manifest and its golden report.
* A byte-comparison test.
* A **diff test**: two configs differing in one parameter, and an assertion about
  the shape of the resulting diff.
* A regeneration command, documented.

## Out of Scope

Rendering changes (`QM-0141`) · the CLI (`QM-0143`).

## Files Expected to Add

* `fixtures/reports/golden-manifest.json`
* `fixtures/reports/golden-report.md`
* `fixtures/reports/golden-report-int8.md`
* `crates/q-report/tests/golden.rs`

## The diff test

This is the part that is easy to skip and is the whole point of the format.

```text
render(manifest_int4) → report_int4.md
render(manifest_int8) → report_int8.md
diff report_int8.md report_int4.md
```

Assertions on the diff:

1. Every changed line is a **table row or a numeric value**, not a heading, a
   caveat, or a column header.
2. The number of changed lines is within a documented bound — a one-parameter
   change must not rewrite the document.
3. No line differs only in whitespace.
4. Section order is unchanged.

A diff that fails these is a formatting bug, and it is invisible without this
test until a design partner tries to track two configs in version control and
finds the comparison useless.

## Memory and Performance Constraints

Hermetic and fast — no checkpoint, no I/O beyond reading the fixtures. This test
runs in CI on every commit.

## Implementation Plan

1. Generate a golden manifest from a fixture-scale run; hand-check the values in
   it against `QM-0122`'s reference comparison.
2. Render and check in the golden report.
3. Byte-comparison test.
4. Produce the int8 variant; check it in.
5. Write the diff test with the four assertions above.
6. Document the regeneration command in `fixtures/reports/README.md`, including
   the rule that a golden change must be reviewed line by line, not accepted
   wholesale.

## Error Handling

* Golden mismatch → fail showing the first differing line and its context, not a
  byte offset. A test whose failure output is unreadable gets regenerated instead
  of fixed.
* Missing golden → fail naming the regeneration command.

## Acceptance Criteria

1. Rendering the golden manifest reproduces the golden report byte for byte.
2. The diff test's four assertions hold between int8 and int4.
3. Failure output names the first differing line with context.
4. Regeneration is one documented command.
5. The goldens are small enough to review by hand — a fixture-scale model, not the
   `QM-0100` checkpoint.
6. The test runs without any checkpoint present.

## Verification Plan

**Automated** — the golden and diff tests.
**Manual** — read the checked-in diff once; if it is not obviously readable, the
formatting is wrong regardless of what the assertions say.

## Suggested Commands

```bash
cargo test -p q-report golden
diff fixtures/reports/golden-report-int8.md fixtures/reports/golden-report.md
cargo run -p q-cli -- report --regenerate-goldens
```

## Test Cases

| Input | Expected |
| --- | --- |
| Golden manifest rendered | Byte-identical to the golden report |
| int8 vs. int4 diff | Only numeric/table-row lines change |
| Whitespace-only change | None |
| Section order | Unchanged |
| Missing golden file | Fails naming the regeneration command |
| No checkpoint present | Test still runs |

## Risks

| Risk | Mitigation |
| --- | --- |
| Goldens regenerated reflexively when they fail | `fixtures/reports/README.md` states the review rule; the failure output makes line-by-line review practical |
| The golden grows too large to review | Fixture-scale model; acceptance criterion 5 |
| The diff test passes while diffs are still unreadable | The manual read is part of the verification plan, not optional |

## Completion Evidence

* The checked-in goldens.
* Test output.
* The actual int8-vs-int4 diff, pasted, so a reader can judge readability
  themselves.
