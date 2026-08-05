//! # q-statistics — Tensor Tile Plane
//!
//! Data plane: **Tensor Tile Plane** (ARCHITECTURE.md §2.1, §5.4).
//!
//! The **CPU reference implementation** of tensor statistics.
//!
//! This is the ground truth every other backend is checked against. When the
//! CUDA kernels in `gpu/cuda/` are eventually run on real hardware, their
//! output must match this to within a documented tolerance; until then, this is
//! the only implementation that produces numbers at all.
//!
//! ## Deliberate choices
//!
//! * **Streaming, single pass, bounded memory.** [`StatisticsAccumulator`]
//!   holds a fixed-size struct plus the histogram, so a caller can feed it a
//!   whole tensor block by block without ever holding the tensor. That is the
//!   same contract as everything else here: nothing scales with checkpoint
//!   size.
//! * **Welford's algorithm for variance.** The textbook
//!   `E[x²] - E[x]²` form loses catastrophic precision on weight
//!   distributions, which cluster tightly around zero with a small spread —
//!   exactly its worst case. Welford is numerically stable there.
//! * **`approximate` is a field, not an afterthought.** [`TensorStatistics`]
//!   carries whether it was computed over every element or a sample, so a UI
//!   can never present a sampled mean as an exact one.

use q_source::error::{QError, Result};
use q_source::DType;
use serde::{Deserialize, Serialize};

/// Bump when a formula changes. Part of the cache key (ARCHITECTURE.md §13.2),
/// so old cached statistics are invalidated rather than silently mixed with new
/// ones.
pub const ALGORITHM_VERSION: u32 = 1;

/// Default histogram resolution. Named, not magic.
pub const DEFAULT_HISTOGRAM_BINS: usize = 64;

/// The `.plan/DATA_ARCHITECTURE.md` §8 fidelity of a statistic.
///
/// Two variants, and the mapping from [`TensorStatistics::approximate`] is
/// **total**: there is no third outcome, and no way to spell one, so the boolean
/// and the label cannot disagree. That is the whole point — `QM-0020`'s data
/// contract requires the mapping to live in exactly one place.
///
/// * [`StatisticsFidelity::Aggregate`] — §8's name for *"a statistic over a
///   region, computed from all its values"*. Every element of the subject was
///   read, so the number is **exact for that region**. It is deliberately not
///   §8's `exact`, which names the *values as stored in the checkpoint* rather
///   than a statistic computed over them.
/// * [`StatisticsFidelity::Sampled`] — §8's *"a statistic or preview computed
///   from a subset"*. Produced whenever
///   [`StatisticsAccumulator::mark_approximate`] was used. A sampled mean must
///   never be presented as an exhaustive one; that is why this is a label
///   carried on the wire and not a comment in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatisticsFidelity {
    Aggregate,
    Sampled,
}

impl StatisticsFidelity {
    /// The one and only mapping. Everything that surfaces a statistic calls
    /// this rather than re-spelling the strings.
    pub fn from_approximate(approximate: bool) -> Self {
        if approximate {
            Self::Sampled
        } else {
            Self::Aggregate
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aggregate => "aggregate",
            Self::Sampled => "sampled",
        }
    }

    /// The inverse of [`Self::from_approximate`], so the round trip is checkable.
    pub fn is_approximate(&self) -> bool {
        matches!(self, Self::Sampled)
    }
}

/// The `tensor_statistics` row of ARCHITECTURE.md §5.4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TensorStatistics {
    pub count: u64,
    pub min_value: f64,
    pub max_value: f64,
    pub mean: f64,
    /// Population variance (divide by N, not N-1): these are complete
    /// populations of weights, not samples drawn from one.
    pub variance: f64,
    pub l1_norm: f64,
    pub l2_norm: f64,
    pub zero_ratio: f64,
    pub positive_ratio: f64,
    pub negative_ratio: f64,
    pub histogram: Histogram,
    /// `true` when computed over a sample rather than every element.
    pub approximate: bool,
    pub algorithm_version: u32,
    /// Which backend produced this, e.g. `"cpu-reference"`.
    pub backend: String,
}

impl TensorStatistics {
    pub fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }

    /// The label every surface must carry. Derived, never stored, so it cannot
    /// drift away from [`Self::approximate`].
    pub fn fidelity(&self) -> StatisticsFidelity {
        StatisticsFidelity::from_approximate(self.approximate)
    }
}

/// A fixed-range histogram.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Histogram {
    pub min: f64,
    pub max: f64,
    pub counts: Vec<u64>,
}

impl Histogram {
    pub fn bins(&self) -> usize {
        self.counts.len()
    }

    pub fn total(&self) -> u64 {
        self.counts.iter().sum()
    }

    /// Bin index for a value. Values at exactly `max` land in the last bin
    /// rather than overflowing.
    pub fn bin_of(&self, value: f64) -> usize {
        if self.counts.is_empty() || self.max <= self.min {
            return 0;
        }
        let t = (value - self.min) / (self.max - self.min);
        let idx = (t * self.counts.len() as f64).floor();
        (idx.max(0.0) as usize).min(self.counts.len() - 1)
    }
}

/// Streaming accumulator. Fixed memory regardless of how much is fed in.
///
/// Two-pass by design: the range must be known before the histogram can be
/// filled, so callers either supply a known range up front (block statistics
/// against a tensor-wide range) or run [`StatisticsAccumulator::finish`] on a
/// range pass first.
#[derive(Debug, Clone)]
pub struct StatisticsAccumulator {
    count: u64,
    min: f64,
    max: f64,
    mean: f64,
    /// Welford's running sum of squared deviations.
    m2: f64,
    l1: f64,
    sum_sq: f64,
    zeros: u64,
    positives: u64,
    negatives: u64,
    histogram: Option<Histogram>,
    approximate: bool,
}

impl Default for StatisticsAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl StatisticsAccumulator {
    pub fn new() -> Self {
        Self {
            count: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            mean: 0.0,
            m2: 0.0,
            l1: 0.0,
            sum_sq: 0.0,
            zeros: 0,
            positives: 0,
            negatives: 0,
            histogram: None,
            approximate: false,
        }
    }

    /// Bind a histogram range so binning can happen in the same pass.
    pub fn with_histogram(mut self, min: f64, max: f64, bins: usize) -> Self {
        self.histogram = Some(Histogram {
            min,
            max,
            counts: vec![0; bins.max(1)],
        });
        self
    }

    /// Mark the result as sampled rather than exhaustive.
    pub fn mark_approximate(mut self) -> Self {
        self.approximate = true;
        self
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn push(&mut self, x: f64) {
        self.count += 1;
        if x < self.min {
            self.min = x;
        }
        if x > self.max {
            self.max = x;
        }
        // Welford update.
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        self.m2 += delta * (x - self.mean);

        self.l1 += x.abs();
        self.sum_sq += x * x;
        if x == 0.0 {
            self.zeros += 1;
        } else if x > 0.0 {
            self.positives += 1;
        } else {
            self.negatives += 1;
        }
        if let Some(h) = &mut self.histogram {
            let bin = h.bin_of(x);
            h.counts[bin] += 1;
        }
    }

    pub fn extend(&mut self, values: impl IntoIterator<Item = f64>) {
        for v in values {
            self.push(v);
        }
    }

    /// Decode a raw byte run and accumulate it, without materializing an
    /// intermediate `Vec<f64>` of the whole run.
    pub fn push_bytes(&mut self, bytes: &[u8], dtype: DType) -> Result<()> {
        let w = dtype.size_in_bytes() as usize;
        if w == 0 || bytes.len() % w != 0 {
            return Err(QError::malformed(
                "statistics",
                format!("{} bytes is not a multiple of {w}", bytes.len()),
            ));
        }
        for chunk in bytes.chunks_exact(w) {
            self.push(dtype.decode_scalar(chunk)?);
        }
        Ok(())
    }

    pub fn finish(self, backend: impl Into<String>) -> Result<TensorStatistics> {
        if self.count == 0 {
            return Err(QError::QueryRejected(
                "cannot compute statistics over zero elements".into(),
            ));
        }
        let n = self.count as f64;
        Ok(TensorStatistics {
            count: self.count,
            min_value: self.min,
            max_value: self.max,
            mean: self.mean,
            variance: self.m2 / n,
            l1_norm: self.l1,
            l2_norm: self.sum_sq.sqrt(),
            zero_ratio: self.zeros as f64 / n,
            positive_ratio: self.positives as f64 / n,
            negative_ratio: self.negatives as f64 / n,
            histogram: self.histogram.unwrap_or(Histogram {
                min: self.min,
                max: self.max,
                counts: Vec::new(),
            }),
            approximate: self.approximate,
            algorithm_version: ALGORITHM_VERSION,
            backend: backend.into(),
        })
    }
}

/// Compute exact statistics over a slice, including a histogram.
///
/// Two passes: one for the range, one for the histogram. For a caller that
/// already knows the range (block statistics under a tensor-wide range), use
/// [`StatisticsAccumulator::with_histogram`] and stay single-pass.
pub fn compute_exact(values: &[f64], bins: usize) -> Result<TensorStatistics> {
    if values.is_empty() {
        return Err(QError::QueryRejected(
            "cannot compute statistics over zero elements".into(),
        ));
    }
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for &v in values {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    let mut acc = StatisticsAccumulator::new().with_histogram(min, max, bins);
    acc.extend(values.iter().copied());
    acc.finish("cpu-reference")
}

/// Cosine similarity between two equal-length vectors (ARCHITECTURE.md §15).
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> Result<f64> {
    require_same_len(a, b, "cosine_similarity")?;
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return Err(QError::QueryRejected(
            "cosine similarity is undefined when either vector is all zeros".into(),
        ));
    }
    Ok(dot / (na * nb))
}

/// Relative L2 distance `||a - b|| / ||a||` (ARCHITECTURE.md §15).
pub fn relative_l2(a: &[f64], b: &[f64]) -> Result<f64> {
    require_same_len(a, b, "relative_l2")?;
    let num: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt();
    let den: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    if den == 0.0 {
        return Err(QError::QueryRejected(
            "relative L2 is undefined when the reference vector is all zeros".into(),
        ));
    }
    Ok(num / den)
}

fn require_same_len(a: &[f64], b: &[f64], op: &str) -> Result<()> {
    if a.len() != b.len() {
        return Err(QError::QueryRejected(format!(
            "{op} requires equal lengths; got {} and {}",
            a.len(),
            b.len()
        )));
    }
    if a.is_empty() {
        return Err(QError::QueryRejected(format!("{op} on empty vectors")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected values below are computed **by hand**, not by the code under
    /// test. For `[1, 2, 3, 4]`:
    ///   mean      = 10/4                = 2.5
    ///   variance  = ((1.5² + .5² + .5² + 1.5²))/4 = (2.25+.25+.25+2.25)/4 = 1.25
    ///   L1        = 1+2+3+4             = 10
    ///   L2        = sqrt(1+4+9+16)      = sqrt(30) = 5.477225575051661
    const SAMPLE: [f64; 4] = [1.0, 2.0, 3.0, 4.0];

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-12, "expected {b}, got {a}");
    }

    #[test]
    fn hand_computed_moments_on_a_small_fixture() {
        let s = compute_exact(&SAMPLE, 4).unwrap();
        assert_eq!(s.count, 4);
        close(s.min_value, 1.0);
        close(s.max_value, 4.0);
        close(s.mean, 2.5);
        close(s.variance, 1.25);
        close(s.std_dev(), 1.25f64.sqrt());
        close(s.l1_norm, 10.0);
        close(s.l2_norm, 30f64.sqrt());
        assert_eq!(s.algorithm_version, ALGORITHM_VERSION);
        assert_eq!(s.backend, "cpu-reference");
        assert!(!s.approximate);
    }

    #[test]
    fn hand_computed_ratios_with_signs_and_zeros() {
        // [-2, 0, 0, 3, -1]: 2 zeros, 1 positive, 2 negative, n = 5.
        // L1 = 2+0+0+3+1 = 6;  L2 = sqrt(4+0+0+9+1) = sqrt(14)
        // mean = 0/5 = 0;  variance = (4+0+0+9+1)/5 = 2.8
        let v = [-2.0, 0.0, 0.0, 3.0, -1.0];
        let s = compute_exact(&v, 8).unwrap();
        close(s.zero_ratio, 0.4);
        close(s.positive_ratio, 0.2);
        close(s.negative_ratio, 0.4);
        close(s.l1_norm, 6.0);
        close(s.l2_norm, 14f64.sqrt());
        close(s.mean, 0.0);
        close(s.variance, 2.8);
        close(s.zero_ratio + s.positive_ratio + s.negative_ratio, 1.0);
    }

    #[test]
    fn hand_computed_histogram_binning() {
        // Range [1, 4] over 3 bins => edges at 1, 2, 3, 4.
        // 1.0 -> bin 0; 2.0 -> bin 1; 3.0 -> bin 2; 4.0 -> clamped into bin 2.
        let s = compute_exact(&SAMPLE, 3).unwrap();
        assert_eq!(s.histogram.bins(), 3);
        assert_eq!(s.histogram.counts, vec![1, 1, 2]);
        assert_eq!(s.histogram.total(), 4);
        close(s.histogram.min, 1.0);
        close(s.histogram.max, 4.0);
    }

    #[test]
    fn histogram_puts_the_maximum_in_the_last_bin_not_out_of_range() {
        let h = Histogram {
            min: 0.0,
            max: 1.0,
            counts: vec![0; 4],
        };
        assert_eq!(h.bin_of(0.0), 0);
        assert_eq!(h.bin_of(0.999), 3);
        assert_eq!(h.bin_of(1.0), 3);
        // Out-of-range values clamp rather than panic.
        assert_eq!(h.bin_of(-5.0), 0);
        assert_eq!(h.bin_of(5.0), 3);
    }

    #[test]
    fn constant_input_has_zero_variance_and_a_degenerate_range() {
        let s = compute_exact(&[7.0; 10], 4).unwrap();
        close(s.mean, 7.0);
        close(s.variance, 0.0);
        close(s.min_value, 7.0);
        close(s.max_value, 7.0);
        // min == max: every value lands in bin 0 rather than dividing by zero.
        assert_eq!(s.histogram.counts[0], 10);
    }

    #[test]
    fn welford_stays_accurate_where_the_naive_formula_collapses() {
        // Values ~1e8 with a spread of 1: E[x²] - E[x]² loses all precision
        // here in f64, while Welford recovers the true variance of 1.25.
        let base = 1e8;
        let v: Vec<f64> = SAMPLE.iter().map(|x| base + x).collect();
        let s = compute_exact(&v, 4).unwrap();
        assert!(
            (s.variance - 1.25).abs() < 1e-6,
            "variance drifted to {}",
            s.variance
        );
    }

    #[test]
    fn streaming_in_chunks_equals_computing_at_once() {
        let all = compute_exact(&SAMPLE, 4).unwrap();
        let mut acc = StatisticsAccumulator::new().with_histogram(1.0, 4.0, 4);
        acc.extend(SAMPLE[..2].iter().copied());
        acc.extend(SAMPLE[2..].iter().copied());
        let streamed = acc.finish("cpu-reference").unwrap();
        close(streamed.mean, all.mean);
        close(streamed.variance, all.variance);
        close(streamed.l2_norm, all.l2_norm);
        assert_eq!(streamed.count, all.count);
    }

    #[test]
    fn bytes_are_decoded_by_dtype() {
        let mut acc = StatisticsAccumulator::new();
        let bytes: Vec<u8> = SAMPLE
            .iter()
            .flat_map(|v| (*v as f32).to_le_bytes())
            .collect();
        acc.push_bytes(&bytes, DType::F32).unwrap();
        let s = acc.finish("cpu-reference").unwrap();
        assert_eq!(s.count, 4);
        close(s.mean, 2.5);

        // A ragged buffer is rejected rather than silently truncated.
        let mut acc2 = StatisticsAccumulator::new();
        assert!(acc2.push_bytes(&[0u8; 7], DType::F32).is_err());
    }

    #[test]
    fn empty_input_is_rejected_rather_than_returning_zeros() {
        assert!(compute_exact(&[], 4).is_err());
        assert!(StatisticsAccumulator::new().finish("cpu").is_err());
    }

    #[test]
    fn approximate_results_are_labelled() {
        let mut acc = StatisticsAccumulator::new().mark_approximate();
        acc.extend(SAMPLE);
        assert!(acc.finish("cpu-reference").unwrap().approximate);
    }

    #[test]
    fn an_exhaustive_statistic_is_labelled_aggregate_not_exact() {
        // `compute_exact` reads every element of `SAMPLE`, so the statistic is
        // exact *for that region*. `.plan/DATA_ARCHITECTURE.md` §8 names that
        // `aggregate`; `exact` names the stored values, not a statistic over
        // them, so claiming `exact` here would overstate what was produced.
        let s = compute_exact(&SAMPLE, 4).unwrap();
        assert!(!s.approximate);
        assert_eq!(s.fidelity(), StatisticsFidelity::Aggregate);
        assert_eq!(s.fidelity().as_str(), "aggregate");
        assert_ne!(s.fidelity().as_str(), "exact");
    }

    #[test]
    fn a_sampled_statistic_is_labelled_sampled_never_aggregate() {
        let mut acc = StatisticsAccumulator::new().mark_approximate();
        acc.extend(SAMPLE);
        let s = acc.finish("cpu-reference").unwrap();
        assert!(s.approximate);
        assert_eq!(s.fidelity(), StatisticsFidelity::Sampled);
        assert_eq!(s.fidelity().as_str(), "sampled");
        assert_ne!(s.fidelity().as_str(), "aggregate");
    }

    #[test]
    fn the_fidelity_mapping_is_total_and_cannot_disagree_with_the_flag() {
        // Both inputs, both outputs, and the inverse — so there is no third
        // label and no way for the boolean and the string to drift apart.
        for approximate in [false, true] {
            let f = StatisticsFidelity::from_approximate(approximate);
            assert_eq!(f.is_approximate(), approximate, "for {approximate}");
        }
        assert_eq!(
            StatisticsFidelity::from_approximate(false),
            StatisticsFidelity::Aggregate
        );
        assert_eq!(
            StatisticsFidelity::from_approximate(true),
            StatisticsFidelity::Sampled
        );
        // Exactly two labels exist, and both are spelled here.
        assert_eq!(StatisticsFidelity::Aggregate.as_str(), "aggregate");
        assert_eq!(StatisticsFidelity::Sampled.as_str(), "sampled");
    }

    #[test]
    fn hand_computed_cosine_similarity_and_relative_l2() {
        // a = [1, 0], b = [0, 1]  -> orthogonal, cosine 0
        close(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).unwrap(), 0.0);
        // a = [1, 1], b = [2, 2]  -> parallel, cosine 1
        close(cosine_similarity(&[1.0, 1.0], &[2.0, 2.0]).unwrap(), 1.0);
        // a = [1, 0], b = [-1, 0] -> antiparallel, cosine -1
        close(cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]).unwrap(), -1.0);

        // a = [3, 4] (norm 5), b = [0, 4]; ||a-b|| = 3; 3/5 = 0.6
        close(relative_l2(&[3.0, 4.0], &[0.0, 4.0]).unwrap(), 0.6);
        close(relative_l2(&[3.0, 4.0], &[3.0, 4.0]).unwrap(), 0.0);
    }

    #[test]
    fn comparison_metrics_reject_undefined_inputs() {
        assert!(cosine_similarity(&[1.0], &[1.0, 2.0]).is_err());
        assert!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]).is_err());
        assert!(relative_l2(&[0.0, 0.0], &[1.0, 1.0]).is_err());
        assert!(cosine_similarity(&[], &[]).is_err());
    }
}
