//! # q-expression — Metadata Plane
//!
//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §7.4).
//!
//! The mathematical-expression AST and its shape algebra.
//!
//! ARCHITECTURE.md §7.4 requires `(A @ B) @ C` to become a tree and to be
//! **type-checked before execution**:
//!
//! ```text
//! MatMul
//! ├── MatMul
//! │   ├── TensorRef(A)
//! │   └── TensorRef(B)
//! └── TensorRef(C)
//! ```
//!
//! This crate is deliberately pure: it holds the tree and the shape rules, and
//! it does not read bytes, touch a catalog, or run anything. Shape checking
//! therefore *cannot* accidentally trigger I/O, which is what makes "reject
//! before execution" enforceable rather than aspirational.
//!
//! ## What this language will never contain
//!
//! No `eval`, no user-defined `Function` constructor, no shell interpolation,
//! no raw SQL passthrough, no dynamic code loading. The [`Expr`] enum is a
//! closed set; a query can only express operations enumerated here. This is a
//! hard security boundary, not a roadmap item — see
//! `docs/decisions/ADR-006-weightql-no-arbitrary-execution.md`.
//!
//! The legacy `mm` visualizer *did* have such a hole (`mm/viz.js:119-126`
//! builds an initializer with `eval?.()`); it is recorded as **Deprecate** in
//! `docs/CURRENT_ARCHITECTURE.md` and is not carried into any extracted module.

use q_nsir::ElementSelector;
use q_source::error::{QError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A tensor shape in the expression algebra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shape(pub Vec<u64>);

impl Shape {
    pub fn rank(&self) -> usize {
        self.0.len()
    }

    pub fn element_count(&self) -> u64 {
        if self.0.is_empty() {
            1
        } else {
            self.0.iter().copied().product()
        }
    }

    pub fn is_scalar(&self) -> bool {
        self.0.is_empty()
    }

    pub fn dims(&self) -> &[u64] {
        &self.0
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, d) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{d}")?;
        }
        write!(f, "]")
    }
}

/// Reductions available in the MVP subset (ARCHITECTURE.md §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reduction {
    Min,
    Max,
    Mean,
    Variance,
    StdDev,
    L1Norm,
    L2Norm,
    ZeroRatio,
}

impl Reduction {
    pub fn as_str(self) -> &'static str {
        match self {
            Reduction::Min => "min",
            Reduction::Max => "max",
            Reduction::Mean => "mean",
            Reduction::Variance => "variance",
            Reduction::StdDev => "stddev",
            Reduction::L1Norm => "l1_norm",
            Reduction::L2Norm => "l2_norm",
            Reduction::ZeroRatio => "zero_ratio",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "min" => Reduction::Min,
            "max" => Reduction::Max,
            "mean" => Reduction::Mean,
            "variance" | "var" => Reduction::Variance,
            "stddev" | "std" => Reduction::StdDev,
            "l1_norm" => Reduction::L1Norm,
            "l2_norm" => Reduction::L2Norm,
            "zero_ratio" => Reduction::ZeroRatio,
            _ => return None,
        })
    }
}

/// Comparison metrics (ARCHITECTURE.md §15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonMetric {
    CosineSimilarity,
    RelativeL2,
}

impl ComparisonMetric {
    pub fn as_str(self) -> &'static str {
        match self {
            ComparisonMetric::CosineSimilarity => "cosine_similarity",
            ComparisonMetric::RelativeL2 => "relative_l2",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "cosine_similarity" => ComparisonMetric::CosineSimilarity,
            "relative_l2" => ComparisonMetric::RelativeL2,
            _ => return None,
        })
    }
}

/// The expression tree. **A closed set** — see the module docs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// `tensor("Q[10]")` or a reference to a previously bound name.
    ///
    /// `text` is the address or alias exactly as written; resolution happens in
    /// `q-weightql`, not here.
    TensorRef { text: String, binding: bool },
    /// A region of another expression, e.g. `A[0:256, 0:256]`.
    Slice {
        operand: Box<Expr>,
        selector: ElementSelector,
    },
    /// `transpose(x)` — explicit only. ARCHITECTURE.md §7.4 step 3 says
    /// transposes and casts are *explicitly declared*, never inserted silently
    /// to make shapes line up.
    Transpose(Box<Expr>),
    /// `a @ b`
    MatMul(Box<Expr>, Box<Expr>),
    /// `a + b`
    Add(Box<Expr>, Box<Expr>),
    /// `a - b`
    Sub(Box<Expr>, Box<Expr>),
    /// `mean(x)`, `l2_norm(x)`, …
    Reduce {
        reduction: Reduction,
        operand: Box<Expr>,
    },
    /// `compare(a, b) by cosine_similarity`
    Compare {
        left: Box<Expr>,
        right: Box<Expr>,
        metric: ComparisonMetric,
    },
}

impl Expr {
    pub fn tensor(text: impl Into<String>) -> Self {
        Expr::TensorRef {
            text: text.into(),
            binding: false,
        }
    }

    pub fn binding(name: impl Into<String>) -> Self {
        Expr::TensorRef {
            text: name.into(),
            binding: true,
        }
    }

    pub fn matmul(a: Expr, b: Expr) -> Self {
        Expr::MatMul(Box::new(a), Box::new(b))
    }

    pub fn transpose(a: Expr) -> Self {
        Expr::Transpose(Box::new(a))
    }

    /// Every tensor reference in the tree, in evaluation order.
    pub fn references(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.walk_refs(&mut out);
        out
    }

    fn walk_refs<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Expr::TensorRef { text, .. } => out.push(text),
            Expr::Slice { operand, .. } | Expr::Transpose(operand) => operand.walk_refs(out),
            Expr::Reduce { operand, .. } => operand.walk_refs(out),
            Expr::MatMul(a, b) | Expr::Add(a, b) | Expr::Sub(a, b) => {
                a.walk_refs(out);
                b.walk_refs(out);
            }
            Expr::Compare { left, right, .. } => {
                left.walk_refs(out);
                right.walk_refs(out);
            }
        }
    }

    /// Count of matrix multiplications — the cost driver of a plan.
    pub fn matmul_count(&self) -> usize {
        match self {
            Expr::MatMul(a, b) => 1 + a.matmul_count() + b.matmul_count(),
            Expr::Slice { operand, .. } | Expr::Transpose(operand) => operand.matmul_count(),
            Expr::Reduce { operand, .. } => operand.matmul_count(),
            Expr::Add(a, b) | Expr::Sub(a, b) => a.matmul_count() + b.matmul_count(),
            Expr::Compare { left, right, .. } => left.matmul_count() + right.matmul_count(),
            Expr::TensorRef { .. } => 0,
        }
    }

    /// Whether the expression needs a compute backend, or is a pure read.
    ///
    /// A pure read (a tensor reference, possibly sliced) can be answered today
    /// by a byte-range read. Anything else needs a backend that does not exist
    /// yet in this pass.
    pub fn is_pure_read(&self) -> bool {
        match self {
            Expr::TensorRef { .. } => true,
            Expr::Slice { operand, .. } => operand.is_pure_read(),
            _ => false,
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::TensorRef { text, binding } => {
                if *binding {
                    f.write_str(text)
                } else {
                    write!(f, "tensor(\"{text}\")")
                }
            }
            Expr::Slice { operand, selector } => write!(f, "{operand}{selector}"),
            Expr::Transpose(a) => write!(f, "transpose({a})"),
            Expr::MatMul(a, b) => write!(f, "({a} @ {b})"),
            Expr::Add(a, b) => write!(f, "({a} + {b})"),
            Expr::Sub(a, b) => write!(f, "({a} - {b})"),
            Expr::Reduce { reduction, operand } => write!(f, "{}({operand})", reduction.as_str()),
            Expr::Compare {
                left,
                right,
                metric,
            } => write!(f, "compare({left}, {right}) by {}", metric.as_str()),
        }
    }
}

/// Supplies the shape of a resolved tensor reference.
///
/// Implemented by `q-weightql` over the catalog. Kept as a trait so this crate
/// stays free of storage dependencies.
pub trait ShapeEnvironment {
    fn shape_of(&self, reference: &str) -> Result<Shape>;
}

/// Infer the shape of an expression, rejecting mismatches.
///
/// This is ARCHITECTURE.md §7.4's type checker: *"An incompatible expression
/// must fail before GPU execution."* Because this function is pure, it
/// literally cannot reach a GPU or a disk, so the ordering guarantee holds by
/// construction.
pub fn infer_shape(expr: &Expr, env: &dyn ShapeEnvironment) -> Result<Shape> {
    match expr {
        Expr::TensorRef { text, .. } => env.shape_of(text),

        Expr::Slice { operand, selector } => {
            let s = infer_shape(operand, env)?;
            if selector.rank() > s.rank() {
                return Err(QError::QueryRejected(format!(
                    "selector {selector} has {} terms but the operand has rank {}",
                    selector.rank(),
                    s.rank()
                )));
            }
            let mut dims = Vec::with_capacity(s.rank());
            for (axis, dim) in s.dims().iter().enumerate() {
                let term = selector
                    .0
                    .get(axis)
                    .copied()
                    .unwrap_or(q_nsir::IndexTerm::All);
                let (start, end) = term.bounds(*dim);
                if end > *dim || end <= start {
                    return Err(QError::QueryRejected(format!(
                        "selector {selector} is out of bounds on axis {axis} of shape {s}"
                    )));
                }
                // A point term drops the axis; a range keeps it.
                if !term.is_point() {
                    dims.push(end - start);
                }
            }
            Ok(Shape(dims))
        }

        Expr::Transpose(a) => {
            let s = infer_shape(a, env)?;
            if s.rank() != 2 {
                return Err(QError::QueryRejected(format!(
                    "transpose requires rank 2, got {s} (rank {})",
                    s.rank()
                )));
            }
            Ok(Shape(vec![s.0[1], s.0[0]]))
        }

        Expr::MatMul(a, b) => {
            let (sa, sb) = (infer_shape(a, env)?, infer_shape(b, env)?);
            if sa.rank() != 2 || sb.rank() != 2 {
                return Err(QError::QueryRejected(format!(
                    "matmul requires rank-2 operands, got {sa} @ {sb}"
                )));
            }
            if sa.0[1] != sb.0[0] {
                return Err(QError::QueryRejected(format!(
                    "shape mismatch: {sa} @ {sb} — inner dimensions {} and {} differ. \
                     Insert an explicit transpose() if that is what you meant.",
                    sa.0[1], sb.0[0]
                )));
            }
            Ok(Shape(vec![sa.0[0], sb.0[1]]))
        }

        Expr::Add(a, b) | Expr::Sub(a, b) => {
            let (sa, sb) = (infer_shape(a, env)?, infer_shape(b, env)?);
            if sa != sb {
                return Err(QError::QueryRejected(format!(
                    "shape mismatch: {sa} and {sb} must be identical for elementwise operations \
                     (broadcasting is not part of the MVP subset)"
                )));
            }
            Ok(sa)
        }

        Expr::Reduce { operand, .. } => {
            infer_shape(operand, env)?;
            Ok(Shape(vec![]))
        }

        Expr::Compare {
            left,
            right,
            metric,
        } => {
            let (sl, sr) = (infer_shape(left, env)?, infer_shape(right, env)?);
            if sl != sr {
                return Err(QError::QueryRejected(format!(
                    "cannot compare {sl} with {sr} by {}: shapes must match",
                    metric.as_str()
                )));
            }
            Ok(Shape(vec![]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_nsir::IndexTerm;
    use std::collections::HashMap;

    struct Env(HashMap<String, Shape>);

    impl Env {
        fn new(pairs: &[(&str, &[u64])]) -> Self {
            Env(pairs
                .iter()
                .map(|(k, v)| (k.to_string(), Shape(v.to_vec())))
                .collect())
        }
    }

    impl ShapeEnvironment for Env {
        fn shape_of(&self, reference: &str) -> Result<Shape> {
            self.0
                .get(reference)
                .cloned()
                .ok_or_else(|| QError::NotFound(format!("tensor `{reference}`")))
        }
    }

    #[test]
    fn matmul_shape_follows_the_architecture_md_worked_example() {
        // A: [128, 4096], B: [4096, 128] -> [128, 128]
        let env = Env::new(&[("A", &[128, 4096]), ("B", &[4096, 128]), ("C", &[128, 4096])]);
        let ab = Expr::matmul(Expr::binding("A"), Expr::binding("B"));
        assert_eq!(infer_shape(&ab, &env).unwrap(), Shape(vec![128, 128]));

        // (A @ B) @ C: [128, 4096]
        let abc = Expr::matmul(ab, Expr::binding("C"));
        assert_eq!(infer_shape(&abc, &env).unwrap(), Shape(vec![128, 4096]));
        assert_eq!(abc.matmul_count(), 2);
    }

    #[test]
    fn shape_mismatch_is_rejected_with_an_actionable_message() {
        let env = Env::new(&[("A", &[2, 3]), ("B", &[2, 2])]);
        let e = Expr::matmul(Expr::binding("A"), Expr::binding("B"));
        let err = infer_shape(&e, &env).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("shape mismatch"), "{msg}");
        assert!(msg.contains("transpose()"), "{msg}");
    }

    #[test]
    fn transpose_must_be_explicit_and_fixes_the_mismatch() {
        let env = Env::new(&[("Q", &[256, 64]), ("K", &[256, 64])]);
        // Q @ K is invalid...
        assert!(infer_shape(&Expr::matmul(Expr::binding("Q"), Expr::binding("K")), &env).is_err());
        // ...but Q @ transpose(K) is [256, 256].
        let ok = Expr::matmul(
            Expr::binding("Q"),
            Expr::transpose(Expr::binding("K")),
        );
        assert_eq!(infer_shape(&ok, &env).unwrap(), Shape(vec![256, 256]));
    }

    #[test]
    fn transpose_rejects_non_matrix_operands() {
        let env = Env::new(&[("v", &[48])]);
        assert!(infer_shape(&Expr::transpose(Expr::binding("v")), &env).is_err());
    }

    #[test]
    fn slicing_drops_point_axes_and_keeps_ranges() {
        let env = Env::new(&[("Q", &[128, 48])]);
        let window = Expr::Slice {
            operand: Box::new(Expr::binding("Q")),
            selector: ElementSelector(vec![
                IndexTerm::Range { start: Some(0), end: Some(32) },
                IndexTerm::Range { start: Some(0), end: Some(16) },
            ]),
        };
        assert_eq!(infer_shape(&window, &env).unwrap(), Shape(vec![32, 16]));

        // A single point on axis 0 leaves a vector.
        let row = Expr::Slice {
            operand: Box::new(Expr::binding("Q")),
            selector: ElementSelector(vec![IndexTerm::Point(100)]),
        };
        assert_eq!(infer_shape(&row, &env).unwrap(), Shape(vec![48]));

        // Points on both axes leave a scalar.
        let scalar = Expr::Slice {
            operand: Box::new(Expr::binding("Q")),
            selector: ElementSelector(vec![IndexTerm::Point(100), IndexTerm::Point(42)]),
        };
        assert!(infer_shape(&scalar, &env).unwrap().is_scalar());
    }

    #[test]
    fn out_of_bounds_slice_is_rejected() {
        let env = Env::new(&[("Q", &[128, 48])]);
        let bad = Expr::Slice {
            operand: Box::new(Expr::binding("Q")),
            selector: ElementSelector(vec![IndexTerm::Range { start: Some(0), end: Some(999) }]),
        };
        assert!(infer_shape(&bad, &env).is_err());
    }

    #[test]
    fn elementwise_ops_require_identical_shapes() {
        let env = Env::new(&[("A", &[4, 4]), ("B", &[4, 4]), ("C", &[4, 5])]);
        assert_eq!(
            infer_shape(
                &Expr::Add(Box::new(Expr::binding("A")), Box::new(Expr::binding("B"))),
                &env
            )
            .unwrap(),
            Shape(vec![4, 4])
        );
        let err = infer_shape(
            &Expr::Add(Box::new(Expr::binding("A")), Box::new(Expr::binding("C"))),
            &env,
        )
        .unwrap_err();
        assert!(err.to_string().contains("broadcasting is not part"));
    }

    #[test]
    fn reductions_and_comparisons_produce_scalars() {
        let env = Env::new(&[("A", &[4, 4]), ("B", &[4, 4])]);
        let r = Expr::Reduce {
            reduction: Reduction::L2Norm,
            operand: Box::new(Expr::binding("A")),
        };
        assert!(infer_shape(&r, &env).unwrap().is_scalar());

        let c = Expr::Compare {
            left: Box::new(Expr::binding("A")),
            right: Box::new(Expr::binding("B")),
            metric: ComparisonMetric::CosineSimilarity,
        };
        assert!(infer_shape(&c, &env).unwrap().is_scalar());
    }

    #[test]
    fn comparison_of_different_shapes_is_rejected() {
        let env = Env::new(&[("A", &[4, 4]), ("C", &[4, 5])]);
        let c = Expr::Compare {
            left: Box::new(Expr::binding("A")),
            right: Box::new(Expr::binding("C")),
            metric: ComparisonMetric::RelativeL2,
        };
        assert!(infer_shape(&c, &env).is_err());
    }

    #[test]
    fn unknown_reference_is_reported_not_invented() {
        let env = Env::new(&[]);
        assert!(matches!(
            infer_shape(&Expr::binding("Nope"), &env),
            Err(QError::NotFound(_))
        ));
    }

    #[test]
    fn pure_reads_are_distinguished_from_compute() {
        let q = Expr::tensor("Q[10]");
        assert!(q.is_pure_read());
        let sliced = Expr::Slice {
            operand: Box::new(q.clone()),
            selector: ElementSelector(vec![IndexTerm::Point(1)]),
        };
        assert!(sliced.is_pure_read());
        assert!(!Expr::matmul(q.clone(), q).is_pure_read());
    }

    #[test]
    fn references_are_collected_in_order() {
        let e = Expr::matmul(
            Expr::matmul(Expr::binding("A"), Expr::binding("B")),
            Expr::binding("C"),
        );
        assert_eq!(e.references(), vec!["A", "B", "C"]);
    }

    #[test]
    fn display_round_trips_structure() {
        let e = Expr::matmul(
            Expr::binding("A"),
            Expr::transpose(Expr::tensor("K[10]")),
        );
        assert_eq!(e.to_string(), "(A @ transpose(tensor(\"K[10]\")))");
    }

    #[test]
    fn reduction_and_metric_names_round_trip() {
        for r in [
            Reduction::Min,
            Reduction::Max,
            Reduction::Mean,
            Reduction::Variance,
            Reduction::StdDev,
            Reduction::L1Norm,
            Reduction::L2Norm,
            Reduction::ZeroRatio,
        ] {
            assert_eq!(Reduction::parse(r.as_str()), Some(r));
        }
        assert_eq!(Reduction::parse("eval"), None);
        assert_eq!(
            ComparisonMetric::parse("cosine_similarity"),
            Some(ComparisonMetric::CosineSimilarity)
        );
        assert_eq!(ComparisonMetric::parse("system"), None);
    }
}
