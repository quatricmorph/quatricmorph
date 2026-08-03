# Quatricmorph Product Brief

Condensed product thesis. **Implementation architecture source of truth:** [`../ARCHITECTURE.md`](../ARCHITECTURE.md). Broader product narrative: [PRODUCT_ARCHITECTURE_v1.md](PRODUCT_ARCHITECTURE_v1.md) (subordinate where it conflicts).

## One-sentence definition

> Quatricmorph is a tensor-native analytical database, model debugger, and controlled transformation runtime for open-weight neural networks.

## Core thesis

```text
SafeTensors
→ semantic tensor address space
→ queryable block hierarchy
→ procedural multiresolution visualization
→ exact on-demand computation
```

The tensor database and virtual computational objects are the core layer; visualization is one projection of the same data and query substrate.

## Four data planes

1. **Artifact** — original SafeTensors, tokenizer, config, shard indexes  
2. **Metadata** — model / layer / tensor / block catalog (DuckDB, Arrow, Parquet)  
3. **Tensor Tile** — summaries, samples, exact blocks (`.qtile`)  
4. **Visualization** — tileset.json, GLB, GPU buffers, labels, camera state  

## Strategic wedge (platform)

```text
Open SafeTensors (HF or local)
→ index without full RAM residency
→ browse model → layer → tensor → block → scalar
→ query via WeightQL
→ visualize with CesiumJS LOD, then custom WebGPU
→ (later) morph + verify with validation before success
```

## Immediate engineering wedge (now)

**Phase 0 — Tensor Tiling Spike** ([ARCHITECTURE.md](../ARCHITECTURE.md) §17–§18):

```text
Open one SafeTensors file
→ select one 4096 × 4096 tensor
→ create five LOD levels
→ generate tileset.json
→ visualize in CesiumJS
→ click a cell and retrieve the exact value
```

Viewer stack for the spike: React or Svelte + CesiumJS + 3D Tiles 1.1 + GLB + `.qtile` sidecars. Full-model support is out of scope for Phase 0.

## Product axioms (must not violate)

1. Checkpoint bytes are the source of truth; indexes are rebuildable.
2. No tensor transformation is successful until validated.
3. Compatibility must be proven, not inferred from matching shapes alone.
4. A model variant stays virtual until materialization is required.
5. Semantic claims require behavioral or causal evidence.
6. Out-of-core execution is first-class.
7. Results expose cost, approximation, confidence, and provenance.
8. Local execution is the default.
9. Visualization is generated from the same query/lineage substrate as automation.
10. Open formats and reproducible recipes beat proprietary containers.

## Explicit non-goals (architecture §19 + platform MVP)

- One cube GLB per weight; absolute positions per scalar
- Sending entire tensors into the browser
- Cesium as a compute engine; chat executing unbounded expressions
- Treating weight color patterns as semantic proof
- Arbitrary architecture conversion; different-tokenizer merging without a contract
- Trillion-parameter distributed infra as Phase 0
- Hosted public model marketplace

## Recommended stacks

| Layer | Stack |
| --- | --- |
| Ingestion, NSIR, catalog, WeightQL, tiles, daemon | Rust crates (`q-*`) |
| Research / eval plugins | Python |
| Prototype viewer | CesiumJS + 3D Tiles + TypeScript |
| Native tensor renderer | Tauri + wgpu / WGSL |
| Large compute plugins | CUDA, Metal Performance Shaders, CPU BLAS |
