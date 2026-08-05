# QM-0002 — Plan↔repository reconciliation and divergence register

## Status

Complete

## Phase

Phase 00 — Repository baseline and shared contracts

## Objective

Validate every citation in `.plan/` against the repository, and produce a single
register of every place `ARCHITECTURE.md`, `STATUS.md`, the code, and this plan
disagree — each with an owner and an ADR candidate.

## Repository Evidence

Three divergences are already identified and must be registered, not
re-discovered:

1. **Plane mapping.** `ARCHITECTURE.md` §8.2 says `A: XY, B: YZ, C: XZ`.
   `apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts:9-10` documents and
   implements `X→J, Y→I, Z→K` with `A on I×K, B on K×J, C on I×J`, which resolves
   to `A: YZ, B: XZ, C: XY`. The task specification §16 agrees with the code.
   13 tests hold the code's version.
2. **Three spatial authorities.** `q_tensor_runtime::Lod`,
   `q_tileset::GeometricError::for_lod` (`ROOT_GEOMETRIC_ERROR = 1024.0`), and
   `apps/web/model-viewer/src/lod-policy.ts:103` (`1024 / 2 ** lod`, under the
   comment at `:101` *"mirrors `q_tileset::GeometricError`"* — hand-mirrored, no
   test).
3. **Catalog technology.** `ARCHITECTURE.md` §5 names DuckDB/Arrow/Parquet;
   the implementation is SQLite. Recorded in `ADR-003`, tracked as `CAT-010`.

Also: `ARCHITECTURE.md` §16 shows a `quatricmorph/` root (`ADR-001`), and §12.1
lists "React or Svelte" where `apps/web/` uses no framework.

## Requirements Covered

`DOC-005`.

## Dependencies

None.

## Blocks

`QM-0004`, `QM-0060`, `QM-0090`.

## Parallelization

Fully parallel with `QM-0001` and `QM-0003`. **Writes only inside `.plan/`.**

## Program Boundary

`.plan/` only. This task changes no repository file.

## Scope

* Mechanically check every file path, symbol name, and test name cited anywhere
  in `.plan/`.
* Produce `.plan/DIVERGENCE_REGISTER.md`: each divergence with sources, evidence,
  recommended resolution, ADR candidate, and owning task.
* Correct any `.plan/` document whose citation no longer resolves.
* Confirm the three known divergences and search for others by comparing
  `ARCHITECTURE.md` §§4–14 against the implementing crates.
* **The checker skips two documented classes of backtick-path-shaped text**,
  because both are legitimate and would otherwise fail the task that introduces
  the checker:
  1. Paths inside a `## Test Cases` block — e.g. this task's own
     `` `crates/q-nope/src/lib.rs` ``, a deliberate example of an unresolvable
     citation.
  2. Paths in sentences asserting **absence** — e.g.
     `CURRENT_ARCHITECTURE.md` §6.3's *"`apps/desktop/` does not exist
     (correctly — Tauri is a non-goal)"*.
  Both conventions are documented in the script's header, and a path claimed as
  evidence must never rely on either exemption.
* Paths listed under `## Files Expected to Add` are planned, not existing, and
  are checked for **shape**, not existence.

## Out of Scope

Editing `ARCHITECTURE.md` (that is `QM-0090`) · editing `STATUS.md` (`QM-0091`) ·
changing any code · resolving a divergence — this task **registers**, it does not
decide.

## Files Expected to Change

* Any `.plan/*.md` with a stale citation.

## Files Expected to Add

* `.plan/DIVERGENCE_REGISTER.md`
* `scripts/check-plan-citations.sh`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

Register row: `id · sources · evidence · recommended · adr · owning task ·
status ∈ {Open, Decided, Resolved}`.

## Memory and Performance Constraints

None. The citation checker runs in seconds.

## Implementation Plan

1. Extract every `path/to/file`, `Symbol::name`, and `test_name` from `.plan/`.
2. For each, confirm it resolves: `test -f`, `grep -q`, or `cargo test --list`.
3. Report unresolved citations; fix them in place.
4. Read `ARCHITECTURE.md` §§4–14 against the implementing crates; note every
   difference.
5. Write the register, cross-referencing the ADR candidates.
6. Add the checker to CI as a non-blocking warning first, blocking once clean.

## Error Handling

* An unresolvable citation is a **plan bug**, fixed here, not deferred.
* An ambiguous symbol (two matches) is reported with both.
* A divergence with no obvious resolution is registered `Open` with both options
  stated. Registering beats guessing.

## Acceptance Criteria

1. `scripts/check-plan-citations.sh` exits 0 — every citation resolves.
2. `DIVERGENCE_REGISTER.md` contains at least the three known divergences.
3. Every row names an ADR candidate and an owning task.
4. No file outside `.plan/` and `scripts/` is modified.
5. Every `.plan/` document with a stale citation is corrected.

## Verification Plan

**Automated** — the citation checker in CI.
**Manual** — a reviewer picks five citations at random and confirms them by hand.

## Suggested Commands

Introduced by this task:

```bash
./scripts/check-plan-citations.sh
```

Useful today:

```bash
grep -rn "geometricErrorForLod\|ROOT_GEOMETRIC_ERROR" crates apps
grep -n "XY plane\|YZ plane\|XZ plane" ARCHITECTURE.md
```

## Test Cases

| Input | Expected |
| --- | --- |
| A `.plan/` doc citing `crates/q-nope/src/lib.rs` | Reported unresolved |
| A doc citing `q_tileset::GeometricError::for_lod` | Resolves |
| A doc citing a renamed test | Reported unresolved |
| `ARCHITECTURE.md` §8.2 versus `grid-ruler.ts:9` | Registered as divergence 1 |

## Risks

| Risk | Mitigation |
| --- | --- |
| The checker produces false positives on prose | Only check citations in backticks matching a path or `::` pattern |
| The register becomes stale | Every task that resolves a divergence updates its row |

## Completion Evidence

* Checker output, exit 0.
* `DIVERGENCE_REGISTER.md` contents.
* The list of `.plan/` corrections made.
* `git status` showing nothing modified outside `.plan/` and `scripts/`.

## Orchestration

* **State:** Awaiting Independent Review
* **Fix cycle:** 1 of at most 3. `review-agent-7` returned `CHANGES_REQUESTED` at
  head `6e99e62`; findings `B1`–`B7` are addressed under
  `.plan/evidence/QM-0002.md` `## Fix cycle 1`.
* **Lane:** V
* **Wave:** 0
* **Branch:** `task/qm-0002-plan-repo-reconciliation`
* **Worktree:** `../.qm-worktrees/qm-0002`
* **Base:** `eca5a6a7e2f4f40e9ab4a9a58250ccb16f0a32a6` (`git merge-base main HEAD`)
* **Head:** recorded in `.plan/evidence/QM-0002.md` at merge; the fix-cycle commit
  is the one whose subject carries `[QM-0002]`. Every measurement in the evidence
  was taken at that commit's parent, `1d49ffa`.
* **`main` when fix cycle 1 committed:** `e8d7997` — it advanced past this branch's
  base three times during the cycle (`QM-0010` and `QM-0020` with code, raising the
  recorded floor to rust 502/43; then an ADR-011 amendment with none). Deliberately
  not chased; the reasoning is in `.plan/evidence/QM-0002.md` `## Claim limits`.
* **Agent:** `impl-agent-11` (fix cycle 1); `impl-agent-6` (implementation)
* **Evidence:** `.plan/evidence/QM-0002.md`
* **Merge path:** L (local squash merge onto local `main`, then pushed to `origin`)
* **Tests added:** none — **Plan-only exempt class** (controller §6.1): every path
  in `## Files Expected to Change` is under `.plan/**` and no behaviour changed.
  Test floor unchanged and machine-checked at this branch's base `eca5a6a`: rust 434
  before = 434 after, web 115 before = 115 after, `./scripts/verify-baseline.sh`
  exit 0 with every count "at floor" against the 434/43/115/13 that
  `scripts/baseline.json` holds at `eca5a6a`. `main` has since raised the rust floor
  to 502/43 at `4bddf6c`; this branch adds no code and does not touch that file, so
  it cannot lower it. Detail under `.plan/evidence/QM-0002.md`
  `### The floor, and that no count fell`.
  Qualification: `.plan/tools/check-plan-citations.py` is executable tooling and
  is evidenced by recorded invocations against the real corpus (12 unresolved
  before, 2 after, on the same tool, at base `eca5a6a`) rather than by a unit test.
  It exits **1**, so acceptance criterion 1 is still not claimed as met.
* **Note:** the branch was fast-forwarded from its original base `ace7d09` to
  `4e0e85c` before any editing, because `ace7d09` predates `QM-0006`'s directory
  rename and the path citations this task repairs point the opposite way there.
  It was then rebased onto `3339485` when `main` advanced again, and the
  independent review found **that** base fifteen commits stale — the root cause of
  all seven findings — so fix cycle 1 rebased onto `eca5a6a`, the base recorded
  above. Rationale and proof in `.plan/evidence/QM-0002.md` `## Recovery` and
  `## Fix cycle 1`.
