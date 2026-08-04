# QM-0070 — Matrix-multiplication execution

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 07 — WeightQL and chat

## Objective

Execute a matrix expression that today only plans. `WQL-006` is a stub that
*"parses, resolves, shape-checks, and estimates I/O — then stops, and says why."*

## Repository Evidence

* `STATUS.md` `WQL-006` — **Stub**; *"no compute backend exists, and the plan
  says so."*
* `crates/q-weightql/src/plan.rs` (673) — resolution, shape check, cost estimate,
  plan IDs. `explicit_transpose_makes_the_expression_type_check` passes.
* `q_expression::Expr::needs_backend()` — already distinguishes a pure read from
  an expression needing compute.
* `q_gpu::Backend` and `CpuBackend` matmul — `MATMUL-004` Verified.
* `q_tensor_runtime::TensorBlock::plan` — byte-range planning.
* `WQL-004` — shape mismatch rejected before execution, by construction.

## Requirements Covered

`WQL-006`, `MVP-28`, `MVP-36`.

## Dependencies

`QM-0031`, `QM-0037`, `QM-0032`.

## Blocks

`QM-0073`, `QM-0080`.

## Parallelization

**Sequential with `QM-0071` and `QM-0072`** — all three edit
`crates/q-weightql/src/plan.rs`. This one first.

## Program Boundary

`crates/q-weightql`, `crates/q-gpu`.

## Scope

* Execute `MatMul`, `Transpose`, `Add`, `Sub`, `Reduce`, and `Compare` over
  bounded blocks, through `q_gpu::Backend`.
* Materialize intermediates only when needed; `(A @ B) @ C` may keep the
  intermediate as an expression until required.
* Enforce cost thresholds **before** reading.
* Return the result with fidelity, provenance, and the backend that ran it.

## Out of Scope

Stacked slices (`QM-0071`) · statistical `SELECT` (`QM-0072`) · cancellation
(`QM-0073`) · CUDA execution, which arrives behind the same trait.

## Files Expected to Change

* `crates/q-weightql/src/plan.rs`
* `crates/q-gpu/src/lib.rs`
* `crates/q-daemon/src/lib.rs`

## Files Expected to Add

* `crates/q-weightql/src/execute.rs`
* `crates/q-weightql/tests/execution.rs`

## Files Expected to Remove or Deprecate

None. The **refusal for unsupported expressions stays** — the closed `Expr` enum
means anything new is an explicit addition.

## Data Contracts

Result per [`WEIGHTQL_ARCHITECTURE.md`](../../WEIGHTQL_ARCHITECTURE.md) §9:
`plan_id`, `status`, `fidelity`, `result{kind, shape, dtype, values | qtile_uri}`,
`provenance{tensors, bytes_read, backend, algorithm_version, elapsed_ms}`.

**Results above `MAX_JSON_RESULT_ELEMENTS = 4 096` are returned as a `.qtile`,
not inline JSON** — a 256×256 result is 65 536 floats, which is not a JSON
payload.

## Memory and Performance Constraints

* Operands are read as bounded blocks; **a whole tensor is never materialized**.
* Working set for `A@B`: `(m×k + k×n + m×n) × 4`. At 256³ that is 768 KiB.
* `WARN_READ_BYTES = 64 MiB` requires confirmation; `MAX_READ_BYTES = 4 GiB`
  refuses; a whole-tensor read is refused **categorically**.

## Implementation Plan

1. `execute.rs`: walk the AST bottom-up, resolving `TensorRef` to a bounded read.
2. `Transpose` as an index remap, not a copy, where the consumer allows it.
3. `MatMul` via `Backend::matmul` on the selected backend.
4. Threshold checks before each read.
5. Large results written to a `.qtile` and returned by URI.
6. `POST /v1/query` with `mode: "execute"` requiring a matching `plan_id`.
7. Tests including a comparison against a hand-computed result.

## Error Handling

* Shape mismatch → **already rejected at planning**, before any read.
* Cost above `MAX_READ_BYTES` → 413 naming the threshold.
* Execute without a matching `plan_id` → 400. **A cost the user never saw cannot
  be paid.**
* Backend failure → the error propagates with the operation and operand shapes.
* An unsupported `Expr` variant → `NotImplemented` naming it.

## Acceptance Criteria

1. `show tensor("Q[10]")[0:256,0:256] @ transpose(tensor("K[10]")[0:256,0:256])`
   executes and matches the CPU reference to `1e-5`.
2. `(A @ B) @ C` executes without materializing the intermediate as a tensor.
3. A shape mismatch is still rejected before any read.
4. Results over 4 096 elements return a `.qtile` URI.
5. A cost above `MAX_READ_BYTES` returns 413 naming the threshold.
6. `mode: "execute"` without a matching `plan_id` returns 400.
7. Every result carries fidelity, provenance, and backend.
8. A whole-tensor read is still refused.
9. `WQL-001`…`WQL-005` and `WQL-009`…`WQL-012` still pass.

## Verification Plan

**Automated** — `execution.rs` comparing against `CpuBackend` and against a
hand-computed 3×3 case; daemon tests for the thresholds.
**Manual** — `q-cli query` with an expression; compare against numpy.

## Suggested Commands

```bash
cargo test -p q-weightql -p q-gpu                                        # verified today
cargo run -p q-cli -- query fixtures/tiny-llama-2shard \
  'show tensor("Q[10]") @ transpose(tensor("K[10]"))'                    # plans today
cargo run -p q-cli -- query … --execute                                   # introduced here
```

## Test Cases

| Input | Expected |
| --- | --- |
| `Q[10][0:256,:] @ transpose(K[10][0:256,:])` | Matches CPU to `1e-5` |
| Hand-computed 3×3 @ 3×3 | Exact match |
| `(A @ B) @ C` | Executes; intermediate not materialized as a tensor |
| `2×3 @ 2×2` | Rejected at planning; no read |
| 256×256 result | Returned as a `.qtile` URI |
| Cost 8 GiB | 413 naming `MAX_READ_BYTES` |
| Execute with a stale `plan_id` | 400 |
| Whole-tensor read | Refused |
| Result | Carries fidelity, provenance, backend |

## Risks

| Risk | Mitigation |
| --- | --- |
| Three tasks editing `plan.rs` | Strict sequence: `QM-0070` → `QM-0071` → `QM-0072` |
| An intermediate is silently materialized at full size | Asserted by a peak-allocation check |
| A large result is returned inline and overwhelms the browser | 4 096-element cap; `.qtile` above it |

## Completion Evidence

* Comparison against the CPU reference and the hand-computed case.
* Peak allocation during `(A @ B) @ C`.
* Threshold-refusal outputs.
* A full result payload showing fidelity and provenance.
