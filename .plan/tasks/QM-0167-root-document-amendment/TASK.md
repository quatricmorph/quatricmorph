# QM-0167 — Root-document amendment

## Status

Complete

Merged to `main` as `f132393` (squash, Path L) and pushed to `origin`. Independent review by `review-agent-5` at reviewed SHA `22260e7`: `CHANGES_REQUESTED` for three recording gaps, **all three resolved** (`b4b36b4` by the implementer, `c772110` by the controller) and re-verified before merge. Byte-identity of `ARCHITECTURE.md` §1–§16, §19, §20 confirmed by independent SHA-256.

Implemented on `task/qm-0167-root-document-amendment` and awaiting independent
review. The two definitions of "the first MVP" are reconciled and
`.plan/README.md`'s precedence note is removed. Evidence:
[`../../evidence/QM-0167.md`](../../evidence/QM-0167.md).

## Phase

Phase 14 — Validation and v1 release

## Objective

Amend the root documents so `ARCHITECTURE.md`, `MASTER_DOCUMENT.md`, `docs/`, and
`.plan/` agree on what v1 is — restoring a single source of truth.

## Repository Evidence

* `.plan/STRATEGY_ALIGNMENT.md` §6 — the table of every document, section, and
  claim that now disagrees.
* `.plan/README.md` — the precedence note this task removes.
* `../ARCHITECTURE.md` §17–§18; `../MASTER_DOCUMENT.md` §2, §20;
  `../docs/ROADMAP.md`; `../docs/PRODUCT_BRIEF.md`; `../docs/requirements/VIZ_MVP.md`.
* `../README.md` and `../STATUS.md` — **already accurate**; they describe only
  what exists and need no scope amendment.

## Requirements Covered

`V1-H5`.

## Dependencies

None technically. Sequenced late so that v1's shape is settled before the root
documents are rewritten to describe it.

## Blocks

`QM-0165`.

## Parallelization

Lane V. Touches root documents that `QM-0090` also edits — sequential with it.

## Program Boundary

`ARCHITECTURE.md`, `MASTER_DOCUMENT.md`, `docs/`, `.plan/README.md`.

## Scope

Per `STRATEGY_ALIGNMENT.md` §6:

| Document | Section | Change |
| --- | --- | --- |
| `ARCHITECTURE.md` | §17 roadmap | Insert the diagnostic wedge as the current release; renumber the visualization phases as following it. **Do not delete them** |
| `ARCHITECTURE.md` | §18 acceptance criteria | Retitle as the platform-release criteria; point to `.plan/DEFINITION_OF_DONE.md` for v1 |
| `MASTER_DOCUMENT.md` | §2 primary MVP workflow | Replace with the v1 pipeline; retain the platform workflow, labelled as the following release |
| `MASTER_DOCUMENT.md` | §20 acceptance criteria | Same disposition as `ARCHITECTURE.md` §18 |
| `docs/ROADMAP.md` | Phase 0 | "now" moves from the tiling spike to the diagnostic wedge |
| `docs/PRODUCT_BRIEF.md` | Immediate engineering wedge | Replace with the diagnostic wedge; keep the strategic wedge |
| `docs/requirements/VIZ_MVP.md` | `TILE-*` | Mark deferred; the file stays |
| `.plan/README.md` | Precedence note | Remove once the conflict is gone |

**Explicitly unchanged:** `ARCHITECTURE.md` §1–§16 and §19 — the four data planes,
ingestion, NSIR, catalog, block and LOD model, memory discipline, and the
structural prohibitions. v1 is built on them, not against them.

## Out of Scope

Rewriting the platform architecture · deleting deferred content · changing
`STATUS.md`'s facts (`QM-0091` regenerates it) · editing the strategy document.

## Files Expected to Change

`ARCHITECTURE.md`, `MASTER_DOCUMENT.md`, `docs/ROADMAP.md`,
`docs/PRODUCT_BRIEF.md`, `docs/requirements/VIZ_MVP.md`, `.plan/README.md`.

## Implementation Plan

1. Work down `STRATEGY_ALIGNMENT.md` §6 row by row.
2. For each superseded section, keep the content and change its **label** — the
   platform release's criteria are still needed and still correct.
3. Add a short "Release history and scope" section to `ARCHITECTURE.md` recording
   that v1 is the diagnostic wedge and why, citing the strategy document by name
   and date. A future reader must be able to reconstruct the decision.
4. Remove `.plan/README.md`'s precedence note; restore `ARCHITECTURE.md` to rank 1
   for implementation.
5. Re-read `README.md` and `STATUS.md` to confirm neither acquired a stale claim.
6. Check every cross-reference still resolves.

## Error Handling

* A section whose disposition is unclear → leave it and note it rather than
  guessing; an ambiguous root document is better than a confidently wrong one.
* A deferred criterion that turns out to be satisfied by v1 → mark it satisfied
  where it stands; do not move it.

## Acceptance Criteria

1. Every row of `STRATEGY_ALIGNMENT.md` §6 is dispositioned.
2. No deferred content is deleted — only relabelled.
3. `ARCHITECTURE.md` records the scope decision, its date, and its source.
4. `.plan/README.md`'s precedence note is removed and `ARCHITECTURE.md` is
   restored as the top authority **for implementation**: Accepted ADRs carrying a
   `Departs from:` line rank 1 — overriding `ARCHITECTURE.md` for exactly the
   section each names — and `ARCHITECTURE.md` ranks 2, above `STATUS.md`,
   `AGENTS.md`, and the whole of `.plan/`. The strategy document is no longer a
   rank at all.

   *Amended by `impl-agent-3` during implementation ([QM-0167]).* The original
   wording — "rank 1 is restored to `ARCHITECTURE.md`" — was **not** followed as
   written, because an unqualified rank 1 would contradict the three Accepted ADRs
   that carry `Departs from:` lines (`ADR-003` §2.1/§5, `ADR-007` §16, `ADR-009`
   §8.2) and `ARCHITECTURE.md` §2.1's own instruction *"Do not 'fix' the code to
   match this paragraph."* The criterion's intent is preserved; only its letter
   changed. Reasoning in `.plan/evidence/QM-0167.md` § "Criterion 4".
5. Every internal link resolves.
6. `ARCHITECTURE.md` §1–§16 and §19 are unchanged — verified by diff.
7. No document claims a capability the tests do not demonstrate.
8. `README.md` and `STATUS.md` remain accurate.

Criterion 6 is checked by reading the diff, not by intention: the temptation
while editing §17–§18 is to "tidy" the sections around them.

## Verification Plan

**Manual** — read the diff in full.
**Automated** — a link checker across the repository's Markdown.

## Suggested Commands

```bash
git diff --stat ARCHITECTURE.md MASTER_DOCUMENT.md docs/
git diff ARCHITECTURE.md | head -200
grep -rniE 'accuracy (loss|drop)|will cost you|trillion.parameter (model )?(loaded|computed)' \
  README.md ARCHITECTURE.md MASTER_DOCUMENT.md docs/
```

## Test Cases

| Check | Expected |
| --- | --- |
| `STRATEGY_ALIGNMENT.md` §6 rows | All dispositioned |
| Deferred content | Present, relabelled |
| `ARCHITECTURE.md` §1–§16, §19 diff | Empty |
| Internal links | All resolve |
| Precedence note in `.plan/README.md` | Removed |
| Forbidden-claim grep | No match outside a negation |

## Risks

| Risk | Mitigation |
| --- | --- |
| Deferred criteria deleted and lost | Criterion 2; `STRATEGY_ALIGNMENT.md` §6 is the checklist |
| The reasoning is lost for a future reader | Criterion 3 records it in `ARCHITECTURE.md` itself |
| Unrelated sections edited while nearby | Criterion 6, verified by diff |
| Done too early, before v1's shape settles | Sequenced in Wave 6 |

## Completion Evidence

* The full diff of every amended document.
* The dispositioned `STRATEGY_ALIGNMENT.md` §6 table.
* Link-checker output.
* Confirmation that `ARCHITECTURE.md` §1–§16 and §19 are untouched.

---

## Orchestration

| Field | Value |
| --- | --- |
| Controller state | `Awaiting Independent Review` |
| Lane | V |
| Wave | 6 |
| Branch | `task/qm-0167-root-document-amendment` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0167` |
| Base commit | `793e122` |
| Implementation commit | `62bb132` — `docs: reconcile root documents with ARCHITECTURE.md and accepted ADRs [QM-0167]` |
| Head commit | a docs-only commit sitting on top of `62bb132`. A commit's SHA cannot appear inside itself; resolve with `git rev-parse task/qm-0167-root-document-amendment`. **The commit to review is `62bb132`** — `git diff 62bb132..HEAD` touches only this table and `.plan/evidence/QM-0167.md` |
| Implementation agent | `impl-agent-3` |
| Evidence record | [`../../evidence/QM-0167.md`](../../evidence/QM-0167.md) |
| Merge path | L |
| Tests added | **none — documentation-only exempt class** (controller §6.1) |

**Tests added: none, and that is the declared disposition, not an omission.** The
change set is six Markdown documents; it adds no symbol, route, schema field, or
branch of executable behaviour for a test to bind to. What replaces the test is
the exempt class's own condition — every claim traces to a passing test, an
accepted ADR, or an explicit "not verified" marker — discharged claim by claim in
`## Tests added` of the evidence record.

**Test floor — unchanged in both directions.** The `After` column is measured on
this branch. The `Before` column is **taken from the controller's task brief and
was not independently re-measured at `793e122`**; the change set touches no Rust
or TypeScript source, so it cannot move either count.

| Gate | Before (`793e122`, not re-measured) | After (measured) | Exit |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | clean | clean | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean | clean | 0 |
| `cargo test --workspace` | 290 passed; 0 failed | **290 passed; 0 failed; 0 ignored** | 0 |
| `cd apps/web && npx vitest run` | 115 passed, 13 files | **115 passed, 13 files** | 0 |

`STATUS.md` still records `101` web tests, measured at the older commit
`5ca434d`. That figure is stale; `QM-0091` owns regenerating it and this task did
not touch it.

**Diff base.** `main` is `04991e9`, two commits ahead of `793e122`; both touch
only `.plan/tasks/QM-0100-real-checkpoint-acquisition/TASK.md`, which this branch
must not and does not modify. `git merge-base main HEAD` is `793e122`, so
`git diff main...HEAD` and `git diff 793e122..HEAD` describe the same change set.

**Scope note.** `ARCHITECTURE.md` is edited because this task's own
`## Files Expected to Change` names it. §1–§16 and §19 are byte-identical to
`793e122`, proven by section extraction in the evidence record. Phases 0–6 were
re-sequenced after v1 but **not renumbered** — renumbering would orphan every
citation of `Phase 0` and `TILE-*` in `AGENTS.md`, `docs/`, `STATUS.md`, and the
commit history, and `AGENTS.md` is not in this task's scope, so the resulting
dangling reference could not be repaired here. Taken under this task's own
`## Error Handling` clause and recorded in `ARCHITECTURE.md` §17.1 itself.
