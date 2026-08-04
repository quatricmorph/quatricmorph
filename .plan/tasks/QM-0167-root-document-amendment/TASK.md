# QM-0167 — Root-document amendment

## Status

Ready

May start as soon as the scope decision is accepted. Until it completes, the
repository contains two different definitions of "the first MVP", and
`.plan/README.md` carries a precedence note explaining which one wins.

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
4. `.plan/README.md`'s precedence note is removed and rank 1 is restored to
   `ARCHITECTURE.md`.
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
