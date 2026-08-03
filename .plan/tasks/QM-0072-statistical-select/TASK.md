# QM-0072 — Statistical `SELECT … GROUP BY layer_index`

## Status

Blocked

Unblocks when `QM-0071`, `QM-0020` reaches `Complete`.

## Phase

Phase 07 — WeightQL and chat

## Objective

Answer *"show the L2 norm of every query projection"* — the one §21 example that
needs a language feature that does not exist.

## Repository Evidence

* `STATUS.md` `WQL-007` — **Not Started**. The parser *"rejects it by name with
  this ID"* (`unsupported_select_target_is_named_with_its_requirement`).
* `ARCHITECTURE.md` §7.3 gives the target form: `SELECT layer_index,
  mean(weight), stddev(weight), l2_norm(weight) FROM model("…").tensors WHERE
  role = "attention_query_projection" GROUP BY layer_index`.
* `q-catalog` — `role_and_layer_filters_drive_alias_resolution`,
  `shape_dtype_and_resolution_filters_work` (`CAT-005` Verified).
* `QM-0020` persists statistics; `QM-0031` computes them per tensor.
* `q_expression::Reduction` — the reduction enum already exists.

## Requirements Covered

`WQL-007`.

## Dependencies

`QM-0071`, `QM-0020`, `QM-0013`.

## Blocks

`QM-0074`.

## Parallelization

**Sequential after `QM-0071`** — shared file `plan.rs`.

## Program Boundary

`crates/q-weightql`, `crates/q-catalog`.

## Scope

* Parse the `SELECT … FROM model(…).tensors WHERE … GROUP BY …` form.
* Group by `layer_index`, `role`, or `component`.
* Aggregates from **persisted statistics**, not from re-reading weights.
* `WHERE` over the catalog's five existing filter kinds.
* Report which tensors had no statistics, rather than treating them as absent.

## Out of Scope

Arbitrary SQL · joins · subqueries · `HAVING` · computing statistics on demand
inside a query, which would make a cheap-looking query arbitrarily expensive.

## Files Expected to Change

* `crates/q-weightql/src/parser.rs`
* `crates/q-weightql/src/plan.rs`
* `crates/q-catalog/src/lib.rs`

## Files Expected to Add

* `crates/q-weightql/tests/statistical_select.rs`

## Files Expected to Remove or Deprecate

* `unsupported_select_target_is_named_with_its_requirement` — **narrowed**, not
  deleted: other unsupported targets must still be named by requirement.

## Data Contracts

```jsonc
{ "kind": "statistics",
  "group_by": "layer_index",
  "rows": [ { "layer_index": 0, "l2_norm": 7.91, "mean": 0.0001, "count": 16777216,
              "tensor_count": 1, "fidelity": "aggregate" } ],
  "missing_statistics": [ { "canonical_address": "…", "reason": "not computed" } ],
  "fidelity": "aggregate" }
```

**`missing_statistics` is required.** A group whose tensors have no statistics
must not silently produce a smaller number — that is a plausible wrong answer,
and the user cannot detect it.

## Memory and Performance Constraints

* Aggregation is a **catalog query over one row per tensor**, indexed by
  `(model_id, role, layer_index)`. No weight is read.
* Budget: **< 1 s on the 47 278-tensor synthetic manifest** — the threshold at
  which `ADR-CANDIDATE-005` reopens the DuckDB question.
* Result rows are bounded by the group count, which is bounded by layer count.

## Implementation Plan

1. Extend the parser for the `SELECT` form; keep the closed function set.
2. Map `WHERE` clauses onto the catalog's existing filters.
3. Aggregate persisted statistics in SQL, grouped as requested.
4. Collect tensors matching `WHERE` but lacking statistics into
   `missing_statistics`.
5. Assert the query reads no payload.
6. Benchmark on the 47 278-tensor manifest.

## Error Handling

* An unknown aggregate function → error naming the closed set (existing idiom).
* An unknown `GROUP BY` column → error naming the supported ones.
* No tensors matching `WHERE` → an empty result with a note, not an error.
* No statistics for any match → empty rows, **full `missing_statistics`**, and an
  explicit message telling the user to run a conversion.
* An aggregate over mixed `algorithm_version` rows → refuse; comparing across
  algorithm versions is not meaningful.

## Acceptance Criteria

1. The `ARCHITECTURE.md` §7.3 query parses and executes.
2. `GROUP BY layer_index` returns one row per layer.
3. `GROUP BY role` and `GROUP BY component` work.
4. Aggregates match a direct computation over the persisted rows.
5. Tensors without statistics appear in `missing_statistics`, **never silently
   omitted**.
6. The query reads **no** weight bytes — asserted.
7. Under 1 s on the 47 278-tensor manifest.
8. Mixed `algorithm_version` rows are refused.
9. Other unsupported `SELECT` targets are still named by requirement ID.

## Verification Plan

**Automated** — `statistical_select.rs` including a no-payload-read assertion and
the timing benchmark.
**Manual** — run the §21 example through the CLI and read the output.

## Suggested Commands

```bash
cargo test -p q-weightql -p q-catalog                        # verified today
cargo run -p q-cli -- query fixtures/tiny-llama-2shard \
  'SELECT layer_index, l2_norm(weight) FROM model("…").tensors
   WHERE role = "attention_query_projection" GROUP BY layer_index'   # new
```

## Test Cases

| Input | Expected |
| --- | --- |
| The §7.3 query | Parses and executes |
| `GROUP BY layer_index`, 12 layers | 12 rows |
| `GROUP BY role` | One row per role |
| Aggregate values | Match direct computation over persisted rows |
| One tensor lacking statistics | Appears in `missing_statistics` |
| No matching tensors | Empty result with a note, not an error |
| Payload read counter | Zero |
| 47 278-tensor manifest | < 1 s |
| Mixed `algorithm_version` | Refused |
| `SELECT nonsense FROM …` | Named by requirement ID |

## Risks

| Risk | Mitigation |
| --- | --- |
| A group silently omits tensors without statistics | `missing_statistics` is required in the contract and asserted |
| SQLite too slow at scale | Benchmarked; > 1 s reopens `ADR-CANDIDATE-005` |
| The parser addition opens a general SQL surface | The function set stays closed; `WQL-009` still passes |

## Completion Evidence

* Query output for the §7.3 example.
* Aggregate-versus-direct-computation comparison.
* A `missing_statistics` demonstration.
* Timing on the 47 278-tensor manifest.
* The zero-payload-read assertion.
