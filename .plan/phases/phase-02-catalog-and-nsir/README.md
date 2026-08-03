# Phase 02 — Catalog and NSIR completion

## Goal

```text
Raw tensor names → canonical model hierarchy → queryable local catalog
   ... already done.
Statistics, blocks, and tiles → PERSISTED, not just computable.
```

## The gap this phase closes

`STATUS.md`, on statistics: *"`q-statistics` and `q_gpu::CpuBackend` work; nothing
has run a statistics pass, so `tensor_statistics` is empty and the API returns
501."*

The catalog has six tables. Three are populated (`models`, `tensors`,
`conversion_jobs`); three are empty (`tensor_blocks`, `tensor_statistics`,
`visual_tiles`). Everything downstream — tiles, GLB, the viewer's inspector,
statistical queries — reads from the three empty ones.

## Entry conditions

* Phase 00 complete; **G1** passed.
* `QM-0012` complete (model-level metadata), so statistics have a subject to
  attach to.

## What is already done

Nine `CAT-*` requirements `Verified`: schema and migrations, idempotent and
future-refusing; hierarchy browse; canonical-address lookup with raw-name
fallback; five filter kinds; byte-range resolution as pure metadata arithmetic;
survival across close and reopen; idempotent re-import; and **`CAT-006`** — a
10¹²-parameter manifest at 35.7 MB peak.

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0020` | Persist and serve tensor statistics | Implementation | `STAT-002`, `API-005` |
| `QM-0021` | `visual_tiles` rows and tile↔tensor resolution | Implementation | `CAT-012`, `MVP-06` |
| `QM-0022` | `tensor_blocks` registry wired to conversion | Implementation | `CAT-013` |
| `QM-0023` | Candidate resolution surfaced end to end | Verification | `NSIR-007`, `API-007`, `MVP-34` |

## Design constraints

* **Statistics rows are versioned, not overwritten.** `StatisticsId` includes
  `algorithm_version` (`ADR-CANDIDATE-018`), so recomputing with a new algorithm
  mints new rows and two versions can be compared. Overwriting would destroy the
  ability to detect that a change altered results.
* **`approximate` is a column, not a comment.** `STAT-005` already labels sampled
  results; the column carries that to the API and then to the badge.
* **Tile↔tensor resolution must work both ways.** The viewer picks a feature and
  needs a tensor; the search box names a tensor and needs a tile to fly to.
* SQLite stands (`ADR-CANDIDATE-005`); revisit only if `QM-0072` measures a
  statistical query above 1 s on the 47 278-tensor manifest.

## Exit conditions

1. A statistics pass over one tensor writes rows readable after a reopen.
2. `GET /v1/tensors/{id}/statistics` returns data with a fidelity label instead
   of 501.
3. `visual_tiles` rows exist for a converted tensor, with `parent_tile_id` and
   `child_count` consistent, and both resolution directions tested.
4. `tensor_blocks` rows carry `source_byte_ranges` and `content_hash`, and the
   content hash is what makes resume able to skip a completed block.
5. An ambiguous alias returns candidates through the API, and the daemon test
   `an_ambiguous_alias_is_a_409_carrying_its_candidates` still passes.
6. Migrations remain idempotent and a future schema is still refused.

## Parallelization

`QM-0020`, `QM-0021`, and `QM-0022` all touch `crates/q-catalog/src/lib.rs` and
`schema.rs` — **high merge-conflict risk**. Run them **sequentially** in that
order. `QM-0023` is independent and may run in parallel with any of them.

## Risks

| Risk | Mitigation |
| --- | --- |
| Three tasks editing one 987-line file | Sequential execution, recorded in [`DEPENDENCY_GRAPH.md`](../../DEPENDENCY_GRAPH.md) §4 |
| A migration edited rather than appended | Review rule: **never edit a shipped migration** — it has already run on someone's disk |
| Row-per-block inserts dominating conversion time | Batch in transactions; measured in `QM-0031` |
