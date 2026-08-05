# `.plan/` — Quatricmorph MVP implementation plan

## Purpose

`.plan/` is the execution plan that takes Quatricmorph from **the repository as it
stands today** to **v1**: one Level-3 diagnostic — out-of-core quantization-error
forensics on a real open-weight checkpoint — shipped end to end with a shareable
report, and validated against real design partners.

**v1 is a wedge, not the platform.** The full tensor-visualization platform
(CesiumJS model viewer, matrix-multiplication workspace, chat) is still the
long-term architecture and is still designed for in this directory — but it is
**deferred to post-v1** and no longer sits on the critical path. The reason is
recorded in [`STRATEGY_ALIGNMENT.md`](STRATEGY_ALIGNMENT.md), and the boundary is
enforced by [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md).

It is a **delta plan**, not a greenfield plan. The repository already contains a
Rust workspace of `ls crates | wc -l` crates (**18** at the time of writing),
three web applications, `find schemas -name '*.json' | wc -l` JSON schemas
(**5**), checked-in SafeTensors fixtures, the accepted ADRs in
[`../docs/decisions/`](../docs/decisions/) (`ls docs/decisions/*.md | wc -l` —
**13**), and a requirement-traceability document built
from real test output. A plan that ignored that would be fiction. Every task in
`tasks/` therefore starts from a cited file, symbol, or test that exists now.

Each of those counts is given as **the command first and the number second**, on
purpose: every one of them has been stale in this file at least once. Crates went
17 → 18 when `QM-0140` added `crates/q-report`, schemas 4 → 5 when the same task
added `schemas/diagnostics/manifest.v1.json`, and the ADR count read `eight` and
then `ten` while thirteen were accepted. Run the command; do not cite the number.

**This directory contains no code and mandates no change outside `.plan/`.**
Where the plan concludes that a file outside `.plan/` is wrong — including
`ARCHITECTURE.md` and `STATUS.md` — the correction is written as a *task*, not
performed here.

## Authoritative documents

Precedence, highest first. Where two disagree, the higher one wins and the lower
one gets a task to fix it.

| Rank | Document | Authority |
| --- | --- | --- |
| 1 | [`../docs/decisions/ADR-0XX-*.md`](../docs/decisions/) marked **Accepted** with a `Departs from:` line | Overrides `ARCHITECTURE.md` **for exactly the section it names**, and nothing else. The code follows the ADR, not the superseded paragraph |
| 2 | [`../ARCHITECTURE.md`](../ARCHITECTURE.md) | **Implementation architecture — the single source of truth.** Authoritative on scope, sequencing, and how anything is built. §17.1 records the v1 scope decision and its source |
| 3 | [`../STATUS.md`](../STATUS.md) | What is actually built and tested. The factual baseline |
| 4 | [`../AGENTS.md`](../AGENTS.md) | Non-negotiable agent rules — notably architecture §19 and "never claim semantic understanding of weights" |
| 5 | [`MASTER_PLAN.md`](MASTER_PLAN.md) | Scope, phases, critical path, release criteria |
| 6 | [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) | v1 / seam / deferred / non-goal boundary |
| 7 | The remaining `.plan/*.md` architecture documents | Per-subsystem design |
| 8 | `phases/*/README.md` | Phase entry and exit conditions |
| 9 | `tasks/QM-XXXX-*/TASK.md` | Executable unit of work |

**Where the v1 scope decision came from.**
[`../Quatricmorph - Standalone Business, Market, and Technical Strategy.md`](<../Quatricmorph - Standalone Business, Market, and Technical Strategy.md>)
(August 2026) is the origin of the decision that v1 is one diagnostic rather than
the visualization platform. That decision is now **recorded in
[`../ARCHITECTURE.md`](../ARCHITECTURE.md) §17.1**, where rank 2 carries it, and
the root documents were amended to match by
[`QM-0167`](tasks/QM-0167-root-document-amendment/TASK.md). The strategy document
remains the place to read the reasoning, the market evidence, and the conditions
that would reverse the decision; it is authoritative on *why*, not a rank in the
table above. [`STRATEGY_ALIGNMENT.md`](STRATEGY_ALIGNMENT.md) is the
reconciliation and records what the deferral costs.

`decisions/ADR-CANDIDATE-*.md` are **not** authoritative. They are proposals with
a recommended default and a decision deadline. A task that depends on an
undecided ADR candidate says so in its `Dependencies` section.

## Document map

| Document | Answers |
| --- | --- |
| [`STRATEGY_ALIGNMENT.md`](STRATEGY_ALIGNMENT.md) | **Read first.** Why v1 is one diagnostic, what moved out of v1, and what that costs |
| [`MASTER_PLAN.md`](MASTER_PLAN.md) | What v1 is, in what order, and when it is done |
| [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) | What is deliberately *not* in v1 |
| [`DIAGNOSTIC_ARCHITECTURE.md`](DIAGNOSTIC_ARCHITECTURE.md) | The quantization-error engine: metrics, chunking, mixed-precision frontier, what may never be claimed |
| [`REPORT_ARCHITECTURE.md`](REPORT_ARCHITECTURE.md) | The Markdown report, the JSON manifest, and the CI/agent interface |
| [`VALIDATION_PLAN.md`](VALIDATION_PLAN.md) | Design partners, PMF signals, kill criteria, pivot criteria, the 90-day sequence |
| [`REPOSITORY_ANALYSIS.md`](REPOSITORY_ANALYSIS.md) | What the repository actually contains, verified |
| [`CURRENT_ARCHITECTURE.md`](CURRENT_ARCHITECTURE.md) | How today's code is put together, and where it diverges from `ARCHITECTURE.md` |
| [`TARGET_ARCHITECTURE.md`](TARGET_ARCHITECTURE.md) | The program boundaries the MVP must end with |
| [`DATA_ARCHITECTURE.md`](DATA_ARCHITECTURE.md) | Four planes, IDs, cache keys, exactness, versioning |
| [`GRID_ARCHITECTURE.md`](GRID_ARCHITECTURE.md) | The shared 3D grid ruler, N-D extension, sphere-block cells |
| [`TILING_ARCHITECTURE.md`](TILING_ARCHITECTURE.md) | LOD ladder, block layout, `.qtile`, GLB, `tileset.json` |
| [`CUDA_ARCHITECTURE.md`](CUDA_ARCHITECTURE.md) | RTX 3090 targeting, memory budgets, CPU fallback, determinism |
| [`CESIUM_VIEWER_ARCHITECTURE.md`](CESIUM_VIEWER_ARCHITECTURE.md) | Viewer components, picking, exactness display, error states |
| [`MATRIX_WORKSPACE_ARCHITECTURE.md`](MATRIX_WORKSPACE_ARCHITECTURE.md) | `mm` reuse, grid alignment, real-block matmul, state separation |
| [`WEIGHTQL_ARCHITECTURE.md`](WEIGHTQL_ARCHITECTURE.md) | Grammar, AST, aliases, cost planning, execution tiers |
| [`QUERY_UI_ARCHITECTURE.md`](QUERY_UI_ARCHITECTURE.md) | Chat, KaTeX, candidates, cost confirmation, cancellation |
| [`API_CONTRACTS.md`](API_CONTRACTS.md) | Daemon routes, payloads, status codes, progress transport |
| [`SCHEMA_PLAN.md`](SCHEMA_PLAN.md) | The four JSON schemas, `.qtile` binary format, versioning and migration |
| [`MEMORY_BUDGET.md`](MEMORY_BUDGET.md) | Every buffer, as a formula with a configuration variable |
| [`PERFORMANCE_PLAN.md`](PERFORMANCE_PLAN.md) | Benchmarks, budgets, and what is measured versus asserted |
| [`SECURITY_MODEL.md`](SECURITY_MODEL.md) | File access, parsing, resource limits, sanitization, origin policy |
| [`TEST_STRATEGY.md`](TEST_STRATEGY.md) | What is tested where, and which tests need which hardware |
| [`MIGRATION_STRATEGY.md`](MIGRATION_STRATEGY.md) | Moving from today's layout to the target without breaking the baseline |
| [`RISK_REGISTER.md`](RISK_REGISTER.md) | Ranked risks with owners, triggers, and mitigations |
| [`REQUIREMENT_TRACEABILITY.md`](REQUIREMENT_TRACEABILITY.md) | Every requirement → tasks → verification |
| [`DEPENDENCY_GRAPH.md`](DEPENDENCY_GRAPH.md) | Task dependency edges, shared-file risk, integration gates |
| [`EXECUTION_ORDER.md`](EXECUTION_ORDER.md) | Critical path, parallel lanes, hardware gating |
| [`DEFINITION_OF_DONE.md`](DEFINITION_OF_DONE.md) | The 32 v1 acceptance criteria, and the disposition of the previous 46 |

## Task numbering

```text
QM-XXXX-short-name/TASK.md
```

`XXXX` is a stable four-digit number. Numbers are allocated in blocks of ten per
phase and are **never reused or renumbered** — a task that is abandoned becomes
`Superseded` and keeps its number, because task IDs appear in commit messages,
branch names, and `Dependencies` lists elsewhere in this plan.

| Block | Phase | v1? |
| --- | --- | --- |
| `QM-0001`–`QM-0009` | Phase 00 — Repository baseline and shared contracts | Partly |
| `QM-0010`–`QM-0019` | Phase 01 — SafeTensors ingestion completion | Partly |
| `QM-0020`–`QM-0029` | Phase 02 — Catalog and NSIR completion | Partly |
| `QM-0030`–`QM-0039` | Phase 03 — Block runtime and compute | **Yes** |
| `QM-0040`–`QM-0049` | Phase 04 — Tensor tiles, GLB, and tileset | Deferred |
| `QM-0050`–`QM-0059` | Phase 05 — Cesium model viewer | Deferred |
| `QM-0060`–`QM-0069` | Phase 06 — Grid matrix workspace | Deferred |
| `QM-0070`–`QM-0079` | Phase 07 — WeightQL and chat | Deferred |
| `QM-0080`–`QM-0089` | Phase 08 — Integration and performance | Partly |
| `QM-0090`–`QM-0099` | Phase 09 — Documentation and release | Partly |
| `QM-0100`–`QM-0119` | **Phase 10 — Out-of-core proof on a real checkpoint** | **Yes** |
| `QM-0120`–`QM-0139` | **Phase 11 — Quantization-error diagnostic engine** | **Yes** |
| `QM-0140`–`QM-0149` | **Phase 12 — Report, manifest, and CI/agent interface** | **Yes** |
| `QM-0150`–`QM-0159` | **Phase 13 — Diagnostic surface** | **Yes** |
| `QM-0160`–`QM-0169` | **Phase 14 — Validation and v1 release** | **Yes** |

Phases 10–14 are the v1 critical path. Phases 00–09 are retained in full: the
tasks in them that v1 needs are marked `Ready`/`Blocked` as usual, and the ones
v1 does not need are marked `Deferred` with the phase that will take them up.
Nothing is deleted and nothing is renumbered.

## Status vocabulary

Task status is recorded in the `## Status` section of each `TASK.md`.

| Status | Meaning |
| --- | --- |
| `Undefined` | The task exists as a placeholder; its scope is not yet written |
| `Ready` | Dependencies are `Complete`; an agent may start now |
| `In Progress` | Claimed and being worked |
| `Blocked` | A dependency, an ADR decision, or hardware is missing. The blocker must be named |
| `Implemented` | Code exists and the acceptance criteria are believed met; verification has not run |
| `Verified` | The verification plan ran and passed; evidence is recorded |
| `Complete` | `Implemented` **and** `Verified`, and `STATUS.md` has been updated |
| `Deferred` | Sound work, correctly specified, **not in v1**. The line below the status names the release that takes it up. An agent may not start it |
| `Superseded` | Replaced by another task, which must be named |

`Deferred` is not `Blocked`. A `Blocked` task is waiting on a dependency and
becomes `Ready` when that dependency lands. A `Deferred` task is waiting on a
**product decision** — v1 shipping, or a pivot in [`VALIDATION_PLAN.md`](VALIDATION_PLAN.md)
§5 — and never becomes `Ready` on its own. Deferral is reversible by editing one
line; that is the point of recording it this way rather than deleting the task.

**A task is not `Complete` until it is both implemented and verified.** This
mirrors the distinction `STATUS.md` already enforces between `Implemented` and
`Verified`, and the reason is the same: this repository's credibility rests on
never claiming a capability it has not exercised.

**`Ready` is derived.** A `Blocked` task becomes `Ready` when every task ID in
its `Dependencies` section has reached `Complete` and any ADR or hardware it
names is available. The `## Status` field records the **current** state, not the
transition — it always holds exactly one of the nine values above, on its own
line, so it can be parsed. Any blocker is named on the line *below* the value.

**Which tasks are `Ready` is never read from this document.** It is derived from
the corpus, and it changes every time a task is claimed or completed, so any
count written here is stale the moment a run starts. Derive it:

```bash
for f in .plan/tasks/*/TASK.md; do
  awk '/^## Status$/{getline; getline; print; exit}' "$f"
done | sort | uniq -c | sort -rn
```

The counts must sum to the number of `TASK.md` files (`ls .plan/tasks/*/TASK.md |
wc -l`); a shortfall means a file whose `## Status` section does not hold exactly
one value on its own line, which is a parser-contract defect to fix in that file.
[`EXECUTION_ORDER.md`](EXECUTION_ORDER.md) §10 lists the tasks whose v1 unblock
condition makes them `Ready` ahead of their original dependency edges, and §11
names the ones to start first.

`Hardware-Unverified` is not a task status. It is a *requirement* status in
`STATUS.md`, and it is what a CUDA task's requirement stays at when the task's
code is written but no RTX 3090 has run it. Such a task sits at `Implemented`,
never at `Verified`.

## Dependency conventions

* `Dependencies` lists task IDs that must reach `Complete` first. Not "should" —
  must.
* `Blocks` lists the inverse. The two must agree; [`DEPENDENCY_GRAPH.md`](DEPENDENCY_GRAPH.md)
  is generated by reading both directions and is the place where a disagreement
  shows up.
* A dependency on an **ADR candidate** is written as
  `ADR-CANDIDATE-0XX (decision required)` and blocks the task at `Blocked` until
  the ADR is promoted to `docs/decisions/ADR-0XX-*.md`.
* A dependency on **hardware** is written as `Requires: RTX 3090` or
  `Requires: Apple M-series GPU` in the `Parallelization` section.
* Cross-phase dependencies are allowed and expected. Phases are a reading aid and
  an integration gate, not a barrier.

## How an autonomous agent selects the next task

1. Read [`../STATUS.md`](../STATUS.md) first. It, not this plan, is the record of
   what is built. If it disagrees with a task's `Repository Evidence`, stop and
   raise the discrepancy — the plan is stale, and fixing the plan is the task.
2. Read [`EXECUTION_ORDER.md`](EXECUTION_ORDER.md) and take the earliest task on
   the critical path whose status is `Ready`.
3. If no critical-path task is `Ready`, take any `Ready` task from a parallel
   lane whose `Parallelization` section does not name a file already being edited
   by another in-progress task.
4. If a task requires hardware that is unavailable, do not start it. Set it
   `Blocked` with the reason, and pick from the CUDA-free lane listed in
   [`EXECUTION_ORDER.md`](EXECUTION_ORDER.md).
4a. **Never start a `Deferred` task.** If a `Deferred` task looks like the
   obvious next thing to build, that is the deferral working as intended. Moving
   a task out of `Deferred` is a product decision recorded in
   [`VALIDATION_PLAN.md`](VALIDATION_PLAN.md), not an engineering judgement call.
5. Before writing code, confirm every path in `Files Expected to Change` still
   exists. If one does not, the plan is stale — fix the plan first.
6. Follow [`../AGENTS.md`](../AGENTS.md). Its non-negotiable rules — notably
   architecture §19 and "never claim semantic understanding of weights" — outrank
   any convenience this plan might seem to offer.

## How verification evidence is recorded

Each `TASK.md` ends with `## Completion Evidence`. Before a task moves to
`Verified`, that section must be filled in **in the task file itself** with:

* the exact command that was run, copy-pasteable;
* its output, or the decisive excerpt — test counts, benchmark numbers, byte
  sizes;
* the commit SHA the run was made against;
* for anything visual, a file path to a screenshot or generated artifact;
* for anything hardware-dependent, the device name and driver version.

"Tests pass" is not evidence. `290 passed; 0 failed` with the command above it
is. This is the standard `STATUS.md` already holds itself to, and tasks inherit
it.

When a task reaches `Complete`, its requirement rows in `../STATUS.md` are
updated in the same **squash merge commit** as the implementation. A task that
changes behaviour without updating `STATUS.md` is not `Complete`.

**There is no pull-request path in this repository — but pushing works.** These
are two different permissions and the distinction matters:

* **Pushing to `origin` succeeds.** `origin` is
  `git@github.com:quatricmorph/quatricmorph.git` and every merge reaches it over
  SSH. `git ls-remote origin refs/heads/main` matches local `main`, and
  `git reflog show origin/main` holds an `update by push` entry per merge.
* **No pull request is creatable.** The `gh` token authenticates as
  `MarkdownOfficial`, for whom
  `gh api repos/quatricmorph/quatricmorph --jq .permissions` returns
  `"push": false`. Opening a PR through that token is genuinely unavailable.

So tasks are integrated as **local squash merges onto local `main`, which is then
pushed**. The squash merge commit, not a PR, is the review artifact, and the
evidence that would have been the PR body is written to
`.plan/evidence/QM-XXXX.md` and lands in that same commit. Recorded in
[`PLAN_CHANGELOG.md`](PLAN_CHANGELOG.md) (2026-08-04, "push to `origin` succeeds;
Run 2's credential finding is superseded", commit `3394510`) and in
[`ORCHESTRATION_STATE.md`](ORCHESTRATION_STATE.md) "Run 4", which states it in
these terms: *"pushing over SSH as `hmthanh` and creating a PR via the `gh` token
are different permissions, and only the PR half is genuinely unavailable."*

An earlier revision of this paragraph said the token "cannot push … Permission
denied", citing the *superseded* 2026-08-04 entry ("Run 1's Stage 0 credential
halt is superseded") rather than the one that supersedes it. That claim was false
when written, and is corrected here.

## How plans are updated when repository facts change

This plan cites line numbers, symbol names, and test names. Those drift.

* **A citation that no longer resolves is a bug in this plan**, and fixing it
  takes precedence over the task that discovered it.
* When a task changes a file another task cites, the changing task updates the
  citation in its own squash merge commit. `DEPENDENCY_GRAPH.md` lists the
  high-traffic files where this is most likely. Where a task cannot reach the
  citing document — because another task owns it, or is editing it concurrently —
  it records the correction as a `PLAN_CHANGELOG.md` finding naming the owner
  instead of editing across the boundary.
* When `ARCHITECTURE.md` and this plan disagree, `ARCHITECTURE.md` wins and this
  plan is corrected — **except** where a divergence has been deliberately
  recorded in [`CURRENT_ARCHITECTURE.md`](CURRENT_ARCHITECTURE.md) §"Recorded
  divergences" with an ADR candidate attached. Those are open questions awaiting
  a decision, not plan errors.
* Adding a task is cheap and expected. Renumbering is forbidden.
