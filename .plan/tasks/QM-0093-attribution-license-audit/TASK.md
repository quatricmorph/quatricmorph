# QM-0093 — Attribution and license audit

## Status

In Progress

No longer waits on `QM-0080` (deferred). A licensing audit needs no running pipeline.

**v1 dependency rewiring.** This task's `## Dependencies` section names tasks that are now `Deferred`. For v1 it is unblocked by the tasks named above; the original edges return with the post-v1 platform release. See [`EXECUTION_ORDER.md`](../../EXECUTION_ORDER.md) §10.

## Phase

Phase 09 — Documentation and release

## Objective

Confirm `mm`'s MIT license and Meta Platforms attribution are intact, and that
every dependency's license is recorded.

## Repository Evidence

* `mm/LICENSE` — MIT, Meta Platforms, Inc.
* `apps/web/quatricmorph-workspace/LICENSE` — the same text, reproduced.
* `apps/web/quatricmorph-workspace/NOTICE.md` — the derivation attribution.
* `apps/web/quatricmorph-workspace/package.json` — `"license": "MIT"`, description:
  *"Matrix multiplication workspace, derived from Meta's mm (see NOTICE.md)"*.
* `Cargo.toml` — `license = "MIT OR Apache-2.0"`.
* `AGENTS.md`: *"`mm/` Historical matrix-viz reference — read-only; do not
  delete; not product surface."*
* `README.md` §License already states the derivation.

## Requirements Covered

`DOC-004`, `MVP-44`.

## Dependencies

`QM-0080`.

## Blocks

`QM-0094`.

## Parallelization

Parallel with `QM-0090`, `QM-0092`.

## Program Boundary

Licensing and attribution files.

## Scope

* Verify `mm/` is byte-identical to its state at the baseline commit.
* Verify the reproduced LICENSE matches `mm/LICENSE` exactly.
* Verify `NOTICE.md` attributes correctly.
* Enumerate every runtime and dev dependency with its license.
* Confirm no copyleft dependency conflicts with `MIT OR Apache-2.0`.
* Add a repository-wide `NOTICE` aggregating third-party attributions.

## Out of Scope

Changing the project's license · removing dependencies · legal advice.

## Files Expected to Change

* `README.md` §License, if the dependency set changed

## Files Expected to Add

* `NOTICE` — repository root, aggregated third-party attributions
* `scripts/license-audit.sh`

## Files Expected to Remove or Deprecate

**None. `mm/` is never modified.**

## Data Contracts

```jsonc
{ "mm_unmodified": true,
  "mm_license_sha256": "…",
  "workspace_license_matches": true,
  "notice_present": true,
  "dependencies": [ { "name": "three", "version": "0.185.1", "license": "MIT" },
                    { "name": "cesium", "version": "…", "license": "Apache-2.0" } ],
  "copyleft_conflicts": [] }
```

## Memory and Performance Constraints

None. The audit runs in seconds.

## Implementation Plan

1. `git diff <baseline> -- mm/` — must be **empty**.
2. `sha256sum` both LICENSE files and compare their text.
3. Verify `NOTICE.md` names Meta Platforms, Inc. and the derivation.
4. `cargo license` (or `cargo-about`) for Rust dependencies.
5. `license-checker` for npm dependencies across all four packages.
6. Flag any GPL, AGPL, or SSPL dependency.
7. Generate the aggregated `NOTICE`.
8. Wire the script into CI.

## Error Handling

* Any modification to `mm/` → **fail**. It is read-only by policy, and the
  attribution depends on it being unmodified.
* LICENSE text mismatch → fail, showing the diff.
* A copyleft dependency → fail; it must be reviewed before release.
* A dependency with no declared license → fail; unknown is not acceptable.

## Acceptance Criteria

1. `git diff` against the baseline shows **no change under `mm/`**.
2. `apps/web/quatricmorph-workspace/LICENSE` matches `mm/LICENSE` textually.
3. `NOTICE.md` attributes Meta Platforms, Inc. and states the derivation.
4. `package.json` and `Cargo.toml` declare licenses correctly.
5. Every dependency's license is recorded.
6. No GPL/AGPL/SSPL dependency is present.
7. No dependency lacks a declared license.
8. `NOTICE` exists at the root aggregating third-party attributions.
9. `README.md` §License is accurate.
10. `license-audit.sh` runs in CI.

## Verification Plan

**Automated** — `license-audit.sh` in CI.
**Manual** — read `NOTICE.md` and confirm the attribution is accurate and
respectful of the original work.

## Suggested Commands

```bash
git diff 5ca434d -- mm/                       # must be empty
sha256sum mm/LICENSE apps/web/quatricmorph-workspace/LICENSE
cargo license --json                           # introduced here
npx license-checker --json --production
./scripts/license-audit.sh
```

## Test Cases

| Input | Expected |
| --- | --- |
| `git diff` on `mm/` | Empty |
| Both LICENSE files | Textually identical |
| `NOTICE.md` | Names Meta Platforms, Inc. |
| Rust dependency licenses | All recorded |
| npm dependency licenses | All recorded |
| A GPL dependency added | Audit fails |
| A dependency with no license | Audit fails |
| `NOTICE` | Present and complete |

## Risks

| Risk | Mitigation |
| --- | --- |
| `mm/` modified by an unrelated task | The diff check is mechanical and runs in CI |
| A transitive copyleft dependency | The audit covers the full tree, not direct dependencies only |
| Attribution technically present but obscure | It appears in `README.md`, `NOTICE.md`, and `package.json` |

## Completion Evidence

* `git diff` on `mm/`, empty.
* LICENSE checksums.
* The full dependency-license table.
* The generated `NOTICE`.
* The CI audit run.

## Orchestration

| Field | Value |
| --- | --- |
| Controller state | `Awaiting Independent Review` |
| Lane | **not assigned** — `.plan/EXECUTION_ORDER.md` §4 lists lanes P, Q, R, S, T, U, V and QM-0093 appears in none of them. Not invented here. (`Merge path` below is `L`; that is a different field.) |
| Wave | **6** — `.plan/EXECUTION_ORDER.md` §2, *"Wave 6 — release"*, lists `QM-0093 attribution and licensing` in the `parallel:` block with `QM-0090`, `QM-0092`, `QM-0167` |
| Branch | `task/qm-0093-attribution-license-audit` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0093` |
| Base commit | `793e122` |
| Implementation commit | `358f008` — `docs(legal): record dependency attribution and licence audit [QM-0093]` |
| Head commit | the documentation-only commit that adds this section, sitting directly on top of `358f008`. Its SHA cannot appear inside itself; resolve with `git rev-parse task/qm-0093-attribution-license-audit`. **The commit to review is `358f008`.** |
| Implementation agent | `impl-agent-2` |
| Evidence record | `.plan/evidence/QM-0093.md` |
| Merge path | L |
| Tests added | **none — Documentation-only exempt class (controller §6.1)** |

All four changed files are in `358f008`
(`NOTICE`, `scripts/license-audit.sh`, `.github/workflows/build.yaml`,
`README.md`). The head commit adds only `.plan/evidence/QM-0093.md` and the
`## Status` and `## Orchestration` edits to this file, so
`git diff 358f008..HEAD` touches nothing outside `.plan/`.

**Floor, measured on `358f008`, unchanged in both directions:**

| Gate | Floor | Measured | Exit |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | — | no output | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | — | no warnings | 0 |
| `cargo test --workspace` | 290 passed / 0 failed | **290 passed / 0 failed** | 0 |
| `cd apps/web && npx vitest run` | 115 passed / 13 files | **115 passed / 13 files** | 0 |
| `scripts/license-audit.sh` (new) | — | 11 checks ok, 13 non-fatal copyleft notes | 0 |

No test was added, and none was removed. The Documentation-only exemption is
discharged by tracing every claim in `NOTICE` to a file citation, to package
metadata resolved by a lockfile, or to an explicit **not verified** marker —
`NOTICE` §9 tabulates the split and §9.1 lists all 11 gaps by name. The audit
script's nine failure paths are exercised in `.plan/evidence/QM-0093.md`
§`Negative paths tested`.

`scripts/baseline.json` was neither created nor edited — `QM-0001` owns it. The
counts above are this run's measurements, recorded in evidence only.

**Merge-order warning for the controller.**
`.plan/ORCHESTRATION_STATE.md` "Sequencing decisions (file-scope conflicts)"
already holds `QM-0093` behind `QM-0001` because *"Both create files under
`scripts/`"* — that part is benign: this branch adds only
`scripts/license-audit.sh`, `QM-0001` adds only `scripts/baseline.json`, and
`scripts/` did not exist before either. The row **above** it is the live risk:
`QM-0001` is itself held behind `QM-0006` because *"Both edit
`.github/workflows/build.yaml`"*, and this branch now edits that same file. It
appends a self-contained `licenses:` job immediately before the existing `web:`
job and changes no existing line, so a textual conflict is unlikely but not
impossible. **Whichever of `QM-0001` and `QM-0093` merges second must re-run
`scripts/license-audit.sh` and the full gate set after the merge**, not before.

**For the reviewer, the two findings most worth a second opinion:**

1. `apps/web/quatricmorph-workspace/src/assets/droid_sans_regular.typeface.js`
   ships in the built product and its own embedded metadata states a proprietary
   Ascender Corporation EULA, not the Apache-2.0 licence usually associated with
   Droid Sans. `NOTICE` §2.1 records the conflict and asserts neither licence.
2. The audit fails on GPL/LGPL/AGPL/SSPL-with-no-permissive-alternative
   (acceptance criterion 6) rather than on any copyleft at all
   (`## Error Handling`). Twelve `lightningcss` MPL-2.0 dev binaries sit in the
   gap between those two readings and are reported, not failed.
