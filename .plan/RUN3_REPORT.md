# Quatricmorph v1 — Autonomous Implementation Report (Run 3)

## Run

- **Started** 2026-08-04T23:49:41Z · **report written** T+4h40m of 5h00m
- **Terminal condition:** budget expiring. Not complete. Executable v1 work remains.
- **Base commit at start** `579107f` · **final commit** `3d99af3`
- **Merges pushed to origin: YES.** This reverses Run 2's recorded finding.
- **This was a restart**, not a fresh run. Six worktrees and a prior run's state
  were reconstructed from Git (§18) before any new work began.

## The two things that most shaped this run

**1. The repository owner re-scoped the project while it was running.** Commit
`579107f` (06:49:34) rewrote `MASTER_PLAN.md` §4 and `QM-0100`'s task file:

> "Focus on small and simple version first, please using model already download
> inside `./models/distilbert-distilgpt2`, and ignore any larger MoE checkpoints"

> "Only focus on first MVP version to development."

A second owner commit `f4a07ef` (06:51:38) landed during orientation and was
picked up automatically; verified an ancestor of `main`, so **no owner commit was
lost**. The owner also deleted the 28.63 GB Qwen1.5-MoE-A2.7B checkpoint a prior
run had downloaded.

**2. A prior run's committed `QM-0100` branch was invalid and was discarded.** It
committed `fixtures/real-checkpoint-record.json` describing the deleted 28.63 GB
checkpoint, and — based at `0ef6ec5` — squash-merging it would have **reverted the
owner's own amendment**. It was the only one of six in-flight branches touching an
owner-amended path. Branch deleted, task rewritten, re-implemented from scratch.

## Plan completion

**46 v1 tasks** of 90 total (44 `Deferred`, untouched by design).

**Parsed `## Status` values across all 90 task files — these four sum to 90:**

| `## Status` | Count | Notes |
| --- | --- | --- |
| **Complete** | **5** | `QM-0006` (prior run) · **`QM-0012`, `QM-0093`, `QM-0140`, `QM-0167`** (this run) |
| **Blocked** | **37** | Includes `QM-0160`, whose scaffolding **was merged** (`e61d28e`) but which correctly stays `Blocked` on a human. Also `QM-0161`…`QM-0164` (human) and `QM-0020`/`QM-0033`/`QM-0126` (blocked by *dependency*, not by an undecided ADR — see below) |
| **Ready** | **4** | `QM-0001`, `QM-0002`, `QM-0100`, `QM-0010` |
| **Deferred** | **44** | Untouched by design |

**Work in flight at cutoff, cross-cut against the table above** (these are the
three `Ready` tasks that have real work on a branch; they are *not* additional
tasks, and are counted once, above):

| Task | Branch head | State at cutoff |
| --- | --- | --- |
| `QM-0001` | `ee30636` | Implemented + evidence complete; **independent review still running** when the merge window closed |
| `QM-0100` | `94bc274` | Implemented + evidence complete; **independent review still running** when the merge window closed |
| `QM-0002` | `6e99e62` | Implementation **finished after the merge cutoff**; **never independently reviewed**, so it could not merge regardless |

`QM-0010` is `Ready` with no work started.

## Gates

| Gate | Status |
| --- | --- |
| **G1** bounded residency on a real checkpoint | **Partially, and re-scoped.** `QM-0100` measured peak RSS 3,473,408 B on the release binary → `C = 2,778,726 B` → **N = 126.97**, clearing N ≥ 100. Residency flat across a 3,265× size span. **But this is a 337 MiB file, not ≥ 24 GB, and an indexing pass, not streaming.** Unmerged at cutoff, pending review. |
| **G2** metric vs Python reference | **Not reached.** Lane Q (`QM-0120`…`QM-0122`) never opened — it is gated behind `QM-0101`, which is gated behind `QM-0100`. |
| **G3** report byte-determinism | **Not reached** (`QM-0141`). `QM-0140` merged the schema and a byte-identity round-trip test beneath it. |
| **G4** surface legibility | **Not reached** (`QM-0151`). |
| **G5** documented decision change | **HUMAN-DEPENDENT — cannot be satisfied by this or any agent run.** |

**Because G5 gates the v1 release and G5 needs a person: v1 is NOT released.**

## Merges

| Task | Lane | Reviewed SHA | Reviewer | Verdict | Merge | On origin |
| --- | --- | --- | --- | --- | --- | --- |
| QM-0160 | V | `16ad32b` | review-agent-2 | APPROVED (scaffolding only) | `e61d28e` | yes |
| QM-0012 | T | `6f2d5eb` | review-agent-1 | APPROVED | `4e0e85c` | yes |
| ADR-011/012/013 | — | — | controller work (§0.3) | — | `37d7231` | yes |
| QM-0093 | — | `6d04e00` | review-agent-3 | APPROVED | `7ec7758` | yes |
| QM-0140 | R | `04ffffc` | review-agent-4 | APPROVED | `f962028` | yes |
| QM-0167 | — | `22260e7` | review-agent-5 | CHANGES_REQUESTED → resolved | `f132393` | yes |

Every merge commit confirmed reachable from `main` via `git merge-base --is-ancestor`.

## Testing

- **Baseline re-measured at start**, on `579107f`: **rust 290 passed / 0 failed**,
  **web 115 passed / 13 files**.
- **Discrepancy vs `STATUS.md`'s 290/101 at `5ca434d`:** rust matches exactly;
  **web is 115, not 101 — stale by 14.** Recorded in `PLAN_CHANGELOG.md`.
- **Final counts on `main` @ `3d99af3`:**

```
$ cargo fmt --all -- --check                              exit 0
$ cargo clippy --workspace --all-targets -- -D warnings   exit 0
$ cargo test --workspace          415 passed; 0 failed    exit 0
$ cd apps/web && npx vitest run   115 passed (13 files)   exit 0
```

- **Tests added by this run: +125 rust** (290 → 415). QM-0012 +28, QM-0140 +97.
  A further +19 (QM-0100) is implemented but unmerged.
- **Failing-first** demonstrated for: `QM-0001` (twice), `QM-0100` (twice).
  **Explicitly NOT achieved** for `QM-0140` — the implementation arrived
  pre-written; it substituted 14 guard-removal demonstrations, all red, and the
  reviewer independently re-ran two of them.
- **Exempt classes, each recorded by its reviewer:** `QM-0093` and `QM-0167`
  (documentation-only), `QM-0160` (human-dependent scaffolding).

### The floor guard did not land — state this plainly

**`scripts/baseline.json` does not exist on `main`.** `QM-0001` creates it; the
task is implemented, evidenced, and in review, but was not merged before the
window closed. **Every merge in this run therefore landed with no floor guard in
force**, and each merge commit says so. Counts were recorded in evidence and
re-measured on merged `main` after every merge instead — 318 after QM-0012, 415
after QM-0140, both confirmed by direct measurement.

## Architecture decisions

**Three ADR candidates promoted** (§0.3 authority, no human input):

- **ADR-011** content-derived identifiers (from candidate 018) — recommended
  default adopted unchanged, restated at byte level.
- **ADR-012** job progress over SSE (from candidate 011) — recommended default
  adopted, plus a no-replay binding. Verified locally rather than trusted:
  `cargo tree -p q-daemon -e features -i axum` → axum 0.7.9 with tokio already on.
- **ADR-013** Metal is the v1 GPU lane (from candidate 003) — adopted the file's
  **revised** decision, **not** its superseded recommended default. Research
  changed the binding: `metal-rs` is **deprecated**, and `metal` 0.33.0 needs MSRV
  1.82 against the workspace's 1.78, while `objc2-metal` 0.3.2 needs 1.71.

**A correction to the controller's own assumption**, found by the promoting agent:
none of `QM-0020`/`QM-0033`/`QM-0126` carries an `ADR-CANDIDATE (decision
required)` edge, so these promotions retire **decision risk** but do **not** remove
dependency edges.

## What this run does NOT establish

- **No design partner has been contacted, run the tool, changed a decision, quoted
  a price, or used it repeatedly.** `QM-0160` is scaffolding only — both CSVs
  contain schema headers and **zero data rows**, verified line by line by an
  independent reviewer. `QM-0161`…`QM-0164` were never started.
- **G5 is unsatisfied and v1 is not released.**
- **No GPU executed anything.** ADR-013 is a build decision, not a hardware result.
  The CUDA lane stays `Deferred`; `crates/q-cuda` compiles no kernels.
- **No deployment. No network transport. No CI run observed** — `QM-0093` wired a
  CI job and says so; it does not claim the job passed.
- **The checkpoint concession gives up real coverage**, recorded rather than
  quietly dropped: the **sharded read path** is not exercised on real data
  (single file, no index JSON); **bf16 exact decode** is not exercised (F32, all 82
  tensors); **MoE expert-keyed aggregation** has no real-checkpoint fixture (no
  experts); **≥ 24 GB scale is not established** (352,824,413 bytes ≈ 337 MiB).
  `V1-01` as written is **not satisfied**.
- **`peak_resident_bytes` in the manifest fixtures is a placeholder** — the
  residency claim is *carried* by the artifact, not *evidenced* by it.
- **No manifest has been produced by a real engine** — `QM-0123` does not exist.

## Two findings that outlive this run

1. **A proprietary font ships in the built product.**
   `apps/web/quatricmorph-workspace/src/assets/droid_sans_regular.typeface.js`
   carries an **Ascender Corporation EULA** — *"you may not copy this font
   software"* — not the Apache-2.0 usually assumed for Droid Sans. The reviewer
   confirmed **empirically**: `vite build` emits a 969 kB bundle containing
   `ascendercorp.com/eula10.html`. `NOTICE` §2.1 records the conflict and
   correctly asserts neither licence. **Needs owner/legal follow-up.**

2. **`docs/requirements/PREREQUISITES.md` tells an autonomous agent it may start
   Phase 0** — which is `Deferred` — while `AGENTS.md` rule 1 points at that file
   as the gate checklist. Six files still present Phase 0 as active and **no task
   owns any of them.** This is a live hazard for exactly this kind of unattended
   run. Recorded as needing a new task.

## Controller errors, recorded rather than smoothed over

- **I over-parallelized.** Seven concurrent agents, five running workspace builds
  in separate target dirs, drove load to **102 on 11 cores at 50 % system time**.
  The cap I was honoring was disk-bound; the binding constraint was CPU. Corrected
  mid-run to three Rust-building agents.
- **I briefed an agent with the wrong ADR-010 layer.** I said `inspect` should
  refuse rank-4 tensors. ADR-010 puts the refusal at `bindAxes()`/`GRID-007` and
  calls the metadata layer **rank-agnostic**. Corrected mid-flight; the agent then
  found `bindAxes()` does not exist in the tree at all (owned by `QM-0061`) and
  correctly logged rather than implemented out of scope.

## Final state

- **All executable v1 tasks merged and complete: NO.**
- **Final clean-checkout verification passed: YES** — fmt, clippy, 415 rust, 115
  web, all exit 0 on `main` @ `3d99af3`.
- **Unresolved blockers:** G5 needs a human; `QM-0001` and `QM-0100` reviewed-but-
  unmerged; `QM-0002` mid-implementation; the floor guard is not yet in force.

**Next run should start with:**

1. `QM-0001` — branch `task/qm-0001-baseline-verification` head `ee30636`,
   review in flight. **Merge first**, and write the *measured* post-merge floor
   (415 rust / 115 web), not the 290 its worktree recorded.
   It also collides with `QM-0093` on `.github/workflows/build.yaml` — re-run
   `scripts/license-audit.sh` and the full gates after merging.
2. `QM-0100` — branch `task/qm-0100-real-checkpoint-verification` head `94bc274`,
   review in flight. Merging it opens Lane P (`QM-0030` → the critical path).
   Its worktree holds a **337 MiB copy** of the checkpoint — delete it.
3. `QM-0002` — branch `task/qm-0002-plan-repo-reconciliation` head `6e99e62`.
   **Implementation finished after the merge cutoff and was never independently
   reviewed — review it before merging.** It reports 17 files changed, all under
   `.plan/`, gates green (rust 318 / web 115 at its base), and the only expected
   conflict is an append/append in `PLAN_CHANGELOG.md`.
   **It caught a genuine near-miss worth carrying forward:** the inherited
   uncommitted WIP had run `npm` at a base predating QM-0006's rename, which
   "reconciled" `apps/web/package-lock.json` *backwards* and deleted 151 lines —
   committing it **would have reverted the lockfile half of `1cfdc9c`**. It
   reverted that, and also reverted an edit to `QM-0167/TASK.md` that would have
   collided with the agent editing it concurrently.
   It also states plainly that **its own AC1 is not met**: the criterion names
   `scripts/check-plan-citations.sh`, but the task's `## Program Boundary` restricts
   it to `.plan/` and `QM-0001` owns `scripts/` — a self-contradiction in the task
   spec, recorded rather than resolved by widening its own scope. Its checker still
   exits 1 with 8 unresolved citations, all owned by other tasks.
4. Create the new task for the six unowned Phase-0 stale docs, and schedule it
   early — it protects every later agent run.
