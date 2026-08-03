# QM-0001 — Baseline verification and evidence capture

## Status

Ready

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
* `scripts/baseline.json` — recorded floor: `{"rust": 290, "web": 101}`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

`scripts/baseline.json`:

```json
{ "commit": "5ca434d", "rust_tests": 290, "web_tests": 101,
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
| Clean checkout | exit 0; `rust=290 web=101` or higher |
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
