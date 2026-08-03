//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §5).
//!
//! Schema and forward-only migrations.
//!
//! The table set follows ARCHITECTURE.md §5 verbatim: `models`, `tensors`,
//! `tensor_blocks`, `tensor_statistics`, `visual_tiles`, plus `conversion_jobs`
//! for the job state machine.
//!
//! Migrations are versioned from the first release even though there is only
//! one so far. Retrofitting migrations onto a schema that shipped without them
//! means guessing what state a user's database is in, so the ledger table
//! exists from day one.
//!
//! **Storage engine:** SQLite. ARCHITECTURE.md §5 names DuckDB/Arrow/Parquet
//! for this plane; the departure is deliberate and recorded in
//! `docs/decisions/ADR-003-catalog-sqlite.md`.

use q_source::error::{QError, Result};
use rusqlite::Connection;

/// The schema version this build writes and expects.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// All migrations, in application order.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: r#"
CREATE TABLE models (
    model_id         TEXT PRIMARY KEY,
    source_uri       TEXT NOT NULL,
    source_key       TEXT NOT NULL,
    source_revision  TEXT NOT NULL DEFAULT '',
    source_hash      TEXT NOT NULL,
    architecture     TEXT NOT NULL,
    resolver_id      TEXT NOT NULL,
    parameter_count  INTEGER NOT NULL,
    layer_count      INTEGER,
    hidden_size      INTEGER,
    tensor_count     INTEGER NOT NULL,
    payload_bytes    INTEGER NOT NULL,
    imported_at      INTEGER NOT NULL
);

CREATE TABLE tensors (
    tensor_id        TEXT PRIMARY KEY,
    model_id         TEXT NOT NULL REFERENCES models(model_id) ON DELETE CASCADE,
    raw_name         TEXT NOT NULL,
    canonical_name   TEXT NOT NULL,
    layer_index      INTEGER,
    expert_index     INTEGER,
    component        TEXT NOT NULL,
    role             TEXT NOT NULL,
    shape            TEXT NOT NULL,          -- JSON array of u64
    rank             INTEGER NOT NULL,
    dtype            TEXT NOT NULL,
    shard_uri        TEXT NOT NULL,
    byte_start       INTEGER NOT NULL,
    byte_length      INTEGER NOT NULL,
    parameter_count  INTEGER NOT NULL,
    resolved         INTEGER NOT NULL,       -- 0 = role is `unknown`
    UNIQUE (model_id, raw_name)
);

CREATE INDEX idx_tensors_model_layer ON tensors(model_id, layer_index);
CREATE INDEX idx_tensors_canonical   ON tensors(model_id, canonical_name);
CREATE INDEX idx_tensors_role        ON tensors(model_id, role, layer_index);

CREATE TABLE tensor_blocks (
    block_id            TEXT PRIMARY KEY,
    tensor_id           TEXT NOT NULL REFERENCES tensors(tensor_id) ON DELETE CASCADE,
    lod                 INTEGER NOT NULL,
    row_start           INTEGER NOT NULL,
    row_end             INTEGER NOT NULL,
    column_start        INTEGER NOT NULL,
    column_end          INTEGER NOT NULL,
    source_byte_ranges  TEXT NOT NULL,       -- JSON array of [start, end]
    statistics_id       TEXT,
    content_hash        TEXT NOT NULL
);

CREATE INDEX idx_blocks_tensor_lod ON tensor_blocks(tensor_id, lod);

CREATE TABLE tensor_statistics (
    statistics_id     TEXT PRIMARY KEY,
    subject_id        TEXT NOT NULL,
    count             INTEGER NOT NULL,
    min_value         REAL NOT NULL,
    max_value         REAL NOT NULL,
    mean              REAL NOT NULL,
    variance          REAL NOT NULL,
    l1_norm           REAL NOT NULL,
    l2_norm           REAL NOT NULL,
    zero_ratio        REAL NOT NULL,
    positive_ratio    REAL NOT NULL,
    negative_ratio    REAL NOT NULL,
    histogram         TEXT NOT NULL,         -- JSON
    approximate       INTEGER NOT NULL,      -- 1 = sampled, not exact
    algorithm_version INTEGER NOT NULL,
    backend           TEXT NOT NULL          -- which compute backend produced it
);

CREATE INDEX idx_statistics_subject ON tensor_statistics(subject_id);

CREATE TABLE visual_tiles (
    tile_id         TEXT PRIMARY KEY,
    parent_tile_id  TEXT,
    model_id        TEXT NOT NULL REFERENCES models(model_id) ON DELETE CASCADE,
    tensor_id       TEXT,
    lod             INTEGER NOT NULL,
    bounds          TEXT NOT NULL,           -- JSON
    geometric_error REAL NOT NULL,
    qtile_uri       TEXT,
    glb_uri         TEXT,
    child_count     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_tiles_model_lod ON visual_tiles(model_id, lod);

CREATE TABLE conversion_jobs (
    job_id        TEXT PRIMARY KEY,
    model_id      TEXT NOT NULL REFERENCES models(model_id) ON DELETE CASCADE,
    kind          TEXT NOT NULL,
    state         TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    units_total   INTEGER NOT NULL DEFAULT 0,
    units_done    INTEGER NOT NULL DEFAULT 0,
    resume_token  TEXT,
    error_message TEXT,
    requirement   TEXT
);

CREATE INDEX idx_jobs_model_state ON conversion_jobs(model_id, state);
"#,
}];

/// Apply every migration newer than the database's recorded version.
pub fn migrate(conn: &Connection) -> Result<u32> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             name       TEXT NOT NULL,
             applied_at INTEGER NOT NULL
         );",
    )
    .map_err(sql_err)?;

    let current: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .map_err(sql_err)?;

    if current > CURRENT_SCHEMA_VERSION {
        return Err(QError::Catalog(format!(
            "database schema version {current} is newer than this build supports ({CURRENT_SCHEMA_VERSION}); \
             upgrade Quatricmorph rather than downgrading the database"
        )));
    }

    for m in MIGRATIONS {
        if m.version <= current {
            continue;
        }
        conn.execute_batch(m.sql).map_err(|e| {
            QError::Catalog(format!("migration {} ({}) failed: {e}", m.version, m.name))
        })?;
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![m.version, m.name, now_unix()],
        )
        .map_err(sql_err)?;
    }

    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )
    .map_err(sql_err)
}

pub(crate) fn sql_err(e: rusqlite::Error) -> QError {
    QError::Catalog(e.to_string())
}

pub(crate) fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_cleanly_to_an_empty_database() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&conn).unwrap(), CURRENT_SCHEMA_VERSION);
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for expected in [
            "conversion_jobs",
            "models",
            "schema_migrations",
            "tensor_blocks",
            "tensor_statistics",
            "tensors",
            "visual_tiles",
        ] {
            assert!(tables.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        // A second run must not re-apply DDL (which would fail on CREATE TABLE).
        assert_eq!(migrate(&conn).unwrap(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn a_future_schema_is_refused_rather_than_corrupted() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (999, 'future', 0)",
            [],
        )
        .unwrap();
        let err = migrate(&conn).unwrap_err();
        assert!(err.to_string().contains("newer than this build"));
    }

    #[test]
    fn migration_versions_are_unique_and_ordered() {
        let mut prev = 0;
        for m in MIGRATIONS {
            assert!(m.version > prev, "migration versions must increase");
            prev = m.version;
        }
        assert_eq!(prev, CURRENT_SCHEMA_VERSION);
    }
}
