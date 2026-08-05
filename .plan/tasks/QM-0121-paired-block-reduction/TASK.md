# QM-0121 — Paired block reduction in `q_gpu::Backend`

## Status

In Progress

Unblock condition **met**: `QM-0120` reached `Complete` (merged `539c41c`, floor
raised to 598/49, later 677/51). Dispatched by the controller on the critical path
— `QM-0122` behind this task carries gate **G2**.

## Phase

Phase 11 — Quantisation-error diagnostic engine

## Objective

Extend the compute backend with a **paired** reduction: a base block and a
counterpart block in, whole-block and per-output-channel partials out. This is
the kernel the entire product is built on.

## Repository Evidence

* `crates/q-gpu/src/lib.rs:73` — `pub trait Backend`, today single-tensor:
  `block_statistics(&self, source, descriptor, extent, histogram_bins)`.
* `crates/q-gpu/src/lib.rs:105` — `BlockData::new(rows, columns, values)`.
* `crates/q-gpu/src/lib.rs:89` — `check_workload`, the existing ceiling-refusal
  pattern.
* `q_gpu::CpuBackend` — `GPU-002 Verified`, 7 tests; the numerical reference.
* `crates/q-statistics/src/lib.rs` — `relative_l2`, hand-verified, and the
  Welford accumulator whose numerical-stability discipline this follows.

## Requirements Covered

`QUANT-002`, `V1-11`. Opens the `DIFF-001` seam at no extra cost.

## Dependencies

`QM-0120`.

## Blocks

`QM-0122`, `QM-0126`.

## Parallelization

Lane Q. Edits `crates/q-gpu/src/lib.rs`, which `QM-0126` also touches — sequential
with it.

## Program Boundary

`crates/q-gpu` (trait and CPU implementation).

## Scope

* `paired_block_reduction` on the `Backend` trait.
* `PairedPartials` and `ChannelPartials` types.
* A CPU implementation that is the numerical ground truth.
* Shape and axis validation, refusing before any arithmetic.

## Out of Scope

Quantisation itself (`QM-0120`) · streaming (`QM-0122`) · aggregation
(`QM-0123`) · the Metal implementation (`QM-0126`) · reading blocks from disk.

## Design note — build it generically

The counterpart is **any** second block, not "this block's own quantisation". The
signature must not mention quantisation. With that, checkpoint-diff forensics
(`DIFF-001` — the strategy's third module) is the same kernel with a different
second operand, and MoE expert-pair comparison (`MOE-001`) is the same kernel
again. Hard-coding the quantisation case here would cost two later modules weeks
each, and generalising costs nothing now.

## Files Expected to Change

* `crates/q-gpu/src/lib.rs` — the trait, the types, the CPU implementation

## Files Expected to Add

* `crates/q-gpu/src/paired.rs` — if the implementation warrants its own module

## Data Contracts

```rust
pub enum ChannelAxis { Rows, Columns }

pub struct ChannelPartials {
    pub count: u64,
    pub sum_sq_base: f64,
    pub sum_sq_delta: f64,
    pub sum_abs_delta: f64,
    pub max_abs_delta: f64,
    pub max_abs_base: f64,
}

pub struct PairedPartials {
    pub count: u64,
    pub sum_sq_base: f64,
    pub sum_sq_delta: f64,
    pub sum_abs_delta: f64,
    pub max_abs_delta: f64,
    pub max_abs_base: f64,
    pub per_channel: Vec<ChannelPartials>,
}

pub trait Backend {
    // ... existing methods unchanged ...
    fn paired_block_reduction(
        &self,
        base: &BlockData,
        counterpart: &BlockData,
        axis: ChannelAxis,
    ) -> Result<PairedPartials>;
}
```

**Everything is a partial; nothing is a finished metric.** No RMSE, no relative
error, no norm — those are derived once, at the top of the aggregation, because
they do not compose. A block-level RMSE that later gets averaged is the single
most likely correctness bug in this engine.

`f64` accumulators over `f32` inputs, throughout. The inputs are f32; the sums
are not.

## Memory and Performance Constraints

```text
allocation = per_channel.len() × size_of::<ChannelPartials>()
           = channels × 48 B
```

For a 256-column block, ~12 KB. Nothing proportional to tensor size. The two
input blocks are borrowed, never copied.

Single-threaded accumulation within a block, in a fixed element order. Parallel
reduction with floating-point addition is not associative, and `V1-13` requires
byte-identical output across runs.

## Implementation Plan

1. Define `ChannelAxis`, `ChannelPartials`, `PairedPartials`.
2. Add the trait method with a default that refuses, naming `QUANT-002`, so
   existing backends compile unchanged.
3. Implement it on `CpuBackend`: single pass, fixed order, f64 accumulators.
4. Validate first: identical shapes, non-empty, axis within rank. Refuse before
   arithmetic.
5. Extend `check_workload` so the paired case accounts for two blocks.
6. Tests: hand-computed, deliberately asymmetric.

## Error Handling

* Shape mismatch between base and counterpart → refuse, naming both shapes,
  before reading any value.
* Empty block → refuse; an empty reduction has no meaningful partials.
* Non-finite value in either block → refuse, naming the position. `QM-0120`
  refuses NaN at the source; this is defence in depth.
* Axis out of range → refuse naming the rank.
* Workload exceeding backend capacity → the existing `BudgetExceeded` path,
  counting both blocks.

## Acceptance Criteria

1. Hand-computed 3×4 case: every field of `PairedPartials` and every
   `ChannelPartials` matches values computed by hand in the test.
2. **Orientation:** a non-square, deliberately asymmetric fixture where reducing
   over the wrong axis gives a different answer, and the test would fail if it
   did.
3. Partials compose: reducing a block in two halves and summing equals reducing
   it whole, for every additive field; `max_*` composes by maximum.
4. Two runs are bit-identical.
5. Shape mismatch, empty block, non-finite value, and bad axis all refuse before
   arithmetic, each naming the reason.
6. Allocation is proportional to channel count, not element count.
7. The signature mentions neither quantisation nor any specific second-operand
   provenance.

## Verification Plan

**Automated** — unit tests with hand-computed expectations; a composition
property test; a determinism test.
**Manual** — none required.

## Suggested Commands

```bash
cargo test -p q-gpu paired
cargo test -p q-gpu            # the 7 existing tests must still pass
```

## Test Cases

| Input | Expected |
| --- | --- |
| 3×4 base and counterpart, hand-computed | Every field matches |
| Identical base and counterpart | All delta fields exactly 0; `sum_sq_base` unchanged |
| Counterpart all zeros | `sum_sq_delta == sum_sq_base` |
| Asymmetric 2×5, axis Rows vs. Columns | Different, correct answers; orientation proven |
| Split into halves, summed | Equals the whole-block reduction |
| Shape mismatch | Refused, both shapes named |
| NaN in the counterpart | Refused, position named |
| Empty block | Refused |
| Two identical runs | Bit-identical |

## Risks

| Risk | Mitigation |
| --- | --- |
| Per-channel axis silently transposed | The asymmetric orientation test is an acceptance criterion, not an optional extra |
| f32 accumulation loses precision on large blocks | f64 accumulators throughout; the composition test would expose drift |
| Someone adds a finished metric to `PairedPartials` for convenience | Documented in the type: partials only. The aggregation owns derived metrics |
| The trait is specialised to quantisation | Acceptance criterion 7 |

## Completion Evidence

* `cargo test -p q-gpu` output including the pre-existing 7 tests.
* The hand computation for the 3×4 case, written out in the evidence.
* The asymmetric orientation fixture and both axis results.
* Confirmation that the signature is provenance-neutral.
