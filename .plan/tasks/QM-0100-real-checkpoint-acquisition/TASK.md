# QM-0100 — Acquire and verify a real ≥ 24 GB checkpoint

## Status

Ready

**Start this first.** It is the longest wall-clock lead time in the plan and
nothing about it compresses by working harder.

## Phase

Phase 10 — Out-of-core proof on a real checkpoint

## Objective

Put a real, sharded, open-weight SafeTensors checkpoint of **≥ 24 GB** on disk,
verify it indexes from headers alone, and record its identity — so that every
performance claim v1 makes has an artifact behind it.

## Repository Evidence

* `models/distilbert-distilgpt2/model.safetensors` — 339 MB, single-file,
  GPT-2 family, resolved by the generic resolver. The largest real artifact in
  the tree, and roughly 1 % of what this task needs.
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

Runs alone in Lane P and blocks no other lane while downloading. `QM-0001`,
`QM-0002` and `QM-0160` proceed beside it.

## Program Boundary

`models/` (gitignored), `fixtures/` (the manifest record only), `crates/q-cli`.

## Scope

* Select a checkpoint meeting the criteria in "Selection" below.
* Download it, verify integrity, and record source URI, revision hash, and size.
* Confirm it indexes through the existing ingestion path.
* Record a small, committed **metadata record** describing it, so the plan's
  numbers stay checkable after the checkpoint is deleted.

## Out of Scope

Streaming measurement (`QM-0101`) · statistics · quantisation · any modification
of the checkpoint · committing the weights.

## Selection

Hard constraints, in order:

| Constraint | Value | Reason |
| --- | --- | --- |
| Format | SafeTensors, **sharded**, with `model.safetensors.index.json` | Exercises the sharded path; single-file would not |
| Size on disk | **≥ 24 GB**, ideally 28–40 GB | The strategy's reference ceiling; the upper bound is this machine's 51 GB free disk |
| dtype | bf16 or f16 | Already exactly decoded (`SRC-016`); avoids fp8, which `SRC-014` refuses on purpose |
| Architecture | Qwen- or Llama-family, ideally with MoE experts | `NSIR-002`/`NSIR-003` resolve them; MoE exercises expert-keyed aggregation for free |
| Licence | Open weights, redistributable inspection | A private checkpoint cannot appear in evidence |

**Preference for an MoE checkpoint**: expert-keyed aggregation (`QM-0123`) and
the deferred `MOE-001` seam both become testable at no extra cost.

Record the choice and the rejected alternatives in the completion evidence — a
later reader needs to know what "a real checkpoint" meant here.

## Files Expected to Change

* `.gitignore` — confirm `models/` stays ignored
* `crates/q-cli/src/main.rs` — only if `inspect` needs a bytes-read counter it
  does not already have

## Files Expected to Add

* `models/<checkpoint>/` — **not committed**
* `fixtures/real-checkpoint-record.json` — committed: name, source URI, revision
  hash, byte size, shard count, tensor count, parameter count, dtype histogram

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
// fixtures/real-checkpoint-record.json
{
  "name": "…", "source_uri": "…", "revision": "…",
  "bytes_on_disk": 0, "shard_count": 0, "tensor_count": 0,
  "parameter_count": 0, "dtypes": { "BF16": 0 },
  "architecture": "qwen", "has_experts": true,
  "sha256_of_index_json": "…"
}
```

This file is the durable part of the task. The checkpoint may be deleted to
reclaim disk; the record keeps the plan's numbers auditable.

## Memory and Performance Constraints

Indexing must read **headers only**. Expected bytes read: a few MB against tens
of GB — under 0.1 % of file size, and `SRC-007` already asserts the property on
fixtures. This task asserts it at scale.

Peak allocation during indexing is bounded by the existing metadata budget and
must not scale with checkpoint size.

## Implementation Plan

1. Check free disk **before** downloading: at least 1.3 × the checkpoint size.
2. Select per the table above; record the alternatives considered.
3. Download with a resumable client. Verify per-shard checksums where the source
   publishes them.
4. `cargo run -p q-cli -- inspect models/<checkpoint>` — confirm shard resolution,
   tensor count, dtypes, and architecture resolution.
5. Record bytes read during indexing; assert < 0.1 % of file size.
6. Write `fixtures/real-checkpoint-record.json`.
7. Confirm `models/` is gitignored and nothing large is staged.

## Error Handling

* Insufficient disk → stop before downloading, report the shortfall. Do not
  partially download and leave the machine full.
* Checksum mismatch on a shard → re-download that shard; never proceed on a
  corrupt artifact.
* A dtype the reader refuses (fp8) → that is `SRC-014` working. Choose a
  different checkpoint rather than widening dtype support in this task.
* Architecture unresolved → acceptable; `NSIR-001` keeps it `unknown`, and the
  record says so. Do not add a resolver here.

## Acceptance Criteria

1. A SafeTensors checkpoint ≥ 24 GB is on disk, sharded, with an index JSON.
2. `q-cli inspect` completes and lists its tensors.
3. Bytes read during indexing < 0.1 % of the checkpoint's size, **measured**.
4. Peak allocation during indexing is within the existing metadata budget.
5. `fixtures/real-checkpoint-record.json` is committed and accurate.
6. Nothing from `models/` is staged for commit.
7. The checkpoint's licence permits its use as published evidence.

## Verification Plan

**Automated** — a test that reads the record and asserts internal consistency
(parameter count vs. dtype histogram vs. byte size).
**Manual** — `du -sh`, the `inspect` output, and the bytes-read counter, all
pasted into the evidence.

## Suggested Commands

```bash
df -h .                                              # before anything
du -sh models/<checkpoint>
cargo run -p q-cli -- inspect models/<checkpoint>
/usr/bin/time -l cargo run --release -p q-cli -- inspect models/<checkpoint>
git status --short models/                           # must be empty
```

## Test Cases

| Input | Expected |
| --- | --- |
| `inspect` on the real checkpoint | Tensor list, correct shard attribution |
| Bytes-read counter | < 0.1 % of file size |
| Peak RSS during indexing | Within the metadata budget, unrelated to file size |
| A deliberately truncated shard copy | Refused with context (`SRC-013`, `SRC-015`) |
| `git status models/` | Empty |

## Risks

| Risk | Mitigation |
| --- | --- |
| Disk exhaustion mid-download | Check for 1.3 × headroom first; prefer 28–32 GB over 40 GB |
| The download is slow enough to stall the plan | It blocks only Lane P; `QM-0001`, `QM-0002`, `QM-0160`, `QM-0140` all proceed |
| The chosen checkpoint uses a refused dtype | Verify the dtype histogram from the index **before** downloading the shards |
| A licence that forbids publishing results | Check before downloading; this is evidence, not private use |
| Someone commits 30 GB of weights | `.gitignore` check is an acceptance criterion, not an afterthought |

## Completion Evidence

* `df -h` before and after.
* `du -sh` of the checkpoint.
* Full `q-cli inspect` output (excerpt if long, with the totals intact).
* The bytes-read measurement and its ratio to file size.
* `/usr/bin/time -l` peak RSS for the indexing run.
* The committed record file.
* The checkpoint's source URI, revision, and licence.
* The alternatives considered and why they were rejected.
