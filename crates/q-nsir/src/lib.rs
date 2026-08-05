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
//! Implemented: the generic transformer resolver, the Llama-family resolver, the
//! Qwen-family resolver (`NSIR-006`, Qwen2/Qwen3, dense and MoE), canonical
//! address construction and parsing, the alias grammar, and ambiguity handling.
//! Kimi and DeepSeek exist as declared-but-unimplemented plugin manifests and
//! never claim a model. Adding one is a manifest edit, not a rewrite — Qwen
//! needed no Rust change at all, which is the evidence for that claim.
//!
//! A resolved address is **exact**: it is a deterministic function of the raw
//! name and a declared rule table, with nothing sampled and nothing estimated.
//! What it is *not* is a statement about meaning. Resolution establishes where a
//! tensor lives and what its author called it; it establishes nothing about what
//! the tensor has learned or computes.

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
