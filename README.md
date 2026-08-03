# Quatricmorph

Local-first, tensor-native platform for inspecting, querying, and visualizing
open-weight models from SafeTensors — **without loading entire checkpoints into
RAM**.

**Canonical architecture (source of truth):** [ARCHITECTURE.md](ARCHITECTURE.md)
**What is actually built, with tests:** [STATUS.md](STATUS.md)

## What works today

```bash
# Parse headers and resolve names — reads ~20 KB of a 1.2 MB checkpoint
cargo run -p q-cli -- inspect fixtures/tiny-llama-2shard

# Read one exact weight: 4 bytes, by canonical address or alias
cargo run -p q-cli -- value fixtures/tiny-llama-2shard 'Q[10]' --index 100,42
#> 0.006408154033124447
#>   model.layers[10].self_attention.query_projection.weight at [100,42]
#>   — 4 bytes read from model-00002-of-00002.safetensors at offset 419928 (exact)

# Plan a matrix expression: shapes are checked, nothing is computed
cargo run -p q-cli -- query fixtures/tiny-llama-2shard \
  'show tensor("Q[10]") @ transpose(tensor("K[10]"))'

# CPU-reference statistics over one block
cargo run -p q-cli -- stats fixtures/tiny-llama-2shard \
  'model.layers[10].self_attention.query_projection.weight' --rows 100:104 --columns 40:44

# Serve the local API
cargo run -p q-daemon -- --model-root fixtures/tiny-llama-2shard
```

That value matches what Python's `safetensors` library reads at the same index —
asserted in `tests/tests/end_to_end_scalar_slice.rs` against golden values
in `fixtures/tiny-llama-2shard/golden.json`.

## What does not work yet

No tileset, no GLB, no CesiumJS viewer, no CUDA execution, no chat layer, and no
matrix-multiplication *execution*. Every one of those returns an explicit
`NotImplemented` carrying a requirement ID rather than a plausible-looking
result. [STATUS.md](STATUS.md) is the full list.

**"Trillion-scale" in this codebase means metadata and addressing scale under
bounded memory.** It never means loading a trillion parameters anywhere.
`crates/q-catalog/tests/trillion_scale_manifest.rs` indexes and queries a
10¹²-parameter manifest — 47 278 tensors describing 2.10 TB of payload — using
35.7 MB of peak allocation, while opening no artifact at all.

## Build and test

```bash
cargo test --workspace                        # 290 tests
cd apps/web && npm install && npx vitest run  # 101 tests
```

Fixtures are checked in and no test touches the network. To regenerate them:

```bash
python3 -m venv .venv && .venv/bin/pip install numpy safetensors
.venv/bin/python fixtures/generate_fixtures.py
```

## Start here

| Doc | Purpose |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | **Root source of truth** — implementation architecture |
| [STATUS.md](STATUS.md) | Requirement → code → test traceability |
| [docs/CURRENT_ARCHITECTURE.md](docs/CURRENT_ARCHITECTURE.md) | Evidence record of `mm/`, the code this workspace derives from |
| [docs/decisions/](docs/decisions/) | Architecture decision records |
| [AGENTS.md](AGENTS.md) | Autonomous agent guide |
| [docs/requirements/VIZ_MVP.md](docs/requirements/VIZ_MVP.md) | Phase 0 acceptance criteria (`TILE-*`) |
| [docs/requirements/MVP_REQUIREMENTS.md](docs/requirements/MVP_REQUIREMENTS.md) | Platform P0/P1 (`PLAT-*`) |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Phases 0–6 |

## The four data planes

Per [ARCHITECTURE.md §2.1](ARCHITECTURE.md). Every module declares which plane it
belongs to in its top-of-file doc comment.

| Plane | Contents | Crates |
| --- | --- | --- |
| **Artifact** | Immutable `config.json`, shard index, `*.safetensors`. Never rewritten. | `q-source`, `q-safetensors` |
| **Metadata** | Model, layer, tensor, block, statistics, tile, job records. | `q-architecture`, `q-nsir`, `q-catalog`, `q-tensor-runtime`, `q-expression`, `q-weightql` |
| **Tensor Tile** | Multiresolution tensor-native data (`*.qtile`). **Never GLB.** | `q-tiles`, `q-statistics` |
| **Visualization** | Render-only `tileset.json` and GLB tile content. | `q-tileset`, `q-gltf` |

## Repository layout

```text
ARCHITECTURE.md     Canonical implementation architecture (immutable SoT)
STATUS.md           Requirement traceability, built from real test output
Cargo.toml          Rust workspace root
crates/             17 crates: ingestion, NSIR, catalog, WeightQL, tiles, compute, API, CLI
gpu/cuda/           CUDA kernel sources — HARDWARE-UNVERIFIED, never compiled
gpu/wgsl/, metal/   Placeholder shaders
apps/web/
  matrix-workspace/ Matrix multiplication workspace, derived from mm (see its NOTICE.md)
  model-viewer/     CesiumJS browser — app shell only
  query-interface/  WeightQL input with KaTeX preview
architectures/      Family resolvers: generic + llama implemented; qwen/kimi/deepseek declared
schemas/            NSIR, qtile, WeightQL, visualization — describing what is implemented
fixtures/           Small checked-in SafeTensors fixtures + their generator
tests/              Cross-crate integration tests, incl. the end-to-end vertical slice
docs/               Product, requirements, evidence record, decisions
python/             Python bindings (scaffold)
mm/                 Historical matrix-viz reference — read-only, not product surface
```

## Non-goals (architecture §19)

- One cube GLB per weight
- Storing absolute positions for every scalar
- Sending entire tensors into the browser
- Using Cesium as a tensor compute engine
- Letting chat freely execute terabyte-scale expressions
- Treating colour patterns as semantic proof of model concepts

## License

Rust and web code: `MIT OR Apache-2.0`.
`apps/web/matrix-workspace` derives from **mm** by Meta Platforms, Inc.; its MIT
license is reproduced at `apps/web/matrix-workspace/LICENSE` with attribution in
`apps/web/matrix-workspace/NOTICE.md`. `mm/` retains its original license.
