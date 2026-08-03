//! Data plane: **Metadata Plane** → **Artifact Plane** (ARCHITECTURE.md §7, §14.5).
//!
//! Reference resolution, shape checking, planning, and — for the paths that
//! need no compute backend — execution.
//!
//! ## The planner's order of operations (ARCHITECTURE.md §7.4)
//!
//! 1. resolve tensor references (alias or canonical address);
//! 2. check shapes;
//! 3. honour *explicitly declared* transposes and casts — never insert one;
//! 4. determine the computation tier;
//! 5. choose exact / sampled / block-level execution;
//! 6. build the visualization graph;
//! 7. execute **when the user requests it**.
//!
//! Steps 1–5 run here and are pure metadata work: no weight byte is read while
//! planning, so "an incompatible expression must fail before GPU execution" is
//! a structural guarantee, not a code-review convention.
//!
//! ## What executes in this pass
//!
//! | expression                | status                                       |
//! |---------------------------|----------------------------------------------|
//! | `tensor("Q[10][100,42]")` | **executes** — one byte-range read           |
//! | `tensor("Q[10][0:4,0:4]")`| **executes** — one range read per row        |
//! | `A @ B`, `mean(A)`, …     | **plans only** — no compute backend (`WQL-006`)|
//!
//! A plan-only result says so explicitly and carries the requirement ID. It
//! never returns invented numbers.

use crate::parser::{Script, Statement};
use q_catalog::{Catalog, TensorRow};
use q_expression::{infer_shape, Expr, Shape, ShapeEnvironment};
use q_nsir::{CanonicalAddress, ElementSelector, IndexTerm, ParsedAlias, Registry};
use q_safetensors::{read_scalar, read_slice_2d, ScalarRead, SliceRead};
use q_source::error::{QError, Result};
use q_source::manifest::ModelSource;
use q_source::role::TensorRole;
use q_source::{AccessScale, ResultFidelity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How a textual reference was understood.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceKind {
    /// A canonical address or a raw tensor name.
    Canonical,
    /// A contextual alias such as `Q[10]`.
    Alias,
    /// A name bound earlier in the script.
    Binding,
}

/// A reference resolved against the catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedReference {
    pub text: String,
    pub kind: ReferenceKind,
    pub tensor_id: String,
    pub canonical_name: String,
    pub raw_name: String,
    pub shape: Vec<u64>,
    pub dtype: String,
    pub shard_uri: String,
    pub role: String,
    /// Element selector carried by an alias like `Q[10][100,42]`.
    pub selector: Option<ElementSelector>,
    /// 1.0 unique, 1/n ambiguous. Ambiguous references never reach a plan —
    /// they are rejected — but the value is reported for diagnostics.
    pub confidence: f32,
}

/// A validated query plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryPlan {
    pub plan_id: String,
    pub model_id: String,
    /// The expression, normalized back to text.
    pub expression: String,
    pub references: Vec<ResolvedReference>,
    pub output_shape: Vec<u64>,
    /// For a single-reference pure read, the region to fetch.
    ///
    /// Captured while the expression tree is in hand rather than recovered by
    /// re-parsing the rendered expression — the rendered form is for humans and
    /// must never be load-bearing.
    pub element_selector: Option<ElementSelector>,
    pub access_scale: AccessScale,
    pub fidelity: ResultFidelity,
    /// Bytes this plan would read if executed. Metadata arithmetic only.
    pub estimated_read_bytes: u64,
    pub matmul_count: usize,
    /// `false` when no backend exists for this plan in this build.
    pub executable_now: bool,
    /// Requirement ID explaining why, when `executable_now` is false.
    pub blocked_by: Option<String>,
    pub blocked_reason: Option<String>,
}

impl QueryPlan {
    /// Deterministic ID: same query against the same model yields the same
    /// plan ID, which makes plans quotable and cacheable.
    fn derive_id(model_id: &str, expression: &str) -> String {
        let mut h = blake3::Hasher::new();
        h.update(b"quatricmorph/plan/v1");
        h.update(model_id.as_bytes());
        h.update(b"\0");
        h.update(expression.as_bytes());
        format!("plan:b3:{}", &h.finalize().to_hex()[..32])
    }
}

/// The outcome of running a script.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryOutcome {
    /// One exactly-read scalar.
    Scalar { plan: QueryPlan, read: ScalarRead },
    /// One exactly-read window.
    Slice { plan: QueryPlan, read: SliceRead },
    /// A validated plan with no backend to run it.
    Planned(QueryPlan),
}

impl QueryOutcome {
    pub fn plan(&self) -> &QueryPlan {
        match self {
            QueryOutcome::Scalar { plan, .. }
            | QueryOutcome::Slice { plan, .. }
            | QueryOutcome::Planned(plan) => plan,
        }
    }

    pub fn fidelity(&self) -> ResultFidelity {
        self.plan().fidelity
    }
}

/// Plans and (where possible) executes WeightQL against one model.
pub struct QueryEngine<'a> {
    catalog: &'a Catalog,
    model_id: String,
    /// Absent when the engine is used for planning only — which is exactly how
    /// "shape checking never touches bytes" is enforced at the type level.
    source: Option<&'a dyn ModelSource>,
    alias_map: BTreeMap<String, Vec<String>>,
    resolver_id: String,
}

impl<'a> QueryEngine<'a> {
    /// A planning-only engine. Cannot read payload even if asked.
    pub fn planning_only(catalog: &'a Catalog, model_id: &str) -> Result<Self> {
        Self::build(catalog, model_id, None)
    }

    /// A full engine that can execute scalar and slice reads.
    pub fn with_source(
        catalog: &'a Catalog,
        model_id: &str,
        source: &'a dyn ModelSource,
    ) -> Result<Self> {
        Self::build(catalog, model_id, Some(source))
    }

    fn build(
        catalog: &'a Catalog,
        model_id: &str,
        source: Option<&'a dyn ModelSource>,
    ) -> Result<Self> {
        let model = catalog
            .get_model(model_id)?
            .ok_or_else(|| QError::NotFound(format!("model `{model_id}`")))?;
        let registry = Registry::builtin()?;
        let alias_map = registry
            .get(&model.resolver_id)
            .map(|p| p.alias_map())
            .unwrap_or_default();
        Ok(Self {
            catalog,
            model_id: model_id.to_string(),
            source,
            alias_map,
            resolver_id: model.resolver_id,
        })
    }

    pub fn resolver_id(&self) -> &str {
        &self.resolver_id
    }

    // --- reference resolution ----------------------------------------------

    /// Resolve one textual reference.
    ///
    /// Canonical addresses and raw names are tried first because they are
    /// unambiguous; aliases second. An alias matching several tensors is
    /// rejected with the candidate list — never silently narrowed.
    pub fn resolve_reference(&self, text: &str) -> Result<ResolvedReference> {
        // 1. Canonical address / raw name.
        if let Ok(addr) = CanonicalAddress::parse(text) {
            let path = addr.tensor_path();
            if let Some(row) = self.catalog.get_by_canonical_name(&self.model_id, &path)? {
                return Ok(self.reference_from_row(
                    text,
                    ReferenceKind::Canonical,
                    row,
                    addr.selector,
                    1.0,
                ));
            }
        }

        // 2. Contextual alias.
        let parsed = ParsedAlias::parse(text)?;
        let roles = self.alias_map.get(&parsed.alias).ok_or_else(|| {
            QError::QueryRejected(format!(
                "`{text}` is neither a known canonical address in this model nor an alias \
                 declared by the `{}` resolver (which declares {} aliases)",
                self.resolver_id,
                self.alias_map.len()
            ))
        })?;

        let mut candidates: Vec<TensorRow> = Vec::new();
        for role in roles {
            let mut rows = self.catalog.find_by_role(
                &self.model_id,
                TensorRole::parse(role),
                parsed.layer_index,
            )?;
            if let Some(expert) = parsed.expert_index {
                rows.retain(|r| r.expert_index == Some(expert));
            }
            candidates.extend(rows);
        }

        match candidates.len() {
            0 => Err(QError::NotFound(format!(
                "alias `{text}` matched no tensor in this model"
            ))),
            1 => Ok(self.reference_from_row(
                text,
                ReferenceKind::Alias,
                candidates.remove(0),
                parsed.selector,
                1.0,
            )),
            n => Err(QError::AmbiguousAlias {
                alias: text.to_string(),
                candidates: candidates.into_iter().map(|c| c.canonical_name).collect(),
            })
            .map_err(|e| {
                // Preserve the confidence signal in the message for the UI.
                match e {
                    QError::AmbiguousAlias { alias, candidates } => QError::AmbiguousAlias {
                        alias: format!("{alias} (confidence {:.2})", 1.0 / n as f32),
                        candidates,
                    },
                    other => other,
                }
            }),
        }
    }

    fn reference_from_row(
        &self,
        text: &str,
        kind: ReferenceKind,
        row: TensorRow,
        selector: Option<ElementSelector>,
        confidence: f32,
    ) -> ResolvedReference {
        ResolvedReference {
            text: text.to_string(),
            kind,
            tensor_id: row.tensor_id,
            canonical_name: row.canonical_name,
            raw_name: row.raw_name,
            shape: row.shape,
            dtype: row.dtype,
            shard_uri: row.shard_uri,
            role: row.role,
            selector,
            confidence,
        }
    }

    /// List the candidates for an alias without failing on ambiguity.
    ///
    /// This is what a UI or the chat layer calls to *present* the choice.
    pub fn alias_candidates(&self, text: &str) -> Result<Vec<ResolvedReference>> {
        let parsed = ParsedAlias::parse(text)?;
        let roles = match self.alias_map.get(&parsed.alias) {
            Some(r) => r,
            None => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        for role in roles {
            let mut rows = self.catalog.find_by_role(
                &self.model_id,
                TensorRole::parse(role),
                parsed.layer_index,
            )?;
            if let Some(expert) = parsed.expert_index {
                rows.retain(|r| r.expert_index == Some(expert));
            }
            for row in rows {
                out.push(self.reference_from_row(
                    text,
                    ReferenceKind::Alias,
                    row,
                    parsed.selector.clone(),
                    0.0,
                ));
            }
        }
        let n = out.len().max(1) as f32;
        for r in &mut out {
            r.confidence = 1.0 / n;
        }
        Ok(out)
    }

    // --- planning -----------------------------------------------------------

    /// Plan a script: resolve, shape-check, estimate. Reads no payload.
    pub fn plan(&self, script: &Script) -> Result<QueryPlan> {
        let statement = script.output_statement().ok_or_else(|| {
            QError::QueryRejected(
                "script has no output statement; add `show <expr>` or a `SELECT`".into(),
            )
        })?;

        // Bindings are substituted structurally, so `A` in `show A @ B` resolves
        // to whatever `A = …` said. No name is ever evaluated as code.
        let mut bindings: BTreeMap<String, Expr> = BTreeMap::new();
        for s in &script.statements {
            if let Statement::Assign { name, expr } = s {
                let expanded = substitute(expr, &bindings)?;
                bindings.insert(name.clone(), expanded);
            }
        }

        let expr = match statement {
            Statement::Show(e) => substitute(e, &bindings)?,
            Statement::SelectValue { reference, index } => Expr::Slice {
                operand: Box::new(Expr::tensor(reference.clone())),
                selector: ElementSelector(index.iter().map(|i| IndexTerm::Point(*i)).collect()),
            },
            Statement::SelectSlice {
                reference,
                rows,
                columns,
            } => {
                let mut terms = Vec::new();
                terms.push(range_term(*rows));
                terms.push(range_term(*columns));
                Expr::Slice {
                    operand: Box::new(Expr::tensor(reference.clone())),
                    selector: ElementSelector(terms),
                }
            }
            Statement::Assign { .. } => unreachable!("output_statement excludes assignments"),
        };

        let env = CatalogShapeEnv {
            engine: self,
            cache: std::cell::RefCell::new(BTreeMap::new()),
        };
        let output_shape = infer_shape(&expr, &env)?;

        let mut references = Vec::new();
        for text in expr.references() {
            references.push(self.resolve_reference(text)?);
        }

        let expression = expr.to_string();
        let estimated_read_bytes = self.estimate_read_bytes(&expr, &env)?;
        let matmul_count = expr.matmul_count();
        let pure_read = expr.is_pure_read();

        let (access_scale, fidelity) = if pure_read {
            (AccessScale::SelectedBlockExact, ResultFidelity::Exact)
        } else {
            (AccessScale::SelectedBlockCompute, ResultFidelity::Exact)
        };

        let element_selector = if pure_read {
            Some(pure_read_selector(&expr, &references)?)
        } else {
            None
        };

        let (executable_now, blocked_by, blocked_reason) = if pure_read {
            (true, None, None)
        } else {
            (
                false,
                Some("WQL-006".to_string()),
                Some(format!(
                    "this plan needs a compute backend ({matmul_count} matmul(s), \
                     reduction/comparison nodes); no GPU or CPU expression backend is wired in \
                     this pass. The plan is validated — shapes check out and the output would be \
                     {output_shape} — but nothing was computed. See ARCHITECTURE.md §7.4."
                )),
            )
        };

        Ok(QueryPlan {
            plan_id: QueryPlan::derive_id(&self.model_id, &expression),
            model_id: self.model_id.clone(),
            expression,
            references,
            output_shape: output_shape.0,
            element_selector,
            access_scale,
            fidelity,
            estimated_read_bytes,
            matmul_count,
            executable_now,
            blocked_by,
            blocked_reason,
        })
    }

    /// Bytes a plan would read: the selected region of each leaf, not the
    /// tensors themselves.
    fn estimate_read_bytes(&self, expr: &Expr, env: &CatalogShapeEnv<'_, '_>) -> Result<u64> {
        Ok(match expr {
            Expr::TensorRef { text, .. } => {
                let s = env.shape_of(text)?;
                let r = self.resolve_reference(text)?;
                let width = q_source::DType::parse_safetensors(&r.dtype)?.size_in_bytes();
                match &r.selector {
                    Some(sel) => sel.element_count(s.dims()) * width,
                    None => s.element_count() * width,
                }
            }
            Expr::Slice { operand, selector } => {
                let s = infer_shape(operand, env)?;
                let base = self.estimate_read_bytes(operand, env)?;
                let full = s.element_count().max(1);
                let selected = selector.element_count(s.dims());
                base.saturating_mul(selected) / full
            }
            Expr::Transpose(a) => self.estimate_read_bytes(a, env)?,
            Expr::Reduce { operand, .. } => self.estimate_read_bytes(operand, env)?,
            Expr::MatMul(a, b) | Expr::Add(a, b) | Expr::Sub(a, b) => {
                self.estimate_read_bytes(a, env)? + self.estimate_read_bytes(b, env)?
            }
            Expr::Compare { left, right, .. } => {
                self.estimate_read_bytes(left, env)? + self.estimate_read_bytes(right, env)?
            }
        })
    }

    // --- execution ----------------------------------------------------------

    /// Plan, then execute if a backend exists for the plan.
    pub fn run(&self, src: &str) -> Result<QueryOutcome> {
        let script = crate::parser::parse(src)?;
        let plan = self.plan(&script)?;
        self.execute(plan)
    }

    /// Execute a validated plan.
    ///
    /// Only pure reads execute in this pass. Everything else returns
    /// [`QueryOutcome::Planned`] with the blocking requirement recorded —
    /// never a fabricated value.
    pub fn execute(&self, plan: QueryPlan) -> Result<QueryOutcome> {
        if !plan.executable_now {
            return Ok(QueryOutcome::Planned(plan));
        }
        let source = match self.source {
            Some(s) => s,
            None => {
                let mut p = plan;
                p.executable_now = false;
                p.blocked_by = Some("WQL-005".into());
                p.blocked_reason = Some(
                    "this engine was built for planning only and has no ModelSource, \
                     so no bytes can be read. Use QueryEngine::with_source to execute."
                        .into(),
                );
                return Ok(QueryOutcome::Planned(p));
            }
        };

        let reference = plan.references.first().ok_or_else(|| {
            QError::QueryRejected("executable plan has no tensor reference".into())
        })?;
        let row = self
            .catalog
            .get_tensor(&reference.tensor_id)?
            .ok_or_else(|| QError::NotFound(format!("tensor `{}`", reference.tensor_id)))?;
        let descriptor = row.to_descriptor()?;

        match plan.element_selector.clone() {
            Some(sel) if sel.is_scalar_for(&descriptor.shape) => {
                let index = sel.as_point_index(&descriptor.shape).expect("checked above");
                let read = read_scalar(source, &descriptor, &index)?;
                Ok(QueryOutcome::Scalar { plan, read })
            }
            Some(sel) => {
                let (rows, cols) = sel.resolve_2d(&descriptor.shape)?;
                let read = read_slice_2d(source, &descriptor, rows, cols)?;
                Ok(QueryOutcome::Slice { plan, read })
            }
            // Unreachable in practice: `plan` refuses a selector-less pure read
            // before marking it executable. Kept as an error rather than an
            // `unwrap` so a future planner change surfaces as a message, not a
            // panic inside the daemon.
            None => Err(QError::QueryRejected(format!(
                "`{}` has no element selector; whole-tensor reads are refused",
                reference.text
            ))),
        }
    }
}

/// The region a pure-read expression selects.
///
/// A pure read is a tensor reference, optionally wrapped in one `Slice` node.
/// The selector can arrive from two places — the alias itself
/// (`tensor("Q[10][100,42]")`) or a postfix/`SELECT` slice
/// (`tensor("Q[10]")[100,42]`) — but never both, and never stacked. Composing
/// nested slices is well-defined but unimplemented, so it is refused with a
/// requirement ID instead of being approximated.
fn pure_read_selector(
    expr: &Expr,
    references: &[ResolvedReference],
) -> Result<ElementSelector> {
    let from_reference = references.first().and_then(|r| r.selector.clone());

    let mut from_expression: Option<ElementSelector> = None;
    let mut node = expr;
    loop {
        match node {
            Expr::Slice { operand, selector } => {
                if from_expression.is_some() {
                    return Err(QError::not_implemented(
                        "WQL-008",
                        "stacked slices (`A[0:64][0:8]`) are not composed in this pass; \
                         write a single selector instead",
                    ));
                }
                from_expression = Some(selector.clone());
                node = operand;
            }
            Expr::TensorRef { .. } => break,
            _ => {
                return Err(QError::QueryRejected(
                    "internal: pure-read plan contained a compute node".into(),
                ))
            }
        }
    }

    match (from_reference, from_expression) {
        (Some(_), Some(_)) => Err(QError::QueryRejected(
            "the reference already selects a region (e.g. `Q[10][0:4,0:4]`); \
             drop either the inline selector or the trailing one"
                .into(),
        )),
        (Some(s), None) | (None, Some(s)) => Ok(s),
        (None, None) => Err(QError::QueryRejected(format!(
            "`{}` names a whole tensor; whole-tensor reads are refused. Select a region, \
             e.g. `[0:256, 0:256]`. (Reading an entire tensor at checkpoint scale is exactly \
             what this system exists to avoid.)",
            references
                .first()
                .map(|r| r.text.as_str())
                .unwrap_or("<expression>")
        ))),
    }
}

fn range_term(r: Option<(Option<u64>, Option<u64>)>) -> IndexTerm {
    match r {
        None => IndexTerm::All,
        Some((None, None)) => IndexTerm::All,
        Some((start, end)) => IndexTerm::Range { start, end },
    }
}

/// Substitute bound names into an expression, structurally.
///
/// Rejects self-reference rather than looping.
fn substitute(expr: &Expr, bindings: &BTreeMap<String, Expr>) -> Result<Expr> {
    Ok(match expr {
        Expr::TensorRef { text, binding } => {
            if *binding {
                match bindings.get(text) {
                    Some(bound) => bound.clone(),
                    None => Expr::TensorRef {
                        text: text.clone(),
                        binding: true,
                    },
                }
            } else {
                expr.clone()
            }
        }
        Expr::Slice { operand, selector } => Expr::Slice {
            operand: Box::new(substitute(operand, bindings)?),
            selector: selector.clone(),
        },
        Expr::Transpose(a) => Expr::Transpose(Box::new(substitute(a, bindings)?)),
        Expr::MatMul(a, b) => Expr::MatMul(
            Box::new(substitute(a, bindings)?),
            Box::new(substitute(b, bindings)?),
        ),
        Expr::Add(a, b) => Expr::Add(
            Box::new(substitute(a, bindings)?),
            Box::new(substitute(b, bindings)?),
        ),
        Expr::Sub(a, b) => Expr::Sub(
            Box::new(substitute(a, bindings)?),
            Box::new(substitute(b, bindings)?),
        ),
        Expr::Reduce { reduction, operand } => Expr::Reduce {
            reduction: *reduction,
            operand: Box::new(substitute(operand, bindings)?),
        },
        Expr::Compare {
            left,
            right,
            metric,
        } => Expr::Compare {
            left: Box::new(substitute(left, bindings)?),
            right: Box::new(substitute(right, bindings)?),
            metric: *metric,
        },
    })
}

/// Shape lookup backed by the catalog. Reads metadata only.
struct CatalogShapeEnv<'a, 'e> {
    engine: &'a QueryEngine<'e>,
    cache: std::cell::RefCell<BTreeMap<String, Shape>>,
}

impl ShapeEnvironment for CatalogShapeEnv<'_, '_> {
    fn shape_of(&self, reference: &str) -> Result<Shape> {
        if let Some(s) = self.cache.borrow().get(reference) {
            return Ok(s.clone());
        }
        let r = self.engine.resolve_reference(reference)?;
        // A reference carrying its own selector (`Q[10][0:256,0:256]`) already
        // denotes the selected region, so that is its shape in the algebra.
        let shape = match &r.selector {
            Some(sel) => {
                let mut dims = Vec::new();
                for (axis, dim) in r.shape.iter().enumerate() {
                    let term = sel.0.get(axis).copied().unwrap_or(IndexTerm::All);
                    let (start, end) = term.bounds(*dim);
                    if end > *dim || end <= start {
                        return Err(QError::QueryRejected(format!(
                            "`{reference}` selects {start}:{end} on axis {axis} of shape {:?}",
                            r.shape
                        )));
                    }
                    if !term.is_point() {
                        dims.push(end - start);
                    }
                }
                Shape(dims)
            }
            None => Shape(r.shape.clone()),
        };
        self.cache
            .borrow_mut()
            .insert(reference.to_string(), shape.clone());
        Ok(shape)
    }
}
