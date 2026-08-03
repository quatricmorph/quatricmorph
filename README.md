# Quatricmorph

Local-first tensor-native platform for inspecting, querying, and visualizing open-weight models from SafeTensors — without loading entire checkpoints into RAM.

**Canonical architecture (source of truth):** [ARCHITECTURE.md](ARCHITECTURE.md)

**Active MVP:** Phase 0 — Tensor Tiling Spike (SafeTensors → LOD tiles → CesiumJS → exact scalar lookup). See [docs/requirements/VIZ_MVP.md](docs/requirements/VIZ_MVP.md).

## Start here

| Doc | Purpose |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | **Root source of truth** — implementation architecture |
| [AGENTS.md](AGENTS.md) | Autonomous agent guide |
| [docs/README.md](docs/README.md) | Documentation index |
| [docs/PRODUCT_BRIEF.md](docs/PRODUCT_BRIEF.md) | Condensed product thesis |
| [docs/requirements/PREREQUISITES.md](docs/requirements/PREREQUISITES.md) | Gate before large implementation |
| [docs/requirements/VIZ_MVP.md](docs/requirements/VIZ_MVP.md) | Phase 0 acceptance criteria |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Phases 0–6 from architecture |

## Target flow

```text
SafeTensors
→ NSIR semantic model
→ tensor-native block database
→ multiresolution Tensor Tiles (.qtile + tileset.json / GLB)
→ WeightQL and mathematical expressions
→ CesiumJS overview + custom WebGPU tensor renderer
→ Metal / CUDA acceleration
→ runtime activations and model morphing
```

## Concrete MVP (Phase 0)

From [ARCHITECTURE.md](ARCHITECTURE.md) §18:

```text
Model: 0.5B–7B SafeTensors (Qwen or Llama-like)
Tensor: Q projection or MLP down projection
Viewer: CesiumJS
LOD: model → layer → tensor → block
Query: exact scalar and tensor slice
Math: one A @ B visualization on a real block
```

Do **not** start from full trillion-scale models or one-cube-per-weight GLBs.

## Repository layout (target)

Per architecture §16. Current tree may still contain legacy reference code; new work follows this structure:

```text
ARCHITECTURE.md     Canonical implementation architecture (immutable SoT)
crates/             Rust: SafeTensors, NSIR, catalog, WeightQL, tiles, …
apps/web/           CesiumJS / WebGPU viewer
apps/desktop/       Tauri + wgpu (later phases)
architectures/      Family resolvers (llama, qwen, …)
schemas/            NSIR, qtile, WeightQL, visualization
fixtures/           Small allowlisted SafeTensors fixtures
docs/               Product, requirements, agent charter
mm/                 Historical matrix-viz reference (read-only; not product)
quatricmorph/       Legacy Three.js experiment (not architecture target)
```

## Non-goals (architecture §19)

- One cube GLB per weight
- Storing absolute positions for every scalar
- Sending entire tensors into the browser
- Using Cesium as a tensor compute engine
- Letting chat freely execute terabyte-scale expressions
- Treating color patterns as semantic proof of model concepts
