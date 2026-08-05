# Roadmap

Derived from [`ARCHITECTURE.md`](../ARCHITECTURE.md) §17–§18. That document is authoritative if this file drifts.

## Release v1 — Out-of-core quantization-error diagnostic (now)

```text
Real open-weight SafeTensors checkpoint
→ stream under a configured resident-byte ceiling
→ simulate quantization block by block
→ measure per-channel / per-tensor / per-layer weight-space error
→ rank fragile layers; compute a bytes-versus-error frontier
→ deterministic Markdown report + versioned JSON manifest
→ one 2D heat-map fed by that manifest
```

v1 input: `models/distilbert-distilgpt2/`; larger MoE checkpoints are out of v1 scope
([`.plan/MASTER_PLAN.md`](../.plan/MASTER_PLAN.md) §4). Release gate: `V1-01` … `V1-32`
in [`.plan/DEFINITION_OF_DONE.md`](../.plan/DEFINITION_OF_DONE.md). The scope decision
and its source are recorded in [`ARCHITECTURE.md`](../ARCHITECTURE.md) §17.1.

v1 reports *weight-space* error, measured. It does not predict a downstream
behavioural or benchmark delta.

---

**Phases 0–6 below are the platform release, which follows v1.** They are retained
unchanged and not renumbered; `TILE-*` and `PLAT-*` keep their meanings.

## Phase 0 — Tensor Tiling Spike (deferred to the platform release)

```text
Open one SafeTensors file
→ select one 4096 × 4096 tensor
→ create five LOD levels
→ generate tileset.json
→ visualize in CesiumJS
→ click a cell and retrieve the exact value
```

Full-model support is out of scope. Requirements: [requirements/VIZ_MVP.md](requirements/VIZ_MVP.md).

## Phase 1 — Dense Model Browser

Sharded SafeTensors, architecture resolver, model/layer/tensor hierarchy, tensor statistics, Cesium LOD, exact weight lookup, local cache.

```text
Open a Qwen/Llama-like model
→ zoom model → layer → tensor → block → scalar
```

## Phase 2 — Mathematical Query Engine

Tensor aliases, slices, transpose, reshape, addition, multiplication, reduction, query plans, visual expression graph. Goal: visualize `(A @ B) @ C` on real tensor blocks via WeightQL.

## Phase 3 — Custom WebGPU Renderer

Replace detailed GLBs with GPU storage buffers, procedural cells, compute culling, indirect drawing, data-driven shaders. Cesium remains overview-only or is replaced in the tensor workspace.

## Phase 4 — Native GPU Desktop

Tauri, wgpu, Metal / Vulkan / DX12, CUDA compute plugin, GPU memory scheduler, multi-GPU jobs.

## Phase 5 — Runtime Neural Observability

Hidden states, Q/K/V activations, attention probabilities, residual stream, MoE routing, token-conditioned visualization, matmul from real prompts.

## Phase 6 — Trillion-Scale Remote Execution

Object storage, distributed block workers, Arrow transfer, server-side tile generation, query result streaming, shared workspaces, CDN-published visualization summaries.

## Later product verbs (from product vision)

Inspect → Query → Morph → Verify remain the long-term product verbs. Morph/Verify work must not mark Phase 0–2 complete, and must keep validation-before-success and root architecture constraints (no cube-per-weight, out-of-core first).
