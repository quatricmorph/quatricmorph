# Plan Changelog

Records material corrections to `.plan/` discovered during autonomous orchestration.
Format per controller §16.

## 2026-08-04 — CONTROLLER — Stage 0 credential halt

**Discovered during:** Stage 0 capability probe
**Defect:** `gh` CLI authenticated as `MarkdownOfficial` has `permissions.push: false`
(pull only) on `quatricmorph/quatricmorph`, while SSH `git push --dry-run` to
`origin` succeeds. Pull-request creation is therefore unavailable; Path A and
Path B both require a PR artifact per `.plan/README.md` / controller §1.
**Correction:** No plan task content changed. Controller halted before Wave 0
worktrees. Merge path not selected. See `.plan/ORCHESTRATION_STATE.md`.
**Files changed:** `.plan/ORCHESTRATION_STATE.md` (created), `.plan/PLAN_CHANGELOG.md` (created)
**Dependency impact:** All tasks remain unstarted by this run.
**Evidence:** Stage 0 probe output recorded verbatim in `ORCHESTRATION_STATE.md`
(`permissions.push: false`; push dry-run succeeded for `qm-capability-probe`).

## 2026-08-04 — CONTROLLER — Run 1's Stage 0 credential halt is superseded

**Discovered during:** Run 2 Stage 0
**Defect:** Run 1 halted before any worktree because no pull request could be
created. Controller §1 now fixes the merge path as **L (local squash)** and states
that no run halts on a credential.
**Correction:** Halt removed. Merge path L adopted. `.plan/README.md`'s requirement
that a task's `STATUS.md` update land "in the same pull request" is unsatisfiable
with no PR; substituted by an evidence record at `.plan/evidence/QM-XXXX.md` landing
in the same squash commit as the implementation.
**Files changed:** `.plan/ORCHESTRATION_STATE.md`, `.plan/PLAN_CHANGELOG.md`, `.plan/evidence/` (created)
**Dependency impact:** None. All tasks become schedulable.
**Evidence:** `gh api …/branches/main/protection` → 404 (unprotected);
`permissions.push: false`; Run 1's own `git push --dry-run` over SSH succeeded.

## 2026-08-04 — QM-0006 (new) — commit 103297d silently disabled 74 web tests and broke the web build

**Discovered during:** Run 2 baseline verification (controller §5)
**Defect:** Commit `103297d` ("Refactor workspace references from matrix-workspace
to quatricmorph-workspace") rewrote every *reference* to the web workspace across
57 files — `apps/web/package.json`, `apps/web/vitest.config.ts`,
`.github/workflows/build.yaml`, `STATUS.md`, `README.md`, and four accepted ADRs —
but **never renamed the directory**, which is still `apps/web/matrix-workspace`.
Three consequences, all silent:
1. `vitest.config.ts` includes `quatricmorph-workspace/src/**`, which matches
   nothing. `npx vitest run` reports **27 passed (3 files)** and exit 0. Nine test
   files holding **74 tests** are not collected. No failure is printed.
2. `apps/web/package.json` lists workspace path `quatricmorph-workspace`, which
   does not exist, so `npm run build --workspace quatricmorph-workspace` exits 1
   with "No workspaces found". The web build gate does not run at all.
3. The sed double-applied to the package name:
   `apps/web/matrix-workspace/package.json` reads
   `"name": "quatricmorph-quatricmorph-workspace"`.
**Correction:** New task **`QM-0006` — Web workspace path repair**, Phase 00,
Lane S, next free ID in the Phase 00 block. It performs the `git mv` that 103297d
omitted and adds a regression test asserting every path in the `workspaces` array
resolves on disk. **It is a prerequisite of `QM-0001`**, which would otherwise
record 27 as the permanent web floor — a number `QM-0001`'s own floor-only-rises
rule would then make irreversible.
**Direction of the fix (rename the directory, do not revert the config):** 55 files
now reference `quatricmorph-workspace`, including `ADR-001`, `ADR-006`, `ADR-009`,
`ADR-010`, `STATUS.md`, `README.md` and CI. Only 8 files still say
`matrix-workspace`, all of them `.plan/` prose and `COMPONENTS_MAP.md`. Reverting
the config would require editing four accepted ADRs and `STATUS.md` outside the
tasks that own them, which controller §11 forbids. Renaming the directory edits
neither.
**Files changed:** `.plan/tasks/QM-0006-web-workspace-path-repair/TASK.md` (added)
**Dependency impact:** `QM-0006` blocks `QM-0001`. `QM-0002`, `QM-0093`, `QM-0167`
are §6.1-exempt tasks required to show "the counts did not fall", which is
meaningless measured against 27; they are sequenced after `QM-0006` as well.
**Evidence:** `npx vitest run` → `Test Files 3 passed (3) / Tests 27 passed (27)`
against 12 `*.test.ts` files on disk; `27 + 74 = 101` matches `STATUS.md` exactly.
`npm run build --workspace quatricmorph-workspace` → exit 1, "No workspaces found".
`grep '"name"' apps/web/matrix-workspace/package.json` →
`"name": "quatricmorph-quatricmorph-workspace"`.

## 2026-08-04 — CONTROLLER — controller §0.2's "citation defect" framing predates 103297d

**Discovered during:** Run 2 baseline verification
**Defect:** Controller §0.2 records the `quatricmorph-workspace` /
`matrix-workspace` split as a citation defect for `QM-0002` to fix inside `.plan/`.
That framing was written before `103297d` moved build configuration, CI, `STATUS.md`,
`README.md` and four accepted ADRs to the new name. It is now a build defect, and
`QM-0002`'s declared boundary is `.plan/` only — "This task changes no repository
file" — so `QM-0002` cannot repair it.
**Correction:** Routed to `QM-0006` (repository files) instead. `QM-0002` keeps its
`.plan/`-only boundary and updates the 8 remaining `.plan/` citations.
**Files changed:** none beyond this entry.
**Dependency impact:** `QM-0002` scope unchanged; repair moved to `QM-0006`.
**Evidence:** `QM-0002/TASK.md` "## Program Boundary — `.plan/` only. This task
changes no repository file."

## 2026-08-04 — CONTROLLER — `.plan/README.md` status-value count and Ready count both disagree with the corpus

**Discovered during:** Run 2 task parse
**Defect:** Two internal inconsistencies in `.plan/README.md`:
1. Its status table lists **nine** values while its prose says a task's `## Status`
   holds "exactly one of the **eight** values above". The table gained `Deferred`
   and the sentence was not updated.
2. It states three tasks start `Ready`; parsing all 89 `TASK.md` files yields
   **nine**: `QM-0001`, `QM-0002`, `QM-0010`, `QM-0012`, `QM-0093`, `QM-0100`,
   `QM-0140`, `QM-0160`, `QM-0167`.
**Correction:** Readiness is derived by parsing, never read from prose. Both
sentence fixes routed to `QM-0002`.
**Files changed:** none beyond this entry.
**Dependency impact:** None.
**Evidence:** Parsed distribution over 89 task directories: 44 `Deferred`,
36 `Blocked`, 9 `Ready` — matching controller §2 exactly.
