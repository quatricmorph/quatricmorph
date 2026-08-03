# Roadmap

From architecture §28. This repo currently executes **Phase 0 / Track A** (viz) in parallel with documentation for Phase 1+.

## Phase 0 — Visualization foundation (now)

- Migrate `mm` → Vite TypeScript (`quatricmorph/`) — done
- Modularize viz modules — done
- Margin-grid MVP (`A @ B = C`) — in progress
- Unit tests for pure math — scaffolding

## Phase 1 — Inspect

SafeTensors ingestion, shard resolver, NSIR ontology, architecture plugins, metadata catalog, global statistics, architecture/tensor browser.

**Deps:** Rust core, fixture library, local daemon, desktop shell.

## Phase 2 — Query

WeightQL subset, planner, derived indexes, block engine, Tensor Tiles, query editor, Python/Rust APIs.

## Phase 3 — Morph

MIR / Virtual Models, interpolation, task arithmetic, TIES/DARE later, LoRA composition, dry-run, streaming export.

## Phase 4 — Verify & govern

Evaluation manifests, sampled forward / perplexity, policy gates, later enterprise registry.

Do not skip validation gates when implementing Morph.
