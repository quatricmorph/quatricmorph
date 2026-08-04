# QM-0127 — Metal differential verification against CPU

## Status

Blocked

Unblocks when `QM-0126` reaches `Complete`.

## Phase

Phase 11 — Quantisation-error diagnostic engine

## Objective

Prove on real Apple GPU hardware that the Metal backend produces the same answers
as the CPU reference, within a stated tolerance — and only then let it claim
verification.

## Repository Evidence

* `crates/q-gpu/src/lib.rs` — `CpuBackend`, `GPU-002 Verified`, the reference.
* `QM-0126` — `MetalBackend` with `capabilities().verified == false`.
* `crates/q-cuda/src/lib.rs` — `every_operation_refuses_with_a_requirement_id_rather_than_faking_output`:
  the standard this repository holds unverified hardware to.
* `.plan/CUDA_ARCHITECTURE.md` §6 — the tolerance convention this mirrors.

## Requirements Covered

`V1-14`, and it is what flips `GPU-003` from `Hardware-Unverified` to `Verified`
in `STATUS.md`.

## Dependencies

`QM-0126`, `QM-0122`.

## Parallelization

Lane U. Requires Apple GPU hardware — present on the development machine.

## Program Boundary

`crates/q-gpu`, `tests/`.

## Scope

* A differential test running identical inputs through both backends.
* Tolerances, stated per metric and justified.
* A determinism check on the Metal path itself.
* An end-to-end run of `QM-0122`'s pass on both backends, compared.

## Out of Scope

Performance comparison (`QM-0102`) · optimisation · CUDA.

## Tolerances

Per [`DIAGNOSTIC_ARCHITECTURE.md`](../../DIAGNOSTIC_ARCHITECTURE.md) §4.3:

| Field | Tolerance | Why |
| --- | --- | --- |
| `sum_sq_base`, `sum_sq_delta`, `sum_abs_delta` | relative ≤ 1e-6 | Different summation order over f32 inputs; f64 accumulation bounds the drift |
| `max_abs_delta`, `max_abs_base` | **exact** | A maximum has no rounding excuse. Any deviation is a bug, not precision |
| `count` | **exact** | Integer |
| `per_channel[*]` | as above, per field | Same reasoning, per channel |
| Derived `relative_error` at tensor level | relative ≤ 1e-6 | Inherits from the sums |

The exactness requirement on maxima is the sharpest test in the task: it catches
indexing errors, boundary handling, and dropped elements that a tolerance on sums
would absorb.

## Files Expected to Add

* `tests/tests/metal_differential.rs`

## Data Contracts

None new.

## Memory and Performance Constraints

The differential run uses the same bounded block sizes as production. It must not
require materialising a whole tensor on either backend, so the test doubles as a
residency check on the Metal path.

## Implementation Plan

1. Fixture blocks covering: square, non-square, single-row, single-column, block
   sizes 64 / 256 / 512, and a block whose channel count is not a multiple of the
   threadgroup width.
2. Run each through both backends; compare field by field at the tolerances.
3. Run `QM-0122`'s whole-tensor pass on a fixture tensor through both backends;
   compare the resulting `TensorDiagnostic`.
4. Metal determinism: the same input twice on device, bit-identical.
5. Include values that stress precision — very large, very small, mixed sign,
   subnormal.
6. On success, set `capabilities().verified = true` and update `STATUS.md`.
7. Where no device is present, skip with a named reason.

## Error Handling

* A deviation beyond tolerance → **fail**, naming the field, both values, and the
  block. Do not widen the tolerance to pass; that is how a wrong number ships.
* Metal nondeterminism across two identical runs → fail; the kernel's reduction
  order is wrong (`QM-0126` step 4).
* Device unavailable → skip with a reason; `GPU-003` stays `Hardware-Unverified`
  and the documentation says so.

## Acceptance Criteria

1. Every fixture block agrees within tolerance on every field.
2. `max_abs_delta` and `max_abs_base` agree **exactly** on every fixture.
3. A whole-tensor pass agrees across backends within tolerance.
4. Two Metal runs of the same input are bit-identical.
5. The non-multiple-of-threadgroup-width case passes — the classic boundary bug.
6. Subnormal and near-overflow inputs pass or are refused identically by both
   backends.
7. `capabilities().verified` becomes `true` only after all of the above.
8. Device name, OS version, and per-field maximum observed deviation are recorded.

## Verification Plan

**Automated** — the differential test.
**Manual** — the whole-tensor comparison on the real `QM-0100` checkpoint if time
and disk permit; recorded either way.

## Suggested Commands

```bash
cargo test -p q-gpu --features metal metal_differential -- --nocapture
sw_vers && system_profiler SPDisplaysDataType | head -20
```

## Test Cases

| Input | Expected |
| --- | --- |
| 256×256 f32 random | Within tolerance; maxima exact |
| 4×1024 (single-ish row) | Within tolerance |
| 1024×4 (single-ish column) | Within tolerance |
| Channels = 33 (not a multiple of threadgroup width) | Correct; no dropped channel |
| Values spanning 1e-38 … 1e38 | Within tolerance or identically refused |
| All-zero counterpart | Deltas equal base sums exactly on both |
| Same input twice on Metal | Bit-identical |
| Whole fixture tensor, both backends | `TensorDiagnostic` agrees |

## Risks

| Risk | Mitigation |
| --- | --- |
| The tolerance is widened until the test passes | The tolerance is fixed in `DIAGNOSTIC_ARCHITECTURE.md` §4.3 and changing it requires editing that document with a reason |
| A dropped channel hides inside a sum tolerance | Maxima are exact; the channel-count boundary case is explicit |
| Verified is set before the test passes | Acceptance criterion 7; `STATUS.md` regeneration audits it |
| Passing on fixtures but not real data | The whole-tensor comparison, and `QM-0122` re-run on both backends |

## Completion Evidence

* Full differential test output with per-field maximum deviations.
* Device name and OS version.
* The determinism check.
* The `STATUS.md` diff flipping `GPU-003`.
* If skipped for lack of hardware: the skip reason, and `GPU-003` left
  `Hardware-Unverified`.
