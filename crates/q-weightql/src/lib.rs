//! # q-weightql — Metadata Plane
//!
//! Data plane: **Metadata Plane**, reaching into the **Artifact Plane** only
//! when a plan is executed (ARCHITECTURE.md §2.1, §7, §14.5).
//!
//! WeightQL: the single query layer every Quatricmorph interface goes through.
//! The viewer, the CLI, the HTTP API, and (eventually) the chat assistant all
//! call this — none of them read weight bytes directly (ARCHITECTURE.md §15).
//!
//! ```text
//! text ──lexer──> tokens ──parser──> Script ──plan──> QueryPlan ──execute──> QueryOutcome
//!                                              │
//!                                    resolve references (q-nsir + q-catalog)
//!                                    check shapes        (q-expression)
//!                                    estimate read bytes
//! ```
//!
//! ## Two hard rules
//!
//! 1. **Shape mismatches are rejected before execution.** Planning is pure
//!    metadata work; it cannot reach a disk or a GPU even by accident.
//! 2. **No arbitrary code execution.** No `eval`, no user-defined functions,
//!    no shell interpolation, no raw SQL passthrough. The function set is
//!    closed and enforced by [`q_expression::Expr`] being a closed enum. See
//!    `docs/decisions/ADR-006-weightql-no-arbitrary-execution.md`.
//!
//! ## Scope in this pass
//!
//! Scalar and slice queries execute for real against SafeTensors byte ranges.
//! Matrix multiplication, reductions, and comparisons parse, resolve, and
//! shape-check into a validated plan, then stop — there is no compute backend
//! yet (`WQL-006`). A plan-only outcome says so; it never invents numbers.

pub mod lexer;
pub mod parser;
pub mod plan;

pub use lexer::{tokenize, Token};
pub use parser::{parse, Script, Statement};
pub use plan::{
    QueryEngine, QueryOutcome, QueryPlan, ReferenceKind, ResolvedReference,
};
