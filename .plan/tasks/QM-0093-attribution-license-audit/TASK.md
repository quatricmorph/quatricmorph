# QM-0093 — Attribution and license audit

## Status

Blocked

Unblocks when `QM-0080` reaches `Complete`.

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
