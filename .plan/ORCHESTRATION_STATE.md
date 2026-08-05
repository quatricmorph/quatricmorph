# Orchestration State

Controller registry for the autonomous implementation loop.
Created: 2026-08-04T13:40:00Z
Git is the evidence; this file is a restart cache.

## Stage 0 — Capability probe (verbatim)

Recorded at controller checkout `/Users/thanh/Quatricmorph/Quatricmorph`
on branch `main` at `85b5b63d666146a4e143d049ff333d3124e881cf`.

```text
=== git remote -v ===
origin	git@github.com:quatricmorph/quatricmorph.git (fetch)
origin	git@github.com:quatricmorph/quatricmorph.git (push)

=== HEAD branch ===
main

=== HEAD sha ===
85b5b63d666146a4e143d049ff333d3124e881cf

=== log ===
85b5b63 feat: Add tasks for runtime error audit, documentation update, and acceptance audit

=== status ===
## main...origin/main
(local dirty tree present — see Pre-existing local state)

=== fetch ===
(ok)

=== worktree ===
/Users/thanh/Quatricmorph/Quatricmorph 85b5b63 [main]

=== gh auth ===
github.com
  ✓ Logged in to github.com account MarkdownOfficial (keyring)
  - Active account: true
  - Git operations protocol: ssh
  - Token: gho_************************************
  - Token scopes: 'gist', 'read:org', 'repo'

=== repo api ===
{"allow_auto_merge":null,"allow_squash_merge":null,"default_branch":"main","delete_branch_on_merge":null,"permissions":{"admin":false,"maintain":false,"pull":true,"push":false,"triage":false}}

=== branch protection ===
{"message":"Not Found","documentation_url":"https://docs.github.com/rest/branches/branch-protection#get-branch-protection","status":"404"}
gh: Not Found (HTTP 404)

=== push dry-run ===
To github.com:quatricmorph/quatricmorph.git
 * [new branch]      HEAD -> qm-capability-probe
```

### Interpretation

| Fact | Value |
| --- | --- |
| SSH write (`git push --dry-run`) | succeeds |
| `gh` account | `MarkdownOfficial` |
| `permissions.push` | **false** |
| `permissions.pull` | true |
| Branch protection on `main` | 404 (none) |
| Selected merge path | **none — halted** |

### Halt reason

`BLOCKED_BY_CREDENTIAL` — see controller report. Pull-request creation cannot be
performed with pull-only API identity. Per controller §1, do not proceed to
worktrees or Path-C local merges without operator credential fix.

Probe branch `qm-capability-probe` was dry-run only; not left on the remote
(dry-run does not publish). No lasting remote probe state intended.

## Pre-existing local state (not claimed as this run's work)

Controller checkout was **not clean** at probe time. Uncommitted / untracked:

- Modified: multiple `.plan/**` files, `ARCHITECTURE.md`
- Untracked: `docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md`,
  `docs/decisions/ADR-010-tensor-rank-ceiling.md`
- Untracked (unrelated): `autonomous_sample_prompts.md`

Honesty baseline commit cited in the run brief (`5ca434d`) is an ancestor of
current `origin/main` (`85b5b63`); main has advanced past that snapshot.

## Baseline verification

**Not run.** Halted at Stage 0 before baseline and before first worktree.

## Active tasks

None. Controller halted before Wave 0.

## Completed tasks (this run)

None.

## Blockers

1. `BLOCKED_BY_CREDENTIAL` — `gh` as `MarkdownOfficial` has `pull` only;
   SSH can push branches; pull requests cannot be created under this API identity.

---

# Run 2 — 2026-08-04T16:03:45Z

**Start:** 2026-08-04T16:03:45Z · **Deadline (T+5h00m):** 2026-08-04T21:03:45Z
**Controller checkout:** `/Users/thanh/Quatricmorph/Quatricmorph` · branch `main`
**Base commit at start:** `fe501e536dc45c8b564c5fac470f43fdf9937fed`
**`origin/main` == local `main`** at start; working tree clean.

## Stage 0 probe — informational only (controller §1)

```text
gh account MarkdownOfficial; scopes gist, read:org, repo, workflow
permissions: {"admin":false,"maintain":false,"pull":true,"push":false}
branch protection on main: 404 Not Found  (no required checks, no required approvals)
git fetch origin: exit 0; origin/main = fe501e5
df -h .: 51Gi available
```

**Merge path: L (local squash), fixed.** No PR is creatable (`push: false` on the
`gh` token). The Git remote is SSH under a different identity, and Run 1's probe
recorded `git push --dry-run` succeeding for a new branch, so `git push origin main`
is expected to work. This is confirmed at first merge, not assumed.

**Run 1's Stage 0 credential halt is superseded.** Controller §1 removes the halt:
a missing token scope is never a reason to stop. Run 1's history is retained above.

## Verified baseline at `fe501e5` (raw commands, controller §5)

| Gate | Command | Exit | Result |
| --- | --- | --- | --- |
| fmt | `cargo fmt --all -- --check` | 0 | clean |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | clean |
| build | `cargo build --workspace --all-targets` | 0 | clean |
| rust test | `cargo test --workspace` | 0 | **290 passed; 0 failed; 0 ignored** |
| web test | `cd apps/web && npx vitest run` | 0 | **27 passed (3 files)** — BROKEN, see QM-0006 |
| web build | `npm run build --workspace quatricmorph-workspace` | 1 | **FAILS: "No workspaces found"** — BROKEN, see QM-0006 |
| fixtures | `python3 fixtures/generate_fixtures.py` + `git diff --exit-code -- fixtures/` | 0 / 0 | reproducible |

Rust matches `STATUS.md`'s 290 exactly. **Web does not**: `STATUS.md` claims
101 passing across 12 files; the tree runs 27 across 3. The 9 missing files hold
74 tests (27 + 74 = 101). Cause and repair: `QM-0006`.

`numpy` / `safetensors` are absent from the system Python (PEP 668 blocks
`pip install --user`). The fixtures gate was run from a scratch venv at
`.../scratchpad/fxvenv/bin/python`. Recorded so the gate's provenance is checkable.


## Task registry

| Task | Lane | Wave | State | Branch | Worktree | Impl | Review | Merge |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `QM-0006` web workspace path repair | S | 0 | Implement | `task/qm-0006-web-workspace-path-repair` | `../.qm-worktrees/qm-0006` | impl-agent-1 | pending | — |
| `QM-0140` manifest schema | R | 1 | Implement | `task/qm-0140-manifest-schema` | `../.qm-worktrees/qm-0140` | impl-agent-2 | pending | — |
| `QM-0012` config model metadata | T | 2 | Implement | `task/qm-0012-config-model-metadata` | `../.qm-worktrees/qm-0012` | impl-agent-3 | pending | — |
| `QM-0002` plan reconciliation | V | 0 | Implement | `task/qm-0002-plan-repo-reconciliation` | `../.qm-worktrees/qm-0002` | impl-agent-4 | pending | — |

All four branched from `ace7d09`, proven clean (`git status --short` empty) and
correctly based (`git merge-base --is-ancestor main HEAD` exit 0) before assignment.

## Sequencing decisions (file-scope conflicts)

| Held task | Blocked behind | Conflict |
| --- | --- | --- |
| `QM-0001` baseline verification | `QM-0006` | Both edit `.github/workflows/build.yaml`; and `QM-0001`'s own floor spec names `web: 101`, unmeetable until `QM-0006` lands |
| `QM-0010` Qwen resolver | `QM-0012` | Both touch `crates/q-architecture` |
| `QM-0093` licence audit | `QM-0001` | Both create files under `scripts/` |
| `QM-0167` root document amendment | `QM-0002` | Both touch `.plan/README.md`; also sequenced late by its own design |
| `QM-0160` outreach scaffolding | disk | Human-dependent (§3.2); queued until the checkpoint download frees headroom |

## ADR candidates

Eighteen candidates remain unpromoted. **None is named by any task in flight or
next up** (`QM-0006`, `QM-0140`, `QM-0012`, `QM-0002`, `QM-0001`, `QM-0010`,
`QM-0093`, `QM-0160`, `QM-0167` — grepped, zero `ADR-CANDIDATE-` references).
No promotion is therefore blocking, and none is performed speculatively: a
promotion sweep would raise task counts without producing verified code.

## v1 set reconciled against the wave table (controller §21 step 7)

Parsed from all 89 `TASK.md` files at `ace7d09`:

| Measure | Value |
| --- | --- |
| Task directories | 89 |
| `Deferred` | 44 |
| `Blocked` | 36 |
| `Ready` | 9 (+`QM-0006` added by this run = 10) |
| v1 set (non-`Deferred`) | **45** |
| Tasks named in `.plan/EXECUTION_ORDER.md` §2's wave block | 42 |
| Wave-named tasks that are `Deferred` | 0 |

**Three v1 tasks appear in no wave: `QM-0031` (CPU statistics pass), `QM-0037`
(backend selection), `QM-0153` (rendering-ceiling degradation).** They are named
only in §10's rewiring table or a lane range. The wave table is a sequencing aid,
not an allowlist — these are scheduled by lane and dependencies alone and are not
stranded. Registered for `QM-0002`; the fix to §2 belongs to that task.

The 44/36/9 distribution matches controller §2 exactly. `.plan/README.md`'s
"three start `Ready`" is wrong by six, and its "eight values" prose contradicts its
own nine-row table — both routed to `QM-0002`, both recorded in
`.plan/PLAN_CHANGELOG.md`.

## Disk budget — the binding constraint on concurrency (controller §3.3)

Measured at T+18m, mid-download:

```
free 18 GB · checkpoint 22 of 28.63 GB on disk · projected free after download ≈ 11 GB
per-worktree cargo targets: qm-0140 193M, qm-0012 279M (growing toward ~2 GB each)
controller target/ 2.1 GB
```

**Cap: two concurrent Rust-building worktrees** (`qm-0140` lane R, `qm-0012` lane T)
while the 28.63 GB checkpoint is resident. `qm-0006` (web only), `qm-0002`
(`.plan/` only) and `qm-0160` (scaffolding) build no Rust and are free.

Policy for the rest of the run:
* `df -g .` before creating any worktree; do not create one if the projection
  falls under 10 GB free.
* `cargo clean` and `git worktree remove` each worktree immediately after its
  post-merge verification passes — recovers ~2 GB apiece.
* No shared `CARGO_TARGET_DIR` between concurrently running agents (§8).
* A persistent monitor fires at < 12 GB free.

If disk forces it, the checkpoint may be trimmed **after** `QM-0100`'s inspect
evidence and `fixtures/real-checkpoint-record.json` are recorded — the record is
the durable artifact by the task's own design. Any such trim is recorded as a
limitation in the final report, and `QM-0101`/`QM-0102` then stay `Blocked` with
"checkpoint trimmed to reclaim disk" as the named blocker rather than being
measured against a partial artifact.

## QM-0100 — checkpoint acquired (controller-run download, T+21m)


| Check | Result |
| --- | --- |
| Shards on disk | 8 of 8, plus `model.safetensors.index.json` |
| On-disk bytes | `28_632_144_944` |
| `index.metadata.total_size` | `28_631_568_384` |
| Delta | `+576_560` — **exactly** the sum of the eight safetensors headers (71368, 83792, 83432, 84456, 84112, 84472, 84112, 816). `total_size` counts payload only. Not corruption; verified by reading each shard's leading u64 header length |
| Tensors in `weight_map` | 4659 |
| Expert-keyed tensors | 4320 (`model.layers.N.mlp.experts.K.*`) |
| dtype | bf16 (`torch_dtype: bfloat16`) — exactly decoded per `SRC-016`, not the fp8 that `SRC-014` refuses |
| `git status --short models/` | empty — `.gitignore:22:/models/` confirmed via `git check-ignore -v` |
| sha256 of index.json | `ece1b223efe32f4349d0dfa2a522249ac10bcb89369ed25b222c35175cd90b53` |

**Acquisition is done; the task is not.** `QM-0100` still requires `q-cli inspect`
against the checkpoint, the bytes-read-during-indexing measurement (< 0.1 % of file
size), a `/usr/bin/time -l` peak-RSS figure, `fixtures/real-checkpoint-record.json`,
and a record-consistency test. Those need a Rust worktree and are **held on disk
headroom** until one of the five in-flight worktrees merges and is cleaned. The
long-lead item — the download itself — is secured, which was the reason to start it
in the first minute.

Controller `target/` was `cargo clean`ed at T+20m to free 2.5 GB ahead of the final
shard. It rebuilds at first merge validation.

## Controller process error at T+33m — reviewer dispatched into a live worktree

**What I did wrong.** I dispatched `review-agent-1` against `QM-0006` at head
`26756f1` while `impl-agent-1` was still finishing. For a few minutes two agents
held the same writable worktree, which controller §8 forbids: *"Never let two
agents share a writable worktree."* The trigger was my own check — I saw the
evidence record written and the worktree clean and read that as "implementer
finished", when the agent had not yet reported completion.

**Consequence.** `impl-agent-1` amended its evidence commit: `26756f1` → `b554e4d`.

**Damage assessment.** `git diff 26756f1..b554e4d --name-only` returns exactly one
path: `.plan/evidence/QM-0006.md`. The amend only refined the diffstat wording
inside the evidence record. **No implementation file, no test, and no
configuration differs between the reviewed commit and the current head**, so
controller §4's exception applies — the review is not invalidated, because the
diff under review is provably unchanged. The reviewed SHA is recorded as `26756f1`
and the merged SHA will be recorded separately; both appear in the evidence record
rather than being conflated.

**Correction adopted for the rest of the run.** A review agent is dispatched only
after the implementation agent's own completion notification has arrived — not
merely when its worktree looks clean and its files look written. A clean worktree
proves a commit happened, not that the agent is done.

**Residual risk being watched.** `impl-agent-1` and `review-agent-1` may both have
written `.plan/evidence/QM-0006.md`. Before merging I verify the final record
contains both the implementer's sections and the reviewer's filled-in
`## Independent review` block; if either was clobbered, the reviewer re-records
before the merge proceeds.

## Push to `origin` is not available — local `main` is the integration branch

First attempted at the `QM-0006` merge (T+45m). Verbatim results:

```
$ ssh -T git@github.com
Hi hmthanh! You've successfully authenticated, but GitHub does not provide shell access.
  → exit 0. The SSH identity is `hmthanh`, NOT the `gh` token's `MarkdownOfficial`.

$ git push --verbose origin main            # SSH, BatchMode, 60s cap
Pushing to github.com:quatricmorph/quatricmorph.git
  → exit 124 (timeout). Authenticates, then hangs on pack upload. No rejection
    message is ever emitted; the connection simply stalls.

$ git push https://github.com/quatricmorph/quatricmorph.git main
remote: Permission to quatricmorph/quatricmorph.git denied to MarkdownOfficial.
fatal: unable to access '...': The requested URL returned error: 403
  → exit 1, explicit denial.

$ git rev-parse --short main origin/main
19b7ba0   fe501e5        → local main is 9 commits ahead
```

**Consequence, per controller §1.** Merges land on the controller's local `main`,
which is the integration branch for the remainder of the run. Every dependency
check that reads "on `origin/main`" reads "on the controller's `main`". The final
report states this plainly. Run 1's probe recorded `git push --dry-run` succeeding
for a *new branch* (`qm-capability-probe`) — creating a ref is not the same
permission as updating `main`, so that observation did not predict this, and the
prompt's expectation that SSH push would work is not borne out.

**No retry beyond once per wave.** The run does not halt, does not ask, and does
not treat this as a blocker. There is no `BLOCKED_BY_CREDENTIAL` state.

## The floor-staleness asymmetry — binding rule for every merge after `QM-0001`

`QM-0001` is writing `scripts/baseline.json` as `{rust: 290, web: 115}` from a
worktree cut at `145257b`. `QM-0012` is review-pending at **318 rust (+28)**.
Whichever merges second, the floor is wrong:

* `QM-0001` first → `QM-0012` merges, `main` measures 318, the floor still says
  290. Stale by 28.
* `QM-0012` first → `QM-0001` writes 290 from its now-stale worktree. Stale by 28
  at birth.

**`QM-0001`'s own guard cannot catch this.** It fires when the floor is set
*above* the real count — that is the `999` demonstration. A floor set *below*
reality is silent. That is precisely the asymmetry that let `27 passed` read as
green for as long as `103297d` sat on `main`: a gate that under-reports does not
fail, it just stops protecting anything.

None of `QM-0012`, `QM-0140`, `QM-0100` or `QM-0002` were briefed to touch
`baseline.json`, because the file did not exist when their packets were written.
Controller §6.1 item 6 and §14 make this the controller's responsibility, not
theirs.

**Rule, effective the moment `QM-0001` merges.** For every subsequent merge:

1. Squash-merge the branch.
2. Re-measure on the **merged** `main`: `cargo test --workspace` and, if any web
   file changed, `cd apps/web && npx vitest run`.
3. Write those **exact measured numbers** into `scripts/baseline.json` — not
   "≥ previous", the measured value — **in the same squash commit**.
4. The floor still may only rise. A merge that would lower it is rejected.
5. At the end of the run, assert `scripts/baseline.json` equals a fresh
   measurement exactly, and record that comparison in the final report.

Floor updates therefore serialise through the controller, one merge at a time,
which §14 already requires for this file.

---

# Run 3 — 2026-08-04T23:49:41Z → deadline 2026-08-05T04:49:41Z (5h00m)

Controller checkout: `/Users/thanh/Quatricmorph/Quatricmorph`, branch `main`.
Integration branch: **local `main`** — push to `origin` is unavailable (Run 2
verified `remote: Permission to quatricmorph/quatricmorph.git denied to
MarkdownOfficial`, and SSH-as-`hmthanh` stalls on pack upload). Every "on
`origin/main`" dependency check reads "on the controller's local `main`".

## What Run 3 inherited, verified against Git rather than the registry

| Branch | Worktree | Real state at reconstruction | Disposition |
| --- | --- | --- | --- |
| `task/qm-0001-baseline-verification` | `qm-0001` | At `145257b`, an **ancestor of `main`**, zero commits ahead, clean. Run 2's claim that it "is writing `scripts/baseline.json`" is not borne out — nothing was committed | Branch **reset to `793e122`**, re-dispatched |
| `task/qm-0002-plan-repo-reconciliation` | `qm-0002` | Zero commits ahead; **uncommitted** edits to three `TASK.md` files + `apps/web/package-lock.json`, untracked `.plan/tools/` | Re-dispatched as a **recovery** task: audit the WIP, don't inherit it |
| `task/qm-0012-config-model-metadata` | `qm-0012` | **4 commits, clean**, 13 files / +1545. Real, complete-looking work | Dispatched to **independent review** |
| `task/qm-0100-real-checkpoint-acquisition` | `qm-0100` | 1 commit; `fixtures/real-checkpoint-record.json` describes **Qwen1.5-MoE-A2.7B, 28.63 GB, 8 shards** — a checkpoint no longer on disk. Based at `0ef6ec5`, so squash-merging it would **revert the owner's `579107f`** | **Discarded.** Branch deleted, worktree removed, task rewritten, re-dispatched as `task/qm-0100-real-checkpoint-verification` |
| `task/qm-0140-manifest-schema` | `qm-0140` | Zero commits ahead; **uncommitted** `crates/q-report/` (8 files), `schemas/diagnostics/`, `Cargo.toml`/`Cargo.lock` workspace-member edit, and a 567-line draft evidence record | Re-dispatched as a **recovery** task |
| `task/qm-0160-design-partner-outreach` | `qm-0160` | **1 commit, clean**, 14 files / +1614 | Dispatched to **independent review**, with a mandatory fabrication audit |

Only `qm-0100`'s branch touched an owner-amended path. The other five are clean of
`.plan/MASTER_PLAN.md`, `.plan/ORCHESTRATION_STATE.md`, and
`.plan/tasks/QM-0100-real-checkpoint-acquisition/TASK.md` — verified per-branch with
`git diff main...<branch> --name-only`.

## The owner's re-scope supersedes the controller prompt's premise

Commit `579107f` (`Thanh Hoang-Minh`, the repository owner) rewrote
`.plan/MASTER_PLAN.md` §4 and `QM-0100`'s TASK.md to direct the project at
`models/distilbert-distilgpt2` and to **"ignore any larger MoE checkpoints"**, and
deleted the 28.63 GB checkpoint Run 2 had downloaded. The controller prompt's §3.3
premise — that `QM-0100` is a multi-hour download and the first task in the plan
because of its lead time — **no longer holds**. There is no long-lead item in Run 3,
so nothing is sequenced behind one. Full record and the coverage given up:
`.plan/PLAN_CHANGELOG.md`, Run 3 section.

## Baseline, re-measured on `main` @ `579107f`

| Gate | Command | Result |
| --- | --- | --- |
| Rust tests | `cargo test --workspace` | **290 passed; 0 failed**, exit 0 — matches `STATUS.md` exactly |
| Web tests | `cd apps/web && npx vitest run` | **115 passed across 13 files**, exit 0 — `STATUS.md` still says 101; **stale by 14** |

`scripts/` does not exist, so **`scripts/baseline.json` does not exist and no floor
guard is in force.** `QM-0001` is dispatched first for exactly that reason. Every
task merged before `QM-0001` lands records "no floor guard in force at merge time;
counts recorded in evidence only" in its evidence record.

## Disk

Measured 21 GB free at T+0 with 96 % capacity — far below the prompt's stated ~51 GB.
By T+30m it read **52 GB free**: the owner's deletion of the 28.63 GB checkpoint had
been held as APFS purgeable space and was reclaimed. `df -h .` is still run before
every worktree creation per §3.3; the concurrency cap on Rust-building worktrees is
lifted from three to what the lane structure needs, not to unbounded.

## Wave 0/1 dispatch — eight agents

| Task | Agent | Role | Worktree | Base |
| --- | --- | --- | --- | --- |
| QM-0012 | `review-agent-1` | Independent review | `qm-0012` | head `6f2d5eb` |
| QM-0160 | `review-agent-2` | Independent review + fabrication audit | `qm-0160` | head `16ad32b` |
| QM-0001 | `impl-agent-1` | Implement (floor guard, measured 290/115) | `qm-0001` | `793e122` |
| QM-0093 | `impl-agent-2` | Implement (docs-only exempt class) | `qm-0093` | `793e122` |
| QM-0167 | `impl-agent-3` | Implement (docs-only exempt class) | `qm-0167` | `793e122` |
| QM-0100 | `impl-agent-4` | Implement (re-scoped to distilgpt2) | `qm-0100` | `04991e9` |
| QM-0140 | `impl-agent-5` | Recover uncommitted WIP + finish | `qm-0140` | pre-`579107f` |
| QM-0002 | `impl-agent-6` | Recover uncommitted WIP + finish | `qm-0002` | pre-`579107f` |

## Correction at T+78m — **push to `origin` works.** Run 2's finding is superseded

Run 2 recorded, with verbatim output, that pushing `main` was impossible: SSH
authenticated as `hmthanh` then stalled on pack upload (exit 124), and HTTPS
returned `remote: Permission to quatricmorph/quatricmorph.git denied to
MarkdownOfficial` (403). On that basis Run 2 declared local `main` the integration
branch and the controller prompt's §1 encodes the same expectation.

At the first Run 3 merge the push **succeeded**:

```
$ git push origin main
To github.com:quatricmorph/quatricmorph.git
   f4a07ef..4e0e85c  main -> main            exit 0
```

`origin/main` had independently advanced to `f4a07ef` (owner commits), and the
controller's `main` fast-forwarded it cleanly. **Merges in Run 3 reach `origin`.**
Whatever blocked Run 2 — a transient stall, or a permission the owner has since
changed — no longer applies. The final report says merges were pushed, not that
they were stranded locally.

The push is still attempted **once per wave**, and a rejection would still be
recorded and routed around rather than halting the run.

## Merges — Wave 0/1

| Task | Lane | Reviewed SHA | Reviewer | Verdict | Merge commit | On origin | Post-merge |
| --- | --- | --- | --- | --- | --- | --- | --- |
| QM-0160 | V | `16ad32b` | `review-agent-2` | APPROVED (scaffolding only; task stays `Blocked`) | `e61d28e` | yes | rust 290 unchanged |
| QM-0012 | T | `6f2d5eb` | `review-agent-1` | APPROVED | `4e0e85c` | yes | **rust 318 passed; 0 failed**, exit 0 |

Both merge commits confirmed reachable from `main` with `git merge-base
--is-ancestor`. Neither branch touched an owner-amended path.

**Floor now: rust 318 / web 115.** `scripts/baseline.json` still does not exist —
`QM-0001` is in flight and must write the *measured* value at its own merge time,
not the 290 its worktree was cut against. This is the floor-staleness asymmetry:
a floor set below reality does not fail, it silently stops protecting anything.

## Controller error at T+105m — I over-parallelized the Rust-building lanes

**What I did wrong.** I dispatched seven agents concurrently, five of which run
`cargo build --workspace --all-targets` and `cargo clippy --workspace --all-targets`
in **separate worktrees with separate `target/` directories**. Each spawns rustc
jobs sized to the whole machine, so the effective job count was roughly
5 × ncpu against 11 cores.

**Measured consequence:**

```
$ uptime                 load averages: 102.75  62.71  38.71
$ top -l 1 -n 0          CPU usage: 49.5% user, 50.94% sys, 0.0% idle
                         Processes: 1180 total, 23 running
                         MemRegions: 12G resident
$ sysctl -n hw.ncpu      11
```

**50 % system time is the tell** — that is contention and page-fault overhead, not
useful work. Every agent is now slower than it would have been had I serialised
two of them.

**Why the rule I was following did not save me.** Controller §3.3 caps concurrent
Rust worktrees for a **disk** reason, and disk turned out to be fine (54 GB free
after the checkpoint deletion was reclaimed). I lifted the cap on the disk
argument and did not replace it with a **CPU** argument. §14's "when uncertain,
serialize. Correctness, isolation, and reviewability outrank maximum concurrency"
is the rule that actually applied, and I did not apply it.

**Correction adopted for the rest of the run.** The concurrency cap on
Rust-building lanes is **CPU-bound, not disk-bound**: at most **three** agents
running `cargo build`/`clippy`/`test` at once, regardless of free disk. Lanes that
build no Rust — plan-only, docs-only, ADR promotion, web-only — do not count
against that cap and stay parallel. No new task is dispatched while
`uptime` reports a 1-minute load average above ~20.

**Not corrected by killing anything.** All seven agents are mid-task with
uncommitted work; killing one would discard real work and leave a worktree in the
same half-finished state Run 3 spent its first hour recovering from. They are
allowed to drain.
