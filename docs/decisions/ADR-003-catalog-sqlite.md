# ADR-003 — SQLite for the metadata catalog, departing from ARCHITECTURE.md §5

**Status:** Accepted (scoped to this pass)
**Date:** 2026-08-03
**Departs from:** ARCHITECTURE.md §2.1 and §5

## Context

**ARCHITECTURE.md is explicit and this decision departs from it.** §2.1 says of
the Metadata Plane: *"This is where DuckDB, Arrow, and Parquet are used."* §3's
diagram labels the Metadata Catalog box *"DuckDB/Parquet"*.
`docs/requirements/MVP_REQUIREMENTS.md` repeats it (`PLAT-P0-CATALOG`:
*"DuckDB/Parquet"*).

The catalog in this pass must support: schema versioning and migrations from the
first release, hierarchy browsing (model → layer → tensor), canonical-address
lookup with a raw-name fallback, role and shape filters, byte-range resolution,
conversion-job persistence, and a ~47 000-row insert in one transaction for the
trillion-scale metadata test.

## Decision

SQLite, via `rusqlite` with the `bundled` feature (the SQLite amalgamation
compiled from source, so the build does not depend on a system library).

## Alternatives considered

**DuckDB, as ARCHITECTURE.md specifies.** The right long-term answer, and the
reason is sound: the statistical queries of §7.3 — `GROUP BY layer_index` over
aggregate norms across every tensor — are analytical, and DuckDB's columnar
engine is built for exactly that. Deferred for this pass because
`duckdb-rs` bundles a large C++ engine that materially lengthens a clean build,
and because none of §7.3's statistical queries can run yet: no statistics have
been computed, and `tensor_statistics` is empty. Paying DuckDB's build cost to
store rows nothing aggregates would be premature.

**Parquet files with no query engine.** Rejected: the catalog needs point
lookups by canonical address on every query, and Parquet without a query engine
means scanning.

**In-memory only, persisted as JSON.** Rejected: `AC-008` requires the cache and
catalog to be reusable after reopening, and hand-rolling indexes over JSON is
how one ends up writing a worse database.

## Why SQLite specifically

* **Migrations from day one.** Retrofitting a migration ledger onto a schema
  that already shipped means guessing what state each user's database is in.
  `schema_migrations` exists in migration 1.
* **Point lookups are the dominant query.** Every WeightQL reference resolves
  through `get_by_canonical_name` or `find_by_role`. Both are indexed B-tree
  lookups; this is SQLite's strongest case.
* **One file, no server, no daemon-of-a-daemon.**
* **`bundled` means the build is hermetic.** No system SQLite version to
  discover at link time.

## Consequences

* `crates/q-catalog/src/schema.rs` holds SQL DDL. Migrating to DuckDB later
  means rewriting that file and the row mappers in `lib.rs`; the public API
  (`Catalog::list_layers`, `get_by_canonical_name`, `find_by_role`,
  `resolve_byte_range`) is engine-agnostic and would not change.
* The connection sits behind a `Mutex` because `rusqlite::Connection` is `Send`
  but not `Sync`, and the daemon shares one catalog across concurrent requests.
  Serializing catalog access is acceptable: every query is metadata-scale, and
  the expensive work — byte-range reads — happens outside the lock. DuckDB would
  not need this.
* Analytical queries (§7.3 `GROUP BY layer_index`, aggregate norms) will be
  slower on SQLite once statistics exist. **That is the trigger to revisit this
  decision**, not a general "when we have time".
* The departure is recorded here rather than silently taken, per the standing
  rule that ARCHITECTURE.md wins and conflicts get written down.
