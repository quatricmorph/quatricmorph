# QM-0003 — LOD-capable generated fixture with golden values

## Status

Ready

## Phase

Phase 00 — Repository baseline and shared contracts

## Objective

Extend the fixture generator to produce a checkpoint containing at least one
4096×4096 tensor, with golden values from Python `safetensors`, generated on
demand and **not committed**.

## Repository Evidence

* `fixtures/tiny-llama-2shard/config.json` — `hidden_size: 48`,
  `intermediate_size: 64`, `num_hidden_layers: 12`, `vocab_size: 64`,
  `torch_dtype: float32`.
* `golden.json` — 111 tensors, 2 shards, **1 196 736 bytes total**. The largest
  tensor is `model.layers.10.self_attn.q_proj.weight` at `[128, 48]` = 6 144
  elements.
* `fixtures/generate_fixtures.py` exists; CI's `fixtures` job regenerates and
  runs `git diff --exit-code` to prove reproducibility.
* `golden.json` cites `safetensors==0.8.0`, `numpy 2.5.1`.

**A `[128, 48]` tensor cannot be decomposed into 256×256 blocks, cannot produce a
five-level LOD pyramid, and cannot demonstrate that zooming out avoids exact
reads — there is nothing to zoom out from.**

## Requirements Covered

`SRC-019`; enables `MVP-13`…`MVP-15`, `MVP-18`…`MVP-21`.

## Dependencies

None.

## Blocks

`QM-0030`, `QM-0031`, `QM-0040`, `QM-0041`, `QM-0042`, `QM-0044`, `QM-0051`,
`QM-0080`, `QM-0084`.

## Parallelization

Fully parallel with `QM-0001` and `QM-0002`. Touches only `fixtures/`.

## Program Boundary

`fixtures/` and `.gitignore`. No crate changes.

## Scope

* Add a `tiny-llama-large` fixture: 4 layers, `hidden_size = 4096`, so each
  `q_proj.weight` is `[4096, 4096]` f32 = **64 MiB**, total ≈ 400 MB.
* Generate golden scalars, golden slices, and golden block statistics.
* **Do not commit the `.safetensors` files.** Commit `golden.json` and
  `config.json` only.
* Add `.gitignore` entries and a guard that fails clearly when a test needs the
  fixture and it is absent.
* Include a bf16 tensor and an f16 tensor, since `SRC-016` covers both.

## Out of Scope

Downloading a real model · changing the existing fixtures · any test that needs
a network.

## Files Expected to Change

* `fixtures/generate_fixtures.py`
* `.gitignore`
* `.github/workflows/build.yaml` — a job that generates the large fixture and
  runs the artifact tests
* `docs/TESTING.md` — the fixture policy

## Files Expected to Add

* `fixtures/tiny-llama-large/config.json` (committed)
* `fixtures/tiny-llama-large/golden.json` (committed)
* `fixtures/tiny-llama-large/*.safetensors` (**generated, ignored**)
* `crates/q-source/src/fixture.rs` — a helper that locates the fixture or skips
  with a clear message

## Files Expected to Remove or Deprecate

None. Existing fixtures stay; they cover ingestion correctness well.

## Data Contracts

`golden.json` extends the existing shape with:

```jsonc
{ "blocks": [ { "tensor": "…q_proj.weight", "rows": [0,256], "columns": [0,256],
                "min": …, "max": …, "mean": …, "variance": …,
                "l1_norm": …, "l2_norm": …,
                "zero_ratio": …, "positive_ratio": …, "negative_ratio": … } ] }
```

Statistics are computed by **numpy in float64**, independent of the Rust code
under test — the same discipline as the existing hand-computed values.

## Memory and Performance Constraints

* The generator must write **shard by shard**, never holding 400 MB.
* Generation under 60 s.
* Values are deterministic from a fixed seed, or `git diff --exit-code` on
  `golden.json` cannot work.

## Implementation Plan

1. Parameterize `generate_fixtures.py` by model config.
2. Add the `tiny-llama-large` config: 4 layers, `hidden_size 4096`,
   `intermediate_size 11008`, sharded into ~200 MB pieces.
3. Generate with a fixed `numpy` seed, writing incrementally.
4. Read back with `safetensors`; compute golden scalars, slices, and block
   statistics in float64.
5. Add the Rust fixture helper: locate, or skip the test with
   "run `python3 fixtures/generate_fixtures.py --large`".
6. Add the `.gitignore` entries and the CI job.

## Error Handling

* Missing `numpy`/`safetensors` → a message naming the pip command.
* Insufficient disk → fail before writing, naming the required space.
* A test needing the fixture without it → **skip with a clear reason**, never a
  false pass.

## Acceptance Criteria

1. `python3 fixtures/generate_fixtures.py --large` produces a checkpoint with at
   least one `[4096, 4096]` f32 tensor.
2. `golden.json` contains ≥ 6 scalars, ≥ 2 slices, and ≥ 4 block statistics
   entries, computed by numpy in float64.
3. `.safetensors` files are gitignored; `git status` is clean after generation.
4. Regenerating twice produces an identical `golden.json`.
5. Generation peak RSS **< 256 MB** for a 400 MB checkpoint.
6. The existing fixtures and all 290 Rust tests are unaffected.
7. A test requiring the fixture skips with an actionable message when absent.

## Verification Plan

**Automated** — a CI job generating the fixture and asserting the golden values;
`git status --porcelain` empty afterwards.
**Manual** — inspect `golden.json` for plausibility; confirm peak RSS with
`/usr/bin/time -l`.

## Suggested Commands

Verified today:

```bash
python3 -m venv .venv && .venv/bin/pip install numpy safetensors
.venv/bin/python fixtures/generate_fixtures.py
```

Introduced by this task:

```bash
.venv/bin/python fixtures/generate_fixtures.py --large
/usr/bin/time -l .venv/bin/python fixtures/generate_fixtures.py --large
```

## Test Cases

| Input | Expected |
| --- | --- |
| `--large` on a clean tree | Fixture generated; `git status` clean |
| Run twice | `golden.json` byte-identical |
| `q-cli value … --index 2000,3000` | Matches `golden.json` |
| A 256×256 block's statistics | Match numpy float64 within `1e-9` |
| Fixture absent, artifact test run | Skipped with an actionable message |

## Risks

| Risk | Mitigation |
| --- | --- |
| 400 MB is slow to generate in CI | Only the artifact job generates it; the default suite does not |
| Generated ≠ committed fixtures diverge in structure | Same generator, same code path, different config |
| A developer forgets to generate it | The skip message names the exact command |

## Completion Evidence

* Generator output and timing.
* `golden.json` excerpt showing scalars, slices, and block statistics.
* `/usr/bin/time -l` peak RSS.
* `git status --porcelain` empty after generation.
* CI run showing the artifact job passing.
