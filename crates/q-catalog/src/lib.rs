//! # q-catalog — Metadata Plane
//!
//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §5).
//!
//! The queryable metadata store: models, tensors, blocks, statistics, visual
//! tiles, and conversion jobs.
//!
//! ## What "trillion-scale" means here
//!
//! This catalog stores **descriptors, not weights**. A tensor row is a few
//! hundred bytes whether the tensor holds 48 parameters or 268 million. A
//! trillion-parameter MoE checkpoint has on the order of 10^5 tensors, so its
//! catalog is tens of megabytes — while its payload is terabytes that this
//! crate never opens. `tests/trillion_scale_manifest.rs` asserts exactly that,
//! with a counting allocator, and asserts equally that no payload was touched.
//!
//! Nothing in this crate reads a `.safetensors` file. Byte-range *resolution*
//! (which shard, which offset) is a metadata operation; performing the read is
//! `q-safetensors`' job.
//!
//! ## Storage engine
//!
//! SQLite via `rusqlite` with the bundled amalgamation. ARCHITECTURE.md §5
//! names DuckDB/Arrow/Parquet for this plane; the departure is deliberate,
//! scoped to this pass, and recorded in
//! `docs/decisions/ADR-003-catalog-sqlite.md`.

pub mod job;
pub mod schema;

use q_nsir::{CanonicalAddress, ResolvedModel};
use q_source::error::{QError, Result};
use q_source::role::{Component, TensorRole};
use q_source::{DType, ModelId, TensorDescriptor, TensorId};
use rusqlite::{params, Connection, OptionalExtension};
use schema::{now_unix, sql_err};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub use job::{ConversionJob, JobKind, JobState};
pub use schema::{migrate, CURRENT_SCHEMA_VERSION};

/// A model row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRow {
    pub model_id: String,
    pub source_uri: String,
    pub source_key: String,
    pub source_revision: String,
    pub source_hash: String,
    pub architecture: String,
    pub resolver_id: String,
    pub parameter_count: u64,
    pub layer_count: Option<u32>,
    pub hidden_size: Option<u32>,
    pub tensor_count: u64,
    pub payload_bytes: u64,
    pub imported_at: i64,
}

/// A tensor row: everything needed to address a tensor, nothing of its payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TensorRow {
    pub tensor_id: String,
    pub model_id: String,
    pub raw_name: String,
    pub canonical_name: String,
    pub layer_index: Option<u32>,
    pub expert_index: Option<u32>,
    pub component: String,
    pub role: String,
    pub shape: Vec<u64>,
    pub dtype: String,
    pub shard_uri: String,
    pub byte_start: u64,
    pub byte_length: u64,
    pub parameter_count: u64,
    pub resolved: bool,
}

impl TensorRow {
    /// Rebuild the descriptor the reader needs, without touching the artifact.
    pub fn to_descriptor(&self) -> Result<TensorDescriptor> {
        Ok(TensorDescriptor {
            tensor_id: TensorId::from_hex(&self.tensor_id)
                .ok_or_else(|| QError::Catalog(format!("bad tensor_id `{}`", self.tensor_id)))?,
            raw_name: self.raw_name.clone(),
            canonical_name: self.canonical_name.clone(),
            shape: self.shape.clone(),
            dtype: DType::parse_safetensors(&self.dtype)?,
            shard_uri: self.shard_uri.clone(),
            byte_start: self.byte_start,
            byte_end: self.byte_start + self.byte_length,
            layer_index: self.layer_index,
            semantic_role: TensorRole::parse(&self.role),
        })
    }
}

/// One layer in the browse hierarchy (model → layer → tensor).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSummary {
    pub layer_index: u32,
    pub tensor_count: u64,
    pub parameter_count: u64,
    pub payload_bytes: u64,
}

/// Filters for tensor listing.
#[derive(Debug, Clone, Default)]
pub struct TensorFilter {
    pub layer_index: Option<u32>,
    pub role: Option<TensorRole>,
    pub component: Option<Component>,
    pub dtype: Option<DType>,
    pub min_rank: Option<usize>,
    pub only_resolved: bool,
    pub limit: Option<u32>,
}

/// The metadata catalog.
///
/// The connection lives behind a `Mutex` so that `&Catalog` is `Sync`.
/// `rusqlite::Connection` is `Send` but not `Sync` — it has interior
/// mutability with no internal locking — and the daemon needs to share one
/// catalog across concurrently-served requests. Serializing catalog access is
/// the right trade here: every query is metadata-scale (microseconds), and the
/// expensive work (byte-range reads) happens outside the lock.
pub struct Catalog {
    conn: Mutex<Connection>,
}

impl Catalog {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref()).map_err(sql_err)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory().map_err(sql_err)?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(sql_err)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        // Poisoning would mean a prior catalog operation panicked mid-query.
        // Recovering the guard is correct here: SQLite's own state is
        // transactional, so a panicked reader cannot have left it inconsistent.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn schema_version(&self) -> Result<u32> {
        self.conn()
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
            .map_err(sql_err)
    }

    // --- writes -------------------------------------------------------------

    /// Persist a model and every tensor descriptor in one transaction.
    ///
    /// Insertion is batched through a single prepared statement so that a
    /// 10^5-tensor manifest is one transaction with a bounded working set, not
    /// 10^5 round trips.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_model(
        &self,
        model_id: ModelId,
        source_uri: &str,
        source_key: &str,
        source_revision: &str,
        source_hash: &str,
        architecture: &str,
        resolver_id: &str,
        hidden_size: Option<u32>,
        descriptors: &[TensorDescriptor],
    ) -> Result<ModelRow> {
        let parameter_count: u64 = descriptors.iter().map(|d| d.element_count()).sum();
        let payload_bytes: u64 = descriptors.iter().map(|d| d.byte_length()).sum();
        let layer_count = descriptors
            .iter()
            .filter_map(|d| d.layer_index)
            .max()
            .map(|m| m + 1);
        let row = ModelRow {
            model_id: model_id.to_hex(),
            source_uri: source_uri.to_string(),
            source_key: source_key.to_string(),
            source_revision: source_revision.to_string(),
            source_hash: source_hash.to_string(),
            architecture: architecture.to_string(),
            resolver_id: resolver_id.to_string(),
            parameter_count,
            layer_count,
            hidden_size,
            tensor_count: descriptors.len() as u64,
            payload_bytes,
            imported_at: now_unix(),
        };

        let mut guard = self.conn();
        let tx = guard.transaction().map_err(sql_err)?;
        tx.execute(
            "INSERT INTO models (model_id, source_uri, source_key, source_revision, source_hash,
                                 architecture, resolver_id, parameter_count, layer_count,
                                 hidden_size, tensor_count, payload_bytes, imported_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(model_id) DO UPDATE SET
                 source_uri=excluded.source_uri,
                 architecture=excluded.architecture,
                 resolver_id=excluded.resolver_id,
                 parameter_count=excluded.parameter_count,
                 layer_count=excluded.layer_count,
                 hidden_size=excluded.hidden_size,
                 tensor_count=excluded.tensor_count,
                 payload_bytes=excluded.payload_bytes,
                 imported_at=excluded.imported_at",
            params![
                row.model_id,
                row.source_uri,
                row.source_key,
                row.source_revision,
                row.source_hash,
                row.architecture,
                row.resolver_id,
                row.parameter_count as i64,
                row.layer_count,
                row.hidden_size,
                row.tensor_count as i64,
                row.payload_bytes as i64,
                row.imported_at,
            ],
        )
        .map_err(sql_err)?;

        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO tensors (tensor_id, model_id, raw_name, canonical_name,
                         layer_index, expert_index, component, role, shape, rank, dtype,
                         shard_uri, byte_start, byte_length, parameter_count, resolved)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
                     ON CONFLICT(tensor_id) DO UPDATE SET
                         canonical_name=excluded.canonical_name,
                         layer_index=excluded.layer_index,
                         expert_index=excluded.expert_index,
                         component=excluded.component,
                         role=excluded.role,
                         resolved=excluded.resolved",
                )
                .map_err(sql_err)?;

            for d in descriptors {
                let expert = CanonicalAddress::parse(&d.canonical_name)
                    .ok()
                    .and_then(|a| a.expert_index());
                let shape_json =
                    serde_json::to_string(&d.shape).map_err(|e| QError::json("tensor shape", e))?;
                stmt.execute(params![
                    d.tensor_id.to_hex(),
                    row.model_id,
                    d.raw_name,
                    d.canonical_name,
                    d.layer_index,
                    expert,
                    d.semantic_role.component().as_str(),
                    d.semantic_role.as_str(),
                    shape_json,
                    d.shape.len() as i64,
                    d.dtype.as_safetensors_str(),
                    d.shard_uri,
                    d.byte_start as i64,
                    d.byte_length() as i64,
                    d.element_count() as i64,
                    d.semantic_role.is_known() as i64,
                ])
                .map_err(sql_err)?;
            }
        }
        tx.commit().map_err(sql_err)?;
        Ok(row)
    }

    /// Persist an NSIR-resolved model.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_resolved(
        &self,
        model_id: ModelId,
        source_uri: &str,
        source_key: &str,
        source_revision: &str,
        source_hash: &str,
        architecture: &str,
        hidden_size: Option<u32>,
        resolved: &ResolvedModel,
    ) -> Result<ModelRow> {
        self.upsert_model(
            model_id,
            source_uri,
            source_key,
            source_revision,
            source_hash,
            architecture,
            &resolved.resolver_id,
            hidden_size,
            &resolved.descriptors,
        )
    }

    // --- reads --------------------------------------------------------------

    pub fn list_models(&self) -> Result<Vec<ModelRow>> {
        let guard = self.conn();
        let mut stmt = guard
            .prepare("SELECT * FROM models ORDER BY imported_at DESC, model_id")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([], model_from_row)
            .map_err(sql_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_err)?;
        Ok(rows)
    }

    pub fn get_model(&self, model_id: &str) -> Result<Option<ModelRow>> {
        self.conn()
            .query_row(
                "SELECT * FROM models WHERE model_id = ?1",
                params![model_id],
                model_from_row,
            )
            .optional()
            .map_err(sql_err)
    }

    /// Model → layer summaries. The hierarchy browse of ARCHITECTURE.md §9.
    pub fn list_layers(&self, model_id: &str) -> Result<Vec<LayerSummary>> {
        let guard = self.conn();
        let mut stmt = guard
            .prepare(
                "SELECT layer_index, COUNT(*), SUM(parameter_count), SUM(byte_length)
                 FROM tensors
                 WHERE model_id = ?1 AND layer_index IS NOT NULL
                 GROUP BY layer_index
                 ORDER BY layer_index",
            )
            .map_err(sql_err)?;
        let rows = stmt
            .query_map(params![model_id], |r| {
                Ok(LayerSummary {
                    layer_index: r.get::<_, i64>(0)? as u32,
                    tensor_count: r.get::<_, i64>(1)? as u64,
                    parameter_count: r.get::<_, i64>(2)? as u64,
                    payload_bytes: r.get::<_, i64>(3)? as u64,
                })
            })
            .map_err(sql_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_err)?;
        Ok(rows)
    }

    pub fn get_tensor(&self, tensor_id: &str) -> Result<Option<TensorRow>> {
        self.conn()
            .query_row(
                "SELECT * FROM tensors WHERE tensor_id = ?1",
                params![tensor_id],
                tensor_from_row,
            )
            .optional()
            .map_err(sql_err)
    }

    /// Look up by canonical address, falling back to the raw name.
    ///
    /// The fallback matters: an unresolved tensor's canonical name *is* its raw
    /// name, so both spellings must reach it.
    pub fn get_by_canonical_name(
        &self,
        model_id: &str,
        canonical_name: &str,
    ) -> Result<Option<TensorRow>> {
        let found = self
            .conn()
            .query_row(
                "SELECT * FROM tensors WHERE model_id = ?1 AND canonical_name = ?2",
                params![model_id, canonical_name],
                tensor_from_row,
            )
            .optional()
            .map_err(sql_err)?;
        if found.is_some() {
            return Ok(found);
        }
        self.conn()
            .query_row(
                "SELECT * FROM tensors WHERE model_id = ?1 AND raw_name = ?2",
                params![model_id, canonical_name],
                tensor_from_row,
            )
            .optional()
            .map_err(sql_err)
    }

    /// Tensors matching a role (and optionally a layer) — the query behind
    /// alias resolution.
    pub fn find_by_role(
        &self,
        model_id: &str,
        role: TensorRole,
        layer_index: Option<u32>,
    ) -> Result<Vec<TensorRow>> {
        let guard = self.conn();
        let mut stmt = guard
            .prepare(
                "SELECT * FROM tensors
                 WHERE model_id = ?1 AND role = ?2
                   AND (?3 IS NULL OR layer_index = ?3)
                 ORDER BY layer_index, expert_index, canonical_name",
            )
            .map_err(sql_err)?;
        let rows = stmt
            .query_map(
                params![model_id, role.as_str(), layer_index],
                tensor_from_row,
            )
            .map_err(sql_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_err)?;
        Ok(rows)
    }

    /// Filtered tensor listing (shape/dtype/role/component/layer).
    pub fn list_tensors(&self, model_id: &str, filter: &TensorFilter) -> Result<Vec<TensorRow>> {
        // Every caller-supplied value is bound as a parameter. The only strings
        // interpolated into the SQL are `&'static str`s produced by enum
        // `as_str()` methods, which cannot originate from user input.
        let mut sql = String::from("SELECT * FROM tensors WHERE model_id = ?1");
        let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(model_id.to_string())];

        if let Some(layer) = filter.layer_index {
            binds.push(Box::new(layer));
            sql.push_str(&format!(" AND layer_index = ?{}", binds.len()));
        }
        if let Some(r) = filter.min_rank {
            binds.push(Box::new(r as i64));
            sql.push_str(&format!(" AND rank >= ?{}", binds.len()));
        }
        // Enum-derived, never user input.
        if let Some(role) = filter.role {
            sql.push_str(&format!(" AND role = '{}'", role.as_str()));
        }
        if let Some(c) = filter.component {
            sql.push_str(&format!(" AND component = '{}'", c.as_str()));
        }
        if let Some(dt) = filter.dtype {
            sql.push_str(&format!(" AND dtype = '{}'", dt.as_safetensors_str()));
        }
        if filter.only_resolved {
            sql.push_str(" AND resolved = 1");
        }
        sql.push_str(" ORDER BY layer_index, canonical_name");
        if let Some(n) = filter.limit {
            // A `u32` has no SQL-significant spelling, so rendering it is safe;
            // it is not bound because `LIMIT ?` complicates the parameter
            // numbering above for no benefit.
            sql.push_str(&format!(" LIMIT {n}"));
        }

        let guard = self.conn();
        let mut stmt = guard.prepare(&sql).map_err(sql_err)?;
        let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(refs.as_slice(), tensor_from_row)
            .map_err(sql_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_err)?;
        Ok(rows)
    }

    /// Resolve a logical element index to an absolute byte range.
    ///
    /// Pure metadata arithmetic: no artifact is opened.
    pub fn resolve_byte_range(
        &self,
        model_id: &str,
        canonical_name: &str,
        index: &[u64],
    ) -> Result<(String, u64, u64)> {
        let row = self
            .get_by_canonical_name(model_id, canonical_name)?
            .ok_or_else(|| QError::NotFound(format!("tensor `{canonical_name}`")))?;
        let d = row.to_descriptor()?;
        let start = d.element_byte_offset(index)?;
        Ok((d.shard_uri, start, start + d.dtype.size_in_bytes()))
    }

    pub fn tensor_count(&self, model_id: &str) -> Result<u64> {
        self.conn()
            .query_row(
                "SELECT COUNT(*) FROM tensors WHERE model_id = ?1",
                params![model_id],
                |r| r.get::<_, i64>(0).map(|v| v as u64),
            )
            .map_err(sql_err)
    }

    pub fn unresolved_count(&self, model_id: &str) -> Result<u64> {
        self.conn()
            .query_row(
                "SELECT COUNT(*) FROM tensors WHERE model_id = ?1 AND resolved = 0",
                params![model_id],
                |r| r.get::<_, i64>(0).map(|v| v as u64),
            )
            .map_err(sql_err)
    }

    // --- jobs ---------------------------------------------------------------

    pub fn insert_job(&self, job: &ConversionJob) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO conversion_jobs (job_id, model_id, kind, state, created_at,
                     updated_at, units_total, units_done, resume_token, error_message, requirement)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    job.job_id,
                    job.model_id,
                    job.kind.as_str(),
                    job.state.as_str(),
                    job.created_at,
                    job.updated_at,
                    job.units_total as i64,
                    job.units_done as i64,
                    job.resume_token,
                    job.error_message,
                    job.requirement,
                ],
            )
            .map_err(sql_err)?;
        Ok(())
    }

    pub fn update_job(&self, job: &ConversionJob) -> Result<()> {
        let n = self
            .conn()
            .execute(
                "UPDATE conversion_jobs SET state=?2, updated_at=?3, units_total=?4,
                     units_done=?5, resume_token=?6, error_message=?7, requirement=?8
                 WHERE job_id=?1",
                params![
                    job.job_id,
                    job.state.as_str(),
                    job.updated_at,
                    job.units_total as i64,
                    job.units_done as i64,
                    job.resume_token,
                    job.error_message,
                    job.requirement,
                ],
            )
            .map_err(sql_err)?;
        if n == 0 {
            return Err(QError::NotFound(format!("job `{}`", job.job_id)));
        }
        Ok(())
    }

    pub fn get_job(&self, job_id: &str) -> Result<Option<ConversionJob>> {
        let raw = self
            .conn()
            .query_row(
                "SELECT * FROM conversion_jobs WHERE job_id = ?1",
                params![job_id],
                job_from_row,
            )
            .optional()
            .map_err(sql_err)?;
        raw.transpose()
    }

    pub fn list_jobs(&self, model_id: &str) -> Result<Vec<ConversionJob>> {
        let guard = self.conn();
        let mut stmt = guard
            .prepare("SELECT * FROM conversion_jobs WHERE model_id = ?1 ORDER BY created_at")
            .map_err(sql_err)?;
        let rows: Vec<Result<ConversionJob>> = stmt
            .query_map(params![model_id], job_from_row)
            .map_err(sql_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_err)?;
        rows.into_iter().collect()
    }
}

fn model_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ModelRow> {
    Ok(ModelRow {
        model_id: r.get("model_id")?,
        source_uri: r.get("source_uri")?,
        source_key: r.get("source_key")?,
        source_revision: r.get("source_revision")?,
        source_hash: r.get("source_hash")?,
        architecture: r.get("architecture")?,
        resolver_id: r.get("resolver_id")?,
        parameter_count: r.get::<_, i64>("parameter_count")? as u64,
        layer_count: r.get::<_, Option<i64>>("layer_count")?.map(|v| v as u32),
        hidden_size: r.get::<_, Option<i64>>("hidden_size")?.map(|v| v as u32),
        tensor_count: r.get::<_, i64>("tensor_count")? as u64,
        payload_bytes: r.get::<_, i64>("payload_bytes")? as u64,
        imported_at: r.get("imported_at")?,
    })
}

fn tensor_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TensorRow> {
    let shape_json: String = r.get("shape")?;
    let shape: Vec<u64> = serde_json::from_str(&shape_json).unwrap_or_default();
    Ok(TensorRow {
        tensor_id: r.get("tensor_id")?,
        model_id: r.get("model_id")?,
        raw_name: r.get("raw_name")?,
        canonical_name: r.get("canonical_name")?,
        layer_index: r.get::<_, Option<i64>>("layer_index")?.map(|v| v as u32),
        expert_index: r.get::<_, Option<i64>>("expert_index")?.map(|v| v as u32),
        component: r.get("component")?,
        role: r.get("role")?,
        shape,
        dtype: r.get("dtype")?,
        shard_uri: r.get("shard_uri")?,
        byte_start: r.get::<_, i64>("byte_start")? as u64,
        byte_length: r.get::<_, i64>("byte_length")? as u64,
        parameter_count: r.get::<_, i64>("parameter_count")? as u64,
        resolved: r.get::<_, i64>("resolved")? != 0,
    })
}

/// Job rows carry two enum columns that can be invalid on disk, so the row
/// mapper returns a nested `Result`: the outer one for SQLite, the inner one
/// for domain validation.
fn job_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Result<ConversionJob>> {
    let kind_str: String = r.get("kind")?;
    let state_str: String = r.get("state")?;
    let job_id: String = r.get("job_id")?;
    let model_id: String = r.get("model_id")?;
    let created_at: i64 = r.get("created_at")?;
    let updated_at: i64 = r.get("updated_at")?;
    let units_total: i64 = r.get("units_total")?;
    let units_done: i64 = r.get("units_done")?;
    let resume_token: Option<String> = r.get("resume_token")?;
    let error_message: Option<String> = r.get("error_message")?;
    let requirement: Option<String> = r.get("requirement")?;

    Ok((|| {
        Ok(ConversionJob {
            job_id,
            model_id,
            kind: JobKind::parse(&kind_str)?,
            state: JobState::parse(&state_str)?,
            created_at,
            updated_at,
            units_total: units_total as u64,
            units_done: units_done as u64,
            resume_token,
            error_message,
            requirement,
        })
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_nsir::Registry;

    fn descriptor(raw: &str, shape: Vec<u64>, start: u64) -> TensorDescriptor {
        let model = ModelId::derive("m", "", "f");
        let n: u64 = shape.iter().product();
        TensorDescriptor {
            tensor_id: TensorId::derive(model, raw),
            raw_name: raw.to_string(),
            canonical_name: raw.to_string(),
            shape,
            dtype: DType::F32,
            shard_uri: "model-00001-of-00001.safetensors".into(),
            byte_start: start,
            byte_end: start + n * 4,
            layer_index: None,
            semantic_role: TensorRole::Unknown,
        }
    }

    fn seeded() -> (Catalog, String) {
        let reg = Registry::builtin().unwrap();
        let mut ds = Vec::new();
        let mut offset = 1024u64;
        for layer in 0..3u32 {
            for (suffix, shape) in [
                ("self_attn.q_proj.weight", vec![128u64, 48]),
                ("self_attn.k_proj.weight", vec![32, 48]),
                ("mlp.down_proj.weight", vec![48, 64]),
                ("input_layernorm.weight", vec![48]),
            ] {
                let d = descriptor(&format!("model.layers.{layer}.{suffix}"), shape, offset);
                offset = d.byte_end;
                ds.push(d);
            }
        }
        ds.push(descriptor(
            "model.embed_tokens.weight",
            vec![64, 48],
            offset,
        ));
        ds.push(descriptor(
            "mystery.tensor.weight",
            vec![2, 2],
            offset + 100_000,
        ));

        let resolved = ResolvedModel::build(&reg, Some("llama"), None, ds).unwrap();
        let model_id = ModelId::derive("local:test", "", "fp");
        let cat = Catalog::open_in_memory().unwrap();
        cat.upsert_resolved(
            model_id,
            "/models/test",
            "local:test",
            "",
            "fp",
            "llama",
            Some(48),
            &resolved,
        )
        .unwrap();
        (cat, model_id.to_hex())
    }

    #[test]
    fn schema_is_at_the_current_version_after_open() {
        let cat = Catalog::open_in_memory().unwrap();
        assert_eq!(cat.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn model_and_tensors_persist_with_derived_totals() {
        let (cat, model_id) = seeded();
        let m = cat.get_model(&model_id).unwrap().unwrap();
        assert_eq!(m.tensor_count, 14);
        assert_eq!(m.layer_count, Some(3));
        assert_eq!(m.resolver_id, "llama");
        assert_eq!(m.hidden_size, Some(48));
        let expected: u64 = 3 * (128 * 48 + 32 * 48 + 48 * 64 + 48) + 64 * 48 + 4;
        assert_eq!(m.parameter_count, expected);
        assert_eq!(cat.list_models().unwrap().len(), 1);
    }

    #[test]
    fn hierarchy_browse_returns_one_summary_per_layer() {
        let (cat, model_id) = seeded();
        let layers = cat.list_layers(&model_id).unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].layer_index, 0);
        assert_eq!(layers[0].tensor_count, 4);
        assert_eq!(layers[0].parameter_count, 128 * 48 + 32 * 48 + 48 * 64 + 48);
    }

    #[test]
    fn canonical_address_lookup_and_raw_name_fallback() {
        let (cat, model_id) = seeded();
        let t = cat
            .get_by_canonical_name(
                &model_id,
                "model.layers[1].self_attention.query_projection.weight",
            )
            .unwrap()
            .unwrap();
        assert_eq!(t.raw_name, "model.layers.1.self_attn.q_proj.weight");
        assert_eq!(t.shape, vec![128, 48]);
        assert!(cat.get_tensor(&t.tensor_id).unwrap().is_some());

        let m = cat
            .get_by_canonical_name(&model_id, "mystery.tensor.weight")
            .unwrap()
            .unwrap();
        assert!(!m.resolved);
        assert_eq!(m.role, "unknown");
    }

    #[test]
    fn role_and_layer_filters_drive_alias_resolution() {
        let (cat, model_id) = seeded();
        let all_q = cat
            .find_by_role(&model_id, TensorRole::AttentionQueryProjection, None)
            .unwrap();
        assert_eq!(all_q.len(), 3);
        let one = cat
            .find_by_role(&model_id, TensorRole::AttentionQueryProjection, Some(2))
            .unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].layer_index, Some(2));
    }

    #[test]
    fn shape_dtype_and_resolution_filters_work() {
        let (cat, model_id) = seeded();
        let rank2 = cat
            .list_tensors(
                &model_id,
                &TensorFilter {
                    min_rank: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(rank2.iter().all(|t| t.shape.len() >= 2));

        let resolved_only = cat
            .list_tensors(
                &model_id,
                &TensorFilter {
                    only_resolved: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(resolved_only.iter().all(|t| t.resolved));
        assert_eq!(cat.unresolved_count(&model_id).unwrap(), 1);

        let attention = cat
            .list_tensors(
                &model_id,
                &TensorFilter {
                    component: Some(Component::Attention),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(attention.len(), 6); // q + k across 3 layers

        let by_layer = cat
            .list_tensors(
                &model_id,
                &TensorFilter {
                    layer_index: Some(1),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(by_layer.len(), 4);

        let limited = cat
            .list_tensors(
                &model_id,
                &TensorFilter {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(limited.len(), 2);

        let f32_only = cat
            .list_tensors(
                &model_id,
                &TensorFilter {
                    dtype: Some(DType::BF16),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(f32_only.is_empty());
    }

    #[test]
    fn byte_range_resolution_is_pure_metadata_arithmetic() {
        let (cat, model_id) = seeded();
        let (shard, start, end) = cat
            .resolve_byte_range(
                &model_id,
                "model.layers[0].self_attention.query_projection.weight",
                &[100, 42],
            )
            .unwrap();
        assert_eq!(shard, "model-00001-of-00001.safetensors");
        assert_eq!(end - start, 4);
        assert_eq!(start, 1024 + (100 * 48 + 42) * 4);
    }

    #[test]
    fn byte_range_resolution_rejects_out_of_bounds_indices() {
        let (cat, model_id) = seeded();
        assert!(matches!(
            cat.resolve_byte_range(
                &model_id,
                "model.layers[0].self_attention.query_projection.weight",
                &[128, 0],
            ),
            Err(QError::IndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn reimporting_the_same_model_is_idempotent() {
        let (cat, model_id) = seeded();
        let before = cat.tensor_count(&model_id).unwrap();
        let reg = Registry::builtin().unwrap();
        let ds = vec![descriptor("model.embed_tokens.weight", vec![64, 48], 0)];
        let resolved = ResolvedModel::build(&reg, Some("llama"), None, ds).unwrap();
        cat.upsert_resolved(
            ModelId::from_hex(&model_id).unwrap(),
            "/models/test",
            "local:test",
            "",
            "fp",
            "llama",
            Some(48),
            &resolved,
        )
        .unwrap();
        assert_eq!(cat.tensor_count(&model_id).unwrap(), before);
    }

    #[test]
    fn jobs_persist_and_reload_with_their_state() {
        let (cat, model_id) = seeded();
        let mut job = ConversionJob::new("job-1", &model_id, JobKind::TilePyramid);
        job.units_total = 128;
        job.requirement = Some("TILE-004".into());
        cat.insert_job(&job).unwrap();

        job.transition(JobState::Running).unwrap();
        job.units_done = 32;
        job.resume_token = Some(r#"{"completed_shards":["s1"]}"#.into());
        cat.update_job(&job).unwrap();

        let back = cat.get_job("job-1").unwrap().unwrap();
        assert_eq!(back.state, JobState::Running);
        assert_eq!(back.units_done, 32);
        assert_eq!(back.progress(), 0.25);
        assert_eq!(back.requirement.as_deref(), Some("TILE-004"));
        assert_eq!(cat.list_jobs(&model_id).unwrap().len(), 1);
    }

    #[test]
    fn updating_a_missing_job_is_not_found() {
        let (cat, model_id) = seeded();
        let job = ConversionJob::new("ghost", &model_id, JobKind::StatisticsPass);
        assert!(matches!(cat.update_job(&job), Err(QError::NotFound(_))));
    }

    #[test]
    fn catalog_survives_close_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        let model_id = ModelId::derive("local:x", "", "fp");
        {
            let reg = Registry::builtin().unwrap();
            let ds = vec![descriptor(
                "model.layers.0.self_attn.q_proj.weight",
                vec![128, 48],
                8,
            )];
            let resolved = ResolvedModel::build(&reg, Some("llama"), None, ds).unwrap();
            let cat = Catalog::open(&path).unwrap();
            cat.upsert_resolved(
                model_id, "/x", "local:x", "", "fp", "llama", None, &resolved,
            )
            .unwrap();
        }
        let cat = Catalog::open(&path).unwrap();
        assert_eq!(cat.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        assert_eq!(cat.tensor_count(&model_id.to_hex()).unwrap(), 1);
    }
}
