//! Paired block reduction — a base block against a counterpart block.
//!
//! `QUANT-002`, `V1-11`. Specified by
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4 and
//! `.plan/tasks/QM-0121-paired-block-reduction/TASK.md`.
//!
//! ## What the counterpart is
//!
//! **Anything.** The counterpart is a second block of the same shape, and this
//! module never asks where it came from. Three consumers are already named in
//! the architecture and all three are the same kernel with a different second
//! operand:
//!
//! | consumer | base | counterpart |
//! | --- | --- | --- |
//! | v1 error diagnostic (`QUANT-002`) | a checkpoint block | a simulated reconstruction of it |
//! | checkpoint diff (`DIFF-001`) | one checkpoint's block | another checkpoint's block |
//! | expert-pair comparison (`MOE-001`) | one expert's block | a sibling expert's block |
//!
//! Nothing in the public signature names any of them, which is acceptance
//! criterion 7 of `QM-0121` and the reason `q-gpu` does not depend on `q-quant`.
//!
//! ## Everything here is a partial; nothing is a finished metric
//!
//! There is no RMSE, no relative error and no norm in [`PairedPartials`], and
//! adding one would be a defect rather than a convenience. Sums of squares
//! compose across blocks by addition; a root does not. Computing a finished
//! metric per block and averaging the results is, in the words of
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.1, *"the single most likely correctness
//! bug in this engine"*. The aggregation layer (`QM-0123`) takes the root, once,
//! at the top.
//!
//! ## Numerics
//!
//! Inputs are `f32`; every accumulator is `f64`. Accumulation is single-threaded
//! and in a fixed row-major order, because floating-point addition is not
//! associative and `V1-13` requires byte-identical output across runs. There is
//! no parallel reduction here and there must not be one added: a faster wrong
//! answer that changes between runs is worse than a slower right one.
//!
//! ## Allocation
//!
//! One `Vec<ChannelPartials>`, of length equal to the channel count of the
//! chosen axis: `channels × 48 B` — for a 256-column block, 12 288 bytes,
//! whatever the block's element count. **Nothing scales with the element
//! count.** Both input blocks are borrowed and never copied.
//!
//! That is the whole of the *scaling* term, but not quite the whole of the
//! allocation: [`Backend::check_workload`] calls `capabilities()`, which builds
//! two short `String`s on every call. It is a constant, it is under half a
//! kilobyte, and it is unaffected by either the channel count or the element
//! count — but it is not nothing, and saying "nothing else" would be an
//! overstatement. `crates/q-gpu/tests/paired_allocation_bounds.rs` measures the
//! scaling term by difference rather than asserting an absolute, precisely so
//! that this constant cannot hide an element-proportional term behind it.

use q_source::error::{QError, Result};

use crate::{Backend, BlockData, CpuBackend, Workload};

/// Which axis of a rank-2 block indexes the channels.
///
/// The variant names the axis the channels *lie along*, not the axis summed
/// over: [`ChannelAxis::Rows`] produces one [`ChannelPartials`] **per row**, so
/// `per_channel.len() == block.rows`. Stated this bluntly because a silently
/// transposed channel axis is the risk `QM-0121` acceptance criterion 2 exists
/// to catch, and `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.2 requires the answer to
/// change when the axis does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelAxis {
    /// One channel per row: `per_channel[i]` covers row `i`.
    ///
    /// This is the output-channel axis of a row-major `[out_features,
    /// in_features]` projection, which is how SafeTensors stores one.
    Rows,
    /// One channel per column: `per_channel[j]` covers column `j`.
    Columns,
}

impl ChannelAxis {
    /// Bind a numeric axis index against a tensor's rank, or refuse.
    ///
    /// Callers that hold a `TensorDescriptor` have an axis *number*, not a
    /// variant, and an out-of-range number must be refused rather than clamped
    /// into one. The rules, in the order they are applied:
    ///
    /// | rank | outcome |
    /// | --- | --- |
    /// | `> 3` | refuses with `GRID-007` — ADR-010: *rank above 3 refuses rather than flattens* |
    /// | `!= 2` | refuses, as [`q_tensor_runtime::BlockExtent::clamped_to`] already refuses a non-rank-2 shape |
    /// | `2`, axis `>= 2` | refuses, naming the rank |
    /// | `2`, axis `0` | [`ChannelAxis::Rows`] |
    /// | `2`, axis `1` | [`ChannelAxis::Columns`] |
    ///
    /// ADR-010 is honoured rather than reimplemented: rank 3 *is* within the
    /// implemented ceiling for axis binding, but a block is a 2-D slice today
    /// (`BlockExtent` gains an optional depth extent under `QM-0040`), so rank 3
    /// refuses here for the same reason the block planner does — not by
    /// flattening it into something confidently wrong.
    pub fn from_index(axis: usize, rank: usize) -> Result<Self> {
        if rank > 3 {
            return Err(QError::NotImplemented {
                requirement: "GRID-007",
                detail: format!(
                    "rank {rank} is above the implemented ceiling of 3 (ADR-010); \
                     axis binding refuses rather than flattening"
                ),
            });
        }
        if rank != 2 {
            return Err(QError::QueryRejected(format!(
                "channel axes are indexed against rank-2 blocks; got rank {rank}"
            )));
        }
        match axis {
            0 => Ok(ChannelAxis::Rows),
            1 => Ok(ChannelAxis::Columns),
            _ => Err(QError::QueryRejected(format!(
                "channel axis {axis} is out of range for a rank-{rank} tensor"
            ))),
        }
    }

    /// How many channels this axis has in a `rows × columns` block.
    pub fn channel_count(self, rows: usize, columns: usize) -> usize {
        match self {
            ChannelAxis::Rows => rows,
            ChannelAxis::Columns => columns,
        }
    }
}

/// Partials for one channel. Sums only — see the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ChannelPartials {
    /// Elements this channel contributed.
    pub count: u64,
    /// `Σ w²` — the denominator of a relative error, never the error itself.
    pub sum_sq_base: f64,
    /// `Σ (w − ŵ)²` — the numerator; `‖·‖_F²` *before* the root.
    pub sum_sq_delta: f64,
    /// `Σ |w − ŵ|`.
    pub sum_abs_delta: f64,
    /// `max |w − ŵ|`. Composes across blocks by maximum, not by addition.
    pub max_abs_delta: f64,
    /// `max |w|`, for outlier attribution.
    pub max_abs_base: f64,
}

impl ChannelPartials {
    /// The identity for this accumulator: an empty channel.
    pub const ZERO: ChannelPartials = ChannelPartials {
        count: 0,
        sum_sq_base: 0.0,
        sum_sq_delta: 0.0,
        sum_abs_delta: 0.0,
        max_abs_delta: 0.0,
        max_abs_base: 0.0,
    };

    /// Fold one already-validated pair of values in.
    ///
    /// `base` and `delta` arrive as `f64` because the caller widened them once
    /// and uses them for both the whole-block and the per-channel accumulator.
    #[inline]
    fn absorb(&mut self, base: f64, delta: f64) {
        let abs_delta = delta.abs();
        let abs_base = base.abs();
        self.count += 1;
        self.sum_sq_base += base * base;
        self.sum_sq_delta += delta * delta;
        self.sum_abs_delta += abs_delta;
        if abs_delta > self.max_abs_delta {
            self.max_abs_delta = abs_delta;
        }
        if abs_base > self.max_abs_base {
            self.max_abs_base = abs_base;
        }
    }
}

/// Whole-block partials plus one [`ChannelPartials`] per channel.
///
/// `per_channel.len()` is the channel count of the axis the reduction was asked
/// for, and it is the *only* thing here that depends on the axis: the
/// whole-block fields are accumulated in row-major element order regardless of
/// the axis, so they are bit-identical whichever axis was requested
/// (`the_whole_block_partials_are_bit_identical_whichever_axis_is_requested`).
/// That matters because `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.2 builds the
/// tensor-level figure out of these whole-block fields, and a tensor RMSE that
/// shifted in the last bit according to which channel axis a caller happened to
/// ask for would be a reproducibility defect.
///
/// **Do not reconstruct the whole-block fields by re-summing `per_channel`.**
/// They agree exactly only when the sums are exact; in general the two
/// groupings associate the additions differently and differ in the last ulp,
/// and floating-point addition is not associative. The whole-block fields are
/// the ones to compose across blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct PairedPartials {
    /// Elements in the block. `rows × columns`.
    pub count: u64,
    /// `Σ w²` over the whole block.
    pub sum_sq_base: f64,
    /// `Σ (w − ŵ)²` over the whole block.
    pub sum_sq_delta: f64,
    /// `Σ |w − ŵ|` over the whole block.
    pub sum_abs_delta: f64,
    /// `max |w − ŵ|` over the whole block.
    pub max_abs_delta: f64,
    /// `max |w|` over the whole block.
    pub max_abs_base: f64,
    /// One entry per channel of the requested axis, in axis order.
    pub per_channel: Vec<ChannelPartials>,
}

/// Refuse a pair of blocks whose shapes disagree, or that hold no elements.
///
/// `O(1)` — it reads four `usize` fields and no element. Both refusals precede
/// every read of `values`, which is what `TASK.md` §Error Handling means by
/// *"before reading any value"*.
fn validate_pair(base: &BlockData, counterpart: &BlockData) -> Result<()> {
    if base.rows != counterpart.rows || base.columns != counterpart.columns {
        return Err(QError::QueryRejected(format!(
            "paired reduction shape mismatch: base is [{}, {}], counterpart is [{}, {}] \
             — a paired reduction requires identical shapes",
            base.rows, base.columns, counterpart.rows, counterpart.columns
        )));
    }
    if base.rows == 0 || base.columns == 0 {
        return Err(QError::QueryRejected(format!(
            "paired reduction refuses an empty block: [{}, {}] holds no elements, \
             and an empty reduction has no meaningful partials",
            base.rows, base.columns
        )));
    }
    Ok(())
}

/// Refuse a block whose value count disagrees with the shape it declares.
///
/// `BlockData::new` rejects a ragged block, but `rows`, `columns` and `values`
/// are all `pub`, so a block whose declared shape outruns its buffer reaches
/// this kernel from safe code. Without this check the accumulation loop indexes
/// past the end and **panics**. A panic is not a refusal: it carries no
/// requirement ID, cannot be reported to a caller that was told the operation
/// merely might fail, and `.plan/DIAGNOSTIC_ARCHITECTURE.md` §8's
/// refuse-rather-than-fabricate rule wants an error either way.
///
/// Deliberately ordered **after** `check_workload`: a declared shape can be
/// enormous while the buffer is empty, and the budget refusal is the more
/// specific answer for that case — which is what
/// `every_refusal_precedes_every_accumulation` asserts with a 4096×4096
/// declaration. Both still precede every read of a value.
///
/// `O(1)`: one multiplication and one length comparison per block.
fn require_dense(block: &BlockData, role: &str) -> Result<()> {
    let declared = block.rows.checked_mul(block.columns);
    if declared != Some(block.values.len()) {
        return Err(QError::QueryRejected(format!(
            "paired reduction refuses a ragged block: the {role} block holds {} values \
             but declares the shape [{}, {}] — {} values",
            block.values.len(),
            block.rows,
            block.columns,
            declared
                .map(|n| n.to_string())
                .unwrap_or_else(|| "overflowingly many".to_string())
        )));
    }
    Ok(())
}

/// Refuse a block holding `NaN` or `±Inf`, naming where it is.
///
/// Defence in depth: `QM-0120` refuses a non-finite value at the source. If one
/// arrives here anyway it would poison a sum irreversibly — one `NaN` makes
/// `sum_sq_delta` `NaN` for the whole tensor, and a `max` comparison against
/// `NaN` silently keeps the old value instead. Both would be reported as
/// numbers.
fn require_finite(block: &BlockData, role: &str) -> Result<()> {
    for (index, &value) in block.values.iter().enumerate() {
        if !value.is_finite() {
            let row = index / block.columns;
            let column = index % block.columns;
            return Err(QError::QueryRejected(format!(
                "paired reduction refuses a non-finite value: the {role} block holds \
                 {value} at row {row}, column {column} (flat index {index})"
            )));
        }
    }
    Ok(())
}

impl CpuBackend {
    /// The reference paired reduction. See [`crate::Backend::paired_block_reduction`].
    ///
    /// Refusal order is deliberate and is asserted by
    /// `every_refusal_precedes_every_accumulation`: shape, then emptiness, then
    /// the workload budget, then finiteness — and only then arithmetic. The
    /// finiteness scan is a separate pass rather than a check inside the
    /// accumulation loop so that *"refuses before arithmetic"* is a structural
    /// property of the code rather than an argument about which partial results
    /// a caller can observe. It costs one extra read of two blocks that are
    /// already resident and allocates nothing.
    pub(crate) fn reduce_paired_blocks(
        &self,
        base: &BlockData,
        counterpart: &BlockData,
        axis: ChannelAxis,
    ) -> Result<PairedPartials> {
        validate_pair(base, counterpart)?;
        self.check_workload(Workload::for_paired_blocks(base.rows, base.columns))?;
        require_dense(base, "base")?;
        require_dense(counterpart, "counterpart")?;
        require_finite(base, "base")?;
        require_finite(counterpart, "counterpart")?;

        let channels = axis.channel_count(base.rows, base.columns);
        // The only allocation: one entry per channel, never one per element.
        let mut per_channel = vec![ChannelPartials::ZERO; channels];
        let mut whole = ChannelPartials::ZERO;

        // Fixed row-major order, single-threaded. Two runs are bit-identical
        // because this loop visits the same elements in the same sequence and
        // performs the same additions in the same order.
        for row in 0..base.rows {
            for column in 0..base.columns {
                let index = row * base.columns + column;
                let b = base.values[index] as f64;
                let delta = b - counterpart.values[index] as f64;
                whole.absorb(b, delta);
                let channel = match axis {
                    ChannelAxis::Rows => row,
                    ChannelAxis::Columns => column,
                };
                per_channel[channel].absorb(b, delta);
            }
        }

        Ok(PairedPartials {
            count: whole.count,
            sum_sq_base: whole.sum_sq_base,
            sum_sq_delta: whole.sum_sq_delta,
            sum_abs_delta: whole.sum_abs_delta,
            max_abs_delta: whole.max_abs_delta,
            max_abs_base: whole.max_abs_base,
            per_channel,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Backend;

    /// The hand fixture of acceptance criterion 1.
    ///
    /// ```text
    /// base                              counterpart
    ///  1.00  -2.00   3.00  -4.00         1.50  -1.00   3.00  -3.50
    ///  0.50   1.50  -2.50   4.50         0.25   2.00  -2.00   4.00
    /// -1.25   2.25   0.00   3.75        -1.00   2.50   0.50   3.00
    ///
    /// delta = base - counterpart
    /// -0.50  -1.00   0.00  -0.50
    ///  0.25  -0.50  -0.50   0.50
    /// -0.25  -0.25  -0.50   0.75
    /// ```
    ///
    /// Every value is a multiple of 1/4, so every square is a multiple of 1/16
    /// and every sum below is **exact** in binary floating point. The expected
    /// numbers may therefore be compared with `==`, with no tolerance to hide
    /// behind.
    fn hand_base() -> BlockData {
        BlockData::new(
            3,
            4,
            vec![
                1.0, -2.0, 3.0, -4.0, //
                0.5, 1.5, -2.5, 4.5, //
                -1.25, 2.25, 0.0, 3.75,
            ],
        )
        .unwrap()
    }

    fn hand_counterpart() -> BlockData {
        BlockData::new(
            3,
            4,
            vec![
                1.5, -1.0, 3.0, -3.5, //
                0.25, 2.0, -2.0, 4.0, //
                -1.0, 2.5, 0.5, 3.0,
            ],
        )
        .unwrap()
    }

    #[test]
    fn hand_computed_3x4_reduction_matches_every_field_computed_in_this_test() {
        let out = CpuBackend
            .paired_block_reduction(&hand_base(), &hand_counterpart(), ChannelAxis::Rows)
            .unwrap();

        // -- whole block, summed term by term ---------------------------------
        // sum_sq_base  = (1 + 4 + 9 + 16) + (0.25 + 2.25 + 6.25 + 20.25)
        //              + (1.5625 + 5.0625 + 0 + 14.0625)
        //              = 30 + 29 + 20.6875 = 79.6875
        // sum_sq_delta = (0.25 + 1 + 0 + 0.25) + (0.0625 + 0.25 + 0.25 + 0.25)
        //              + (0.0625 + 0.0625 + 0.25 + 0.5625)
        //              = 1.5 + 0.8125 + 0.9375 = 3.25
        // sum_abs_delta= (0.5 + 1 + 0 + 0.5) + (0.25 + 0.5 + 0.5 + 0.5)
        //              + (0.25 + 0.25 + 0.5 + 0.75)
        //              = 2 + 1.75 + 1.75 = 5.5
        // max_abs_delta= 1.0   (row 0, column 1)
        // max_abs_base = 4.5   (row 1, column 3)
        assert_eq!(out.count, 12);
        assert_eq!(out.sum_sq_base, 79.6875);
        assert_eq!(out.sum_sq_delta, 3.25);
        assert_eq!(out.sum_abs_delta, 5.5);
        assert_eq!(out.max_abs_delta, 1.0);
        assert_eq!(out.max_abs_base, 4.5);

        // -- per row ----------------------------------------------------------
        assert_eq!(out.per_channel.len(), 3, "one channel per row");
        assert_eq!(
            out.per_channel[0],
            ChannelPartials {
                count: 4,
                // 1 + 4 + 9 + 16
                sum_sq_base: 30.0,
                // 0.25 + 1 + 0 + 0.25
                sum_sq_delta: 1.5,
                // 0.5 + 1 + 0 + 0.5
                sum_abs_delta: 2.0,
                max_abs_delta: 1.0,
                max_abs_base: 4.0,
            }
        );
        assert_eq!(
            out.per_channel[1],
            ChannelPartials {
                count: 4,
                // 0.25 + 2.25 + 6.25 + 20.25
                sum_sq_base: 29.0,
                // 0.0625 + 0.25 + 0.25 + 0.25
                sum_sq_delta: 0.8125,
                // 0.25 + 0.5 + 0.5 + 0.5
                sum_abs_delta: 1.75,
                max_abs_delta: 0.5,
                max_abs_base: 4.5,
            }
        );
        assert_eq!(
            out.per_channel[2],
            ChannelPartials {
                count: 4,
                // 1.5625 + 5.0625 + 0 + 14.0625
                sum_sq_base: 20.6875,
                // 0.0625 + 0.0625 + 0.25 + 0.5625
                sum_sq_delta: 0.9375,
                // 0.25 + 0.25 + 0.5 + 0.75
                sum_abs_delta: 1.75,
                max_abs_delta: 0.75,
                max_abs_base: 3.75,
            }
        );
    }

    #[test]
    fn hand_computed_3x4_reduction_over_columns_matches_every_field_computed_in_this_test() {
        // The same fixture read down the columns instead of across the rows.
        // base columns: (1, 0.5, -1.25) (-2, 1.5, 2.25) (3, -2.5, 0) (-4, 4.5, 3.75)
        // delta columns: (-0.5, 0.25, -0.25) (-1, -0.5, -0.25)
        //                (0, -0.5, -0.5)     (-0.5, 0.5, 0.75)
        let out = CpuBackend
            .paired_block_reduction(&hand_base(), &hand_counterpart(), ChannelAxis::Columns)
            .unwrap();

        // The whole-block figures are the same numbers as the row reduction:
        // the block holds the same twelve elements either way.
        assert_eq!(out.count, 12);
        assert_eq!(out.sum_sq_base, 79.6875);
        assert_eq!(out.sum_sq_delta, 3.25);
        assert_eq!(out.sum_abs_delta, 5.5);
        assert_eq!(out.max_abs_delta, 1.0);
        assert_eq!(out.max_abs_base, 4.5);

        assert_eq!(out.per_channel.len(), 4, "one channel per column");
        assert_eq!(
            out.per_channel[0],
            ChannelPartials {
                count: 3,
                // 1 + 0.25 + 1.5625
                sum_sq_base: 2.8125,
                // 0.25 + 0.0625 + 0.0625
                sum_sq_delta: 0.375,
                // 0.5 + 0.25 + 0.25
                sum_abs_delta: 1.0,
                max_abs_delta: 0.5,
                max_abs_base: 1.25,
            }
        );
        assert_eq!(
            out.per_channel[1],
            ChannelPartials {
                count: 3,
                // 4 + 2.25 + 5.0625
                sum_sq_base: 11.3125,
                // 1 + 0.25 + 0.0625
                sum_sq_delta: 1.3125,
                // 1 + 0.5 + 0.25
                sum_abs_delta: 1.75,
                max_abs_delta: 1.0,
                max_abs_base: 2.25,
            }
        );
        assert_eq!(
            out.per_channel[2],
            ChannelPartials {
                count: 3,
                // 9 + 6.25 + 0
                sum_sq_base: 15.25,
                // 0 + 0.25 + 0.25
                sum_sq_delta: 0.5,
                // 0 + 0.5 + 0.5
                sum_abs_delta: 1.0,
                max_abs_delta: 0.5,
                max_abs_base: 3.0,
            }
        );
        assert_eq!(
            out.per_channel[3],
            ChannelPartials {
                count: 3,
                // 16 + 20.25 + 14.0625
                sum_sq_base: 50.3125,
                // 0.25 + 0.25 + 0.5625
                sum_sq_delta: 1.0625,
                // 0.5 + 0.5 + 0.75
                sum_abs_delta: 1.75,
                max_abs_delta: 0.75,
                max_abs_base: 4.5,
            }
        );
    }

    #[test]
    fn the_whole_block_partials_equal_the_sum_of_the_per_channel_partials() {
        // An invariant, on the exact fixture where it is exact. It is NOT
        // asserted on an inexact fixture, because it is not true there: the two
        // groupings associate the additions differently. See `PairedPartials`.
        for axis in [ChannelAxis::Rows, ChannelAxis::Columns] {
            let out = CpuBackend
                .paired_block_reduction(&hand_base(), &hand_counterpart(), axis)
                .unwrap();
            let count: u64 = out.per_channel.iter().map(|c| c.count).sum();
            let sum_sq_base: f64 = out.per_channel.iter().map(|c| c.sum_sq_base).sum();
            let sum_sq_delta: f64 = out.per_channel.iter().map(|c| c.sum_sq_delta).sum();
            let sum_abs_delta: f64 = out.per_channel.iter().map(|c| c.sum_abs_delta).sum();
            let max_abs_delta = out
                .per_channel
                .iter()
                .fold(0.0f64, |m, c| m.max(c.max_abs_delta));
            let max_abs_base = out
                .per_channel
                .iter()
                .fold(0.0f64, |m, c| m.max(c.max_abs_base));
            assert_eq!(count, out.count, "{axis:?}");
            assert_eq!(sum_sq_base, out.sum_sq_base, "{axis:?}");
            assert_eq!(sum_sq_delta, out.sum_sq_delta, "{axis:?}");
            assert_eq!(sum_abs_delta, out.sum_abs_delta, "{axis:?}");
            assert_eq!(max_abs_delta, out.max_abs_delta, "{axis:?}");
            assert_eq!(max_abs_base, out.max_abs_base, "{axis:?}");
        }
    }

    #[test]
    fn an_identical_counterpart_yields_exactly_zero_delta_and_leaves_the_base_energy_intact() {
        // TASK.md Test Cases row 2. Not "small" — exactly zero, every field,
        // whole block and every channel.
        let base = hand_base();
        for axis in [ChannelAxis::Rows, ChannelAxis::Columns] {
            let out = CpuBackend
                .paired_block_reduction(&base, &hand_base(), axis)
                .unwrap();
            assert_eq!(out.sum_sq_delta, 0.0);
            assert_eq!(out.sum_abs_delta, 0.0);
            assert_eq!(out.max_abs_delta, 0.0);
            // The base energy is untouched by the counterpart being identical.
            assert_eq!(out.sum_sq_base, 79.6875);
            assert_eq!(out.max_abs_base, 4.5);
            for (index, channel) in out.per_channel.iter().enumerate() {
                assert_eq!(channel.sum_sq_delta, 0.0, "channel {index}");
                assert_eq!(channel.sum_abs_delta, 0.0, "channel {index}");
                assert_eq!(channel.max_abs_delta, 0.0, "channel {index}");
            }
        }
    }

    #[test]
    fn an_all_zero_counterpart_makes_the_delta_energy_equal_the_base_energy() {
        // TASK.md Test Cases row 3. With ŵ = 0 the delta is w, so
        // sum_sq_delta must be sum_sq_base exactly — the degenerate case where
        // a relative error of 1 is the right answer.
        let base = hand_base();
        let zeros = BlockData::new(3, 4, vec![0.0; 12]).unwrap();
        let out = CpuBackend
            .paired_block_reduction(&base, &zeros, ChannelAxis::Rows)
            .unwrap();
        assert_eq!(out.sum_sq_delta, out.sum_sq_base);
        assert_eq!(out.sum_sq_delta, 79.6875);
        assert_eq!(out.max_abs_delta, out.max_abs_base);
        for channel in &out.per_channel {
            assert_eq!(channel.sum_sq_delta, channel.sum_sq_base);
            assert_eq!(channel.max_abs_delta, channel.max_abs_base);
        }
    }

    // -- acceptance criterion 2: orientation ---------------------------------

    /// A deliberately asymmetric 2×5 block. Non-square, so the two axes do not
    /// even produce the same number of channels, and no row is a permutation of
    /// any column.
    ///
    /// ```text
    /// base                                  counterpart
    ///  1.00  2.00   3.00  4.00   5.00        0.50  2.50   3.00  3.00   4.00
    /// -0.50  0.25  -1.50  2.75  -3.00       -0.25  0.50  -1.00  3.00  -2.00
    ///
    /// delta
    ///  0.50 -0.50   0.00  1.00   1.00
    /// -0.25 -0.25  -0.50 -0.25  -1.00
    /// ```
    fn asymmetric_base() -> BlockData {
        BlockData::new(
            2,
            5,
            vec![
                1.0, 2.0, 3.0, 4.0, 5.0, //
                -0.5, 0.25, -1.5, 2.75, -3.0,
            ],
        )
        .unwrap()
    }

    fn asymmetric_counterpart() -> BlockData {
        BlockData::new(
            2,
            5,
            vec![
                0.5, 2.5, 3.0, 3.0, 4.0, //
                -0.25, 0.5, -1.0, 3.0, -2.0,
            ],
        )
        .unwrap()
    }

    #[test]
    fn reducing_over_the_wrong_axis_changes_the_result() {
        let base = asymmetric_base();
        let counterpart = asymmetric_counterpart();
        let by_row = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap();
        let by_column = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Columns)
            .unwrap();

        // Rows: 2 channels of 5. Hand-computed.
        assert_eq!(by_row.per_channel.len(), 2);
        assert_eq!(
            by_row.per_channel[0],
            ChannelPartials {
                count: 5,
                // 1 + 4 + 9 + 16 + 25
                sum_sq_base: 55.0,
                // 0.25 + 0.25 + 0 + 1 + 1
                sum_sq_delta: 2.5,
                // 0.5 + 0.5 + 0 + 1 + 1
                sum_abs_delta: 3.0,
                max_abs_delta: 1.0,
                max_abs_base: 5.0,
            }
        );
        assert_eq!(
            by_row.per_channel[1],
            ChannelPartials {
                count: 5,
                // 0.25 + 0.0625 + 2.25 + 7.5625 + 9
                sum_sq_base: 19.125,
                // 0.0625 + 0.0625 + 0.25 + 0.0625 + 1
                sum_sq_delta: 1.4375,
                // 0.25 + 0.25 + 0.5 + 0.25 + 1
                sum_abs_delta: 2.25,
                max_abs_delta: 1.0,
                max_abs_base: 3.0,
            }
        );

        // Columns: 5 channels of 2. Hand-computed.
        assert_eq!(by_column.per_channel.len(), 5);
        let expected_columns = [
            // (1, -0.5)     deltas (0.5, -0.25)
            ChannelPartials {
                count: 2,
                sum_sq_base: 1.25,
                sum_sq_delta: 0.3125,
                sum_abs_delta: 0.75,
                max_abs_delta: 0.5,
                max_abs_base: 1.0,
            },
            // (2, 0.25)     deltas (-0.5, -0.25)
            ChannelPartials {
                count: 2,
                sum_sq_base: 4.0625,
                sum_sq_delta: 0.3125,
                sum_abs_delta: 0.75,
                max_abs_delta: 0.5,
                max_abs_base: 2.0,
            },
            // (3, -1.5)     deltas (0, -0.5)
            ChannelPartials {
                count: 2,
                sum_sq_base: 11.25,
                sum_sq_delta: 0.25,
                sum_abs_delta: 0.5,
                max_abs_delta: 0.5,
                max_abs_base: 3.0,
            },
            // (4, 2.75)     deltas (1, -0.25)
            ChannelPartials {
                count: 2,
                sum_sq_base: 23.5625,
                sum_sq_delta: 1.0625,
                sum_abs_delta: 1.25,
                max_abs_delta: 1.0,
                max_abs_base: 4.0,
            },
            // (5, -3)       deltas (1, -1)
            ChannelPartials {
                count: 2,
                sum_sq_base: 34.0,
                sum_sq_delta: 2.0,
                sum_abs_delta: 2.0,
                max_abs_delta: 1.0,
                max_abs_base: 5.0,
            },
        ];
        assert_eq!(by_column.per_channel, expected_columns);

        // The two orientations are different answers, not the same answer in a
        // different order: no multiset of the row partials equals the column
        // partials, and the channel counts differ.
        assert_ne!(by_row.per_channel.len(), by_column.per_channel.len());
        for row_channel in &by_row.per_channel {
            assert!(
                !by_column.per_channel.contains(row_channel),
                "a row partial reappeared among the column partials; the fixture \
                 is not asymmetric enough to prove orientation"
            );
        }

        // Whole-block figures are axis-invariant: the same ten elements.
        assert_eq!(by_row.count, by_column.count);
        assert_eq!(by_row.sum_sq_base, by_column.sum_sq_base);
        assert_eq!(by_row.sum_sq_delta, by_column.sum_sq_delta);
    }

    #[test]
    fn a_square_block_still_distinguishes_the_two_axes() {
        // Companion to the 2×5 case. A square fixture keeps a transposed
        // implementation *in bounds*, so a transposition shows up as wrong
        // numbers rather than as an index panic — which is the failure mode a
        // reviewer should be able to see. Powers of two make every row sum and
        // every column sum distinct and exact.
        //
        // base                 counterpart          delta
        //   1    2    4          1    1    2          0    1    2
        //   8   16   32          4    8   16          4    8   16
        //  64  128  256         32   64  128         32   64  128
        let base = BlockData::new(3, 3, vec![1., 2., 4., 8., 16., 32., 64., 128., 256.]).unwrap();
        let counterpart =
            BlockData::new(3, 3, vec![1., 1., 2., 4., 8., 16., 32., 64., 128.]).unwrap();

        let by_row = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap();
        let by_column = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Columns)
            .unwrap();

        assert_eq!(by_row.per_channel.len(), 3);
        assert_eq!(by_column.per_channel.len(), 3);

        // Rows: sum_sq_base = 1+4+16 | 64+256+1024 | 4096+16384+65536
        assert_eq!(
            by_row
                .per_channel
                .iter()
                .map(|c| c.sum_sq_base)
                .collect::<Vec<_>>(),
            vec![21.0, 1344.0, 86016.0]
        );
        // Columns: 1+64+4096 | 4+256+16384 | 16+1024+65536
        assert_eq!(
            by_column
                .per_channel
                .iter()
                .map(|c| c.sum_sq_base)
                .collect::<Vec<_>>(),
            vec![4161.0, 16644.0, 66576.0]
        );
        // Rows: sum_sq_delta = 0+1+4 | 16+64+256 | 1024+4096+16384
        assert_eq!(
            by_row
                .per_channel
                .iter()
                .map(|c| c.sum_sq_delta)
                .collect::<Vec<_>>(),
            vec![5.0, 336.0, 21504.0]
        );
        // Columns: 0+16+1024 | 1+64+4096 | 4+256+16384
        assert_eq!(
            by_column
                .per_channel
                .iter()
                .map(|c| c.sum_sq_delta)
                .collect::<Vec<_>>(),
            vec![1040.0, 4161.0, 16644.0]
        );
        // Every channel differs between the two orientations. A transposed
        // implementation of the channel index would land on the other list and
        // every one of these equalities would fail.
        for (r, c) in by_row.per_channel.iter().zip(&by_column.per_channel) {
            assert_ne!(r, c);
        }
    }

    #[test]
    fn per_channel_length_follows_the_requested_axis_not_the_block_shape() {
        // `channel_count` is the whole of the axis's effect on the output size,
        // stated separately so a change to it cannot pass unnoticed.
        assert_eq!(ChannelAxis::Rows.channel_count(2, 5), 2);
        assert_eq!(ChannelAxis::Columns.channel_count(2, 5), 5);
        let out = CpuBackend
            .paired_block_reduction(
                &asymmetric_base(),
                &asymmetric_counterpart(),
                ChannelAxis::Rows,
            )
            .unwrap();
        assert_eq!(out.per_channel.len(), 2);
        assert!(out.per_channel.iter().all(|c| c.count == 5));
    }

    // -- acceptance criterion 3: composition ---------------------------------

    /// A 4×3 fixture of quarter-multiples, so every sum below is exact and the
    /// composition may be asserted with `==` rather than a tolerance.
    fn compose_blocks() -> (BlockData, BlockData) {
        let base = BlockData::new(
            4,
            3,
            vec![
                1.0, -2.0, 0.5, //
                3.0, 4.0, -1.5, //
                -0.25, 2.5, 6.0, //
                0.75, -3.5, 1.25,
            ],
        )
        .unwrap();
        let counterpart = BlockData::new(
            4,
            3,
            vec![
                0.5, -1.0, 0.25, //
                2.5, 4.5, -1.0, //
                0.0, 2.0, 5.5, //
                1.0, -3.0, 1.0,
            ],
        )
        .unwrap();
        (base, counterpart)
    }

    fn halves(block: &BlockData) -> (BlockData, BlockData) {
        let split = block.rows / 2;
        let cut = split * block.columns;
        (
            BlockData::new(split, block.columns, block.values[..cut].to_vec()).unwrap(),
            BlockData::new(
                block.rows - split,
                block.columns,
                block.values[cut..].to_vec(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn partials_from_two_halves_sum_to_the_whole_block_reduction() {
        // TASK.md Test Cases row 5 and acceptance criterion 3. The halves are
        // cut across the rows while the channels run down the columns, so every
        // channel is split between the two halves — the case that actually
        // exercises composition rather than concatenation.
        //
        // The merge arithmetic is written out here rather than called from the
        // crate: `PairedPartials` deliberately has no `merge`, because the
        // aggregation layer (QM-0123) owns it, and a test that called the
        // implementation's own merge would prove nothing about composability.
        let (base, counterpart) = compose_blocks();
        let (base_top, base_bottom) = halves(&base);
        let (counterpart_top, counterpart_bottom) = halves(&counterpart);

        let whole = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Columns)
            .unwrap();
        let top = CpuBackend
            .paired_block_reduction(&base_top, &counterpart_top, ChannelAxis::Columns)
            .unwrap();
        let bottom = CpuBackend
            .paired_block_reduction(&base_bottom, &counterpart_bottom, ChannelAxis::Columns)
            .unwrap();

        // Whole block: additive fields add, max fields take the maximum.
        assert_eq!(whole.count, top.count + bottom.count);
        assert_eq!(whole.sum_sq_base, top.sum_sq_base + bottom.sum_sq_base);
        assert_eq!(whole.sum_sq_delta, top.sum_sq_delta + bottom.sum_sq_delta);
        assert_eq!(
            whole.sum_abs_delta,
            top.sum_abs_delta + bottom.sum_abs_delta
        );
        assert_eq!(
            whole.max_abs_delta,
            top.max_abs_delta.max(bottom.max_abs_delta)
        );
        assert_eq!(
            whole.max_abs_base,
            top.max_abs_base.max(bottom.max_abs_base)
        );

        // Per channel, the same rule, channel by channel.
        assert_eq!(whole.per_channel.len(), 3);
        assert_eq!(top.per_channel.len(), 3);
        assert_eq!(bottom.per_channel.len(), 3);
        for (channel, (w, (t, b))) in whole
            .per_channel
            .iter()
            .zip(top.per_channel.iter().zip(&bottom.per_channel))
            .enumerate()
        {
            assert_eq!(w.count, t.count + b.count, "channel {channel} count");
            assert_eq!(
                w.sum_sq_base,
                t.sum_sq_base + b.sum_sq_base,
                "channel {channel} sum_sq_base"
            );
            assert_eq!(
                w.sum_sq_delta,
                t.sum_sq_delta + b.sum_sq_delta,
                "channel {channel} sum_sq_delta"
            );
            assert_eq!(
                w.sum_abs_delta,
                t.sum_abs_delta + b.sum_abs_delta,
                "channel {channel} sum_abs_delta"
            );
            assert_eq!(
                w.max_abs_delta,
                t.max_abs_delta.max(b.max_abs_delta),
                "channel {channel} max_abs_delta"
            );
            assert_eq!(
                w.max_abs_base,
                t.max_abs_base.max(b.max_abs_base),
                "channel {channel} max_abs_base"
            );
        }

        // The halves are genuinely different from one another, so the equality
        // above is not the trivial one.
        assert_ne!(top.per_channel, bottom.per_channel);
    }

    #[test]
    fn splitting_along_the_channel_axis_concatenates_the_per_channel_partials() {
        // The other half of composition: cut across the rows while the channels
        // ARE the rows, and the per-channel lists concatenate untouched.
        let (base, counterpart) = compose_blocks();
        let (base_top, base_bottom) = halves(&base);
        let (counterpart_top, counterpart_bottom) = halves(&counterpart);

        let whole = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap();
        let top = CpuBackend
            .paired_block_reduction(&base_top, &counterpart_top, ChannelAxis::Rows)
            .unwrap();
        let bottom = CpuBackend
            .paired_block_reduction(&base_bottom, &counterpart_bottom, ChannelAxis::Rows)
            .unwrap();

        let mut joined = top.per_channel.clone();
        joined.extend_from_slice(&bottom.per_channel);
        assert_eq!(whole.per_channel, joined);
        assert_eq!(whole.sum_sq_delta, top.sum_sq_delta + bottom.sum_sq_delta);
    }

    // -- acceptance criterion 4: determinism ---------------------------------

    /// A fixture whose sums are **inexact**, so that accumulation order is
    /// observable. Tenths are not representable in binary, and the f32→f64
    /// widening keeps them inexact.
    fn inexact_blocks() -> (BlockData, BlockData) {
        let base: Vec<f32> = (0..20).map(|k| 0.1 * (k + 1) as f32).collect();
        let counterpart: Vec<f32> = (0..20).map(|k| 0.03 * (k + 1) as f32).collect();
        (
            BlockData::new(5, 4, base).unwrap(),
            BlockData::new(5, 4, counterpart).unwrap(),
        )
    }

    #[test]
    fn two_runs_of_the_same_reduction_are_bit_identical() {
        // TASK.md Test Cases row 9, acceptance criterion 4, requirement V1-13.
        // Compared as raw bit patterns: `==` on f64 would accept -0.0 for 0.0.
        let (base, counterpart) = inexact_blocks();
        let first = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Columns)
            .unwrap();
        // A second, independently allocated pair of blocks holding the same
        // values, so the result cannot depend on where the data sits.
        let (base_again, counterpart_again) = inexact_blocks();
        let second = CpuBackend
            .paired_block_reduction(&base_again, &counterpart_again, ChannelAxis::Columns)
            .unwrap();

        assert_eq!(first.count, second.count);
        assert_eq!(first.sum_sq_base.to_bits(), second.sum_sq_base.to_bits());
        assert_eq!(first.sum_sq_delta.to_bits(), second.sum_sq_delta.to_bits());
        assert_eq!(
            first.sum_abs_delta.to_bits(),
            second.sum_abs_delta.to_bits()
        );
        assert_eq!(
            first.max_abs_delta.to_bits(),
            second.max_abs_delta.to_bits()
        );
        assert_eq!(first.max_abs_base.to_bits(), second.max_abs_base.to_bits());
        assert_eq!(first.per_channel.len(), second.per_channel.len());
        for (a, b) in first.per_channel.iter().zip(&second.per_channel) {
            assert_eq!(a.sum_sq_base.to_bits(), b.sum_sq_base.to_bits());
            assert_eq!(a.sum_sq_delta.to_bits(), b.sum_sq_delta.to_bits());
            assert_eq!(a.sum_abs_delta.to_bits(), b.sum_abs_delta.to_bits());
            assert_eq!(a.max_abs_delta.to_bits(), b.max_abs_delta.to_bits());
            assert_eq!(a.max_abs_base.to_bits(), b.max_abs_base.to_bits());
        }

        // The claim above is not vacuous: on THIS fixture a different
        // accumulation order gives a different answer. Re-summing the
        // per-channel partials groups the additions by column instead of in
        // row-major element order, and the last bit moves.
        let regrouped: f64 = first.per_channel.iter().map(|c| c.sum_sq_delta).sum();
        assert_ne!(
            regrouped.to_bits(),
            first.sum_sq_delta.to_bits(),
            "the determinism fixture must have inexact sums, or bit-identity \
             between two runs proves nothing about the reduction order"
        );
        let regrouped: f64 = first.per_channel.iter().map(|c| c.sum_sq_base).sum();
        assert_ne!(
            regrouped.to_bits(),
            first.sum_sq_base.to_bits(),
            "same, for the base energy"
        );
    }

    #[test]
    fn the_whole_block_partials_are_bit_identical_whichever_axis_is_requested() {
        // The whole-block accumulator runs in row-major element order and does
        // not consult the axis. On the inexact fixture, an axis-dependent
        // accumulation order would show up here as a last-bit difference.
        let (base, counterpart) = inexact_blocks();
        let by_row = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap();
        let by_column = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Columns)
            .unwrap();
        assert_eq!(
            by_row.sum_sq_base.to_bits(),
            by_column.sum_sq_base.to_bits()
        );
        assert_eq!(
            by_row.sum_sq_delta.to_bits(),
            by_column.sum_sq_delta.to_bits()
        );
        assert_eq!(
            by_row.sum_abs_delta.to_bits(),
            by_column.sum_abs_delta.to_bits()
        );
        assert_eq!(
            by_row.max_abs_delta.to_bits(),
            by_column.max_abs_delta.to_bits()
        );
        assert_eq!(
            by_row.max_abs_base.to_bits(),
            by_column.max_abs_base.to_bits()
        );
    }

    #[test]
    fn accumulation_is_in_f64_and_does_not_lose_a_small_delta_to_a_large_base() {
        // f32 accumulators would drop this entirely: 2^24 + 1 is not
        // representable in f32, so a sum of squares taken in f32 would swallow
        // the small terms. In f64 the exact answer is representable.
        //
        // base = [4096, 0.5], counterpart = [4096, 0]
        //   sum_sq_base  = 16777216 + 0.25 = 16777216.25
        //   sum_sq_delta = 0 + 0.25        = 0.25
        let base = BlockData::new(1, 2, vec![4096.0, 0.5]).unwrap();
        let counterpart = BlockData::new(1, 2, vec![4096.0, 0.0]).unwrap();
        let out = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap();
        assert_eq!(out.sum_sq_base, 16_777_216.25);
        assert_eq!(out.sum_sq_delta, 0.25);
        assert_eq!(out.sum_sq_base as f32, 16_777_216.0, "f32 would lose it");
    }

    // -- acceptance criterion 5: refusals ------------------------------------

    #[test]
    fn a_shape_mismatch_is_refused_naming_both_shapes() {
        let base = hand_base();
        let counterpart = BlockData::new(2, 4, vec![0.0; 8]).unwrap();
        let message = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap_err()
            .to_string();
        assert!(message.contains("shape mismatch"), "{message}");
        assert!(message.contains("base is [3, 4]"), "{message}");
        assert!(message.contains("counterpart is [2, 4]"), "{message}");
    }

    #[test]
    fn a_transposed_counterpart_is_a_shape_mismatch_even_though_the_element_count_matches() {
        // 3×4 against 4×3 holds exactly as many values, so a check on
        // `values.len()` alone would let it through and silently reduce a
        // transposed counterpart.
        let base = hand_base();
        let counterpart = BlockData::new(4, 3, vec![0.0; 12]).unwrap();
        let message = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap_err()
            .to_string();
        assert!(message.contains("base is [3, 4]"), "{message}");
        assert!(message.contains("counterpart is [4, 3]"), "{message}");
    }

    #[test]
    fn an_empty_block_is_refused_rather_than_reduced_to_fabricated_partials() {
        // TASK.md Test Cases row 8. Zero rows, zero columns, and zero of both.
        for (rows, columns) in [(0usize, 4usize), (3, 0), (0, 0)] {
            let base = BlockData::new(rows, columns, vec![]).unwrap();
            let counterpart = BlockData::new(rows, columns, vec![]).unwrap();
            let message = CpuBackend
                .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
                .unwrap_err()
                .to_string();
            assert!(
                message.contains("refuses an empty block"),
                "[{rows}, {columns}]: {message}"
            );
            assert!(
                message.contains(&format!("[{rows}, {columns}]")),
                "[{rows}, {columns}]: {message}"
            );
        }
    }

    #[test]
    fn a_non_finite_value_is_refused_naming_the_block_and_the_position() {
        // TASK.md Test Cases row 7. NaN, +Inf and -Inf, in either operand.
        // Index 6 of a 3×4 block is row 1, column 2.
        for (poison, rendered) in [
            (f32::NAN, "NaN"),
            (f32::INFINITY, "inf"),
            (f32::NEG_INFINITY, "-inf"),
        ] {
            let mut values = hand_counterpart().values;
            values[6] = poison;
            let counterpart = BlockData::new(3, 4, values).unwrap();
            let message = CpuBackend
                .paired_block_reduction(&hand_base(), &counterpart, ChannelAxis::Rows)
                .unwrap_err()
                .to_string();
            assert!(message.contains("non-finite"), "{message}");
            assert!(message.contains("counterpart block"), "{message}");
            assert!(message.contains("row 1, column 2"), "{message}");
            assert!(message.contains("flat index 6"), "{message}");
            assert!(message.contains(rendered), "{message}");

            // The same value in the base names the base, not the counterpart.
            let mut values = hand_base().values;
            values[11] = poison;
            let base = BlockData::new(3, 4, values).unwrap();
            let message = CpuBackend
                .paired_block_reduction(&base, &hand_counterpart(), ChannelAxis::Rows)
                .unwrap_err()
                .to_string();
            assert!(message.contains("base block"), "{message}");
            assert!(message.contains("row 2, column 3"), "{message}");
        }
    }

    #[test]
    fn an_axis_index_outside_the_tensor_rank_is_refused_naming_the_rank() {
        assert_eq!(ChannelAxis::from_index(0, 2).unwrap(), ChannelAxis::Rows);
        assert_eq!(ChannelAxis::from_index(1, 2).unwrap(), ChannelAxis::Columns);
        let message = ChannelAxis::from_index(2, 2).unwrap_err().to_string();
        assert!(message.contains("channel axis 2"), "{message}");
        assert!(message.contains("rank-2"), "{message}");
        let message = ChannelAxis::from_index(9, 2).unwrap_err().to_string();
        assert!(message.contains("channel axis 9"), "{message}");
        assert!(message.contains("rank-2"), "{message}");
    }

    #[test]
    fn a_rank_that_is_not_two_is_refused_the_way_the_block_planner_refuses_it() {
        // `BlockExtent::clamped_to` already refuses a shape that is not rank 2
        // with "block extents apply to rank-2 tensors"; this refuses the axis
        // for the same reason rather than inventing a second rule.
        for rank in [0usize, 1, 3] {
            let message = ChannelAxis::from_index(0, rank).unwrap_err().to_string();
            assert!(message.contains("rank-2 blocks"), "rank {rank}: {message}");
            assert!(message.contains(&format!("rank {rank}")), "{message}");
        }
    }

    #[test]
    fn a_rank_above_the_adr_010_ceiling_refuses_rather_than_flattening() {
        // ADR-010: rank ≤ 3 is implemented, rank > 3 refuses with GRID-007
        // rather than flattening into a confidently wrong picture.
        for rank in [4usize, 5, 17] {
            let error = ChannelAxis::from_index(0, rank).unwrap_err();
            assert!(
                matches!(
                    error,
                    QError::NotImplemented {
                        requirement: "GRID-007",
                        ..
                    }
                ),
                "rank {rank}: {error}"
            );
            let message = error.to_string();
            assert!(message.contains("ADR-010"), "{message}");
            assert!(message.contains("ceiling of 3"), "{message}");
            assert!(
                !message.contains("flatten") || message.contains("rather than flattening"),
                "{message}"
            );
        }
    }

    #[test]
    fn a_block_whose_buffer_is_shorter_than_its_shape_is_refused_not_panicked_on() {
        // `BlockData::new` rejects a ragged block, but `rows`, `columns` and
        // `values` are all `pub`, so one reaches this kernel from safe code.
        // Small enough to pass the budget and shaped consistently with its
        // counterpart, it clears every other refusal and lands in the
        // accumulation loop, where `values[index]` panics. A panic is not a
        // refusal: it carries no requirement ID and cannot be reported to a
        // caller that was told the operation returns a `Result`.
        let short = BlockData {
            rows: 2,
            columns: 2,
            values: vec![1.0],
        };
        let full = BlockData::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();

        let message = CpuBackend
            .paired_block_reduction(&short, &full, ChannelAxis::Rows)
            .unwrap_err()
            .to_string();
        assert!(message.contains("ragged block"), "{message}");
        assert!(message.contains("base block"), "{message}");
        assert!(message.contains("holds 1 values"), "{message}");
        assert!(message.contains("[2, 2]"), "{message}");

        // The counterpart is checked too, and named as the counterpart.
        let message = CpuBackend
            .paired_block_reduction(&full, &short, ChannelAxis::Rows)
            .unwrap_err()
            .to_string();
        assert!(message.contains("ragged block"), "{message}");
        assert!(message.contains("counterpart block"), "{message}");

        // A buffer LONGER than the shape is refused as well: the extra values
        // would be silently ignored, which is a fabricated answer rather than a
        // loud one.
        let long = BlockData {
            rows: 2,
            columns: 2,
            values: vec![1.0, 2.0, 3.0, 4.0, 5.0],
        };
        assert!(CpuBackend
            .paired_block_reduction(&long, &full, ChannelAxis::Rows)
            .is_err());

        // And a declared shape whose product overflows `usize` refuses rather
        // than wrapping to a length that happens to match.
        let overflowing = BlockData {
            rows: usize::MAX,
            columns: 3,
            values: vec![1.0],
        };
        assert!(CpuBackend
            .paired_block_reduction(&overflowing, &overflowing, ChannelAxis::Rows)
            .is_err());
    }

    #[test]
    fn every_refusal_precedes_every_accumulation() {
        // Acceptance criterion 5's "before arithmetic", asserted structurally.
        //
        // Each block below declares a shape but carries an EMPTY value buffer.
        // `BlockData`'s fields are public, so this is a value the type admits;
        // it is used here because any code path that read `values` before
        // refusing would panic on an out-of-bounds index instead of returning
        // the refusal these assertions expect. A refusal that arrives is
        // therefore proof that nothing was read.
        let ragged = |rows, columns| BlockData {
            rows,
            columns,
            values: Vec::new(),
        };

        // 1. Shape is checked first — before emptiness, and before any read.
        let message = CpuBackend
            .paired_block_reduction(&ragged(3, 4), &ragged(2, 4), ChannelAxis::Rows)
            .unwrap_err()
            .to_string();
        assert!(message.contains("shape mismatch"), "{message}");

        // 2. Emptiness next, still before any read.
        let message = CpuBackend
            .paired_block_reduction(&ragged(0, 4), &ragged(0, 4), ChannelAxis::Rows)
            .unwrap_err()
            .to_string();
        assert!(message.contains("refuses an empty block"), "{message}");

        // 3. The budget next. A 4096×4096 pair is 128 MiB against the CPU
        //    backend's 64 MiB single-read budget, and the refusal arrives
        //    without either buffer having been touched.
        let error = CpuBackend
            .paired_block_reduction(&ragged(4096, 4096), &ragged(4096, 4096), ChannelAxis::Rows)
            .unwrap_err();
        assert!(
            matches!(
                error,
                QError::BudgetExceeded {
                    budget_name: "device_memory",
                    requested: 134_217_728,
                    ..
                }
            ),
            "{error}"
        );

        // 4. And a non-finite value never reaches a sum: the result is an
        //    error, not a `PairedPartials` whose fields are NaN.
        let mut values = hand_base().values;
        values[0] = f32::NAN;
        let poisoned = BlockData::new(3, 4, values).unwrap();
        assert!(CpuBackend
            .paired_block_reduction(&poisoned, &hand_counterpart(), ChannelAxis::Rows)
            .is_err());
    }

    #[test]
    fn a_paired_workload_charges_the_budget_for_both_blocks() {
        // TASK.md Implementation Plan step 5. Counting one block would let a
        // pair through at twice the declared limit.
        let single = Workload {
            element_count: 1000,
            bytes_per_element: 4,
        };
        let pair = Workload::for_paired_blocks(20, 50);
        assert_eq!(pair.element_count, 2000);
        assert_eq!(pair.bytes(), single.bytes() * 2);

        // 64 MiB budget / 4 bytes / 2 blocks = 8 388 608 elements per block.
        assert!(CpuBackend
            .check_workload(Workload::for_paired_blocks(4096, 2048))
            .is_ok());
        assert!(matches!(
            CpuBackend
                .check_workload(Workload::for_paired_blocks(4096, 2049))
                .unwrap_err(),
            QError::BudgetExceeded { .. }
        ));

        // A shape whose product overflows must saturate to a refusal, never
        // wrap to a small number that passes.
        let huge = Workload::for_paired_blocks(usize::MAX, usize::MAX);
        assert_eq!(huge.bytes(), u64::MAX);
        assert!(matches!(
            CpuBackend.check_workload(huge).unwrap_err(),
            QError::BudgetExceeded { .. }
        ));
    }

    #[test]
    fn a_backend_without_its_own_implementation_refuses_naming_quant_002() {
        // The trait default. A backend that has not built this must say so
        // rather than return zeroes, which would read as a perfect match.
        struct Unimplemented;
        impl Backend for Unimplemented {
            fn capabilities(&self) -> crate::ComputeCapabilities {
                crate::ComputeCapabilities {
                    backend_id: "test-unimplemented".to_string(),
                    display_name: "test".to_string(),
                    device_memory_bytes: 1 << 30,
                    supports_statistics: false,
                    supports_matmul: false,
                    supports_histogram: false,
                    hardware_verified: false,
                    caveat_requirement: Some("QUANT-002".to_string()),
                }
            }
            fn block_statistics(
                &self,
                _source: &dyn q_source::manifest::ModelSource,
                _descriptor: &q_source::TensorDescriptor,
                _extent: q_tensor_runtime::BlockExtent,
                _histogram_bins: usize,
            ) -> Result<q_statistics::TensorStatistics> {
                Err(QError::NotImplemented {
                    requirement: "QUANT-002",
                    detail: "test backend".to_string(),
                })
            }
            fn matmul(&self, _a: &BlockData, _b: &BlockData) -> Result<BlockData> {
                Err(QError::NotImplemented {
                    requirement: "QUANT-002",
                    detail: "test backend".to_string(),
                })
            }
        }

        let error = Unimplemented
            .paired_block_reduction(&hand_base(), &hand_counterpart(), ChannelAxis::Rows)
            .unwrap_err();
        assert!(
            matches!(
                error,
                QError::NotImplemented {
                    requirement: "QUANT-002",
                    ..
                }
            ),
            "{error}"
        );
        assert!(
            error.to_string().contains("test-unimplemented"),
            "the refusal must name the backend that refused: {error}"
        );
    }

    // -- acceptance criterion 7: a provenance-neutral signature ---------------

    /// Every line of `paired.rs` outside its documentation and its test module.
    fn declaration_lines(source: &str) -> Vec<&str> {
        let mut out = Vec::new();
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with("#[cfg(test)]") {
                break;
            }
            out.push(line);
        }
        out
    }

    #[test]
    fn the_public_signature_names_neither_quantisation_nor_second_operand_provenance() {
        // Acceptance criterion 7. The kernel must be generic over what the
        // counterpart is, and the way that is enforced is that the API cannot
        // say. Checked over declarations and code, not prose: the module
        // documentation names quantisation, checkpoint diff and expert pairs
        // together as three peers, which is the opposite of specialising to one.
        const FORBIDDEN: [&str; 10] = [
            "quant",
            "dequant",
            "gptq",
            "awq",
            "int8",
            "int4",
            "scale",
            "zero_point",
            "simulat",
            "rtn",
        ];

        let mut checked = declaration_lines(include_str!("paired.rs"));

        // Plus the trait method's own signature, which lives in lib.rs.
        let lib = include_str!("lib.rs");
        let mut in_signature = false;
        let mut signature_lines = 0;
        for line in lib.lines() {
            if line.contains("fn paired_block_reduction") {
                in_signature = true;
            }
            if in_signature {
                checked.push(line);
                signature_lines += 1;
                if line.contains("-> Result<PairedPartials>") {
                    in_signature = false;
                }
            }
        }
        assert!(
            signature_lines >= 12,
            "expected to find both `paired_block_reduction` signatures in lib.rs, \
             found {signature_lines} lines"
        );

        for line in &checked {
            let lowered = line.to_ascii_lowercase();
            for needle in FORBIDDEN {
                assert!(
                    !lowered.contains(needle),
                    "the paired reduction API names `{needle}`, which ties this \
                     kernel to one provenance of the counterpart: {line}"
                );
            }
        }

        // And the structural proof behind the textual one: no *dependency* on
        // the quantisation crate. Declarations only — a comment that mentions
        // `q-quant` in passing is prose, and prose is not a dependency edge.
        for line in include_str!("../Cargo.toml").lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            assert!(
                !trimmed.starts_with("q-quant"),
                "q-gpu must not depend on the quantisation crate; the counterpart \
                 is any second block: {line}"
            );
        }
    }

    #[test]
    fn the_same_kernel_serves_a_counterpart_with_no_quantisation_anywhere_in_sight() {
        // The behavioural half of criterion 7: two independently authored
        // blocks — the checkpoint-diff case (DIFF-001) — go through the same
        // call, with nothing in the caller referring to a reconstruction.
        let checkpoint_a = BlockData::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let checkpoint_b = BlockData::new(2, 2, vec![1.5, 2.0, 2.0, 4.5]).unwrap();
        let out = CpuBackend
            .paired_block_reduction(&checkpoint_a, &checkpoint_b, ChannelAxis::Rows)
            .unwrap();
        // deltas -0.5, 0, 1, -0.5 -> 0.25 + 0 + 1 + 0.25 = 1.5
        assert_eq!(out.sum_sq_delta, 1.5);
        assert_eq!(out.max_abs_delta, 1.0);
    }
}
