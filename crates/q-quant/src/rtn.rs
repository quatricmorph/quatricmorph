//! Round-to-nearest quantisation, exactly as `.plan/DIAGNOSTIC_ARCHITECTURE.md`
//! §3.1 fixes it:
//!
//! ```text
//! symmetric:    s = max|g| / (n/2 - 1)
//!               q = clamp(round_half_to_even(x / s), -(n/2), n/2 - 1)
//!               x̂ = q · s
//!
//! asymmetric:   s = (max(g) - min(g)) / (n - 1)
//!               z = round_half_to_even(-min(g) / s)
//!               q = clamp(round_half_to_even(x / s) + z, 0, n - 1)
//!               x̂ = (q - z) · s
//! ```
//!
//! RTN is not claimed to be the best quantiser. It is the one every other method
//! is measured against, it is exactly reproducible in NumPy, and it needs no
//! calibration data (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3).
//!
//! ## Arithmetic, stated
//!
//! Checkpoint weights are `f32`, so every step above is `f32` — and so is every
//! step in `python/reference/quantise_reference.py`, which is why the two agree
//! **bit-for-bit** rather than to a tolerance. IEEE-754 single-precision divide,
//! multiply and subtract are correctly rounded, and so is
//! `roundToIntegralTiesToEven`; two implementations performing the same
//! operations in the same order on the same inputs therefore produce the same
//! bits, and any tolerance would only hide a real disagreement.
//!
//! One step is deliberately not `f32`: forming the integer code index (the `+ z`
//! and the `clamp`) happens in `f64` and then `i64`. `f32 -> f64` is always
//! exact, every `i32` zero point is exact in `f64`, and any sum that can land
//! inside the code range is below `2^53` — so this step introduces **no rounding
//! of its own wherever the result is in range**. Doing it in `f32` would lose the
//! low bits of `round_half_to_even(x/s) + z` for a large zero point and make the
//! two implementations disagree on a value neither of them rounded. The rounding
//! the scheme names happens in [`round_half_to_even`], before this step.
//!
//! ## Degenerate cases
//!
//! §3.1 tabulates them, and this module implements the table plus two extensions
//! recorded in `.plan/evidence/QM-0120.md`.
//!
//! **Zero dynamic range.** The table's `max|g| == 0` row exists so a unit with
//! zero dynamic range never divides by zero. Under `symmetric` the only such unit
//! is the all-zero one; under `asymmetric` the scale is `(max − min) / (n − 1)`,
//! which is zero for **any constant** unit, so a rule is needed whenever
//! `max == min`. The rule is **`s = |c|`** for a constant `c ≠ 0`, and the
//! tabulated `s = 1` only for `c == 0`. §3.1's zero-point and code formulas are
//! left untouched, so the deviation is one line: `z = round_half_to_even(-c/|c|)`
//! comes out as `∓1` and `x̂ = ±|c| = c` bit-exactly, for both signs and every
//! magnitude.
//!
//! `s = 1` would **not** do that, and the difference is not academic: at `s = 1`
//! §3.1's own formulas send a constant `0.5` to `0.0` and a constant `0.823457` to
//! `1.0`, silently, while `symmetric` reconstructs both exactly. A constant bias
//! or norm weight is common, so that is a real wrong number rather than a corner
//! case. A constant *subnormal* unit now refuses on §3.1's existing
//! subnormal-scale row, which is also what `symmetric` already did with it.
//!
//! **A reconstruction that is not finite.** §3.1's subnormal row says "never
//! silently produce infinities", and a normal scale is not sufficient for that:
//! `(q − z) · s` can overflow `f32` when the scale is within a factor of the code
//! range of `f32::MAX` — a constant unit at `f32::MAX` under int8 symmetric has a
//! perfectly normal `s = f32::MAX/127`, yet `127 · s` rounds up past `f32::MAX`.
//! Such a reconstruction is **refused**, naming the unit and the index. It can
//! only be detected per value, so `out`'s contents are unspecified when this
//! refusal is returned; every other refusal in this module precedes execution and
//! leaves `out` untouched.
//!
//! ## Allocation
//!
//! `O(unit)`, never `O(tensor)`. [`simulate_into`] allocates **nothing** on the
//! success path, [`group_extents`] is an iterator rather than a `Vec` (a `Vec` of
//! group extents would be `O(tensor / group)`), and [`simulate`] allocates
//! exactly one buffer of the unit's length for callers without one.
//! `crates/q-quant/tests/allocation_bounds.rs` measures this rather than
//! asserting it.

use crate::{Granularity, QuantConfig, QuantError, QuantParams, Result, UnitId, ZeroPoint};

/// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1's `round_half_to_even`.
///
/// Named, and given its own tests, because the alternative — half-away-from-zero
/// — disagrees with NumPy's `np.rint` on `0.5`, `2.5` and every other tie, which
/// is exactly what the golden vectors contain.
///
/// `f32::round_ties_even` is IEEE-754 `roundToIntegralTiesToEven`, the same
/// operation `np.rint` performs on a `float32` array. The sign of a zero is
/// preserved by both: `-0.5` rounds to `-0.0`, not `0.0`.
#[inline]
pub fn round_half_to_even(x: f32) -> f32 {
    x.round_ties_even()
}

/// Refuse a unit holding `NaN` or `±Inf`.
///
/// §3.1: *"A group contains NaN or ±Inf — refuse the tensor. A checkpoint with
/// non-finite weights is a finding, reported as one."* Propagating it would turn
/// one bad weight into an infinite scale and a whole tensor of `NaN`
/// reconstructions that still looked like numbers.
fn require_finite(values: &[f32], unit: UnitId<'_>) -> Result<()> {
    for (index, &value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(QuantError::NonFinite {
                unit: unit.to_string(),
                index,
                value,
            });
        }
    }
    Ok(())
}

/// Refuse a scale that is zero, subnormal, infinite, `NaN`, or not positive.
///
/// §3.1: *"`s` underflows to subnormal — refuse the group, naming the tensor and
/// offset. Never silently produce infinities."* `f32::is_normal` is exactly that
/// predicate; the positivity test additionally rejects a negative scale supplied
/// by a caller, which would invert every value it touched.
fn require_normal_scale(scale: f32, unit: UnitId<'_>) -> Result<()> {
    if !scale.is_normal() || scale <= 0.0 {
        return Err(QuantError::ScaleNotNormal {
            unit: unit.to_string(),
            scale,
        });
    }
    Ok(())
}

/// Derive parameters for one granularity unit.
///
/// `values` is **one granularity unit**, never a whole tensor: one whole tensor
/// under [`Granularity::PerTensor`], one output channel's values under
/// [`Granularity::PerOutputChannel`], one group under
/// [`Granularity::PerGroup`]. Selecting the unit is the caller's job — deriving
/// per-channel parameters under bounded memory is `QM-0122`'s two-pass design,
/// and keeping it out of here is what keeps this crate testable in isolation.
pub fn derive_params(values: &[f32], config: &QuantConfig) -> Result<QuantParams> {
    derive_params_named(values, config, UnitId::UNNAMED)
}

/// [`derive_params`], with a unit to name in a refusal.
///
/// §3.1 requires a refused group to name "the tensor and offset". [`UnitId`]
/// borrows its name, so naming a unit costs no allocation on the success path.
pub fn derive_params_named(
    values: &[f32],
    config: &QuantConfig,
    unit: UnitId<'_>,
) -> Result<QuantParams> {
    // Config first: §3.1's last error row refuses "before any arithmetic".
    config.validate()?;
    if values.is_empty() {
        return Err(QuantError::EmptyUnit {
            unit: unit.to_string(),
        });
    }
    require_finite(values, unit)?;

    let levels = config.precision.levels();
    match config.zero_point {
        ZeroPoint::Symmetric => {
            let mut max_abs = 0.0f32;
            for &v in values {
                let a = v.abs();
                if a > max_abs {
                    max_abs = a;
                }
            }
            if max_abs == 0.0 {
                // Tabulated: all-zero unit -> s = 1, every code 0, output all
                // zero. No division by zero, and no NaN.
                return Ok(QuantParams::IDENTITY);
            }
            let scale = max_abs / (levels / 2 - 1) as f32;
            require_normal_scale(scale, unit)?;
            Ok(QuantParams { scale, zero: 0 })
        }
        ZeroPoint::Asymmetric => {
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for &v in values {
                if v < lo {
                    lo = v;
                }
                if v > hi {
                    hi = v;
                }
            }
            let scale = if hi == lo {
                // Zero dynamic range — the extension of §3.1's
                // no-division-by-zero rule. `s = |c|`, NOT `s = 1`: at `s = 1`
                // §3.1's own formulas send a constant 0.5 to 0.0 and a constant
                // 0.823457 to 1.0, silently, while `symmetric` reconstructs both
                // exactly. See the module docs.
                let scale = if lo == 0.0 { 1.0f32 } else { lo.abs() };
                require_normal_scale(scale, unit)?;
                scale
            } else {
                let s = (hi - lo) / (levels - 1) as f32;
                require_normal_scale(s, unit)?;
                s
            };
            // `z` is compared in f64 because `i32::MAX as f32` rounds UP to
            // 2147483648 and would admit a zero point that does not fit.
            let z = f64::from(round_half_to_even(-lo / scale));
            if !(z >= f64::from(i32::MIN) && z <= f64::from(i32::MAX)) {
                return Err(QuantError::ZeroPointOutOfRange {
                    unit: unit.to_string(),
                    detail: format!(
                        "the unit needs zero point {z:.0}, which does not fit in i32; \
                         refused rather than wrapped"
                    ),
                });
            }
            Ok(QuantParams {
                scale,
                zero: z as i32,
            })
        }
    }
}

/// Quantise and dequantise one unit in one step, into a fresh buffer.
///
/// The signature `TASK.md` §Data Contracts fixes. It returns only the
/// dequantised values and never the integer codes: v1 diagnoses, it does not
/// emit a quantised model, and an API that cannot produce one cannot accidentally
/// be used to.
///
/// The result is a **simulation** ([`crate::Provenance::Simulated`]), and its
/// fidelity is [`crate::QuantFidelity::of_round_trip`] — `quantized` unless every
/// value came back bit-identical.
pub fn simulate(values: &[f32], params: &QuantParams, config: &QuantConfig) -> Result<Vec<f32>> {
    // Validate before allocating, so a rejected config costs nothing.
    config.validate()?;
    let mut out = vec![0.0f32; values.len()];
    simulate_into(values, params, config, &mut out)?;
    Ok(out)
}

/// [`simulate`] into a caller-provided buffer, so a streaming pass reuses one
/// buffer across every block instead of allocating per block.
///
/// A length mismatch between `values` and `out` is rejected **before** any
/// arithmetic runs.
pub fn simulate_into(
    values: &[f32],
    params: &QuantParams,
    config: &QuantConfig,
    out: &mut [f32],
) -> Result<()> {
    simulate_into_named(values, params, config, out, UnitId::UNNAMED)
}

/// [`simulate_into`], with a unit to name in a refusal.
pub fn simulate_into_named(
    values: &[f32],
    params: &QuantParams,
    config: &QuantConfig,
    out: &mut [f32],
    unit: UnitId<'_>,
) -> Result<()> {
    // A single parameter pair cannot express per-group granularity, and silently
    // applying it as if it were per-tensor would produce a plausible wrong
    // number. Refused for the same reason `simulate_per_group_into` refuses
    // `PerTensor`, and before any arithmetic.
    if let Granularity::PerGroup { size } = config.granularity {
        return Err(QuantError::config_rejected(format!(
            "one parameter pair cannot express per-group granularity at group size \
             {size}; use simulate_per_group_into, which derives a scale per group"
        )));
    }
    simulate_unit_into(values, params, config, out, unit)
}

/// The body of [`simulate_into_named`], without the per-group config guard.
///
/// [`simulate_per_group_into`] calls this because it has already split the unit
/// into groups and derived one parameter pair per group, so the guard that
/// protects a caller from applying one pair to a whole per-group tensor would
/// reject the very thing it is doing correctly.
fn simulate_unit_into(
    values: &[f32],
    params: &QuantParams,
    config: &QuantConfig,
    out: &mut [f32],
    unit: UnitId<'_>,
) -> Result<()> {
    // Everything that can be rejected is rejected before the first divide.
    config.validate()?;
    if values.len() != out.len() {
        return Err(QuantError::LengthMismatch {
            operation: "simulate_into",
            left: values.len(),
            right: out.len(),
        });
    }
    require_normal_scale(params.scale, unit)?;
    if config.zero_point == ZeroPoint::Symmetric && params.zero != 0 {
        return Err(QuantError::ZeroPointOutOfRange {
            unit: unit.to_string(),
            detail: format!(
                "a symmetric config carries no zero point, so it must be 0, not {}",
                params.zero
            ),
        });
    }
    require_finite(values, unit)?;

    let (qmin, qmax) = config.code_range();
    let zero = i64::from(params.zero);
    let zero_f = zero as f64;
    let qmin_f = qmin as f64;
    let qmax_f = qmax as f64;

    for (index, (dst, &x)) in out.iter_mut().zip(values).enumerate() {
        // q = clamp(round_half_to_even(x / s) + z, qmin, qmax)
        let rounded = round_half_to_even(x / params.scale);
        let code = (f64::from(rounded) + zero_f).clamp(qmin_f, qmax_f) as i64;
        // x̂ = (q - z) · s
        let reconstructed = (code - zero) as f32 * params.scale;
        if !reconstructed.is_finite() {
            // §3.1: "never silently produce infinities". The scale is normal and
            // the input is finite, but `(q - z) · s` overflowed — which can only
            // be detected here, per value, so `out` is left unspecified.
            return Err(QuantError::ReconstructionNotFinite {
                unit: unit.to_string(),
                index,
                code,
                scale: params.scale,
            });
        }
        *dst = reconstructed;
    }
    Ok(())
}

/// Derive parameters per group and simulate, group by group, into `out`.
///
/// Only for [`Granularity::PerGroup`]: per-tensor and per-channel parameters come
/// from the caller because they need a whole-axis view this crate never has.
///
/// The final group is **clamped, never padded** — the same rule
/// `BlockExtent::clamped_to` already applies. Allocates nothing.
pub fn simulate_per_group_into(
    values: &[f32],
    config: &QuantConfig,
    out: &mut [f32],
    unit: UnitId<'_>,
) -> Result<()> {
    config.validate()?;
    let size = match config.granularity {
        Granularity::PerGroup { size } => size,
        other => {
            return Err(QuantError::config_rejected(format!(
                "simulate_per_group_into needs per-group granularity; under {other:?} the \
                 parameters come from a whole-axis view this crate never has and must be \
                 supplied by the caller"
            )))
        }
    };
    if values.len() != out.len() {
        return Err(QuantError::LengthMismatch {
            operation: "simulate_per_group_into",
            left: values.len(),
            right: out.len(),
        });
    }
    for extent in group_extents(values.len(), size)? {
        let group = UnitId {
            tensor: unit.tensor,
            offset: unit.offset + extent.start as u64,
        };
        let params = derive_params_named(&values[extent.clone()], config, group)?;
        simulate_unit_into(
            &values[extent.clone()],
            &params,
            config,
            &mut out[extent],
            group,
        )?;
    }
    Ok(())
}

/// The group extents of an axis of `len` values at group size `size`.
///
/// An **iterator**, not a `Vec`: a 4096×4096 tensor at group size 128 has 131 072
/// groups, and materialising them would be an allocation proportional to tensor
/// size — the class of bug `QError::BudgetExceeded` exists to surface elsewhere
/// in this repository.
///
/// The final extent is clamped to `len`, never padded
/// (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1, last degenerate row).
pub fn group_extents(len: usize, size: u32) -> Result<GroupExtents> {
    if size == 0 {
        return Err(QuantError::config_rejected(
            "group size 0 would produce no groups; per-group granularity needs a positive size",
        ));
    }
    Ok(GroupExtents {
        len,
        size: size as usize,
        next_start: 0,
    })
}

/// The iterator [`group_extents`] returns. Holds three words and allocates
/// nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupExtents {
    len: usize,
    size: usize,
    next_start: usize,
}

impl GroupExtents {
    /// How many groups an axis of `len` values yields at group size `size`,
    /// counting the clamped final group. `0` when `len` is `0`.
    pub fn count_of(len: usize, size: u32) -> Result<usize> {
        if size == 0 {
            return Err(QuantError::config_rejected(
                "group size 0 would produce no groups; per-group granularity needs a positive size",
            ));
        }
        Ok(len.div_ceil(size as usize))
    }
}

impl Iterator for GroupExtents {
    type Item = std::ops::Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_start >= self.len {
            return None;
        }
        let start = self.next_start;
        // Clamped to `len`, never padded out to `size`.
        let end = start.saturating_add(self.size).min(self.len);
        self.next_start = end;
        Some(start..end)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.next_start).div_ceil(self.size);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for GroupExtents {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Precision, QuantFidelity, MAX_CODE_BITS, MAX_IMPLEMENTED_RANK};

    /// Every expected value in this module is computed **by hand** from
    /// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1, not by the code under test. The
    /// f32 bit patterns quoted were checked against
    /// `python/reference/quantise_reference.py`'s output, which was generated
    /// before this file existed.
    fn cfg(precision: Precision, zero_point: ZeroPoint) -> QuantConfig {
        QuantConfig::per_tensor(precision, zero_point)
    }

    // -- round_half_to_even -------------------------------------------------

    #[test]
    fn round_half_to_even_breaks_every_tie_towards_the_even_integer() {
        // NumPy `np.rint`: 0.5 -> 0, 1.5 -> 2, 2.5 -> 2, 3.5 -> 4.
        // Half-away-from-zero would give 1, 2, 3, 4 and disagree on two of four.
        assert_eq!(round_half_to_even(0.5), 0.0);
        assert_eq!(round_half_to_even(1.5), 2.0);
        assert_eq!(round_half_to_even(2.5), 2.0);
        assert_eq!(round_half_to_even(3.5), 4.0);
        assert_eq!(round_half_to_even(-1.5), -2.0);
        assert_eq!(round_half_to_even(-2.5), -2.0);
        // Non-ties are unaffected.
        assert_eq!(round_half_to_even(0.4999999), 0.0);
        assert_eq!(round_half_to_even(0.5000001), 1.0);
        assert_eq!(round_half_to_even(-3.25), -3.0);
    }

    #[test]
    fn round_half_to_even_preserves_the_sign_of_a_zero() {
        // `-0.5` rounds to NEGATIVE zero, exactly as `np.rint` does. Comparing
        // values would let this through, since `-0.0 == 0.0`; comparing bits
        // does not.
        assert_eq!(round_half_to_even(-0.5).to_bits(), (-0.0f32).to_bits());
        assert_eq!(round_half_to_even(0.5).to_bits(), 0.0f32.to_bits());
        assert_ne!(round_half_to_even(-0.5).to_bits(), 0.0f32.to_bits());
    }

    // -- hand computations --------------------------------------------------

    #[test]
    fn hand_computed_int8_symmetric_round_trip_of_minus_one_zero_one() {
        // HAND COMPUTATION 1 — `TASK.md` §Test Cases row 1.
        //   max|g| = 1.0;  n = 256;  n/2 - 1 = 127
        //   s = 1.0 / 127 -> nearest f32 is 0x3C010204 = 0.007874015718698502
        //       (the true value 0.0078740157480315 is 0.003 ULP away)
        //   x = -1: -1/s = -127.0000000473 -> nearest f32 is exactly -127.0
        //           rint(-127) = -127, inside [-128, 127], so q = -127
        //           x̂ = -127 · s = -0.999999996 -> nearest f32 is exactly -1.0
        //   x =  0: q = 0, x̂ = 0
        //   x =  1: q = 127, x̂ = 1.0
        // Every input is a code times the scale, so the round trip is EXACT.
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let values = [-1.0f32, 0.0, 1.0];
        let params = derive_params(&values, &config).unwrap();
        assert_eq!(params.scale.to_bits(), 0x3C01_0204);
        assert_eq!(params.zero, 0);

        let out = simulate(&values, &params, &config).unwrap();
        for (a, b) in values.iter().zip(&out) {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "expected {a} bit-exactly, got {b}"
            );
        }
        assert_eq!(
            QuantFidelity::of_round_trip(&values, &out).unwrap(),
            QuantFidelity::Exact
        );
    }

    #[test]
    fn hand_computed_int4_symmetric_round_trip_at_one_seventh() {
        // HAND COMPUTATION 2 — the first group of the 9-value per-group golden.
        //   g = [-1, 0.25, 0.5, 1];  max|g| = 1.0;  n = 16;  n/2 - 1 = 7
        //   s = 1/7 -> nearest f32 0x3E124925 = 0.14285714924335480, which is
        //       ABOVE the true 1/7 = 0.142857142857...  That sign matters:
        //   x = 0.5:  0.5/s = 3.4999999814 — just BELOW the 3.5 tie, so
        //             rint gives 3, NOT the 4 that exact decimal 1/7 would give.
        //             x̂ = 3·s = 0.4285714477
        //   x = 0.25: 0.25/s = 1.7499999907 -> 2;  x̂ = 2·s = 0.2857142985
        //   x = -1:   -1/s = -6.99999996 -> -7 (inside [-8, 7]); x̂ = -1.0 exactly
        //   x = 1:    7 -> x̂ = 7·s = 1.0000000447 -> nearest f32 is 1.0
        let config = cfg(Precision::Int4, ZeroPoint::Symmetric);
        let values = [-1.0f32, 0.25, 0.5, 1.0];
        let params = derive_params(&values, &config).unwrap();
        assert_eq!(params.scale.to_bits(), 0x3E12_4925);

        let out = simulate(&values, &params, &config).unwrap();
        assert_eq!(out[0].to_bits(), (-1.0f32).to_bits());
        assert_eq!(out[1].to_bits(), (2.0f32 * params.scale).to_bits());
        assert_eq!(out[2].to_bits(), (3.0f32 * params.scale).to_bits());
        assert_eq!(out[3].to_bits(), 1.0f32.to_bits());
        // 0.5 came back as 3·s, not 4·s: the tie never happened in f32.
        assert!(
            out[2] < 0.5,
            "0.5 must reconstruct BELOW itself, got {}",
            out[2]
        );
        assert_eq!(
            QuantFidelity::of_round_trip(&values, &out).unwrap(),
            QuantFidelity::Quantized
        );
    }

    #[test]
    fn hand_computed_int4_asymmetric_zero_point_is_itself_rounded() {
        // HAND COMPUTATION 3 — `TASK.md` §Test Cases row 2, and the case the
        // §Risks table warns about ("asymmetric zero-point off by one").
        //   f32(0.1) = 0.10000000149011612,  f32(0.3) = 0.30000001192092896
        //   hi - lo  = 0.20000001043081284 exactly, which is the MIDPOINT
        //              between f32 0x3E4CCCCD and 0x3E4CCCCE — a tie, broken to
        //              even, so it rounds UP to 0x3E4CCCCE = 0.20000001788139343
        //   s = that / 15  -> 0x3C5A740F = 0.0133333345875144
        //   -lo/s = -7.4999998 (NOT -7.5, because s came out slightly large)
        //   z = rint(-7.4999998) = -7        <- not the -8 exact decimals give
        //   x = 0.1: rint(7.4999998) = 7;  q = clamp(7 + (-7), 0, 15) = 0
        //            x̂ = (0 - (-7))·s = 7·s = 0.09333334118127823
        //   x = 0.2: rint(14.99999955) = 15; q = 8;  x̂ = 15·s = 0.2000000178
        //   x = 0.3: rint(22.4999993) = 22;  q = 15; x̂ = 22·s = 0.2933333516
        // The reconstructed minimum lands BELOW 0.1 and the maximum BELOW 0.3
        // because the grid is offset by the rounding of z itself. This is §3.1's
        // arithmetic, not an off-by-one.
        let config = cfg(Precision::Int4, ZeroPoint::Asymmetric);
        let values = [0.1f32, 0.2, 0.3];
        let params = derive_params(&values, &config).unwrap();
        assert_eq!(params.scale.to_bits(), 0x3C5A_740F);
        assert_eq!(
            params.zero, -7,
            "z is -7 in f32 arithmetic, not the decimal -8"
        );

        let out = simulate(&values, &params, &config).unwrap();
        assert_eq!(out[0].to_bits(), (7.0f32 * params.scale).to_bits());
        assert_eq!(out[1].to_bits(), (15.0f32 * params.scale).to_bits());
        assert_eq!(out[2].to_bits(), (22.0f32 * params.scale).to_bits());
        assert!(out[0] < 0.1, "the reconstructed minimum sits below 0.1");
    }

    #[test]
    fn hand_computed_asymmetric_clamping_at_both_ends_of_the_int4_code_range() {
        // HAND COMPUTATION 4 — params supplied rather than derived, which is the
        // per-tensor and per-channel case: the unit does not set its own scale,
        // so values outside the calibrated range CLAMP.
        //   s = 1, z = 8, code range [0, 15]
        //   x = -9: rint(-9) + 8 = -1 -> clamps to 0  -> x̂ = (0-8)·1 = -8
        //   x =  8: rint(8) + 8 = 16  -> clamps to 15 -> x̂ = (15-8)·1 = 7
        //   x = 50: 58 -> clamps to 15 -> x̂ = 7
        let config = cfg(Precision::Int4, ZeroPoint::Asymmetric);
        let params = QuantParams::new(1.0, 8);
        let values = [-9.0f32, -8.0, -1.0, 0.0, 1.0, 7.0, 8.0, 50.0];
        let out = simulate(&values, &params, &config).unwrap();
        assert_eq!(out, vec![-8.0, -8.0, -1.0, 0.0, 1.0, 7.0, 7.0, 7.0]);
    }

    #[test]
    fn hand_computed_symmetric_clamping_uses_the_asymmetric_negative_headroom() {
        // s = 1, code range [-8, 7] for int4 symmetric — one more level below
        // zero than above it, which is where an off-by-one would show.
        let config = cfg(Precision::Int4, ZeroPoint::Symmetric);
        let params = QuantParams::new(1.0, 0);
        let values = [7.0f32, 8.0, 9.0, 100.0, -8.0, -9.0, -12.0, 0.0];
        let out = simulate(&values, &params, &config).unwrap();
        assert_eq!(out, vec![7.0, 7.0, 7.0, 7.0, -8.0, -8.0, -8.0, 0.0]);
        assert_eq!(config.code_range(), (-8, 7));
        assert_eq!(
            QuantConfig::per_tensor(Precision::Int8, ZeroPoint::Symmetric).code_range(),
            (-128, 127)
        );
        assert_eq!(
            QuantConfig::per_tensor(Precision::Int8, ZeroPoint::Asymmetric).code_range(),
            (0, 255)
        );
        assert_eq!(config.code_range().0, -8);
    }

    #[test]
    fn the_rounding_boundaries_go_through_simulate_at_scale_one() {
        // Acceptance criterion 2's four named ties, reaching the clamp through
        // `simulate` rather than only through the helper.
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let params = QuantParams::new(1.0, 0);
        let values = [0.5f32, 1.5, -0.5, 2.5, -1.5, -2.5];
        let out = simulate(&values, &params, &config).unwrap();
        assert_eq!(out, vec![0.0, 2.0, 0.0, 2.0, -2.0, -2.0]);
        // -0.5 rounds to -0.0, and (0 - 0) as f32 is POSITIVE zero, so the
        // reconstruction is +0.0. Stated because it is easy to assume otherwise.
        assert_eq!(out[2].to_bits(), 0.0f32.to_bits());
    }

    // -- degenerate cases, one test each -----------------------------------

    #[test]
    fn an_all_zero_unit_gets_scale_one_and_reconstructs_to_zero_without_dividing_by_zero() {
        for zero_point in [ZeroPoint::Symmetric, ZeroPoint::Asymmetric] {
            let config = cfg(Precision::Int8, zero_point);
            let values = [0.0f32; 3];
            let params = derive_params(&values, &config).unwrap();
            assert_eq!(params.scale, 1.0, "scale must be 1, not 0 ({zero_point:?})");
            assert_eq!(params.zero, 0);
            let out = simulate(&values, &params, &config).unwrap();
            assert_eq!(out, vec![0.0, 0.0, 0.0]);
            assert!(out.iter().all(|v| !v.is_nan()), "no NaN may appear");
            assert!(out.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn a_constant_non_integer_unit_round_trips_exactly_under_both_zero_points() {
        // Zero dynamic range, non-zero. The rule is `s = |c|`, NOT `s = 1`:
        // §3.1's formulas at s = 1 send a constant 0.5 to 0.0 and a constant
        // 0.823457 to 1.0, silently, while `symmetric` reconstructs both exactly.
        // Two modes disagreeing on a constant tensor is the tell, and a constant
        // bias or norm weight is common enough that it would be a real wrong
        // number rather than a corner case.
        //
        //   c > 0:  s = c;   z = rint(-c/c) = -1;  q = clamp(rint(1) - 1, 0, n-1) = 0
        //           x̂ = (0 - (-1))·c = c            exactly
        //   c < 0:  s = |c|; z = rint(-c/|c|) = +1; q = clamp(rint(-1) + 1, 0, n-1) = 0
        //           x̂ = (0 - 1)·|c| = c             exactly
        // Under ASYMMETRIC the rule guarantees exactness at every magnitude,
        // including f32::MAX, because the reachable code is 0 or ∓1.
        for precision in [Precision::Int8, Precision::Int4] {
            for c in [
                1.0f32,
                0.5,
                -0.3,
                0.823457,
                2.0,
                -1.0,
                -4e9,
                1e38,
                f32::MAX,
                -f32::MAX,
            ] {
                let config = cfg(precision, ZeroPoint::Asymmetric);
                let values = [c; 6];
                let params = derive_params(&values, &config)
                    .unwrap_or_else(|e| panic!("{c} under Asymmetric/{precision:?} refused: {e}"));
                assert_eq!(
                    params.scale.to_bits(),
                    c.abs().to_bits(),
                    "the scale of a constant unit is |c|, not 1"
                );
                let out = simulate(&values, &params, &config).unwrap();
                for v in &out {
                    assert_eq!(
                        v.to_bits(),
                        c.to_bits(),
                        "a constant unit at {c} must reconstruct bit-exactly under \
                         Asymmetric/{precision:?}; got {v}"
                    );
                }
                assert_eq!(
                    QuantFidelity::of_round_trip(&values, &out).unwrap(),
                    QuantFidelity::Exact,
                    "for {c} under Asymmetric/{precision:?}"
                );
            }
        }
        // Under SYMMETRIC the scale is max|g|/(n/2−1) and exactness depends on
        // `(n/2−1) · s` recovering `c`. It does at these magnitudes, and the point
        // of listing them is that the two modes now AGREE where they used to
        // disagree — 0.5 and 0.823457 were the tell.
        for precision in [Precision::Int8, Precision::Int4] {
            for c in [1.0f32, 0.5, -0.3, 0.823457, 2.0, -1.0, 1e38] {
                let config = cfg(precision, ZeroPoint::Symmetric);
                let values = [c; 6];
                let params = derive_params(&values, &config).unwrap();
                let out = simulate(&values, &params, &config).unwrap();
                for v in &out {
                    assert_eq!(
                        v.to_bits(),
                        c.to_bits(),
                        "a constant unit at {c} must reconstruct bit-exactly under \
                         Symmetric/{precision:?}; got {v}"
                    );
                }
            }
        }
        // The zero point is what makes the sign work, and it is derived from
        // §3.1's unchanged formula rather than special-cased.
        let asym4 = cfg(Precision::Int4, ZeroPoint::Asymmetric);
        assert_eq!(derive_params(&[0.5f32; 8], &asym4).unwrap().scale, 0.5);
        assert_eq!(derive_params(&[0.5f32; 8], &asym4).unwrap().zero, -1);
        assert_eq!(derive_params(&[-0.3f32; 8], &asym4).unwrap().zero, 1);
        // And the all-zero unit keeps §3.1's tabulated s = 1, z = 0.
        assert_eq!(derive_params(&[0.0f32; 8], &asym4).unwrap().scale, 1.0);
        assert_eq!(derive_params(&[0.0f32; 8], &asym4).unwrap().zero, 0);
    }

    #[test]
    fn a_large_but_valid_zero_point_survives_the_rounding_i64_to_f32_conversion() {
        // The only path where `(q - z) as f32` actually ROUNDS. Every other
        // accepted case has |z| <= 8, so the conversion is exact and this branch
        // of the arithmetic is never exercised.
        //
        // A huge offset with a two-ULP range gives z = 1687448320, which FITS in
        // i32 — the one-ULP version overflows and is refused
        // (`a_zero_point_outside_i32_is_refused_rather_than_wrapped`). With
        // |q - z| > 2^24 the i64 -> f32 cast rounds, and Rust rounds ties to even
        // exactly as NumPy's int64 -> float32 does. Asserted bit-for-bit against
        // the reference in the golden
        // `large_but_valid_zero_point_int8_asymmetric`, and here directly.
        let config = cfg(Precision::Int8, ZeroPoint::Asymmetric);
        let lo = -1e30f32;
        let mid = f32::from_bits(lo.to_bits() - 1);
        let hi = f32::from_bits(lo.to_bits() - 2);
        let values = [lo, mid, hi];
        let params = derive_params(&values, &config).unwrap();
        assert_eq!(
            params.zero, 1_687_448_320,
            "the zero point must be the large one"
        );
        assert!(
            i64::from(params.zero) > 1 << 24,
            "the point of this test is that |q - z| exceeds 2^24 and the cast rounds"
        );
        let out = simulate(&values, &params, &config).unwrap();
        for (a, b) in values.iter().zip(&out) {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "a large zero point must not perturb the round trip; {a} became {b}"
            );
        }
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn a_reconstruction_that_would_be_infinite_is_refused_rather_than_emitted() {
        // §3.1's "never silently produce infinities", which a normal-scale check
        // alone does NOT deliver: here the scale is a perfectly normal
        // f32::MAX/127 and every input is finite, but 127·s rounds UP past
        // f32::MAX. Symmetric int8 at f32::MAX is the smallest real example.
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        for c in [f32::MAX, -f32::MAX] {
            let values = [c; 4];
            let params = derive_params(&values, &config).unwrap();
            assert!(
                params.scale.is_normal(),
                "the scale itself is fine; it is the product that overflows"
            );
            let mut out = vec![0.0f32; values.len()];
            let err =
                simulate_into_named(&values, &params, &config, &mut out, UnitId::new("w", 64))
                    .unwrap_err();
            assert_eq!(err.kind(), "reconstruction_not_finite", "for {c}");
            let message = err.to_string();
            assert!(
                message.contains("w") && message.contains("64"),
                "got {message}"
            );
            assert!(message.contains("overflows f32"), "got {message}");
            // And nothing infinite escaped into the buffer.
            assert!(out.iter().all(|v| v.is_finite()));
        }
        // int4 at the same magnitude does NOT overflow — the reachable code is 7,
        // not 127 — so it must succeed rather than be refused defensively.
        let int4 = cfg(Precision::Int4, ZeroPoint::Symmetric);
        let values = [-f32::MAX; 4];
        let params = derive_params(&values, &int4).unwrap();
        let out = simulate(&values, &params, &int4).unwrap();
        assert_eq!(out[0].to_bits(), (-f32::MAX).to_bits());
        // Reached from supplied params too. s = 2e38, x = f32::MAX -> code 2,
        // 2·s = 4e38, overflow. A scale of f32::MAX itself would NOT reach this:
        // every finite input then rounds to code 0.
        let mut out = [0.0f32; 2];
        let err = simulate_into_named(
            &[f32::MAX, -f32::MAX],
            &QuantParams::new(2e38, 0),
            &int4,
            &mut out,
            UnitId::new("w", 0),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "reconstruction_not_finite");
        // A scale of f32::MAX with ORDINARY inputs does not reach the overflow:
        // x/s underflows, the code is 0, and the reconstruction is 0.0. Stated
        // because it is the case a defensive params-only check would wrongly
        // refuse.
        let out = simulate(&[-1.0, 1.0], &QuantParams::new(f32::MAX, 0), &int4).unwrap();
        assert_eq!(
            out,
            vec![0.0, 0.0],
            "code 0 at a huge scale reconstructs to 0"
        );
    }

    #[test]
    fn one_parameter_pair_cannot_be_applied_to_a_per_group_config() {
        // The inverse of `per_group_simulation_refuses_a_granularity_whose_
        // parameters_it_cannot_derive`. Applying one scale to a config that asks
        // for one scale per group would silently produce per-tensor numbers under
        // a per-group label — a plausible wrong number, which is worse than a
        // refusal. `QM-0122` constructs configs and parameters separately, so this
        // is the mistake it can actually make.
        let config = QuantConfig::per_group(Precision::Int8, ZeroPoint::Symmetric, 4);
        let values = [1.0f32, 2.0, 3.0, 4.0, 5.0];
        let mut out = vec![f32::NAN; values.len()];
        let err = simulate_into(&values, &QuantParams::IDENTITY, &config, &mut out).unwrap_err();
        assert_eq!(err.kind(), "config_rejected");
        assert!(
            err.to_string().contains("simulate_per_group_into"),
            "the refusal must name the function that does work; got {err}"
        );
        assert!(out.iter().all(|v| v.is_nan()), "refused before execution");
        assert!(simulate(&values, &QuantParams::IDENTITY, &config).is_err());
        // And the per-group entry point still works on the same config.
        simulate_per_group_into(&values, &config, &mut out, UnitId::UNNAMED).unwrap();
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn a_constant_subnormal_unit_refuses_under_both_zero_points() {
        // A consequence of `s = |c|` worth stating: a constant subnormal unit now
        // refuses on §3.1's existing subnormal-scale row, which is what
        // `symmetric` already did with the same input. Under an `s = 1` rule the
        // asymmetric path silently reconstructed it to 0.0.
        for zero_point in [ZeroPoint::Symmetric, ZeroPoint::Asymmetric] {
            let config = cfg(Precision::Int4, zero_point);
            let err =
                derive_params_named(&[1e-45f32; 4], &config, UnitId::new("norm", 3)).unwrap_err();
            assert_eq!(err.kind(), "scale_not_normal", "for {zero_point:?}");
            assert!(err.to_string().contains("norm"), "got {err}");
        }
    }

    #[test]
    fn a_scale_that_underflows_to_subnormal_is_refused_and_the_unit_is_named() {
        // `TASK.md` §Test Cases row 4: max|g| is the smallest f32 subnormal, so
        // s = max|g|/127 underflows to zero.
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let values = [1e-45f32, 0.0];
        let err = derive_params_named(&values, &config, UnitId::new("layers.0.q_proj.weight", 512))
            .unwrap_err();
        assert_eq!(err.kind(), "scale_not_normal");
        let message = err.to_string();
        assert!(
            message.contains("layers.0.q_proj.weight") && message.contains("512"),
            "the refusal must name the tensor and offset; got {message:?}"
        );
        assert!(
            message.contains("infinities"),
            "the refusal must say why; got {message:?}"
        );
    }

    #[test]
    fn a_scale_that_overflows_to_infinity_is_refused_rather_than_emitting_infinities() {
        // max - min overflows f32. §3.1: "never silently produce infinities".
        let config = cfg(Precision::Int8, ZeroPoint::Asymmetric);
        let values = [f32::MAX, -f32::MAX];
        let err = derive_params_named(&values, &config, UnitId::new("t", 0)).unwrap_err();
        assert_eq!(err.kind(), "scale_not_normal");
    }

    #[test]
    fn a_non_finite_value_refuses_the_tensor_rather_than_propagating() {
        // `TASK.md` §Test Cases row 5. NaN, +Inf and -Inf all refuse, and the
        // refusal names the tensor because a non-finite weight is a finding.
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let values = [1.0f32, bad];
            let err = derive_params_named(&values, &config, UnitId::new("mlp.down.weight", 8))
                .unwrap_err();
            assert_eq!(err.kind(), "non_finite", "for {bad}");
            let message = err.to_string();
            assert!(
                message.contains("mlp.down.weight") && message.contains("index 1"),
                "got {message:?}"
            );
            assert!(message.contains("finding"), "got {message:?}");
            // And it refuses in `simulate` too: params may come from elsewhere.
            let mut out = [0.0f32; 2];
            let err = simulate_into_named(
                &values,
                &QuantParams::IDENTITY,
                &config,
                &mut out,
                UnitId::new("mlp.down.weight", 8),
            )
            .unwrap_err();
            assert_eq!(err.kind(), "non_finite");
        }
    }

    #[test]
    fn a_zero_point_outside_i32_is_refused_rather_than_wrapped() {
        // A huge offset with a one-ULP range: lo = -1e30 and hi its neighbour
        // toward zero, so s = ulp(1e30)/255 = 2.96e20 and z = rint(1e30/s) =
        // 3374896640, outside i32 (max 2147483647).
        //
        // NOT a constant unit. Under the `s = |c|` rule a constant reconstructs
        // exactly and z is ∓1, so constants no longer reach this path at all —
        // which is why this test names a two-value unit instead.
        let config = cfg(Precision::Int8, ZeroPoint::Asymmetric);
        let lo = -1e30f32;
        let hi = f32::from_bits(lo.to_bits() - 1); // one ULP toward zero
        assert!(
            hi > lo && hi - lo > 0.0,
            "the range must be exactly one ULP"
        );
        let err = derive_params_named(&[lo, hi], &config, UnitId::new("big", 1)).unwrap_err();
        assert_eq!(err.kind(), "zero_point_out_of_range");
        let message = err.to_string();
        assert!(message.contains("3374896640"), "got {message:?}");
        assert!(message.contains("big"), "got {message:?}");
        assert!(message.contains("i32"), "got {message:?}");
        // A constant at the same magnitude is exact rather than refused.
        let params = derive_params(&[-4e9f32; 4], &config).unwrap();
        assert_eq!(params.zero, 1);
        assert_eq!(
            simulate(&[-4e9f32; 4], &params, &config).unwrap()[0].to_bits(),
            (-4e9f32).to_bits()
        );
    }

    #[test]
    fn a_symmetric_config_refuses_a_non_zero_zero_point_before_any_arithmetic() {
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let mut out = [0.0f32; 2];
        let err = simulate_into_named(
            &[1.0, 2.0],
            &QuantParams::new(1.0, 3),
            &config,
            &mut out,
            UnitId::new("t", 0),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "zero_point_out_of_range");
        // Nothing was written: the refusal preceded execution.
        assert_eq!(out, [0.0, 0.0]);
    }

    #[test]
    fn dividing_by_a_zero_or_negative_scale_is_refused_not_attempted() {
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let mut out = [0.0f32; 2];
        for scale in [
            0.0f32,
            -1.0,
            f32::MIN_POSITIVE / 2.0,
            f32::INFINITY,
            f32::NAN,
        ] {
            let err = simulate_into_named(
                &[1.0, 2.0],
                &QuantParams::new(scale, 0),
                &config,
                &mut out,
                UnitId::new("t", 7),
            )
            .unwrap_err();
            assert_eq!(err.kind(), "scale_not_normal", "for scale {scale}");
            assert_eq!(out, [0.0, 0.0], "nothing may be written for scale {scale}");
        }
    }

    #[test]
    fn an_empty_unit_is_refused_rather_than_given_a_fabricated_scale() {
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let err = derive_params_named(&[], &config, UnitId::new("t", 0)).unwrap_err();
        assert_eq!(err.kind(), "empty_unit");
        // `simulate` over an empty unit is a no-op, not an error: the caller
        // already holds valid parameters, and there is nothing to reconstruct.
        assert!(simulate(&[], &QuantParams::IDENTITY, &config)
            .unwrap()
            .is_empty());
    }

    // -- shape and length mismatch, rejected before execution ---------------

    #[test]
    fn a_length_mismatch_between_input_and_output_is_rejected_before_execution() {
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let mut out = [f32::NAN; 2];
        let err =
            simulate_into(&[1.0, 2.0, 3.0], &QuantParams::IDENTITY, &config, &mut out).unwrap_err();
        assert_eq!(err.kind(), "length_mismatch");
        assert!(err.to_string().contains("3"), "got {err}");
        // Untouched: the check precedes the loop.
        assert!(out.iter().all(|v| v.is_nan()));
    }

    // -- per-group granularity ---------------------------------------------

    #[test]
    fn a_group_size_that_does_not_divide_the_axis_clamps_the_final_group() {
        // `TASK.md` §Test Cases row 6 and acceptance criterion 4.
        let extents: Vec<_> = group_extents(130, 128).unwrap().collect();
        assert_eq!(extents, vec![0..128, 128..130]);
        assert_eq!(extents[0].len(), 128);
        assert_eq!(extents[1].len(), 2, "clamped to 2, never padded to 128");
        assert_eq!(extents.iter().map(|e| e.len()).sum::<usize>(), 130);
        assert_eq!(GroupExtents::count_of(130, 128).unwrap(), 2);

        // 9 at 4 -> 4 + 4 + 1.
        let extents: Vec<_> = group_extents(9, 4).unwrap().collect();
        assert_eq!(extents, vec![0..4, 4..8, 8..9]);
        assert_eq!(GroupExtents::count_of(9, 4).unwrap(), 3);

        // A group larger than the axis is one clamped group, not a refusal.
        assert_eq!(
            group_extents(3, 128).unwrap().collect::<Vec<_>>(),
            vec![0..3]
        );
        // An empty axis yields no groups at all.
        assert_eq!(group_extents(0, 128).unwrap().count(), 0);
        assert_eq!(GroupExtents::count_of(0, 128).unwrap(), 0);
        // Exact division has no short final group.
        assert_eq!(
            group_extents(8, 4).unwrap().collect::<Vec<_>>(),
            vec![0..4, 4..8]
        );
    }

    #[test]
    fn group_size_zero_is_refused_at_config_validation_before_any_arithmetic() {
        assert_eq!(group_extents(130, 0).unwrap_err().kind(), "config_rejected");
        assert_eq!(
            GroupExtents::count_of(130, 0).unwrap_err().kind(),
            "config_rejected"
        );
        let config = QuantConfig::per_group(Precision::Int8, ZeroPoint::Symmetric, 0);
        assert_eq!(config.validate().unwrap_err().kind(), "config_rejected");
        // And nothing downstream of validation runs.
        assert_eq!(
            derive_params(&[1.0, 2.0], &config).unwrap_err().kind(),
            "config_rejected"
        );
    }

    #[test]
    fn each_clamped_group_derives_its_own_scale_from_its_own_values() {
        // 9 values at group size 4, int4 symmetric. max|g| per group is 1, 4,
        // 0.5, so the scales are 1/7, 4/7 and 0.5/7 — the final one-element
        // group is exactly representable at its own scale.
        let config = QuantConfig::per_group(Precision::Int4, ZeroPoint::Symmetric, 4);
        let values = [-1.0f32, 0.25, 0.5, 1.0, -4.0, 2.0, 0.0, 1.0, -0.5];
        let mut out = vec![f32::NAN; values.len()];
        simulate_per_group_into(&values, &config, &mut out, UnitId::new("t", 0)).unwrap();

        let scales: Vec<f32> = group_extents(values.len(), 4)
            .unwrap()
            .map(|e| derive_params(&values[e], &config).unwrap().scale)
            .collect();
        assert_eq!(scales[0].to_bits(), (1.0f32 / 7.0).to_bits());
        assert_eq!(scales[1].to_bits(), (4.0f32 / 7.0).to_bits());
        assert_eq!(scales[2].to_bits(), (0.5f32 / 7.0).to_bits());
        // The one-element final group round-trips exactly at its own scale.
        assert_eq!(out[8].to_bits(), (-0.5f32).to_bits());
        // A single scale over all nine values would NOT reproduce this: -0.5
        // under the whole-unit scale 4/7 would land on -0.5714... instead.
        let whole = derive_params(
            &values,
            &QuantConfig::per_tensor(Precision::Int4, ZeroPoint::Symmetric),
        )
        .unwrap();
        let whole_out = simulate(
            &values,
            &whole,
            &QuantConfig::per_tensor(Precision::Int4, ZeroPoint::Symmetric),
        )
        .unwrap();
        assert_ne!(whole_out[8].to_bits(), out[8].to_bits());
    }

    #[test]
    fn per_group_simulation_refuses_a_granularity_whose_parameters_it_cannot_derive() {
        for granularity in [Granularity::PerTensor, Granularity::PerOutputChannel] {
            let config = QuantConfig {
                precision: Precision::Int8,
                granularity,
                zero_point: ZeroPoint::Symmetric,
                round: crate::RoundMode::NearestEven,
            };
            let mut out = [0.0f32; 2];
            let err = simulate_per_group_into(&[1.0, 2.0], &config, &mut out, UnitId::UNNAMED)
                .unwrap_err();
            assert_eq!(err.kind(), "config_rejected", "for {granularity:?}");
            assert!(
                err.to_string().contains("supplied by the caller"),
                "got {err}"
            );
        }
    }

    // -- idempotence --------------------------------------------------------

    #[test]
    fn round_tripping_an_already_representable_value_is_idempotent() {
        // Acceptance criterion 7 and `TASK.md` §Test Cases row 7: every value is
        // a code times the scale, so quantising twice changes nothing.
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let s = 4.0f32 / 127.0;
        let values: Vec<f32> = (-127..=0).map(|k| k as f32 * s).collect();
        let params = derive_params(&values, &config).unwrap();
        let once = simulate(&values, &params, &config).unwrap();
        for (a, b) in values.iter().zip(&once) {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "{a} did not survive the round trip"
            );
        }
        // Applying it again is a fixed point, with the SAME derived parameters.
        let reparams = derive_params(&once, &config).unwrap();
        assert_eq!(reparams.scale.to_bits(), params.scale.to_bits());
        let twice = simulate(&once, &reparams, &config).unwrap();
        for (a, b) in once.iter().zip(&twice) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        assert_eq!(
            QuantFidelity::of_round_trip(&values, &twice).unwrap(),
            QuantFidelity::Exact
        );
    }

    #[test]
    fn a_second_pass_over_a_lossy_reconstruction_is_also_a_fixed_point() {
        // Not the same claim as idempotence on exact values: after one pass the
        // values ARE codes times the scale, so a second pass must change nothing
        // even where the first pass lost information.
        let config = cfg(Precision::Int4, ZeroPoint::Symmetric);
        let values = [-0.75f32, -0.25, 0.0, 0.125, 0.5, 1.0];
        let params = derive_params(&values, &config).unwrap();
        let once = simulate(&values, &params, &config).unwrap();
        assert_eq!(
            QuantFidelity::of_round_trip(&values, &once).unwrap(),
            QuantFidelity::Quantized,
            "this input is NOT representable; the first pass must be lossy"
        );
        let twice = simulate(&once, &params, &config).unwrap();
        for (a, b) in once.iter().zip(&twice) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    // -- the unnamed unit --------------------------------------------------

    #[test]
    fn the_contract_signatures_still_refuse_but_cannot_name_the_tensor() {
        let config = cfg(Precision::Int8, ZeroPoint::Symmetric);
        let err = derive_params(&[f32::NAN], &config).unwrap_err();
        assert_eq!(err.kind(), "non_finite");
        assert!(
            err.to_string().contains("<unnamed unit>"),
            "the contract signature has no name to give; got {err}"
        );
        assert_eq!(UnitId::UNNAMED.offset, 0);
        assert_eq!(
            UnitId::new("t", 9).to_string(),
            "tensor t at offset 9",
            "the naming format a refusal embeds"
        );
    }

    #[test]
    fn the_widest_code_the_v1_surface_stores_is_eight_bits() {
        // §3.1's last error row: `n` must fit the stored code range, checked at
        // config validation. Both closed variants satisfy it, and both are
        // spelled here so a future third variant cannot slip past unnoticed.
        for precision in [Precision::Int8, Precision::Int4] {
            assert!(precision.bits() <= MAX_CODE_BITS, "{precision:?}");
            assert_eq!(
                precision.levels(),
                1u32 << precision.bits(),
                "{precision:?}"
            );
            assert!(precision.bits() >= 2, "{precision:?}");
            QuantConfig::per_tensor(precision, ZeroPoint::Symmetric)
                .validate()
                .unwrap();
        }
        assert_eq!(MAX_IMPLEMENTED_RANK, 3);
    }
}
