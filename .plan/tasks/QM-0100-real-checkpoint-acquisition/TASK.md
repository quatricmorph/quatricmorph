# QM-0100 — Verify the real local checkpoint indexes from headers alone

## Status

In Progress

**Re-scoped by the repository owner in commit `579107f`.** The original task
acquired a ≥ 24 GB sharded MoE checkpoint over a multi-hour download. The owner
removed that requirement, deleted the 28.63 GB Qwen1.5-MoE-A2.7B checkpoint from
disk, and directed the project at the checkpoint already present locally:

> "Focus on small and simple version first, please using model already download
> inside `./models/distilbert-distilgpt2`, and ignore any larger MoE checkpoints"

> "Only using model inside `distilbert-distilgpt2` instead of using large MoE
> checkpoints is a **temporary** concession to the machine's disk. Only focus on
> first MVP version to development."

There is therefore **no download and no long lead time**. See
`.plan/PLAN_CHANGELOG.md` for the full record, including what coverage this
concession gives up.

## Phase

Phase 10 — Out-of-core proof on a real checkpoint

## Objective

Verify that the real, open-weight SafeTensors checkpoint already on disk at
`models/distilbert-distilgpt2/` indexes **from its header alone**, and record its
identity — so that every performance claim v1 makes has an artifact behind it,
and so the numbers stay auditable after the checkpoint is deleted.

## Repository Evidence

* `models/distilbert-distilgpt2/model.safetensors` — **the checkpoint this task
  now targets.** Measured directly from the file at Run 3 start:

  | Property | Measured value |
  | --- | --- |
  | Bytes on disk | `352_824_413` |
  | SafeTensors header length | `8_277` bytes (leading `u64` = `0x2055`) |
  | Header as a fraction of the file | **0.00235 %** — two orders of magnitude under the 0.1 % ceiling |
  | Tensor count | 82 |
  | dtype histogram | **`F32`: 82** — *not* bf16/f16 |
  | Tensor ranks | rank 1 × 50, rank 2 × 26, **rank 4 × 6** |
  | Shard count | **1 — single file, no `model.safetensors.index.json`** |
  | Architecture | `gpt2` / `GPT2LMHeadModel`, `n_layer: 6` — GPT-2 family, **no experts** |
  | SafeTensors metadata | `{"format": "pt"}` |

  Reproduce with: `python3 -c "import json,struct,collections;f=open('models/distilbert-distilgpt2/model.safetensors','rb');n=struct.unpack('<Q',f.read(8))[0];h=json.loads(f.read(n));h.pop('__metadata__',None);print(n,len(h),collections.Counter(v['dtype'] for v in h.values()),collections.Counter(len(v['shape']) for v in h.values()))"`

* **The six rank-4 tensors are the important find.** `transformer.h.N.attn.bias`
  has shape `[1, 1, 1024, 1024]` for `N` in `0..6`. `ADR-010` (Accepted) caps
  supported rank at 3 and requires rank > 3 to be **refused rather than
  flattened**. This checkpoint therefore exercises the ADR-010 refusal path
  against real data, not a synthetic fixture — that is a required test case
  below, not an optional one.
* `fixtures/tiny-llama-2shard`, `fixtures/tiny-llama-single` — 1.2 MB synthetic.
* `crates/q-catalog/tests/trillion_scale_manifest.rs` — `CAT-006`. Indexes a
  synthetic 10¹² manifest in 35.7 MB peak. **Metadata only; opens no artifact.**
* `crates/q-safetensors/src/index.rs` — `model.safetensors.index.json` handling,
  `ingests_a_sharded_checkpoint`.
* `crates/q-source/src/local.rs` — mmap range reads, `path_traversal_is_refused`.
* `crates/q-source/src/budget.rs` — named budgets, already enforced.

## Requirements Covered

`SRC-020` (new), `V1-01`, `V1-02`. Enables `PERF-002`, `V1-03`…`V1-05`, and every
benchmark in `QM-0102`.

## Dependencies

None.

## Blocks

`QM-0101`, `QM-0102`, `QM-0122` (real-data verification), `QM-0125`, `QM-0161`.

## Parallelization

Runs alone in Lane P and blocks no other lane. With the download removed by the
owner's re-scope, this task is now **short** rather than the plan's longest lead
time — it is no longer a reason to sequence anything. `QM-0001`, `QM-0002`,
`QM-0093`, `QM-0140`, `QM-0160` and `QM-0167` proceed beside it.

## Program Boundary

`models/` (gitignored), `fixtures/` (the manifest record only), `crates/q-cli`.

## Scope

* Verify the checkpoint already on disk at `models/distilbert-distilgpt2/` — the
  owner has already selected it; there is nothing to choose and nothing to download.
* Measure its identity from the file: byte size, header length, tensor count,
  dtype histogram, rank histogram, architecture.
* Confirm it indexes through the existing ingestion path, reading headers only.
* Confirm rank > 3 is refused rather than flattened (ADR-010).
* Record a small, committed **metadata record** describing it, so the plan's
  numbers stay checkable after the checkpoint is deleted.

## Out of Scope

Streaming measurement (`QM-0101`) · statistics · quantisation · any modification
of the checkpoint · committing the weights.

## Selection

**Selection is already decided by the repository owner — there is nothing to
choose.** The checkpoint is `models/distilbert-distilgpt2/`. The table below
records what it actually is, and — in the last column — what the original
constraint wanted and is no longer getting. That gap is the honest cost of the
MVP concession, and it must appear in the evidence record and the final report.

| Constraint | Original requirement | What distilgpt2 actually is | Coverage given up |
| --- | --- | --- | --- |
| Format | SafeTensors, **sharded**, with `model.safetensors.index.json` | SafeTensors, **single file, no index JSON** | **The sharded read path is no longer exercised by this task.** Multi-shard attribution stays covered only by `fixtures/tiny-llama-2shard` and `crates/q-safetensors`'s `ingests_a_sharded_checkpoint`. v1 must not claim real-checkpoint shard coverage |
| Size on disk | ≥ 24 GB, ideally 28–40 GB | **352,824,413 bytes (~337 MiB)** | The ≥ 24 GB scale claim is not established on real data. `crates/q-catalog/tests/trillion_scale_manifest.rs` remains **metadata-only and opens no artifact** |
| dtype | bf16 or f16 | **F32, all 82 tensors** | The bf16 exact-decode path (`SRC-016`) is not exercised by this checkpoint; it stays covered by fixtures. `SRC-014`'s fp8 refusal is untouched either way |
| Architecture | Qwen- or Llama-family, ideally MoE | **`gpt2` / `GPT2LMHeadModel`, 6 layers, no experts** | **MoE expert-keyed aggregation has no real-checkpoint fixture.** `QM-0123` is provable only against generated fixtures, and the deferred `MOE-001` seam gains nothing here |
| Rank | (not previously constrained) | rank 1 × 50, rank 2 × 26, **rank 4 × 6** | **Gains** coverage: the ADR-010 rank > 3 refusal is now exercised against real data |
| Licence | Open weights, redistributable inspection | Verify from `models/distilbert-distilgpt2/README.md` and record it | — |

**`models/` is gitignored, so no weights are redistributed by this repository.**
State that precisely rather than implying the checkpoint ships with the source.

### What this means for gate G1

`.plan/MASTER_PLAN.md` §4 still requires peak RSS ≤ 1.25 × C while streaming a
checkpoint **N ≥ 100 ×** larger than C. Against a 352,824,413-byte file, N ≥ 100
forces a residency ceiling **C ≤ ~3.4 MB**. The structural property survives and
is still measurable with `/usr/bin/time -l`; the headline number does not. **Record
the ratio you actually measured against the file you actually measured.** Do not
restate the plan's 30 GB-era figures as if they were observed.

## Files Expected to Change

* `.gitignore` — confirm `models/` stays ignored
* `crates/q-cli/src/main.rs` — only if `inspect` needs a bytes-read counter it
  does not already have

## Files Expected to Add

* `models/distilbert-distilgpt2/` — **already present, not committed, gitignored**
* `fixtures/real-checkpoint-record.json` — committed: name, source URI, revision,
  licence, byte size, shard count, tensor count, parameter count, dtype histogram,
  rank histogram, architecture, `has_experts`, and the header hash. Every field
  measured; unmeasurable fields `null` or `"not verified"`.
* `tests/tests/real_checkpoint_record.rs` — the record-consistency test and the
  ADR-010 rank-4 refusal test

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
// fixtures/real-checkpoint-record.json — every field is a MEASURED value
{
  "name": "distilbert-distilgpt2",
  "source_uri": "…",           // from models/distilbert-distilgpt2/README.md, or null if the tree does not record it
  "revision": "…",             // or null — do NOT invent a hash
  "licence": "…",              // read from README.md, or "not verified"
  "bytes_on_disk": 352824413,
  "shard_count": 1,            // single file; there is no index.json
  "tensor_count": 82,
  "parameter_count": 0,        // computed from the header's shapes
  "dtypes": { "F32": 0 },      // F32 only — NOT BF16
  "rank_histogram": { "1": 50, "2": 26, "4": 6 },
  "architecture": "gpt2",
  "has_experts": false,
  "sha256_of_header": "…"      // the header bytes; there is no index.json to hash
}
```

**`sha256_of_index_json` is renamed to `sha256_of_header`** because this checkpoint
has no index JSON. Hash the 8,277 header bytes instead, and say in the record which
bytes were hashed. Any field you cannot measure from a file in the tree is `null`
or `"not verified"` — **never a plausible-looking invented value.**

This file is the durable part of the task. The checkpoint may be deleted to
reclaim disk; the record keeps the plan's numbers auditable.

## Memory and Performance Constraints

Indexing must read **headers only**. Expected bytes read here: **8,277 header
bytes against 352,824,413 file bytes = ~0.00235 %**, comfortably under the 0.1 %
ceiling. `SRC-007` already asserts the property on fixtures; this task asserts it
on a real artifact — **but at ~337 MiB, not "at scale."** The scale claim the
original task carried is not established by this checkpoint and must not be
restated as if it were.

Peak allocation during indexing is bounded by the existing metadata budget and
must not scale with checkpoint size. Measure it on the **release binary** with
`/usr/bin/time -l`, and report the measured ceiling `C` and ratio
`N = 352_824_413 / C` rather than the plan's 30 GB-era figures.

## Implementation Plan

**There is no download step.** The checkpoint is already on disk.

1. Confirm the checkpoint is present and measure it: `ls -l`, byte size, and the
   header facts in `## Repository Evidence` above. Re-derive them yourself rather
   than copying the table.
2. `cargo run -p q-cli -- inspect models/distilbert-distilgpt2` — confirm the
   tensor list, dtypes, and architecture resolution. Single-file resolution is the
   expected path here; there is no index JSON to resolve.
3. Measure bytes read during indexing and assert the ratio to file size is
   < 0.1 %. The header is 8,277 of 352,824,413 bytes, so the expected ratio is
   ~0.00235 % — but **measure it, do not assert the arithmetic**.
4. Measure peak RSS with `/usr/bin/time -l` and record the ceiling `C` it implies
   and the resulting ratio `N = file_size / C`. Report the measured N; do not
   claim the plan's N ≥ 100 unless your measurement actually reaches it.
5. Confirm the six rank-4 tensors are **refused, not flattened** (ADR-010).
6. Write `fixtures/real-checkpoint-record.json` from measured values only.
7. Confirm `models/` is gitignored (`git check-ignore -v models/`) and that
   `git status --short models/` is empty.

## Error Handling

* A dtype the reader refuses (fp8) → that is `SRC-014` working. Do not widen dtype
  support in this task. (This checkpoint is F32 throughout, so the path is not
  exercised here — say so.)
* **Rank > 3 → refused with context, never flattened (ADR-010).** This checkpoint
  contains six rank-4 tensors, so this is a live path, not a hypothetical.
* Architecture unresolved → acceptable; `NSIR-001` keeps it `unknown` and the
  record says so. Do not add a resolver here.
* A truncated or corrupt file → refused with context (`SRC-013`, `SRC-015`).
  Test this against a **copy**; never truncate the real checkpoint.

## Acceptance Criteria

1. `models/distilbert-distilgpt2/model.safetensors` is present, and its measured
   byte size, header length, tensor count, dtype histogram, and rank histogram are
   recorded from the file itself.
2. `q-cli inspect` completes and lists its tensors.
3. Bytes read during indexing < 0.1 % of the checkpoint's size, **measured**, with
   the measurement method recorded.
4. Peak RSS during indexing is measured with `/usr/bin/time -l`, and the implied
   ceiling `C` and ratio `N = file_size / C` are reported **as measured**.
5. The six rank-4 tensors are refused rather than flattened, proven by a test
   whose name asserts it (ADR-010).
6. A truncated **copy** is refused with context (`SRC-013`, `SRC-015`).
7. `fixtures/real-checkpoint-record.json` is committed and every field in it is a
   measured value.
8. Nothing from `models/` is staged for commit; `models/` is confirmed gitignored.
9. The checkpoint's licence is read from `models/distilbert-distilgpt2/README.md`
   and recorded; if it cannot be determined from a file in the tree, it is recorded
   as **"not verified"** rather than guessed.
10. The evidence record states plainly which coverage this checkpoint does **not**
    provide — sharded path, bf16 decode, MoE experts, and ≥ 24 GB scale — per the
    Selection table's last column.

## Verification Plan

**Automated** — a test that reads the record and asserts internal consistency
(parameter count vs. dtype histogram vs. byte size).
**Manual** — `du -sh`, the `inspect` output, and the bytes-read counter, all
pasted into the evidence.

## Suggested Commands

```bash
ls -l models/distilbert-distilgpt2/
stat -f%z models/distilbert-distilgpt2/model.safetensors     # 352824413
cargo run -p q-cli -- inspect models/distilbert-distilgpt2
cargo build --release -p q-cli                                # build FIRST, so the
/usr/bin/time -l ./target/release/q-cli inspect models/distilbert-distilgpt2
git check-ignore -v models/
git status --short models/                                    # must be empty
```

**Measure peak RSS on the built binary, not on `cargo run`** — timing `cargo run`
measures the build tool's residency, not the program's, and would silently inflate
every G1 figure.

## Test Cases

| Input | Expected |
| --- | --- |
| `inspect` on `models/distilbert-distilgpt2` | Lists 82 tensors; single-file resolution; architecture resolved as GPT-2 family or left `unknown` per `NSIR-001` — never guessed |
| Bytes-read counter during indexing | < 0.1 % of file size, **measured** (expected ~0.00235 %) |
| Peak RSS during indexing, `/usr/bin/time -l` | Bounded by the metadata budget and **not scaling with file size**; report measured `C` and `N = 352_824_413 / C` |
| **A rank-4 tensor (`transformer.h.0.attn.bias`, `[1,1,1024,1024]`)** | **Refused with context, never flattened (ADR-010)** |
| A deliberately truncated **copy** of the checkpoint | Refused with context (`SRC-013`, `SRC-015`). Never truncate the original |
| A file whose declared header length exceeds the file size | Refused before allocating |
| `fixtures/real-checkpoint-record.json` internal consistency | Record's tensor count, dtype histogram, parameter count and byte size agree with each other and with the file |
| `git status --short models/` | Empty |
| `git check-ignore -v models/` | Reports the ignoring rule |

Note the removed row: **"correct shard attribution" is no longer testable by this
task**, because the checkpoint is single-file. It stays covered by
`fixtures/tiny-llama-2shard`.

## Risks

| Risk | Mitigation |
| --- | --- |
| **The MVP concession is silently forgotten and v1 claims ≥ 24 GB / sharded / MoE / bf16 coverage it does not have** | The Selection table's last column, acceptance criterion 10, and the final report all state the gap explicitly. This is the headline risk of the re-scope |
| The record file drifts from the checkpoint after the checkpoint is deleted | Every field is measured, and the consistency test asserts the record against itself |
| A rank-4 tensor is flattened to make `inspect` succeed | Forbidden by ADR-010 and asserted by a named test. This checkpoint makes it a live path |
| Someone commits the weights | `git check-ignore` and empty `git status models/` are acceptance criteria |
| A licence is guessed rather than read | Criterion 9 requires "not verified" over a guess |

## Completion Evidence

* The measured byte size, header length, tensor count, dtype histogram and rank
  histogram, each re-derived from the file rather than copied from this task.
* Full `q-cli inspect` output (excerpt if long, with the totals intact).
* The bytes-read measurement and its ratio to file size.
* `/usr/bin/time -l` peak RSS for the indexing run, taken on the **release binary**,
  plus the implied ceiling `C` and the measured ratio `N = file_size / C`.
* The ADR-010 rank-4 refusal, with the refusal message.
* The truncated-copy refusal, with its message.
* The committed record file.
* The checkpoint's licence as read from a file in the tree, or "not verified".
* **An explicit statement of the coverage this checkpoint does not provide** —
  sharded path, bf16 exact decode, MoE expert-keyed aggregation, and ≥ 24 GB scale.
* `git check-ignore -v models/` and an empty `git status --short models/`.

## Orchestration

| Field | Value |
| --- | --- |
| Controller state | **Awaiting Independent Review** |
| Lane | P |
| Wave | 0 |
| Branch | `task/qm-0100-real-checkpoint-verification` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0100` |
| Base commit | `04991e9` |
| Commits on branch | `e64a7a3` implementation · `10bf3bb` commit-SHA row · `61ad753` verified-degradation evidence · `4th` review-fix (streamed truncation fixture, module-doc correction, env-var semantics, recorded-fraction assertion) |
| Head commit | **`git rev-parse HEAD` is authoritative** — a commit cannot name its own SHA. `git log --oneline main..HEAD` lists all of them; merge path L squashes them into one |
| Agent | `impl-agent-4` |
| Evidence | `.plan/evidence/QM-0100.md` |
| Merge path | L (local squash) |
| Tests added | **19**, all in `tests/tests/real_checkpoint_record.rs` |
| Floor change | **290 → 309** (`cargo test --workspace`, exit 0, measured in this worktree at base `04991e9`) |
| Production code changed | **none** — `q-cli` already carried the bytes-read counter; `.gitignore` needed no edit |

**Carried findings for the controller**

* The binary is **`q`**, not `q-cli` (`[[bin]] name = "q"`). This task's
  `## Suggested Commands` block is wrong and should be corrected by whoever owns
  it; every measurement here used `./target/release/q`.
* `git check-ignore -v models/` **exits 1**, because 9 files under `models/` are
  tracked and `check-ignore` skips index-tracked paths. Acceptance criterion 8
  is met via `git check-ignore --no-index -v models/` and the file-path form,
  both exit 0, plus an empty `git status --short models/`. Recorded, not
  substituted.
* ADR-010's `bindAxes()`/`GRID-007` refusal **exists nowhere in the tree** and is
  owned by `QM-0061` / `QM-0040` / `QM-0004`. Not implemented here (out of
  declared file scope). Logged in `.plan/PLAN_CHANGELOG.md`; the six real rank-4
  tensors are now a real-data fixture those tasks can use.
* No floor guard was in force at implementation time — `scripts/baseline.json`
  does not exist on this base and `QM-0001` owns it. Counts are recorded in the
  evidence record only.
