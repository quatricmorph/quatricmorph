# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository is

Quatricmorph: a local-first, tensor-native platform for inspecting, querying, and
visualizing open-weight SafeTensors checkpoints **without loading them into RAM**.
A Rust workspace (`ls crates | wc -l` — 19 crates) plus three npm workspaces under
`apps/web/`.

Current release scope (`ARCHITECTURE.md` §17.1, `.plan/PRODUCT_SCOPE.md`): **v1 is
one diagnostic** — out-of-core quantization-error forensics. The CesiumJS viewer,
`.qtile` pyramid, matrix workspace, and chat layer are designed but **deferred
post-v1** and off the critical path.

## The rule that governs everything else

**This repository's credibility rests on never claiming a capability it has not
exercised.** That is not a slogan here; it is enforced mechanically, and most of
the unusual conventions below exist to serve it.

- An unbuilt subsystem returns `QError::NotImplemented { requirement, detail }`
  carrying a `STATUS.md` requirement ID — never a plausible-looking fake result
  (no invented scalar, no empty-but-valid tileset). See `crates/q-source/src/error.rs`.
- Every result is labelled **exact**, **sampled**, or **approximate**. Never
  present a sampled figure as exact.
- Never claim semantic understanding of model weights. A colour pattern is not a
  concept (`ARCHITECTURE.md` §19).
- "Trillion-scale" means **metadata and addressing scale under bounded memory**.
  It never means loading a trillion parameters. Any allocation proportional to
  total checkpoint size is a bug, surfaced as `QError::BudgetExceeded`.
- Test counts, crate counts, and task counts in prose go stale constantly.
  **Cite the command, derive the number** — including for numbers written in this
  file. `README.md` and `STATUS.md` currently quote 290/101 while
  `scripts/baseline.json` records a floor of 677/115; the docs are the stale half.

## Commands

```bash
# Rust workspace — the four gates CI runs, in CI's order
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets
cargo test --workspace

# One crate / one test
cargo test -p q-catalog
cargo test -p q-catalog canonical_address_lookup_and_raw_name_fallback
cargo test -p q-tensor-runtime --test bounded_residency

# Web (npm workspaces: quatricmorph-workspace, model-viewer, query-interface)
cd apps/web && npm install
cd apps/web && npx vitest run                      # all three
cd apps/web && npx vitest run query-interface      # one workspace / file filter
cd apps/web && npm run build --workspace quatricmorph-workspace
```

**Before claiming a task done, run the floor guard.** It re-runs every gate above
plus the CLI goldens and asserts nothing regressed:

```bash
./scripts/verify-baseline.sh          # needs apps/web/node_modules; touches no network
./scripts/license-audit.sh            # attribution + licence gate, also in CI
```

Fixture regeneration (only when a fixture must change — CI diffs the result):

```bash
python3 -m venv .venv && .venv/bin/pip install numpy safetensors
.venv/bin/python fixtures/generate_fixtures.py
```

Exercising the product:

```bash
cargo run -p q-cli -- inspect fixtures/tiny-llama-2shard
cargo run -p q-cli -- value fixtures/tiny-llama-2shard 'Q[10]' --index 100,42
cargo run -p q-cli -- query fixtures/tiny-llama-2shard 'show tensor("Q[10]") @ transpose(tensor("K[10]"))'
cargo run -p q-cli -- stream fixtures/tiny-llama-2shard --resident-ceiling 4MB --io pread
cargo run -p q-daemon -- --model-root fixtures/tiny-llama-2shard
```

`q --help` is authoritative for subcommands; the doc comment at the top of
`crates/q-cli/src/main.rs` lists them all.

## Architecture

### Four data planes

Every module declares its plane in a top-of-file doc comment, citing the
`ARCHITECTURE.md` section it implements. Preserve that when adding files — it is
how the plane boundary stays auditable.

| Plane | Contents | Crates |
| --- | --- | --- |
| **Artifact** | Immutable `config.json`, shard index, `*.safetensors`. Never rewritten. | `q-source`, `q-safetensors` |
| **Metadata** | Model, layer, tensor, block, statistics, tile, job records. | `q-architecture`, `q-nsir`, `q-catalog`, `q-tensor-runtime`, `q-expression`, `q-weightql` |
| **Tensor Tile** | Multiresolution tensor-native data (`*.qtile`). **Never GLB.** | `q-tiles`, `q-statistics`, `q-quant`, `q-gpu`, `q-cuda`, `q-cache` |
| **Visualization** | Render-only `tileset.json` + GLB tile content. | `q-tileset`, `q-gltf` |
| **Report** | Versioned diagnostic manifest (`.plan/REPORT_ARCHITECTURE.md`). | `q-report` |

### Pipeline

```text
SafeTensors artifact
  → q-safetensors      headers, shard index, exact byte-range reads (headers only at ingest)
  → q-architecture     plugin registry: config.json model_type → family resolver
  → q-nsir             raw name → canonical address + contextual alias (Q[10][100,42])
  → q-catalog          SQLite metadata store (descriptors, never weights)
  → q-tensor-runtime   blocks, tile identity, LOD ladder, bounded streaming reader
  → q-statistics / q-quant / q-gpu   CPU reference is ground truth; backends are diffed against it
  → q-tiles / q-gltf / q-tileset     deferred post-v1
  → q-weightql         the single query layer
  → q-cli, q-daemon
```

Two invariants that shape the code:

- **Everything goes through `q-weightql`.** The CLI, the HTTP API, the viewer,
  and eventually chat all query through it; none reads weight bytes directly.
  That is where addressing, shape checking, cost estimation, and fidelity
  labelling happen exactly once. Shape mismatches are rejected *before*
  execution; whole-tensor reads are refused outright.
- **`q-quant` takes values in and values out** — no file access, no catalog, no
  dependency on any other Quatricmorph crate. Keep it that way so it stays
  diffable against a NumPy reference.

Architecture plugins are declarative: `architectures/<family>/plugin.toml`.
`generic`, `llama`, and `qwen` carry `implemented = true`; `kimi` and `deepseek`
are declared with `implemented = false`, and a test asserts they never claim a
model. (`STATUS.md` NSIR-006 still reads "Not Started" for Qwen — it predates
`QM-0010`. The `plugin.toml` files are the live answer.)

`gpu/cuda/*.cu` is **HARDWARE-UNVERIFIED** and never compiled — no `nvcc`, no
`build.rs`, no FFI. There is deliberately no CUDA CI job, because a job that
"passed" without running kernels would be worse than none. Do not add one.

### Document authority

Where two documents disagree, the higher rank wins and the lower one gets fixed.

1. `docs/decisions/ADR-0XX-*.md` marked **Accepted** with a `Departs from:` line —
   overrides `ARCHITECTURE.md` for exactly the section it names.
2. **`ARCHITECTURE.md`** — the implementation source of truth.
3. **`STATUS.md`** — what is actually built and tested. The factual baseline;
   read it before trusting any plan.
4. `AGENTS.md` — non-negotiable agent rules.
5. `.plan/MASTER_PLAN.md`, then `.plan/PRODUCT_SCOPE.md`, then the remaining
   `.plan/*.md` subsystem designs, then `.plan/tasks/QM-XXXX-*/TASK.md`.

`.plan/decisions/ADR-CANDIDATE-*.md` are **not** authoritative — they are
proposals with a deadline. `.plan/` contains no code and mandates no change
outside `.plan/`: where the plan concludes a file outside it is wrong, the
correction is written as a task.

## Working conventions

### The `.plan/` task system

Work binds to a requirement ID. `.plan/tasks/QM-XXXX-short-name/TASK.md` is the
executable unit; numbers are allocated in blocks of ten per phase and are **never
reused or renumbered** (they appear in commit messages and dependency lists).

Task status lives in the `## Status` section, one value on its own line, any
blocker named on the line below: `Undefined`, `Ready`, `In Progress`, `Blocked`,
`Implemented`, `Verified`, `Complete`, `Deferred`, `Superseded`.

- `Complete` = implemented **and** verified **and** `STATUS.md` updated, all in
  the same squash merge commit.
- **Never start a `Deferred` task.** Deferred waits on a product decision, not a
  dependency; if it looks like the obvious next thing to build, that is the
  deferral working. Phases 04–07 are deferred wholesale for v1.
- Which tasks are `Ready` is derived, never read from a document:
  ```bash
  for f in .plan/tasks/*/TASK.md; do
    awk '/^## Status$/{getline; getline; print; exit}' "$f"
  done | sort | uniq -c | sort -rn
  ```
- Before writing code, confirm every path in the task's `Files Expected to
  Change` still exists. If one does not, the plan is stale and **fixing the plan
  takes precedence over the task that discovered it**.

`## Completion Evidence` in the task file must carry the copy-pasteable command,
its decisive output, and the commit SHA. "Tests pass" is not evidence;
`677 passed; 0 failed` with the command above it is. Longer derivations go in
`.plan/evidence/QM-XXXX.md`.

### Test floor

`scripts/baseline.json` records a floor (`rust_tests`, `rust_binaries`,
`web_tests`, `web_files`) plus CLI golden values derived independently of `q-cli`
— from `fixtures/tiny-llama-2shard/golden.json` (Python `safetensors`) and the
fixture's own `config.json`.

**The floor may only ever be raised, never lowered.** Raise it in the same commit
that adds the tests, and reconcile the delta exactly (`545 + 79 = 624`) — that
arithmetic is also the check that no existing test was removed, weakened, or
`#[ignore]`d. `verify-baseline.sh` reports a stale floor loudly but exits 0, so a
floor sitting below reality protects nothing; don't leave one there.

The `rust_binaries` count is a structural floor: a test binary that fails to
build stops printing its `test result:` line, which would otherwise shrink the
total silently.

### Tests

- No network in default tests, ever. Everything runs against checked-in
  `fixtures/`. No large weight downloads in automated flows.
- No GPU / WebGL / Cesium required in unit tests.
- Name tests after requirement IDs or the property they prove — this repo favours
  long declarative names (`the_g1_ceiling_is_derived_from_the_checkpoint_size_and_never_from_a_measured_peak`,
  `fp8_refuses_rather_than_approximates`, `streaming_in_chunks_equals_computing_at_once`).
- Golden expectations should be hand-computed or produced by an independent
  reference, not by the code under test.
- Cross-crate integration tests live in `tests/tests/`.

### Git

Commit subject: `type(scope): description [QM-XXXX]`, or `[CONTROLLER]` for plan
bookkeeping. Branches: `task/qm-XXXX-short-name`.

**There is no pull-request path** — the `gh` token has `push: false` on the repo.
Tasks integrate as **local squash merges onto `main`, which is then pushed over
SSH**. The squash merge commit, not a PR, is the review artifact.

## Gotchas

- **`models/` is gitignored.** `models/distilbert-distilgpt2` (352,824,413 bytes,
  single-file, F32, GPT-2 family, 82 tensors) is v1's headline checkpoint and
  exists only locally. Tasks citing it will not find it in a fresh clone. Larger
  MoE checkpoints are explicitly out of scope by owner directive.
- **`mm/` is read-only history** — Meta's matrix-viz reference, not product
  surface. `license-audit.sh` asserts it is unmodified. Do not delete or expand it.
  `apps/web/quatricmorph-workspace` is the derived, licensed copy.
- **Legacy `quatricmorph/` Three.js is not the architecture target.** Prefer
  `crates/q-*` and `apps/web`.
- The single-file checkpoint means the **sharded** read path is exercised only at
  1.2 MB (`fixtures/tiny-llama-2shard`), and bf16 decode and MoE expert
  aggregation have no real-checkpoint fixture at all. `.plan/DEFINITION_OF_DONE.md`
  §1 lists these qualifications; carry them forward rather than restating a
  qualified pass as a bare ✅.
- `CAT-006` (the synthetic 10¹² manifest) proves metadata scale and is **silent
  about streaming real bytes**. It may never stand in for the out-of-core claim.
- `.plan/EXECUTION_ORDER.md` §6 names where concurrency is forbidden; check it
  before parallelising edits across tasks.
