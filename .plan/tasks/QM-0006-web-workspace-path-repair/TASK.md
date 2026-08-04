# QM-0006 — Web workspace path repair

## Status

Ready

**Start this before `QM-0001`.** `QM-0001` records the permanent test floor, and
its own specification names `{"rust": 290, "web": 101}`. The tree currently runs
27 web tests. Recording a floor against a broken collector would either fail
`QM-0001` outright or enshrine a number 74 below reality, and the floor may only
ever rise.

## Phase

Phase 00 — Repository baseline and shared contracts

## Objective

Complete the directory rename that commit `103297d` began, so that the web test
collector finds all twelve test files again and the web build gate runs at all —
and add a regression test that makes this class of breakage impossible to
reintroduce silently.

## Repository Evidence

Commit `103297d` ("Refactor workspace references from matrix-workspace to
quatricmorph-workspace") rewrote every *reference* to the web workspace across 57
files but never renamed the directory. Measured at `fe501e5`:

* `apps/web/vitest.config.ts` includes `quatricmorph-workspace/src/**/*.test.ts`.
  No such directory exists. `npx vitest run` reports
  `Test Files 3 passed (3) / Tests 27 passed (27)` and **exit 0** — a silent
  under-collection, not a failure.
* Twelve `*.test.ts` files exist under `apps/web`. Nine live under
  `matrix-workspace/` and hold 74 tests. `27 + 74 = 101`, exactly the count
  `STATUS.md` claims.
* `apps/web/package.json` lists workspace path `quatricmorph-workspace`.
  `npm run build --workspace quatricmorph-workspace` exits 1 with
  `npm error No workspaces found`.
* `apps/web/matrix-workspace/package.json` reads
  `"name": "quatricmorph-quatricmorph-workspace"` — the rename sed applied twice
  to a name that already carried the `quatricmorph-` prefix.
* `.github/workflows/build.yaml` was rewritten by the same commit and refers to
  the new path, so CI's web job cannot be passing either.

## Requirements Covered

None new. Restores the measurable baseline that `DOC-004` and every later
floor-raising task depend on.

## Dependencies

None.

## Blocks

`QM-0001` (records the floor), and every task whose evidence must show the web
count did not fall — `QM-0002`, `QM-0093`, `QM-0167`.

## Parallelization

Lane S. Touches only `apps/web/**` and `.github/workflows/build.yaml`. Fully
parallel with `QM-0002` (`.plan/` only), `QM-0012` and `QM-0140` (Rust crates).

**Owns `.github/workflows/build.yaml` until it merges.** `QM-0001` also edits that
file and must be sequenced after this task.

## Program Boundary

`apps/web/**`, `.github/workflows/build.yaml`.

## Scope

* `git mv apps/web/matrix-workspace apps/web/quatricmorph-workspace`.
* Fix the double-prefixed package name to `quatricmorph-workspace`.
* Regenerate `apps/web/package-lock.json` so the recorded workspace path matches.
* Clear the stale workspace symlink under `apps/web/node_modules/`.
* Confirm `vitest.config.ts`, `apps/web/package.json` and `build.yaml` need no
  further edits — they already name the new path. Change them only if they do not.
* Add a regression test asserting every path in `apps/web/package.json`'s
  `workspaces` array resolves to a directory containing a `package.json`, and that
  every `include` glob in `vitest.config.ts` matches at least one file on disk.

## Out of Scope

Renaming the workspace back to `matrix-workspace` · editing any `ADR` ·
editing `STATUS.md` (that is `QM-0091`) · editing `ARCHITECTURE.md` (that is
`QM-0090`/`QM-0167`) · the eight remaining `.plan/` prose citations that still say
`matrix-workspace` (that is `QM-0002`) · changing any test's assertions.

## Direction of the fix, and why it is forced

55 files reference `quatricmorph-workspace`, including `ADR-001`, `ADR-006`,
`ADR-009`, `ADR-010`, `STATUS.md`, `README.md` and CI. Eight files still say
`matrix-workspace`, all of them `.plan/` prose plus `COMPONENTS_MAP.md`.

Reverting the configuration to `matrix-workspace` would require editing four
accepted ADRs and `STATUS.md` outside the tasks that own them, which the
controller's rules forbid. Renaming the directory edits neither. The rename is
therefore the only direction available, independent of which name is preferable.

## Files Expected to Change

* `apps/web/matrix-workspace/**` → `apps/web/quatricmorph-workspace/**` (rename)
* `apps/web/quatricmorph-workspace/package.json` — `name` field
* `apps/web/package-lock.json` — regenerated
* `.github/workflows/build.yaml` — only if it still resolves to a missing path

## Files Expected to Add

* `apps/web/quatricmorph-workspace/src/util/__tests__/workspace-paths.test.ts`

## Files Expected to Remove or Deprecate

None. The rename preserves history via `git mv`.

## Memory and Performance Constraints

None.

## Implementation Plan

1. **Write the failing test first.** Add `workspace-paths.test.ts` asserting each
   `workspaces` entry resolves and each vitest `include` glob matches ≥ 1 file.
   Run it against the broken tree and record the failure.
2. `git mv apps/web/matrix-workspace apps/web/quatricmorph-workspace`.
3. Fix the `name` field to `quatricmorph-workspace`.
4. `rm -rf apps/web/node_modules/quatricmorph-workspace apps/web/node_modules/.package-lock.json`
   then `npm install` in `apps/web` to regenerate the lock and the symlink.
5. Re-run the new test; record it passing.
6. `npx vitest run` — expect **12 files, 101 tests**.
7. `npm run build --workspace quatricmorph-workspace` — expect exit 0.
8. Confirm `git status` shows renames, not delete-plus-add.

## Error Handling

* If `npm install` rewrites unrelated dependency versions, restore them; this task
  changes paths, not the dependency set.
* If any of the 74 recovered tests **fails** once collected, that is a real
  regression `103297d` was hiding. Record it, fix it if it is a path artifact, and
  raise it as a separate finding if it is not. Do not delete or skip the test.

## Acceptance Criteria

1. `apps/web/quatricmorph-workspace/` exists; `apps/web/matrix-workspace/` does not.
2. `npx vitest run` collects **12 test files** and **101 tests**, 0 failed.
3. `npm run build --workspace quatricmorph-workspace` exits 0.
4. `apps/web/quatricmorph-workspace/package.json` `name` is `quatricmorph-workspace`.
5. The new regression test fails on the pre-rename tree and passes after.
6. `git log --follow` resolves through the rename for at least one moved file.
7. No ADR, `STATUS.md`, or `ARCHITECTURE.md` is modified.

## Verification Plan

**Automated** — `workspace-paths.test.ts`, plus the recovered 74 tests.
**Manual** — the vitest summary line and the npm build exit code, pasted whole.

## Suggested Commands

```bash
cd apps/web && npx vitest run
cd apps/web && npm run build --workspace quatricmorph-workspace
git status --short
git log --follow --oneline -3 -- apps/web/quatricmorph-workspace/src/math/matmul.ts
git diff --stat HEAD -- ':!apps/web/package-lock.json'
```

## Test Cases

| Input | Expected |
| --- | --- |
| `npx vitest run` before the rename | 3 files, 27 tests — the bug |
| `npx vitest run` after the rename | **12 files, 101 tests, 0 failed** |
| `workspace-paths.test.ts` before the rename | fails, naming the unresolved path |
| `workspace-paths.test.ts` after the rename | passes |
| A `workspaces` entry pointing at a missing directory | test fails with that path named |
| A vitest `include` glob matching nothing | test fails with that glob named |
| `npm run build --workspace quatricmorph-workspace` | exit 0 |

## Risks

| Risk | Mitigation |
| --- | --- |
| `npm install` bumps unrelated versions | Diff the lock; restore anything not path-related |
| A recovered test genuinely fails | Record it as a finding; never skip or delete it |
| The rename lands as delete+add, losing history | Use `git mv`; verify with `git log --follow` |
| `QM-0001` edits `build.yaml` concurrently | This task owns that file until it merges |

## Completion Evidence

* `npx vitest run` before and after, with both summary lines intact.
* The new test failing, then passing.
* `npm run build` exit code.
* `git status --short` showing `R` rename entries.
* `git log --follow` output proving history survived.
* Confirmation that no ADR, `STATUS.md`, or `ARCHITECTURE.md` was touched.
