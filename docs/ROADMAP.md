# Roadmap

Derived from [`ARCHITECTURE.md`](../ARCHITECTURE.md) §17–§18. That document is authoritative if this file drifts.

## Phase 0 — Tensor Tiling Spike (now)

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
