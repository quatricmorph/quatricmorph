# QM-0001 — Baseline verification and evidence capture

## Status

In Progress

> **Controller correction, 2026-08-04 (Run 2).** Every `web: 101` in this task is
> **stale**. `QM-0006` repaired `apps/web`'s test collection — commit `103297d` had
> pointed vitest's include globs at a directory that did not exist, so the suite was
> silently collecting 3 of 12 files and reporting `27 passed` at exit 0. After
> `QM-0006`, the measured suite is **13 files / 115 tests** (the pre-existing corpus
> is exactly 12 files / 101 tests; `QM-0006` adds a 14-test guard that both root
> `include` globs match).
>
> **Record `{"rust": 290, "web": 115}`.** Copying `101` from the lines below would
> set the floor 14 tests below reality — the same class of defect `QM-0006` exists
> to fix, and the floor may only ever rise. Re-measure before writing the file; do
> not trust this note either.
>
> This task is sequenced **after `QM-0006` merges**. It also edits
> `.github/workflows/build.yaml`, which `QM-0006` owns until then. While you are in
> that file, fix the `upload-artifact` step's `name:` — it reads
> `quatricmorph-quatricmorph-workspace`, the same double-sed wart `QM-0006` fixed in
> `package.json`. It is an artifact label, not a path, so `QM-0006` left it
> deliberately and recorded it under `## Not performed`.


## Phase

Phase 00 — Repository baseline and shared contracts

## Objective

Produce a reproducible, committed record of the current test baseline, and a
script that fails if any of the 102 `Verified` requirements regresses.

## Repository Evidence

* `cargo test --workspace` → **exit 0, 290 passed; 0 failed** (run at commit
  `5ca434d`).
* `cd apps/web && npx vitest run` → **exit 0, 101 passed**, 12 files, 832 ms.
* `STATUS.md` claims the same counts for its `2026-08-03` run — confirmed
  accurate, and therefore adopted as this plan's factual baseline.
* `.github/workflows/build.yaml` runs `rust` (fmt, clippy, build, test),
  `fixtures` (regenerate + `git diff --exit-code`), and `web` (vitest + build).
* `README.md` lists CLI commands whose output has never been asserted in a test.

## Requirements Covered

The 102 `Verified` rows in `STATUS.md`, and acceptance criteria `MVP-02`,
`MVP-03`, `MVP-04`, `MVP-07`, `MVP-29`, `MVP-32`, `MVP-33`, `MVP-35`.

## Dependencies

None. This is the plan's first task.

## Blocks

`QM-0002`, `QM-0004`, and by extension every task in the plan — nothing should be
built on an unverified baseline.

## Parallelization

Fully parallel with `QM-0002` and `QM-0003`. Touches no source file.

## Program Boundary

Repository-wide tooling. No crate or application changes behaviour.

## Scope

* Run both suites and capture full output.
* Add `scripts/verify-baseline.sh` running fmt, clippy, both suites, and the
  documented CLI commands, asserting counts do not fall below a recorded floor.
* Assert the `README.md` CLI examples produce the documented output — notably
  `q-cli value fixtures/tiny-llama-2shard 'Q[10]' --index 100,42` returning
  `0.006408154033124447`.
* Record the baseline in this task's `Completion Evidence`.

## Out of Scope

Changing any test · adding tests for unbuilt features · modifying `STATUS.md`
(that is `QM-0091`) · CI changes beyond invoking the script.

## Files Expected to Change

* `.github/workflows/build.yaml` — invoke the script in the `rust` job.

## Files Expected to Add

* `scripts/verify-baseline.sh`
* `scripts/baseline.json` — recorded floor: `{"rust": 290, "web": 115}` (see the controller correction under `## Status`; the `101` this line originally carried is stale)
* `scripts/verify-baseline.test.sh` — unit tests for the guard's own parsing and
  comparison functions, run as its preflight step. **Added by `impl-agent-1`
  during implementation; recorded in `.plan/PLAN_CHANGELOG.md`.** Controller §6
  requires the guard's behaviour to be tested, and these tests cannot live in
  `cargo test` or `vitest` without raising the very counts the floor records.

## Files Expected to Remove or Deprecate

None.

## Data Contracts

`scripts/baseline.json`:

```json
{ "commit": "<re-measure>", "rust_tests": 290, "web_tests": 115,
  "cli_golden": { "value_q10_100_42": "0.006408154033124447" } }
```

The floor may only be **raised** by a task that adds tests. A task that lowers it
is rejected in review.

## Memory and Performance Constraints

Total runtime under 5 minutes on a laptop, or it will be skipped in practice.

## Implementation Plan

1. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -D warnings`,
   `cargo test --workspace`, and `npx vitest run` from `apps/web`; capture output.
2. Write `scripts/verify-baseline.sh` — parse `test result:` lines, sum passed
   and failed, compare against `baseline.json`, exit non-zero on any regression.
3. Add CLI golden checks for `inspect`, `layers`, `value`, `slice`, `query`, and
   `stats` against `fixtures/tiny-llama-2shard`.
4. Wire it into CI after the existing test step.

## Error Handling

* Any suite failing → non-zero exit, with the failing test names.
* Passed count below the floor → explicit "baseline regression" message naming
  both numbers.
* A missing fixture → clear message pointing at `fixtures/generate_fixtures.py`.
* The script must not depend on a network, a GPU, or a Python virtualenv.

## Acceptance Criteria

1. `scripts/verify-baseline.sh` exits 0 on a clean checkout.
2. It exits non-zero when a test is deliberately broken (demonstrated).
3. It exits non-zero when `baseline.json`'s floor is raised above the real count.
4. CLI golden checks pass, including the exact scalar value.
5. CI invokes it and it passes.
6. Total runtime under 5 minutes.

## Verification Plan

**Automated** — the script itself, in CI.
**Manual** — break one test, confirm a clear failure, restore it.

## Suggested Commands

Verified today:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd apps/web && npx vitest run
cargo run -p q-cli -- value fixtures/tiny-llama-2shard 'Q[10]' --index 100,42
```

Introduced by this task:

```bash
./scripts/verify-baseline.sh
```

## Test Cases

| Input | Expected |
| --- | --- |
| Clean checkout | exit 0; `rust=290 web=115` or higher |
| One `#[test]` renamed to fail | exit 1 naming it |
| `baseline.json` rust floor set to 999 | exit 1, "baseline regression: 290 < 999" |
| `q-cli value … --index 100,42` | `0.006408154033124447` |
| `fixtures/` removed | exit 1 pointing at the generator |

## Risks

| Risk | Mitigation |
| --- | --- |
| Parsing `test result:` lines is brittle across cargo versions | Also check the process exit code; the count is a secondary assertion |
| The script becomes the only thing anyone runs | It is a floor, not a substitute for `cargo test` |

## Completion Evidence

* Full output of all four commands, with counts.
* The commit SHA the run was made against.
* Output of the deliberate-failure demonstration.
* CI run URL showing the new step passing.

### Recorded baseline — measured at `793e122`, worktree `.qm-worktrees/qm-0001`

| Command | Result | Exit |
| --- | --- | --- |
| `cargo fmt --all -- --check` | no output | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 lines matching `^warning`/`^error` | 0 |
| `cargo build --workspace --all-targets` | — | 0 |
| `cargo test --workspace` | **290 passed; 0 failed; 0 ignored** over **39 binaries** | 0 |
| `cd apps/web && npx vitest run` | **Test Files 13 passed (13) / Tests 115 passed (115)** | 0 |
| `<venv>/bin/python fixtures/generate_fixtures.py` then `git diff --exit-code -- fixtures/` | fixtures reproduce byte-identically | 0 |
| `./scripts/verify-baseline.sh` | 27 checks green, `verify-baseline: OK`, 13 s warm / 82 s partly-cold | 0 |

`cargo test` prints one `test result:` line per binary, not a total; 290 is the
sum over all 39. `ignored=0` and `115 passed (115)` together confirm nothing was
`#[ignore]`d or `.skip()`ped to reach these numbers.

**Deliberate-failure demonstration** (`.plan/evidence/QM-0001.md` §4):
`crates/q-statistics/src/lib.rs:335` `assert_eq!(s.count, 4)` → `5`.

```
FAIL  cargo test --workspace exited 101
--- failing Rust tests ---
tests::hand_computed_moments_on_a_small_fixture
--------------------------
rust: 221 passed; 1 failed; 0 ignored; 15 binaries
baseline regression: 221 < 290 (rust tests)
baseline regression: 15 < 39 (rust test binaries)
verify-baseline: FAILED — 4 check(s) did not pass
EXIT=1
```

Restored with `git checkout --`; `git diff --exit-code` exit 0 and the file's
SHA1 is identical before and after (`96031105d85e8603956271b04f397e0bd020250c`).

**Floor-raised demonstration** (§3): `rust_tests` `290` → `999` →
`baseline regression: 290 < 999 (rust tests)`, `EXIT=1`, exactly one check
failing. Restored byte-exact (`cmp` exit 0).

**No CI run URL.** No push is possible from this repository, so AC-5's
"and it passes" half is unevidenced; the invocation itself is verified by
re-parsing `.github/workflows/build.yaml`.

## Orchestration

| Field | Value |
| --- | --- |
| Controller state | `Awaiting Independent Review` |
| Lane | P |
| Wave | 0 |
| Branch | `task/qm-0001-baseline-verification` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0001` |
| Base commit | `793e122` |
| Implementation commit | `b0c9b46` — the whole change: scripts, CI wiring, evidence record, this file |
| Head commit | one commit ahead of `b0c9b46`, containing **only** the two SHA annotations in this row and in the evidence record's `## Task`. A commit cannot contain its own hash; `b0c9b46` is the SHA to review. `git diff b0c9b46 HEAD` touches no file under `scripts/`, `.github/`, `crates/` or `fixtures/`. |
| Implementation agent | `impl-agent-1` |
| Evidence record | `.plan/evidence/QM-0001.md` |
| Merge path | L |

**Tests added:** 46, all in `scripts/verify-baseline.test.sh` (shell unit tests
over the guard's parsing and comparison functions, run as the guard's own
preflight step). They are deliberately **not** `cargo test` or `vitest` tests —
adding them to either suite would raise the very counts the floor records.
`cargo test --workspace` therefore still measures 290 and `npx vitest run` still
measures 115 at this head.

**Floor change:** none → `{"rust_tests": 290, "rust_binaries": 39,
"web_tests": 115, "web_files": 13}`. No floor existed before this task;
`scripts/` did not exist at `793e122`, so every merge before this one landed
with no floor guard in force.

**Floor staleness at merge time — controller action required.** The floor above
is a measurement of `793e122`. A **trial merge with `main@ae600c9`**, run inside
this worktree and then aborted, measured **`318 passed; 0 failed; 0 ignored; 39
binaries`** with web unchanged at `115/13` — confirming the controller's 318 by
measurement rather than on trust. The guard exited **0** on that merged tree with
**every one of the 12 CLI goldens intact**, including through `QM-0012`'s
285-line rewrite of `crates/q-cli/src/main.rs`.

`scripts/baseline.json` is nevertheless committed at **290**, the honest
measurement of this task's base commit. **The controller re-measures on the
merged `main` and raises `rust_tests` to the measured value in the same squash
commit.** The harness compares with `-ge`, not equality, so raising the floor to
318 needs **no change to `scripts/verify-baseline.test.sh`**. Until it is raised
the guard reports `FLOOR IS STALE by 28` and still exits 0 — a floor below
reality is the guard's documented blind spot, not a failure.
See `.plan/evidence/QM-0001.md` §12.

**Guard firing demonstrated** (`.plan/evidence/QM-0001.md` §3, §4): floor raised
to 999 → exit 1, `baseline regression: 290 < 999 (rust tests)`; one Rust test
deliberately broken → exit 1, naming
`tests::hand_computed_moments_on_a_small_fixture`. Both restorations verified
byte-exact.
