# Phase 12 — Report, manifest, and the machine interface

## Goal

```text
one diagnostic result tree
→ manifest.json   (versioned, schema-validated, the only serialization)
→ report.md       (deterministic, Git-diffable, readable without the tool)
→ CLI exit codes + daemon routes  (CI and coding agents)
```

## Why this phase is not cosmetic

The strategy is specific:

> Ship a Markdown-native, Git-diffable report artifact from day one — it is cheap
> to build, reusable across all three diagnostics, and doubles as your
> distribution mechanism when partners share it. *(§9)*

A design partner who cannot share a finding without opening the tool will not
share it. The report is the distribution channel, the CI integration, and the
evidence for `V1-30` — the decision-change case that gates the release.

## Design

[`../../REPORT_ARCHITECTURE.md`](../../REPORT_ARCHITECTURE.md). Three rules carry
the phase:

1. **The manifest is the only serialization.** The report renders it, the daemon
   serves it, the surface reads it. A number in the report that is not in the
   manifest is a bug.
2. **Determinism is a test, not an intention.** Same checkpoint + same config →
   byte-identical output. Timestamps and measured wall clock live only in
   `## Run metadata`, excluded from the comparison.
3. **`refusals` is a first-class array.** Every capability the run could not
   provide is enumerated with its requirement ID, so a consumer can distinguish
   "zero" from "not computed". That distinction is what keeps a diagnostic tool
   trustworthy.

## Entry conditions

* `QM-0140` (the schema) may start immediately — it depends on the *shape* of the
  engine's output, not on its data, and it is scheduled in Wave 1 for that reason.
* `QM-0141` onward need `QM-0123` complete so there is a real result tree.

## Tasks

| ID | Title | Kind | Lane | Requirements |
| --- | --- | --- | --- | --- |
| `QM-0140` | Manifest schema v1 and serialization | Implementation | R | `REP-001`, `V1-16` |
| `QM-0141` | Deterministic Markdown report | Implementation | R | `REP-002`, `V1-17`, `V1-18`, `V1-21`, `V1-22` |
| `QM-0142` | Golden report and config-diff test | Verification | R | `REP-003`, `V1-19` |
| `QM-0143` | CLI exit codes and daemon diagnostics routes | Implementation | R | `REP-004`, `API-012`, `V1-23` |

## Exit conditions — Gate G3

1. Two runs of the same checkpoint and config produce byte-identical reports —
   `cmp` returns 0.
2. Changing int8 → int4 produces a `git diff` where numbers change and the
   document does not reflow.
3. The manifest validates against `schemas/diagnostics/manifest.v1.json`.
4. `quatricmorph diagnose --fail-above 0.05` returns a documented non-zero exit
   code.
5. `GET /v1/diagnostics/{runId}` returns bytes identical to the CLI's manifest.
6. The report contains the required caveat section and no accuracy prediction.

Failure at (1) is almost always floating-point reduction order or a timestamp
that escaped the metadata block. Fix the ordering rather than loosening the test.
