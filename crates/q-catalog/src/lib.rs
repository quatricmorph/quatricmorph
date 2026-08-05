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

use q_architecture::ModelConfigMetadata;
use q_nsir::{CanonicalAddress, ResolvedModel};
use q_source::error::{QError, Result};
use q_source::ids::ID_SCHEME_VERSION;
use q_source::role::{Component, TensorRole};
use q_source::{DType, ModelId, TensorDescriptor, TensorId};
use q_statistics::{Histogram, StatisticsFidelity, TensorStatistics};
use rusqlite::{params, Connection, OptionalExtension};
use schema::{now_unix, sql_err};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub use job::{ConversionJob, JobKind, JobState};
pub use schema::{migrate, CURRENT_SCHEMA_VERSION};

/// Re-exported so callers that persist a model do not each need a direct
/// dependency on `q-architecture`.
pub use q_architecture::ModelConfigMetadata as ConfigMetadata;

/// How many layers the descriptors themselves show.
///
/// `max(layer_index) + 1`, over the descriptors a resolver annotated. This is
/// **observed**: it comes from the shard headers, so it is exact for the
/// tensors that were described. It is `None` when no descriptor carries a layer
/// index — the generic-fallback case — which is the only situation in which
/// `config.json`'s `num_hidden_layers` is consulted
/// ([`ModelConfigMetadata::layer_count`]).
pub fn observed_layer_count(descriptors: &[TensorDescriptor]) -> Option<u32> {
    descriptors
        .iter()
        .filter_map(|d| d.layer_index)
        .max()
        .map(|m| m + 1)
}

/// Sum a per-descriptor quantity, refusing to wrap.
///
/// A `u64` cannot overflow at 10¹² parameters (≈2⁴⁰), so this can only fire on
/// a corrupt or hostile manifest — which is exactly when a silently wrapped
/// total would be worst.
fn checked_sum(
    descriptors: &[TensorDescriptor],
    what: &str,
    f: impl Fn(&TensorDescriptor) -> u64,
) -> Result<u64> {
    descriptors.iter().try_fold(0u64, |acc, d| {
        acc.checked_add(f(d)).ok_or_else(|| {
            QError::Catalog(format!(
                "{what} overflows u64 while summing tensor `{}`",
                d.raw_name
            ))
        })
    })
}

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

// --- statistics identity and serialization -----------------------------------

/// The hash domain of a [`StatisticsId`]
/// (`docs/decisions/ADR-011-content-derived-identifiers.md`).
pub const STATISTICS_ID_DOMAIN: &str = "quatricmorph/statistics/v1";

/// The largest histogram this catalog will persist.
///
/// A row's histogram blob is `20 + 8 × bins` bytes
/// (see [`encode_histogram`]), so 256 bins is 2 068 bytes. The design point is
/// [`q_statistics::DEFAULT_HISTOGRAM_BINS`] = 64, i.e. 532 bytes; at the ~47 000
/// tensors of `tests/trillion_scale_manifest.rs` that is 25 MB of histogram,
/// which is what keeps this crate's "the catalog is tens of megabytes" claim
/// true. The ceiling is four times the design point — 97 MB at the same row
/// count — and a request above it is **refused**, not truncated: a silently
/// shortened histogram would mis-state the distribution it claims to describe.
pub const MAX_HISTOGRAM_BINS: usize = 256;

/// Fixed bytes of a histogram blob: `u32` bin count, `f64` min, `f64` max.
const HISTOGRAM_BLOB_HEADER_BYTES: usize = 4 + 8 + 8;

/// Stable identifier for one statistics row.
///
/// `blake3(ID_SCHEME_VERSION ‖ domain ‖ 0x00 ‖ len‖subject_id ‖ len‖version_le)`,
/// truncated to 16 bytes — byte-for-byte the construction `q_source::ids`
/// applies to `ModelId` and `TensorId`, so persisted IDs are identical to what
/// `define_id!` would produce.
///
/// **Why the code is here and not in `q_source::ids`.**
/// `docs/decisions/ADR-011-content-derived-identifiers.md` says new ID kinds go
/// through that crate's `define_id!` macro. `q-source` is outside this task's
/// program boundary and is concurrently owned by another task, so the
/// construction is reproduced here instead of moved. It is reproduced
/// *exactly*: `statistics_ids_follow_the_shipped_digest_construction_byte_for_byte`
/// re-derives the digest from the ADR's byte layout independently and pins the
/// result to a literal, so relocating this into `q_source::ids` later cannot
/// change a single persisted ID.
///
/// **One discrepancy in ADR-011 is resolved deliberately.** Its prose rule says
/// fixed-width components (`[u8;16]`, `u32`) are appended *without* a length
/// prefix, and its Consequences restate the formula that way. The shipped
/// `q_source::ids::digest16` it claims to codify length-prefixes **every**
/// component, including the `[u8;16]` `model_id` inside `TensorId::derive`.
/// `QM-0020`'s own `TASK.md` §Scope agrees with the code:
/// `blake3(len‖subject_id ‖ len‖algorithm_version)`. The code and the task win
/// over the ADR's prose, because matching the shipped construction is what makes
/// this ID kind the same scheme rather than a second one.
///
/// `subject_kind` is **not** hashed — a `TensorId` and a `TileId` are already
/// separated by their own domains, so the 16 bytes of `subject_id` carry the
/// kind implicitly (ADR-011, point 2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StatisticsId(pub [u8; 16]);

impl StatisticsId {
    pub const DOMAIN: &'static str = STATISTICS_ID_DOMAIN;

    /// Derive from the subject's raw 16 bytes and the algorithm version.
    pub fn derive(subject_id: [u8; 16], algorithm_version: u32) -> Self {
        let mut h = blake3::Hasher::new();
        h.update(&[ID_SCHEME_VERSION]);
        h.update(Self::DOMAIN.as_bytes());
        h.update(&[0]);
        for part in [
            subject_id.as_slice(),
            algorithm_version.to_le_bytes().as_slice(),
        ] {
            h.update(&(part.len() as u64).to_le_bytes());
            h.update(part);
        }
        let mut out = [0u8; 16];
        out.copy_from_slice(&h.finalize().as_bytes()[..16]);
        Self(out)
    }

    /// Derive from the persisted 32-hex spelling of a subject.
    ///
    /// Every subject in this catalog — `TensorId`, `TileId`/`BlockId` — is
    /// stored as bare lowercase 32-hex (ADR-011, point 4), so this is the form
    /// callers actually hold. Anything else is refused rather than padded,
    /// truncated, or hashed as text.
    pub fn derive_from_hex(subject_id: &str, algorithm_version: u32) -> Result<Self> {
        Ok(Self::derive(
            parse_subject_id(subject_id)?,
            algorithm_version,
        ))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        parse_subject_id(s).ok().map(Self)
    }
}

impl fmt::Debug for StatisticsId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StatisticsId({})", self.to_hex())
    }
}

impl fmt::Display for StatisticsId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// A 32-hex subject identifier → its 16 raw bytes.
fn parse_subject_id(s: &str) -> Result<[u8; 16]> {
    if s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(QError::Catalog(format!(
            "`{s}` is not a subject id; expected 32 lowercase hex characters"
        )));
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|e| QError::Catalog(format!("`{s}` is not a subject id: {e}")))?;
    }
    Ok(out)
}

/// What a statistics row describes.
///
/// A column for readability, never an identity component (ADR-011, point 2).
/// There are exactly two kinds, and an unrecognized string — including the
/// `'unknown'` default that migration 2 leaves on a pre-existing row — is
/// **refused**, never resolved to whichever is more likely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    /// The subject is a whole tensor; `subject_id` is its `TensorId`.
    Tensor,
    /// The subject is one block of a tensor; `subject_id` is its
    /// `BlockId` — which is `TileId::for_block`, per ADR-011.
    Block,
}

impl SubjectKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tensor => "tensor",
            Self::Block => "block",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "tensor" => Ok(Self::Tensor),
            "block" => Ok(Self::Block),
            other => Err(QError::Catalog(format!(
                "`{other}` is not a statistics subject kind; expected `tensor` or `block`. \
                 A row whose kind was never recorded is refused rather than guessed at"
            ))),
        }
    }
}

/// Serialize a histogram to the persisted blob.
///
/// Little-endian on every host, matching `.qtile`
/// (`.plan/DATA_ARCHITECTURE.md` §6), because a host-endian blob would read back
/// as noise on a different machine and the catalog file is portable.
///
/// ```text
/// u32          bins        length prefix; 0 means "no histogram was computed"
/// f64          min
/// f64          max
/// bins × u64   counts
/// ```
///
/// `20 + 8 × bins` bytes; 532 for the 64-bin default. `TASK.md`'s "64 × u64 =
/// 512 bytes" counts the payload only — the 20-byte header carries the length
/// prefix the same section asks for, plus the range the counts are meaningless
/// without.
pub fn encode_histogram(h: &Histogram) -> Result<Vec<u8>> {
    if h.counts.len() > MAX_HISTOGRAM_BINS {
        return Err(QError::BudgetExceeded {
            budget_name: "histogram_bins",
            requested: h.counts.len() as u64,
            limit: MAX_HISTOGRAM_BINS as u64,
        });
    }
    let mut out = Vec::with_capacity(HISTOGRAM_BLOB_HEADER_BYTES + 8 * h.counts.len());
    out.extend_from_slice(&(h.counts.len() as u32).to_le_bytes());
    out.extend_from_slice(&h.min.to_le_bytes());
    out.extend_from_slice(&h.max.to_le_bytes());
    for c in &h.counts {
        out.extend_from_slice(&c.to_le_bytes());
    }
    Ok(out)
}

/// Parse a persisted histogram blob, refusing anything that is not exactly one.
///
/// A blob of unexpected length is **malformed**, not best-effort: half a
/// histogram would render as a distribution with missing mass, which looks like
/// data rather than like corruption.
pub fn decode_histogram(bytes: &[u8]) -> Result<Histogram> {
    if bytes.len() < HISTOGRAM_BLOB_HEADER_BYTES {
        return Err(QError::malformed(
            "tensor_statistics.histogram",
            format!(
                "{} bytes is shorter than the {HISTOGRAM_BLOB_HEADER_BYTES}-byte histogram header",
                bytes.len()
            ),
        ));
    }
    let bins = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if bins > MAX_HISTOGRAM_BINS {
        return Err(QError::malformed(
            "tensor_statistics.histogram",
            format!("blob declares {bins} bins; the ceiling is {MAX_HISTOGRAM_BINS}"),
        ));
    }
    let expected = HISTOGRAM_BLOB_HEADER_BYTES + 8 * bins;
    if bytes.len() != expected {
        return Err(QError::malformed(
            "tensor_statistics.histogram",
            format!(
                "blob declares {bins} bins, which needs {expected} bytes, but it is {} bytes long",
                bytes.len()
            ),
        ));
    }
    let f64_at = |o: usize| {
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[o..o + 8]);
        f64::from_le_bytes(b)
    };
    let mut counts = Vec::with_capacity(bins);
    for i in 0..bins {
        let o = HISTOGRAM_BLOB_HEADER_BYTES + 8 * i;
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[o..o + 8]);
        counts.push(u64::from_le_bytes(b));
    }
    Ok(Histogram {
        min: f64_at(4),
        max: f64_at(12),
        counts,
    })
}

/// One `tensor_statistics` row: a subject, its kind, and the numbers.
///
/// [`StatisticsRow::new`] is the constructor that validates, and it derives
/// `statistics_id` rather than taking it. The fields are `pub` and `Deserialize`
/// is derived, though, so a struct literal or a deserialized row **can** carry an
/// id that disagrees with its `(subject_id, algorithm_version)`. That is checked
/// again at the persistence boundary — [`Catalog::put_statistics_batch`] refuses
/// such a row before opening its transaction — which is where the invariant has
/// to hold, because it is the identity nothing else could reproduce.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatisticsRow {
    pub statistics_id: String,
    pub subject_id: String,
    pub subject_kind: SubjectKind,
    pub statistics: TensorStatistics,
}

impl StatisticsRow {
    /// Validate and mint a row.
    ///
    /// Everything checkable is checked **before** any SQL runs: the subject id's
    /// spelling, the histogram's size against [`MAX_HISTOGRAM_BINS`], and the
    /// histogram's own consistency — every element that was counted landed in
    /// exactly one bin, so a present histogram must total the element count.
    pub fn new(
        subject_id: &str,
        subject_kind: SubjectKind,
        statistics: TensorStatistics,
    ) -> Result<Self> {
        let raw = parse_subject_id(subject_id)?;
        if statistics.count == 0 {
            return Err(QError::QueryRejected(
                "a statistics row over zero elements would describe nothing; refused".into(),
            ));
        }
        if statistics.histogram.counts.len() > MAX_HISTOGRAM_BINS {
            return Err(QError::BudgetExceeded {
                budget_name: "histogram_bins",
                requested: statistics.histogram.counts.len() as u64,
                limit: MAX_HISTOGRAM_BINS as u64,
            });
        }
        if !statistics.histogram.counts.is_empty()
            && statistics.histogram.total() != statistics.count
        {
            return Err(QError::QueryRejected(format!(
                "histogram counts sum to {} but the statistic covers {} elements; \
                 every counted element lands in exactly one bin, so these must agree",
                statistics.histogram.total(),
                statistics.count
            )));
        }
        Ok(Self {
            statistics_id: StatisticsId::derive(raw, statistics.algorithm_version).to_hex(),
            subject_id: subject_id.to_string(),
            subject_kind,
            statistics,
        })
    }

    /// The `.plan/DATA_ARCHITECTURE.md` §8 label, from the single mapping in
    /// `q-statistics`. Never re-spelled here.
    pub fn fidelity(&self) -> StatisticsFidelity {
        self.statistics.fidelity()
    }

    pub fn algorithm_version(&self) -> u32 {
        self.statistics.algorithm_version
    }
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
    ///
    /// `config` supplies the *declared* dimensions (`ARCHITECTURE.md` §5.1's
    /// `hidden_size`, and `num_hidden_layers` as a fallback only). Every count
    /// — `parameter_count`, `payload_bytes`, `tensor_count`, and `layer_count`
    /// whenever the descriptors show one — is summed from the descriptors
    /// instead, because config arithmetic would be an estimate and this is a
    /// number the UI presents as fact. No payload is read either way: a
    /// descriptor already carries its shape and byte range.
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
        config: &ModelConfigMetadata,
        descriptors: &[TensorDescriptor],
    ) -> Result<ModelRow> {
        let parameter_count = checked_sum(
            descriptors,
            "parameter_count",
            TensorDescriptor::element_count,
        )?;
        let payload_bytes =
            checked_sum(descriptors, "payload_bytes", TensorDescriptor::byte_length)?;
        let layer_count = config.layer_count(observed_layer_count(descriptors));
        let hidden_size = config.hidden_size;
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
        config: &ModelConfigMetadata,
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
            config,
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

    /// How many distinct shards hold this model's tensors.
    ///
    /// Derived from the tensor rows rather than stored, so it cannot disagree
    /// with them. Pure metadata: no artifact is opened.
    pub fn shard_count(&self, model_id: &str) -> Result<u64> {
        self.conn()
            .query_row(
                "SELECT COUNT(DISTINCT shard_uri) FROM tensors WHERE model_id = ?1",
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

    // --- statistics ---------------------------------------------------------

    /// Persist one statistics row.
    pub fn put_statistics(&self, row: &StatisticsRow) -> Result<()> {
        self.put_statistics_batch(std::slice::from_ref(row))?;
        Ok(())
    }

    /// Persist many rows in **one transaction**.
    ///
    /// A statistics pass produces one row per block, and row-per-block inserts
    /// outside a transaction are a measured risk
    /// (`.plan/PERFORMANCE_PLAN.md` §5): SQLite would fsync per row. One
    /// prepared statement inside one transaction keeps the working set bounded
    /// regardless of how many blocks a tensor was cut into.
    ///
    /// A second write at an `algorithm_version` that already exists for the
    /// subject **replaces** it. A different version does not: it mints a new row,
    /// because `StatisticsId` hashes the version, so two algorithms coexist and
    /// can be compared instead of one silently erasing the other's history.
    pub fn put_statistics_batch(&self, rows: &[StatisticsRow]) -> Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }
        // Every check happens before the transaction opens, so a bad row aborts
        // without having held a write lock or written a partial batch.
        //
        // The id is re-derived rather than trusted. `StatisticsRow`'s fields are
        // `pub` and it derives `Deserialize`, so a row can reach here without
        // having gone through `StatisticsRow::new` — and an id that is not
        // `derive(subject_id, algorithm_version)` would either be written under
        // an identity nothing can reproduce, or collide with the
        // `(subject_id, algorithm_version)` unique index and surface as a raw
        // SQLite constraint error instead of a reasoned refusal.
        for r in rows {
            let expected =
                StatisticsId::derive_from_hex(&r.subject_id, r.statistics.algorithm_version)?;
            if expected.to_hex() != r.statistics_id {
                return Err(QError::Catalog(format!(
                    "statistics row `{}` carries an id that is not derived from its subject \
                     `{}` and algorithm version {} (expected `{}`); it is refused rather than \
                     written under an identity nothing can reproduce",
                    r.statistics_id,
                    r.subject_id,
                    r.statistics.algorithm_version,
                    expected.to_hex()
                )));
            }
        }
        let blobs = rows
            .iter()
            .map(|r| encode_histogram(&r.statistics.histogram))
            .collect::<Result<Vec<_>>>()?;

        let mut guard = self.conn();
        let tx = guard.transaction().map_err(sql_err)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO tensor_statistics (statistics_id, subject_id, subject_kind, count,
                         min_value, max_value, mean, variance, l1_norm, l2_norm, zero_ratio,
                         positive_ratio, negative_ratio, histogram, approximate,
                         algorithm_version, backend)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
                     ON CONFLICT(statistics_id) DO UPDATE SET
                         subject_kind=excluded.subject_kind,
                         count=excluded.count,
                         min_value=excluded.min_value,
                         max_value=excluded.max_value,
                         mean=excluded.mean,
                         variance=excluded.variance,
                         l1_norm=excluded.l1_norm,
                         l2_norm=excluded.l2_norm,
                         zero_ratio=excluded.zero_ratio,
                         positive_ratio=excluded.positive_ratio,
                         negative_ratio=excluded.negative_ratio,
                         histogram=excluded.histogram,
                         approximate=excluded.approximate,
                         backend=excluded.backend",
                )
                .map_err(sql_err)?;
            for (row, blob) in rows.iter().zip(&blobs) {
                let s = &row.statistics;
                stmt.execute(params![
                    row.statistics_id,
                    row.subject_id,
                    row.subject_kind.as_str(),
                    s.count as i64,
                    s.min_value,
                    s.max_value,
                    s.mean,
                    s.variance,
                    s.l1_norm,
                    s.l2_norm,
                    s.zero_ratio,
                    s.positive_ratio,
                    s.negative_ratio,
                    blob,
                    s.approximate as i64,
                    s.algorithm_version,
                    s.backend,
                ])
                .map_err(sql_err)?;
            }
        }
        tx.commit().map_err(sql_err)?;
        Ok(rows.len())
    }

    /// One subject's statistics at one algorithm version.
    ///
    /// `Ok(None)` means no such row. Callers must surface that as "not found"
    /// and never as a row of zeros — a zero-filled statistic is a claim about
    /// the weights, and it would be false.
    pub fn get_statistics(
        &self,
        subject_id: &str,
        algorithm_version: u32,
    ) -> Result<Option<StatisticsRow>> {
        let raw = self
            .conn()
            .query_row(
                "SELECT * FROM tensor_statistics
                 WHERE subject_id = ?1 AND algorithm_version = ?2",
                params![subject_id, algorithm_version],
                statistics_from_row,
            )
            .optional()
            .map_err(sql_err)?;
        raw.transpose()
    }

    /// Every algorithm version held for one subject, oldest version first.
    ///
    /// Indexed by `subject_id` (`idx_statistics_subject`, migration 1).
    pub fn list_statistics(&self, subject_id: &str) -> Result<Vec<StatisticsRow>> {
        let guard = self.conn();
        let mut stmt = guard
            .prepare(
                "SELECT * FROM tensor_statistics
                 WHERE subject_id = ?1
                 ORDER BY algorithm_version",
            )
            .map_err(sql_err)?;
        let rows: Vec<Result<StatisticsRow>> = stmt
            .query_map(params![subject_id], statistics_from_row)
            .map_err(sql_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_err)?;
        rows.into_iter().collect()
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

/// A statistics row carries three fields that can be invalid on disk, so — like
/// [`job_from_row`] — the mapper returns a nested `Result`: the outer one for
/// SQLite, the inner one for domain validation.
///
/// The three, and why each refuses rather than defaults:
///
/// * `subject_kind` — `'unknown'` is what migration 2 leaves on a row written
///   before the column existed. Reading it as `tensor` would invent a fact.
/// * `histogram` — must be a well-formed little-endian blob. A `TEXT` value left
///   by some other writer (migration 1's comment described JSON) fails the
///   length check and is refused, not reinterpreted.
/// * `approximate` — a row that does not carry it cannot be read at all. Without
///   it there is no fidelity label, and an unlabelled statistic is exactly what
///   `AC-010` exists to prevent. `TASK.md` §Error Handling: *"Fidelity mapping is
///   not optional: a row without `approximate` cannot be read."*
fn statistics_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Result<StatisticsRow>> {
    let statistics_id: String = r.get("statistics_id")?;
    let subject_id: String = r.get("subject_id")?;
    let subject_kind: String = r.get("subject_kind")?;
    let count: i64 = r.get("count")?;
    let min_value: f64 = r.get("min_value")?;
    let max_value: f64 = r.get("max_value")?;
    let mean: f64 = r.get("mean")?;
    let variance: f64 = r.get("variance")?;
    let l1_norm: f64 = r.get("l1_norm")?;
    let l2_norm: f64 = r.get("l2_norm")?;
    let zero_ratio: f64 = r.get("zero_ratio")?;
    let positive_ratio: f64 = r.get("positive_ratio")?;
    let negative_ratio: f64 = r.get("negative_ratio")?;
    // `Value`, not `Vec<u8>`: migration 1 declares this column `TEXT`, so a
    // `TEXT` value is legal SQLite and must be refused with a *reason* rather
    // than surfacing as rusqlite's bare "Invalid column type".
    let histogram: rusqlite::types::Value = r.get("histogram")?;
    // `Option<i64>` rather than `i64`: a NULL must become a refusal with a
    // reason, not a rusqlite type error with none.
    let approximate: Option<i64> = r.get("approximate")?;
    let algorithm_version: i64 = r.get("algorithm_version")?;
    let backend: String = r.get("backend")?;

    Ok((|| {
        let approximate = approximate.ok_or_else(|| {
            QError::malformed(
                "tensor_statistics",
                format!(
                    "row `{statistics_id}` has no `approximate` flag, so no fidelity label can \
                     be derived for it; an unlabelled statistic is refused rather than served"
                ),
            )
        })? != 0;
        let histogram = match histogram {
            rusqlite::types::Value::Blob(b) => b,
            other => {
                return Err(QError::malformed(
                    "tensor_statistics.histogram",
                    format!(
                        "row `{statistics_id}` stores a {} where a little-endian histogram blob \
                         belongs; it is refused rather than reinterpreted",
                        match other {
                            rusqlite::types::Value::Null => "NULL",
                            rusqlite::types::Value::Integer(_) => "integer",
                            rusqlite::types::Value::Real(_) => "real",
                            rusqlite::types::Value::Text(_) => "text value",
                            rusqlite::types::Value::Blob(_) => unreachable!(),
                        }
                    ),
                ))
            }
        };
        Ok(StatisticsRow {
            statistics_id,
            subject_id,
            subject_kind: SubjectKind::parse(&subject_kind)?,
            statistics: TensorStatistics {
                count: count as u64,
                min_value,
                max_value,
                mean,
                variance,
                l1_norm,
                l2_norm,
                zero_ratio,
                positive_ratio,
                negative_ratio,
                histogram: decode_histogram(&histogram)?,
                approximate,
                algorithm_version: algorithm_version as u32,
                backend,
            },
        })
    })())
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

    /// A config declaring only the fields a test cares about.
    fn declared(hidden_size: Option<u32>, num_hidden_layers: Option<u32>) -> ModelConfigMetadata {
        ModelConfigMetadata {
            hidden_size,
            num_hidden_layers,
            ..Default::default()
        }
    }

    fn seeded() -> (Catalog, String) {
        seeded_with(declared(Some(48), None))
    }

    fn seeded_with(config: ModelConfigMetadata) -> (Catalog, String) {
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
            &config,
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
    fn config_declared_hidden_size_persists_and_reloads() {
        let (cat, model_id) = seeded_with(declared(Some(48), Some(12)));
        assert_eq!(
            cat.get_model(&model_id).unwrap().unwrap().hidden_size,
            Some(48)
        );
    }

    #[test]
    fn observed_layer_count_wins_over_a_disagreeing_declared_one() {
        // The descriptors describe three layers; the config claims twelve. The
        // artifact is the authority, so the declared value never overwrites it.
        let (cat, model_id) = seeded_with(declared(Some(48), Some(12)));
        assert_eq!(
            cat.get_model(&model_id).unwrap().unwrap().layer_count,
            Some(3)
        );
    }

    #[test]
    fn declared_layer_count_fills_in_only_when_none_was_observed() {
        // The generic-fallback case: no plugin claimed the model, so no
        // descriptor carries a layer index and nothing is observed. Reporting
        // NULL for a model whose own config says twelve would throw away a
        // fact we hold, so the declared value is used — and only here.
        let cat = Catalog::open_in_memory().unwrap();
        let model_id = ModelId::derive("local:generic", "", "fp");
        let ds = vec![descriptor("mystery.tensor.weight", vec![4, 4], 0)];
        assert!(ds.iter().all(|d| d.layer_index.is_none()));
        cat.upsert_model(
            model_id,
            "/models/generic",
            "local:generic",
            "",
            "fp",
            "unknown",
            "generic",
            &declared(None, Some(12)),
            &ds,
        )
        .unwrap();
        let m = cat.get_model(&model_id.to_hex()).unwrap().unwrap();
        assert_eq!(m.layer_count, Some(12));
        assert_eq!(m.hidden_size, None);
    }

    #[test]
    fn a_model_without_a_config_persists_null_columns_never_zero() {
        let cat = Catalog::open_in_memory().unwrap();
        let model_id = ModelId::derive("local:noconfig", "", "fp");
        let ds = vec![descriptor("mystery.tensor.weight", vec![4, 4], 0)];
        cat.upsert_model(
            model_id,
            "/models/noconfig",
            "local:noconfig",
            "",
            "fp",
            "unknown",
            "generic",
            &ModelConfigMetadata::default(),
            &ds,
        )
        .unwrap();
        let hex = model_id.to_hex();
        let m = cat.get_model(&hex).unwrap().unwrap();
        assert_eq!(m.hidden_size, None);
        assert_eq!(m.layer_count, None);
        // SQL NULL, not 0. Zero would be a lie, and a UI cannot tell the two
        // apart once the column has been coerced.
        let stored: (Option<i64>, Option<i64>) = cat
            .conn()
            .query_row(
                "SELECT hidden_size, layer_count FROM models WHERE model_id = ?1",
                params![hex],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, (None, None));
    }

    #[test]
    fn persisted_parameter_count_is_summed_from_descriptors_not_from_config_arithmetic() {
        let (cat, model_id) = seeded_with(declared(Some(48), Some(12)));
        let m = cat.get_model(&model_id).unwrap().unwrap();
        let from_manifest: u64 = 3 * (128 * 48 + 32 * 48 + 48 * 64 + 48) + 64 * 48 + 4;
        assert_eq!(m.parameter_count, from_manifest);
        // The obvious config-arithmetic estimate — hidden_size × layers, or any
        // product of the declared dimensions — does not reproduce it, which is
        // the point: only the manifest can.
        assert_ne!(m.parameter_count, 48 * 12);
        assert_ne!(m.parameter_count, u64::from(48u32) * u64::from(3u32));
    }

    #[test]
    fn shard_count_is_the_number_of_distinct_shards_described() {
        let cat = Catalog::open_in_memory().unwrap();
        let model_id = ModelId::derive("local:two", "", "fp");
        let mut a = descriptor("model.layers.0.self_attn.q_proj.weight", vec![4, 4], 0);
        a.shard_uri = "model-00001-of-00002.safetensors".into();
        let mut b = descriptor("model.layers.1.self_attn.q_proj.weight", vec![4, 4], 0);
        b.shard_uri = "model-00002-of-00002.safetensors".into();
        let mut c = descriptor("model.layers.1.mlp.down_proj.weight", vec![4, 4], 64);
        c.shard_uri = "model-00002-of-00002.safetensors".into();
        let reg = Registry::builtin().unwrap();
        let resolved = ResolvedModel::build(&reg, Some("llama"), None, vec![a, b, c]).unwrap();
        cat.upsert_resolved(
            model_id,
            "/models/two",
            "local:two",
            "",
            "fp",
            "llama",
            &ModelConfigMetadata::default(),
            &resolved,
        )
        .unwrap();
        assert_eq!(cat.shard_count(&model_id.to_hex()).unwrap(), 2);
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
            &declared(Some(48), None),
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

    // --- statistics ---------------------------------------------------------

    /// Every expected number below is computed **by hand** from `[1, 2, 3, 4]`,
    /// the same fixture and the same arithmetic as
    /// `q_statistics::tests::hand_computed_moments_on_a_small_fixture`
    /// (`STAT-001`). Nothing here is read back out of `q-statistics`:
    ///
    /// ```text
    /// count    = 4
    /// min/max  = 1 / 4
    /// mean     = 10/4                                  = 2.5
    /// variance = (1.5² + 0.5² + 0.5² + 1.5²)/4          = 1.25
    /// L1       = 1+2+3+4                               = 10
    /// L2       = sqrt(1+4+9+16) = sqrt(30)             = 5.477225575051661
    /// ratios   = 0 zero, 4 positive, 0 negative        = 0 / 1 / 0
    /// hist     = range [1,4] over 3 bins, edges 1,2,3,4
    ///            1→bin 0, 2→bin 1, 3→bin 2, 4→clamped into bin 2
    ///                                                  = [1, 1, 2]
    /// ```
    const HAND: (u64, f64, f64, f64, f64, f64) = (4, 1.0, 4.0, 2.5, 1.25, 10.0);

    fn hand_computed_statistics() -> TensorStatistics {
        TensorStatistics {
            count: HAND.0,
            min_value: HAND.1,
            max_value: HAND.2,
            mean: HAND.3,
            variance: HAND.4,
            l1_norm: HAND.5,
            l2_norm: 5.477225575051661,
            zero_ratio: 0.0,
            positive_ratio: 1.0,
            negative_ratio: 0.0,
            histogram: Histogram {
                min: 1.0,
                max: 4.0,
                counts: vec![1, 1, 2],
            },
            approximate: false,
            algorithm_version: 1,
            backend: "cpu-reference".into(),
        }
    }

    /// A syntactically valid 32-hex subject id, spelled out rather than derived.
    const SUBJECT_A: &str = "0123456789abcdef0123456789abcdef";
    const SUBJECT_B: &str = "fedcba9876543210fedcba9876543210";

    /// Re-derive `StatisticsId` from ADR-011's byte layout, independently of
    /// [`StatisticsId::derive`]. This is the reference the implementation is
    /// checked against; it exists so the expectation does not come from the code
    /// under test.
    fn statistics_id_reference(subject_hex: &str, algorithm_version: u32) -> String {
        let mut raw = [0u8; 16];
        for i in 0..16 {
            raw[i] = u8::from_str_radix(&subject_hex[i * 2..i * 2 + 2], 16).unwrap();
        }
        let mut h = blake3::Hasher::new();
        h.update(&[1u8]); // ID_SCHEME_VERSION, spelled literally
        h.update(b"quatricmorph/statistics/v1");
        h.update(&[0]);
        // len‖subject_id, then len‖algorithm_version — TASK.md §Scope, and what
        // `q_source::ids::digest16` does to every component it is given.
        h.update(&16u64.to_le_bytes());
        h.update(&raw);
        h.update(&4u64.to_le_bytes());
        h.update(&algorithm_version.to_le_bytes());
        h.finalize().as_bytes()[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[test]
    fn statistics_ids_follow_the_shipped_digest_construction_byte_for_byte() {
        // The literal is the output of `statistics_id_reference` above — a
        // transcription of ADR-011's byte layout — not of the code under test.
        const GOLDEN_A_V1: &str = "4b0df4930f8ee4bb1637bcfbcf49499c";
        assert_eq!(statistics_id_reference(SUBJECT_A, 1), GOLDEN_A_V1);
        assert_eq!(
            StatisticsId::derive_from_hex(SUBJECT_A, 1)
                .unwrap()
                .to_hex(),
            GOLDEN_A_V1
        );
        // 32 lowercase hex, the frozen persisted form (ADR-011, point 4).
        assert_eq!(GOLDEN_A_V1.len(), 32);
        assert!(GOLDEN_A_V1
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(
            StatisticsId::from_hex(GOLDEN_A_V1).unwrap().to_hex(),
            GOLDEN_A_V1
        );
    }

    #[test]
    fn a_new_algorithm_version_mints_a_new_statistics_id_rather_than_reusing_one() {
        let v1 = StatisticsId::derive_from_hex(SUBJECT_A, 1).unwrap();
        let v2 = StatisticsId::derive_from_hex(SUBJECT_A, 2).unwrap();
        assert_ne!(v1, v2, "changing the algorithm must not overwrite history");
        // Same inputs, same id: stable across calls and therefore across reopen.
        assert_eq!(v1, StatisticsId::derive_from_hex(SUBJECT_A, 1).unwrap());
        // Different subjects never share an id.
        assert_ne!(v1, StatisticsId::derive_from_hex(SUBJECT_B, 1).unwrap());
    }

    #[test]
    fn a_subject_id_that_is_not_thirty_two_hex_characters_is_refused() {
        for bad in ["", "abc", "0123456789abcdef0123456789abcde", "zz"] {
            let err = StatisticsId::derive_from_hex(bad, 1).unwrap_err();
            assert!(
                err.to_string().contains("not a subject id"),
                "for `{bad}`: {err}"
            );
        }
        // Non-hex of the right length is refused too, rather than hashed as text.
        assert!(StatisticsId::derive_from_hex(&"g".repeat(32), 1).is_err());
    }

    #[test]
    fn an_unrecognized_subject_kind_is_refused_rather_than_guessed() {
        assert_eq!(SubjectKind::parse("tensor").unwrap(), SubjectKind::Tensor);
        assert_eq!(SubjectKind::parse("block").unwrap(), SubjectKind::Block);
        for bad in ["unknown", "Tensor", "layer", ""] {
            let err = SubjectKind::parse(bad).unwrap_err();
            assert!(
                err.to_string().contains("not a statistics subject kind"),
                "for `{bad}`: {err}"
            );
            assert!(err.to_string().contains("refused rather than guessed"));
        }
    }

    #[test]
    fn a_sixty_four_bin_histogram_round_trips_through_the_blob_exactly() {
        // Counts chosen so that no two bins share a value: a byte-order or
        // stride mistake could not produce the same vector back.
        let counts: Vec<u64> = (0..64u64).map(|i| i * 7 + 1).collect();
        let h = Histogram {
            min: -0.31,
            max: 0.29,
            counts: counts.clone(),
        };
        let blob = encode_histogram(&h).unwrap();
        // 4 (bins) + 8 (min) + 8 (max) + 64 × 8 (counts).
        assert_eq!(blob.len(), 20 + 512);
        // Little-endian, asserted rather than assumed: bins = 64 is 0x40 in the
        // first byte and zero in the next three.
        assert_eq!(&blob[..4], &[0x40, 0x00, 0x00, 0x00]);
        assert_eq!(&blob[20..28], &1u64.to_le_bytes());
        assert_eq!(&blob[28..36], &8u64.to_le_bytes());

        let back = decode_histogram(&blob).unwrap();
        assert_eq!(back.counts, counts);
        assert_eq!(back.bins(), 64);
        assert_eq!(back.min.to_bits(), (-0.31f64).to_bits());
        assert_eq!(back.max.to_bits(), 0.29f64.to_bits());
        assert_eq!(back, h);
    }

    #[test]
    fn a_histogram_blob_of_unexpected_length_is_refused_as_malformed() {
        let h = Histogram {
            min: 0.0,
            max: 1.0,
            counts: vec![1, 2, 3, 4],
        };
        let blob = encode_histogram(&h).unwrap();

        // Truncated inside the counts.
        let err = decode_histogram(&blob[..blob.len() - 3]).unwrap_err();
        assert!(err.to_string().contains("declares 4 bins"), "{err}");
        // Shorter than the header.
        let err = decode_histogram(&[0u8; 7]).unwrap_err();
        assert!(err.to_string().contains("histogram header"), "{err}");
        // Empty.
        assert!(decode_histogram(&[]).is_err());
        // A JSON value in the column — what migration 1's comment described — is
        // refused rather than reinterpreted.
        let err = decode_histogram(br#"{"min":0.0,"max":1.0,"counts":[1,2,3,4]}"#).unwrap_err();
        assert!(err.to_string().contains("malformed"), "{err}");
        // One count too many for the declared binize.
        let mut long = blob.clone();
        long.extend_from_slice(&9u64.to_le_bytes());
        assert!(decode_histogram(&long).is_err());
    }

    #[test]
    fn a_histogram_above_the_bin_ceiling_is_refused_rather_than_truncated() {
        let h = Histogram {
            min: 0.0,
            max: 1.0,
            counts: vec![1; MAX_HISTOGRAM_BINS + 1],
        };
        assert!(matches!(
            encode_histogram(&h),
            Err(QError::BudgetExceeded {
                budget_name: "histogram_bins",
                ..
            })
        ));
        // At the ceiling it is accepted, so the boundary is inclusive.
        let ok = Histogram {
            min: 0.0,
            max: 1.0,
            counts: vec![1; MAX_HISTOGRAM_BINS],
        };
        assert_eq!(
            encode_histogram(&ok).unwrap().len(),
            20 + 8 * MAX_HISTOGRAM_BINS
        );
        // A blob that *claims* more bins than the ceiling is refused on read too,
        // before it can drive an allocation.
        let mut hostile = vec![0u8; 20];
        hostile[..4].copy_from_slice(&(u32::MAX).to_le_bytes());
        assert!(decode_histogram(&hostile).is_err());

        // And the row builder refuses it before any SQL runs.
        let mut stats = hand_computed_statistics();
        stats.histogram = h;
        assert!(matches!(
            StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, stats),
            Err(QError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn statistics_persist_and_read_back_with_hand_computed_values() {
        let cat = Catalog::open_in_memory().unwrap();
        let row =
            StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, hand_computed_statistics()).unwrap();
        cat.put_statistics(&row).unwrap();

        let back = cat.get_statistics(SUBJECT_A, 1).unwrap().unwrap();
        assert_eq!(back, row);
        let s = &back.statistics;
        assert_eq!(s.count, HAND.0);
        assert_eq!(s.min_value.to_bits(), HAND.1.to_bits());
        assert_eq!(s.max_value.to_bits(), HAND.2.to_bits());
        assert_eq!(s.mean.to_bits(), HAND.3.to_bits());
        assert_eq!(s.variance.to_bits(), HAND.4.to_bits());
        assert_eq!(s.l1_norm.to_bits(), HAND.5.to_bits());
        assert_eq!(s.l2_norm.to_bits(), 30f64.sqrt().to_bits());
        assert_eq!(s.zero_ratio, 0.0);
        assert_eq!(s.positive_ratio, 1.0);
        assert_eq!(s.negative_ratio, 0.0);
        assert_eq!(s.histogram.counts, vec![1, 1, 2]);
        assert_eq!(s.backend, "cpu-reference");
        assert_eq!(s.algorithm_version, 1);
        assert!(!s.approximate);
        assert_eq!(back.subject_kind, SubjectKind::Tensor);
        assert_eq!(
            back.statistics_id,
            statistics_id_reference(SUBJECT_A, 1),
            "the persisted id must be the derived one"
        );
        assert_eq!(back.fidelity(), StatisticsFidelity::Aggregate);
    }

    #[test]
    fn statistics_survive_close_and_reopen_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        let row =
            StatisticsRow::new(SUBJECT_A, SubjectKind::Block, hand_computed_statistics()).unwrap();
        {
            let cat = Catalog::open(&path).unwrap();
            cat.put_statistics(&row).unwrap();
        }
        let cat = Catalog::open(&path).unwrap();
        assert_eq!(cat.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        let back = cat.get_statistics(SUBJECT_A, 1).unwrap().unwrap();
        // Byte-identical, including the histogram and the subject kind.
        assert_eq!(back, row);
        assert_eq!(back.subject_kind, SubjectKind::Block);
        assert_eq!(back.statistics.histogram.counts, vec![1, 1, 2]);
    }

    #[test]
    fn two_algorithm_versions_coexist_for_one_subject() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut v1 = hand_computed_statistics();
        v1.algorithm_version = 1;
        let mut v2 = hand_computed_statistics();
        v2.algorithm_version = 2;
        v2.mean = 2.75; // a different formula would give a different number
        let r1 = StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, v1).unwrap();
        let r2 = StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, v2).unwrap();
        assert_ne!(r1.statistics_id, r2.statistics_id);
        cat.put_statistics_batch(&[r1.clone(), r2.clone()]).unwrap();

        assert_eq!(cat.get_statistics(SUBJECT_A, 1).unwrap().unwrap(), r1);
        assert_eq!(cat.get_statistics(SUBJECT_A, 2).unwrap().unwrap(), r2);
        let all = cat.list_statistics(SUBJECT_A).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].algorithm_version(), 1);
        assert_eq!(all[1].algorithm_version(), 2);
        assert_eq!(all[1].statistics.mean, 2.75);
    }

    #[test]
    fn rewriting_one_algorithm_version_replaces_it_rather_than_duplicating() {
        let cat = Catalog::open_in_memory().unwrap();
        let first =
            StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, hand_computed_statistics()).unwrap();
        cat.put_statistics(&first).unwrap();

        let mut revised = hand_computed_statistics();
        revised.mean = 2.6;
        revised.backend = "cpu-reference".into();
        let second = StatisticsRow::new(SUBJECT_A, SubjectKind::Block, revised).unwrap();
        assert_eq!(second.statistics_id, first.statistics_id);
        cat.put_statistics(&second).unwrap();

        let all = cat.list_statistics(SUBJECT_A).unwrap();
        assert_eq!(all.len(), 1, "the later write must replace, not duplicate");
        assert_eq!(all[0].statistics.mean, 2.6);
        assert_eq!(all[0].subject_kind, SubjectKind::Block);
    }

    #[test]
    fn a_subject_with_no_statistics_reads_as_absent_never_as_zeros() {
        let cat = Catalog::open_in_memory().unwrap();
        assert!(cat.get_statistics(SUBJECT_A, 1).unwrap().is_none());
        assert!(cat.list_statistics(SUBJECT_A).unwrap().is_empty());
        // Writing one subject must not conjure a row for another.
        cat.put_statistics(
            &StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, hand_computed_statistics())
                .unwrap(),
        )
        .unwrap();
        assert!(cat.get_statistics(SUBJECT_B, 1).unwrap().is_none());
        // Nor for a version that was never written.
        assert!(cat.get_statistics(SUBJECT_A, 99).unwrap().is_none());
    }

    #[test]
    fn an_absent_histogram_persists_as_zero_bins_not_as_sixty_four_zero_counts() {
        // `StatisticsAccumulator::finish` with no bound range yields an empty
        // `counts`. Storing 64 zeros instead would read as "every bin empty",
        // which is a statement about the distribution rather than the absence of
        // one — the same lie as persisting an unknown count as 0.
        let cat = Catalog::open_in_memory().unwrap();
        let mut stats = hand_computed_statistics();
        stats.histogram = Histogram {
            min: 1.0,
            max: 4.0,
            counts: Vec::new(),
        };
        let row = StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, stats).unwrap();
        cat.put_statistics(&row).unwrap();

        let back = cat.get_statistics(SUBJECT_A, 1).unwrap().unwrap();
        assert_eq!(back.statistics.histogram.counts, Vec::<u64>::new());
        assert_eq!(back.statistics.histogram.bins(), 0);
        assert_ne!(back.statistics.histogram.counts, vec![0u64; 64]);
        // On disk it is the 20-byte header alone: no counts at all, rather than
        // 64 zeroed slots.
        let blob: Vec<u8> = cat
            .conn()
            .query_row(
                "SELECT histogram FROM tensor_statistics WHERE subject_id = ?1",
                params![SUBJECT_A],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(blob.len(), 20);
        assert_eq!(&blob[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn the_persisted_histogram_is_a_little_endian_blob_not_json() {
        let cat = Catalog::open_in_memory().unwrap();
        cat.put_statistics(
            &StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, hand_computed_statistics())
                .unwrap(),
        )
        .unwrap();
        // Migration 1 declares `histogram TEXT`; SQLite's TEXT affinity stores a
        // BLOB value as a BLOB unchanged, and this asserts that it did.
        let (kind, blob): (String, Vec<u8>) = cat
            .conn()
            .query_row(
                "SELECT typeof(histogram), histogram FROM tensor_statistics WHERE subject_id = ?1",
                params![SUBJECT_A],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "blob");
        assert_eq!(blob.len(), 20 + 3 * 8);
        assert_eq!(&blob[..4], &[3, 0, 0, 0]); // three bins, little-endian
        assert_eq!(&blob[20..28], &1u64.to_le_bytes());
        assert_eq!(&blob[36..44], &2u64.to_le_bytes());
        assert!(!blob.starts_with(b"{"), "a JSON object was stored");
    }

    #[test]
    fn a_sampled_row_is_persisted_and_read_back_as_sampled() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut stats = hand_computed_statistics();
        stats.approximate = true;
        let row = StatisticsRow::new(SUBJECT_A, SubjectKind::Block, stats).unwrap();
        assert_eq!(row.fidelity(), StatisticsFidelity::Sampled);
        cat.put_statistics(&row).unwrap();

        let back = cat.get_statistics(SUBJECT_A, 1).unwrap().unwrap();
        assert!(back.statistics.approximate);
        assert_eq!(back.fidelity(), StatisticsFidelity::Sampled);
        assert_eq!(back.fidelity().as_str(), "sampled");
        // Stored as 1, so the flag survives independently of the label.
        let stored: i64 = cat
            .conn()
            .query_row(
                "SELECT approximate FROM tensor_statistics WHERE subject_id = ?1",
                params![SUBJECT_A],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, 1);
    }

    #[test]
    fn a_row_without_an_approximate_flag_cannot_be_read() {
        // Migration 1 declares `approximate INTEGER NOT NULL`, so this catalog's
        // own writer cannot produce such a row. A database written by another
        // tool, or restored from a partial migration, can — and the reader must
        // refuse it with a reason rather than defaulting the flag to `false` and
        // serving a statistic labelled `aggregate` on no evidence.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tensor_statistics (
                 statistics_id TEXT PRIMARY KEY, subject_id TEXT NOT NULL,
                 subject_kind TEXT NOT NULL, count INTEGER NOT NULL,
                 min_value REAL NOT NULL, max_value REAL NOT NULL, mean REAL NOT NULL,
                 variance REAL NOT NULL, l1_norm REAL NOT NULL, l2_norm REAL NOT NULL,
                 zero_ratio REAL NOT NULL, positive_ratio REAL NOT NULL,
                 negative_ratio REAL NOT NULL, histogram BLOB NOT NULL,
                 approximate INTEGER, algorithm_version INTEGER NOT NULL,
                 backend TEXT NOT NULL);",
        )
        .unwrap();
        let blob = encode_histogram(&hand_computed_statistics().histogram).unwrap();
        conn.execute(
            "INSERT INTO tensor_statistics VALUES ('sid', ?1, 'tensor', 4, 1.0, 4.0, 2.5, 1.25,
                 10.0, 5.477225575051661, 0.0, 1.0, 0.0, ?2, NULL, 1, 'cpu-reference')",
            params![SUBJECT_A, blob],
        )
        .unwrap();

        let err = conn
            .query_row("SELECT * FROM tensor_statistics", [], statistics_from_row)
            .unwrap()
            .unwrap_err();
        assert!(
            err.to_string().contains("no `approximate` flag"),
            "expected a fidelity refusal, got: {err}"
        );
        assert!(err.to_string().contains("refused rather than served"));

        // With the flag present the very same row reads fine, so the refusal is
        // about the missing label and nothing else.
        conn.execute("UPDATE tensor_statistics SET approximate = 1", [])
            .unwrap();
        let ok = conn
            .query_row("SELECT * FROM tensor_statistics", [], statistics_from_row)
            .unwrap()
            .unwrap();
        assert_eq!(ok.fidelity(), StatisticsFidelity::Sampled);
    }

    #[test]
    fn a_row_whose_subject_kind_was_never_recorded_cannot_be_read() {
        let cat = Catalog::open_in_memory().unwrap();
        cat.put_statistics(
            &StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, hand_computed_statistics())
                .unwrap(),
        )
        .unwrap();
        cat.conn()
            .execute("UPDATE tensor_statistics SET subject_kind = 'unknown'", [])
            .unwrap();
        let err = cat.get_statistics(SUBJECT_A, 1).unwrap_err();
        assert!(
            err.to_string().contains("not a statistics subject kind"),
            "{err}"
        );
    }

    #[test]
    fn a_row_whose_histogram_is_not_a_blob_cannot_be_read() {
        let cat = Catalog::open_in_memory().unwrap();
        cat.put_statistics(
            &StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, hand_computed_statistics())
                .unwrap(),
        )
        .unwrap();
        cat.conn()
            .execute(
                "UPDATE tensor_statistics SET histogram = ?1",
                params![r#"{"min":1.0,"max":4.0,"counts":[1,1,2]}"#],
            )
            .unwrap();
        let err = cat.get_statistics(SUBJECT_A, 1).unwrap_err();
        assert!(err.to_string().contains("malformed"), "{err}");
        assert!(err.to_string().contains("histogram"), "{err}");
        // Named as a text value, and explicitly refused rather than parsed.
        assert!(err.to_string().contains("text value"), "{err}");
        assert!(
            err.to_string()
                .contains("refused rather than reinterpreted"),
            "{err}"
        );
    }

    #[test]
    fn a_histogram_that_does_not_account_for_every_element_is_refused_before_the_insert() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut stats = hand_computed_statistics();
        // Four elements, but the bins only account for three.
        stats.histogram.counts = vec![1, 1, 1];
        let err = StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, stats).unwrap_err();
        assert!(err.to_string().contains("sum to 3"), "{err}");
        assert!(err.to_string().contains("4 elements"), "{err}");
        // Nothing reached the database.
        assert!(cat.list_statistics(SUBJECT_A).unwrap().is_empty());
    }

    #[test]
    fn a_statistics_row_over_zero_elements_is_refused() {
        let mut stats = hand_computed_statistics();
        stats.count = 0;
        stats.histogram.counts = Vec::new();
        let err = StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, stats).unwrap_err();
        assert!(err.to_string().contains("zero elements"), "{err}");
    }

    #[test]
    fn a_row_whose_id_was_not_derived_from_its_subject_and_version_is_refused() {
        // `StatisticsRow`'s fields are `pub` and it derives `Deserialize`, so a
        // row can reach the writer without having passed through `new`. The
        // persistence boundary re-derives the id rather than trusting it.
        let cat = Catalog::open_in_memory().unwrap();
        let mut row =
            StatisticsRow::new(SUBJECT_A, SubjectKind::Tensor, hand_computed_statistics()).unwrap();
        row.statistics_id = "deadbeefdeadbeefdeadbeefdeadbeef".into();
        let err = cat.put_statistics(&row).unwrap_err();
        assert!(
            err.to_string().contains("not derived from its subject"),
            "{err}"
        );
        assert!(err
            .to_string()
            .contains(&statistics_id_reference(SUBJECT_A, 1)));
        assert!(cat.list_statistics(SUBJECT_A).unwrap().is_empty());

        // The same mismatch inside a batch aborts the whole batch, so a good row
        // beside a bad one is not written either.
        let good =
            StatisticsRow::new(SUBJECT_B, SubjectKind::Block, hand_computed_statistics()).unwrap();
        assert!(cat.put_statistics_batch(&[good, row]).is_err());
        assert!(cat.list_statistics(SUBJECT_B).unwrap().is_empty());
    }

    #[test]
    fn a_batch_of_statistics_is_written_in_one_transaction() {
        let cat = Catalog::open_in_memory().unwrap();
        let rows: Vec<StatisticsRow> = (0..8u32)
            .map(|i| {
                let subject = format!("{:032x}", i);
                let mut stats = hand_computed_statistics();
                stats.mean = i as f64;
                StatisticsRow::new(&subject, SubjectKind::Block, stats).unwrap()
            })
            .collect();
        assert_eq!(cat.put_statistics_batch(&rows).unwrap(), 8);
        for (i, r) in rows.iter().enumerate() {
            let back = cat.get_statistics(&r.subject_id, 1).unwrap().unwrap();
            assert_eq!(back.statistics.mean, i as f64);
            assert_eq!(back.subject_kind, SubjectKind::Block);
        }
        // An empty batch is a no-op rather than an error.
        assert_eq!(cat.put_statistics_batch(&[]).unwrap(), 0);
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
                model_id,
                "/x",
                "local:x",
                "",
                "fp",
                "llama",
                &ModelConfigMetadata::default(),
                &resolved,
            )
            .unwrap();
        }
        let cat = Catalog::open(&path).unwrap();
        assert_eq!(cat.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        assert_eq!(cat.tensor_count(&model_id.to_hex()).unwrap(), 1);
    }
}
