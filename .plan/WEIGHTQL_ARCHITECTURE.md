# WEIGHTQL_ARCHITECTURE — grammar, planning, execution tiers

## 0. State

WeightQL is the most complete subsystem in the repository. Twelve requirement
rows, ten `Verified`. The lexer, parser, resolver, shape checker, cost estimator,
and plan-ID generator all work; scalar and slice queries **execute**. Three gaps
remain: matmul execution (`WQL-006`), statistical `SELECT` (`WQL-007`), and
stacked slices (`WQL-008`).

This document specifies the whole language and marks each part's state.

---

## 1. Grammar

The implemented grammar, verbatim from `crates/q-weightql/src/parser.rs:7-24`:

```text
script      := statement (';'? statement)* ';'?
statement   := assignment | show | select_value | select_slice
assignment  := IDENT '=' expr
show        := 'show' expr
select_value:= 'SELECT' 'value' 'FROM' tensor_call 'AT' '[' int (',' int)* ']'
select_slice:= 'SELECT' 'slice' 'FROM' tensor_call
               ['ROWS' range] ['COLUMNS' range]

expr        := add
add         := mul (('+'|'-') mul)*
mul         := postfix ('@' postfix)*
postfix     := primary ('[' selector ']')*
primary     := tensor_call | call | IDENT | '(' expr ')'
tensor_call := 'tensor' '(' STRING ')'
call        := ('transpose'|'min'|'max'|'mean'|'variance'|'stddev'
               |'l1_norm'|'l2_norm'|'zero_ratio') '(' expr ')'
             | 'compare' '(' expr ',' expr ')' 'by' IDENT
```

Keywords match case-insensitively, so both the SQL-flavoured forms of
`ARCHITECTURE.md` §7.1–7.3 and the expression form of §7.4 read naturally.

**There is no production for `eval`, for a function *definition*, for a shell
escape, or for raw SQL.** Adding one would require changing
`q_expression::Expr`, which is a closed enum. That is the structural answer to
`mm`'s `eval`-from-a-URL vulnerability (`docs/CURRENT_ARCHITECTURE.md` §5,
`ADR-006`).

### 1.1 To be added

| Form | Requirement | Task |
| --- | --- | --- |
| `SELECT layer_index, mean(weight), l2_norm(weight) FROM model(…).tensors WHERE role = … GROUP BY layer_index` | `WQL-007` | `QM-0072` |
| Stacked slices — `A[0:64][0:8]` composing to `A[0:8]` | `WQL-008` | `QM-0071` |

`WQL-007` is currently rejected **by name, carrying its own requirement ID**
(`unsupported_select_target_is_named_with_its_requirement`) — the error tells the
user which gap they hit rather than "syntax error".

---

## 2. AST

`q_expression::Expr` — a closed enum (`crates/q-expression/src/lib.rs:147`):

| Variant | Form | Note |
| --- | --- | --- |
| `TensorRef { text }` | `tensor("Q[10]")` or a bound name | Text is the address **as written**; resolution happens in `q-weightql`, keeping `q-expression` storage-free |
| `Slice { selector, .. }` | `A[0:256, 0:256]` | |
| `Transpose(x)` | `transpose(x)` | **Explicit only.** Never inserted silently to make shapes line up (`ARCHITECTURE.md` §7.4 step 3) |
| `MatMul(a, b)` | `a @ b` | The cost driver |
| `Add(a, b)` / `Sub(a, b)` | `a + b`, `a - b` | |
| `Reduce { op, x }` | `mean(x)`, `l2_norm(x)`, … | `Reduction` enum |
| `Compare { a, b, metric }` | `compare(a,b) by cosine_similarity` | `ComparisonMetric` enum |

```text
show tensor("Q[10]") @ transpose(tensor("K[10]"))

MatMul
├── TensorRef("Q[10]")
└── Transpose
    └── TensorRef("K[10]")
```

Helper methods already present and used by the planner: every tensor reference in
evaluation order; matmul count; and **whether the expression needs a compute
backend or is a pure read** — the distinction that lets scalar and slice queries
execute today while matmul stops with an explanation.

---

## 3. Addresses and aliases

### 3.1 Canonical

```text
model.layers[10].self_attention.query_projection.weight
model.layers[10].self_attention.query_projection.weight[100,42]
```

Unique, stable across resolution runs, and the join key everywhere
([`DATA_ARCHITECTURE.md`](DATA_ARCHITECTURE.md) §4).

### 3.2 Aliases

Verified by `alias::tests::parses_the_five_architecture_md_forms`:

```text
Q[10]                      K[10][0:256,0:256]
Q[10][100,42]              MLP.down[24][:]
Expert[12,37].up[0:128,:]
```

### 3.3 Contextual selectors

The UI may accept `layer[0][10].attention[1].Q[0]`. **It must resolve to a
canonical address before execution** — no execution path accepts a contextual
form. The resolution step is where "current UI selection" is allowed to
disambiguate, and where that use is recorded in the plan so the user can see what
was assumed.

### 3.4 Ambiguity

`Att[10]` may mean Q, K, V, O, or attention-related metadata.

**Return candidates. Never pick.** Already implemented
(`ambiguous_alias_returns_candidates_not_a_silent_pick`) and surfaced by the
daemon as `409` with the candidate list (`an_ambiguous_alias_is_a_409_carrying_its_candidates`).

A candidate carries: canonical address, raw name, role, shape, dtype, layer
index, and — where UI selection could disambiguate — a `suggested` flag with the
reason. The user chooses; the system proposes.

---

## 4. Pipeline

```text
input text
  → tokenize                    lexer.rs      WQL-001 ✓
  → parse                       parser.rs     WQL-002 ✓
  → AST                         q-expression  ✓
  → alias resolution            plan.rs       WQL-003 ✓   (may return candidates)
  → canonical tensor references                ✓
  → shape checking              infer_shape   WQL-004 ✓   ← fails HERE, before any read
  → cost estimation             plan.rs       WQL-010 ✓
  → execution tier selection                  WQL-013 ✗   → QM-0073
  → query plan (with plan_id)   WQL-012 ✓
  → EXPLICIT user execution                   partial     → QM-0073
```

**Shape checking happens before anything can read or launch, by construction.**
`infer_shape` is a pure function over the AST and a `ShapeEnvironment`; it has no
`ModelSource` and no backend, so it *cannot* reach a disk or a GPU. The test
`shape_mismatch_is_rejected_before_execution` notes exactly this: the
planning-only engine has no `ModelSource`, so it cannot read even if it wanted
to. The daemon surfaces a mismatch as `400` before any read
(`a_shape_mismatch_is_a_400_before_any_read`).

Type-checking example from `ARCHITECTURE.md` §7.4:

```text
A: [128, 4096]   B: [4096, 128]   A @ B: [128, 128]
C: [128, 4096]   (A @ B) @ C: [128, 4096]
```

---

## 5. Cost estimation

Every plan carries an estimate before anything runs (`WQL-010`).

```text
estimated_read_bytes  = Σ over tensor refs: SourceByteRanges::total_bytes()
estimated_host_bytes  = Σ over materialized intermediates: elements × 4
estimated_gpu_bytes   = max over matmuls: (m×k + k×n + m×n) × 4
estimated_flops       = Σ over matmuls: 2 × m × k × n
```

For `Q[10][0:256,0:256] @ transpose(K[10][0:256,0:256])`, f32:

```text
read   2 × 256 × 256 × 4  =    512 KiB
gpu    3 × 256 × 256 × 4  =    768 KiB
flops  2 × 256³           = 33.5 MFLOP
```

Trivial — which is the point of showing it. The user learns to read the number on
cheap queries so the expensive one is legible when it appears.

### 5.1 Refusal thresholds

| Threshold | Default | Behaviour |
| --- | --- | --- |
| `WARN_READ_BYTES` | 64 MiB | Plan returns with a warning; execution needs explicit confirmation |
| `MAX_READ_BYTES` | 4 GiB | Refused with an explanation and a suggested narrower slice |
| `MAX_GPU_BYTES` | per [`CUDA_ARCHITECTURE.md`](CUDA_ARCHITECTURE.md) §3 | Refused before any launch |
| Whole-tensor read | — | **Always refused**, at any size (`whole_tensor_reads_are_refused_with_an_explanation`) |

The last one is categorical rather than numeric: a whole-tensor read is a
category error in a system whose premise is that checkpoints do not fit in
memory. Users are directed to a slice or a statistic.

---

## 6. Execution tiers

`WQL-013`, task `QM-0073`. The planner selects, records the choice in the plan,
and the response reports which tier ran.

| Tier | When | Fidelity | State |
| --- | --- | --- | --- |
| **Metadata** | Shape, dtype, address, byte range | `metadata` | ✓ |
| **Catalog** | Pre-computed statistics already in `tensor_statistics` | `aggregate` / `sampled` | Blocked on `QM-0020` |
| **Exact read** | Scalar or bounded slice | `exact` | ✓ (`WQL-005`) |
| **CPU block compute** | Matmul, reductions, comparisons on bounded blocks | `exact` | `QM-0070` |
| **GPU block compute** | Same, when a verified backend exists and the block is large enough to pay for the transfer | `exact` | Lane E |
| **Sampled** | Explicitly requested approximation over a large region | `sampled` | Extension point |

Selection rules: prefer the cheapest tier that meets the requested fidelity;
never silently downgrade fidelity to save cost — refuse and explain instead; a
GPU tier requires a *verified* backend, so an unverified CUDA build never
silently becomes the executor.

---

## 7. Query plan

`schemas/weightql/schema.json`, 166 lines, already fixes the shape.

```jsonc
{
  "plan_id": "plan:b3:…",              // deterministic; quotable (WQL-012)
  "status": "planned",                  // planned | running | complete | failed | cancelled
  "statements": [ /* resolved AST */ ],
  "references": [
    { "input": "Q[10]",
      "canonical": "model.layers[10].self_attention.query_projection.weight",
      "tensor_id": "…", "shape": [128,48], "dtype": "F32",
      "byte_ranges": [[419928, 419932]], "confidence": 1.0 }
  ],
  "result_shape": [256, 256],
  "estimated_read_bytes": 524288,
  "estimated_gpu_bytes": 786432,
  "execution_tier": "cpu-block",
  "fidelity": "exact",
  "warnings": [],
  "requires_confirmation": false
}
```

`plan_id` is a deterministic hash of the resolved plan, so the same query yields
the same ID — which is what makes it quotable in a chat response, a log line, and
a cache key.

---

## 8. Security boundaries

| Boundary | Mechanism | Evidence |
| --- | --- | --- |
| No arbitrary code execution | Closed `Expr` enum; no `eval` production | `arbitrary_code_execution_constructs_are_rejected` |
| Unknown function named against a closed set | Parser lists the legal set in the error | `unknown_function_error_names_the_closed_function_set` |
| No raw SQL from user input | Catalog binds every caller value; only enum-derived `&'static str` is interpolated | `SEC-005` |
| No file path from user input | References resolve through the catalog to a `shard_uri` inside a configured root | `SEC-001` |
| No unbounded resource use | Cost thresholds §5.1; whole-tensor reads categorically refused | `WQL-011` |
| Browser parser matches Rust | `apps/web/query-interface/src/weightql.ts`, 17 tests | `CHAT-002`, `SEC-003` |

**Why two parsers.** The browser parses for immediate feedback — a caret under
the offending character (`CHAT-004`) and a KaTeX preview — while Rust remains
authoritative for execution. The risk is divergence, mitigated by
`ADR-005` (hand-written parsers on both sides, same grammar) and by a shared
conformance corpus that both suites run (`QM-0074`). The alternative, a WASM
build of the Rust parser, is `ADR-CANDIDATE-012` and is not the MVP default:
it would add a build step and a payload for a parser that is 640 lines.

---

## 9. Result schema

```jsonc
{
  "plan_id": "plan:b3:…",
  "status": "complete",
  "fidelity": "exact",
  "result": {
    "kind": "scalar" | "slice" | "matrix" | "statistics" | "comparison",
    "shape": [256, 256],
    "dtype": "F32",
    "values": "…",                    // omitted when the client asked for a reference
    "qtile_uri": "…"                  // large results go to a tile, not into JSON
  },
  "provenance": {
    "tensors": [ /* canonical addresses + byte ranges actually read */ ],
    "bytes_read": 524288,
    "backend": "cpu-reference",
    "algorithm_version": 1,
    "elapsed_ms": 12
  }
}
```

Two rules:

* **`fidelity` is mandatory.** A result without it cannot be rendered, because
  the UI could not label it and `AC-010` would be unsatisfiable.
* **`provenance.tensors` lists what was actually read**, not what was planned. A
  cache hit reads nothing, and the response says so.

---

## 10. Requirements

| ID | Requirement | State | Task |
| --- | --- | --- | --- |
| `WQL-001`…`WQL-005` | Lexer, parser, resolution, shape check, scalar/slice execution | ✓ Verified | verify only |
| `WQL-006` | Matrix-multiplication execution | Stub | `QM-0070` |
| `WQL-007` | Statistical `SELECT … GROUP BY` | Not started | `QM-0072` |
| `WQL-008` | Stacked slice composition | Stub | `QM-0071` |
| `WQL-009`…`WQL-012` | No arbitrary execution; cost; whole-tensor refusal; plan IDs | ✓ Verified | verify only |
| `WQL-013` | Execution tier selection recorded in the plan | New | `QM-0073` |
