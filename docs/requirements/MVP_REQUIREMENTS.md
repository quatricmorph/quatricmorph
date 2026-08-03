# Platform MVP Requirements (P0 / P1)

Aligned to [`ARCHITECTURE.md`](../../ARCHITECTURE.md) Phases 1–2 and product acceptance themes. Phase 0 spike uses `TILE-*` in [VIZ_MVP.md](VIZ_MVP.md). Use IDs in commits and agent reports.

**Authority:** root `ARCHITECTURE.md` wins on ingestion, LOD, tiles, WeightQL, renderers, APIs, and “what not to do.”

## P0 — Must ship for dense browser + query foundation

| ID | Area | Requirement | Validation |
| --- | --- | --- | --- |
| PLAT-P0-INGEST | Ingestion | Local SafeTensors + sharded checkpoints; HF config/tokenizer metadata; range/mmap reads; no full-RAM load | Fixture import; cancel/resume metadata |
| PLAT-P0-ADAPTER | Ontology | Architecture plugins (generic, llama, qwen, …) → NSIR; may return `unknown`; never guess role from shape alone | Invariant tests; unresolved warnings |
| PLAT-P0-CATALOG | Catalog | Models/tensors/blocks/statistics tables (architecture §5); DuckDB/Parquet | Deterministic stats across runs |
| PLAT-P0-TILES | Tiles | LOD 0–5 pipeline; `.qtile` sidecar; GLB for viz only; tileset.json | Aggregation checksums; no cube-per-weight |
| PLAT-P0-LOOKUP | Exact I/O | Canonical address + exact scalar/slice via range read; match Python SafeTensors reference | Golden index fixtures |
| PLAT-P0-CACHE | Cache | Content-addressed L1/L2 keys (architecture §13); reopen reuse | Cache-hit metrics |
| PLAT-P0-API | Local API | Metadata / block / value / visualization endpoints (architecture §14) | Contract tests |
| PLAT-P0-UX | Interfaces | CesiumJS browser for hierarchy LOD; CLI/daemon as needed for ingest | Core browse without notebook |

## P1 — WeightQL and expression visualization

| ID | Area | Requirement |
| --- | --- | --- |
| PLAT-P1-WQL | WeightQL | Scalar, slice, statistical queries; aliases resolve or return candidates |
| PLAT-P1-EXPR | Expressions | Plan `(A @ B) @ C`; shape-check before execute; cost estimate; exact/sampled/approx labels |
| PLAT-P1-MATMUL-VIZ | Viz | Block-mode matmul animation (Concept / Tensor Block; Full Compute only with cost gate) |
| PLAT-P1-CHAT | Chat | Assistant calls WeightQL planner only; never raw weight bytes |

## Later (do not treat as Phase 0–1 done)

Morph (MIR / Virtual Models), streaming SafeTensors export, and full Verify gates remain product-vision items from [PRODUCT_ARCHITECTURE_v1.md](../PRODUCT_ARCHITECTURE_v1.md). Implement only when explicitly tasked; they must still obey validation-before-success and root architecture constraints.

## Explicit exclusions

- One cube GLB per weight; absolute positions per scalar
- Sending entire tensors to the browser
- Cesium as compute engine
- Chat without plan + I/O estimate
- Semantic claims from color patterns alone
- Arbitrary architecture conversion; different-tokenizer merging without contract
- Hosted public model marketplace

## Acceptance criteria (dense browser + query)

1. Do not load the entire checkpoint into RAM.
2. Successfully parse sharded SafeTensors.
3. Metadata import can be cancelled and resumed.
4. Clicking a visual cell returns the correct tensor address.
5. Exact scalar matches Python SafeTensors reference.
6. Zooming out does not load exact values; zooming in range-reads only needed bytes.
7. Cache reused after reopening.
8. Shape-mismatched expression rejected before execution.
9. UI clearly indicates exact, sampled, or approximate results.
