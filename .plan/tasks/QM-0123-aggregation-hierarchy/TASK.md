# QM-0123 — Aggregation: channel → tensor → module → layer → expert → model

## Status

Blocked

Unblocks when `QM-0122` reaches `Complete`.

## Phase

Phase 11 — Quantisation-error diagnostic engine

## Objective

Roll tensor-level partials up the canonical address hierarchy without losing
exactness, so that "layer 27 is fragile" is a computed statement rather than an
average of averages.

## Repository Evidence

* `crates/q-nsir/src/address.rs`, `resolver.rs` — canonical addresses,
  `canonical_names_are_stable_across_resolution_runs` (`NSIR-004`).
* `crates/q-nsir/src/resolver.rs` — `llama_resolves_moe_expert_tensors`
  (`NSIR-003`). Expert-keyed aggregation is free because this exists.
* `crates/q-catalog/src/lib.rs` — `hierarchy_browse_returns_one_summary_per_layer`
  (`CAT-003`), the hierarchy this mirrors.
* `QM-0122` — `TensorDiagnostic` with derived metrics as methods.

## Requirements Covered

`QUANT-004`, `V1-12`.

## Dependencies

`QM-0122`.

## Blocks

`QM-0124`, `QM-0125`, `QM-0140`, `QM-0141`.

## Parallelization

Lane Q, sequential after `QM-0122` (same crate).

## Program Boundary

`crates/q-diagnostics`.

## Scope

* An aggregation tree keyed by canonical address: model → layer → module →
  tensor, plus an expert axis where the resolver found one.
* Exact composition of additive partials; maxima by maximum.
* Derived metrics computed at read time at every level.
* Handling of tensors the resolver left `unknown`.

## Out of Scope

Ranking and the frontier (`QM-0125`) · outlier attribution (`QM-0124`) ·
serialization (`QM-0140`) · persistence (`QM-0020`).

## Files Expected to Change

* `crates/q-diagnostics/src/lib.rs`

## Files Expected to Add

* `crates/q-diagnostics/src/aggregate.rs`

## Data Contracts

```rust
pub struct Aggregate {
    pub count: u64,
    pub sum_sq_base: f64,
    pub sum_sq_delta: f64,
    pub sum_abs_delta: f64,
    pub max_abs_delta: f64,
    pub max_abs_base: f64,
    pub bytes_at_base_precision: u64,
    pub bytes_at_target_precision: u64,
    pub tensor_count: u64,
}

impl Aggregate {
    pub fn rmse(&self) -> f64;
    pub fn relative_error(&self) -> f64;
    pub fn merge(&mut self, other: &Aggregate);   // exact; additive + max
}

pub struct RunDiagnostic {
    pub model: Aggregate,
    pub layers:  BTreeMap<u32, Aggregate>,
    pub modules: BTreeMap<(u32, Component), Aggregate>,
    pub experts: BTreeMap<(u32, u32), Aggregate>,   // (layer, expert), where present
    pub tensors: Vec<TensorDiagnostic>,             // canonical-address order
    pub unresolved: Vec<TensorId>,                  // resolver returned unknown
}
```

`BTreeMap`, not `HashMap`, throughout — iteration order is part of the
determinism contract (`V1-13`, `V1-18`).

## Memory and Performance Constraints

`O(tensors + layers + experts)` — a few MB even for a 47 000-tensor manifest
(`CAT-006`'s scale). Aggregation touches no weight bytes; it is arithmetic over
already-computed partials.

## Implementation Plan

1. `Aggregate::merge` — additive fields sum, `max_*` fields take the maximum,
   counts sum. No division anywhere in the merge path.
2. Walk `TensorDiagnostic`s in canonical-address order, merging into each
   ancestor.
3. Key the expert axis from the resolved address (`NSIR-003`); absent experts
   yield an empty map, not zeros.
4. Collect `unresolved` separately: a tensor the resolver could not place is
   included in the model total and excluded from layer/module breakdowns, and
   the report says how many.
5. Derived metrics as methods at every level.

## Error Handling

| Case | Behaviour |
| --- | --- |
| Tensor with no resolved layer | Counted in the model total; listed in `unresolved`; never assigned to a guessed layer |
| Duplicate tensor id | Refuse — the catalog already rejects duplicates (`SRC-012`); a duplicate here means a bug upstream |
| `sum_sq_base == 0` at some level (all-zero weights) | `relative_error` returns 0 with a documented convention, never NaN |
| Empty aggregate | Refuse to report derived metrics; there is nothing to divide |

The `unknown` handling is not a detail. `NSIR-001` exists precisely so that
unresolved names stay unresolved, and an aggregation that quietly bucketed them
into "layer 0" would violate the repository's oldest rule.

## Acceptance Criteria

1. Model-level `sum_sq_delta` equals the direct sum over all tensors — asserted,
   not assumed.
2. Layer aggregates equal the direct sum over that layer's tensors.
3. `relative_error` at every level equals `sqrt(Σδ² / Σw²)` computed directly
   from the tensor partials — **not** the mean of tensor-level relative errors.
   A test constructs a case where the two differ and asserts the correct one.
4. Expert aggregates are present for an MoE fixture and absent (not zero-filled)
   for a dense one.
5. Unresolved tensors appear in `unresolved` and in the model total, and in no
   layer bucket.
6. Iteration order is deterministic across runs.
7. All-zero weights give `relative_error == 0`, never NaN.

Criterion 3 is the one that catches the most likely bug in the whole engine.

## Verification Plan

**Automated** — direct-computation equality at every level; the mean-of-ratios
counterexample; determinism.

## Suggested Commands

```bash
cargo test -p q-diagnostics aggregate
cargo test -p q-diagnostics                 # whole crate
```

## Test Cases

| Input | Expected |
| --- | --- |
| Three tensors, hand-computed | Model aggregate matches by hand |
| Two tensors with very different `sum_sq_base` | Aggregate relative error ≠ mean of the two; the correct one is asserted |
| MoE fixture | Expert map populated with correct keys |
| Dense fixture | Expert map empty, not zero-filled |
| One unresolved tensor | In `unresolved`; in the model total; in no layer |
| All-zero tensor | `relative_error == 0`, no NaN |
| Two runs | Identical iteration order and values |

## Risks

| Risk | Mitigation |
| --- | --- |
| Averaging ratios instead of composing sums | Acceptance criterion 3, with a deliberate counterexample |
| `HashMap` ordering leaks into the report | `BTreeMap` in the contract; the determinism test guards it |
| Unresolved tensors quietly bucketed | Acceptance criterion 5; `NSIR-001`'s rule |
| Expert keys collide across layers | The key is `(layer, expert)`, tested on a fixture with the same expert index in two layers |

## Completion Evidence

* Test output with counts.
* The hand computation for the three-tensor case.
* The mean-of-ratios counterexample, both values printed, showing which is
  reported.
* Confirmation that a dense model produces an empty expert map.
