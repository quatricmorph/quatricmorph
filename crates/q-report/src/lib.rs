//! # q-report — Report Plane
//!
//! Data plane: **Report Plane** (`.plan/REPORT_ARCHITECTURE.md`).
//!
//! The versioned JSON manifest a diagnostic run emits, and nothing else. The
//! Markdown report (`REP-002`, `QM-0141`), the daemon routes (`API-012`,
//! `QM-0143`) and the heat-map surface (`QM-0150`) all derive from the manifest
//! this crate defines; a number that appears in one of them and not in the
//! manifest is a bug.
//!
//! ## Why the manifest lands before its producers
//!
//! `REPORT_ARCHITECTURE.md` §1: the manifest is the **only** serialization the
//! other three surfaces derive from. Four consumers reading four
//! independently-invented shapes is the drift this crate exists to prevent, so
//! the schema and its serde mirror are written against the contracts in
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` rather than waiting for the engine
//! (`QM-0123`) that will fill them in.
//!
//! ## What this crate does not do
//!
//! No computation and no I/O policy (`REPORT_ARCHITECTURE.md` §5). It is handed
//! a result tree and produces bytes. There is deliberately no code here that
//! *derives* a metric: the manifest carries the composable partials
//! (`DIAGNOSTIC_ARCHITECTURE.md` §4.1) and the consumer finishes the arithmetic,
//! because a metric stored twice is a metric that will disagree with itself.
//!
//! ## The two rules that shape the API
//!
//! * **Refuse rather than reinterpret.** A future `manifest_version`, a
//!   non-finite float, a duplicate canonical address, a rank above ADR-010's
//!   ceiling, a blank revision hash — every one of these refuses, naming what
//!   it saw. `ARCHITECTURE.md` §19 and `SCHEMA_PLAN.md` §5 both say a reader
//!   that guesses produces a plausible wrong answer, which is the failure mode
//!   this repository most consistently designs against.
//! * **Canonicalize on write.** Every array has a total order fixed by content
//!   ([`manifest::Manifest::to_json_string`] imposes it), so two runs over the
//!   same data produce byte-identical bytes regardless of the order the engine
//!   happened to visit tensors in. That is what makes `V1-18` achievable.

pub mod manifest;

pub use manifest::{
    Backend, ErrorAggregate, ExpertEntry, Fidelity, Frontier, FrontierMethod, FrontierStep,
    Granularity, GranularityKind, LayerEntry, Manifest, Model, OutlierAttribution, Precision,
    Projection, QuantConfigRecord, RankingEntry, Refusal, ResolverConfidence, RoundMode, Run,
    TensorEntry, ZeroPoint, FRONTIER_CLAIM, MANIFEST_SCHEMA_ID, MANIFEST_SCHEMA_PATH,
    MANIFEST_VERSION, MAX_IMPLEMENTED_RANK,
};
