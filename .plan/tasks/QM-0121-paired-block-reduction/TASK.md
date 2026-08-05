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

## Orchestration

| Field | Value |
| --- | --- |
| Controller state | Awaiting Independent Review |
| Lane | Q |
| Wave | 2 |
| Branch | `task/qm-0121-metric-kernels` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0121` |
| Base | `e49ac24` |
| Head | `131d102` — five commits: `988d4c6`, `9b7dc84`, `131d102` by `impl-agent-7`; `670edb6`, `be721b4` by the concurrent differential-verification agent |
| Agent | `impl-agent-7` |
| Evidence | `.plan/evidence/QM-0121.md` |
| Merge path | L |

**Tests added:** 27 — 26 unit tests in `crates/q-gpu/src/paired.rs` and 1 in the
new binary `crates/q-gpu/tests/paired_allocation_bounds.rs`.

**Floor change:** `rust_tests` 677 → **704** by this agent's 27 tests, then → **715**
by the other agent's 11; `rust_binaries` 51 → **52** → **53**. Raised only, never
lowered — `main`'s floor was independently checked and is still 677/51.
`677 + 27 = 704` and `704 + 11 = 715` reconcile exactly. Web floors
untouched at `115 / 13` — this task changed no web file. `scripts/baseline.json`
`commit` repinned to `131d102`, the first commit whose own tree measures 715/53.

**Branch-name note:** the branch is `task/qm-0121-metric-kernels` because it was
pre-created from an illustrative example name; the task is
`QM-0121-paired-block-reduction`. The `qm-0121` id is what identifies it.

**Read this before reviewing:** a second agent was concurrently writing its own
QM-0121 artefacts into the same worktree. None of it was deleted, modified, or
committed here; see `.plan/evidence/QM-0121.md` §Claim limits, and the note there
on the web gate, which fails for an environment reason (`three` is not installed)
that predates this branch and is unaffected by it.

### Addendum — the differential-verification half (`impl-agent-14`)

Appended inside this section rather than as a second `## Orchestration` heading,
because a parser reads these headings. The table above describes the
implementation half only; the rows below supersede its **Head** and **Floor
change** for the branch as a whole.

| Field | Value |
| --- | --- |
| Controller state | Awaiting Independent Review |
| Lane | Q |
| Branch | `task/qm-0121-metric-kernels` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0121` |
| Base | `e82fe98` (dispatch brief); `e49ac24` is the branch point actually present |
| Head | **`670edb6`** |
| Agent | `impl-agent-14` |
| Evidence | `.plan/evidence/QM-0121-differential-verification.md` |
| Merge path | L |

**Tests added:** 11, in the new binary
`crates/q-gpu/tests/paired_reference_goldens.rs` — the differential test against
`python/reference/paired_reduction_reference.py`, the NumPy reference
`.plan/evidence/QM-0121.md` §Not performed records as absent from the
implementation half and as needed for gate **G2**.

**Floor change:** `rust_tests` 704 → **715**, `rust_binaries` 52 → **53**, both
raised only; `704 + 11 = 715` and `52 + 1 = 53` reconcile exactly. Web floors
untouched at `115 / 13`. `scripts/baseline.json` `commit` is left at `988d4c6`,
a real ancestor a reviewer can check out, and the new note there says plainly
that that commit yields 704/52 on its own and 715/53 once this binary is present.
**Two other branches are raising this floor concurrently; the controller
reconciles at merge.**

**Found by the verification:** the kernel **panicked** instead of refusing on a
`BlockData` whose declared shape outran its buffer. Fixed by `require_dense` in
`crates/q-gpu/src/paired.rs`, which `impl-agent-7` then absorbed into `988d4c6`
with a unit test of its own. This is the only edit `impl-agent-14` made to
another agent's file.
