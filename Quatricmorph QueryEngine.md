# Quatricmorph QueryEngine — Mathematical Expressions and LaTeX Architecture

## 1. Purpose

The Quatricmorph QueryEngine is the unified semantic and execution layer for:

* WeightQL queries;
* tensor addressing;
* mathematical expressions;
* LaTeX input and output;
* shape and axis validation;
* exact, sampled, and approximate computation;
* backend selection;
* query cost estimation;
* matrix-operation visualization;
* reproducible query results.

The QueryEngine must make the following interaction possible:

```text
User expression
→ parse
→ resolve tensor references
→ type-check shapes and axes
→ generate logical plan
→ estimate cost
→ select physical backend
→ execute lazily
→ stream results
→ generate LaTeX explanation
→ generate visualization instructions
```

It must not treat LaTeX, WeightQL, the renderer, or the chat assistant as independent execution systems.

All input surfaces compile into one canonical, closed, typed intermediate representation.

---

# 2. Core Architectural Decision

Quatricmorph should support three user-facing syntaxes but only one semantic representation.

```text
WeightQL
Tensor Math syntax
Restricted LaTeX syntax
        │
        ▼
Canonical Tensor Math IR
        │
        ├── Type and shape checker
        ├── Query optimizer
        ├── Physical planner
        ├── Tensor runtime
        ├── LaTeX emitter
        └── Visualization compiler
```

## 2.1 WeightQL

WeightQL is used for:

* querying catalogs;
* filtering tensors;
* aggregating statistics;
* joining models;
* comparing checkpoints;
* querying runtime traces;
* selecting result formats;
* declaring execution and approximation policies.

Example:

```sql
SELECT
    layer_index,
    mean(weight),
    stddev(weight),
    l2_norm(weight)
FROM model("kimi-k3").tensors
WHERE role = "attention_query_projection"
GROUP BY layer_index
USING APPROXIMATION block_index
WITH ERROR <= 0.01;
```

## 2.2 Tensor Math syntax

Tensor Math is the compact executable notation used in query cells, APIs, CLI commands, and generated plans.

```text
let A = Q[10][0:128, :]
let B = transpose(K[10][0:128, :])
let C = V[10][0:128, :]

show (A @ B) @ C
```

## 2.3 Restricted LaTeX syntax

LaTeX is a mathematical input and presentation surface.

Example:

```latex
A = Q_{10}[0:128,:]
```

```latex
B = K_{10}[0:128,:]^{\mathsf T}
```

```latex
Y = \left(A B\right)C
```

The LaTeX parser converts these expressions into the same Tensor Math IR as the textual syntax.

LaTeX must never be executed directly.

---

# 3. Design Principles

## 3.1 LaTeX is notation, not an execution language

LaTeX describes mathematical structure. It does not provide:

* file access;
* arbitrary macros;
* user-defined executable functions;
* network access;
* JavaScript evaluation;
* shell interpolation;
* raw SQL;
* backend-specific kernels.

## 3.2 Query semantics are renderer-independent

The renderer receives an already validated visualization plan.

It must not decide:

* what `@` means;
* whether shapes are compatible;
* which tensor an alias represents;
* whether a result is exact;
* whether a computation is allowed;
* which blocks should be read from storage.

## 3.3 Every expensive query is planned first

Before execution, the engine returns:

```text
resolved tensors
result type
result shape
contracted axes
execution tier
estimated bytes read
estimated decoded bytes
estimated peak memory
estimated operations
selected backend
cache expectations
approximation policy
visualization availability
```

## 3.4 Exactness is part of the type

Exact, approximate, sampled, and visualization-only values must not be represented as interchangeable results.

## 3.5 Tensor axes carry semantics

Matching dimensions are necessary but not always sufficient.

For example:

```text
[4096, 4096] @ [4096, 4096]
```

may be numerically valid but semantically questionable if the contracted axes represent unrelated concepts.

The type checker should therefore validate both:

```text
dimension compatibility
+
axis semantic compatibility
```

## 3.6 Parsing must be closed and deterministic

The expression language must use a closed AST or enum.

Unknown operations produce diagnostics rather than falling back to runtime evaluation.

---

# 4. High-Level Architecture

```text
┌───────────────────────────────────────────────────────────┐
│ Query Surfaces                                            │
│ WeightQL Editor · Math Editor · LaTeX Editor · Chat · API │
└───────────────────────────┬───────────────────────────────┘
                            ▼
┌───────────────────────────────────────────────────────────┐
│ Frontend Parsing Layer                                    │
│ WeightQL Parser · Tensor Math Parser · Restricted TeX     │
└───────────────────────────┬───────────────────────────────┘
                            ▼
┌───────────────────────────────────────────────────────────┐
│ Normalization Layer                                       │
│ syntax AST → canonical Query Document + Tensor Math IR    │
└───────────────────────────┬───────────────────────────────┘
                            ▼
┌───────────────────────────────────────────────────────────┐
│ Semantic Resolver                                         │
│ aliases · model scope · tensors · slices · axes · symbols │
└───────────────────────────┬───────────────────────────────┘
                            ▼
┌───────────────────────────────────────────────────────────┐
│ Type and Shape Engine                                     │
│ dtype · rank · dimensions · axes · exactness · devices    │
└───────────────────────────┬───────────────────────────────┘
                            ▼
┌───────────────────────────────────────────────────────────┐
│ Logical Planner                                           │
│ scans · slices · transforms · matmul · reductions · joins │
└───────────────────────────┬───────────────────────────────┘
                            ▼
┌───────────────────────────────────────────────────────────┐
│ Optimizer and Cost Planner                                │
│ pushdown · fusion · block selection · indexes · caching   │
└───────────────────────────┬───────────────────────────────┘
                            ▼
┌───────────────────────────────────────────────────────────┐
│ Physical Planner                                          │
│ catalog · mmap · range · CPU · WebGPU · CUDA · remote     │
└───────────────────────────┬───────────────────────────────┘
                            ▼
┌───────────────────────────────────────────────────────────┐
│ Execution Runtime                                         │
│ block scheduler · memory budget · streaming · cancellation│
└──────────────┬──────────────────────┬─────────────────────┘
               ▼                      ▼
┌────────────────────────┐  ┌───────────────────────────────┐
│ Result and Provenance  │  │ Presentation Compilers        │
│ Arrow · tensor · scalar│  │ LaTeX · explanation · visual  │
└────────────────────────┘  └───────────────────────────────┘
```

---

# 5. Query Document Model

A query should compile into a document rather than a single expression.

```rust
pub struct QueryDocument {
    pub model_scope: ModelScope,
    pub statements: Vec<QueryStatement>,
    pub output: OutputRequest,
    pub execution_policy: ExecutionPolicy,
}
```

```rust
pub enum QueryStatement {
    Let {
        name: Symbol,
        value: Expr,
    },
    Select(SelectQuery),
    Compare(CompareQuery),
    Assert(Assertion),
    Show(Expr),
    Explain(Expr),
    Materialize(MaterializeRequest),
}
```

Example:

```text
use model "kimi-k3"

let Q10 = Q[10][0:128, :]
let K10 = K[10][0:128, :]
let score = Q10 @ transpose(K10)

assert shape(score) == [128, 128]

show softmax(score / sqrt(128))
```

This structure supports:

* multiple declarations;
* reusable intermediate expressions;
* assertions;
* visual output;
* explain-only queries;
* optional materialization;
* reproducible query documents.

---

# 6. Canonical Tensor Math IR

The existing `q-expression` crate should become the authoritative Tensor Math IR rather than creating separate ASTs for each interface.

```rust
pub enum Expr {
    Literal(Literal),
    Symbol(SymbolId),
    TensorRef(TensorReference),
    Slice {
        input: ExprId,
        slices: Vec<SliceSpec>,
    },
    Index {
        input: ExprId,
        indices: Vec<IndexExpr>,
    },
    Unary {
        operator: UnaryOperator,
        input: ExprId,
    },
    Binary {
        operator: BinaryOperator,
        left: ExprId,
        right: ExprId,
    },
    MatMul {
        left: ExprId,
        right: ExprId,
    },
    Transpose {
        input: ExprId,
        permutation: Option<Vec<usize>>,
    },
    Reshape {
        input: ExprId,
        shape: Vec<DimensionExpr>,
    },
    Reduce {
        operator: ReductionOperator,
        input: ExprId,
        axes: Vec<AxisSelector>,
        keep_dimensions: bool,
    },
    Function {
        function: FunctionId,
        arguments: Vec<ExprId>,
        named_arguments: Vec<NamedArgument>,
    },
    Compare {
        left: ExprId,
        right: ExprId,
        metrics: Vec<MetricId>,
    },
}
```

## 6.1 Initial operator set

### Structural

```text
slice
index
transpose
reshape
flatten
squeeze
unsqueeze
broadcast
cast
```

### Arithmetic

```text
+
-
*
/
@
pow
negate
abs
```

### Element-wise neural operations

```text
relu
gelu
silu
sigmoid
exp
log
sqrt
clamp
```

### Reductions

```text
sum
mean
min
max
variance
stddev
l1_norm
l2_norm
infinity_norm
argmin
argmax
```

### Matrix and tensor operations

```text
matmul
batch_matmul
outer
dot
einsum
trace
diagonal
```

`einsum` should be introduced after ordinary matrix multiplication and named-axis reduction are stable.

### Statistical analysis

```text
histogram
quantile
sparsity
cosine_similarity
relative_l2
spectral_sketch
approximate_rank
```

---

# 7. Tensor References and Address Resolution

## 7.1 Canonical reference

```text
model.layers[10].self_attention.query_projection.weight
```

## 7.2 Contextual alias

```text
Q[10]
```

## 7.3 Fully sliced expression

```text
Q[10][0:128, 0:4096]
```

## 7.4 LaTeX alias

```latex
Q_{10}[0:128,0:4096]
```

All forms resolve into:

```rust
pub struct TensorReference {
    pub model_id: ModelId,
    pub tensor_id: TensorId,
    pub canonical_address: CanonicalTensorAddress,
    pub source_name: String,
    pub semantic_role: TensorRole,
    pub resolution: ResolutionEvidence,
}
```

```rust
pub struct ResolutionEvidence {
    pub resolver_plugin: String,
    pub resolver_version: String,
    pub confidence: f32,
    pub alias_source: AliasSource,
    pub warnings: Vec<ResolutionWarning>,
}
```

## 7.5 Alias resolution rules

Resolution must consider:

1. explicitly declared bindings;
2. current selected tensor;
3. active model scope;
4. architecture plugin aliases;
5. canonical role;
6. raw tensor name;
7. contextual UI scope.

If more than one candidate remains, the engine returns ambiguity:

```json
{
  "code": "AMBIGUOUS_TENSOR_ALIAS",
  "input": "Att[10]",
  "candidates": [
    "model.layers.10.self_attn.q_proj.weight",
    "model.layers.10.self_attn.k_proj.weight",
    "model.layers.10.self_attn.v_proj.weight",
    "model.layers.10.self_attn.o_proj.weight"
  ]
}
```

The engine must never select a tensor merely because one candidate appears first.

---

# 8. Tensor Type System

```rust
pub struct TensorType {
    pub dtype: DType,
    pub dimensions: Vec<DimensionExpr>,
    pub axes: Vec<AxisType>,
    pub layout: TensorLayout,
    pub semantic_role: Option<TensorRole>,
    pub exactness: Exactness,
    pub materialization: MaterializationState,
}
```

## 8.1 Symbolic dimensions

Dimensions may be:

```rust
pub enum DimensionExpr {
    Known(u64),
    Symbol(DimensionSymbol),
    Product(Vec<DimensionExpr>),
    Quotient(Box<DimensionExpr>, Box<DimensionExpr>),
    Inferred,
}
```

Example:

```text
Q: [tokens, head_count, head_dimension]
Kᵀ: [head_count, head_dimension, tokens]
```

Symbolic dimensions make it possible to validate a plan before exact runtime values are known.

## 8.2 Axis types

```rust
pub struct AxisType {
    pub role: AxisRole,
    pub size: DimensionExpr,
    pub coordinate_space: Option<CoordinateSpaceId>,
}
```

Possible roles:

```text
batch
token
sequence
layer
expert
attention_head
query_head
key_value_head
input_channel
output_channel
hidden_channel
intermediate_channel
vocabulary
row
column
unknown
```

## 8.3 Exactness

```rust
pub enum Exactness {
    Exact,
    DeterministicApproximation {
        algorithm: AlgorithmId,
        error_bound: Option<f64>,
    },
    Sampled {
        algorithm: AlgorithmId,
        sample_count: u64,
        seed: u64,
    },
    VisualizationOnly {
        source_lod: u8,
    },
}
```

Exactness propagates through expressions.

For example:

```text
Exact @ Exact
→ Exact

Sampled @ Exact
→ Sampled

Approximate reduction over Exact
→ DeterministicApproximation
```

The UI must never display a sampled result as an exact tensor.

---

# 9. Shape and Axis Validation

## 9.1 Matrix multiplication rule

For:

```text
A: [..., M, K]
B: [..., K, N]
```

the result is:

```text
A @ B: [..., M, N]
```

Validation includes:

```text
A.last_dimension == B.second_last_dimension
```

and, when available:

```text
A.last_axis.semantic_space
is compatible with
B.second_last_axis.semantic_space
```

## 9.2 Example success

```text
A: [128, 4096]
axes:
  [token, hidden_channel]

B: [4096, 128]
axes:
  [hidden_channel, projected_channel]

A @ B:
  [128, 128]
axes:
  [token, projected_channel]
```

## 9.3 Semantic mismatch

```text
A: [128, 4096]
axes:
  [token, hidden_channel]

B: [4096, 128]
axes:
  [vocabulary, projected_channel]
```

Even though the dimensions match, the engine should report:

```text
The contracted dimensions are both 4096, but their semantic
axes differ: hidden_channel versus vocabulary.
```

The user may proceed only through an explicit unsafe cast:

```text
assume_axis(B, 0, hidden_channel)
```

Such assumptions must be recorded in query provenance.

## 9.4 Broadcasting

Broadcasting should be conservative.

Allowed:

```text
matrix + scalar
matrix + matching vector along declared axis
batched tensor + compatible batch dimension
```

Rejected unless explicit:

```text
[32, 4096] + [32]
```

The engine cannot know whether `[32]` represents rows, heads, batches, or channels.

Use:

```text
broadcast(bias, axis = output_channel)
```

---

# 10. Tensor Math Syntax

## 10.1 Declarations

```text
let A = tensor("Q[10]")
let B = tensor("K[10]")
```

## 10.2 Slicing

```text
A[0:128, :]
A[:, 512:1024]
A[0:128:2, :]
```

## 10.3 Transpose

```text
transpose(A)
A.T
```

`.T` should only be accepted for rank-two tensors.

For higher ranks:

```text
transpose(A, [0, 2, 1])
```

## 10.4 Matrix multiplication

```text
A @ B
(A @ B) @ C
A @ (B @ C)
```

The parser must preserve grouping because floating-point matrix multiplication is not generally associative in implementation results.

## 10.5 Element-wise multiplication

```text
A * B
```

`*` must never silently mean matrix multiplication.

## 10.6 Reductions

```text
mean(A)
mean(A, axis = output_channel)
l2_norm(A, axis = input_channel)
sum(A, axes = [1, 2], keep_dimensions = true)
```

## 10.7 Named-axis notation

```text
A[row = 0:128, column = :]
Q[token = 42, hidden_channel = :]
Expert[layer = 12, expert = 37]
```

Named-axis selectors are preferable when architecture semantics are available.

---

# 11. Restricted LaTeX Language

## 11.1 Supported constructs

Initial executable LaTeX should support:

```latex
A + B
```

```latex
A - B
```

```latex
\alpha A
```

```latex
A B
```

```latex
A^{\mathsf T}
```

```latex
A_{i,j}
```

```latex
\sum_{k=0}^{K-1} A_{i,k}B_{k,j}
```

```latex
\frac{A}{\sqrt{d}}
```

```latex
\left\lVert A \right\rVert_2
```

```latex
\operatorname{softmax}(A)
```

```latex
Q_{10}[0:128,:]
```

## 11.2 Operator interpretation

| LaTeX                        | Tensor Math IR                        |
| ---------------------------- | ------------------------------------- |
| `A+B`                        | `Add(A,B)`                            |
| `A-B`                        | `Subtract(A,B)`                       |
| `\alpha A`                   | `Multiply(alpha,A)`                   |
| `A B`                        | Type-directed multiplication          |
| `A\odot B`                   | Element-wise multiplication           |
| `A\otimes B`                 | Kronecker product                     |
| `A^{\mathsf T}`              | Transpose                             |
| `A^{-1}`                     | Matrix inverse, initially unsupported |
| `A_{i,j}`                    | Index                                 |
| `\sum_k`                     | Reduction                             |
| `\lVert A\rVert_2`           | L2 norm                               |
| `\operatorname{mean}(A)`     | Mean reduction                        |
| `\operatorname{matmul}(A,B)` | Explicit matrix multiplication        |

## 11.3 Implicit multiplication

Implicit adjacency is potentially ambiguous.

```latex
A B
```

Resolution rules:

1. scalar and scalar: scalar multiplication;
2. scalar and tensor: scalar multiplication;
3. rank-two tensor and rank-two tensor: matrix multiplication when compatible;
4. batched tensor operands: batch matrix multiplication when compatible;
5. otherwise: ambiguity diagnostic.

For critical or reusable queries, the canonical emitter should prefer:

```latex
\operatorname{matmul}(A,B)
```

over ambiguous adjacency.

## 11.4 Unsupported LaTeX

The parser must reject:

```text
arbitrary macro definitions
URL commands
HTML commands
graphics inclusion
file inclusion
shell escape concepts
unknown environments
persistent global definitions
```

Presentation-only commands such as color or spacing may be ignored by the semantic parser, but they must never modify execution semantics.

## 11.5 Custom Quatricmorph commands

A small fixed set of semantic macros may be defined:

```latex
\qtensor{Q[10]}
\qslice{Q[10]}{0:128,:}
\qmean{A}
\qnorm{A}{2}
\qmatmul{A}{B}
```

These are parser-recognized constructs, not user-defined LaTeX macros.

---

# 12. LaTeX Processing Pipeline

```text
LaTeX source
→ lexical scanner
→ restricted TeX syntax tree
→ presentation-command removal
→ mathematical normalization
→ symbol and tensor resolution
→ Tensor Math IR
→ type checking
```

The engine should not use rendered HTML as parser input.

## 12.1 Separate input parser and output renderer

```text
q-latex-parser
    LaTeX → Tensor Math IR

q-latex-emitter
    Tensor Math IR → canonical LaTeX

KaTeX
    canonical LaTeX → HTML/MathML presentation
```

## 12.2 Canonical round-trip

Input:

```latex
(K_{10}^{T}Q_{10})V_{10}
```

Normalized IR:

```text
MatMul(
    MatMul(
        Transpose(K[10]),
        Q[10]
    ),
    V[10]
)
```

Canonical LaTeX:

```latex
\operatorname{matmul}
\left(
  \operatorname{matmul}
  \left(
    K_{10}^{\mathsf T},
    Q_{10}
  \right),
  V_{10}
\right)
```

Canonical Tensor Math:

```text
(transpose(K[10]) @ Q[10]) @ V[10]
```

The original source may be retained for display, but execution uses only normalized IR.

---

# 13. WeightQL and Math Integration

WeightQL should embed math expressions rather than implement a separate arithmetic subsystem.

```sql
LET A = tensor("Q[10]")[0:128, :];
LET B = transpose(tensor("K[10]")[0:128, :]);
LET C = tensor("V[10]")[0:128, :];

SHOW (A @ B) @ C
USING EXACT
WITH BACKEND AUTO;
```

Statistical query:

```sql
SELECT
    layer_index,
    l2_norm(weight) AS norm,
    mean(weight) AS mean
FROM model("kimi-k3").tensors
WHERE role = "attention_query_projection"
USING APPROXIMATION "block-index"
WITH ERROR <= 0.01;
```

Explain-only query:

```sql
EXPLAIN
SHOW
    softmax(
        Q[10][0:128, :] @ transpose(K[10][0:128, :])
        / sqrt(128)
    );
```

---

# 14. Compilation Pipeline

## Stage 1 — Parse

Output:

```text
syntax-specific AST
source ranges
parse diagnostics
comments
original formatting
```

## Stage 2 — Normalize

Normalize equivalent operations:

```text
A.T
transpose(A)
A^{\mathsf T}
```

into:

```text
Transpose(A)
```

## Stage 3 — Resolve

Resolve:

* model references;
* tensor aliases;
* variables;
* functions;
* axes;
* slices;
* architecture roles.

## Stage 4 — Type-check

Infer:

* dtype;
* dimensions;
* named axes;
* exactness;
* materialization state;
* semantic compatibility.

## Stage 5 — Build logical plan

Example:

```text
MatMul
├── Slice
│   └── TensorScan(Q[10])
└── TransposeView
    └── Slice
        └── TensorScan(K[10])
```

## Stage 6 — Optimize

Possible rewrites:

```text
slice pushdown
predicate pushdown
projection pruning
transpose as view
cast elimination
constant folding
block alignment
reduction fusion
matmul-reduction fusion
cached-summary substitution
common-subexpression elimination
```

## Stage 7 — Estimate cost

Calculate:

```text
artifact bytes requested
range-request count
decoded bytes
CPU memory
GPU memory
host-to-device transfer
estimated operation count
temporary tensor size
cache hits
result size
```

## Stage 8 — Select physical plan

Choose:

```text
catalog lookup
derived-index lookup
mmap block reader
HTTP range reader
CPU SIMD
BLAS
WebGPU
CUDA
Metal backend
Python runtime adapter
distributed worker
```

## Stage 9 — Execute

Execute by tensor block with:

* cancellation;
* memory budgets;
* progress events;
* partial reductions;
* resumable large jobs;
* cache writes;
* provenance collection.

## Stage 10 — Compile outputs

Produce:

* scalar;
* table;
* tensor view;
* materialized tensor;
* virtual tensor;
* query explanation;
* canonical LaTeX;
* visualization plan.

---

# 15. Logical Query Plan

```rust
pub enum LogicalOperator {
    CatalogScan,
    SemanticFilter,
    TensorReference,
    TensorSlice,
    TensorIndex,
    Cast,
    Transpose,
    Reshape,
    Broadcast,
    Elementwise,
    MatMul,
    Reduction,
    Statistics,
    Compare,
    Join,
    Materialize,
    Visualize,
}
```

Logical operators do not contain backend-specific implementation details.

Example:

```text
Visualize(mode = matrix_multiplication)
└── MatMul
    ├── Slice(rows = 0:128, columns = all)
    │   └── TensorReference(Q[10])
    └── Transpose
        └── Slice(rows = 0:128, columns = all)
            └── TensorReference(K[10])
```

---

# 16. Physical Query Plan

```rust
pub enum PhysicalOperator {
    DuckDbCatalogScan,
    ParquetIndexScan,
    CachedStatisticsLookup,
    SafeTensorRangeRead,
    MemoryMappedBlockRead,
    DecodeBlock,
    Reblock,
    CpuElementwise,
    CpuMatMul,
    BlasMatMul,
    WebGpuMatMul,
    CudaMatMul,
    MetalMatMul,
    PartialReduction,
    MergeReduction,
    ResultStream,
    QTileEmitter,
}
```

Example:

```text
ResultStream
└── WebGpuMatMul
    ├── UploadBlocks
    │   └── SafeTensorRangeRead(Q ranges)
    └── UploadBlocks
        └── TransposeView
            └── SafeTensorRangeRead(K ranges)
```

---

# 17. Execution Tiers

```text
Tier 0 — Catalog only
No tensor bytes read.

Tier 1 — Derived-index lookup
Statistics, sketches, cached summaries.

Tier 2 — Selected tile or block
Small ranged reads.

Tier 3 — Full single-tensor scan
Streamed tensor scan.

Tier 4 — Aligned multi-tensor scan
Comparison or joined tensor operations.

Tier 5 — GPU tensor execution
Matmul, large reductions, quantization.

Tier 6 — Materialization
Write a derived tensor or model artifact.

Tier 7 — Runtime activation capture
Model execution required.

Tier 8 — Distributed execution or evaluation
Remote workers and aggregation.
```

The compile response must expose the tier before execution.

---

# 18. Execution Policy

```rust
pub struct ExecutionPolicy {
    pub mode: ExecutionMode,
    pub approximation: ApproximationPolicy,
    pub backend: BackendPolicy,
    pub memory_budget: MemoryBudget,
    pub io_budget: IoBudget,
    pub materialization: MaterializationPolicy,
}
```

## 18.1 Execution mode

```rust
pub enum ExecutionMode {
    ExplainOnly,
    Interactive,
    BackgroundJob,
    FullCompute,
}
```

## 18.2 Approximation policy

```rust
pub enum ApproximationPolicy {
    ExactOnly,
    PreferExact,
    AllowApproximate {
        maximum_error: Option<f64>,
    },
    Sample {
        sample_count: u64,
        seed: u64,
    },
    VisualizationOnly,
}
```

## 18.3 Backend policy

```rust
pub enum BackendPolicy {
    Auto,
    Cpu,
    WebGpu,
    Cuda,
    Metal,
    Remote,
}
```

Backend selection must not change query semantics, although floating-point results may differ slightly due to operation ordering and precision.

---

# 19. Block-Oriented Matrix Multiplication

The QueryEngine must not materialize full operands when the requested output is a selected region.

For:

```text
C = A @ B
```

and requested output:

```text
C[0:128, 0:128]
```

the planner computes:

```text
for each K block:
    read A[0:128, Kblock]
    read B[Kblock, 0:128]
    partial = Ablock @ Bblock
    accumulate partial into Cblock
```

Physical plan:

```text
Initialize output block C[0:128,0:128]

for k_block in contracted_axis:
    RangeRead A[0:128,k_block]
    RangeRead B[k_block,0:128]
    Decode
    MatMul
    Accumulate

Return C block
```

## 19.1 Virtual intermediates

For:

```text
(A @ B) @ C
```

the first multiplication result may remain virtual.

The optimizer may evaluate either:

```text
materialize(A @ B) → multiply by C
```

or:

```text
fuse selected blocks through both operations
```

depending on:

* dimensions;
* selected output slice;
* memory budget;
* cache state;
* backend;
* expected reuse.

The optimizer must preserve the user’s explicit grouping unless an algebraic rewrite is explicitly permitted.

---

# 20. Visualization IR

Query IR and visualization state must remain separate.

```rust
pub struct VisualizationPlan {
    pub plan_id: PlanId,
    pub result_id: Option<ResultId>,
    pub root: VisualNode,
    pub data_bindings: Vec<VisualDataBinding>,
    pub exactness: Exactness,
}
```

```rust
pub enum VisualNode {
    TensorPlane(TensorPlaneSpec),
    MatMul(MatMulVisualSpec),
    Reduction(ReductionVisualSpec),
    ExpressionGraph(ExpressionGraphSpec),
    Heatmap(HeatmapSpec),
    Distribution(DistributionSpec),
    ScalarInspector(ScalarInspectorSpec),
}
```

## 20.1 Matrix multiplication visualization

```rust
pub struct MatMulVisualSpec {
    pub left: VisualTensorRef,
    pub right: VisualTensorRef,
    pub output: VisualTensorRef,
    pub contracted_axis: AxisDescriptor,
    pub selected_output_region: LogicalSlice,
    pub animation_schedule: AnimationSchedule,
}
```

Animation events:

```rust
pub enum AnimationEvent {
    HighlightLeftCell,
    HighlightRightCell,
    EmitProduct,
    AccumulateOutput,
    CompleteOutputCell,
    AdvanceContractedIndex,
    CompleteBlock,
}
```

The existing `mm` behavior can be retained as the visual implementation of these events, while its parser and data ownership are replaced.

## 20.2 Example schedule

For one output cell:

```text
C[i,j] = Σₖ A[i,k]B[k,j]
```

```json
[
  {
    "event": "highlight_left",
    "index": ["i", 0]
  },
  {
    "event": "highlight_right",
    "index": [0, "j"]
  },
  {
    "event": "emit_product",
    "expression": "A[i,0] * B[0,j]"
  },
  {
    "event": "accumulate_output",
    "index": ["i", "j"]
  }
]
```

---

# 21. Result Model

```rust
pub struct QueryResult {
    pub result_id: ResultId,
    pub plan_id: PlanId,
    pub result_type: ResultType,
    pub exactness: Exactness,
    pub value: ResultPayload,
    pub provenance: QueryProvenance,
    pub canonical_math: CanonicalMathRepresentations,
}
```

```rust
pub enum ResultPayload {
    Scalar(ScalarValue),
    RecordBatch(ArrowBatchReference),
    TensorView(TensorView),
    MaterializedTensor(MaterializedTensorReference),
    VirtualTensor(VirtualTensorReference),
    Visualization(VisualizationPlan),
}
```

## 21.1 Canonical math representations

```rust
pub struct CanonicalMathRepresentations {
    pub tensor_math: String,
    pub latex: String,
    pub weightql: Option<String>,
}
```

## 21.2 Provenance

```rust
pub struct QueryProvenance {
    pub source_model_hashes: Vec<ContentHash>,
    pub tensor_ids: Vec<TensorId>,
    pub source_ranges: Vec<SourceRange>,
    pub query_hash: ContentHash,
    pub logical_plan_hash: ContentHash,
    pub physical_plan_hash: ContentHash,
    pub backend: BackendDescriptor,
    pub algorithm_versions: Vec<AlgorithmVersion>,
    pub approximation: Exactness,
    pub seed: Option<u64>,
}
```

---

# 22. Query API

## 22.1 Compile query

```http
POST /v1/query/compile
```

```json
{
  "model": "kimi-k3",
  "language": "latex",
  "source": "\\operatorname{softmax}\\left(\\frac{Q_{10}K_{10}^{\\mathsf T}}{\\sqrt{128}}\\right)",
  "bindings": {},
  "policy": {
    "mode": "interactive",
    "approximation": "exact_only",
    "backend": "auto",
    "maximum_gpu_bytes": 1073741824
  }
}
```

Response:

```json
{
  "plan_id": "plan:b3:...",
  "normalized_expression": "softmax((Q[10] @ transpose(K[10])) / sqrt(128))",
  "canonical_latex": "\\operatorname{softmax}\\left(\\frac{\\operatorname{matmul}(Q_{10},K_{10}^{\\mathsf T})}{\\sqrt{128}}\\right)",
  "result_type": {
    "dtype": "f32",
    "shape": [4096, 4096],
    "axes": ["token", "token"]
  },
  "tier": 5,
  "estimated_read_bytes": 67108864,
  "estimated_gpu_bytes": 201326592,
  "exactness": "exact",
  "backend": "webgpu",
  "diagnostics": [],
  "executable": true
}
```

## 22.2 Execute plan

```http
POST /v1/query/plans/{planId}/execute
```

## 22.3 Stream progress

```http
GET /v1/query/executions/{executionId}/events
```

Possible events:

```text
plan_started
range_read_started
range_read_completed
block_decoded
operator_progress
partial_result
visualization_ready
execution_completed
execution_cancelled
execution_failed
```

## 22.4 Cancel

```http
POST /v1/query/executions/{executionId}/cancel
```

## 22.5 Inspect plan

```http
GET /v1/query/plans/{planId}
GET /v1/query/plans/{planId}/logical
GET /v1/query/plans/{planId}/physical
GET /v1/query/plans/{planId}/visualization
```

---

# 23. Query Editor UX

The query workspace should contain five coordinated areas.

```text
┌─────────────────────────────────────────────────────────┐
│ Model scope · backend · exactness · memory budget       │
├──────────────────────────┬──────────────────────────────┤
│ Query editor             │ Live mathematical preview    │
│ WeightQL / Math / LaTeX  │ Canonical LaTeX              │
├──────────────────────────┼──────────────────────────────┤
│ Diagnostics              │ Resolved tensor inspector    │
├──────────────────────────┴──────────────────────────────┤
│ Plan · Cost · Results · Visualization · Provenance      │
└─────────────────────────────────────────────────────────┘
```

## 23.1 Editor modes

```text
WeightQL
Tensor Math
LaTeX
```

Changing the editor mode should convert through canonical IR, not directly from one source string to another.

```text
LaTeX
→ IR
→ Tensor Math
```

not:

```text
LaTeX
→ regex replacement
→ Tensor Math
```

## 23.2 Live diagnostics

Examples:

```text
Q[10] resolves to:
model.layers.10.self_attn.q_proj.weight
shape: [4096, 4096]
dtype: BF16
```

```text
Shape mismatch:
left contracted dimension = 4096
right contracted dimension = 5120
```

```text
This query requires approximately 64 MiB of ranged reads.
```

```text
The displayed heatmap is sampled from 262,144 values.
```

## 23.3 Completion

Completion sources:

* models;
* layers;
* architecture components;
* tensor aliases;
* canonical tensor names;
* variables;
* functions;
* axes;
* named arguments;
* approximation strategies;
* execution backends.

## 23.4 Hover information

Hovering over `Q[10]` should show:

```text
Canonical tensor
Semantic role
Shape
Dtype
Shard
Byte range
Cached summaries
Resolution confidence
```

---

# 24. LaTeX Editor and Renderer

Recommended UI separation:

```text
Monaco
→ WeightQL and textual Tensor Math editing

MathLive-style math field
→ structured interactive LaTeX entry

KaTeX
→ passive rendering of canonical equations
```

Neither UI library is authoritative for tensor semantics.

The Rust QueryEngine remains the source of truth.

## 24.1 Rendering policy

For untrusted LaTeX:

```text
trust = false
persistent macros = disabled
macro expansion bounded
render size bounded
external resources forbidden
errors escaped before display
```

Only a fixed Quatricmorph macro dictionary should be supplied.

## 24.2 Math editor policy

The math field may emit:

```text
LaTeX source
structured MathJSON-like tree
```

However, Quatricmorph must parse and validate the expression again through its own restricted grammar.

The browser-side structure is an editor aid, not trusted executable IR.

---

# 25. Error Model

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub source_range: SourceRange,
    pub related: Vec<RelatedDiagnostic>,
    pub suggestions: Vec<FixSuggestion>,
}
```

## 25.1 Diagnostic categories

```text
syntax error
unknown symbol
ambiguous tensor
unknown function
invalid slice
shape mismatch
axis mismatch
dtype mismatch
unsupported LaTeX
unsupported operation
execution policy violation
memory budget exceeded
I/O budget exceeded
backend unavailable
approximation requirement unsatisfied
```

## 25.2 Example LaTeX diagnostic

Input:

```latex
Q_{10}K_{10}
```

Diagnostic:

```text
Ambiguous multiplication.

Both operands are rank-two tensors. The expression can be
interpreted as matrix multiplication, but their contracted
dimensions are incompatible:

Q₁₀: [4096, 4096]
K₁₀: [1024, 4096]

Did you mean:

Q₁₀ K₁₀ᵀ
```

Suggested edit:

```latex
Q_{10}K_{10}^{\mathsf T}
```

---

# 26. Caching

## 26.1 Plan cache

```text
hash(
    normalized_query,
    model_hashes,
    resolver_versions,
    function_registry_version,
    execution_policy
)
```

## 26.2 Result cache

```text
hash(
    logical_plan_hash,
    source_hashes,
    logical_slices,
    exactness_policy,
    algorithm_versions,
    backend_precision_contract
)
```

## 26.3 Presentation cache

```text
hash(
    canonical_latex,
    rendering_options,
    renderer_version
)
```

Presentation settings such as color theme must not invalidate the tensor result cache.

---

# 27. Security Requirements

## 27.1 No arbitrary execution

The QueryEngine must contain no:

```text
eval
Function constructor
dynamic JavaScript compilation
raw shell command
raw Python execution
unrestricted SQL execution
user-defined native plugin loading
LaTeX shell escape
remote resource inclusion
```

## 27.2 Closed function registry

```rust
pub struct FunctionDefinition {
    pub id: FunctionId,
    pub signatures: Vec<FunctionSignature>,
    pub logical_builder: LogicalOperatorBuilder,
    pub capabilities: FunctionCapabilities,
}
```

Only registered functions can appear in executable IR.

## 27.3 Resource limits

Every query has:

```text
maximum source bytes
maximum decoded bytes
maximum CPU memory
maximum GPU memory
maximum output bytes
maximum execution tier
maximum parallel range reads
maximum expression depth
maximum AST nodes
maximum macro expansion
```

## 27.4 Chat restrictions

The chat assistant may:

* generate a query;
* compile a query;
* explain diagnostics;
* request a plan;
* execute a permitted plan.

The chat assistant may not:

* bypass compile;
* change budgets silently;
* reinterpret ambiguous aliases silently;
* automatically approve full-model scans;
* claim approximate results are exact.

---

# 28. Repository Structure

The design should extend the current crates rather than duplicating them.

```text
quatricmorph/
├── crates/
│   ├── q-weightql/
│   │   ├── lexer.rs
│   │   ├── parser.rs
│   │   ├── document_ast.rs
│   │   └── formatter.rs
│   │
│   ├── q-expression/
│   │   ├── expr.rs
│   │   ├── operators.rs
│   │   ├── functions.rs
│   │   ├── types.rs
│   │   └── visitor.rs
│   │
│   ├── q-latex/
│   │   ├── lexer.rs
│   │   ├── parser.rs
│   │   ├── normalizer.rs
│   │   ├── emitter.rs
│   │   ├── macros.rs
│   │   └── diagnostics.rs
│   │
│   ├── q-query-engine/
│   │   ├── compile.rs
│   │   ├── resolver.rs
│   │   ├── typecheck.rs
│   │   ├── logical_plan.rs
│   │   ├── optimizer.rs
│   │   ├── cost.rs
│   │   └── policy.rs
│   │
│   ├── q-tensor-runtime/
│   │   ├── physical_plan.rs
│   │   ├── scheduler.rs
│   │   ├── block.rs
│   │   ├── memory.rs
│   │   ├── stream.rs
│   │   └── backends/
│   │       ├── cpu.rs
│   │       ├── webgpu.rs
│   │       ├── cuda.rs
│   │       ├── metal.rs
│   │       └── remote.rs
│   │
│   ├── q-query-result/
│   │   ├── result.rs
│   │   ├── exactness.rs
│   │   ├── provenance.rs
│   │   └── arrow.rs
│   │
│   └── q-visualization-plan/
│       ├── visual_ir.rs
│       ├── matmul.rs
│       ├── heatmap.rs
│       └── animation.rs
│
├── apps/
│   └── web/
│       ├── query-interface/
│       └── quatricmorph-workspace/
│
├── schemas/
│   ├── query/
│   ├── expression/
│   ├── execution-plan/
│   └── query-result/
│
└── fixtures/
    ├── golden-queries/
    ├── golden-latex/
    ├── shape-errors/
    └── backend-parity/
```

---

# 29. Integration with the Existing `mm` Visualizer

## Reuse

Retain the mathematical and visual concepts behind:

```text
recursive MatMul tree
block decomposition
dot-product sequencing
matrix placement
row and column guides
animation cursor scheduling
highlight and accumulation behavior
```

## Replace

Replace:

```text
hand-written query parser
URL-controlled expressions
eval-based initializers
renderer-owned matrix data
global mutable parameters
scene reconstruction as query execution
```

## Target boundary

```text
QueryEngine
→ VisualizationPlan
→ quatricmorph-workspace adapter
→ renderer
```

The matrix workspace should not receive raw WeightQL or LaTeX.

Example adapter:

```ts
export interface MatrixWorkspacePlan {
  expressionId: string;
  root: MatrixVisualNode;
  tensors: VisualTensorBinding[];
  animation: AnimationSchedule;
  exactness: ExactnessDescriptor;
}
```

---

# 30. Implementation Phases

## Phase QE-0 — Closed Math Expression Core

Implement:

* closed `Expr` enum;
* scalar literals;
* tensor symbols;
* `+`, `-`, `*`, `/`, `@`;
* transpose;
* slices;
* shape inference;
* deterministic formatter;
* no execution.

Acceptance:

```text
(A @ B) @ C
```

parses into a stable recursive AST.

## Phase QE-1 — Tensor Resolution

Implement:

* canonical tensor addresses;
* contextual aliases;
* model scope;
* ambiguity results;
* architecture resolver integration;
* exact scalar and slice lookup.

Acceptance:

```text
Q[10][100,42]
```

resolves to one exact SafeTensors address and byte range.

## Phase QE-2 — Type and Shape Engine

Implement:

* dtype;
* symbolic dimensions;
* named axes;
* matrix multiplication checks;
* broadcasting rules;
* exactness propagation;
* diagnostics.

Acceptance:

Shape-incompatible expressions fail without reading tensor bytes.

## Phase QE-3 — Planner and Block Runtime

Implement:

* logical plans;
* physical plans;
* cost estimation;
* SafeTensors range reads;
* CPU block matrix multiplication;
* cancellation;
* progress events.

Acceptance:

```text
Q[10][0:128,:] @ transpose(K[10][0:128,:])
```

reads only required ranges.

## Phase QE-4 — WeightQL Integration

Implement:

* declarations;
* `SELECT`;
* statistics;
* `SHOW`;
* `EXPLAIN`;
* execution policies;
* approximation controls.

Acceptance:

Metadata-only queries remain Tier 0 and read no tensor bytes.

## Phase QE-5 — LaTeX Input and Output

Implement:

* restricted LaTeX parser;
* canonical LaTeX emitter;
* tensor aliases with subscripts;
* transpose;
* fractions;
* norms;
* reductions;
* fixed semantic macros.

Acceptance:

```latex
\operatorname{softmax}
\left(
  \frac{Q_{10}K_{10}^{\mathsf T}}{\sqrt{128}}
\right)
```

round-trips through Tensor Math IR.

## Phase QE-6 — Visualization Compiler

Implement:

* expression graph;
* matrix planes;
* contracted-axis descriptions;
* block selection;
* animation schedules;
* exactness labels.

Acceptance:

The same compiled query drives both numerical execution and matrix animation.

## Phase QE-7 — GPU Backends

Implement:

* WebGPU block operations;
* native `wgpu`;
* CUDA plugin;
* backend parity tests;
* memory scheduler;
* transfer-aware cost model.

## Phase QE-8 — Runtime Activations

Implement:

* activation references;
* token and sequence axes;
* prompt-bound query scopes;
* runtime Q/K/V;
* attention score queries;
* MoE routing tables.

---

# 31. MVP Scope

The first useful QueryEngine MVP should support:

```text
one active model
known Qwen/Llama-like architecture
SafeTensors
canonical tensor references
Q/K/V aliases
rank-one and rank-two tensors
scalar and rectangular slices
transpose
addition
element-wise multiplication
matrix multiplication
mean and L2 norm
CPU block execution
exact-only mode
Tensor Math syntax
canonical LaTeX output
matrix multiplication visualization
```

LaTeX editing can follow after the canonical IR and type checker are stable.

The engine should generate LaTeX before it accepts LaTeX as executable input.

This sequence prevents presentation syntax from defining the core semantics.

---

# 32. Acceptance Criteria

1. No query path uses `eval` or dynamic code execution.
2. WeightQL, Tensor Math, and LaTeX compile into the same IR.
3. An ambiguous tensor alias never resolves silently.
4. Shape errors are reported before tensor bytes are read.
5. Axis-semantic mismatches are distinguishable from dimension mismatches.
6. Exact, sampled, approximate, and visualization-only results use different result states.
7. The planner exposes estimated I/O and memory before expensive execution.
8. Small slices do not materialize full tensors.
9. Matrix multiplication executes by logical tensor blocks.
10. Transpose uses a view where the backend supports it.
11. Cached statistics can satisfy eligible queries without scanning source tensors.
12. The renderer receives a Visualization Plan rather than query source.
13. The same result can be accessed through UI, CLI, SDK, and API.
14. Every result includes source hashes and algorithm versions.
15. Canonical Tensor Math and canonical LaTeX are generated for every mathematical result.
16. Invalid or unsupported LaTeX produces source-positioned diagnostics.
17. User-defined persistent LaTeX macros are disabled.
18. Query cancellation stops new block scheduling.
19. Backend implementations pass numerical tolerance tests.
20. Exact scalar results match the Python SafeTensors reference.

---

# 33. End-to-End Example

## User LaTeX

```latex
S =
\operatorname{softmax}
\left(
  \frac{
    Q_{10}[0:128,:]
    K_{10}[0:128,:]^{\mathsf T}
  }{
    \sqrt{128}
  }
\right)
```

## Canonical Tensor Math

```text
let Q_block = Q[10][0:128, :]
let K_block = K[10][0:128, :]

show softmax(
    (Q_block @ transpose(K_block))
    / sqrt(128)
)
```

## Resolved types

```text
Q_block:
  BF16[128, 4096]
  axes: [token, hidden_channel]

K_block:
  BF16[128, 4096]
  axes: [token, hidden_channel]

transpose(K_block):
  BF16[4096, 128]
  axes: [hidden_channel, token]

Q_block @ transpose(K_block):
  F32[128, 128]
  axes: [token, token]
```

## Logical plan

```text
Softmax(axis = token)
└── Divide(sqrt(128))
    └── MatMul
        ├── Slice(Q[10], 0:128, :)
        └── Transpose
            └── Slice(K[10], 0:128, :)
```

## Physical plan

```text
ResultStream
└── CpuSoftmax
    └── CpuScale
        └── BlasMatMul
            ├── SafeTensorRangeRead(Q block)
            └── SafeTensorRangeRead(K block)
```

## Visualization plan

```text
Matrix multiplication
├── Q block on XY plane
├── Kᵀ block on YZ plane
├── score matrix on XZ plane
├── hidden_channel as contracted axis
└── softmax as result epilogue
```

## Result contract

```json
{
  "exactness": "exact",
  "source": "checkpoint",
  "execution_backend": "cpu_blas",
  "result_shape": [128, 128],
  "visualization_mode": "real_tensor_block"
}
```

---

# 34. Final Architectural Recommendation

The QueryEngine should be treated as Quatricmorph’s central compiler and analytical runtime.

Its authoritative flow should be:

```text
WeightQL / Tensor Math / Restricted LaTeX
→ canonical typed Tensor Math IR
→ semantic tensor resolution
→ shape and axis validation
→ logical query plan
→ cost-aware physical plan
→ block-oriented execution
→ reproducible result
→ LaTeX and visualization projections
```

The critical separation is:

```text
LaTeX describes mathematics.
WeightQL describes queries.
Tensor Math IR defines semantics.
The planner decides execution.
The runtime computes values.
The visualization compiler explains the computation.
```

This creates one substrate for:

```text
Inspect
→ Query
→ Visualize
→ Compare
→ Morph
→ Verify
```

rather than separate and eventually inconsistent systems for SQL-like queries, mathematical formulas, chat commands, and 3D visualization.
