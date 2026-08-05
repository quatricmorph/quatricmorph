# QM-0141 — Deterministic Markdown report

## Status

Blocked

Unblocks when `QM-0140` and `QM-0125` reach `Complete`.

## Phase

Phase 12 — Report, manifest, and the machine interface — **Gate G3**

## Objective

Render the manifest as a Markdown report an engineer can read, act on, and paste
into a pull request — byte-identical across runs, so that two configurations diff
cleanly.

## Repository Evidence

* `QM-0140` — the manifest; the only input.
* `QM-0125` — ranking and frontier, including the `method` caveat string.
* `QM-0124` — outlier attribution and its `resolution` string.
* `.plan/PRODUCT_SCOPE.md` §5.2 — the forbidden-claims table this renders against.
* `STATUS.md` — the house style for evidence-carrying documents: numbers with
  the command above them, no status more favourable than its evidence.

## Requirements Covered

`REP-002`, `V1-17`, `V1-18`, `V1-21`, `V1-22`.

## Dependencies

`QM-0140`, `QM-0125`, `QM-0124`.

## Blocks

`QM-0142`, `QM-0161`, `QM-0162`.

## Parallelization

Lane R, sequential after `QM-0140` (same crate).

## Program Boundary

`crates/q-report`. **No computation** — it is handed a manifest and produces
bytes.

## Scope

* The section structure in [`REPORT_ARCHITECTURE.md`](../../REPORT_ARCHITECTURE.md) §3.
* Deterministic rendering: fixed ordering, fixed float formatting, fixed column
  widths.
* Run metadata confined to one delimited block.
* The caveat section, non-optional.

## Out of Scope

The golden test (`QM-0142`) · the CLI (`QM-0143`) · HTML or PDF export · charts.

## Files Expected to Add

* `crates/q-report/src/markdown.rs`
* `crates/q-report/templates/report.md.template` — if templating is used

## The structure

```markdown
# Quantisation-error diagnosis — <model> @ <revision>

<one paragraph: config, checkpoint size, what was measured, what was not>

## Verdict
<3–6 lines: the fragile layers, the frontier recommendation, the caveat>

## Fragile layers
## Mixed-precision frontier
## Error by layer
## Outlier attribution
## What this does not tell you
## Run metadata
```

`## Verdict` is second because the reader is an engineer deciding something, not a
reviewer reading a paper. If they read nothing else, they read that.

`## What this does not tell you` is required and its content is fixed by
`PRODUCT_SCOPE.md` §5.2 — weight-space only, no accuracy measured, greedy not
optimal, attribution at bucket resolution, backend used, sampled vs. exact.

## Data Contracts

Input: a `Manifest`. Output: `String`. No I/O, no clock, no environment access —
everything that varies between runs arrives in the manifest, which is what makes
the golden test hermetic.

## Memory and Performance Constraints

`O(manifest)`. The report renders the **summary** projection plus the top-*N*
tensors (default 50), not every tensor — a 47 000-row table is not a document.
The count rendered is stated in the report, and the manifest carries the rest.

## Implementation Plan

1. Render the fixed section skeleton.
2. Fixed float formatting: a documented number of significant digits per column,
   the same every run.
3. Fixed column widths so a changed number does not reflow a table.
4. Confine timestamps, elapsed time, host, and peak RSS to `## Run metadata`,
   with an explicit delimiter comment the determinism test uses to split.
5. Render the caveat section from constants, not from prose written per run.
6. Render `frontier.method` and attribution `resolution` verbatim from the
   manifest.
7. State the top-*N* truncation wherever it applies.

## Error Handling

| Case | Behaviour |
| --- | --- |
| Manifest version unknown | Refuse to render, naming the version |
| Empty ranking (lossless config) | Render the report with an explicit statement, not an empty table |
| A `refusals[]` entry | Rendered in the caveat section with its requirement ID |
| Missing optional section data (e.g. no experts) | Section omitted entirely, never rendered empty |
| A value the manifest marks `sampled` | Labelled at the point of use, not only in the caveat |

## Acceptance Criteria

1. Two renders of the same manifest are byte-identical.
2. Everything above `## Run metadata` is byte-identical for two runs of the same
   checkpoint and config **on different days**.
3. The report states peak RSS, checkpoint size, backend, elapsed time, and bytes
   read.
4. The report contains no accuracy prediction — audited against `PRODUCT_SCOPE.md`
   §5.2 string by string.
5. `frontier.method` and attribution `resolution` appear verbatim.
6. A reader who has not used the tool can name the fragile layers and the
   recommended keep-set from the report alone.
7. Top-*N* truncation is stated wherever applied.
8. Rendering performs no I/O and reads no clock.

Criterion 8 is what makes `QM-0142` possible.

## Verification Plan

**Automated** — double-render byte comparison; a no-clock/no-I/O assertion by
construction (the function takes no such capability); a string audit test against
the forbidden list.
**Manual** — hand the report to someone unfamiliar and ask what they would do.

## Suggested Commands

```bash
cargo test -p q-report markdown
./target/release/q-cli report /tmp/manifest.json > /tmp/a.md
./target/release/q-cli report /tmp/manifest.json > /tmp/b.md
cmp /tmp/a.md /tmp/b.md
```

## Test Cases

| Input | Expected |
| --- | --- |
| A full manifest, rendered twice | Byte-identical |
| Same config, two different runs | Identical above `## Run metadata` |
| Lossless config (empty frontier) | Explicit statement, no empty table |
| Dense model (no experts) | Expert section omitted, not empty |
| Manifest with an `EVAL-001` refusal | Appears in the caveat with its ID |
| A sampled result | Labelled where used |
| 47 000 tensors | Top-50 rendered; truncation stated |
| Forbidden-string audit | No match for "accuracy", "will cost", "optimal" outside the negations |

## Risks

| Risk | Mitigation |
| --- | --- |
| A timestamp escapes the metadata block | The determinism test splits at the delimiter and compares the rest |
| Column reflow makes diffs unreadable | Fixed widths; `QM-0142` diffs two configs and reads the result |
| The caveat is softened over time to sound better | Rendered from constants; audited by `QM-0090` |
| The report becomes a data dump | Top-*N* default of 50, and `## Verdict` first |

## Completion Evidence

* Two renders and `cmp` output.
* A rendered report, checked in as the golden for `QM-0142`.
* The string-audit test output.
* An account of at least one unfamiliar reader's response to criterion 6.
