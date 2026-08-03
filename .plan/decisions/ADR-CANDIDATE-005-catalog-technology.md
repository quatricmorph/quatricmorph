# ADR-CANDIDATE-005 — Catalog technology

## Status

`Open` — the implementation is settled; whether to revisit is not.

## Context

`ARCHITECTURE.md` §2.1 and §5 name DuckDB, Arrow, and Parquet for the Metadata
Plane. The implementation is SQLite.

## Repository evidence

* `crates/q-catalog/` — 987 lines + 261 of schema, `rusqlite` with `bundled`.
* `CURRENT_SCHEMA_VERSION = 1`; six tables plus `schema_migrations`; three
  indices on `tensors`, one each on `tensor_blocks`, `tensor_statistics`,
  `visual_tiles`, `conversion_jobs`.
* `docs/decisions/ADR-003-catalog-sqlite.md` — the departure is recorded.
* `STATUS.md` `CAT-010` — DuckDB/Arrow/Parquet, **Not Started**.
* `CAT-006` **Verified**: 47 278 tensors, 1.048×10¹² parameters, 2.10 TB
  described, indexed and queried at **35.7 MB peak**.
* Nine `CAT-*` rows `Verified`, including migrations, hierarchy browse,
  canonical-address lookup with raw-name fallback, five filter kinds, byte-range
  resolution, reopen survival, and idempotent re-import.

## Decision required

Does the MVP move to DuckDB/Arrow/Parquet, or does SQLite stand?

## Options

| Option | |
| --- | --- |
| **A** | SQLite stands for the MVP; revisit on measured need |
| **B** | Move to DuckDB now, matching `ARCHITECTURE.md` §5 |
| **C** | SQLite for metadata, Parquet for bulk statistics export |

## Advantages

* **A** — nine verified requirements, no migration, no new dependency,
  `bundled` so there is no system-library skew. The 35.7 MB measurement is
  already the strongest evidence any option could offer.
* **B** — columnar analytics; `GROUP BY layer_index` over a large model is
  DuckDB's shape; matches the architecture document literally.
* **C** — keeps the point-lookup path and adds a good export format.

## Disadvantages

* **A** — `WQL-007`'s statistical aggregation may be slower on SQLite at large
  model sizes. **Unmeasured.**
* **B** — a large new dependency, a migration of nine verified requirements, and
  the loss of `bundled`'s hermeticity, in exchange for a benefit nobody has
  measured a need for.
* **C** — two storage engines to keep consistent.

## Risks

* **A** — `WQL-007` (`QM-0072`) turns out slow. Mitigation: measure. The catalog
  holds one row per tensor; a 47 278-row `GROUP BY` is not a workload SQLite
  struggles with.
* **B** — migrating working, tested code to satisfy a document rather than a
  measurement. This is the more likely way to lose.

## Recommended default

**A.** SQLite stands. Revisit **only** if `QM-0072` measures a statistical query
above 1 s on the 47 278-tensor synthetic manifest.

The catalog's workload is point lookups by canonical address, hierarchy walks,
and filtered scans over one row per tensor — SQLite's shape, with the indices
already in place. `CAT-010` remains open in `STATUS.md` as an honest record that
the architecture document names something else.

## Tasks affected

`QM-0020`, `QM-0021`, `QM-0022`, `QM-0072`, `QM-0084`.

## Decision deadline

Before Phase 08, when `QM-0084` produces the scaling numbers that would justify
reopening it.
