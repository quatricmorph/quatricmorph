# Quatricmorph Product Brief

Condensed from [PRODUCT_ARCHITECTURE_v1.md](PRODUCT_ARCHITECTURE_v1.md). Prefer the full doc for schemas and deep design.

## One-sentence definition

> Quatricmorph is a tensor-native analytical database, model debugger, and controlled transformation runtime for open-weight neural networks.

## Core verbs

```text
Inspect → Query → Morph → Verify
```

## Strategic wedge (platform MVP)

```text
Open two compatible SafeTensors checkpoints
→ index them locally
→ inspect architecture and tensor statistics
→ query and visualize differences
→ define a layer-aware morph recipe
→ preview as a virtual model
→ run structural + lightweight behavioral validation
→ export a reproducible SafeTensors artifact
```

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

## Immediate engineering wedge (this repo, now)

Before the full SafeTensors platform, this repository’s **active coding target** is the browser visualization MVP:

```text
A @ B = C
```

in a shared 3D grid-ruled-lines coordinate system, built on the migrated `mm` / Three.js app under `quatricmorph/`.

See [requirements/VIZ_MVP.md](requirements/VIZ_MVP.md) and root `prompts.md`.

## Explicit non-goals (platform MVP)

- Arbitrary architecture conversion
- Different-tokenizer merging without a contract
- Trillion-parameter distributed infra first
- MoE transplantation / semantic expert labeling without experiments
- Hosted public model marketplace

## Recommended stacks (from architecture)

| Layer | Language |
| --- | --- |
| Tensor IO, catalog, planner, export integrity | Rust |
| PyTorch / eval / research plugins | Python |
| Desktop/web UI, viz | TypeScript |
| Interactive tiles | WebGPU (not authoritative for export) |
