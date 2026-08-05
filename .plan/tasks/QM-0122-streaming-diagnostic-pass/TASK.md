# QM-0122 — Streaming diagnostic pass over a whole tensor

## Status

Blocked

Unblocks when `QM-0121` reaches `Complete`.

## Phase

Phase 11 — Quantisation-error diagnostic engine — **Gate G2**

## Objective

Run the whole pipeline over one real tensor under bounded memory — derive
quantisation parameters, stream blocks, simulate, reduce, accumulate — and prove
every resulting metric against an independent NumPy reference.

## Repository Evidence

* `QM-0030` — `BlockStream`, bounded buffers, backpressure, cancellation between
  blocks, deterministic row-major block order.
* `QM-0120` — `derive_params`, `simulate`.
* `QM-0121` — `paired_block_reduction`, `PairedPartials`.
* `crates/q-statistics/src/lib.rs` — `streaming_in_chunks_equals_computing_at_once`
  (`STAT-004`), the property this task extends to paired metrics.
* `tests/tests/end_to_end_scalar_slice.rs` — the Python-reference idiom
  (`AC-005`), against `safetensors==0.8.0`.

## Requirements Covered

`QUANT-003`, `V1-09`, `V1-10`, `V1-13`.

## Dependencies

`QM-0121`, `QM-0030`, `QM-0101`.

## Blocks

`QM-0123`, `QM-0102`, `QM-0126`.

## Parallelization

Lane Q. Owns `crates/q-diagnostics`.

## Program Boundary

`crates/q-diagnostics` (new).

## Scope

* The two-pass tensor pass: pass 1 accumulates scale statistics per granularity
  unit; pass 2 streams blocks, simulates, reduces, accumulates.
* Derived metrics computed **once**, at the tensor level.
* An independent NumPy reference for the whole pipeline, not just its parts.
* Cancellation between blocks; deterministic accumulation order.

## Out of Scope

Aggregation above a tensor (`QM-0123`) · outlier attribution (`QM-0124`) ·
ranking (`QM-0125`) · persistence (`QM-0020`) · the report (`QM-0141`).

## The two-pass design

Per-channel and per-tensor granularity need whole-axis statistics — a maximum over
a full column — which a single streaming pass over row-major blocks cannot have
without buffering. Buffering a column of a large tensor is a residency bug.

```text
Pass 1:  stream all blocks → accumulate per-unit min/max only     O(units) memory
         (per-group granularity within a block: skipped entirely)
Pass 2:  stream all blocks again → derive params → simulate → reduce
```

Two passes over disk, constant memory. The alternative — one pass with a buffered
column — breaks `V1-03` on exactly the checkpoints that matter.

**Per-group granularity needs no pass 1** when the group lies within a block,
which is why the block grid must align to the group size. The planner enforces
alignment and refuses configurations where it cannot; a silently misaligned group
would quantise across a boundary that the reference does not.

## Files Expected to Change

* `Cargo.toml` — workspace member

## Files Expected to Add

* `crates/q-diagnostics/Cargo.toml`
* `crates/q-diagnostics/src/lib.rs`
* `crates/q-diagnostics/src/pass.rs`
* `python/reference/diagnose_reference.py`
* `tests/tests/diagnostic_pass_reference.rs`

## Data Contracts

```rust
pub struct TensorDiagnostic {
    pub tensor_id: TensorId,
    pub canonical_address: String,
    pub shape: Vec<u64>,
    pub dtype: DType,
    pub config: QuantConfig,

    pub count: u64,
    pub sum_sq_base: f64,
    pub sum_sq_delta: f64,
    pub sum_abs_delta: f64,
    pub max_abs_delta: f64,
    pub max_abs_base: f64,
    pub per_channel: Vec<ChannelPartials>,

    pub bytes_at_base_precision: u64,
    pub bytes_at_target_precision: u64,
    pub blocks_processed: u64,
    pub bytes_read: u64,
    pub fidelity: Fidelity,          // Exact | Sampled
}

impl TensorDiagnostic {
    pub fn rmse(&self) -> f64;             // sqrt(sum_sq_delta / count)
    pub fn relative_error(&self) -> f64;   // sqrt(sum_sq_delta / sum_sq_base)
    pub fn mean_abs_delta(&self) -> f64;
}
```

Derived metrics are **methods, not fields**. A field could be serialised
pre-rounded and then re-aggregated; a method cannot.

## Memory and Performance Constraints

```text
resident = base block + counterpart block + per-unit params + per-channel partials
         ≈ 2 × (256 × 256 × 4 B) + O(channels)
         ≈ 1 MB at defaults, independent of tensor and checkpoint size
```

`bytes_read` = 2 × tensor size (two passes) for per-tensor and per-channel
granularity, 1 × for aligned per-group. The report states which, so a reader can
account for the I/O.

Accumulation is single-threaded in row-major block order even where block
computation is parallel — `V1-13` requires byte-identical output.

## Implementation Plan

1. `diagnose_tensor(source, descriptor, config, backend) -> TensorDiagnostic`.
2. Validate the config against the tensor: granularity vs. shape, group-size
   alignment vs. block grid. Refuse misalignment naming both.
3. Pass 1 where the granularity needs it: stream blocks, accumulate per-unit
   min/max. Skip entirely for aligned per-group.
4. Derive `QuantParams` per unit.
5. Pass 2: stream blocks; for each, `simulate` into a reused buffer, then
   `paired_block_reduction`; accumulate in fixed order.
6. Check cancellation between blocks; on cancel, return the tensors completed and
   mark the run cancelled — never a partial tensor presented as whole.
7. Write the NumPy reference: load the tensor with `safetensors`, quantise with
   the same arithmetic, compute the metrics directly, emit JSON.
8. Compare Rust against it on fixture tensors, then on one real tensor from the
   `QM-0100` checkpoint.

## Error Handling

| Case | Behaviour |
| --- | --- |
| Group size not aligned to the block grid | Refuse, naming both, before any read |
| Granularity incompatible with rank | Refuse naming the rank (`ADR-010` rank ceiling applies) |
| Non-finite weights | Refuse the tensor, naming the position — a real finding |
| Short read | Error naming tensor and byte range; never zero-fill |
| Cancellation | Stop at a block boundary; report completed work; mark cancelled |
| Backend refuses the workload | Propagate `BudgetExceeded` with the budget name |

## Acceptance Criteria

1. Every metric matches `diagnose_reference.py` on at least four fixture tensors
   spanning f32 and bf16, square and non-square.
2. The same agreement holds on one real tensor from the `QM-0100` checkpoint.
3. Streaming in blocks equals whole-tensor computation — the `STAT-004` property,
   extended to paired metrics.
4. Two runs are byte-identical, including `per_channel` ordering.
5. Peak residency during a full-tensor pass is within `QM-0101`'s band and is
   independent of tensor size, measured at three tensor sizes.
6. A misaligned group size is refused before any read.
7. Cancellation stops at a block boundary and the result is marked cancelled.
8. `bytes_read` matches the two-pass or one-pass expectation exactly.

## Verification Plan

**Automated** — reference comparison; the streaming-equals-batch property;
determinism; residency assertions.
**Manual** — one real-tensor comparison, output pasted.

## Suggested Commands

```bash
cargo test -p q-diagnostics
python3 python/reference/diagnose_reference.py \
    --checkpoint fixtures/tiny-llama-2shard \
    --tensor 'model.layers[10].self_attention.query_projection.weight' \
    --precision int4 --granularity group:128 --asymmetric
cargo test --test diagnostic_pass_reference
/usr/bin/time -l ./target/release/q-cli diagnose-tensor models/<checkpoint> <address> --precision int4
```

## Test Cases

| Input | Expected |
| --- | --- |
| 4096×4096 f32, int8 per-channel symmetric | Matches reference on every metric |
| Same tensor, int4 group-128 asymmetric | Matches reference |
| bf16 tensor | Matches; decode is exact (`SRC-016`) |
| Non-square 4096×1024 | Matches; per-channel length equals the output-channel count |
| Blocks 256 vs. 512 | Identical results — blocking is an implementation detail |
| Group 128, block columns 100 | Refused: misaligned |
| Cancel after 10 blocks | Stops at a boundary; marked cancelled |
| Two runs | Byte-identical |
| Real checkpoint tensor | Matches the reference |

The "blocks 256 vs. 512" case is the strongest single test in the task: if
blocking leaks into the answer, everything downstream is wrong.

## Risks

> **Inherited risk from `QM-0120`, recorded 2026-08-05 — read before implementing.**
> This task derives per-channel parameters from accumulated min/max and therefore
> **cannot call `derive_params_named`**, which makes its **`max == min` branch the
> exact place `QM-0120`'s worst defect can reappear.**
>
> `QM-0120` first used `s = 1` for a constant non-zero group, reconstructing
> `0.5 → 0.0` — a **100 % error**. It survived a differential test against an
> independent NumPy reference because that golden set's only constant magnitude was
> `c = 1`, **the single value at which `s = 1` and `s = \|c\|` agree.** The
> corrected rule is now specified in `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1's
> degenerate-case table and implemented in `crates/q-quant/src/rtn.rs`.
>
> **Do not re-derive it.** Reuse `q-quant`'s logic, and make sure this task's golden
> set contains a constant non-zero group whose magnitude is **not 1**, so the test can
> actually discriminate. Agreement with a reference proves the arithmetic matches on
> the values you chose; it does not prove you chose values that can tell two candidate
> formulas apart. **This task owns gate G2** — `.plan/EXECUTION_ORDER.md` §7: *"the
> engine is the product; a wrong number is worse than no number."*


| Risk | Mitigation |
| --- | --- |
| Pass 1 buffers a column and breaks residency | Residency assertion at three tensor sizes is an acceptance criterion |
| Group misalignment quantises across a boundary silently | Refused at validation, before any read |
| The NumPy reference reimplements a shared misconception | It loads with `safetensors` and computes metrics directly from the array — no shared code path with Rust |
| Parallel accumulation breaks determinism | Fixed-order single-threaded accumulation; the determinism test guards it |
| Two passes double I/O unexpectedly for a partner | `bytes_read` is reported and the report states the pass count |

## Completion Evidence

* Reference-vs-Rust comparison table, per metric, for every test tensor.
* The real-tensor comparison.
* Residency measurements at three tensor sizes.
* Determinism check output.
* `cargo test -p q-diagnostics` counts and the commit SHA.
