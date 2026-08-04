# QM-0120 — Quantisation simulation

## Status

Blocked

Unblocks when `QM-0030` reaches `Complete`.

## Phase

Phase 11 — Quantisation-error diagnostic engine

## Objective

Given a block of decoded f32 values and a `QuantConfig`, produce the dequantised
counterpart `Ŵ = dequant(quant(W, config))` — exactly, reproducibly, and in
agreement with an independent NumPy reference.

## Repository Evidence

* `crates/q-source/src/dtype.rs` — `DType` including `I8`, `U8`, `F8E4M3`,
  `F8E5M2`; exact f32/bf16/f16 decode (`SRC-016`); `unknown_dtype_is_rejected_not_guessed`
  and `fp8_refuses_rather_than_approximates` — the refusal idiom this task extends.
* `crates/q-gpu/src/lib.rs` — `BlockData::new(rows, columns, values)`.
* `crates/q-statistics/src/lib.rs` — `relative_l2`, `cosine_similarity`, both
  hand-verified; Welford accumulation.
* `crates/q-tensor-runtime` — `BlockExtent::clamped_to`; blocks are clamped,
  never padded.

## Requirements Covered

`QUANT-001`, `V1-08`, `V1-15`.

## Dependencies

`QM-0030`.

## Blocks

`QM-0121`, `QM-0122`.

## Parallelization

First task in Lane Q. Owns the new `crates/q-quant` exclusively.

## Program Boundary

`crates/q-quant` (new). **No file access, no catalog, no I/O policy** — values in,
values out. That separation is what lets a later module reuse it to verify a
third-party quantised checkpoint.

## Scope

* `QuantConfig`: precision (int8, int4), granularity (per-tensor, per-output-
  channel, per-group{size}), zero point (symmetric, asymmetric), rounding
  (nearest-even only).
* Scale and zero-point derivation.
* Quantise-then-dequantise producing f32 output of the same shape.
* Every degenerate case specified in `DIAGNOSTIC_ARCHITECTURE.md` §3.1.
* Refusal of every scheme not implemented, naming `QUANT-011`.

## Out of Scope

Reading quantised checkpoints (`QUANT-010`) · GPTQ error feedback, AWQ scaling,
NF4, MXFP4 (`QUANT-011`) · error metrics (`QM-0121`) · streaming (`QM-0122`) ·
writing a quantised model — v1 never materialises one.

## Files Expected to Change

* `Cargo.toml` — add the workspace member

## Files Expected to Add

* `crates/q-quant/Cargo.toml`
* `crates/q-quant/src/lib.rs`
* `crates/q-quant/src/rtn.rs`
* `python/reference/quantise_reference.py` — the NumPy reference

## Data Contracts

```rust
pub enum Precision { Int8, Int4 }
pub enum Granularity { PerTensor, PerOutputChannel, PerGroup { size: u32 } }
pub enum ZeroPoint { Symmetric, Asymmetric }
pub enum RoundMode { NearestEven }

pub struct QuantConfig {
    pub precision: Precision,
    pub granularity: Granularity,
    pub zero_point: ZeroPoint,
    pub round: RoundMode,
}

/// Scale/zero-point parameters for one granularity unit.
pub struct QuantParams { pub scale: f32, pub zero: i32 }

/// Derive parameters. `values` is one granularity unit, never a whole tensor.
pub fn derive_params(values: &[f32], config: &QuantConfig) -> Result<QuantParams>;

/// Quantise and dequantise in one step. v1 never needs the integer codes.
pub fn simulate(values: &[f32], params: &QuantParams, config: &QuantConfig)
    -> Result<Vec<f32>>;
```

Returning only the dequantised values — not the integer codes — is deliberate:
v1 diagnoses, it does not emit a quantised model, and an API that cannot produce
one cannot accidentally be used to.

## The arithmetic

```text
symmetric:  s = max|g| / (n/2 - 1)
            q = clamp(round_half_to_even(x / s), -(n/2), n/2 - 1)
            x̂ = q · s

asymmetric: s = (max(g) - min(g)) / (n - 1)
            z = round_half_to_even(-min(g) / s)
            q = clamp(round_half_to_even(x / s) + z, 0, n - 1)
            x̂ = (q - z) · s
```

`round_half_to_even`, stated explicitly: half-away-from-zero disagrees with NumPy
on exactly the boundary values the golden tests contain.

## Memory and Performance Constraints

Allocation is `O(unit size)`, never `O(tensor)`. `simulate` writes into a caller-
provided buffer where the caller has one, so the streaming pass reuses buffers
across blocks.

Per-channel granularity needs whole-column statistics; **this task does not
compute them**. It accepts derived `QuantParams`. Deriving them under bounded
memory is `QM-0122`'s two-pass design, and keeping that concern out of here is
what keeps this crate testable in isolation.

## Implementation Plan

1. Define the config types and a closed `Precision` enum with explicit level
   counts.
2. `derive_params` for symmetric and asymmetric, with the degenerate cases first.
3. `simulate`, in-place where a buffer is supplied.
4. `round_half_to_even` as a named helper with its own tests.
5. Refusal paths for unimplemented schemes, each naming `QUANT-011`.
6. The NumPy reference script, and golden vectors generated from it.
7. Tests: hand-computed small groups, boundary rounding, every degenerate case,
   and agreement with the goldens.

## Error Handling

| Case | Behaviour |
| --- | --- |
| All-zero unit | `s = 1`, all codes 0, output all zero. No division by zero |
| Scale underflows to subnormal | Refuse, naming the unit. Never emit infinities |
| NaN or ±Inf in the unit | Refuse the tensor. Non-finite weights are a finding, reported as one |
| Group size does not divide the axis | Final group is **clamped**, never padded |
| Unimplemented scheme requested | `QError::NotImplemented` naming `QUANT-011` and listing what is implemented |
| `n` would exceed the dtype's range | Refuse at config validation, before any arithmetic |

## Acceptance Criteria

1. Symmetric and asymmetric int8 and int4 match the NumPy reference on the golden
   vectors — exactly where the arithmetic is exact.
2. `round_half_to_even` agrees with NumPy at `0.5`, `1.5`, `−0.5`, `2.5`, and at
   the clamping boundaries.
3. Every degenerate case behaves as tabulated, with a test each.
4. Per-group granularity with a non-dividing size clamps the final group; element
   count is asserted.
5. Unimplemented schemes refuse, naming `QUANT-011` and the implemented set.
6. `simulate` allocates nothing proportional to tensor size.
7. Round-tripping a value already representable at the target precision is
   idempotent.

## Verification Plan

**Automated** — unit tests with hand-computed expectations; golden comparison
against committed NumPy output.
**Manual** — regenerate the goldens and confirm they are unchanged.

## Suggested Commands

```bash
cargo test -p q-quant
python3 python/reference/quantise_reference.py --emit-goldens fixtures/quant-goldens/
cargo test -p q-quant golden
```

## Test Cases

| Input | Expected |
| --- | --- |
| `[-1, 0, 1]`, int8 symmetric | Hand-computed scale; exact round trip |
| `[0.1, 0.2, 0.3]`, int4 asymmetric | Matches NumPy to the last bit |
| `[0, 0, 0]` | Scale 1, all zeros, no NaN |
| `[1e-45, 0]` (subnormal) | Refused, unit named |
| `[1.0, f32::NAN]` | Refused, tensor named |
| 130 values, group size 128 | Two groups: 128 and 2, clamped |
| Exactly-representable values | Idempotent |
| `Precision::Nf4` | `NotImplemented` naming `QUANT-011` |
| 4096-element unit | Allocation independent of a 4096×4096 tensor |

## Risks

| Risk | Mitigation |
| --- | --- |
| Rounding mode silently disagrees with NumPy | Boundary values are their own test; the golden comparison is exact |
| Asymmetric zero-point off by one | Hand-computed fixtures with known asymmetric ranges |
| The crate grows I/O and stops being reusable | Program boundary states values-in/values-out; a dependency on `q-source` here is a review failure |
| Someone adds a "just approximate it" path for fp8 | `SRC-014` already refuses; the same discipline applies |

## Completion Evidence

* `cargo test -p q-quant` output with counts.
* The NumPy reference script and the diff of its output against the Rust
  implementation across the golden vectors.
* Hand computations for at least three cases, written out.
* Confirmation that `q-quant`'s dependency list contains no I/O crate.
