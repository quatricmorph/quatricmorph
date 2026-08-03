//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1).
//!
//! The single error type shared by every Quatricmorph crate.
//!
//! Two variants deserve special note because they encode project rules rather
//! than failure modes:
//!
//! * [`QError::NotImplemented`] — the *only* sanctioned way for an unbuilt
//!   subsystem to answer. Stubs must never return fabricated-but-plausible
//!   data (no fake tileset, no invented scalar). Every occurrence carries a
//!   requirement ID so `STATUS.md` and the HTTP 501 bodies stay traceable.
//! * [`QError::BudgetExceeded`] — raised when a read would allocate more than
//!   the caller's declared [`crate::budget::MemoryBudget`]. Trillion-scale
//!   support in this codebase means *metadata and addressing* scale under
//!   bounded memory; any allocation proportional to total checkpoint size is a
//!   bug, and this variant is how that bug surfaces at runtime.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, QError>;

#[derive(Debug, thiserror::Error)]
pub enum QError {
    /// Deliberately unbuilt. `requirement` is the STATUS.md requirement ID
    /// (e.g. `"CUDA-001"`), `detail` says what is missing and where to read
    /// about it.
    #[error("not implemented [{requirement}]: {detail}")]
    NotImplemented {
        requirement: &'static str,
        detail: String,
    },

    /// A read would have allocated more than the declared budget.
    #[error("memory budget exceeded [{budget_name}]: requested {requested} bytes, limit {limit} bytes")]
    BudgetExceeded {
        budget_name: &'static str,
        requested: u64,
        limit: u64,
    },

    /// The operation was cancelled via a [`crate::cancel::CancellationToken`].
    #[error("cancelled at checkpoint {checkpoint}")]
    Cancelled { checkpoint: String },

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("io error: {0}")]
    BareIo(#[from] std::io::Error),

    #[error("json error in {context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    /// A checkpoint file is structurally invalid.
    #[error("malformed artifact in {uri}: {detail}")]
    MalformedArtifact { uri: String, detail: String },

    /// A byte range is outside the declared extent of its tensor or file.
    #[error("byte range {start}..{end} is outside {uri} (length {length})")]
    RangeOutOfBounds {
        uri: String,
        start: u64,
        end: u64,
        length: u64,
    },

    /// A logical index is outside a tensor's shape.
    #[error("index {index:?} is out of bounds for tensor {tensor} with shape {shape:?}")]
    IndexOutOfBounds {
        tensor: String,
        index: Vec<u64>,
        shape: Vec<u64>,
    },

    #[error("unsupported dtype {dtype:?} for {operation}")]
    UnsupportedDType { dtype: String, operation: String },

    #[error("shard {shard} referenced by the index is missing from {root}")]
    MissingShard { shard: String, root: String },

    #[error("duplicate tensor name {name} (first in {first_uri}, again in {second_uri})")]
    DuplicateTensorName {
        name: String,
        first_uri: String,
        second_uri: String,
    },

    /// A path escaped the configured model roots. See `SEC-001`.
    #[error("path {requested} is outside the configured model roots")]
    PathOutsideRoot { requested: String },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("catalog error: {0}")]
    Catalog(String),

    /// A query was syntactically or semantically rejected *before* execution.
    #[error("query rejected: {0}")]
    QueryRejected(String),

    /// An alias resolved to more than one tensor. Per ARCHITECTURE.md §6.2 the
    /// system must return candidates rather than silently picking one.
    #[error("ambiguous alias {alias}: {} candidates", candidates.len())]
    AmbiguousAlias {
        alias: String,
        candidates: Vec<String>,
    },
}

impl QError {
    pub fn not_implemented(requirement: &'static str, detail: impl Into<String>) -> Self {
        QError::NotImplemented {
            requirement,
            detail: detail.into(),
        }
    }

    pub fn malformed(uri: impl Into<String>, detail: impl Into<String>) -> Self {
        QError::MalformedArtifact {
            uri: uri.into(),
            detail: detail.into(),
        }
    }

    pub fn json(context: impl Into<String>, source: serde_json::Error) -> Self {
        QError::Json {
            context: context.into(),
            source,
        }
    }

    /// The requirement ID attached to a `NotImplemented`, for HTTP 501 bodies.
    pub fn requirement_id(&self) -> Option<&'static str> {
        match self {
            QError::NotImplemented { requirement, .. } => Some(requirement),
            _ => None,
        }
    }
}
