//! # q-nsir — Metadata Plane
//!
//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §4.2, §6).
//!
//! The NSIR compiler: canonical semantic normalization and tensor addressing.
//!
//! ```text
//! model.layers.10.self_attn.q_proj.weight        (raw, from the header)
//!        │
//!        ├─ NsirRecord { stack, layer, component, operation, parameter, axes }
//!        │
//!        └─ model.layers[10].self_attention.query_projection.weight  (canonical)
//!                                                    + element selector [100,42]
//! ```
//!
//! Two address kinds, per ARCHITECTURE.md §6:
//!
//! * [`CanonicalAddress`] — unique and reusable across queries, APIs, reports,
//!   and annotations;
//! * [`ParsedAlias`] — what a person types (`Q[10][100,42]`, `Att[10][100]`),
//!   resolved to *candidates* so an ambiguous alias is never silently
//!   collapsed to one tensor.
//!
//! ## Scope in this pass
//!
//! Implemented: the generic transformer resolver, the Llama-family resolver,
//! canonical address construction and parsing, the alias grammar, and ambiguity
//! handling. Qwen / Kimi / DeepSeek exist as declared-but-unimplemented plugin
//! manifests (`NSIR-006`) — adding one is a manifest edit, not a rewrite.

pub mod address;
pub mod alias;
pub mod record;
pub mod resolver;

pub use address::{CanonicalAddress, ElementSelector, IndexTerm, PathSegment};
pub use alias::ParsedAlias;
pub use record::NsirRecord;
pub use resolver::{canonical_name, AliasCandidate, AliasResolution, NsirResolver, ResolvedModel};

// Re-exported so downstream crates need only depend on q-nsir to select a
// resolver.
pub use q_architecture::{ArchitecturePlugin, Registry};
