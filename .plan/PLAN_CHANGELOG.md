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
