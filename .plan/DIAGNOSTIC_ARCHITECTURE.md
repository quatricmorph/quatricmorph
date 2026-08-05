# DIAGNOSTIC_ARCHITECTURE — the quantisation-error engine

The v1 product surface is one question, answered honestly:

> **Which layers should I leave at higher precision, and what does that cost me
> in bytes?**

This document specifies the engine that answers it. Scope boundary:
[`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md). Output format:
[`REPORT_ARCHITECTURE.md`](REPORT_ARCHITECTURE.md).

---

## 1. Why simulation, not ingestion

v1 computes `Ŵ = dequant(quant(W, config))` itself, block by block, from the base
checkpoint. It does **not** read a third-party quantised artifact.

| | Simulate (v1) | Ingest a quantised checkpoint (post-v1) |
| --- | --- | --- |
| Question answered | "Which layers should I leave at higher precision?" — a **pre**-quantisation decision | "Did the quantisation I already ran go wrong, and where?" |
| New input formats | **None.** The existing SafeTensors reader suffices | GPTQ, AWQ, `compressed-tensors`, GGUF — three packing conventions and a k-quant zoo |
| Configs comparable | Any number, in one run, without re-quantising anything | One, the one they shipped |
| Time to first result | Days | Weeks of format archaeology |

The strategy's own value proposition sentence (§11) is the pre-quantisation
question. Simulation answers it completely. Ingestion is `QUANT-010`, a seam, and
a later module.

**Consequence worth stating:** because v1 never materialises a quantised
checkpoint, it never writes one. The output is a *diagnosis*, not a model. That
keeps v1 clear of "no proprietary lock-in on the underlying checkpoint" (strategy
§7.7) by construction.

---

## 2. Crate boundaries

Two new crates, both pure computation with no I/O policy of their own.

```text
crates/q-quant/         Quantisation schemes. Given a block of f32 and a config,
                        produce the dequantised block. No file access, no catalog.
crates/q-diagnostics/   The engine. Streams blocks, pairs them, reduces, aggregates,
                        ranks, and computes the frontier. Depends on q-quant,
                        q-gpu, q-tensor-runtime, q-nsir, q-catalog.
```

`q-quant` is deliberately separable: it is the piece a later module reuses when
*verifying* someone else's quantisation, and the piece most likely to be
independently testable against `torch.quantize_per_channel` and friends.

Existing crates that participate unchanged: `q-source` (budgets, mmap reads,
dtype decode), `q-tensor-runtime` (block planning, streaming), `q-statistics`
(Welford, `relative_l2`, `cosine_similarity`), `q-gpu` (`Backend`), `q-catalog`
(persistence), `q-cache` (content-addressed reuse).

---

## 3. Quantisation schemes — v1 surface

```rust
pub struct QuantConfig {
    pub precision: Precision,       // Int8 | Int4
    pub granularity: Granularity,   // PerTensor | PerOutputChannel | PerGroup { size: u32 }
    pub zero_point: ZeroPoint,      // Symmetric | Asymmetric
    pub round: RoundMode,           // NearestEven   (the only v1 variant)
}
```

**RTN (round-to-nearest) only in v1.** Not because it is the best quantiser, but
because it is the one every other method is measured against, it is exactly
reproducible in NumPy, and it needs no calibration data. GPTQ-style error feedback,
AWQ-style activation-aware scaling, NF4 and MXFP4 are named variants of
`QuantScheme` that refuse with `QUANT-011`.

### 3.1 The arithmetic, fixed and testable

For a group of values `g` with `n` levels (`n = 2^bits`):

```text
symmetric:    s = max|g| / (n/2 - 1)
              q = clamp(round_half_to_even(x / s), -(n/2), n/2 - 1)
              x̂ = q · s

asymmetric:   s = (max(g) - min(g)) / (n - 1)
              z = round_half_to_even(-min(g) / s)
              q = clamp(round_half_to_even(x / s) + z, 0, n - 1)
              x̂ = (q - z) · s
```

Degenerate cases are specified, not left to the implementation:

| Case | Behaviour |
| --- | --- |
| `max\|g\| == 0` (all-zero group) | `s = 1`, all `q = 0`, `x̂ = 0`. No division by zero |
| **`min(g) == max(g) != 0`** (constant non-zero group, asymmetric) | **`s = \|c\|`, not `s = 1`.** Added 2026-08-05 by `QM-0120`; see the note below |
| `s` underflows to subnormal | Refuse the group, naming the tensor and offset. Never silently produce infinities |
| **Reconstruction is non-finite** | **Refuse per value.** `s` itself can be perfectly normal while `q_max · s` overflows — e.g. `s = f32::MAX/127` is `is_normal()`, yet `127·s` rounds past `f32::MAX`. A params-only bound is insufficient *and* over-refuses: three cases it would reject do reconstruct finitely. Added 2026-08-05 by `QM-0120` |
| A group contains NaN or ±Inf | Refuse the tensor. A checkpoint with non-finite weights is a finding, reported as one |
| Group size does not divide the axis | Final group is **clamped, never padded** — the same rule `BlockExtent::clamped_to` already applies |

`round_half_to_even` is stated explicitly because half-away-from-zero would
disagree with NumPy on exactly the boundary values a golden test will contain.

> **The constant-non-zero-group rule, and why it is now specified here.**
> The `max|g| == 0` row above is conditioned on the group being **all zero**, so it
> never reaches a *constant non-zero* group — this section previously specified
> nothing for that case. `QM-0120` first read the table literally and used `s = 1`
> there, which reconstructs `0.5 → 0.0`: a **100 % error**.
>
> It survived a differential test against an independent NumPy reference because the
> first golden set's only constant magnitude was `c = 1` — **the single value at
> which `s = 1` and `s = |c|` produce identical output.** `review-agent-12`
> re-derived the corrected rule in an independent driver crate: `0.5`, `−0.3` and
> `0.823457` are all bit-exact under `s = |c|`, while `s = 1` gives 100 %, 100 %,
> and wrong-direction error. The all-zero group keeps the tabulated `s = 1, z = 0`.
>
> **The transferable lesson for `QM-0121` and `QM-0122`, which G2 depends on:**
> agreement with a reference proves the arithmetic matches *on the values you
> chose*. It does not prove you chose values that can distinguish two candidate
> formulas. A golden set needs inputs selected to **discriminate**, not merely to
> cover.
>
> **`QM-0122` inherits a specific instance of this.** It derives per-channel
> parameters from accumulated min/max and therefore cannot call
> `derive_params_named`, which makes its `max == min` branch the exact place this
> defect can reappear. `crates/q-quant/src/rtn.rs` holds the corrected logic;
> `QM-0122` must not re-derive it independently.

### 3.2 Axis convention

Granularity is defined against the **canonical** axis semantics NSIR already
assigns (`output_channel`, `input_channel`), not against raw tensor order.
`PerOutputChannel` means one scale per output channel regardless of how the
checkpoint stores the matrix. A test with a non-square, non-symmetric fixture
asserts this — transposing the axis must change the answer, and the test must
catch it if it does not (`V1-11`).

---

## 4. The paired reduction

### 4.1 Why the `Backend` trait must change

Today:

```rust
fn block_statistics(&self, source, descriptor, extent, histogram_bins) -> Result<TensorStatistics>;
```

One tensor in, one summary out. The wedge needs two blocks in — a base block and
its counterpart — and per-channel partials out. This is the first real
engineering task on the v1 critical path (`QM-0121`).

```rust
pub trait Backend {
    // ... existing methods unchanged ...

    /// Reduce a base block against a counterpart block, producing whole-block
    /// and per-output-channel partials. The counterpart may be a simulated
    /// quantisation of the base (v1) or an independently sourced block
    /// (checkpoint diff — DIFF-001, post-v1).
    fn paired_block_reduction(
        &self,
        base: &BlockData,
        counterpart: &BlockData,
        axis: ChannelAxis,
    ) -> Result<PairedPartials>;
}

pub struct PairedPartials {
    pub count: u64,
    pub sum_sq_base: f64,        // Σ w²        — denominator of relative error
    pub sum_sq_delta: f64,       // Σ (w − ŵ)²  — numerator; ‖·‖_F² before the root
    pub sum_abs_delta: f64,      // Σ |w − ŵ|
    pub max_abs_delta: f64,      // max |w − ŵ|
    pub max_abs_base: f64,       // max |w|     — for outlier attribution
    pub per_channel: Vec<ChannelPartials>,   // len == channel count of this block
}
```

**Everything is a partial, nothing is a final metric.** Sums of squares compose
across blocks; RMSE and relative error do not. Computing the finished metric per
block and averaging is the single most likely correctness bug in this engine, and
`V1-12` exists to catch it.

### 4.2 Composition

```text
tensor.sum_sq_delta = Σ_blocks block.sum_sq_delta
tensor.rmse         = sqrt(tensor.sum_sq_delta / tensor.count)
tensor.rel_error    = sqrt(tensor.sum_sq_delta / tensor.sum_sq_base)
tensor.max_abs_delta = max_blocks block.max_abs_delta
```

Determinism requires a fixed reduction order. Blocks reduce in **row-major block
order** — the order `q-tensor-runtime` already streams them in — and partials
accumulate in that order, single-threaded at the accumulation step even when block
computation is parallel. Parallel accumulation with floating-point addition is
not associative, and `V1-13` requires byte-identical output across runs.

### 4.3 CPU is the reference; Metal is an accelerator

| Backend | Role | Tolerance |
| --- | --- | --- |
| `CpuBackend` | **Numerical ground truth.** Every golden test compares against it, and it alone is compared against Python | Exact where the arithmetic is exact |
| `MetalBackend` | Accelerator behind the same trait | Relative deviation ≤ 1e-6 on `sum_sq_*` in f64 accumulation; `max_abs_*` **exact** — a max reduction has no rounding excuse |

The backend that ran is recorded per run and printed in the report (`V1-21`).
Claiming a GPU computed something the CPU computed is on the forbidden-claims list
([`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §5.2).

If the Metal lane slips, v1 ships CPU-only with a slower benchmark and a note.

---

## 5. Streaming and memory

The engine inherits, and may not weaken, the residency discipline in
[`MEMORY_BUDGET.md`](MEMORY_BUDGET.md).

```text
resident at any instant =
    base block            (block_rows × block_cols × 4 B)
  + counterpart block     (same)
  + per-channel partials  (channels × 40 B)
  + accumulators          (O(1) per tensor)
  × max_concurrent_blocks
```

At the defaults — 256×256 blocks, 4 concurrent — that is under 3 MB, and it is
**independent of tensor size and of checkpoint size**. `V1-03` and `V1-05` measure
it rather than assert it.

Three rules that make the difference between an engine and a script:

1. **The counterpart is never materialised for a whole tensor.** Quantisation
   parameters that need a whole-axis view (`PerOutputChannel` scales over a
   column) are computed in a **first pass that accumulates only the scale
   statistics** — `max|·|`, `min`, `max` per channel, O(channels) memory — and the
   second pass streams blocks again applying them. Two passes over disk, constant
   memory. A single-pass implementation that buffers a column is a residency bug.
2. **Group-wise granularity needs no second pass** when the group lies within a
   block, which is why the block grid must be aligned to the group size. The
   planner enforces this and refuses configurations where it cannot.
3. **Cancellation is checked between blocks**, and a cancelled run reports the
   tensors completed rather than discarding them.

---

## 6. Aggregation

Partials roll up the address hierarchy NSIR already provides — no new taxonomy:

```text
channel → tensor → module → layer → (expert, where the resolver found one) → model
```

Expert-keyed aggregation comes free from `NSIR-003` (`Expert[12,37].up` already
resolves), and it is what makes the MoE module (`MOE-001`) mostly a reducer swap
later.

Each level carries: `count`, `sum_sq_base`, `sum_sq_delta`, `sum_abs_delta`,
`max_abs_delta`, `bytes_at_base_precision`, `bytes_at_target_precision`. Finished
metrics are derived at read time, never stored pre-rounded.

---

## 7. Ranking and the mixed-precision frontier

This is the part that makes the output a decision rather than a chart.

**Ranking.** Tensors sorted by relative error, descending. Ties broken by
parameter count, then by canonical address — a total order, so the report is
deterministic.

**The frontier.** Given a byte budget, which tensors should stay at higher
precision?

```text
for each tensor t:
    Δerror(t) = sum_sq_delta(t)                      # error removed by keeping t at base precision
    Δbytes(t) = bytes_at_base(t) − bytes_at_target(t) # bytes it costs to keep it

rank by Δerror(t) / Δbytes(t)   descending
accumulate greedily; emit the frontier as (cumulative bytes added, cumulative
                                            fraction of total squared error removed)
```

Greedy over a density ratio is the standard fractional-knapsack heuristic and is
**not** claimed to be optimal for the integer problem. The report says so in one
line. What it *is*: exact arithmetic over two computed quantities, producing a
statement of the form

> Keeping layers 0, 1, and 27 at 8-bit costs **+0.82 GB** and removes **46 %** of
> total weight-space squared error.

Both numbers are computed. Neither is an accuracy prediction.

### 7.1 Outlier attribution

The *why* behind a fragile layer, and cheap to compute in the same pass: the
share of squared error carried by the top-*p* % of weights by magnitude
(`p ∈ {0.1, 1}`). A layer whose error is 90 % attributable to 0.1 % of its weights
is a candidate for outlier-preserving schemes; a layer with diffuse error is not.
That distinction is actionable and is entirely weight-space.

---

## 8. What this engine may never emit

Load-bearing, not editorial. The audience is an engineer deciding whether to
trust the tool with a private checkpoint.

| Forbidden | Why | Required wording |
| --- | --- | --- |
| A predicted accuracy or eval delta | Requires running an evaluation this tool does not run | *"Weight-space error only. Accuracy impact is not measured — run your evaluation on the recommended configuration."* |
| Hessian-weighted sensitivity `s_i = w_i² / [H⁻¹]_ii` | Needs a calibration set and activations; an inference runtime is a non-goal | `EVAL-002` seam refuses, naming what it would need |
| "This expert is dead" from weights alone | Deadness is a routing property, needs runtime | Weight-space redundancy may be reported as *redundancy*, never as deadness |
| "This layer is important/unimportant" | Importance is task-relative | *"Ranked by relative weight-space error, a proxy for sensitivity"* |
| A frontier described as optimal | It is greedy | *"Greedy over error-per-byte; not proven optimal"* |

The `EVAL-001` / `EVAL-002` seams exist as types whose constructors refuse with a
requirement ID — the idiom already used throughout this repository. When a
calibration-based sensitivity module is built, the report gains a section; until
then it gains a caveat.

---

## 9. Verification

| Level | What | Reference |
| --- | --- | --- |
| Unit | Each scheme against hand-computed values on tiny groups, including every degenerate case in §3.1 | Hand computation in the test |
| Unit | `round_half_to_even` boundary values (`0.5`, `1.5`, `−0.5`, `2.5`) | NumPy |
| Golden | Full quantise-and-reduce over checked-in fixture tensors | A committed Python/NumPy script under `python/`, run in CI-equivalent form |
| Property | Streaming aggregation equals whole-tensor computation | The `STAT-004` pattern extended to paired metrics |
| Property | Peak residency flat across three checkpoint sizes | `/usr/bin/time -l` |
| Differential | Metal vs. CPU at §4.3 tolerances | `CpuBackend` |
| Determinism | Two runs byte-identical; two machines byte-identical | `cmp` on the manifest |

The Python reference is the same discipline `AC-005` already uses for exact scalar
reads against `safetensors==0.8.0`, and for the same reason: an independent
implementation is the only thing that catches a shared misconception.

---

## 10. Requirement IDs introduced

| ID | Capability |
| --- | --- |
| `QUANT-001` | RTN simulation: int8/int4, per-tensor / per-channel / per-group, symmetric / asymmetric |
| `QUANT-002` | Paired block reduction in `q_gpu::Backend` with per-channel partials |
| `QUANT-003` | Streaming diagnostic pass over a whole tensor, Python-verified |
| `QUANT-004` | Aggregation channel → tensor → module → layer → expert → model |
| `QUANT-005` | Outlier attribution |
| `QUANT-006` | Fragility ranking and the mixed-precision frontier |
| `QUANT-010` | *(seam)* Third-party quantised checkpoint ingestion |
| `QUANT-011` | *(seam)* Additional quantisation schemes |
| `EVAL-001` | *(seam)* Accuracy estimation from a calibration set |
| `EVAL-002` | *(seam)* Hessian-weighted sensitivity |
| `DIAG-001` | Diagnostic results persisted in the catalog |
| `DIFF-001` | *(seam)* Checkpoint diff via the same paired reduction |
| `MOE-001` | *(seam)* Weight-space expert health |
| `PERF-002` | Bounded residency on a real ≥ 24 GB checkpoint |
