//! # q-quant — quantisation simulation
//!
//! Data plane: **Tensor Tile Plane** (ARCHITECTURE.md §2.1, §5.4).
//! Subsystem: `.plan/DIAGNOSTIC_ARCHITECTURE.md` §2, §3. Requirement `QUANT-001`.
//!
//! Given a slice of decoded `f32` values and a [`QuantConfig`], produce the
//! dequantised counterpart `Ŵ = dequant(quant(W, config))`.
//!
//! ## What this crate is not
//!
//! **Values in, values out.** No file access, no catalog, no I/O policy, and no
//! dependency on any other Quatricmorph crate. That separation is deliberate:
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` §2 keeps this crate "the piece a later
//! module reuses when *verifying* someone else's quantisation", and `TASK.md`
//! §Risks names a dependency on `q-source` here as a review failure. The cost is
//! a local [`QuantError`]; the price is worth paying and the `NotImplemented`
//! variant's message is byte-identical to `q_source::error::QError`'s so that a
//! conversion added downstream changes no asserted string.
//!
//! ## What it simulates, and what that word means
//!
//! v1 **simulates** quantisation from a base-precision checkpoint
//! (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §1). It never reads a third-party
//! quantised artifact — that is `QUANT-010`, a seam — and it never writes one.
//! Every value produced here is therefore [`Provenance::Simulated`]. That enum
//! has exactly one variant, so there is no way to spell "measured": an API that
//! cannot claim a measurement cannot accidentally be used to.
//!
//! Returning only the dequantised values and never the integer codes is the
//! same discipline: v1 diagnoses, it does not emit a quantised model.
//!
//! ## Exactness
//!
//! [`QuantFidelity`] carries `.plan/DATA_ARCHITECTURE.md` §8's label. A
//! reconstruction is §8's `quantized` — *"values present but lossily encoded"* —
//! unless it reproduced its input bit-for-bit, in which case it is `exact`. The
//! label is **derived from bit equality**, never asserted, so a reconstruction
//! error can never be presented as an exact value.
//!
//! Quantisation error is arithmetic. It says nothing about what a tensor means
//! or what a layer does, and nothing in this crate claims otherwise.
//!
//! ## The reference
//!
//! Every golden value under `tests/goldens/` was produced by
//! `python/reference/quantise_reference.py`, written from
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1 **before** this crate existed. See
//! `.plan/evidence/QM-0120.md` for the provenance and how to re-derive it.

pub mod rtn;

use serde::{Deserialize, Serialize};

pub use rtn::{
    derive_params, derive_params_named, group_extents, round_half_to_even, simulate, simulate_into,
    simulate_into_named, simulate_per_group_into, GroupExtents,
};

/// Bump when an arithmetic rule changes. Part of the cache key
/// (ARCHITECTURE.md §13.2), so results computed under an older rule are
/// invalidated rather than silently mixed with newer ones.
pub const ALGORITHM_VERSION: u32 = 1;

/// The rank ceiling ADR-010 sets for the MVP. Rank above this **refuses**; it is
/// never flattened, because a flattened picture is confidently wrong in a form
/// the viewer cannot detect.
pub const MAX_IMPLEMENTED_RANK: usize = 3;

/// The widest integer code the v1 surface stores.
///
/// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1's last error row — *"`n` would exceed
/// the dtype's range: refuse at config validation, before any arithmetic"* — is
/// this constant plus [`QuantConfig::validate`]. [`Precision`] is closed today,
/// so nothing can currently exceed it; the check exists so that a variant added
/// later cannot slip a code range past the storage width it is written into.
pub const MAX_CODE_BITS: u32 = 8;

pub type Result<T> = std::result::Result<T, QuantError>;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// This crate's error type.
///
/// [`QuantError::NotImplemented`]'s `Display` is **byte-identical** to
/// `q_source::error::QError::NotImplemented`'s, and [`QuantError::requirement_id`]
/// mirrors its accessor, so a downstream `From<QuantError> for QError` maps that
/// variant one-to-one and every other variant to `QError::QueryRejected(
/// err.to_string())` without changing any message a test asserts.
///
/// Every rejection variant that names a unit does so because
/// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1 requires it: *"refuse the group,
/// naming the tensor and offset"*.
// `PartialEq` but not `Eq`: two variants carry an `f32`, which has no total
// equality. Tests compare `kind()` and the message rather than relying on
// float equality of a scale.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum QuantError {
    /// Deliberately unbuilt. `requirement` is the `STATUS.md` requirement ID,
    /// `detail` says what is missing and what exists instead.
    #[error("not implemented [{requirement}]: {detail}")]
    NotImplemented {
        requirement: &'static str,
        detail: String,
    },

    /// A configuration or parameter pair was rejected **before** any arithmetic.
    #[error("quantisation config rejected: {0}")]
    ConfigRejected(String),

    /// The unit holds `NaN` or `±Inf`. A checkpoint with non-finite weights is a
    /// finding, reported as one, not something to quantise.
    #[error("{unit} holds a non-finite value at index {index}: {value}; non-finite weights are a finding, not something to quantise")]
    NonFinite {
        unit: String,
        index: usize,
        value: f32,
    },

    /// The scale is zero, subnormal, infinite, `NaN`, or not positive.
    /// Quantising with it would divide by zero or emit infinities.
    #[error("{unit} scale {scale:e} is not a normal positive f32; quantising with it would divide by zero or emit infinities")]
    ScaleNotNormal { unit: String, scale: f32 },

    /// `(q − z) · s` overflowed `f32`, so a finite input would reconstruct to an
    /// infinity. `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1: never silently produce
    /// infinities.
    ///
    /// Distinct from [`QuantError::ScaleNotNormal`]: the scale here is a perfectly
    /// normal `f32` and every input is finite. It is the product that overflows,
    /// and it can only be detected per value, so the output buffer's contents are
    /// **unspecified** when this is returned.
    #[error("{unit} reconstructs to a non-finite value at index {index}: code {code} times scale {scale:e} overflows f32; refused rather than emitted")]
    ReconstructionNotFinite {
        unit: String,
        index: usize,
        code: i64,
        scale: f32,
    },

    /// The zero point cannot be represented, or contradicts the config.
    #[error("{unit} zero point is out of range: {detail}")]
    ZeroPointOutOfRange { unit: String, detail: String },

    /// There is nothing to quantise, and no scale may be fabricated for it.
    #[error("{unit} is empty; there is nothing to quantise")]
    EmptyUnit { unit: String },

    /// Two lengths that must agree do not. Raised before execution.
    #[error("{operation} requires equal lengths; got {left} and {right}")]
    LengthMismatch {
        operation: &'static str,
        left: usize,
        right: usize,
    },
}

impl QuantError {
    pub fn not_implemented(requirement: &'static str, detail: impl Into<String>) -> Self {
        QuantError::NotImplemented {
            requirement,
            detail: detail.into(),
        }
    }

    pub fn config_rejected(detail: impl Into<String>) -> Self {
        QuantError::ConfigRejected(detail.into())
    }

    /// The requirement ID attached to a `NotImplemented`, for HTTP 501 bodies.
    /// Mirrors `QError::requirement_id`.
    pub fn requirement_id(&self) -> Option<&'static str> {
        match self {
            QuantError::NotImplemented { requirement, .. } => Some(requirement),
            _ => None,
        }
    }

    /// A stable machine-readable kind.
    ///
    /// These strings are **the reference's own vocabulary**: they are the `kind`
    /// field `python/reference/quantise_reference.py` emits for each refusal, so
    /// the refusal paths are checked differentially against the reference rather
    /// than merely being self-consistent.
    pub fn kind(&self) -> &'static str {
        match self {
            QuantError::NotImplemented { .. } => "not_implemented",
            QuantError::ConfigRejected(_) => "config_rejected",
            QuantError::NonFinite { .. } => "non_finite",
            QuantError::ScaleNotNormal { .. } => "scale_not_normal",
            QuantError::ReconstructionNotFinite { .. } => "reconstruction_not_finite",
            QuantError::ZeroPointOutOfRange { .. } => "zero_point_out_of_range",
            QuantError::EmptyUnit { .. } => "empty_unit",
            QuantError::LengthMismatch { .. } => "length_mismatch",
        }
    }
}

// ---------------------------------------------------------------------------
// Naming a unit
// ---------------------------------------------------------------------------

/// Identifies one granularity unit, **for error messages only**.
///
/// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1 requires a refused group to name "the
/// tensor and offset", which a signature taking only `&[f32]` cannot do. This
/// borrows its name and copies a `u64`, so naming a unit costs no allocation on
/// the streaming path — the error path allocates, the success path does not.
///
/// It is a label, not a handle: it opens nothing and resolves nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitId<'a> {
    /// The tensor's canonical address, or any caller-chosen label.
    pub tensor: &'a str,
    /// Element offset of this unit within that tensor.
    pub offset: u64,
}

impl<'a> UnitId<'a> {
    pub fn new(tensor: &'a str, offset: u64) -> Self {
        UnitId { tensor, offset }
    }
}

impl UnitId<'static> {
    /// The label used by the signatures `TASK.md` §Data Contracts fixes, which
    /// take no unit. It still reads as a refusal, it simply cannot say which
    /// tensor — so callers on the streaming path pass a real one.
    pub const UNNAMED: Self = UnitId {
        tensor: "<unnamed unit>",
        offset: 0,
    };
}

impl std::fmt::Display for UnitId<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tensor {} at offset {}", self.tensor, self.offset)
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Target integer precision. **Closed**: int8 and int4 are the whole v1 surface
/// (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3), and every other scheme lives on the
/// open [`QuantScheme`] where it refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    Int8,
    Int4,
}

impl Precision {
    pub fn bits(self) -> u32 {
        match self {
            Precision::Int8 => 8,
            Precision::Int4 => 4,
        }
    }

    /// `n = 2^bits`. Explicit rather than derived, so the level count is a
    /// stated fact and not a shift the reader has to perform.
    pub fn levels(self) -> u32 {
        match self {
            Precision::Int8 => 256,
            Precision::Int4 => 16,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Precision::Int8 => "int8",
            Precision::Int4 => "int4",
        }
    }
}

/// Which values share one scale. Defined against the **canonical** axis
/// semantics NSIR assigns, not raw tensor order
/// (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    PerTensor,
    PerOutputChannel,
    PerGroup { size: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZeroPoint {
    Symmetric,
    Asymmetric,
}

impl ZeroPoint {
    pub fn as_str(self) -> &'static str {
        match self {
            ZeroPoint::Symmetric => "symmetric",
            ZeroPoint::Asymmetric => "asymmetric",
        }
    }
}

/// The only v1 variant. Stated explicitly because half-away-from-zero disagrees
/// with NumPy on exactly the boundary values the golden vectors contain
/// (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundMode {
    NearestEven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantConfig {
    pub precision: Precision,
    pub granularity: Granularity,
    pub zero_point: ZeroPoint,
    pub round: RoundMode,
}

impl QuantConfig {
    /// A per-tensor config, the shape most tests want.
    pub fn per_tensor(precision: Precision, zero_point: ZeroPoint) -> Self {
        QuantConfig {
            precision,
            granularity: Granularity::PerTensor,
            zero_point,
            round: RoundMode::NearestEven,
        }
    }

    pub fn per_group(precision: Precision, zero_point: ZeroPoint, size: u32) -> Self {
        QuantConfig {
            precision,
            granularity: Granularity::PerGroup { size },
            zero_point,
            round: RoundMode::NearestEven,
        }
    }

    /// `(qmin, qmax)` — the two `clamp` bounds in
    /// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1, widened to `i64` so forming a
    /// code index cannot itself overflow.
    pub fn code_range(&self) -> (i64, i64) {
        let n = i64::from(self.precision.levels());
        match self.zero_point {
            ZeroPoint::Symmetric => (-(n / 2), n / 2 - 1),
            ZeroPoint::Asymmetric => (0, n - 1),
        }
    }

    /// Validate the config **before any arithmetic**.
    ///
    /// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1's last error row — *"`n` would
    /// exceed the dtype's range: refuse at config validation, before any
    /// arithmetic"* — is enforced here rather than discovered mid-loop.
    pub fn validate(&self) -> Result<()> {
        // Exhaustive rather than ignored: adding a rounding mode must not
        // silently inherit nearest-even's goldens.
        match self.round {
            RoundMode::NearestEven => {}
        }
        let levels = self.precision.levels();
        let bits = self.precision.bits();
        if levels != 1u32 << bits {
            return Err(QuantError::config_rejected(format!(
                "{}: {levels} levels contradicts {bits} bits; n must be 2^bits",
                self.precision.as_str()
            )));
        }
        if bits < 2 {
            return Err(QuantError::config_rejected(format!(
                "{}: {bits} bits leaves no code range to quantise into",
                self.precision.as_str()
            )));
        }
        if bits > MAX_CODE_BITS {
            return Err(QuantError::config_rejected(format!(
                "{}: n = {levels} would exceed the dtype's range; a simulated code is \
                 stored in at most {MAX_CODE_BITS} bits",
                self.precision.as_str()
            )));
        }
        if let Granularity::PerGroup { size } = self.granularity {
            if size == 0 {
                return Err(QuantError::config_rejected(
                    "group size 0 would produce no groups; per-group granularity needs a \
                     positive size",
                ));
            }
        }
        Ok(())
    }

    /// Validate the config against a tensor shape, **before any arithmetic**.
    ///
    /// This is the only place granularity-versus-shape validation can live in a
    /// crate that never sees a tensor: the caller holds the shape, this crate
    /// holds the rule. Rank above [`MAX_IMPLEMENTED_RANK`] **refuses** rather
    /// than being flattened (ADR-010, `GRID-007`).
    pub fn validate_for_shape(&self, shape: &[u64]) -> Result<()> {
        self.validate()?;
        if shape.len() > MAX_IMPLEMENTED_RANK {
            return Err(QuantError::not_implemented(
                "GRID-007",
                format!(
                    "rank {} exceeds the implemented ceiling of {MAX_IMPLEMENTED_RANK} \
                     (ADR-010); shape {shape:?} is refused rather than flattened, because a \
                     flattened view invites the reader to read adjacency between values that \
                     are not adjacent",
                    shape.len()
                ),
            ));
        }
        if shape.contains(&0) {
            return Err(QuantError::config_rejected(format!(
                "shape {shape:?} has a zero-length axis; there is nothing to quantise"
            )));
        }
        if matches!(self.granularity, Granularity::PerOutputChannel) && shape.is_empty() {
            return Err(QuantError::config_rejected(
                "per-output-channel granularity needs an output-channel axis, and a rank-0 \
                 tensor has none",
            ));
        }
        Ok(())
    }

    /// Reject a unit length that cannot be a unit of this granularity under
    /// `shape`, **before any arithmetic**.
    ///
    /// Deliberately conservative. It asserts only what is true regardless of
    /// which axis NSIR calls the output channel, because binding granularity to a
    /// named axis needs a whole-axis view this crate never has
    /// (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.2 and `V1-11` belong to `QM-0122`).
    pub fn validate_unit_len_for_shape(&self, shape: &[u64], unit_len: usize) -> Result<()> {
        self.validate_for_shape(shape)?;
        // The empty product is 1, which is exactly right: ADR-010's rank-0 case
        // is "one cell at the tensor anchor".
        let elements: u64 = shape.iter().product();
        let elements = usize::try_from(elements).map_err(|_| {
            QuantError::config_rejected(format!(
                "shape {shape:?} describes more elements than this machine can index"
            ))
        })?;
        if unit_len == 0 {
            return Err(QuantError::config_rejected(
                "a granularity unit of zero values describes no data".to_string(),
            ));
        }
        if unit_len > elements {
            return Err(QuantError::config_rejected(format!(
                "a unit of {unit_len} values cannot come from shape {shape:?}, which holds \
                 {elements}"
            )));
        }
        match self.granularity {
            Granularity::PerTensor if unit_len != elements => {
                Err(QuantError::config_rejected(format!(
                    "per-tensor granularity needs the whole tensor as one unit: shape \
                     {shape:?} holds {elements} values, not {unit_len}"
                )))
            }
            Granularity::PerOutputChannel if elements % unit_len != 0 => {
                Err(QuantError::config_rejected(format!(
                    "a channel of {unit_len} values does not partition shape {shape:?} \
                     ({elements} values)"
                )))
            }
            _ => Ok(()),
        }
    }
}

/// Scale and zero-point parameters for one granularity unit.
///
/// `zero` is always `0` under [`ZeroPoint::Symmetric`]; a non-zero value there is
/// an inconsistent pair and is refused rather than ignored.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QuantParams {
    pub scale: f32,
    pub zero: i32,
}

impl QuantParams {
    pub fn new(scale: f32, zero: i32) -> Self {
        QuantParams { scale, zero }
    }

    /// The parameters of an exactly-representable identity: scale `1`, zero `0`.
    /// This is what §3.1's all-zero row produces.
    pub const IDENTITY: Self = QuantParams {
        scale: 1.0,
        zero: 0,
    };
}

// ---------------------------------------------------------------------------
// Schemes — the open enum, and the refusal
// ---------------------------------------------------------------------------

/// Quantisation schemes. **Open**: v1 implements RTN only and names the others
/// in the refusal (`.plan/PRODUCT_SCOPE.md` §"Additional quantisation schemes",
/// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3).
///
/// RTN is not claimed to be the best quantiser. It is the one every other method
/// is measured against, it is exactly reproducible in NumPy, and it needs no
/// calibration data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum QuantScheme {
    /// Round-to-nearest. The whole of v1.
    Rtn,
    /// 4-bit NormalFloat. `QUANT-011`.
    Nf4,
    /// Microscaling FP4. `QUANT-011`.
    Mxfp4,
    /// AWQ-style activation-aware scaling. `QUANT-011`.
    AwqScaling,
    /// GPTQ-style error feedback. `QUANT-011`.
    GptqErrorFeedback,
}

impl QuantScheme {
    /// Everything v1 implements. One entry, and the refusal quotes this list so
    /// it cannot drift from reality.
    pub const IMPLEMENTED: &'static [QuantScheme] = &[QuantScheme::Rtn];

    pub fn as_str(self) -> &'static str {
        match self {
            QuantScheme::Rtn => "rtn",
            QuantScheme::Nf4 => "nf4",
            QuantScheme::Mxfp4 => "mxfp4",
            QuantScheme::AwqScaling => "awq-scaling",
            QuantScheme::GptqErrorFeedback => "gptq-error-feedback",
        }
    }

    pub fn is_implemented(self) -> bool {
        Self::IMPLEMENTED.contains(&self)
    }

    /// Refuse an unimplemented scheme, naming `QUANT-011` and listing what is
    /// implemented. Never guesses, never approximates: the discipline
    /// `SRC-014` already applies to unknown dtypes.
    pub fn require_implemented(self) -> Result<()> {
        if self.is_implemented() {
            return Ok(());
        }
        let implemented: Vec<&str> = Self::IMPLEMENTED.iter().map(|s| s.as_str()).collect();
        Err(QuantError::not_implemented(
            "QUANT-011",
            format!(
                "quantisation scheme {} is not implemented; v1 implements [{}] only — \
                 round-to-nearest at int8 or int4, per-tensor, per-output-channel or \
                 per-group, symmetric or asymmetric, with nearest-even rounding. It is \
                 refused rather than approximated with RTN, which would produce a \
                 confidently wrong number.",
                self.as_str(),
                implemented.join(", ")
            ),
        ))
    }
}

// ---------------------------------------------------------------------------
// Fidelity and provenance
// ---------------------------------------------------------------------------

/// The `.plan/DATA_ARCHITECTURE.md` §8 fidelity of a reconstruction.
///
/// Two variants, and the mapping from bit equality is **total**, the same
/// discipline `q_statistics::StatisticsFidelity` applies to sampling:
///
/// * [`QuantFidelity::Quantized`] — §8's *"values present but lossily
///   encoded"*. This is what a reconstruction normally is, and it is the label
///   §8 requires for the whole point of this crate: a quantisation error is a
///   loss, and presenting it as an exact value would be a false claim.
/// * [`QuantFidelity::Exact`] — §8's *"the values as stored in the
///   checkpoint"*. Earned **only** when every reconstructed value equals its
///   input bit-for-bit, which happens when each input is already a code times
///   the scale. Never asserted by a caller; always derived.
///
/// §8's `aggregate` and `sampled` do not apply here: this crate produces values,
/// not statistics over them, and it reads every value it is handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantFidelity {
    Exact,
    Quantized,
}

impl QuantFidelity {
    /// The one and only mapping. Derived from **bit** equality, not a
    /// tolerance — a value that differs in its last bit is a loss, and calling
    /// it exact would be the failure this label exists to prevent.
    pub fn of_round_trip(original: &[f32], reconstructed: &[f32]) -> Result<Self> {
        if original.len() != reconstructed.len() {
            return Err(QuantError::LengthMismatch {
                operation: "QuantFidelity::of_round_trip",
                left: original.len(),
                right: reconstructed.len(),
            });
        }
        if original.is_empty() {
            // A label over no data would describe no data. Refused rather than
            // vacuously reported as `exact`.
            return Err(QuantError::EmptyUnit {
                unit: "a round trip over zero values".to_string(),
            });
        }
        let exact = original
            .iter()
            .zip(reconstructed)
            .all(|(a, b)| a.to_bits() == b.to_bits());
        Ok(if exact {
            QuantFidelity::Exact
        } else {
            QuantFidelity::Quantized
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            QuantFidelity::Exact => "exact",
            QuantFidelity::Quantized => "quantized",
        }
    }

    /// `true` when values were lost. The inverse of the `Exact` case, so the
    /// round trip through [`Self::as_str`] is checkable.
    pub fn is_lossy(self) -> bool {
        matches!(self, QuantFidelity::Quantized)
    }
}

/// Where a value came from.
///
/// **One variant, on purpose.** v1 simulates quantisation from a base-precision
/// checkpoint and never reads a quantised artifact
/// (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §1; ingestion is the `QUANT-010` seam).
/// There is therefore no way to spell "measured", and no way for a simulated
/// result to be labelled as a measurement of a real quantised checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Simulated,
}

impl Provenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Provenance::Simulated => "simulated",
        }
    }
}

/// The provenance of every value this crate produces.
pub const PROVENANCE: Provenance = Provenance::Simulated;

/// The sentence a surface must carry alongside any result derived from this
/// crate. `.plan/DIAGNOSTIC_ARCHITECTURE.md` §8 forbids presenting weight-space
/// error as anything more than it is.
pub const SIMULATION_CAVEAT: &str = "Simulated quantisation of a base-precision \
checkpoint; not a measurement of a real quantised checkpoint. Weight-space error \
only. Accuracy impact is not measured — run your evaluation on the recommended \
configuration.";

#[cfg(test)]
mod tests {
    use super::*;

    // -- the refusal of every unimplemented scheme -------------------------

    #[test]
    fn requesting_nf4_refuses_naming_quant_011_and_listing_what_is_implemented() {
        // `TASK.md` §Test Cases row 8 spells this `Precision::Nf4`. NF4 is a
        // SCHEME, not an integer precision: `.plan/DIAGNOSTIC_ARCHITECTURE.md`
        // §3 says "NF4 and MXFP4 are named variants of `QuantScheme` that refuse
        // with `QUANT-011`", and `.plan/PRODUCT_SCOPE.md` says "`QuantScheme` is
        // an open enum ... v1 implements RTN only and names the others in the
        // refusal". `Precision` stays closed at {Int8, Int4} exactly as
        // §Data Contracts fixes it, so the row is implemented here.
        let err = QuantScheme::Nf4.require_implemented().unwrap_err();
        assert_eq!(err.requirement_id(), Some("QUANT-011"));
        let message = err.to_string();
        assert!(
            message.starts_with("not implemented [QUANT-011]:"),
            "the message must match QError::NotImplemented byte-for-byte; got {message:?}"
        );
        assert!(message.contains("nf4"), "got {message:?}");
        assert!(
            message.contains("rtn"),
            "the refusal must list what IS implemented; got {message:?}"
        );
        assert!(
            message.contains("refused rather than approximated"),
            "got {message:?}"
        );
    }

    #[test]
    fn every_unimplemented_scheme_refuses_and_only_rtn_is_implemented() {
        for scheme in [
            QuantScheme::Nf4,
            QuantScheme::Mxfp4,
            QuantScheme::AwqScaling,
            QuantScheme::GptqErrorFeedback,
        ] {
            assert!(!scheme.is_implemented(), "{scheme:?}");
            let err = scheme.require_implemented().unwrap_err();
            assert_eq!(err.requirement_id(), Some("QUANT-011"), "{scheme:?}");
            assert_eq!(err.kind(), "not_implemented", "{scheme:?}");
            assert!(
                err.to_string().contains(scheme.as_str()),
                "the refusal must name the scheme requested; {scheme:?}"
            );
        }
        assert!(QuantScheme::Rtn.is_implemented());
        QuantScheme::Rtn.require_implemented().unwrap();
        assert_eq!(QuantScheme::IMPLEMENTED, &[QuantScheme::Rtn]);
        assert_eq!(
            QuantScheme::IMPLEMENTED.len(),
            1,
            "exactly one scheme is implemented in v1; the refusal quotes this list"
        );
    }

    #[test]
    fn an_unknown_scheme_name_cannot_be_guessed_into_existence() {
        // The same discipline `SRC-014` applies to unknown dtypes: a name this
        // crate does not know is not silently mapped onto RTN.
        let parsed: std::result::Result<QuantScheme, _> =
            serde_json::from_str::<QuantScheme>("\"gptq_error_feedback\"")
                .map_err(|e| e.to_string());
        assert_eq!(parsed.unwrap(), QuantScheme::GptqErrorFeedback);
        assert!(
            serde_json::from_str::<QuantScheme>("\"secret_new_scheme\"").is_err(),
            "an unknown scheme name must be rejected, not defaulted"
        );
        assert!(
            serde_json::from_str::<Precision>("\"int3\"").is_err(),
            "an unknown precision must be rejected, not defaulted"
        );
        assert!(
            serde_json::from_str::<RoundMode>("\"half_away_from_zero\"").is_err(),
            "the only v1 rounding mode is nearest-even"
        );
    }

    // -- ADR-010 -----------------------------------------------------------

    #[test]
    fn rank_above_three_is_refused_rather_than_flattened() {
        let config = QuantConfig::per_tensor(Precision::Int8, ZeroPoint::Symmetric);
        // Rank 0 through 3 are implemented (ADR-010's binding table).
        for shape in [vec![], vec![8], vec![4, 8], vec![2, 4, 8]] {
            config
                .validate_for_shape(&shape)
                .unwrap_or_else(|e| panic!("rank {} must be accepted: {e}", shape.len()));
        }
        // Rank 4 refuses, carrying GRID-007 — it is never flattened to [32, 16384].
        let err = config.validate_for_shape(&[32, 4, 128, 128]).unwrap_err();
        assert_eq!(err.requirement_id(), Some("GRID-007"));
        let message = err.to_string();
        assert!(message.contains("rank 4"), "got {message:?}");
        assert!(
            message.contains("ADR-010") && message.contains("flattened"),
            "got {message:?}"
        );
        assert!(config.validate_for_shape(&[2; 9]).is_err());
    }

    #[test]
    fn a_zero_length_axis_and_a_rank_zero_channel_axis_are_both_refused() {
        let per_tensor = QuantConfig::per_tensor(Precision::Int8, ZeroPoint::Symmetric);
        assert_eq!(
            per_tensor.validate_for_shape(&[4, 0]).unwrap_err().kind(),
            "config_rejected"
        );
        let per_channel = QuantConfig {
            precision: Precision::Int8,
            granularity: Granularity::PerOutputChannel,
            zero_point: ZeroPoint::Symmetric,
            round: RoundMode::NearestEven,
        };
        // Rank 0 has no channel axis to put a scale on.
        assert_eq!(
            per_channel.validate_for_shape(&[]).unwrap_err().kind(),
            "config_rejected"
        );
        // Rank 1 does: a bias is one value per output channel.
        per_channel.validate_for_shape(&[8]).unwrap();
    }

    // -- shape mismatch, rejected before execution -------------------------

    #[test]
    fn a_unit_length_that_cannot_come_from_the_shape_is_rejected_before_execution() {
        let per_tensor = QuantConfig::per_tensor(Precision::Int8, ZeroPoint::Symmetric);
        // A per-tensor unit IS the whole tensor.
        per_tensor.validate_unit_len_for_shape(&[4, 8], 32).unwrap();
        assert_eq!(
            per_tensor
                .validate_unit_len_for_shape(&[4, 8], 31)
                .unwrap_err()
                .kind(),
            "config_rejected"
        );
        // More values than the tensor holds is a mismatch under any granularity.
        for granularity in [
            Granularity::PerTensor,
            Granularity::PerOutputChannel,
            Granularity::PerGroup { size: 8 },
        ] {
            let config = QuantConfig {
                precision: Precision::Int8,
                granularity,
                zero_point: ZeroPoint::Symmetric,
                round: RoundMode::NearestEven,
            };
            assert_eq!(
                config
                    .validate_unit_len_for_shape(&[4, 8], 33)
                    .unwrap_err()
                    .kind(),
                "config_rejected",
                "for {granularity:?}"
            );
            assert_eq!(
                config
                    .validate_unit_len_for_shape(&[4, 8], 0)
                    .unwrap_err()
                    .kind(),
                "config_rejected",
                "for {granularity:?}"
            );
        }
        // A channel must partition the tensor.
        let per_channel = QuantConfig {
            precision: Precision::Int8,
            granularity: Granularity::PerOutputChannel,
            zero_point: ZeroPoint::Symmetric,
            round: RoundMode::NearestEven,
        };
        per_channel.validate_unit_len_for_shape(&[4, 8], 8).unwrap();
        assert_eq!(
            per_channel
                .validate_unit_len_for_shape(&[4, 8], 7)
                .unwrap_err()
                .kind(),
            "config_rejected"
        );
        // And the rank ceiling still applies through this entry point.
        assert_eq!(
            per_tensor
                .validate_unit_len_for_shape(&[2, 2, 2, 2], 16)
                .unwrap_err()
                .requirement_id(),
            Some("GRID-007")
        );
    }

    // -- fidelity ----------------------------------------------------------

    #[test]
    fn a_lossy_reconstruction_is_labelled_quantized_never_exact() {
        // `.plan/DATA_ARCHITECTURE.md` §8: `quantized` means "values present but
        // lossily encoded". A quantisation error must never be presented as an
        // exact value — that is the whole reason this label exists.
        let original = [0.1f32, 0.2, 0.3];
        let reconstructed = [0.09333334f32, 0.20000002, 0.29333335];
        let fidelity = QuantFidelity::of_round_trip(&original, &reconstructed).unwrap();
        assert_eq!(fidelity, QuantFidelity::Quantized);
        assert_eq!(fidelity.as_str(), "quantized");
        assert_ne!(fidelity.as_str(), "exact");
        assert!(fidelity.is_lossy());
    }

    #[test]
    fn a_reconstruction_is_labelled_exact_only_when_every_bit_survived() {
        let original = [-1.0f32, 0.0, 1.0];
        let fidelity = QuantFidelity::of_round_trip(&original, &original).unwrap();
        assert_eq!(fidelity, QuantFidelity::Exact);
        assert_eq!(fidelity.as_str(), "exact");
        assert!(!fidelity.is_lossy());

        // One bit is enough to lose the label: 1.0 versus its next neighbour.
        let nudged = [-1.0f32, 0.0, f32::from_bits(1.0f32.to_bits() + 1)];
        assert_eq!(
            QuantFidelity::of_round_trip(&original, &nudged).unwrap(),
            QuantFidelity::Quantized,
            "a one-ULP difference is a loss, not an exact value"
        );
        // And a sign-of-zero difference is a loss too: `-0.0 == 0.0` would hide it.
        assert_eq!(
            QuantFidelity::of_round_trip(&[-0.0f32], &[0.0f32]).unwrap(),
            QuantFidelity::Quantized
        );
    }

    #[test]
    fn the_fidelity_mapping_is_total_and_cannot_be_asserted_by_a_caller() {
        // Exactly two labels exist, both are spelled here, and the only way to
        // obtain one is to hand over both value sets.
        assert_eq!(QuantFidelity::Exact.as_str(), "exact");
        assert_eq!(QuantFidelity::Quantized.as_str(), "quantized");
        assert!(!QuantFidelity::Exact.is_lossy());
        assert!(QuantFidelity::Quantized.is_lossy());
        // §8's `aggregate` and `sampled` are statistics labels and are not
        // spellable here; this crate produces values, not statistics.
        for label in ["aggregate", "sampled", "metadata"] {
            assert!(
                serde_json::from_str::<QuantFidelity>(&format!("\"{label}\"")).is_err(),
                "{label} is not a fidelity this crate may claim"
            );
        }
    }

    #[test]
    fn a_fidelity_label_over_no_data_or_mismatched_data_is_refused() {
        // A result that does not describe real input data is refused, not emitted.
        assert_eq!(
            QuantFidelity::of_round_trip(&[], &[]).unwrap_err().kind(),
            "empty_unit"
        );
        assert_eq!(
            QuantFidelity::of_round_trip(&[1.0], &[1.0, 2.0])
                .unwrap_err()
                .kind(),
            "length_mismatch"
        );
    }

    // -- provenance --------------------------------------------------------

    #[test]
    fn every_value_this_crate_produces_is_labelled_simulated_and_measured_is_unspellable() {
        assert_eq!(PROVENANCE, Provenance::Simulated);
        assert_eq!(PROVENANCE.as_str(), "simulated");
        // One variant, so no code path anywhere can label a q-quant result as a
        // measurement of a real quantised checkpoint. v1 never reads one
        // (`.plan/DIAGNOSTIC_ARCHITECTURE.md` §1; ingestion is QUANT-010).
        for claim in ["measured", "ingested", "observed"] {
            assert!(
                serde_json::from_str::<Provenance>(&format!("\"{claim}\"")).is_err(),
                "{claim} must not be a spellable provenance"
            );
        }
        assert_eq!(
            serde_json::to_string(&PROVENANCE).unwrap(),
            "\"simulated\"",
            "the wire form must say so too"
        );
        assert!(SIMULATION_CAVEAT.contains("not a measurement of a real quantised checkpoint"));
        assert!(SIMULATION_CAVEAT.contains("Accuracy impact is not measured"));
    }

    // -- the error type's contract with q-source ---------------------------

    #[test]
    fn the_not_implemented_message_matches_q_sources_error_byte_for_byte() {
        // q_source::error::QError::NotImplemented renders
        // "not implemented [{requirement}]: {detail}". Matching it is what lets
        // a downstream `From<QuantError> for QError` change no asserted string,
        // without this crate depending on q-source (TASK.md §Risks).
        let err = QuantError::not_implemented("QUANT-011", "detail here");
        assert_eq!(err.to_string(), "not implemented [QUANT-011]: detail here");
        assert_eq!(err.requirement_id(), Some("QUANT-011"));
        assert_eq!(
            QuantError::config_rejected("nope").requirement_id(),
            None,
            "only NotImplemented carries a requirement ID"
        );
    }

    #[test]
    fn every_error_kind_is_a_distinct_stable_string() {
        let kinds = [
            QuantError::not_implemented("X", "d").kind(),
            QuantError::config_rejected("d").kind(),
            QuantError::NonFinite {
                unit: "u".into(),
                index: 0,
                value: f32::NAN,
            }
            .kind(),
            QuantError::ScaleNotNormal {
                unit: "u".into(),
                scale: 0.0,
            }
            .kind(),
            QuantError::ReconstructionNotFinite {
                unit: "u".into(),
                index: 0,
                code: 127,
                scale: 1.0,
            }
            .kind(),
            QuantError::ZeroPointOutOfRange {
                unit: "u".into(),
                detail: "d".into(),
            }
            .kind(),
            QuantError::EmptyUnit { unit: "u".into() }.kind(),
            QuantError::LengthMismatch {
                operation: "op",
                left: 1,
                right: 2,
            }
            .kind(),
        ];
        let mut sorted = kinds.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            kinds.len(),
            "two variants share a kind string; the reference's vocabulary would be ambiguous"
        );
        // These exact strings are the reference's `kind` field. Changing one
        // breaks the differential refusal test, which is the point.
        assert!(sorted.contains(&"non_finite"));
        assert!(sorted.contains(&"scale_not_normal"));
        assert!(sorted.contains(&"reconstruction_not_finite"));
        assert!(sorted.contains(&"zero_point_out_of_range"));
        assert!(sorted.contains(&"config_rejected"));
        // `scale_not_normal` and `reconstruction_not_finite` are deliberately
        // separate: the first is a bad scale, the second a normal scale whose
        // product with a code overflows. Collapsing them would hide which.
        assert_ne!(
            QuantError::ScaleNotNormal {
                unit: "u".into(),
                scale: 0.0
            }
            .kind(),
            QuantError::ReconstructionNotFinite {
                unit: "u".into(),
                index: 0,
                code: 1,
                scale: 1.0
            }
            .kind()
        );
    }

    // -- config surface ----------------------------------------------------

    #[test]
    fn the_config_round_trips_through_serde_without_inventing_a_default() {
        let config = QuantConfig::per_group(Precision::Int4, ZeroPoint::Asymmetric, 128);
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(
            serde_json::from_str::<QuantConfig>(&json).unwrap(),
            config,
            "serialised {json}"
        );
        assert!(json.contains("\"int4\"") && json.contains("\"asymmetric\""));
        // A config missing a field is rejected rather than defaulted: there is no
        // safe default precision.
        assert!(serde_json::from_str::<QuantConfig>("{\"precision\":\"int8\"}").is_err());
    }

    #[test]
    fn level_counts_are_explicit_and_the_precision_enum_is_closed() {
        assert_eq!(Precision::Int8.bits(), 8);
        assert_eq!(Precision::Int8.levels(), 256);
        assert_eq!(Precision::Int4.bits(), 4);
        assert_eq!(Precision::Int4.levels(), 16);
        assert_eq!(Precision::Int8.as_str(), "int8");
        assert_eq!(Precision::Int4.as_str(), "int4");
        assert_eq!(ZeroPoint::Symmetric.as_str(), "symmetric");
        assert_eq!(ZeroPoint::Asymmetric.as_str(), "asymmetric");
        assert_eq!(ALGORITHM_VERSION, 1);
        assert_eq!(QuantParams::IDENTITY.scale, 1.0);
        assert_eq!(QuantParams::IDENTITY.zero, 0);
        assert_eq!(QuantParams::new(0.5, -3).zero, -3);
    }
}
