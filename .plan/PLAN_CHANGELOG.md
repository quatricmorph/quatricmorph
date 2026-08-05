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

---

# Run 3 — 2026-08-04T23:49:41Z → deadline 2026-08-05T04:49:41Z

## 2026-08-04 — QM-0100 — owner redirected the checkpoint to `models/distilbert-distilgpt2`; three constraint rows are now stale

**Discovered during:** Run 3 restart reconstruction (§18), reading `579107f`.
**Defect:** The repository owner amended `.plan/MASTER_PLAN.md` §4 and
`.plan/tasks/QM-0100-real-checkpoint-acquisition/TASK.md` in commit `579107f`
with an explicit, twice-stated directive:

> "Focus on small and simple version first, please using model already download
> inside `./models/distilbert-distilgpt2`, and ignore any larger MoE checkpoints"

> "Only using model inside `distilbert-distilgpt2` instead of using large MoE
> checkpoints is a **temporary** concession to the machine's disk. Only focus on
> first MVP version to development."

The owner also **deleted the 28.63 GB Qwen1.5-MoE-A2.7B checkpoint from disk**
(verified: `models/` is 339 MB and holds only `distilbert-distilgpt2`; free disk
is 21 GB, not the 51 GB the plan assumes) and removed the download's background-job
record from `.plan/ORCHESTRATION_STATE.md`.

The prose directive is authoritative. But the owner left three rows of QM-0100's
own constraint table describing the checkpoint that was just abandoned, so the
task now contradicts itself:

| Stale row | Says | Reality of `models/distilbert-distilgpt2` |
| --- | --- | --- |
| Size on disk | "**≥ 24 GB**, using `models/distilbert-distilgpt2/model.safetensors`" | 336 MB — self-contradictory within a single cell |
| Format | "SafeTensors, **sharded**, with `model.safetensors.index.json`" | Single file, **no index.json**, shard count 1 |
| Architecture | "Qwen- or Llama-family, ideally with MoE experts" | GPT-2 family, **no experts** |

**Correction:** The prose wins over the stale rows (owner's explicit intent,
stated twice, and backed by the deletion of the artifact). QM-0100's constraint
table and `## Test Cases` are rewritten to describe the distilgpt2 reality before
any agent implements against it.

**Coverage this concession gives up — recorded, not silently dropped:**

* **The sharded read path is no longer exercised by QM-0100.** The task's own
  stated reason for requiring a sharded checkpoint was that "single-file would
  not" exercise it. Multi-shard attribution remains covered only by the synthetic
  fixtures, and v1 must not claim otherwise.
* **MoE expert-keyed aggregation has no real-checkpoint fixture.** `QM-0123`
  consumes expert-keyed tensors; under this concession it is provable only against
  generated fixtures.
* **Gate G1's ratio changes shape.** `MASTER_PLAN.md` §4 still requires peak RSS
  ≤ 1.25 × C while streaming a checkpoint N ≥ 100 × larger than C. Against a
  336 MB file, N ≥ 100 forces C ≤ ~3.4 MB. The structural property survives and is
  still measurable with `/usr/bin/time -l`; the headline number does not. v1 states
  the ratio it actually measured, against the file it actually measured.

**Files changed:** `.plan/tasks/QM-0100-real-checkpoint-acquisition/TASK.md`
**Dependency impact:** QM-0100 stays Lane P / Wave 0 and still gates QM-0101.
Its previously committed branch is invalidated (see next entry).
**Evidence:** `git show 579107f`; `du -sh models/` → 339M; `df -h .` → 21Gi avail;
`find ~/Quatricmorph -name '*.safetensors' -size +100M` returns only distilgpt2.

## 2026-08-04 — QM-0100 — the committed branch is invalid and is discarded

**Discovered during:** Run 3 restart reconstruction (§18).
**Defect:** `task/qm-0100-real-checkpoint-acquisition` @ `c5743cd` commits
`fixtures/real-checkpoint-record.json` recording `Qwen1.5-MoE-A2.7B`,
`bytes_on_disk: 28632144944`, `shard_count: 8`, `tensor_count: 4659`,
`architecture: qwen2_moe`, `has_experts: true` — **a checkpoint that is no longer
on disk** — plus a 358-line test asserting against it. The branch is also based at
`0ef6ec5`, before the owner's amendment, so squash-merging it would **revert
`579107f`'s edit to QM-0100's TASK.md**. It is the only one of the six in-flight
branches that touches an owner-amended path.

**Correction:** The branch and its worktree are discarded, not merged. QM-0100 is
re-implemented from a worktree cut at `579107f` against distilgpt2.
**Dependency impact:** QM-0101 stays Blocked until the re-implementation merges.
**Evidence:** `git diff main...task/qm-0100-real-checkpoint-acquisition -- .plan/tasks/QM-0100-real-checkpoint-acquisition/TASK.md`
shows the `In Progress` → reverted-to-`Ready` conflict; `cat fixtures/real-checkpoint-record.json`
on that branch names the deleted checkpoint.

## 2026-08-04 — CONTROLLER — Run 3 baseline re-measured; web floor is 115, not STATUS.md's 101

**Discovered during:** §21 step 4, baseline gate on `main` @ `579107f`.
**Defect:** `STATUS.md` records 290 rust / 101 web at `5ca434d`. Rust still
measures **290 passed; 0 failed** exactly. Web measures **115 passed across 13
files**, +14 over the recorded 101.
**Correction:** 290 / **115** is the Run 3 baseline and the floor QM-0001 must
write. `STATUS.md`'s 101 is stale and is corrected by QM-0091's regeneration.
**Dependency impact:** none; QM-0001's floor value changes from 101 to 115.
**Evidence:**
```
$ cargo test --workspace          → sum of "test result: ok." = 290 passed, 0 FAILED, exit 0
$ cd apps/web && npx vitest run    → Test Files 13 passed (13) / Tests 115 passed (115)
```

## 2026-08-04 — CONTROLLER — `scripts/baseline.json` does not exist; no floor guard is in force

**Discovered during:** §18 reconstruction.
**Defect:** Run 2's orchestration record states "QM-0001 is writing
`scripts/baseline.json` as {rust: 290, web: 115}" and builds a floor-staleness
rule on top of that. In fact `task/qm-0001-baseline-verification` is at `145257b`,
an **ancestor of `main`, with zero commits ahead and a clean worktree** — the work
was never committed. `scripts/` does not exist on `main`.
**Correction:** QM-0001 is re-cut from `579107f` and scheduled **first**, because
until it merges there is no floor guard at all. Every task merged before it records
"no floor guard in force at merge time; counts recorded in evidence only" in its
evidence record, exactly as §6.1 item 6 requires.
**Dependency impact:** QM-0001 is promoted ahead of the other Wave 0 tasks.
**Evidence:** `git log main..task/qm-0001-baseline-verification` is empty;
`ls scripts` → "No such file or directory".

## 2026-08-04 — CONTROLLER — Run 2's checkpoint/disk facts are superseded; the prompt's 51 GB is stale

**Discovered during:** §21 steps 1–2.
**Defect:** The controller prompt states ~51 GB free disk and caps the headline
checkpoint at 30–40 GB. Measured at Run 3 start: **21 GB free, 96% capacity**, with
14 GB of that already consumed by six worktree `target/` directories.
**Correction:** The §3.3 disk budget binds harder than the prompt assumed. Concurrent
Rust-building worktrees are capped at **three**, `df -h .` runs before every worktree
creation, and completed worktrees are `cargo clean`ed and removed at merge rather
than at end of run. The distilgpt2 concession removes the checkpoint from the budget
entirely but does **not** create slack for unbounded concurrency.
**Evidence:** `df -h .` → `21Gi Avail, 96% Capacity`; `du -sh ../.qm-worktrees` → 14G.

## 2026-08-04 — CONTROLLER — the checkpoint-size waiver was self-contradictory and is superseded

**Discovered during:** `QM-0160` independent review; the reviewer declined to
"fix" it because every available patch left the sentence still wrong.
**Defect:** `.plan/DEFINITION_OF_DONE.md` §"Waiver — checkpoint size" read *"The
development machine has 21 GB of free disk. v1's headline checkpoint is therefore
capped at roughly 30–40 GB."* **30–40 GB does not fit in 21 GB.** The owner's
commit `f4a07ef` substituted `51 GB` → `21 GB` mechanically across seven files and
left the derived figure behind. The reviewer's judgement was correct: a remedy that
leaves the sentence incoherent is inherited staleness, not a fix.
**Correction:** The waiver is **superseded**, not patched, because commit
`579107f` removed the large-checkpoint requirement entirely. The rewritten waiver
states what v1 claims (bounded residency on the 352,824,413-byte distilgpt2, a
measured 0.00235 % bytes-read ratio) and enumerates what it explicitly does **not**
claim: ≥ 24 GB streamed, 1.5 TB streamed, the sharded path exercised on real data,
bf16 exact decode exercised, MoE expert aggregation fixtured, or `N ≥ 100` unless
`QM-0101` measures it. Criteria rows `V1-01`…`V1-05` are updated to match.
**Also recorded:** free disk has since measured **54 GB** (APFS reclaimed the
deleted checkpoint's purgeable space), so disk is no longer the binding constraint
— **but the owner's directive is not contingent on disk** and must not be reverted
on the grounds that a large checkpoint would now fit.
**Files changed:** `.plan/DEFINITION_OF_DONE.md`
**Dependency impact:** `QM-0100` and `QM-0101` acceptance evidence changes shape;
`V1-04`'s `N ≥ 100` is marked **at risk** rather than assumed.
**Evidence:** `git show f4a07ef --stat` (7 files, mechanical 51→21 substitution);
`.plan/DEFINITION_OF_DONE.md:39` before the edit; `stat -f%z
models/distilbert-distilgpt2/model.safetensors` → `352824413`; `df -h .` → 54Gi.

## 2026-08-04 — QM-0160 — the task has no `## Files Expected to Change` section

**Discovered during:** `QM-0160` independent review.
**Defect:** `.plan/tasks/QM-0160-design-partner-outreach/TASK.md` omits the
`## Files Expected to Change` section that every other task carries and that the
reviewer's scope check depends on. The reviewer verified the 14 committed paths
against `## Program Boundary` instead and found them all conformant, so the merge
was sound — but the scope gate had to be improvised.
**Correction:** Recorded. The section should be added by `QM-0002`, which owns
plan-corpus reconciliation. Not fixed inline, because `QM-0160` was mid-merge and
`QM-0002` is being edited concurrently in another worktree.
**Files changed:** none yet — routed to `QM-0002`.
**Dependency impact:** none.
**Evidence:** the reviewer's scope ruling in `.plan/evidence/QM-0160.md`.

## 2026-08-04 — QM-0012 — two defects in the task file, found by the reviewer, not by the implementer

**Discovered during:** `QM-0012` independent review.
**Defect 1:** `## Data Contracts` states `"parameter_count": 299184`. The correct
value is **302256**. `299184` is `total_bytes / 4` — an F32-uniform assumption that
the fixture's two BF16 tensors break. The reviewer re-derived 302256 independently
by parsing the raw SafeTensors headers with plain `json`+`struct`, using no
repository code. The normative `## Acceptance Criteria` item 2 and `## Scope` both
give 302256, so the plan file states a number its own test suite disproves.
**Defect 2:** `## Repository Evidence` describes `config.json` as being read by
`q-architecture`. At the branch's base commit it was read by
`crates/q-source/src/local.rs`. The claim became true only *after* this branch.
**Correction:** Both recorded here and routed to `QM-0002`. Not fixed inline —
`QM-0012` was already reviewed and merging, and amending the reviewed diff would
have invalidated the review for a non-blocking documentation defect.
**Files changed:** none yet — routed to `QM-0002`.
**Dependency impact:** none. `QM-0012` merged as `4e0e85c` and is `Complete`.
**Evidence:** `.plan/evidence/QM-0012.md` §Independent review, "Controller actions
needed" items 1 and 2.

## 2026-08-04 — CONTROLLER — push to `origin` succeeds; Run 2's credential finding is superseded

**Discovered during:** the first Run 3 merge.
**Defect:** Run 2 recorded, with verbatim output, that `git push origin main` was
impossible (SSH stalled at exit 124; HTTPS returned `403 Permission ... denied to
MarkdownOfficial`) and declared local `main` the integration branch. The controller
prompt §1 encodes the same expectation.
**Correction:** The push now succeeds. Merges reach `origin`.
```
$ git push origin main
To github.com:quatricmorph/quatricmorph.git
   f4a07ef..4e0e85c  main -> main            exit 0
```
**Consequence adopted:** because the owner is committing to this repository
**during the run** (`579107f` at 06:49:34, `f4a07ef` at 06:51:38), the merge
sequence now begins with `git pull --ff-only origin main` on **every** merge, not
once per wave. A merge that clobbers an owner commit is worse than a slow run.
`f4a07ef` was verified to be a proper ancestor of `main` (`git merge-base
--is-ancestor f4a07ef main` → exit 0); no owner commit was lost.
**Files changed:** `.plan/ORCHESTRATION_STATE.md`
**Dependency impact:** none.
**Evidence:** the push transcript above; `git log 579107f..f4a07ef --stat`.

## 2026-08-04 — QM-0093 — scheduled outside its wave, by lane and dependencies; and a real merge-order collision

**Discovered during:** `QM-0093` implementation; the agent routed three decisions
to the controller rather than guessing. All three are answered here.

**1. Wave and lane.** `.plan/EXECUTION_ORDER.md` §2 places `QM-0093` in **Wave 6**;
§4 assigns it **no lane at all**. The agent correctly wrote "not assigned" rather
than inventing one.
**Decision:** scheduling it in Wave 0/1 is correct and stays. §10's rewiring table
gives `QM-0093` the v1 edge **"none — Ready"** ("a licence audit needs no
pipeline"), so it has no dependencies to wait on, and the controller's §10 makes
the wave table a *sequencing aid, not an allowlist*. A v1 task with satisfied
dependencies is scheduled by lane and dependencies, never stranded for lack of a
wave. The lane gap is a real plan defect and is recorded here for `QM-0002`.

**2. Merge-order collision — genuine, and acted on.** `QM-0093` edits
`.github/workflows/build.yaml` (adding a `licenses` job). `QM-0001` is expected to
edit the same file. `.plan/ORCHESTRATION_STATE.md` had recorded the two as
colliding over `scripts/` — that part is benign (different filenames:
`license-audit.sh` vs `verify-baseline.sh`). **The CI workflow file is the real
collision.**
**Decision:** the two merges are **serialised**, and whichever lands second
re-runs `scripts/license-audit.sh` **and** the full gate set on the merged `main`
before its own merge is recorded as verified. This is the controller's job under
§14, not either agent's.

**3. Fail-threshold divergence inside the task itself.** `## Error Handling` says
**any copyleft** fails the audit; acceptance criterion **6** says
**GPL/AGPL/SSPL**. These are not the same rule. The agent implemented the
criterion, downgraded MPL-2.0 to a reported-not-failed item, and documented the
divergence in both the script header and the evidence record. Affected: twelve
`lightningcss` MPL-2.0 **dev-only** binaries, and `r-efi`, which offers a
permissive alternative under an `OR`.
**Decision:** routed to the independent reviewer to rule on with reasoning, since
it is a correctness question about which text is normative, not a taste question.
The divergence between the two sections is itself a plan defect and is recorded
here regardless of the ruling.

**Files changed:** none — findings routed to `QM-0002` and to the reviewer.
**Dependency impact:** `QM-0001` and `QM-0093` merges are now explicitly ordered.
**Evidence:** `.plan/EXECUTION_ORDER.md` §2 (Wave 6) and §4 (no lane) vs §10
("`QM-0093` licensing · `QM-0080` · none — **Ready**"); the task file's
`## Error Handling` vs `## Acceptance Criteria` item 6.
## 2026-08-04 — ADR-011 — `ADR-CANDIDATE-018` promoted; the ID construction is now fixed at the byte level

**Discovered during:** ADR promotion pass (controller §0.3), highest-priority
candidate — `QM-0012` merged at `4e0e85c`, leaving `QM-0020` otherwise schedulable.
**Defect:** `QM-0020`'s `## Scope` and `## Implementation Plan` commit to
`StatisticsId = blake3(len‖subject_id ‖ len‖algorithm_version)` while citing an
**unpromoted** candidate. Worse, that shorthand is lossy against the code it
claims to follow: `q_source::ids::digest16` (`crates/q-source/src/ids.rs:81`)
prefixes `ID_SCHEME_VERSION` and a per-kind domain string before any component,
and `q_tensor_runtime::TileId::for_block` and `q_cache::CacheKey::digest` append
fixed-width fields (`lod`, `algorithm_version`, `extent`) **unprefixed**. An
implementer transcribing the shorthand literally would have frozen a fourth,
incompatible construction into persisted `tensor_statistics` rows.
**Correction:** Promoted to `docs/decisions/ADR-011-content-derived-identifiers.md`,
**Accepted**, adopting the candidate's recommended default (option A) unchanged.
The ADR states the construction at the byte level — variable-length components
length-prefixed `u64` LE, fixed-width components appended LE unprefixed, digest
truncated to 16 bytes — and binds the two new domains
(`quatricmorph/statistics/v1`, `quatricmorph/job/v1`), the `JobId` timestamp
component (the existing `ConversionJob::created_at`), the rule that a resumed job
**keeps** its persisted `JobId`, and bare 32-hex as the persisted form where
`API_CONTRACTS.md` §3 prose shows a `job:` display prefix.
**Files changed:** `docs/decisions/ADR-011-content-derived-identifiers.md` (added),
`.plan/decisions/ADR-CANDIDATE-018-tensor-id.md`, `.plan/decisions/README.md`
**Dependency impact:** None to any `## Dependencies` section. `QM-0020` lists
`QM-0012` only and cites 018 under `Scope`, not `Dependencies`; the same holds for
`QM-0021`, `QM-0022`, and `QM-0033`. This retires the decision risk those tasks
carried, not a dependency edge. **No `TASK.md` was edited.**
**Evidence:** `cargo test --workspace` → 39 `test result: ok` lines summing to
**318 passed / 0 failed**, exit 0. `cd apps/web && npx vitest run` → **13 files /
115 passed**, exit 0. Both equal the `main` baseline; these are documentation-only
changes.

## 2026-08-04 — ADR-012 — `ADR-CANDIDATE-011` promoted; SSE accepted, and event replay is explicitly refused

**Discovered during:** ADR promotion pass (controller §0.3).
**Defect:** `.plan/API_CONTRACTS.md` §0 already states the transport decision as
settled — *"HTTP for request/response, Server-Sent Events for progress
(`ADR-CANDIDATE-011`)"* — and §1 lists four job routes as `new`, while the
candidate itself read `Open`. A frozen route table resting on an unpromoted
candidate is the condition `.plan/decisions/README.md` §"How a deadline is
derived" exists to catch.
**Correction:** Promoted to
`docs/decisions/ADR-012-job-progress-over-server-sent-events.md`, **Accepted**,
adopting the candidate's recommended default (option A, HTTP + SSE) unchanged.
The ADR adds two bindings the candidate left open, both of which an implementer
would otherwise have had to guess: the daemon buffers **no** events and
implements **no** `Last-Event-ID` replay, because `EventSource` reconnects
automatically and a reconnecting client re-reads `GET /v1/jobs/{jobId}`; and the
job record, not the stream, is authoritative state.
**Files changed:** `docs/decisions/ADR-012-job-progress-over-server-sent-events.md`
(added), `.plan/decisions/ADR-CANDIDATE-011-daemon-transport.md`,
`.plan/decisions/README.md`
**Dependency impact:** None. `QM-0033`'s `## Dependencies` names `QM-0032` and
`QM-0022`; it cites 011 under `Repository Evidence` only. `QM-0033` remains gated
on `QM-0032`. **No `TASK.md` was edited.**
**Evidence:** `cargo tree -p q-daemon -e features -i axum` → `axum v0.7.9` with
feature `tokio` already enabled via `default`, which is the feature
`axum::response::sse` is gated behind — so SSE needs **no manifest change**,
verified on this machine rather than taken from documentation. Gates:
rust **318 passed / 0 failed** exit 0; web **13 files / 115 passed** exit 0.

## 2026-08-04 — ADR-013 — `ADR-CANDIDATE-003` promoted on its *revised* decision; the original option A is superseded

**Discovered during:** ADR promotion pass (controller §0.3).
**Defect:** Candidate 003 carries two decisions in one file. Its **original**
recommended default is option A — extension point only, CUDA-first — and its
`## Status` supersedes that with a revised v1 decision (v1 ships CPU + Metal;
CUDA deferred post-v1). `ARCHITECTURE.md` §12.3, `.plan/CUDA_ARCHITECTURE.md`
§12, and `.plan/PRODUCT_SCOPE.md` §2 have **already been rewritten** around the
revised decision, so the repository was operating on a decision that had no
accepted ADR. Separately, the build question — which binding, which feature
shape, how shaders compile — was never settled anywhere.
**Correction:** Promoted to
`docs/decisions/ADR-013-metal-is-v1-gpu-compute-lane.md`, **Accepted**, adopting
the **revised v1 decision**. The superseded original (option A) is recorded as a
rejected alternative with the reason its premise collapsed: it rested on `MVP-10`
naming CUDA, and CUDA left v1 scope. The ADR settles the build shape:
`objc2-metal` as the binding, a `metal` feature with `default = []`,
`optional = true` dependencies, and a `cfg`-guarded `build.rs` compiling
`gpu/metal/*.metal` at build time. No `Departs from:` line — §12.3 already states
the decision, so this ADR ratifies rather than overrides.
**Files changed:** `docs/decisions/ADR-013-metal-is-v1-gpu-compute-lane.md`
(added), `.plan/decisions/ADR-CANDIDATE-003-metal-build.md`,
`.plan/decisions/README.md`
**Dependency impact:** `QM-0126`'s `## Dependencies` reads `QM-0121`,
`ADR-CANDIDATE-003 (Decided)` — not `(decision required)`, which per
`.plan/README.md` is the only form that holds a task at `Blocked` for a decision.
This retires the ADR-decision edge and settles the build shape; **`QM-0126`
remains gated on `QM-0121`.** **No `TASK.md` was edited.**
**Evidence:** External, cited in the ADR with retrieval dates: `metal` (metal-rs)
0.33.0 declares MSRV **1.82** against this workspace's `rust-version = "1.78"`
(`Cargo.toml:26`), and its README declares the crate **deprecated** in favour of
`objc2`/`objc2-metal`; `objc2-metal` 0.3.2 declares MSRV **1.71**. Local gates:
rust **318 passed / 0 failed** exit 0; web **13 files / 115 passed** exit 0.
**No Metal device has executed anything in this repository** — this is a build and
layout decision, and `MetalBackend::capabilities().verified` ships `false` until
`QM-0127`.

## 2026-08-04 — QM-0002 — `QM-0090` cites `ADR-CANDIDATE-014`, which was promoted to `ADR-009`

**Discovered during:** ADR promotion pass (controller §0.3), while auditing which
tasks cite unpromoted candidates.
**Defect:** `.plan/tasks/QM-0090-documentation-update/TASK.md` cites
`ADR-CANDIDATE-014`. That candidate was promoted to
`docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md`, and
`.plan/decisions/README.md` has recorded it as `Promoted → ADR-009` since. The
citation points at a staging document when an accepted ADR exists, which is
exactly backwards for a task whose job is correcting `ARCHITECTURE.md` §8.2 —
`ADR-009` is the authority for that edit.
**Correction:** **None applied here.** This is a stale citation, not a blocker:
`QM-0090` is not held at `Blocked` by it, because `ADR-CANDIDATE-014` is promoted.
Routed to **`QM-0002`**, which owns plan-citation reconciliation and is in flight
in another worktree. Controller §11 forbids this pass from editing a `TASK.md`
another agent holds, and `QM-0090`'s file was **not** edited.
**Files changed:** none — `.plan/PLAN_CHANGELOG.md` (this entry) only.
**Dependency impact:** None. `QM-0090` is unaffected in state; only its citation
text is stale.
**Evidence:** `.plan/decisions/ADR-CANDIDATE-014-model-layout-planes.md`
`## Status` → ```Promoted → ADR-009``` (2026-08-04);
`docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md` exists and is
`Accepted`.

## 2026-08-04 — ADR-CANDIDATE-013 — stale web test count (101 vs 115) and a deadline naming a `Deferred` task

**Discovered during:** ADR promotion pass (controller §0.3), while deciding which
candidates were worth promoting.
**Defect:** `.plan/decisions/ADR-CANDIDATE-013-browser-test-strategy.md` argues
its recommended default from **"101 tests today"**. 101 is the pre-`QM-0006`
figure: before the `matrix-workspace` → `quatricmorph-workspace` directory rename,
`vitest.config.ts` matched only 3 of 12 test files, and the 101 was reconstructed
as `27 collected + 74 uncollected` (see the `QM-0006` entry above). Since
`QM-0006` merged, vitest collects everything and the real count is **115 across 13
files**. An argument resting on a test-count magnitude should rest on the count
that exists.
**Correction:** **None applied, and the candidate was deliberately not promoted.**
Nothing schedulable is held by it, so promoting it would spend an ADR number on a
decision whose supporting evidence needs re-checking first. Recorded so that
whoever promotes it re-derives the argument against 115 rather than inheriting
101.

**Second defect, found while checking the first — its stated deadline is
unreachable.** The candidate's `## Decision deadline` reads *"Before `QM-0050`,
the earliest task in `Tasks affected`"*, and `.plan/decisions/README.md` repeats
it. `QM-0050` is now **`Deferred`** (post-v1 platform release), as are four of
the other six tasks in its `Tasks affected` list (`QM-0051`, `QM-0052`,
`QM-0053`, `QM-0080`). The two that remain — `QM-0082` and `QM-0085`, not
`QM-0082` alone — are `Blocked` behind `QM-0152`. Per
`.plan/decisions/README.md` §"How a deadline is derived", a deadline is derived
mechanically from the `Tasks affected` list, so a list whose earliest entry is
`Deferred` yields a deadline that can never arrive. Whoever promotes this
candidate must re-derive the deadline from the v1-live subset (earliest:
`QM-0082`) at the same time as re-deriving the count.
**Files changed:** none — `.plan/PLAN_CHANGELOG.md` (this entry) only.
**Dependency impact:** None. `QM-0082` and `QM-0085` stay `Blocked` behind
`QM-0152`, unchanged by this pass. Neither names
`ADR-CANDIDATE-013 (decision required)` in `## Dependencies`.
**Evidence:** `cd apps/web && npx vitest run` on this machine at
`9a5398d` → `Test Files 13 passed (13)` / `Tests 115 passed (115)`, exit 0.
The candidate says "101 web tests run in vitest today" (`:9`), "12 test files, 101
tests" (`:15`), and "101 tests today" (`:69`) — three places, all stale; the file
count is also 12 against 13 on disk. `Tasks affected` (`:89`) lists seven tasks;
`QM-0050`–`QM-0053` and `QM-0080` are `Deferred`, `QM-0082` and `QM-0085` are
`Blocked` behind `QM-0152`.

## 2026-08-04 — QM-0167 — precedence rank 1 is an ADR carrying `Departs from:`, not `ARCHITECTURE.md` unqualified

**Discovered during:** `QM-0167` implementation; confirmed and its premises
re-checked by `review-agent-5`, which returned `CHANGES_REQUESTED` **solely**
because the deviation had not been recorded through this mechanism.
**Defect:** `QM-0167`'s acceptance criterion 4 reads *"rank 1 is restored to
`ARCHITECTURE.md`"*. Implemented literally, that would assert `ARCHITECTURE.md`
§5 outranks `ADR-003`, §16 outranks `ADR-007`, and §8.2 outranks `ADR-009` —
contradicting **three Accepted ADRs**, and contradicting `ARCHITECTURE.md` §2.1's
own instruction:

> "**The implementation departs from this.** The catalog is SQLite …
> **Do not "fix" the code to match this paragraph.**" — `ARCHITECTURE.md:73-77`

**Correction:** The precedence table places **Accepted ADRs carrying a
`Departs from:` line at rank 1** and **`ARCHITECTURE.md` at rank 2**, matching
`.plan/README.md`'s authoritative-documents table and this controller's §0.1. The
criterion's *intent* — that `ARCHITECTURE.md` outrank the strategy document and
all of `.plan/` — is met; the strategy document is no longer a rank at all, only
the recorded source of the decision. The criterion text itself is amended by the
task so a later reader does not see a criterion that was silently not followed.

**A false premise found by the reviewer, recorded because it was load-bearing.**
The implementer's evidence record claimed **four** ADRs carry a `Departs from:`
line (ADR-001/003/007/009). `grep -rn "Departs from" docs/decisions/` returns
exactly **three** — ADR-003 (`§2.1 and §5`), ADR-007 (`§16`), ADR-009 (`§8.2`).
**ADR-001 has no such line**: it is Accepted and is a genuine departure, but it is
recorded *inline* in `ARCHITECTURE.md` §16 rather than through the line. The
conclusion survives the miscount — three ADRs are sufficient — but the premise was
wrong and is corrected rather than left standing.

**Files changed:** `.plan/tasks/QM-0167-root-document-amendment/TASK.md`,
`.plan/evidence/QM-0167.md` (by the implementer); this changelog (controller).
**Dependency impact:** none.
**Evidence:** `ARCHITECTURE.md:73-77`, `:259-263`, `:1027-1037` (three inline
departure notes, quoted by the reviewer); `grep -rn "Departs from"
docs/decisions/` → 3 hits; reviewer's SHA-256 byte-identity table showing
§1–§16, §19 and §20 unchanged between `793e122` and `22260e7`.

## 2026-08-04 — CONTROLLER — six root/docs files still present deferred Phase 0 as the active track, and no task owns them

**Discovered during:** `QM-0167` implementation and its independent review.
**Defect:** `QM-0167` corrected the six documents in its declared scope, but
**six further files still describe the deferred Phase 0 tiling spike as the active
track, and no task in the plan declares any of them**:

`docs/PRODUCT_ARCHITECTURE_v1.md:15` · `docs/requirements/PREREQUISITES.md:41,65`
· `docs/TESTING.md:12` · `docs/agent/CHARTER.md` ·
`.plan/STRATEGY_ALIGNMENT.md` §4 and §6 · `docs/requirements/VIZ_MVP.md`
(whose `TILE-11` also restates `ARCHITECTURE.md` §8.2, which `ADR-009` departs
from — correctly left alone rather than re-litigated, since the file is deferred
wholesale and `TILE-11` is an unchecked row).

**Why this is more than a stale-docs nit.** `docs/requirements/PREREQUISITES.md`
tells an autonomous agent that it **may start Phase 0** — and `AGENTS.md` rule 1
points at that file as the gate checklist. Phases 04–07 are `Deferred` wholesale
for v1. The precedence table resolves the contradiction on paper, but an agent
reading `PREREQUISITES.md` first would never reach the table. **This is a live
hazard for exactly the kind of unattended run that produced this entry**, not a
cosmetic inconsistency.

**Correction:** recorded here and requiring a **new task** — no existing task
declares these files, so routing them to `QM-0002` or `QM-0090` would exceed those
tasks' declared scope. Not created in this run: the run is inside its final hour
and creating a task it cannot also execute, review and merge would leave a new
`Undefined` entry with no evidence behind it. The next run should create it and
schedule it early, because it protects every later agent.
**Files changed:** none. This is a recorded finding, not a repair.
**Dependency impact:** none mechanical; a correctness hazard for future agent runs.
**Evidence:** the file/line citations above, from `QM-0167`'s `## Not performed`
items 1, 5 and 8 and independently confirmed by `review-agent-5`.
## 2026-08-05 — QM-0140 — `## Files Expected to Add` did not name the test files the task requires

**Discovered during:** `QM-0140` implementation (recovery of interrupted work)
**Defect:** `QM-0140`'s `## Scope` requires "Schema validation in tests", its
`## Verification Plan` is `**Automated** — schema validation, round trip, version
refusal, ordering, formatting`, and its `## Test Cases` table has eight rows. Its
`## Files Expected to Add` listed only the five source and schema files, naming no
test file. An implementation satisfying the task therefore had to add files the
declared list did not authorise, which makes real scope creep indistinguishable
from required work at review time.
**Correction:** Added `crates/q-report/tests/schema_conformance.rs`,
`crates/q-report/tests/golden/manifest.v1.json` and
`crates/q-report/tests/golden/manifest.v1.summary.json` to `## Files Expected to
Add`, with a note recording why. No scope was widened: every one of those files
implements a `## Test Cases` row or the `## Verification Plan`. `Cargo.toml`
needed no correction — `## Files Expected to Change` already reads
"`Cargo.toml` — workspace member", which authorises the root-configuration change
that adds `crates/q-report` to `[workspace] members` and to the workspace
dependency table.
**Files changed:** `.plan/tasks/QM-0140-manifest-schema/TASK.md`,
`.plan/PLAN_CHANGELOG.md`
**Dependency impact:** None. `QM-0141`, `QM-0143`, `QM-0150` and `QM-0152` consume
the manifest types and the schema, neither of which changed shape.
**Evidence:** `TASK.md` `## Scope` line 3 "Schema validation in tests";
`## Verification Plan` "**Automated** — schema validation, round trip, version
refusal, ordering, formatting"; `## Files Expected to Add` as written listed five
paths, none under `tests/`.
## 2026-08-05 — QM-0001 — `scripts/verify-baseline.test.sh` added to the declared file scope

**Discovered during:** implementing `QM-0001`
**Defect:** `QM-0001`'s `## Files Expected to Add` named only
`scripts/verify-baseline.sh` and `scripts/baseline.json`. Controller §6 requires
the guard's behaviour to be demonstrated by tests written failing-first, and the
guard is a shell script — there was no declared home for its tests.
**Correction:** `scripts/verify-baseline.test.sh` added to `## Files Expected to
Add` in `.plan/tasks/QM-0001-baseline-verification/TASK.md`. It holds 46 unit
tests over the guard's parsing, JSON-validation and floor-comparison functions,
and `verify-baseline.sh` runs it as a preflight step so it cannot rot unnoticed.

**Why it is not a `cargo test` or a `vitest` test:** either would raise the very
counts `scripts/baseline.json` records, making the floor self-referential and
forcing the recorded floor to differ from the measured baseline. Keeping the
tests in shell lets the floor equal reality exactly — `cargo test --workspace`
still measures 290 and `npx vitest run` still measures 115 at this task's head.

**Files changed:** `.plan/tasks/QM-0001-baseline-verification/TASK.md`
**Dependency impact:** none. No other task references `scripts/`.
**Evidence:** `.plan/evidence/QM-0001.md` — `## Tests added` (46 tests) and
`## Validation evidence` §1 (the harness seen failing before the guard existed).

## 2026-08-05 — QM-0001 — the recorded floor is measured at `793e122` and is stale relative to `main`

**Discovered during:** the controller's mid-task correction to `impl-agent-1`
**Defect:** `QM-0001`'s worktree was cut at `793e122`. `QM-0012` has since merged
to `main` as `4e0e85c`, adding 28 Rust tests; the controller measures **318
passed** on `main` at `9a5398d`. The floor this task commits records **290**,
which is the honest measurement of its own base commit but sits 28 below `main`.
**Why it matters:** this is the floor-staleness asymmetry in a concrete instance.
A floor ABOVE the real count fails loudly — `QM-0001` demonstrates exactly that
with its `999` run (`baseline regression: 290 < 999`). A floor BELOW reality is
**silent**: it does not fail, it simply stops protecting the difference.
**Correction:** `290` is committed unchanged, because `318` cannot be verified
from this worktree and writing an unverified number is the failure mode the task
exists to prevent. **The controller re-measures on the merged `main` and corrects
`scripts/baseline.json` in the same squash commit at merge time.**
**Mitigation added:** `scripts/verify-baseline.sh` now reports a stale floor
explicitly — `FLOOR IS STALE by 28; it sits below reality and protects nothing
above 262` — printing measured and floor side by side on every run, success or
failure. The staleness is visible in the log rather than invisible. It remains
non-fatal: raising the floor is the job of the task that added the tests.
**Files changed:** none in `.plan/` beyond this entry; `scripts/baseline.json`
carries the measured `793e122` value.
**Dependency impact:** none.
**Evidence:** `.plan/evidence/QM-0001.md` §10 and `## Claim limits` 1–2.
## 2026-08-05 — QM-0100 — the real checkpoint makes ADR-010's `GRID-007` refusal testable, but the refusal is unimplemented and belongs to QM-0061

**Discovered during:** `QM-0100` implementation (verifying the local
`models/distilbert-distilgpt2` checkpoint from its header alone).
**Defect:** Two related gaps, one in a task brief and one in the code.

1. `QM-0100`'s controller brief asserted that the six rank-4
   `transformer.h.N.attn.bias` tensors (`[1, 1, 1024, 1024]`) must be **refused
   by `q inspect`** per ADR-010, and required a test named
   `refuses_rank_four_attn_bias_rather_than_flattening`. **This is the wrong
   layer.** ADR-010 states that `q_source::TensorDescriptor::shape` is
   "arbitrary rank already" and that "the metadata layer is rank-agnostic";
   the ceiling sits at the **axis binding, block, tile and layout** layers, as
   `rank >3   bindAxes() returns NotImplemented carrying GRID-007`. Making
   `inspect` refuse these six would have contradicted ADR-010 rather than
   honouring it. The controller identified and corrected this mid-task; recorded
   here so the wrong reading does not resurface.
2. **`bindAxes()` and `GRID-007` exist nowhere in the tree.** A whole-repository
   search (excluding `node_modules`, `.git`, `target`) finds them only in
   `.plan/` and `docs/decisions/`, never in code. `schemas/visualization/spatial-contract.json`
   and its `axis_binding.max_implemented_rank` do not exist either. So ADR-010's
   designed refusal is, today, undefended by any test in either language.

**Correction:** No plan task content changed, and **`QM-0100` did not implement
`bindAxes()`** — it is outside that task's declared `## Files Expected to
Change` (`.gitignore`, `crates/q-cli/src/main.rs`), and putting ADR-010's
refusal in the wrong layer would be worse than leaving it absent. The finding is
recorded for the tasks that own it:

* **`QM-0061`** (`.plan/tasks/QM-0061-axis-binding/TASK.md`) owns `bindAxes()`,
  destined for `spatial/axes.ts` per `.plan/TARGET_ARCHITECTURE.md` line 80.
* **`QM-0040`** (LOD block planner) carries its own rank-4 →
  `NotImplemented`/`GRID-007` test case.
* **`QM-0004`** owns `axis_binding.max_implemented_rank` in the spatial
  contract, asserted from both languages at G1 by `QM-0005`.

**The new fact those tasks gain:** ADR-010's rank ceiling now has a **real
artifact** to be tested against, not just a synthetic fixture.
`models/distilbert-distilgpt2/model.safetensors` contains exactly six rank-4
tensors, `transformer.h.{0..5}.attn.bias`, shape `[1, 1, 1024, 1024]`, F32. Any
of the three tasks above can use them directly. `QM-0100` records their names in
`fixtures/real-checkpoint-record.json` (`rank4_tensor_names`) so the fact
survives deletion of the gitignored checkpoint.

**Dependency impact:** None. No task is blocked or reordered. `QM-0061`,
`QM-0040` and `QM-0004` gain a real-data fixture they did not have.

**Evidence:** `grep -rn "bindAxes\|GRID-007"` over the repository returns only
`.plan/` and `docs/decisions/` hits. `QM-0100` verified the adjacent invariant
that *is* testable today — nothing flattens: ingestion preserves
`[1, 1, 1024, 1024]`, exact rank-4 scalar reads match an independent Python read
on both value and byte offset (including the asymmetric causal-mask pair
`[0,0,5,3] = 1.0` vs `[0,0,3,5] = 0.0`), and all four 2-D entry points refuse
with context — e.g. `q stats` → `error: query rejected: block extents apply to
rank-2 tensors; got rank 4`. Full detail in `.plan/evidence/QM-0100.md`.

## 2026-08-05 — QM-0010 — three spec arrows disagree with shipped Llama behaviour; the code wins

**Discovered during:** `QM-0010` implementation, confirmed independently by `review-agent-8` against the base tree
**Defect:** `QM-0010/TASK.md`'s abbreviated canonical-address arrows spell
`moe.expert[37]`, `query_norm` and `moe.router`. The repository produces
`moe.experts[37]`, `query_normalization` and `router.expert_routing`.
**Correction:** resolved toward the shipped form, because each is **pre-existing
unmodified behaviour** made normative by AC-2 and AC-8 — following the spec would
have *changed shipped Llama behaviour*, which is outside this task entirely.
`ARCHITECTURE.md` §6.1 contradicts none of the three. Verified at
`canonical_name`'s `.experts[{expert}]` emission at base, `llama/plugin.toml:200`,
and `llama/plugin.toml:241-245`.
**Files changed:** none — recorded rather than edited, since the arrows are
abbreviations and the task's normative sections already agree with the code.
**Dependency impact:** none.
**Evidence:** `llama_resolves_moe_expert_tensors` already asserted `experts[37]` at
base; `.plan/evidence/QM-0010.md` §Independent review carries the reviewer's
line-cited confirmation.

## 2026-08-05 — QM-0010 — latent: the `experts.` marker is unanchored and would mis-file `shared_experts.N.`

**Discovered during:** `QM-0010` implementation; volunteered by the implementer and
confirmed by `review-agent-8`
**Defect:** `crates/q-nsir/src/resolver.rs:64-77` locates MoE experts with an
unanchored `suffix.find("experts.")`. `"experts."` is a substring of
`"shared_experts."`, so a name like `model.layers.N.mlp.shared_experts.3.up_proj.weight`
would be filed as **routed expert 3** — a shared expert misreported as a routed one.
**Why it was NOT fixed in `QM-0010`:** the code path is byte-identical to base and
shared with the Llama resolver, placing it outside the gate `QM-0010/TASK.md` sets
for `resolver.rs` edits. No name in the committed Qwen manifest reaches it; the
fixture pins only the safe **singular** `shared_expert.` form. **Whether a real
Qwen checkpoint emits the plural indexed spelling is not established by anything in
this repository, and is asserted neither way.** `generic` is safe (no MoE rules).
**Correction:** anchor the marker on a segment boundary. Recorded here for a
follow-up task rather than fixed opportunistically outside its owning scope.
**Files changed:** none.
**Dependency impact:** none. Not a release blocker on current evidence.
**Evidence:** `resolver.rs:64-77`; fixture assertion at `generate_fixtures.py`
`:1057-1060` / `:1070` pinning the singular form only.

## 2026-08-05 — ADR-011 amended — components are length-prefixed unconditionally; `TileId`/`CacheKey` are named frozen exceptions

**Discovered during:** `QM-0020` implementation; adjudicated by `review-agent-9`;
amendment recommended by that reviewer before `QM-0033` could re-litigate it
**Defect:** ADR-011's construction rule said fixed-width components take no length
prefix, **contradicting this ADR's own reference implementation.**
`q_source::ids::digest16` (`crates/q-source/src/ids.rs:86-92`) prefixes every part
unconditionally, and `TensorId::derive` (`:125-128`) routes a fixed-width `[u8;16]`
model id through it — so every already-persisted `TensorId` is prefix-both. The
superseded sentence justified itself by citing `TileId` and `CacheKey`, but
**neither uses `digest16`**, and `TileId::for_block` omits `ID_SCHEME_VERSION`
entirely, which ADR-011 separately freezes. The two constructions the rule appealed
to were never governed by it.
**Correction:** the rule now reads "every component → u64 little-endian length,
then bytes", with `TileId` and `CacheKey` recorded as **named frozen exceptions**
rather than instances of the rule. The superseded text is quoted in the amendment
rather than deleted.
**Why this direction:** decided by measurement. `review-agent-9` implemented BLAKE3
from the specification in Python, validated it against four published test vectors,
and computed both candidate layouts for `StatisticsId`:
`prefix-both 4b0df4930f8ee4bb1637bcfbcf49499c` (shipped, and what `define_id!`
emits) versus `no-prefix 7e771ceb5144f70c830c81281ef0de56`. Adopting the prose rule
would have created the second id grammar ADR-011 exists to prevent **and** required
a migration for ids already on disk. Correcting the prose costs nothing.
**Files changed:** `docs/decisions/ADR-011-content-derived-identifiers.md`
**Dependency impact:** none. Unblocks `QM-0033` from re-deriving the rule
differently. `QM-0020`'s pinned digest needs no change.
**Evidence:** the two digests above, computed by a third independent BLAKE3
implementation — not the `blake3` crate the implementation and its in-test
transcription both share, so the pinned literal is not the code asserting it equals
itself. Recorded in `.plan/evidence/QM-0020.md` §Independent review.

## 2026-08-05 — QM-0030 — `## Data Contracts` specified a dependency cycle; `BlockData` moved to `q-tensor-runtime`

**Discovered during:** `QM-0030` implementation; cycle confirmed independently by `review-agent-10`
**Defect:** `QM-0030/TASK.md` §Data Contracts specified `StreamedBlock.data:
BlockData` with `BlockData` in `q-gpu`. **`crates/q-gpu/Cargo.toml` already declares
`q-tensor-runtime` under `[dependencies]`, so the contract as written is a
dependency cycle and is unsatisfiable.**
**Correction:** `BlockData` moved into `q-tensor-runtime`, character-for-character
identical, with `pub use q_tensor_runtime::BlockData;` at `q-gpu/src/lib.rs:109`.
`q_gpu::BlockData` stays valid for `CpuBackend`, the `Backend` trait, nine unit
tests and `q-cuda:35,138,167`. `TASK.md` §Data Contracts now carries a controller
correction recording this. Ruled a **plan defect correctly handled**, not an
out-of-scope edit.
**Files changed:** `.plan/tasks/QM-0030-streaming-block-reader/TASK.md`
**Dependency impact:** none. `QM-0101` consumes `BlockData` from its new home.
**Evidence:** `crates/q-gpu/Cargo.toml` `[dependencies] q-tensor-runtime`;
re-export at `q-gpu/src/lib.rs:109`; `cargo build --workspace --all-targets` exit 0.

## 2026-08-05 — QM-0030 — the evidence overstates its own per-block assertion coverage

**Discovered during:** `review-agent-10`'s independent review — a finding against the
*evidence record*, not the code
**Defect:** `.plan/evidence/QM-0030.md:425-429` claims `bytes_read` is asserted per
block "in every phase" and that `run_count == extent.rows()` is asserted there.
Measured, it is narrower: the per-block assertion exists only in the three-size
phase (`bounded_residency.rs:237-241`); the 65536² phase asserts the **aggregate
only** (`:309`); the bounded-queue phase asserts **no bytes at all** (its sink is
`|_|`, `:336-342`); and `run_count == rows` lives at `src/stream.rs:1163` as
`shard.reads() == 256`, not in that file.
**Why it is not blocking:** AC2 depends on neither thin phase, and
`TensorBlock::plan` (`src/lib.rs:244-249`) makes one-run-per-row structural rather
than incidental. The residency result itself reproduced byte-identically.
**Correction:** recorded here so a `QM-0101` reader inherits the accurate scope
rather than the claim. A method overstated in an evidence record is the kind of
defect that silently licenses a later, weaker measurement.
**Files changed:** none — the reviewer's own section carries the correction.
**Dependency impact:** none.
**Evidence:** the four file:line citations above, all verified by the reviewer.
## 2026-08-05 — QM-0002 — `.plan/` citation, vocabulary and merge-path reconciliation

**Discovered during:** `QM-0002` corpus sweep, re-run at `1d49ffa` after the
branch was rebased onto `main` at `eca5a6a`, with
`.plan/tools/check-plan-citations.py`.
**Defect:** Unresolved citations across `.plan/`, plus non-path defects the
checker cannot see (a status-value count, a hard-coded `Ready` set, two "pull
request" requirements, and stale repository counts) — **and, found by the
independent review, a false claim this task itself introduced: that the `gh`
token "cannot push".**
**Correction:** Eleven `.plan/` documents corrected; `.plan/DIVERGENCE_REGISTER.md`
created with fourteen rows. Details and proving citations in
`.plan/evidence/QM-0002.md`.
**Files changed:** `.plan/README.md`, `.plan/CURRENT_ARCHITECTURE.md`,
`.plan/REPOSITORY_ANALYSIS.md`, `.plan/MATRIX_WORKSPACE_ARCHITECTURE.md`,
`.plan/EXECUTION_ORDER.md`, `.plan/RISK_REGISTER.md`, `.plan/STRATEGY_ALIGNMENT.md`,
`.plan/phases/phase-13-diagnostic-surface/README.md`, `.plan/evidence/QM-0140.md`,
`.plan/evidence/QM-0167.md`, and the `TASK.md` files of `QM-0002`, `QM-0141`,
`QM-0150`, `QM-0153`, `QM-0165`, `QM-0167`.
**Dependency impact:** None. No task's `Dependencies` or `Blocks` changed, no ID
renumbered, and no `## Status` other than `QM-0002`'s own.
**Evidence:** `python3 .plan/tools/check-plan-citations.py` at `1d49ffa` →
**12 unresolved before, 2 after** on the same tool (the inherited tool read 14 on
the inherited corpus and 15 once the corpus stopped masking itself — all four runs
are tabulated in `.plan/evidence/QM-0002.md`), exit 1 throughout. The two survivors are
`QM-0006`'s deliberate pre-rename record (finding 2) and one citation inside the
frozen `## Independent review` section of `.plan/evidence/QM-0002.md`, which the
controller's brief requires be left intact. Injected-failure demonstration
2 → 5 → 2, with both documented exemptions firing once each. Gates at `1d49ffa`:
`cargo fmt` 0, `cargo clippy -D warnings` 0, `cargo test --workspace` 0
(**434 passed; 0 failed; 0 ignored** over 43 `test result:` lines),
`npx vitest run` 0 (**115 passed, 13 files**), `./scripts/verify-baseline.sh` 0
(at floor 434/43/115/13), `./scripts/verify-baseline.test.sh` 0 (46 run, 0 failed).

### Fix cycle 1 — what the independent review found, and what it changed

`review-agent-7` returned `CHANGES_REQUESTED` at head `6e99e62`: **one root cause,
seven symptoms — the branch's base was fifteen commits behind `main`, so a task
whose whole job is reconciliation asserted stale facts.** This entry is corrected
in place rather than superseded, because it had never merged. The corrections:

1. **The push claim was false, and false when written.** An earlier revision of
   `.plan/README.md` and of `DIV-011` said the `gh` token "cannot push …
   `Permission to quatricmorph/quatricmorph.git denied`", citing the **superseded**
   2026-08-04 entry ("Run 1's Stage 0 credential halt is superseded"). The entry
   that supersedes it — *"push to `origin` succeeds; Run 2's credential finding is
   superseded"*, commit `3394510` — is an **ancestor of this branch's own base** and
   sits in this same file. Both facts are now stated separately: **no PR is
   creatable** (`gh api repos/quatricmorph/quatricmorph --jq .permissions` →
   `"push": false`) and **pushing over SSH succeeds** (`git ls-remote origin
   refs/heads/main` equals local `main`; `git reflog show origin/main` has an
   `update by push` per merge). `DIV-011` moved `Resolved` → `Decided` with the
   source-by-source derivation written out.
2. **Counts re-measured, not excused.** Rust **318 / 39 → 434 / 43**; crates
   **17 → 18** (`QM-0140` added `crates/q-report`); JSON schemas **4 → 5**
   (`schemas/diagnostics/manifest.v1.json`); `Cargo.toml` members **18 → 19**;
   `ARCHITECTURE.md` 1261 → **1376** lines; `STATUS.md` 278/129 rows →
   **279 / 131**; `AGENTS.md` 48 → **47**; root `README.md` 127 → **142**;
   `crates/` ~15 184 → **18 692** lines over 46 files; `mm/` "4 files" → **5**
   (`REPOSITORY_ANALYSIS.md` §3 nine lines below already said five). The crate
   inventory table gained its missing `q-report` row and its hand-copied per-file
   breakdowns became one re-derivable total per crate.
3. **`QM-0167`'s seven citations were repaired, not deferred.** The stated reason
   for deferring them — "another agent is editing that file in this run" — became
   false when `QM-0167` merged (`f132393`) and reached `Complete`. `TASK.md:121`
   is unambiguous. Finding 1 below records what was done instead of what was
   deferred.
4. **The checker's own header no longer says `scripts/` "does not exist yet".**
   It exists, and the same file's `NEW_TOP_LEVEL` comment already said so. The
   reason the checker lives in `.plan/tools/` is the boundary, not absence.
5. **Three checker defects fixed — two false positives and one false exemption.**
   Detail in `.plan/evidence/QM-0002.md` under `## Fix cycle 1`. (i) A
   document-relative template placeholder was shape-checked with its `../` intact,
   so every such placeholder in `.plan/README.md` failed. (ii) The elision `…` was
   not recognised where ASCII `...` was. (iii) **Worse than either**: a `## …`
   heading quoted inside a fenced code block set the document section for everything
   after it, so quoting a `## Test Cases` heading laundered every later citation
   through `E1`. Reproduced: pasting this cycle's probe file verbatim took `E1` from
   420 to 506 and made two real failures vanish. The `section` update now happens
   only outside a fence. **The two documented exemptions are themselves unchanged**
   and `E1` reads 420 before and after.
6. **Conflict resolution honoured a deletion.** `f132393` deleted
   `.plan/README.md`'s "On the rank 1 / rank 2 conflict" paragraph and rewrote the
   authority table 8 → 9 rows. The rebase kept the deletion and dropped this
   task's edit to the deleted paragraph rather than resurrecting it.
7. **A line-initial triple backtick in `.plan/evidence/QM-0002.md` was masking part
   of the corpus from the checker.** An inline code span written at the start of a
   line reads as a **fence delimiter** to the scanner, which skips everything inside
   a fence. That one line flipped fence parity and hid citations below it — including
   inside the independent reviewer's own section — and left the file ending in an
   open-fence state. A checker that cannot see part of its corpus reports a count
   that is **too low, silently**, which is the same asymmetry `QM-0001` records for a
   floor set below reality. Rewritten as an italic quotation; all **181**
   `.plan/**/*.md` files then swept for fence parity, **zero** odd. This is why the
   honest before-count is 15 and not 14.

### Findings, and who they went to

1. **`QM-0167` — seven document-relative citations that did not resolve. FIXED
   HERE in fix cycle 1.** `.plan/tasks/QM-0167-root-document-amendment/TASK.md:28-31`
   (`:25-27` before `main` moved) cited `../ARCHITECTURE.md`,
   `../MASTER_DOCUMENT.md`, `../docs/ROADMAP.md`, `../docs/PRODUCT_BRIEF.md`,
   `../docs/requirements/VIZ_MVP.md`, `../README.md` and `../STATUS.md` inside
   `## Repository Evidence`. From that file's directory `../` is `.plan/tasks/`, so
   `../ARCHITECTURE.md` meant `.plan/tasks/ARCHITECTURE.md`, which does not exist,
   and the other six behaved the same way. The neighbouring bullets in the same
   list are repo-root-relative (`.plan/STRATEGY_ALIGNMENT.md`), which is the
   convention followed: the `../` is dropped, and the ambiguous bare `README.md` is
   written as "the repository-root `README.md`" so it cannot be read as
   `.plan/README.md`. **The earlier deferral reason — "another agent is editing that
   file in this run" — was true when written and false by the time it was read:
   `QM-0167` merged as `f132393` and is `Complete`, so there is no concurrent
   editor and `TASK.md:121` ("an unresolvable citation is a plan bug, fixed here,
   not deferred") governs.** The identical defect in `QM-0141` and `QM-0165` was
   fixed in the first pass; before/after for all of them is in
   `.plan/evidence/QM-0002.md`. `QM-0167`'s `## Status` was **not** touched.
2. **`QM-0006` — one pre-rename evidence citation, deliberately left.**
   `.plan/tasks/QM-0006-web-workspace-path-repair/TASK.md:40` cites
   `apps/web/matrix-workspace/package.json` in `## Repository Evidence`. That path
   no longer exists *because `QM-0006` renamed it*, so the citation is accurate as
   the pre-rename record and rewriting it would destroy the audit trail for a
   merged, `Complete` task. `QM-0006`'s own `## Out of Scope` hands the `.plan/`
   *prose* to `QM-0002` but not its own evidence section. Left as a permanent,
   documented, owner-assigned exception.
3. **`.plan/ORCHESTRATION_STATE.md:141` is stale.** It records the web gate as
   `27 passed (3 files)` — "BROKEN, see QM-0006" — and the web build as failing.
   `QM-0006` merged; both now pass (`115 passed (115)`, 13 files). Controller-owned
   file; `QM-0002` may not edit it. The controller has since appended its "Run 4"
   section to the same file, so the stale line is now clearly historical rather
   than current, but it is still uncorrected in place.
4. **`COMPONENTS_MAP.md` has no owning task.** It sits at the repository root and
   still names the pre-rename workspace directory. `grep -rn "COMPONENTS_MAP" .plan/`
   finds no task whose `## Files Expected to Change` names it. `QM-0002`'s
   `## Program Boundary` is `.plan/` only, so it cannot be corrected here. It needs
   an owner before it can be corrected at all. Registered as `DIV-010`.
5. **`STATUS.md:9-10` is behind the tree.** It claims `290` Rust and
   `101 (12 files)` web; the tree at `1d49ffa` prints **`434`** over 43 binaries and
   `115 (13 files)`. The gap is wider than first recorded — it read `318` then —
   because `QM-0140` and `QM-0100` merged in between. That is `QM-0091`'s
   regeneration, not a plan defect. Registered as `DIV-009`.
6. **`QM-0002`'s own `## Files Expected to Add` contradicts its
   `## Program Boundary`.** The former names `scripts/check-plan-citations.sh`;
   the latter says "`.plan/` only. This task changes no repository file." They
   cannot both hold, so the checker was placed at
   `.plan/tools/check-plan-citations.py`. **The reason is the boundary alone.**
   An earlier revision of this finding and of the checker's own header added that
   `scripts/` "does not exist" — that was false when written (`QM-0093` had already
   landed `scripts/license-audit.sh`, in the very commit this branch was based on)
   and is corrected in fix cycle 1. `scripts/` now also holds `baseline.json`,
   `verify-baseline.sh` and `verify-baseline.test.sh` from `QM-0001`. **Acceptance
   criterion 1, which names the `scripts/` path, is not claimed as met.** A future
   pass should reconcile the two sections; `QM-0002` did not edit its own scope to
   make itself pass, and the independent review endorsed that disposition.
7. **`.plan/evidence/QM-0140.md` and `.plan/evidence/QM-0167.md` — three citations
   repaired, no claim touched.** `QM-0140.md:221` and `:601` cited
   `tests/schema_conformance.rs`, which does not exist — repo-anchored that names
   the root `tests/` crate, and the file is not there. The real path is
   `crates/q-report/tests/schema_conformance.rs`, and the test counts around it
   (59 lib + 38 integration) are unchanged. `QM-0167.md:20` cited
   `docs/decisions/ADR-003`, which does not exist under that name; the directory
   `docs/decisions/` is cited instead and the three ADR ids left as ids. These are
   merged tasks' records, so the repairs add a prefix or point at the directory and
   change nothing anyone claimed. Contrast finding 2: `QM-0006`'s path was *accurate
   as a pre-rename record*, whereas these three were never correct from any
   directory.

## 2026-08-05 — CONTROLLER — the citation checker's first run on the merged corpus finds 18 unresolved; 16 are real

**Discovered during:** post-merge verification of `QM-0002`, running
`.plan/tools/check-plan-citations.py` against `main` at `8b0db9d`
**Result:** `184 markdown files scanned · E1 420 · E2 117 · FAIL — 18 unresolved`,
exit 1. `QM-0002`'s branch corpus measured 2; the merged corpus is larger because
`QM-0010`, `QM-0020`, `QM-0030` and the ADR-011 amendment landed in between. **This
is the deliverable working, not a regression** — the checker's whole purpose is to
find exactly this.

**Breakdown, verified rather than assumed:**

| Count | Class | Disposition |
| --- | --- | --- |
| 15 | Bare `tests/<name>.rs` citations written **crate-relative** instead of repo-anchored, in `QM-0020`'s and `QM-0030`'s evidence and in `QM-0030/TASK.md:217` | **Real defects.** The files exist: `crates/q-tensor-runtime/tests/bounded_residency.rs`, `crates/q-tensor-runtime/tests/real_fixture_blocks.rs`, `crates/q-catalog/tests/trillion_scale_manifest.rs`. Confirmed by `find`. Cosmetic but exactly what the checker is for |
| 1 | `.plan/evidence/QM-0020.md:835` cites `crates/q-daemon/src/lib.rs:1191` | **Real** — a `:NN` line suffix the checker does not strip. Either anchor the citation or teach the checker to strip line suffixes |
| 1 | `.plan/evidence/QM-0020.md:627` cites the glob `tests/*.rs` | **Real** — a glob is not a path; reword |
| 1 | `.plan/tasks/QM-0006-web-workspace-path-repair/TASK.md:40` cites `apps/web/matrix-workspace/package.json` | **Deliberate.** A historical pre-rename record of the defect `QM-0006` fixed. Correct as written |
| 1 | `.plan/evidence/QM-0002.md:1114` cites `.plan/zz-review-probe` | **Deliberate.** A reviewer's probe, since deleted, inside the frozen `## Independent review` section |

**Why this was not a merge blocker for `QM-0002`:** none of the 16 real defects are
in `QM-0002`'s own output. They are citations in other tasks' already-merged
evidence, surfaced *because* `QM-0002` shipped the tool that can see them. Blocking
`QM-0002` on defects its own deliverable discovered elsewhere would be incoherent.
**Correction:** the 16 are recorded here for a follow-up citation-anchoring pass; the
2 deliberate residues are documented as permanent exemptions.

## 2026-08-05 — CONTROLLER — the citation checker has no self-guard against fence parity

**Discovered during:** `review-agent-11`'s second-cycle review of `QM-0002`, carried
forward at that reviewer's explicit request
**Defect:** `QM-0002` fixed the specific laundering bug — a `## ` heading quoted
inside a code fence setting the document section for the rest of the file, silently
exempting ~102 citations through E1 — and swept all 181 `.plan/` files for zero odd
fence parity. **But the sweep is a snapshot, not an invariant.** The checker asserts
nothing about its own fence parity, so a single future line-initial triple backtick
reopens the same silent-underreport class, and the failure is invisible: the tool
still exits 0 while verifying less than it claims.
**Correction:** a follow-up task should make the checker assert **even fence parity
per file** and fail loudly on an odd count, so the guard cannot silently degrade.
This is the same principle the test floor already enforces — a check that can quietly
verify less than it claims is the failure mode this run has now hit twice
(`27 passed` reading as green, and ~102 laundered citations).
**Files changed:** none — recorded for a follow-up task rather than fixed outside its
owning scope.
**Dependency impact:** none. Not a release blocker.
**Evidence:** `review-agent-11` reproduced the laundering with both tool versions
against one identical probe corpus: pre-fix `E1 = 523` with the post-fence citation
**unreported**; post-fix `E1 = 420` with it reported. Recorded in
`.plan/evidence/QM-0002.md` §Independent review, second cycle.

## 2026-08-05 — CONTROLLER ERROR — several agent briefs cited `.plan/TEST_STRATEGY.md` §6.3, which does not exist

**Discovered during:** `QM-0101`, reported by `impl-agent-12` at the end of its run
**Defect:** the controller cited **`.plan/TEST_STRATEGY.md` §6.3** as the authority
for memory-residency testing in the task packets for `QM-0030`, `QM-0101`, `QM-0120`
and others. **That section does not exist.** `## 6. CI` carries no subsections;
`grep -nE '^#+ *6' .plan/TEST_STRATEGY.md` returns `243:## 6. CI` alone. The
residency-testing direction the controller meant to invoke — peak-RSS assertion via
`/usr/bin/time -l` — is carried by each task's own `TASK.md` and by
`.plan/MEMORY_BUDGET.md`, not by `TEST_STRATEGY.md`.
**Consequence:** none to the delivered work. Agents read the real documents and
followed `TASK.md`'s direction; `impl-agent-12` explicitly flagged the bad citation
rather than silently inventing a §6.3. That is the correct response to a defective
brief, and it is recorded here as the reason the citation appears in evidence
records.
**Correction:** future packets cite `.plan/MEMORY_BUDGET.md` and the task's own
`## Verification Plan` for residency, and `.plan/TEST_STRATEGY.md` §0 (the three
properties) plus its §6 for CI. **A controller summary is never a substitute for the
source document — this run's own rule, and the controller broke it.**
**Files changed:** none.
**Dependency impact:** none.
**Evidence:** `grep -cE '^#+ *[0-9]+\.[0-9]' .plan/TEST_STRATEGY.md` → 9 subsections
exist in the file, none of them under §6.

## 2026-08-05 — DIAGNOSTIC_ARCHITECTURE §3.1 amended — the constant-non-zero-group rule is now specification, not evidence

**Discovered during:** `QM-0120`; amendment recommended by `review-agent-12` before
`QM-0121`/`QM-0122` could re-derive it
**Defect:** §3.1's degenerate-case table conditions its `s = 1` row on
`max|g| == 0` — an **all-zero** group — so it specified **nothing** for a *constant
non-zero* group. `QM-0120` read the table literally, used `s = 1`, and reconstructed
`0.5 → 0.0`: a **100 % error**.
**Why the differential test missed it:** the first golden set's only constant
magnitude was `c = 1` — **the single value at which `s = 1` and `s = |c|` produce
identical output.** The reference was independent and correct; the *inputs* could not
discriminate between two candidate formulas.
**Correction:** two rows added to §3.1's table — `min(g) == max(g) != 0` →
`s = |c|`, and non-finite reconstruction → refuse per value (`s` can be `is_normal()`
while `q_max · s` overflows; the derived scale `2.6793887e36` was confirmed normal
while `127·s` rounds past `f32::MAX`). A note records the derivation, and the
transferable lesson: **a golden set needs inputs chosen to discriminate, not merely
to cover.** The all-zero group keeps its tabulated `s = 1, z = 0`.
**Also routed:** `QM-0122/TASK.md` §Risks now carries the inherited hand-off —
it derives per-channel params from accumulated min/max, cannot call
`derive_params_named`, and so its `max == min` branch is the exact place this defect
reappears. It is told to reuse `q-quant`'s logic and to include a constant non-zero
group whose magnitude is **not 1**. Previously this analysis existed only in
`.plan/evidence/QM-0120.md`, invisible to the task that inherits the risk.
**Files changed:** `.plan/DIAGNOSTIC_ARCHITECTURE.md`,
`.plan/tasks/QM-0122-streaming-diagnostic-pass/TASK.md`
**Dependency impact:** none. Prevents `QM-0121`/`QM-0122` from re-deriving the rule
differently and reintroducing a 100 % error behind a passing golden test.
**Evidence:** `review-agent-12` re-derived the rule in an independent driver crate:
`0.5`, `−0.3`, `0.823457` all bit-exact under `s = |c|`; `s = 1` gives 100 %, 100 %,
and wrong-direction error.

## 2026-08-05 — QM-0120 — three stale prose figures; two fixed, one deliberately left

**Discovered during:** `review-agent-12`'s independent review, by measurement
**Defect:** `.plan/evidence/QM-0120.md` said `49 passed` where the measured figure is
**53** (contradicting 598 in the same table), and described `rtn.rs` as holding
26 unit tests where it holds **30**. Separately,
`python/reference/quantise_reference.py:472` and the golden's `why` field describe
"a 41 % error" for `0.823457`; the actual figure is **21.4 %** — 41.4 % belongs to the
original `0.7071` probe.
**Correction:** the two evidence figures are fixed in `QM-0120`'s merge commit. **The
"41 %" is deliberately left unchanged in both the generator and the golden**, because
editing either changes the golden's SHA-256 —
`d4efa48e9f4e5335422835c25df0185a0c467efc9cd6d566dcf7530d41f0466f` — which
`review-agent-12` independently verified across four regenerations. Churning a
reviewer-verified golden to correct a prose percentage would trade real evidence for
cosmetics. Recorded here instead.
**Files changed:** `.plan/evidence/QM-0120.md`
**Dependency impact:** none. `scripts/baseline.json` and `TASK.md` §Orchestration
were both already correct — only prose was stale.
**Evidence:** reviewer measured `cargo test -p q-quant` → 53 = 45 lib + 1 + 7, and
counted 30 tests in `rtn.rs`.

## 2026-08-05 — CONTROLLER — `crates/q-gpu/src/lib.rs` is a third shared mutable file and is missing from the forbidden-concurrency table

**Discovered during:** readiness recomputation after `QM-0120` reached `Complete`.
**Defect:** `.plan/EXECUTION_ORDER.md` §6 lists the sequences that may not run
concurrently, and names two shared mutable files as the reason:
`crates/q-catalog/src/lib.rs` (the `QM-0012` → `QM-0020` → `QM-0032` chain) and the
`QM-0120` → `QM-0125` output chain. **It does not name `crates/q-gpu/src/lib.rs`.**

That file is declared by at least four v1 tasks:

| Task | Declares `crates/q-gpu/src/lib.rs` as |
| --- | --- |
| `QM-0121` paired block reduction | "the trait, the types, the CPU implementation" |
| `QM-0031` CPU statistics pass | "pass driver" |
| `QM-0032` wire the cache | listed first in its change set |
| `QM-0037` backend selection | the selection seam (rewired to `QM-0126`) |

`QM-0031`'s v1 unblock condition (`QM-0030` and `QM-0020` both `Complete`) is now
satisfied, so a controller scheduling purely from the dependency graph and the §6
table **would have started it concurrently with `QM-0121`** and produced two agents
editing the same file in different worktrees — precisely the class of conflict §6
exists to prevent, and one that would surface only at merge as a conflict in the
most safety-critical file in the engine.

**Correction:** `QM-0031` is **held** behind `QM-0121`'s merge. §6's table should
gain `crates/q-gpu/src/lib.rs` with the sequence
`QM-0121` → `QM-0031` → `QM-0032` (and `QM-0037` when the Metal lane opens),
mirroring the `q-catalog` row it already carries.

**Why this was catchable only by checking declared scope, not the graph.** There is
no dependency edge between `QM-0121` and `QM-0031` — they are genuinely independent
in the plan's own ordering. The collision is purely a file-scope one, which is why
§14 requires comparing a candidate task's declared files against every *active*
task's files as a separate check from the dependency test. The dependency graph
alone would have said "go".

**Files changed:** none — recorded as a finding. The §6 edit belongs to whichever
task owns `EXECUTION_ORDER.md`; the controller did not edit it inline because the
owner has been amending `.plan/` during this run.
**Dependency impact:** `QM-0031` queued behind `QM-0121`; no edge added.
**Evidence:** the four tasks' `## Files Expected to Change` sections;
`.plan/EXECUTION_ORDER.md` §6's table as it stands.
