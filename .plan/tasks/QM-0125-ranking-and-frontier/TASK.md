# QM-0125 — Fragility ranking and the mixed-precision frontier

## Status

Blocked

Unblocks when `QM-0123` reaches `Complete`.

## Phase

Phase 11 — Quantisation-error diagnostic engine

## Objective

Turn the measurements into a decision: which tensors are most fragile, and what
it costs in bytes to keep each of them at higher precision. This is the task that
moves the product from Level 2 to Level 3 on the strategy's value ladder.

## Repository Evidence

* `QM-0123` — `RunDiagnostic`, exact aggregates with `bytes_at_base_precision`
  and `bytes_at_target_precision` at every level.
* `crates/q-nsir/src/address.rs` — canonical addresses, stable and orderable, so
  ties break deterministically.
* `.plan/DIAGNOSTIC_ARCHITECTURE.md` §7 — the frontier algorithm.

## Requirements Covered

`QUANT-006`, `V1-20`.

## Dependencies

`QM-0123`.

## Blocks

`QM-0141`, `QM-0150`, `QM-0161`.

## Parallelization

Lane Q, parallel with `QM-0124`.

## Program Boundary

`crates/q-diagnostics`.

## Scope

* A total, deterministic ranking of tensors and of layers by relative error.
* The mixed-precision frontier: for each cumulative keep-set, bytes added and
  fraction of total squared error removed.
* A budget query: given `X` extra bytes, which keep-set fits.
* Explicit, printed statements of what the frontier is and is not.

## Out of Scope

Applying the recommendation · emitting a quantised model · any accuracy claim
(`EVAL-001`) · a solver for the exact integer problem.

## The algorithm

```text
for each tensor t:
    Δerror(t) = sum_sq_delta(t)                          # error removed by keeping t at base precision
    Δbytes(t) = bytes_at_base(t) − bytes_at_target(t)    # cost of keeping it

density(t) = Δerror(t) / Δbytes(t)
rank by density, descending
ties: parameter count descending, then canonical address ascending   # total order

frontier[k] = ( Σ_{i≤k} Δbytes,  Σ_{i≤k} Δerror / total_sum_sq_delta )
```

Greedy over a density ratio is the standard fractional-knapsack heuristic. For
the integer problem it is **not** proven optimal, and the report says so in one
line rather than implying otherwise.

The output is a statement of the form:

> Keeping layers 0, 1, and 27 at 8-bit costs **+0.82 GB** and removes **46 %** of
> total weight-space squared error.

Both numbers are computed from measurements. Neither is an accuracy prediction.

## Files Expected to Change

* `crates/q-diagnostics/src/lib.rs`

## Files Expected to Add

* `crates/q-diagnostics/src/frontier.rs`

## Data Contracts

```rust
pub struct RankedEntry {
    pub address: String,
    pub relative_error: f64,
    pub sum_sq_delta: f64,
    pub parameter_count: u64,
    pub delta_bytes: u64,
    pub density: f64,
}

pub struct FrontierPoint {
    pub keep_set: Vec<String>,          // canonical addresses, in keep order
    pub added_bytes: u64,
    pub error_removed_fraction: f64,    // in [0, 1]
    pub cumulative_relative_error: f64, // of the model, after keeping this set
}

pub struct Frontier {
    pub points: Vec<FrontierPoint>,
    pub method: &'static str,           // "greedy by error-per-byte; not proven optimal"
    pub granularity: FrontierGranularity, // PerTensor | PerLayer
}

impl Frontier {
    /// Largest keep-set fitting a byte budget.
    pub fn at_budget(&self, added_bytes: u64) -> Option<&FrontierPoint>;
}
```

The `method` string is part of the data contract, not a comment: it travels into
the manifest, the report, and any downstream consumer, so the caveat cannot be
lost in rendering.

## Memory and Performance Constraints

`O(tensors log tensors)` for the sort; nothing touches weight bytes. Even at
`CAT-006`'s 47 278 tensors this is milliseconds.

Frontier points are emitted at **layer granularity by default** — a keep-set of
individual tensors is rarely actionable, since engineers configure precision per
layer or per module. Per-tensor granularity is available and reported second.

## Implementation Plan

1. Compute `Δerror`, `Δbytes`, and `density` per tensor and per layer.
2. Sort by density with the documented tie-break, producing a total order.
3. Walk the sorted list accumulating the frontier.
4. `at_budget` by binary search over cumulative bytes.
5. Handle `Δbytes == 0` (a tensor already at target precision, or excluded from
   quantisation): density is undefined — exclude it from the ranking and list it
   separately as "not quantised", never sort it to the top with an infinity.
6. Emit the method string and the granularity with the frontier.

## Error Handling

| Case | Behaviour |
| --- | --- |
| `Δbytes == 0` | Excluded from ranking; listed as "not quantised by this config" |
| `total_sum_sq_delta == 0` (config is lossless for this model) | Frontier is empty; report states the config introduced no measurable error |
| Fewer tensors than requested frontier points | Emit what exists |
| Two entries identical on every tie-break field | Impossible — canonical address is unique (`SRC-006`); assert it |

## Acceptance Criteria

1. The ranking is a **total order**: two runs produce identical sequences,
   including ties.
2. Frontier fractions are monotonically non-decreasing and end at 1.0 when every
   tensor is kept.
3. `at_budget` returns the largest keep-set within budget, verified against
   brute force on a small fixture.
4. On a hand-computed 5-tensor fixture, every frontier point matches by hand.
5. `Δbytes == 0` entries are excluded from the ranking and listed separately —
   asserted, because sorting an infinity to the top would be a visible wrong
   answer in the report's first table.
6. The lossless-config case produces an empty frontier and a clear statement,
   not a division by zero.
7. The `method` string appears in the output and states that greedy is not proven
   optimal.
8. Layer-granularity frontier points are emitted by default.

## Verification Plan

**Automated** — hand-computed fixture; brute-force comparison on ≤ 10 tensors;
determinism; the edge cases.

## Suggested Commands

```bash
cargo test -p q-diagnostics frontier
```

## Test Cases

| Input | Expected |
| --- | --- |
| 5 tensors, hand-computed densities | Ranking and every frontier point match by hand |
| Brute force over 8 tensors | Greedy result within the documented gap; `at_budget` correct |
| Two tensors with equal density | Tie broken by parameter count, then address; stable |
| One tensor with `Δbytes == 0` | Excluded from ranking; listed separately |
| Config that introduces zero error | Empty frontier; explanatory statement |
| Budget below the smallest `Δbytes` | `at_budget` returns the empty point, not `None`-as-error |
| Two runs | Identical output |

## Risks

| Risk | Mitigation |
| --- | --- |
| Greedy presented as optimal | The `method` string is in the data contract and audited by `QM-0090` |
| A partner reads "46 % of error removed" as "46 % of accuracy recovered" | The report's caveat section and the `V1-22` string audit |
| `Δbytes == 0` sorts an infinity to the top | Acceptance criterion 5 |
| Per-tensor keep-sets are unactionable | Layer granularity is the default; per-tensor is secondary |

## Completion Evidence

* The hand-computed 5-tensor fixture with expected and actual output.
* Brute-force comparison results.
* An example frontier rendered as it will appear in the report.
* Determinism check across two runs.
