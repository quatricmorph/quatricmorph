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

## 2026-08-04 — QM-0001 — the specified web floor of 101 is stale; it must be 115

**Discovered during:** `QM-0006` implementation (Stage D), reported by `impl-agent-1`
**Defect:** `QM-0001` specifies `scripts/baseline.json` as
`{"rust": 290, "web": 101}` in three places — `## Files Expected to Add`,
`## Data Contracts`, and its `## Test Cases` table. That `101` was measured before
`103297d` broke vitest's collection, and it does not account for the guard test
`QM-0006` was required to add. After `QM-0006` the measured suite is **13 files /
115 tests**: the pre-existing corpus is exactly 12 files / 101 tests, plus a
14-test guard that both root `include` globs match.
**Why it matters:** the floor may only ever be raised. Writing `101` would set it
14 tests below reality and permanently license a 14-test regression — the same
class of defect `QM-0006` exists to fix, and one the floor rule then makes
irreversible.
**Correction:** all three `web: 101` occurrences in `QM-0001/TASK.md` changed to
`115`, with a note under `## Status` recording why, instructing the implementer to
re-measure rather than trust the note, and sequencing the task after `QM-0006`
merges. The `"commit": "5ca434d"` placeholder in the data contract was replaced
with `<re-measure>` because `HEAD` has moved well past `5ca434d`.
**Also routed to `QM-0001`:** `.github/workflows/build.yaml`'s `upload-artifact`
step still carries `name: quatricmorph-quatricmorph-workspace`, the same
double-sed wart `QM-0006` fixed in `package.json`. `QM-0006` examined it and left
it deliberately — it is an artifact label, not a path, and its `path:` field is
correct — recording it under `## Not performed`. `QM-0001` is the next task to
open that file and should fix it there.
**Files changed:** `.plan/tasks/QM-0001-baseline-verification/TASK.md`
**Dependency impact:** `QM-0001` gains a hard sequencing edge behind `QM-0006`
(already recorded when `QM-0006` was created).
**Evidence:** `npx vitest run` in `../.qm-worktrees/qm-0006/apps/web` →
`Test Files 13 passed (13) / Tests 115 passed (115)`, exit 0, measured by the
controller independently of the implementer. Excluding the new file: 12 files /
101 tests.

## 2026-08-04 — CONTROLLER — a note inserted above a `## Status` value broke the parser

**Discovered during:** applying the `QM-0001` correction above
**Defect:** the controller inserted the correction note immediately after the
`## Status` heading, ahead of the value. `.plan/README.md`'s parser contract takes
the first non-empty line after the heading, so `QM-0001` began parsing as
`> **Controller correction…` instead of `Ready`. Verified by re-running the parse
across all task files: one file returned an illegal value.
**Correction:** the value now sits immediately under the heading and the note
follows it. Re-parsed all 90 task directories: 36 `Blocked`, 44 `Deferred`,
10 `Ready`, zero illegal values.
**Lesson recorded for later tasks:** prose added to a `## Status` section must go
**below** the single value, never above it. `.plan/README.md`'s "exactly one of the
values above, on its own line, so it can be parsed" constrains position, not just
content.
**Files changed:** `.plan/tasks/QM-0001-baseline-verification/TASK.md`
**Dependency impact:** none.
**Evidence:** `for d in .plan/tasks/*/; do awk '/^## Status/{f=1;next} f&&NF{print;exit}' "$d/TASK.md"; done | sort | uniq -c` →
before: one `> **Controller correction…` row; after: `36 Blocked / 44 Deferred / 10 Ready`.

## 2026-08-04 — CONTROLLER — `git push origin main` is unavailable; local `main` is the integration branch

**Discovered during:** the `QM-0006` merge (Stage G), first push attempt
**Defect:** neither available credential can update `main` on the remote. SSH
authenticates as `hmthanh` and then hangs indefinitely on pack upload (exit 124,
no rejection message). HTTPS via the `gh` token is explicitly denied: *"Permission
to quatricmorph/quatricmorph.git denied to MarkdownOfficial"*, HTTP 403. The `gh`
token reports `permissions.push: false`, consistent with the 403.
**Correction:** merge path L proceeds against the controller's local `main`, which
becomes the integration branch. `.plan/README.md`'s requirement that a task's
`STATUS.md` update land "in the same pull request" was already substituted by an
evidence record in the same squash commit; this entry records that the *push* half
of path L is also unavailable, so "reachable from `origin/main`" becomes
"reachable from the controller's `main`" for every dependency check in this run.
**Files changed:** `.plan/ORCHESTRATION_STATE.md`, `.plan/PLAN_CHANGELOG.md`
**Dependency impact:** none — all dependency checks retarget to local `main`.
**Evidence:** the three commands and their verbatim output are recorded in
`.plan/ORCHESTRATION_STATE.md` under "Push to `origin` is not available".
Note that Run 1's `git push --dry-run` success was for creating a *new branch*,
which is a different permission from updating `main`.
