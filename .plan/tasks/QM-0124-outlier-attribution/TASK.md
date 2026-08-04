# QM-0124 — Outlier attribution

## Status

Blocked

Unblocks when `QM-0123` reaches `Complete`.

## Phase

Phase 11 — Quantisation-error diagnostic engine

## Objective

Answer *why* a layer is fragile: what share of its squared quantisation error is
carried by the largest-magnitude weights. A layer whose error is concentrated in
0.1 % of its weights is a candidate for outlier-preserving schemes; a layer with
diffuse error is not. That distinction is actionable and entirely weight-space.

## Repository Evidence

* `crates/q-statistics/src/lib.rs` — `Histogram`, `hand_computed_histogram_binning`,
  and the streaming accumulator whose discipline this follows.
* `QM-0121` — `PairedPartials` already carries `max_abs_base`.
* `QM-0122` — the streaming pass this hooks into, without adding a third pass.

## Requirements Covered

`QUANT-005`.

## Dependencies

`QM-0123`.

## Blocks

`QM-0141` (the report's "why" section).

## Parallelization

Lane Q, parallel with `QM-0125` — different files, both consume `QM-0123`.

## Program Boundary

`crates/q-diagnostics`.

## Scope

* Per-tensor and per-layer attribution: the fraction of `sum_sq_delta`
  contributed by the top-*p* % of weights ranked by `|w|`, for `p ∈ {0.1, 1.0}`.
* Computed within the existing streaming pass — **no additional pass over disk**.
* An honest statement of the approximation this requires.

## Out of Scope

Outlier-preserving quantisation schemes themselves (`QUANT-011`) · any claim
about what to do with the finding beyond naming the pattern · per-channel
attribution (a later refinement if partners ask for it).

## The approximation, stated up front

An exact top-*p* % threshold requires knowing the magnitude distribution before
attributing error, which means either two passes or retaining all values. Neither
is acceptable: a third pass doubles I/O again, and retaining values breaks
residency.

The design instead accumulates a **magnitude-binned histogram of squared error**
in the same pass:

```text
for each element:
    bin = log2_bucket(|w|)          // fixed, documented bucket edges
    hist_sq_delta[bin] += (w - ŵ)²
    hist_count[bin]    += 1
```

The top-*p* % threshold is then located in the histogram and the attribution
interpolated within the boundary bucket. The result is exact to bucket
granularity and the report says so — *"attribution is computed to log₂-magnitude
bucket resolution"* — rather than implying an exact percentile.

This is the honest version. An implementation that reports an exact-looking
percentile from bucketed data would be the kind of quiet overclaim
[`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §5.2 exists to prevent.

## Files Expected to Change

* `crates/q-diagnostics/src/pass.rs` — accumulate the histogram in pass 2
* `crates/q-diagnostics/src/aggregate.rs` — compose histograms across levels

## Files Expected to Add

* `crates/q-diagnostics/src/attribution.rs`

## Data Contracts

```rust
pub struct MagnitudeErrorHistogram {
    pub edges: &'static [f32],   // fixed log2 bucket edges, part of the format contract
    pub counts: Vec<u64>,
    pub sum_sq_delta: Vec<f64>,
    pub sum_sq_base: Vec<f64>,
}

pub struct OutlierAttribution {
    pub top_0_1_pct_error_share: f64,   // in [0, 1]
    pub top_1_pct_error_share: f64,
    pub resolution: &'static str,       // "log2 magnitude bucket"
}
```

Bucket edges are **fixed and versioned**, not data-dependent — two runs on the
same tensor must produce identical histograms, and two different models must be
comparable.

## Memory and Performance Constraints

`O(buckets)` — roughly 64 f64 values per tensor plus counts. Negligible.

**No additional pass over disk.** If the implementation needs one, the feature is
not worth its cost and should be dropped rather than paid for.

## Implementation Plan

1. Fix the bucket edges: powers of two spanning f32's useful range, with
   explicit underflow and overflow buckets. Version them.
2. Accumulate in pass 2 alongside the paired reduction.
3. Compose histograms across blocks and up the aggregation hierarchy — bucket-wise
   addition, exact.
4. Locate the top-*p* % threshold by walking buckets from the top; interpolate
   linearly within the boundary bucket, and document that this is the only
   approximation.
5. Report the shares plus the resolution string.

## Error Handling

* A weight outside every bucket → the explicit overflow/underflow buckets catch
  it; never silently dropped.
* Fewer elements than the *p* % threshold implies → report the share over what
  exists and mark it, rather than extrapolating.
* Empty tensor → no attribution; the field is absent, not zero.

## Acceptance Criteria

1. On a synthetic tensor with a known outlier structure — 99.9 % small values, a
   handful of large ones — the attribution recovers the planted share within one
   bucket's resolution.
2. On a uniform tensor, `top_0_1_pct_error_share ≈ 0.001`, confirming no bias.
3. Histograms compose exactly bucket-wise across blocks and levels.
4. Bucket edges are fixed, versioned, and identical across runs and models.
5. **No additional pass over disk** — `bytes_read` is unchanged from `QM-0122`.
6. The reported resolution string appears in the manifest and the report.
7. Overflow and underflow buckets are exercised by a test.

## Verification Plan

**Automated** — planted-outlier recovery; uniform-distribution baseline;
composition; a `bytes_read` assertion proving no extra pass.

## Suggested Commands

```bash
cargo test -p q-diagnostics attribution
```

## Test Cases

| Input | Expected |
| --- | --- |
| 10 000 values in [−0.01, 0.01] plus 10 at ±5.0 | Top-0.1 % share ≈ the planted share |
| Uniform values | Top-0.1 % share ≈ 0.001 |
| Two blocks composed vs. computed whole | Identical histograms |
| Values at ±f32::MAX and ±1e-40 | Overflow/underflow buckets, not dropped |
| Tensor with 100 elements, `p = 0.1 %` | Share over what exists, marked |
| `bytes_read` before and after this feature | Unchanged |

## Risks

| Risk | Mitigation |
| --- | --- |
| Bucketed attribution reported as an exact percentile | The `resolution` field is part of the contract and appears in the report |
| Data-dependent bucket edges break comparability | Edges are fixed constants and versioned |
| The feature quietly adds a pass | Acceptance criterion 5 asserts `bytes_read` |
| Attribution invites a causal claim about accuracy | The report states the pattern, never a prescription (`PRODUCT_SCOPE.md` §5.2) |

## Completion Evidence

* The planted-outlier test with expected and recovered shares.
* The uniform baseline result.
* `bytes_read` before and after, showing no change.
* The bucket-edge table and its version.
