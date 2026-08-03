# Platform MVP Requirements (P0 / P1)

Derived from architecture §26–§27. Use IDs in commits and agent reports.

## P0 — Must ship for platform MVP acceptance

| ID | Area | Requirement | Validation |
| --- | --- | --- | --- |
| PLAT-P0-INGEST | Ingestion | Local SafeTensors + sharded checkpoints; HF config/tokenizer metadata; mmap/streaming | Fixture import tests; no unsafe remote code |
| PLAT-P0-ADAPTER | Ontology | Adapters for Llama-/Mistral-/Qwen-/Gemma-like dense decoders → NSIR | Round-trip / invariant tests; unresolved mapping warnings |
| PLAT-P0-CATALOG | Catalog | NSIR tensor catalog, architecture map, global + block stats, fingerprints, index cache | Deterministic stats across runs |
| PLAT-P0-COMPARE | Compare | Aligned two-model numerical comparison without full RAM residency (7B–70B class) | Bytes-read metrics; checksum fixtures |
| PLAT-P0-TILES | Tiles | Tensor Tiles Level 0–2 + heatmap / diff / layer ranking | Aggregation checksums |
| PLAT-P0-MIR | Morph | Virtual Model DAG; interpolation; task-vector arithmetic; layer coeffs; include/exclude | Canonical hash; dry-run resource estimate |
| PLAT-P0-EXPORT | Export | Streaming SafeTensors export + manifest + hashes | Byte-valid ST; reproducible hash on 2nd machine |
| PLAT-P0-VERIFY | Verify | Integrity, tokenizer identity, non-finite, numerical scorecard, sampled forward | Injected NaN/shape/tokenizer/missing-tensor caught |
| PLAT-P0-UX | Interfaces | Desktop shell + CLI + Python result access + local daemon | Core workflow without notebook |

## P1 — First public release stretch

| ID | Area | Requirement |
| --- | --- | --- |
| PLAT-P1-WQL | WeightQL | Metadata SELECT, semantic filters, aggregates, explain plan |
| PLAT-P1-TILES | Tiles | Advanced progressive tiles |
| PLAT-P1-EVAL | Eval | Optional small perplexity dataset gate |
| PLAT-P1-LORA | Morph | Simple LoRA application in MIR |

## Explicit exclusions (do not implement in MVP)

- Arbitrary architecture conversion
- Different-tokenizer merging
- MoE Expert Atlas / semantic expert labeling
- Enterprise governance / signing marketplace
- General training platform

## Acceptance criteria (architecture §27.5)

1. Import ≥4 supported architecture families from public fixtures.
2. Compare two related 7B–70B checkpoints without full RAM residency.
3. Deterministic statistics and diffs across repeated runs.
4. Create Virtual Model and export byte-valid SafeTensors.
5. Detect injected NaN, shape, tokenizer, and missing-tensor failures.
6. Reproduce export hash from the same manifest on another machine (same deterministic backend).
7. Complete core workflow without a notebook.
