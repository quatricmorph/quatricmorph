//! # q-daemon — local HTTP API
//!
//! Data plane: serves the **Metadata Plane** and, on demand, exact reads from
//! the **Artifact Plane** (ARCHITECTURE.md §2.1, §14).
//!
//! Every route goes through `q-weightql`; none reads weight bytes directly.
//! That is the same rule ARCHITECTURE.md §15 imposes on the chat assistant, and
//! it applies here for the same reason: one query layer, one place where
//! addressing, shape checking, and fidelity labelling happen.
//!
//! ## Routes
//!
//! | route                                            | status                |
//! |--------------------------------------------------|-----------------------|
//! | `GET  /v1/models`                                | implemented           |
//! | `GET  /v1/models/{id}`                           | implemented           |
//! | `GET  /v1/models/{id}/layers`                    | implemented           |
//! | `GET  /v1/tensors/{id}`                          | implemented           |
//! | `GET  /v1/tensors/{id}/value?index=100,42`       | implemented (exact)   |
//! | `GET  /v1/tensors/{id}/blocks?rows=&columns=`    | implemented (exact)   |
//! | `POST /v1/query`                                 | scalar/slice only     |
//! | `GET  /v1/tensors/{id}/statistics`               | implemented (persisted rows only) |
//! | `GET  /v1/visualizations/{id}/tileset.json`      | **501** `CESIUM-001`  |
//! | `GET  /v1/visualizations/{id}/tiles/{tile}.glb`  | **501** `GLB-001`     |
//! | `POST /v1/conversions`                           | **501** `JOB-002`     |
//!
//! A 501 carries the requirement ID and an explanation. It never returns a
//! fabricated 200.
//!
//! ## Local file access boundary (`SEC-001`)
//!
//! The daemon is configured with one or more **model roots**. A model can only
//! be opened beneath a configured root, and
//! [`q_source::LocalFsSource::resolve`] canonicalizes before comparing, so
//! `..`, absolute paths, and symlinks pointing outside all fail. No route takes
//! a filesystem path from the client.

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use q_catalog::{Catalog, TensorFilter};
use q_nsir::{Registry, ResolvedModel};
use q_safetensors::ingest_local;
use q_source::error::QError;
use q_source::LocalFsSource;
use q_weightql::{QueryEngine, QueryOutcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A model root the daemon is allowed to read from.
#[derive(Debug, Clone)]
pub struct ModelRoot {
    pub label: String,
    pub path: PathBuf,
}

/// Daemon configuration.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub roots: Vec<ModelRoot>,
    pub bind_address: String,
    /// Where the metadata catalog lives.
    ///
    /// `None` — the default — means an in-memory catalog rebuilt from the shard
    /// headers at every start, which is what the daemon has always done. Point it
    /// at a file to serve rows another process persisted: `q stats --persist`
    /// writes `tensor_statistics`, and only a shared file lets this daemon read
    /// them. Model and tensor rows are re-upserted at bootstrap either way, so
    /// the file never has to be primed first and is never trusted over the
    /// checkpoint's own headers.
    pub catalog_path: Option<PathBuf>,
}

impl DaemonConfig {
    pub fn new(bind_address: impl Into<String>) -> Self {
        Self {
            roots: Vec::new(),
            bind_address: bind_address.into(),
            catalog_path: None,
        }
    }

    /// Serve from a catalog on disk instead of an in-memory one.
    pub fn with_catalog(mut self, path: impl Into<PathBuf>) -> Self {
        self.catalog_path = Some(path.into());
        self
    }

    /// Add a model root, canonicalizing it so the boundary check is exact.
    pub fn with_root(
        mut self,
        label: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> q_source::Result<Self> {
        let path = path.as_ref().canonicalize().map_err(|e| QError::Io {
            path: path.as_ref().to_path_buf(),
            source: e,
        })?;
        self.roots.push(ModelRoot {
            label: label.into(),
            path,
        });
        Ok(self)
    }

    /// Whether `candidate` lies within a configured root.
    ///
    /// `candidate` must already be canonicalized by the caller; this is the
    /// second half of the `SEC-001` boundary, the first being
    /// `LocalFsSource::resolve`.
    pub fn is_within_roots(&self, candidate: &Path) -> bool {
        self.roots.iter().any(|r| candidate.starts_with(&r.path))
    }
}

/// One opened model: its catalog identity and its byte source.
struct OpenModel {
    source: LocalFsSource,
    model_id: String,
}

/// Shared daemon state.
pub struct AppState {
    config: DaemonConfig,
    catalog: Catalog,
    models: BTreeMap<String, OpenModel>,
}

impl AppState {
    /// Ingest every model root and build the catalog.
    ///
    /// Metadata only: headers and the shard index. No payload is read at
    /// startup, so a daemon over a 600 GB checkpoint starts in milliseconds.
    pub fn bootstrap(config: DaemonConfig) -> q_source::Result<Arc<Self>> {
        let catalog = match &config.catalog_path {
            Some(path) => Catalog::open(path)?,
            None => Catalog::open_in_memory()?,
        };
        let registry = Registry::builtin()?;
        let mut models = BTreeMap::new();

        for root in &config.roots {
            let ingested = ingest_local(&root.path)?;
            let resolved = ResolvedModel::build(
                &registry,
                ingested.manifest.model_type().as_deref(),
                ingested.manifest.declared_architecture().as_deref(),
                ingested.descriptors.clone(),
            )?;
            catalog.upsert_resolved(
                ingested.model_id,
                &ingested.manifest.root_uri,
                &ingested.manifest.source_key,
                &ingested.manifest.revision,
                &ingested.manifest.fingerprint(),
                &resolved.resolver_id,
                &q_catalog::ConfigMetadata::from_config(ingested.manifest.config.as_ref()),
                &resolved,
            )?;
            let id = ingested.model_id.to_hex();
            models.insert(
                id.clone(),
                OpenModel {
                    source: LocalFsSource::open(&root.path)?,
                    model_id: id,
                },
            );
        }

        Ok(Arc::new(Self {
            config,
            catalog,
            models,
        }))
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    pub fn model_ids(&self) -> Vec<String> {
        self.models.keys().cloned().collect()
    }

    fn engine(&self, model_id: &str) -> Result<QueryEngine<'_>, ApiError> {
        let open = self
            .models
            .get(model_id)
            .ok_or_else(|| ApiError::from(QError::NotFound(format!("model `{model_id}`"))))?;
        QueryEngine::with_source(&self.catalog, &open.model_id, &open.source)
            .map_err(ApiError::from)
    }

    /// The model that owns a tensor.
    fn model_for_tensor(&self, tensor_id: &str) -> Result<String, ApiError> {
        let row = self
            .catalog
            .get_tensor(tensor_id)
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::from(QError::NotFound(format!("tensor `{tensor_id}`"))))?;
        Ok(row.model_id)
    }
}

/// The wire form of an error.
///
/// `requirement` is populated for 501s so a client can look the gap up in
/// `STATUS.md` rather than guess whether the feature is broken or absent.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

/// An error on its way out to a client.
///
/// The body is boxed: it carries up to five strings, and every handler returns
/// `Result<Json<T>, ApiError>`, so an unboxed body would inflate the size of
/// every success path too.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub body: Box<ApiErrorBody>,
}

impl ApiError {
    /// An explicit "not built yet" with its requirement ID.
    pub fn not_implemented(requirement: &str, message: impl Into<String>, doc: &str) -> Self {
        Self {
            status: StatusCode::NOT_IMPLEMENTED,
            body: Box::new(ApiErrorBody {
                error: "not_implemented".into(),
                message: message.into(),
                requirement: Some(requirement.to_string()),
                candidates: None,
                documentation: Some(doc.to_string()),
            }),
        }
    }
}

impl From<QError> for ApiError {
    fn from(e: QError) -> Self {
        let (status, kind, candidates) = match &e {
            QError::NotImplemented { .. } => (StatusCode::NOT_IMPLEMENTED, "not_implemented", None),
            QError::NotFound(_) | QError::MissingShard { .. } => {
                (StatusCode::NOT_FOUND, "not_found", None)
            }
            QError::PathOutsideRoot { .. } => (StatusCode::FORBIDDEN, "forbidden", None),
            QError::AmbiguousAlias { candidates, .. } => (
                StatusCode::CONFLICT,
                "ambiguous_alias",
                Some(candidates.clone()),
            ),
            QError::QueryRejected(_)
            | QError::IndexOutOfBounds { .. }
            | QError::RangeOutOfBounds { .. }
            | QError::UnsupportedDType { .. } => (StatusCode::BAD_REQUEST, "query_rejected", None),
            QError::BudgetExceeded { .. } => {
                (StatusCode::PAYLOAD_TOO_LARGE, "budget_exceeded", None)
            }
            QError::Cancelled { .. } => (StatusCode::SERVICE_UNAVAILABLE, "cancelled", None),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal", None),
        };
        let requirement = e.requirement_id().map(str::to_string);
        Self {
            status,
            body: Box::new(ApiErrorBody {
                error: kind.into(),
                message: e.to_string(),
                requirement,
                candidates,
                documentation: None,
            }),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(*self.body)).into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

// --- responses ---------------------------------------------------------------

/// The fidelity every model summary carries.
///
/// A model row is built entirely at `q_source::AccessScale::Metadata` — shapes,
/// dtypes, addresses, byte ranges, and sums over them. **No weight byte was
/// read to produce any of it.** `.plan/DATA_ARCHITECTURE.md` §8 names that
/// fidelity `metadata`; `q_source::ResultFidelity` covers only the three
/// payload-reading outcomes, so the label is spelled here and pinned by
/// `the_model_summary_fidelity_is_metadata_because_no_payload_was_read`.
pub const MODEL_SUMMARY_FIDELITY: &str = "metadata";

#[derive(Debug, Serialize)]
pub struct ModelSummary {
    pub model_id: String,
    pub source_key: String,
    pub architecture: String,
    pub resolver_id: String,
    /// Summed over the tensor descriptors. Never config arithmetic.
    pub parameter_count: u64,
    pub tensor_count: u64,
    /// Declared by `config.json`; `null` when the checkpoint has none.
    /// **Never `0`** — zero would be a lie about a model we did not read.
    pub hidden_size: Option<u32>,
    pub layer_count: Option<u32>,
    /// Summed tensor payload lengths. See the note in `ModelRow.payload_bytes`:
    /// this excludes shard headers and the index, so it is smaller than the
    /// checkpoint's size on disk.
    pub total_bytes: u64,
    pub shard_count: u64,
    /// Tensors whose semantic role is `unknown`. Surfaced, not hidden.
    pub unresolved_tensors: u64,
    /// Always [`MODEL_SUMMARY_FIDELITY`].
    pub fidelity: String,
}

fn summarize(r: q_catalog::ModelRow, unresolved: u64, shard_count: u64) -> ModelSummary {
    ModelSummary {
        model_id: r.model_id,
        source_key: r.source_key,
        architecture: r.architecture,
        resolver_id: r.resolver_id,
        parameter_count: r.parameter_count,
        tensor_count: r.tensor_count,
        hidden_size: r.hidden_size,
        layer_count: r.layer_count,
        total_bytes: r.payload_bytes,
        shard_count,
        unresolved_tensors: unresolved,
        fidelity: MODEL_SUMMARY_FIDELITY.to_string(),
    }
}

#[derive(Debug, Serialize)]
pub struct ValueResponse {
    pub canonical_name: String,
    pub index: Vec<u64>,
    pub value: f64,
    pub dtype: String,
    pub shard_uri: String,
    pub byte_offset: u64,
    pub bytes_read: u64,
    /// `"exact"`, `"sampled"`, or `"approximate"` — ARCHITECTURE.md §18 AC-010.
    pub fidelity: String,
}

#[derive(Debug, Serialize)]
pub struct SliceResponse {
    pub canonical_name: String,
    pub rows: [u64; 2],
    pub columns: [u64; 2],
    pub values: Vec<f64>,
    pub dtype: String,
    pub bytes_read: u64,
    pub fidelity: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub model: String,
    pub expression: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QueryResponse {
    /// Executed: an exact value.
    Scalar {
        plan_id: String,
        value: f64,
        canonical_name: String,
        fidelity: String,
        bytes_read: u64,
    },
    /// Executed: an exact window.
    Slice {
        plan_id: String,
        canonical_name: String,
        rows: [u64; 2],
        columns: [u64; 2],
        values: Vec<f64>,
        fidelity: String,
        bytes_read: u64,
    },
    /// Validated but not executed — no backend exists for it yet.
    Planned {
        plan_id: String,
        expression: String,
        output_shape: Vec<u64>,
        estimated_read_bytes: u64,
        matmul_count: usize,
        blocked_by: Option<String>,
        blocked_reason: Option<String>,
    },
}

/// The histogram, with its resolution stated rather than inferred from the
/// array's length by the client.
#[derive(Debug, Serialize)]
pub struct HistogramBody {
    pub bins: usize,
    pub min: f64,
    pub max: f64,
    pub counts: Vec<u64>,
}

/// `GET /v1/tensors/{id}/statistics`.
///
/// `fidelity` comes from [`q_statistics::TensorStatistics::fidelity`] — the one
/// mapping — so it cannot disagree with `approximate`. Both are on the wire
/// deliberately: the boolean is the fact, the label is what a UI badges.
#[derive(Debug, Serialize)]
pub struct StatisticsResponse {
    pub statistics_id: String,
    pub subject_id: String,
    pub subject_kind: String,
    pub count: u64,
    pub min_value: f64,
    pub max_value: f64,
    pub mean: f64,
    pub variance: f64,
    pub l1_norm: f64,
    pub l2_norm: f64,
    pub zero_ratio: f64,
    pub positive_ratio: f64,
    pub negative_ratio: f64,
    pub histogram: HistogramBody,
    /// `true` when the row was computed over a sample rather than every element.
    pub approximate: bool,
    pub algorithm_version: u32,
    /// `"aggregate"` or `"sampled"`; never spelled here, always derived.
    pub fidelity: String,
    pub backend: String,
}

impl From<q_catalog::StatisticsRow> for StatisticsResponse {
    fn from(row: q_catalog::StatisticsRow) -> Self {
        // Read before the row is destructured, so the label is derived from the
        // same value that ships in `approximate`.
        let fidelity = row.fidelity().as_str().to_string();
        let subject_kind = row.subject_kind.as_str().to_string();
        let s = row.statistics;
        Self {
            statistics_id: row.statistics_id,
            subject_id: row.subject_id,
            subject_kind,
            count: s.count,
            min_value: s.min_value,
            max_value: s.max_value,
            mean: s.mean,
            variance: s.variance,
            l1_norm: s.l1_norm,
            l2_norm: s.l2_norm,
            zero_ratio: s.zero_ratio,
            positive_ratio: s.positive_ratio,
            negative_ratio: s.negative_ratio,
            histogram: HistogramBody {
                bins: s.histogram.bins(),
                min: s.histogram.min,
                max: s.histogram.max,
                counts: s.histogram.counts,
            },
            approximate: s.approximate,
            algorithm_version: s.algorithm_version,
            fidelity,
            backend: s.backend,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StatisticsQuery {
    /// Pin a specific algorithm version. Omitted, the newest persisted version
    /// for the subject is served — and the response says which one it is.
    pub algorithm_version: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct IndexQuery {
    pub index: String,
}

#[derive(Debug, Deserialize)]
pub struct BlockQuery {
    pub rows: Option<String>,
    pub columns: Option<String>,
}

fn parse_index(s: &str) -> Result<Vec<u64>, ApiError> {
    s.split(',')
        .map(|p| {
            p.trim().parse::<u64>().map_err(|_| {
                ApiError::from(QError::QueryRejected(format!(
                    "`{p}` is not a valid index component; expected `index=100,42`"
                )))
            })
        })
        .collect()
}

fn parse_range(s: &str) -> Result<(u64, u64), ApiError> {
    let (a, b) = s.split_once(':').ok_or_else(|| {
        ApiError::from(QError::QueryRejected(format!(
            "`{s}` is not a range; expected `rows=0:256`"
        )))
    })?;
    let start = a
        .trim()
        .parse::<u64>()
        .map_err(|_| ApiError::from(QError::QueryRejected(format!("`{a}` is not an integer"))))?;
    let end = b
        .trim()
        .parse::<u64>()
        .map_err(|_| ApiError::from(QError::QueryRejected(format!("`{b}` is not an integer"))))?;
    Ok((start, end))
}

// --- handlers ----------------------------------------------------------------

pub async fn list_models(State(s): State<Arc<AppState>>) -> ApiResult<Vec<ModelSummary>> {
    let rows = s.catalog.list_models().map_err(ApiError::from)?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let unresolved = s
            .catalog
            .unresolved_count(&r.model_id)
            .map_err(ApiError::from)?;
        let shards = s.catalog.shard_count(&r.model_id).map_err(ApiError::from)?;
        out.push(summarize(r, unresolved, shards));
    }
    Ok(Json(out))
}

pub async fn get_model(
    State(s): State<Arc<AppState>>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<ModelSummary> {
    let r = s
        .catalog
        .get_model(&model_id)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::from(QError::NotFound(format!("model `{model_id}`"))))?;
    let unresolved = s
        .catalog
        .unresolved_count(&r.model_id)
        .map_err(ApiError::from)?;
    let shards = s.catalog.shard_count(&r.model_id).map_err(ApiError::from)?;
    Ok(Json(summarize(r, unresolved, shards)))
}

pub async fn list_layers(
    State(s): State<Arc<AppState>>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<Vec<q_catalog::LayerSummary>> {
    if s.catalog
        .get_model(&model_id)
        .map_err(ApiError::from)?
        .is_none()
    {
        return Err(ApiError::from(QError::NotFound(format!(
            "model `{model_id}`"
        ))));
    }
    Ok(Json(
        s.catalog.list_layers(&model_id).map_err(ApiError::from)?,
    ))
}

pub async fn list_model_tensors(
    State(s): State<Arc<AppState>>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<Vec<q_catalog::TensorRow>> {
    Ok(Json(
        s.catalog
            .list_tensors(
                &model_id,
                &TensorFilter {
                    limit: Some(1000),
                    ..Default::default()
                },
            )
            .map_err(ApiError::from)?,
    ))
}

pub async fn get_tensor(
    State(s): State<Arc<AppState>>,
    AxumPath(tensor_id): AxumPath<String>,
) -> ApiResult<q_catalog::TensorRow> {
    let row = s
        .catalog
        .get_tensor(&tensor_id)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::from(QError::NotFound(format!("tensor `{tensor_id}`"))))?;
    Ok(Json(row))
}

pub async fn get_tensor_value(
    State(s): State<Arc<AppState>>,
    AxumPath(tensor_id): AxumPath<String>,
    Query(q): Query<IndexQuery>,
) -> ApiResult<ValueResponse> {
    let index = parse_index(&q.index)?;
    let model_id = s.model_for_tensor(&tensor_id)?;
    let row = s
        .catalog
        .get_tensor(&tensor_id)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::from(QError::NotFound(format!("tensor `{tensor_id}`"))))?;
    let engine = s.engine(&model_id)?;
    let index_list = index
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let query = format!(
        r#"SELECT value FROM tensor("{}") AT [{index_list}]"#,
        row.canonical_name
    );
    match engine.run(&query).map_err(ApiError::from)? {
        QueryOutcome::Scalar { read, .. } => Ok(Json(ValueResponse {
            canonical_name: read.canonical_name,
            index: read.index,
            value: read.value,
            dtype: read.dtype.as_safetensors_str().to_string(),
            shard_uri: read.shard_uri,
            byte_offset: read.byte_offset,
            bytes_read: read.bytes_read,
            fidelity: read.fidelity.as_str().to_string(),
        })),
        other => Err(ApiError::from(QError::QueryRejected(format!(
            "expected a scalar for index [{index_list}], got {other:?}"
        )))),
    }
}

pub async fn get_tensor_blocks(
    State(s): State<Arc<AppState>>,
    AxumPath(tensor_id): AxumPath<String>,
    Query(q): Query<BlockQuery>,
) -> ApiResult<SliceResponse> {
    let row = s
        .catalog
        .get_tensor(&tensor_id)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::from(QError::NotFound(format!("tensor `{tensor_id}`"))))?;
    if row.shape.len() != 2 {
        return Err(ApiError::from(QError::QueryRejected(format!(
            "tensor `{tensor_id}` has rank {}; block reads require rank 2",
            row.shape.len()
        ))));
    }
    let rows = match q.rows.as_deref() {
        Some(r) => parse_range(r)?,
        None => (0, row.shape[0].min(64)),
    };
    let columns = match q.columns.as_deref() {
        Some(c) => parse_range(c)?,
        None => (0, row.shape[1].min(64)),
    };
    let engine = s.engine(&row.model_id)?;
    let query = format!(
        r#"SELECT slice FROM tensor("{}") ROWS {}:{} COLUMNS {}:{}"#,
        row.canonical_name, rows.0, rows.1, columns.0, columns.1
    );
    match engine.run(&query).map_err(ApiError::from)? {
        QueryOutcome::Slice { read, .. } => Ok(Json(SliceResponse {
            canonical_name: read.canonical_name,
            rows: [read.row_start, read.row_end],
            columns: [read.column_start, read.column_end],
            values: read.values,
            dtype: read.dtype.as_safetensors_str().to_string(),
            bytes_read: read.bytes_read,
            fidelity: read.fidelity.as_str().to_string(),
        })),
        other => Err(ApiError::from(QError::QueryRejected(format!(
            "expected a slice, got {other:?}"
        )))),
    }
}

pub async fn post_query(
    State(s): State<Arc<AppState>>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<QueryResponse> {
    let engine = s.engine(&req.model)?;
    let outcome = engine.run(&req.expression).map_err(ApiError::from)?;
    Ok(Json(match outcome {
        QueryOutcome::Scalar { plan, read } => QueryResponse::Scalar {
            plan_id: plan.plan_id,
            value: read.value,
            canonical_name: read.canonical_name,
            fidelity: read.fidelity.as_str().to_string(),
            bytes_read: read.bytes_read,
        },
        QueryOutcome::Slice { plan, read } => QueryResponse::Slice {
            plan_id: plan.plan_id,
            canonical_name: read.canonical_name,
            rows: [read.row_start, read.row_end],
            columns: [read.column_start, read.column_end],
            values: read.values,
            fidelity: read.fidelity.as_str().to_string(),
            bytes_read: read.bytes_read,
        },
        QueryOutcome::Planned(plan) => QueryResponse::Planned {
            plan_id: plan.plan_id,
            expression: plan.expression,
            output_shape: plan.output_shape,
            estimated_read_bytes: plan.estimated_read_bytes,
            matmul_count: plan.matmul_count,
            blocked_by: plan.blocked_by,
            blocked_reason: plan.blocked_reason,
        },
    }))
}

/// `GET /v1/tensors/{id}/statistics` — `STAT-002`, `API-005`.
///
/// Reads persisted rows. It never computes: a full-tensor statistics pass is
/// `QM-0031`, and computing a block's statistics inside a GET would read payload
/// on a metadata route.
///
/// Two distinct 404s, and the difference is the point:
///
/// * the tensor is unknown — a bad id;
/// * the tensor is known but has no statistics — nothing has been computed for
///   it. `TASK.md` §Error Handling: an empty row *"would read as all zeros"*, so
///   the route says what is missing and how to produce it instead of serving
///   zeros that look like measurements.
pub async fn get_tensor_statistics(
    State(s): State<Arc<AppState>>,
    AxumPath(tensor_id): AxumPath<String>,
    Query(q): Query<StatisticsQuery>,
) -> ApiResult<StatisticsResponse> {
    let row = s
        .catalog
        .get_tensor(&tensor_id)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::from(QError::NotFound(format!("tensor `{tensor_id}`"))))?;

    let found = match q.algorithm_version {
        Some(v) => s
            .catalog
            .get_statistics(&tensor_id, v)
            .map_err(ApiError::from)?,
        // `list_statistics` orders by `algorithm_version`, so the last row is the
        // newest algorithm held for this subject.
        None => s
            .catalog
            .list_statistics(&tensor_id)
            .map_err(ApiError::from)?
            .pop(),
    };

    let row = found.ok_or_else(|| {
        let which = match q.algorithm_version {
            Some(v) => format!(" at algorithm version {v}"),
            None => String::new(),
        };
        ApiError::from(QError::NotFound(format!(
            "no statistics for tensor `{tensor_id}` (`{}`){which}. Nothing has computed them yet; \
             run `q stats <MODEL_DIR> <ADDRESS> --persist` against a catalog this daemon was \
             started with (`--catalog`). An empty row is not returned in their place, because a \
             row of zeros would read as a measurement",
            row.canonical_name
        )))
    })?;
    Ok(Json(StatisticsResponse::from(row)))
}

// --- the 501s ----------------------------------------------------------------

pub async fn tileset_501(AxumPath(model_id): AxumPath<String>) -> ApiError {
    ApiError::not_implemented(
        "CESIUM-001",
        format!(
            "tileset.json for model `{model_id}` is not generated. Emitting a hand-written \
             tileset would render in CesiumJS and look correct while being fiction."
        ),
        "ARCHITECTURE.md §9, §10",
    )
}

pub async fn glb_tile_501(AxumPath((model_id, tile_id)): AxumPath<(String, String)>) -> ApiError {
    ApiError::not_implemented(
        "GLB-001",
        format!("GLB tile `{tile_id}` of model `{model_id}` is not generated."),
        "ARCHITECTURE.md §10",
    )
}

pub async fn qtile_501(AxumPath((model_id, tile_id)): AxumPath<(String, String)>) -> ApiError {
    ApiError::not_implemented(
        "TILE-004",
        format!(
            "`.qtile` tile `{tile_id}` of model `{model_id}` is not generated. The qtile v1 \
             container is implemented (q-tiles) but no tile pyramid has been built for this model."
        ),
        "ARCHITECTURE.md §9, §10.3",
    )
}

pub async fn conversions_501() -> ApiError {
    ApiError::not_implemented(
        "JOB-002",
        "conversion jobs cannot be started. The job state machine and its persistence exist \
         (q_catalog::ConversionJob) but no runner is wired to them.",
        "ARCHITECTURE.md §14.5, §17",
    )
}

/// Build the router.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/models/:model_id", get(get_model))
        .route("/v1/models/:model_id/layers", get(list_layers))
        .route("/v1/models/:model_id/tensors", get(list_model_tensors))
        .route("/v1/tensors/:tensor_id", get(get_tensor))
        .route("/v1/tensors/:tensor_id/value", get(get_tensor_value))
        .route("/v1/tensors/:tensor_id/blocks", get(get_tensor_blocks))
        .route(
            "/v1/tensors/:tensor_id/statistics",
            get(get_tensor_statistics),
        )
        .route("/v1/query", post(post_query))
        .route(
            "/v1/visualizations/:model_id/tileset.json",
            get(tileset_501),
        )
        .route(
            "/v1/visualizations/:model_id/tiles/:tile_id/content.glb",
            get(glb_tile_501),
        )
        .route(
            "/v1/visualizations/:model_id/tiles/:tile_id/content.qtile",
            get(qtile_501),
        )
        .route("/v1/conversions", post(conversions_501))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/tiny-llama-2shard")
            .canonicalize()
            .expect("run fixtures/generate_fixtures.py")
    }

    fn state() -> Arc<AppState> {
        let config = DaemonConfig::new("127.0.0.1:0")
            .with_root("tiny", fixture_dir())
            .unwrap();
        AppState::bootstrap(config).unwrap()
    }

    #[test]
    fn bootstrap_ingests_metadata_only() {
        let s = state();
        assert_eq!(s.model_ids().len(), 1);
        let model_id = &s.model_ids()[0];
        assert_eq!(s.catalog().tensor_count(model_id).unwrap(), 111);
    }

    #[tokio::test]
    async fn models_and_layers_are_served_from_the_catalog() {
        let s = state();
        let Json(models) = list_models(State(s.clone())).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].resolver_id, "llama");
        assert_eq!(models[0].tensor_count, 111);
        assert_eq!(models[0].layer_count, Some(12));

        let id = models[0].model_id.clone();
        let Json(layers) = list_layers(State(s), AxumPath(id)).await.unwrap();
        assert_eq!(layers.len(), 12);
    }

    #[tokio::test]
    async fn the_model_route_carries_config_metadata_and_a_metadata_fidelity() {
        let s = state();
        let id = s.model_ids()[0].clone();
        let Json(m) = get_model(State(s), AxumPath(id)).await.unwrap();
        // From fixtures/tiny-llama-2shard/config.json.
        assert_eq!(m.hidden_size, Some(48));
        assert_eq!(m.layer_count, Some(12));
        // From the manifest, never from config arithmetic. See
        // `q_safetensors::ingest::tests::total_parameters_is_the_summed_element
        // _count_not_bytes_divided_by_a_uniform_width`.
        assert_eq!(m.parameter_count, 302_256);
        assert_eq!(m.total_bytes, 1_196_736);
        assert_eq!(m.tensor_count, 111);
        assert_eq!(m.shard_count, 2);
        // No weight byte was read to produce any of the above.
        assert_eq!(m.fidelity, "metadata");
    }

    #[test]
    fn the_model_summary_fidelity_is_metadata_because_no_payload_was_read() {
        assert_eq!(MODEL_SUMMARY_FIDELITY, "metadata");
        // The label is pinned to the access scale that guarantees it, so it
        // cannot drift into a claim the pipeline does not support.
        assert!(!q_source::AccessScale::Metadata.reads_payload());
    }

    #[tokio::test]
    async fn absent_config_fields_serialize_as_null_never_zero() {
        let row = q_catalog::ModelRow {
            model_id: "deadbeef".into(),
            source_uri: "/models/x".into(),
            source_key: "local:x".into(),
            source_revision: String::new(),
            source_hash: "fp".into(),
            architecture: "unknown".into(),
            resolver_id: "generic".into(),
            parameter_count: 16,
            layer_count: None,
            hidden_size: None,
            tensor_count: 1,
            payload_bytes: 64,
            imported_at: 0,
        };
        let wire = serde_json::to_value(summarize(row, 0, 1)).unwrap();
        assert!(wire["hidden_size"].is_null(), "{wire}");
        assert!(wire["layer_count"].is_null(), "{wire}");
        assert_ne!(wire["hidden_size"], serde_json::json!(0));
        assert_ne!(wire["layer_count"], serde_json::json!(0));
        assert_eq!(wire["fidelity"], serde_json::json!("metadata"));
    }

    #[tokio::test]
    async fn an_unknown_model_is_404_not_500() {
        let s = state();
        let err = get_model(State(s), AxumPath("deadbeef".to_string()))
            .await
            .unwrap_err();
        assert_eq!(err.status, StatusCode::NOT_FOUND);
        assert_eq!(err.body.error, "not_found");
    }

    #[tokio::test]
    async fn exact_value_route_returns_the_golden_scalar() {
        let s = state();
        let model_id = s.model_ids()[0].clone();
        let row = s
            .catalog()
            .get_by_canonical_name(
                &model_id,
                "model.layers[10].self_attention.query_projection.weight",
            )
            .unwrap()
            .unwrap();
        let Json(v) = get_tensor_value(
            State(s),
            AxumPath(row.tensor_id),
            Query(IndexQuery {
                index: "100,42".into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(v.value as f32, f32::from_bits(0x3BD1FB7E));
        assert_eq!(v.fidelity, "exact");
        assert_eq!(v.bytes_read, 4);
        assert_eq!(v.dtype, "F32");
    }

    #[tokio::test]
    async fn block_route_returns_only_the_requested_window() {
        let s = state();
        let model_id = s.model_ids()[0].clone();
        let row = s
            .catalog()
            .get_by_canonical_name(
                &model_id,
                "model.layers[10].self_attention.query_projection.weight",
            )
            .unwrap()
            .unwrap();
        let Json(sl) = get_tensor_blocks(
            State(s),
            AxumPath(row.tensor_id),
            Query(BlockQuery {
                rows: Some("100:104".into()),
                columns: Some("40:44".into()),
            }),
        )
        .await
        .unwrap();
        assert_eq!(sl.values.len(), 16);
        assert_eq!(sl.bytes_read, 64);
        assert_eq!(sl.values[2] as f32, f32::from_bits(0x3BD1FB7E));
    }

    #[tokio::test]
    async fn query_route_executes_scalars_and_plans_matmuls() {
        let s = state();
        let model_id = s.model_ids()[0].clone();

        let Json(scalar) = post_query(
            State(s.clone()),
            Json(QueryRequest {
                model: model_id.clone(),
                expression: r#"show tensor("Q[10][100,42]")"#.into(),
            }),
        )
        .await
        .unwrap();
        match scalar {
            QueryResponse::Scalar {
                value, fidelity, ..
            } => {
                assert_eq!(value as f32, f32::from_bits(0x3BD1FB7E));
                assert_eq!(fidelity, "exact");
            }
            other => panic!("expected a scalar, got {other:?}"),
        }

        let Json(planned) = post_query(
            State(s),
            Json(QueryRequest {
                model: model_id,
                expression: r#"show tensor("Q[10]") @ transpose(tensor("K[10]"))"#.into(),
            }),
        )
        .await
        .unwrap();
        match planned {
            QueryResponse::Planned {
                output_shape,
                blocked_by,
                matmul_count,
                ..
            } => {
                assert_eq!(output_shape, vec![128, 32]);
                assert_eq!(matmul_count, 1);
                assert_eq!(blocked_by.as_deref(), Some("WQL-006"));
            }
            other => panic!("expected a plan, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_shape_mismatch_is_a_400_before_any_read() {
        let s = state();
        let model_id = s.model_ids()[0].clone();
        let err = post_query(
            State(s),
            Json(QueryRequest {
                model: model_id,
                expression: r#"show tensor("Q[10]") @ tensor("K[10]")"#.into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert!(err.body.message.contains("shape mismatch"));
    }

    #[tokio::test]
    async fn an_ambiguous_alias_is_a_409_carrying_its_candidates() {
        let s = state();
        let model_id = s.model_ids()[0].clone();
        let err = post_query(
            State(s),
            Json(QueryRequest {
                model: model_id,
                expression: r#"show tensor("Att[10][100,0]")"#.into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::CONFLICT);
        assert_eq!(err.body.candidates.as_ref().unwrap().len(), 4);
    }

    /// Narrowed by `QM-0020`, not deleted: `tensor_statistics_501` is gone
    /// because `STAT-002` is now served, and the other four 501s still need this
    /// assertion.
    #[tokio::test]
    async fn unbuilt_routes_return_501_with_a_requirement_id() {
        let mut seen = 0;
        for (err, want) in [
            (tileset_501(AxumPath("m".into())).await, "CESIUM-001"),
            (
                glb_tile_501(AxumPath(("m".into(), "t".into()))).await,
                "GLB-001",
            ),
            (
                qtile_501(AxumPath(("m".into(), "t".into()))).await,
                "TILE-004",
            ),
            (conversions_501().await, "JOB-002"),
        ] {
            assert_eq!(err.status, StatusCode::NOT_IMPLEMENTED);
            assert_eq!(err.body.requirement.as_deref(), Some(want));
            assert!(err.body.documentation.is_some());
            assert!(!err.body.message.is_empty());
            seen += 1;
        }
        // Four, not five: statistics moved out of this list into a real handler,
        // and the count is asserted so a route cannot quietly fall out of it.
        assert_eq!(seen, 4);
    }

    // --- statistics route ---------------------------------------------------

    /// Hand-computed from `[1, 2, 3, 4]`, the `STAT-001` fixture:
    /// mean 10/4 = 2.5, variance (2.25+0.25+0.25+2.25)/4 = 1.25, L1 = 10,
    /// L2 = sqrt(30), histogram over [1,4] in 3 bins = [1, 1, 2].
    fn hand_computed_statistics() -> q_statistics::TensorStatistics {
        q_statistics::TensorStatistics {
            count: 4,
            min_value: 1.0,
            max_value: 4.0,
            mean: 2.5,
            variance: 1.25,
            l1_norm: 10.0,
            l2_norm: 5.477225575051661,
            zero_ratio: 0.0,
            positive_ratio: 1.0,
            negative_ratio: 0.0,
            histogram: q_statistics::Histogram {
                min: 1.0,
                max: 4.0,
                counts: vec![1, 1, 2],
            },
            approximate: false,
            algorithm_version: 1,
            backend: "cpu-reference".into(),
        }
    }

    /// The `Q[10]` tensor of the fixture, whose id the route is keyed by.
    fn q10_tensor_id(s: &Arc<AppState>) -> String {
        let model_id = s.model_ids()[0].clone();
        s.catalog()
            .get_by_canonical_name(
                &model_id,
                "model.layers[10].self_attention.query_projection.weight",
            )
            .unwrap()
            .unwrap()
            .tensor_id
    }

    #[tokio::test]
    async fn the_statistics_route_serves_a_persisted_row_with_a_fidelity_label() {
        let s = state();
        let tensor_id = q10_tensor_id(&s);
        let row = q_catalog::StatisticsRow::new(
            &tensor_id,
            q_catalog::SubjectKind::Tensor,
            hand_computed_statistics(),
        )
        .unwrap();
        s.catalog().put_statistics(&row).unwrap();

        let Json(body) = get_tensor_statistics(
            State(s),
            AxumPath(tensor_id.clone()),
            Query(StatisticsQuery {
                algorithm_version: None,
            }),
        )
        .await
        .unwrap();

        assert_eq!(body.subject_id, tensor_id);
        assert_eq!(body.subject_kind, "tensor");
        assert_eq!(body.statistics_id, row.statistics_id);
        assert_eq!(body.count, 4);
        assert_eq!(body.min_value, 1.0);
        assert_eq!(body.max_value, 4.0);
        assert_eq!(body.mean, 2.5);
        assert_eq!(body.variance, 1.25);
        assert_eq!(body.l1_norm, 10.0);
        assert_eq!(body.l2_norm, 30f64.sqrt());
        assert_eq!(body.zero_ratio, 0.0);
        assert_eq!(body.positive_ratio, 1.0);
        assert_eq!(body.negative_ratio, 0.0);
        assert_eq!(body.histogram.bins, 3);
        assert_eq!(body.histogram.counts, vec![1, 1, 2]);
        assert_eq!(body.algorithm_version, 1);
        assert_eq!(body.backend, "cpu-reference");
        // The label, and the flag it is derived from, both present.
        assert!(!body.approximate);
        assert_eq!(body.fidelity, "aggregate");

        let wire = serde_json::to_value(&body).unwrap();
        assert_eq!(wire["fidelity"], serde_json::json!("aggregate"));
        assert_eq!(wire["approximate"], serde_json::json!(false));
        assert_eq!(wire["histogram"]["bins"], serde_json::json!(3));
    }

    #[tokio::test]
    async fn an_approximate_row_surfaces_as_sampled_never_as_aggregate() {
        let s = state();
        let tensor_id = q10_tensor_id(&s);
        let mut stats = hand_computed_statistics();
        stats.approximate = true;
        s.catalog()
            .put_statistics(
                &q_catalog::StatisticsRow::new(&tensor_id, q_catalog::SubjectKind::Tensor, stats)
                    .unwrap(),
            )
            .unwrap();

        let Json(body) = get_tensor_statistics(
            State(s),
            AxumPath(tensor_id),
            Query(StatisticsQuery {
                algorithm_version: None,
            }),
        )
        .await
        .unwrap();
        assert!(body.approximate);
        assert_eq!(body.fidelity, "sampled");
        assert_ne!(body.fidelity, "aggregate");
        assert_ne!(body.fidelity, "exact");
    }

    #[tokio::test]
    async fn the_wire_fidelity_is_the_single_mapping_not_a_re_spelled_literal() {
        // Whatever `q-statistics` says for each flag value is what the route
        // emits, so the two cannot be changed apart.
        let s = state();
        let tensor_id = q10_tensor_id(&s);
        for approximate in [false, true] {
            let mut stats = hand_computed_statistics();
            stats.approximate = approximate;
            let row =
                q_catalog::StatisticsRow::new(&tensor_id, q_catalog::SubjectKind::Tensor, stats)
                    .unwrap();
            s.catalog().put_statistics(&row).unwrap();
            let Json(body) = get_tensor_statistics(
                State(s.clone()),
                AxumPath(tensor_id.clone()),
                Query(StatisticsQuery {
                    algorithm_version: None,
                }),
            )
            .await
            .unwrap();
            assert_eq!(
                body.fidelity,
                q_statistics::StatisticsFidelity::from_approximate(approximate).as_str(),
                "for approximate = {approximate}"
            );
        }
    }

    #[tokio::test]
    async fn a_tensor_with_no_statistics_is_404_with_an_explanation_never_zeros() {
        let s = state();
        let tensor_id = q10_tensor_id(&s);
        let err = get_tensor_statistics(
            State(s),
            AxumPath(tensor_id),
            Query(StatisticsQuery {
                algorithm_version: None,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::NOT_FOUND);
        assert_eq!(err.body.error, "not_found");
        // The message says what is missing, how to produce it, and why an empty
        // row is not returned instead.
        assert!(err.body.message.contains("no statistics for tensor"));
        assert!(err.body.message.contains("--persist"));
        assert!(err.body.message.contains("row of zeros"));
        // It is not a 501 any more, and carries no requirement ID.
        assert_ne!(err.status, StatusCode::NOT_IMPLEMENTED);
        assert!(err.body.requirement.is_none());
    }

    #[tokio::test]
    async fn an_unknown_tensor_id_is_404_rather_than_an_empty_statistics_row() {
        let s = state();
        let err = get_tensor_statistics(
            State(s),
            AxumPath("deadbeef".to_string()),
            Query(StatisticsQuery {
                algorithm_version: None,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::NOT_FOUND);
        assert!(err.body.message.contains("tensor `deadbeef`"));
    }

    #[tokio::test]
    async fn the_route_serves_the_newest_algorithm_version_and_honours_a_pinned_one() {
        let s = state();
        let tensor_id = q10_tensor_id(&s);
        for (version, mean) in [(1u32, 2.5f64), (2, 2.75)] {
            let mut stats = hand_computed_statistics();
            stats.algorithm_version = version;
            stats.mean = mean;
            s.catalog()
                .put_statistics(
                    &q_catalog::StatisticsRow::new(
                        &tensor_id,
                        q_catalog::SubjectKind::Tensor,
                        stats,
                    )
                    .unwrap(),
                )
                .unwrap();
        }

        let Json(newest) = get_tensor_statistics(
            State(s.clone()),
            AxumPath(tensor_id.clone()),
            Query(StatisticsQuery {
                algorithm_version: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(newest.algorithm_version, 2);
        assert_eq!(newest.mean, 2.75);

        // The older algorithm is still readable — it was not overwritten.
        let Json(pinned) = get_tensor_statistics(
            State(s.clone()),
            AxumPath(tensor_id.clone()),
            Query(StatisticsQuery {
                algorithm_version: Some(1),
            }),
        )
        .await
        .unwrap();
        assert_eq!(pinned.algorithm_version, 1);
        assert_eq!(pinned.mean, 2.5);
        assert_ne!(pinned.statistics_id, newest.statistics_id);

        // A version nobody computed is 404 naming the version, not the newest row.
        let err = get_tensor_statistics(
            State(s),
            AxumPath(tensor_id),
            Query(StatisticsQuery {
                algorithm_version: Some(7),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, StatusCode::NOT_FOUND);
        assert!(err.body.message.contains("algorithm version 7"));
    }

    #[tokio::test]
    async fn the_route_serves_statistics_another_process_persisted_to_a_catalog_file() {
        // This is the `--persist` → `curl` path: one process writes the row, a
        // freshly bootstrapped daemon reads it back out of the same file.
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        let tensor_id = {
            let cat = q_catalog::Catalog::open(&db).unwrap();
            let seed = AppState::bootstrap(
                DaemonConfig::new("127.0.0.1:0")
                    .with_root("tiny", fixture_dir())
                    .unwrap()
                    .with_catalog(&db),
            )
            .unwrap();
            let id = q10_tensor_id(&seed);
            cat.put_statistics(
                &q_catalog::StatisticsRow::new(
                    &id,
                    q_catalog::SubjectKind::Tensor,
                    hand_computed_statistics(),
                )
                .unwrap(),
            )
            .unwrap();
            id
        };

        let served = AppState::bootstrap(
            DaemonConfig::new("127.0.0.1:0")
                .with_root("tiny", fixture_dir())
                .unwrap()
                .with_catalog(&db),
        )
        .unwrap();
        let Json(body) = get_tensor_statistics(
            State(served),
            AxumPath(tensor_id),
            Query(StatisticsQuery {
                algorithm_version: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(body.mean, 2.5);
        assert_eq!(body.histogram.counts, vec![1, 1, 2]);
        assert_eq!(body.fidelity, "aggregate");
    }

    #[test]
    fn the_model_root_boundary_is_enforced() {
        let config = DaemonConfig::new("127.0.0.1:0")
            .with_root("tiny", fixture_dir())
            .unwrap();
        assert!(config.is_within_roots(&fixture_dir().join("config.json")));
        assert!(!config.is_within_roots(Path::new("/etc/passwd")));
        // A root that does not exist cannot be configured at all.
        assert!(DaemonConfig::new("x")
            .with_root("nope", "/definitely/not/here")
            .is_err());
    }

    #[test]
    fn a_traversal_attempt_never_escapes_a_root() {
        let s = state();
        let open = s.models.values().next().unwrap();
        assert!(matches!(
            open.source.resolve("../../etc/passwd"),
            Err(QError::PathOutsideRoot { .. })
        ));
        assert!(matches!(
            open.source.resolve("/etc/passwd"),
            Err(QError::PathOutsideRoot { .. })
        ));
    }

    #[test]
    fn the_router_builds_with_every_documented_route() {
        let _ = router(state());
    }
}
