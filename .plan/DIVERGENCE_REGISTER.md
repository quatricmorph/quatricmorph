# DIVERGENCE_REGISTER — where `ARCHITECTURE.md`, `STATUS.md`, the code and this plan disagree

Produced by `QM-0002`. **This document registers; it does not decide.** A row
whose `Status` is `Decided` names the ADR that decided it — reopening that
question is not a citation repair, and no task may "fix" a `Decided` row by
changing the side the ADR chose.

Row schema, per `QM-0002` `## Data Contracts`:

```text
id · sources · evidence · recommended · adr · owning task · status ∈ {Open, Decided, Resolved}
```

* **`Open`** — no ADR has decided it; both options are stated.
* **`Decided`** — an ADR is `Accepted`; the corrective edit may still be pending.
* **`Resolved`** — decided *and* every document now agrees.

Measured at `1d49ffa` (`QM-0002` rebased onto `main` at `eca5a6a`). Every path and
line number below was confirmed by hand or by
`.plan/tools/check-plan-citations.py`; the commands are in
`.plan/evidence/QM-0002.md`.

---

## 1. Register

| id | sources | evidence | recommended | adr | owning task | status |
| --- | --- | --- | --- | --- | --- | --- |
| `DIV-001` operand plane mapping | `ARCHITECTURE.md:533` §8.2 vs `apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts:9-10` | §8.2 says `A: XY plane`. `grid-ruler.ts:9-10` says `X → J (output cols), Y → I (output rows), Z → K (contraction)` and `A on I×K, B on K×J, C on I×J`, which resolves to `A: YZ, B: XZ, C: XY` — the opposite of §8.2 for A and C. 13 `grid-ruler` tests hold the code's version; the task specification §16 agrees with the code | **Keep the code.** Correct `ARCHITECTURE.md` §8.2 | `ADR-009` (Accepted 2026-08-04) | `QM-0090` | `Decided` |
| `DIV-002` three LOD authorities | `crates/q-tensor-runtime/src/lib.rs:35` (`pub enum Lod`), `crates/q-tileset/src/lib.rs:34,46` (`ROOT_GEOMETRIC_ERROR = 1024.0`, `GeometricError::for_lod`), `apps/web/model-viewer/src/lod-policy.ts:103` | The TypeScript copy is `1024 / 2 ** lod` under a comment at `:101` reading *"mirrors `q_tileset::GeometricError`"*. It is hand-mirrored: nothing tests the two against each other, so they can drift silently | Emit the ladder from one source, or add a test that pins the TypeScript to the Rust constant | none yet — candidate | `QM-0004` + `QM-0005` | `Open` |
| `DIV-003` catalog technology | `ARCHITECTURE.md` §2.1 and §5 vs `crates/q-catalog` | §2.1 names DuckDB / Arrow / Parquet for the Metadata Plane; the implementation is SQLite via `rusqlite`, bundled | **Keep SQLite for the MVP** — the workload is point lookups and small hierarchy queries | `ADR-003` (Accepted, scoped to this pass) | `QM-0090`; requirement row `CAT-010` | `Decided` |
| `DIV-004` repository root | `ARCHITECTURE.md` §16 vs the tree | §16 shows a `quatricmorph/` top directory; the Cargo workspace is at the repository root. `apps/desktop/` does not exist, correctly — Tauri is a non-goal | Keep the workspace at the root | `ADR-001` (Accepted) | `QM-0090` | `Decided` |
| `DIV-005` crate count | `ARCHITECTURE.md` §16 vs `Cargo.toml` | §16's tree lists sixteen crates and `q-cuda` is not among them. Measured at `1d49ffa`: `ls crates \| wc -l` → **18**, and `Cargo.toml` `[workspace] members` has **19** entries (18 crates + `tests`). The two crates §16 omits are `q-cuda`, which implements the `q_gpu::Backend` trait, and `q-report`, added by `QM-0140` (`f962028`). The count was **17** when this row was first written and rose the same day | **Keep `q-cuda`.** This is a decision, not drift; §16's prose already carries the `ADR-007` annotation. **No task may resolve this by proposing the crate be deleted, and no task may resolve a crate-count difference by deleting a crate.** `q-report` is a v1 deliverable, so §16's tree is what needs the edit | `ADR-007` (Accepted 2026-08-03) | `QM-0090` | `Decided` |
| `DIV-006` renderer stack | `ARCHITECTURE.md` §12.1 vs `apps/web/` | §12.1 lists "React or Svelte" for the Cesium prototype; `apps/web/` is plain TypeScript with Vite and no framework | **No framework** for the MVP — the viewer's state is a handful of selections and a camera | `.plan/decisions/ADR-CANDIDATE-010-viewer-shell.md` (candidate, undecided) | `QM-0090` | `Open` |
| `DIV-007` tensor rank ceiling | `q_source::TensorDescriptor` vs `q_tensor_runtime::BlockExtent` vs `q_tiles::QTileHeader` | `TensorDescriptor::shape` is `Vec<u64>` (arbitrary rank); `BlockExtent` is 2-D only; `QTileHeader` allows three | **Rank ≤ 3 is implemented; rank > 3 refuses rather than flattens.** No document may describe silent flattening | `ADR-010` (Accepted 2026-08-04) | `QM-0090` | `Decided` |
| `DIV-008` web workspace directory name | `apps/web/` vs `.plan/` prose | Commit `103297d` rewrote every *reference* from `matrix-workspace` to `quatricmorph-workspace` across 57 files but never renamed the directory, silently de-collecting 9 test files. `QM-0006` renamed the directory (`1cfdc9c`); eight `.plan/` files and `COMPONENTS_MAP.md` still carried the old name | Rename the directory, then correct the `.plan/` prose | none — a defect, not a decision | `QM-0006` (directory, **Complete**) + `QM-0002` (`.plan/` prose, this task) | `Resolved` inside `.plan/`; `COMPONENTS_MAP.md` still `Open` — see `DIV-010` |
| `DIV-009` baseline test counts | `STATUS.md:9-10` vs the tree at `1d49ffa` | `STATUS.md` claims `290 passed` Rust and `101 passed, 0 failed (12 files)` web. Measured at `1d49ffa`: Rust **`434 passed; 0 failed; 0 ignored`** over **43** `test result:` lines; web `115 passed (115)`, 13 files. The gap grew after this row was written: `QM-0006` restored the de-collected web files and added 14 tests, `QM-0012` added Rust tests (290 → 318), then `QM-0140`'s `crates/q-report` and `QM-0100`'s `tests/tests/real_checkpoint_record.rs` took it to 434 over 43. `STATUS.md` also names 131 requirement rows' worth of tests that this row does not enumerate | Regenerate `STATUS.md` from a **fresh** run, and regenerate it against `scripts/baseline.json` read at that moment rather than against any number written down here: the floor is raised at each merge by the controller, and it already stands at rust 502/43 on `main` at `4bddf6c` where this branch's base `eca5a6a` had 434/43 (web 115/13 on both) | none — expected drift, not a decision | `QM-0091` | `Open` |
| `DIV-010` `COMPONENTS_MAP.md` names the old workspace path | `COMPONENTS_MAP.md` vs `apps/web/` | `COMPONENTS_MAP.md` is at the repository root and still names the pre-rename workspace directory. `QM-0002` is bounded to `.plan/` (`## Program Boundary`: *".plan/ only. This task changes no repository file."*), so it is out of reach here. **No task's `## Files Expected to Change` names `COMPONENTS_MAP.md`** — `grep -rn "COMPONENTS_MAP" .plan/` returns only prose mentions | Give the file an owner, then correct the path | none | **unowned — needs assignment** | `Open` |
| `DIV-011` no pull-request path, but pushing works | `.plan/README.md` vs `.plan/PLAN_CHANGELOG.md` (2026-08-04, "push to `origin` succeeds", commit `3394510`) vs `.plan/ORCHESTRATION_STATE.md` "Run 4" vs `.plan/evidence/README.md` | `README.md` required a task's `STATUS.md` update to land "in the same pull request" and a citation repair "as part of its own pull request". **Two separate facts, and an earlier revision of this row conflated them.** (i) **No PR is creatable**: `gh api repos/quatricmorph/quatricmorph --jq .permissions` → `"push": false` for `MarkdownOfficial`. (ii) **Pushing to `origin` succeeds** over SSH: `git ls-remote origin refs/heads/main` equals local `main` and `git reflog show origin/main` holds an `update by push` entry per merge. Integration is a local squash merge onto local `main`, which is then pushed | Describe the squash merge commit as the review artifact and `.plan/evidence/QM-XXXX.md` as the evidence that would have been the PR body — **and never state the no-PR fact as a push failure** | none — recorded in `PLAN_CHANGELOG.md` and `ORCHESTRATION_STATE.md` "Run 4" | `QM-0002` (this task) | `Decided` — see the derivation below the table |
| `DIV-012` `.plan/README.md` status vocabulary count | `.plan/README.md` table vs its own prose | The status table lists nine values; the prose below said `## Status` "always holds exactly one of the **eight** values above". The table gained `Deferred` and the sentence was never updated | Say nine. The one-value-per-line parser contract is unaffected | none | `QM-0002` (this task) | `Resolved` |
| `DIV-013` `.plan/README.md` hard-coded `Ready` set | `.plan/README.md` vs `EXECUTION_ORDER.md` §10 and the corpus | `README.md` said "At the start of the v1 plan, `QM-0100`, `QM-0001`, and `QM-0002` are `Ready`; every other task is `Blocked` or `Deferred`", citing `EXECUTION_ORDER.md` §10 as its authority — while §10's own table declares `QM-0010`, `QM-0012` and `QM-0093` **Ready**. Parsing all 90 `TASK.md` files at `4e0e85c` gave 7 `Ready`, not 3, and one `Complete`, so "every other task is `Blocked` or `Deferred`" is false as well. Re-parsed at `1d49ffa`: **44 `Deferred` / 37 `Blocked` / 7 `Complete` / 1 `Ready` (`QM-0010`) / 1 `In Progress` (`QM-0002`) = 90**, which is a *third* distribution and proves the point — the number cannot be written down | Replace the count with the deriving command. `Ready` is derived, never read from prose | none | `QM-0002` (this task) | `Resolved` |
| `DIV-014` `ARCHITECTURE.md` §17–§18 versus the strategy document | `ARCHITECTURE.md` §17–§18, `MASTER_DOCUMENT.md` §2/§20 vs the strategy document | The root documents defined the first MVP as the CesiumJS + workspace + chat platform; the strategy document defines it as a single quantization-error diagnostic and is newer. **`QM-0167` amended them** (`f132393`, merged, `Complete`). **Both sources checked, not one** — the `:80` rule below requires every one: (i) `ARCHITECTURE.md:1086` is now `## 17.1 Release history and scope`, whose first line reads *"**v1 is the quantization-error diagnostic. It is not the visualization platform.**"*, with §17.2 the v1 release and §17.3 the platform release that follows; (ii) `MASTER_DOCUMENT.md:52` is now `## 2.1 v1 — Out-of-core quantization-error diagnostic (current release)` and points at `ARCHITECTURE.md` §17.1 for the decision, `:102` is `## 2.2 Platform workflow (the release that follows v1)` sequenced after v1, `:950` is `## 20.1 v1 acceptance criteria` deferring to `.plan/DEFINITION_OF_DONE.md` `V1-01`…`V1-32`, and `:958` `## 20.2` is labelled in terms *"acceptance criteria for the platform release … not for v1"*. The row was written while that task was in flight and read `Open` | Amend the root documents so one source of truth remains — **done by `QM-0167`**. Recorded in `.plan/STRATEGY_ALIGNMENT.md` §6 | none | `QM-0167` (**Complete**) | `Resolved` — both `sources` verified against the tree at `1d49ffa`; `QM-0002` records the landed state, it did not decide it |

### `DIV-011` — why the row is `Decided` and not `Resolved`

The §3 rule below is applied mechanically rather than asserted. Every document in
the row's `sources` cell, and what each now says:

| Source | Says | Agrees? |
| --- | --- | --- |
| `.plan/README.md` | no PR is creatable (`gh` token `"push": false`); pushing over SSH succeeds; integration is a local squash merge onto local `main`, then pushed | **yes** — corrected in this commit |
| `.plan/PLAN_CHANGELOG.md` 2026-08-04 ("push to `origin` succeeds", `3394510`) | *"The push now succeeds. Merges reach `origin`."* with verbatim `f4a07ef..4e0e85c main -> main  exit 0` | **yes** |
| `.plan/ORCHESTRATION_STATE.md` "Run 4" | *"pushing over SSH as `hmthanh` and creating a PR via the `gh` token are different permissions, and only the PR half is genuinely unavailable"*; and at `:102-103`, *"SSH can push branches; pull requests cannot be created under this API identity"* | **yes** |
| `.plan/evidence/README.md:5-6` | *"Controller §1 substitutes these for the pull-request bodies `.plan/README.md` **assumes**"* | **no** — `.plan/README.md` no longer assumes a PR body; it names the substitution explicitly. The sentence is now one revision behind |

Three of four agree, so by §3's rule — *"a row moves to `Resolved` only when every
document listed under `sources` agrees"* — the row stays **`Decided`**, which is
exactly the register's *"`Decided` with a pending edit stays `Decided`"* case. The
pending edit is one clause in `.plan/evidence/README.md`, the controller's index of
the evidence directory; `QM-0002` left it rather than rewrite a controller file
whose subject is the controller's own §1, and it is recorded here as the residue
that keeps the row open. The earlier revision of this row asserted `Resolved` **on
false evidence** — it claimed the token "cannot push" — which is the defect the
independent review caught.

---

## 2. What this register deliberately does **not** contain

**Declared v1 deliverables are not divergences**, whether or not they exist yet.
Each is owned by a named task that declares it under `## Files Expected to Add`,
so a citation to one is checked for *shape*, not existence, and the register does
not track it. This list is deliberately written as ownership rather than as
presence, because the set that exists changes with every merge:

| Path | Declared by |
| --- | --- |
| `crates/q-quant/`, `crates/q-diagnostics/`, `crates/q-report/` | the Phase 11–12 engine and report tasks |
| `crates/q-daemon/` diagnostic routes | `QM-0143` |
| `schemas/diagnostics/` | `QM-0140` |
| `gpu/metal/` | `QM-0126`, the v1 GPU lane (`ADR-013`) |
| `apps/web/diagnostics/` | `QM-0150` |
| `scripts/` | `QM-0001`; `scripts/baseline.json` is `QM-0001`'s alone |
| `benchmarks/` | `QM-0102` |

Do not read a row as a claim that the path is missing. Several landed while this
register was being written and their owning tasks are now `Complete`:
`crates/q-report/` and `schemas/diagnostics/` (`QM-0140`, `f962028`) and all of
`scripts/` — `baseline.json`, `verify-baseline.sh`, `verify-baseline.test.sh`
(`QM-0001`) and `license-audit.sh` (`QM-0093`). A row is removed only when its
owning task is `Complete` *and* nothing in `.plan/` still cites the path as
planned, which is why they are still listed: `.plan/` documents written before the
merges continue to cite them under `## Files Expected to Add`.

`ARCHITECTURE.md` §16's `quatricmorph/` root and `mm/` are also not drift:
`mm/` is a read-only historical reference (`AGENTS.md`), and `quatricmorph/` is
a legacy Three.js experiment that `AGENTS.md` forbids expanding as a product
path.

---

## 3. Keeping the register honest

* Every task that resolves a row updates that row's `Status` in the same squash
  merge commit that lands the resolution.
* A row moves to `Resolved` only when every document listed under `sources`
  agrees. `Decided` with a pending edit stays `Decided`.
* A new divergence is added with its `file:line` evidence. A row without a
  citation is an opinion, not a divergence.
* `.plan/tools/check-plan-citations.py` finds *citation* drift. It cannot find
  *semantic* drift — a path that resolves but means something different. Rows
  `DIV-001`, `DIV-003`, `DIV-005`, `DIV-006` and `DIV-007` were all found by
  reading, not by the checker.
