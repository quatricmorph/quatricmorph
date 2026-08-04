# REPORT_ARCHITECTURE — the artifact, the manifest, and the machine interface

The strategy is specific about this and gives a reason worth repeating:

> Ship a Markdown-native, Git-diffable report artifact from day one — it is cheap
> to build, reusable across all three diagnostics, and doubles as your
> distribution mechanism when partners share it. *(§9)*

A design partner who cannot share a finding without opening the tool will not
share it. The report *is* the distribution.

---

## 1. Three surfaces, one source

```text
                 ┌──────────────────────┐
                 │  diagnostics run     │
                 │  (q-diagnostics)     │
                 └──────────┬───────────┘
                            │  one in-memory result tree
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
      manifest.json    report.md      HTTP / CLI
      (REP-001)        (REP-002)      (REP-004, API-012)
      machine-read     human-read     CI + agents
```

The manifest is the **only** serialization the other two derive from. The report
renders it; the daemon serves it; the heat-map surface reads it. A number that
appears in the report and not in the manifest is a bug — that rule is what keeps
the three from drifting.

---

## 2. The manifest

`schemas/diagnostics/manifest.v1.json`, alongside the four schemas already in
`schemas/`. Versioned from the first commit, refusing a future version the way
`CAT-002` already refuses a future catalog schema.

```jsonc
{
  "manifest_version": 1,
  "run": {
    "run_id": "…",                  // deterministic: hash of (model_hash, config, engine_version)
    "engine_version": "0.1.0",
    "backend": "cpu" | "metal",
    "started_at": "…",              // run metadata — excluded from the determinism check
    "elapsed_seconds": 0,
    "peak_resident_bytes": 0,
    "bytes_read": 0
  },
  "model": {
    "model_id": "…",
    "source_uri": "…",
    "revision_hash": "…",
    "checkpoint_bytes": 0,
    "parameter_count": 0,
    "architecture": "qwen" | "llama" | "generic",
    "resolver_confidence": "resolved" | "unknown"
  },
  "config": {
    "precision": "int4",
    "granularity": { "per_group": 128 },
    "zero_point": "asymmetric",
    "round": "nearest_even",
    "block_rows": 256, "block_columns": 256,
    "resident_ceiling_bytes": 2147483648
  },
  "totals":   { /* SumsOfSquares + derived metrics for the whole model */ },
  "layers":   [ /* per layer, in layer_index order */ ],
  "experts":  [ /* present only where the resolver found experts */ ],
  "tensors":  [ /* per tensor, in canonical-address order */ ],
  "ranking":  [ /* tensor addresses, most fragile first */ ],
  "frontier": [ { "keep_set": ["…"], "added_bytes": 0, "error_removed_fraction": 0.0 } ],
  "fidelity": "exact" | "sampled",
  "refusals": [ { "requirement_id": "EVAL-001", "what": "accuracy estimate", "why": "…" } ]
}
```

Three details that are not decoration:

* **`refusals` is a first-class array.** Every capability the run could not
  provide is enumerated with its requirement ID. A consumer can tell the
  difference between "zero" and "not computed", which is exactly the failure mode
  that destroys trust in a diagnostic tool.
* **`fidelity`** carries the same exactness vocabulary the data model already
  types end to end (`SRC-018`, `STAT-005`).
* **`resolver_confidence`** is surfaced because a `generic`-resolved model has
  weaker layer semantics, and the report must not present a guessed hierarchy as
  a known one (`NSIR-001`).

### 2.1 Ordering

Every array has a **total order fixed by content**, never by iteration order:
layers by index, tensors by canonical address, ranking by (relative error desc,
parameter count desc, canonical address asc). Floating-point values serialise with
a fixed representation. This is what makes `V1-18` achievable.

---

## 3. The Markdown report

### 3.1 Shape

```markdown
# Quantisation-error diagnosis — <model> @ <revision>

<one paragraph: config, checkpoint size, what was measured, what was not>

## Verdict
<3–6 lines. The fragile layers. The frontier recommendation. The caveat.>

## Fragile layers            <- ranked table
## Mixed-precision frontier  <- (bytes added, error removed) table
## Error by layer            <- compact table; the heat-map is the visual form
## Outlier attribution       <- top-p% share, per fragile layer
## What this does not tell you   <- the caveat section, never optional
## Run metadata              <- backend, peak RSS, elapsed, bytes read, versions
```

`## Verdict` comes second because the reader is an engineer deciding something,
not a reviewer reading a paper. `## What this does not tell you` is a required
section, and `QM-0090` audits it against
[`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §5.2.

### 3.2 Determinism

Same checkpoint + same config + same engine version → **byte-identical** report.

| Rule | Reason |
| --- | --- |
| Timestamps, wall clock, hostname, and peak RSS live **only** in `## Run metadata` | So a diff of two configs shows the numbers that changed, not the clock |
| The determinism test compares everything above `## Run metadata` | `V1-18`, via `cmp` |
| Fixed float formatting; fixed column widths; fixed row order | A reflowed table is an unreadable diff |
| No emoji, no colour codes, no box drawing | Diffs, terminals, and `git log -p` all stay readable |

`V1-19` is the criterion that matters in practice: changing int8 → int4 must
change *numbers*, not reflow the document. That is the difference between a report
a team can track in version control and a report they regenerate and eyeball.

### 3.3 Self-containment

The report is readable by someone who has never run the tool. It states the model,
the revision, the config, and the limits. It does not link to a local server, a
session, or a UI route — a partner will paste it into Slack or attach it to a PR.

The heat-map is a companion, not a dependency. `QM-0150` may embed a static SVG
when it costs nothing; the report must remain complete without it.

---

## 4. The machine interface

The strategy asks for CI and coding-agent access (§7.8). Both are thin adapters
over the manifest.

### 4.1 CLI

```bash
quatricmorph diagnose <model-path> \
    --precision int4 --granularity group:128 --asymmetric \
    --resident-ceiling 2GiB \
    --out runs/qwen-int4/          # manifest.json + report.md

quatricmorph diagnose … --fail-above 0.05     # exit 1 if any layer's relative error exceeds it
quatricmorph report runs/qwen-int4/ --diff runs/qwen-int8/   # config comparison
```

`--fail-above` is what makes this usable in CI: a quantisation config change that
regresses a layer beyond a threshold fails the build. Exit codes are documented
and stable — `0` clean, `1` threshold exceeded, `2` refused (bad config, unknown
dtype, non-finite weights), `3` cancelled.

### 4.2 Daemon

```http
POST /v1/diagnostics                 → { runId }        (job; progress over SSE)
GET  /v1/diagnostics/{runId}         → manifest.json    (byte-identical to the CLI's)
GET  /v1/diagnostics/{runId}/report  → report.md
GET  /v1/jobs/{jobId}                → existing job route, unchanged
POST /v1/jobs/{jobId}/cancel         → existing, unchanged
```

Routes follow the conventions already in `API_CONTRACTS.md`: 501 with a
requirement ID for anything unbuilt, 400 before any read for a bad config, model-
root boundary enforced (`SEC-001`).

### 4.3 MCP — a seam, not a v1 feature

An MCP server exposing "diagnose this checkpoint" and "fetch this run" is a thin
wrapper over §4.2, and it is `API-013`: designed for, not built. v1's agent story
is the CLI's exit codes and the manifest's stability, which is enough for a CI
pipeline and for a coding agent that can run a command.

---

## 5. Crate

```text
crates/q-report/    Manifest serialization (serde + the JSON schema) and
                    Markdown rendering. No computation, no I/O policy — it is
                    handed a result tree and produces bytes.
```

Keeping rendering free of computation is what lets the golden-report test
(`QM-0142`) be fast and hermetic: it feeds a fixed result tree and compares bytes,
with no checkpoint involved.

---

## 6. Requirement IDs introduced

| ID | Capability |
| --- | --- |
| `REP-001` | Versioned JSON manifest, schema-validated |
| `REP-002` | Deterministic, Git-diffable Markdown report |
| `REP-003` | Golden report and determinism test |
| `REP-004` | CLI exit codes for CI |
| `API-012` | Daemon diagnostics routes |
| `API-013` | *(seam)* MCP-style agent interface |
