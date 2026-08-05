//! Data plane: **Report Plane** (`.plan/REPORT_ARCHITECTURE.md` §2).
//!
//! `manifest.v1` — the serde mirror of
//! [`schemas/diagnostics/manifest.v1.json`](../../../schemas/diagnostics/manifest.v1.json).
//! The schema is the contract; these types must agree with it, and
//! `crates/q-report/tests/schema_conformance.rs` fails if they drift.
//!
//! ## Unknown members: preserved or refused, never dropped
//!
//! Two different rules, deliberately:
//!
//! * At the **top level**, an unrecognised member lands in
//!   [`Manifest::extensions`] and is written back out unchanged. A newer
//!   producer's addition therefore survives a read-modify-write by this build
//!   rather than being silently deleted.
//! * **Inside** `run`, `model`, `config` and every array element, an
//!   unrecognised member is refused, naming it. Those objects are closed in the
//!   schema (`additionalProperties: false`), so quietly ignoring a member there
//!   would hide a producer bug.
//!
//! Neither path drops data. Note that a document carrying a top-level extension
//! is *preserved* by this parser and *reported* by the schema validator — the
//! extension survives the round trip, and validation still tells you it is not
//! v1.
//!
//! ## Floating point
//!
//! `serde_json` formats `f64` with the shortest decimal that round-trips
//! (Ryū), which is deterministic and platform-independent: enough digits to
//! recover the exact `f64`, and the same digits every time. NaN and ±Infinity
//! have no JSON representation, so they are refused *before* serialization —
//! `serde_json` would otherwise write `null`, and a `null` where a measured
//! number belongs is a lie.

use std::collections::{BTreeMap, BTreeSet};

use q_source::error::{QError, Result};
use q_source::{DType, ResultFidelity, TensorRole};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The manifest version this build writes and reads.
///
/// A document declaring anything else is refused rather than partially parsed —
/// the same rule `CAT-002` applies to the catalog schema.
pub const MANIFEST_VERSION: u32 = 1;

/// `$id` of the schema these types mirror. Carries an explicit version
/// (`SCHEMA-001`).
pub const MANIFEST_SCHEMA_ID: &str = "https://quatricmorph.dev/schemas/diagnostics/manifest/v1";

/// Repository-relative path of the schema, for tooling and tests.
pub const MANIFEST_SCHEMA_PATH: &str = "schemas/diagnostics/manifest.v1.json";

/// Highest tensor rank the visualization stack implements (ADR-010).
///
/// A tensor above this rank is recorded in [`Manifest::refusals`] under
/// `GRID-007`; it is never reshaped into a lower rank, because a `[32,128,128]`
/// tensor presented as `[32,16384]` invites the reader to see adjacency that
/// does not exist.
pub const MAX_IMPLEMENTED_RANK: usize = 3;

/// The wording `DIAGNOSTIC_ARCHITECTURE.md` §8 requires alongside any frontier.
///
/// The frontier is greedy over a density ratio — the standard fractional-knapsack
/// heuristic — and is **not** claimed to be optimal for the integer problem.
/// Carrying the sentence in the manifest is what gets it into every consumer.
pub const FRONTIER_CLAIM: &str = "Greedy over error-per-byte; not proven optimal.";

fn refuse(detail: impl Into<String>) -> QError {
    QError::malformed("diagnostics manifest", detail)
}

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// Which projection of a run this document is.
///
/// The discriminator is required rather than inferred from the presence of
/// `tensors`, because "the per-tensor array was not included" and "no tensors
/// were examined" are different facts and a consumer that cannot tell them
/// apart will report one as the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Projection {
    /// Every examined tensor is listed in `tensors`.
    Full,
    /// Totals, layers, experts, ranking and frontier only; `tensors` is absent.
    Summary,
}

/// Which compute backend actually ran.
///
/// `cpu` and `metal` are the two v1 backends. `q_cuda::CudaBackend` is
/// `Hardware-Unverified` (`CUDA-001`) and is deliberately *not* expressible
/// here: claiming a GPU computed something the CPU computed is on the
/// forbidden-claims list (`PRODUCT_SCOPE.md` §5.2), and a vocabulary that can
/// name a backend which has never run is how that claim gets made by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Cpu,
    Metal,
}

/// Whether an architecture plugin recognised the hierarchy.
///
/// `unknown` means the generic resolver produced it. `NSIR-001` forbids
/// presenting a guessed hierarchy as a known one, so this is surfaced rather
/// than inferred from the architecture name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolverConfidence {
    Resolved,
    Unknown,
}

/// How the numbers in this manifest were obtained.
///
/// The same three-way vocabulary `q_source::ResultFidelity` already types end to
/// end (`SRC-018`, `STAT-005`) and `AGENTS.md` rule 4 requires. It is spelled
/// again here because the wire form is snake_case while
/// `ResultFidelity`'s derived `Serialize` is PascalCase; [`From`] keeps the two
/// from drifting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fidelity {
    Exact,
    Sampled,
    Approximate,
}

impl Fidelity {
    /// Every label, for exhaustiveness checks against the schema's enum.
    pub const ALL: &'static [Fidelity] =
        &[Fidelity::Exact, Fidelity::Sampled, Fidelity::Approximate];

    pub fn as_str(self) -> &'static str {
        match self {
            Fidelity::Exact => "exact",
            Fidelity::Sampled => "sampled",
            Fidelity::Approximate => "approximate",
        }
    }
}

impl From<ResultFidelity> for Fidelity {
    /// Exhaustive by construction: a new `ResultFidelity` variant fails to
    /// compile here rather than silently missing from the manifest vocabulary.
    fn from(value: ResultFidelity) -> Self {
        match value {
            ResultFidelity::Exact => Fidelity::Exact,
            ResultFidelity::Sampled => Fidelity::Sampled,
            ResultFidelity::Approximate => Fidelity::Approximate,
        }
    }
}

/// Target precision of the simulated quantisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    Int8,
    Int4,
}

/// Zero-point convention (`DIAGNOSTIC_ARCHITECTURE.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZeroPoint {
    Symmetric,
    Asymmetric,
}

/// Rounding mode. `nearest_even` is the only v1 variant, stated explicitly
/// because half-away-from-zero disagrees with NumPy on exactly the boundary
/// values a golden test contains (`DIAGNOSTIC_ARCHITECTURE.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundMode {
    NearestEven,
}

/// Quantisation granularity, against NSIR's canonical axis semantics rather
/// than raw tensor order (`DIAGNOSTIC_ARCHITECTURE.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GranularityKind {
    PerTensor,
    PerOutputChannel,
    PerGroup,
}

/// The frontier search. One variant, so the claim below it cannot be attached
/// to a method that did not produce it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontierMethod {
    GreedyErrorPerByte,
}

// ---------------------------------------------------------------------------
// serde adapters for the two vocabularies q-source already owns
// ---------------------------------------------------------------------------

/// `TensorRole` on the wire, using `q_source`'s own snake_case spellings.
///
/// [`TensorRole::parse`] maps *any* unrecognised string to `Unknown`, which is
/// the right answer for a checkpoint tensor name nobody has taught the resolver
/// and the wrong answer for a manifest field: a misspelled role would become
/// `unknown` and read as an honest "we do not know". So an unrecognised spelling
/// is refused here, while the literal `"unknown"` is accepted as the
/// first-class value `ARCHITECTURE.md` §4.2 requires it to be.
mod role_serde {
    use super::*;

    pub fn serialize<S: Serializer>(
        role: &TensorRole,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(role.as_str())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<TensorRole, D::Error> {
        let raw = String::deserialize(d)?;
        let parsed = TensorRole::parse(&raw);
        if parsed == TensorRole::Unknown && raw != TensorRole::Unknown.as_str() {
            return Err(serde::de::Error::custom(format!(
                "unknown semantic role {raw:?}; a role outside the NSIR vocabulary is refused, \
                 not coerced to `unknown`"
            )));
        }
        Ok(parsed)
    }
}

/// `DType` on the wire, using the SafeTensors header spellings
/// [`DType::as_safetensors_str`] already defines. An unrecognised tag is refused
/// rather than guessed (`SRC-014`).
mod dtype_serde {
    use super::*;

    pub fn serialize<S: Serializer>(dtype: &DType, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(dtype.as_safetensors_str())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<DType, D::Error> {
        let raw = String::deserialize(d)?;
        DType::parse_safetensors(&raw).map_err(|_| {
            serde::de::Error::custom(format!(
                "unknown dtype {raw:?}; a dtype outside the SafeTensors vocabulary is refused, \
                 not guessed"
            ))
        })
    }
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// Run metadata. `started_at` and `elapsed_seconds` are wall-clock facts and are
/// excluded from the determinism comparison (`REPORT_ARCHITECTURE.md` §3.2);
/// everything else is a property of the inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    /// Deterministic: derived from (model revision hash, config, engine
    /// version). Two runs over the same inputs share it.
    pub run_id: String,
    pub engine_version: String,
    pub backend: Backend,
    /// RFC 3339 timestamp, e.g. `2026-08-04T00:00:00Z`.
    pub started_at: String,
    pub elapsed_seconds: f64,
    /// Measured peak resident set size, in bytes. The product's central claim
    /// lives here, in the artifact, so a reader never has to take it on trust.
    pub peak_resident_bytes: u64,
    pub bytes_read: u64,
}

/// Which checkpoint was diagnosed. A diagnosis of an unidentified checkpoint is
/// not evidence, so the identity fields may not be blank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub model_id: String,
    pub source_uri: String,
    /// Revision of the checkpoint, as the source names it.
    pub revision_hash: String,
    pub checkpoint_bytes: u64,
    pub parameter_count: u64,
    /// The resolver that produced the hierarchy — one of the plugins under
    /// `architectures/`. Not an enum: a new plugin must not require a schema
    /// version bump.
    pub architecture: String,
    pub resolver_confidence: ResolverConfidence,
}

/// Granularity plus its parameter. `group_size` is present exactly when
/// `kind` is `per_group`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Granularity {
    pub kind: GranularityKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_size: Option<u32>,
}

/// The quantisation configuration that was simulated.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantConfigRecord {
    pub precision: Precision,
    pub granularity: Granularity,
    pub zero_point: ZeroPoint,
    pub round: RoundMode,
    pub block_rows: u32,
    pub block_columns: u32,
    pub resident_ceiling_bytes: u64,
}

/// The composable partials for one level of the aggregation hierarchy.
///
/// **Partials only.** Sums of squares compose across blocks; RMSE and relative
/// error do not (`DIAGNOSTIC_ARCHITECTURE.md` §4.1). A consumer derives
/// `rmse = sqrt(sum_sq_delta / count)` and
/// `relative_error = sqrt(sum_sq_delta / sum_sq_base)` at read time. Storing the
/// finished metric here as well would give the same quantity two definitions,
/// and two definitions drift.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorAggregate {
    /// Number of weights reduced at this level.
    pub count: u64,
    /// `Σ w²` — the denominator of relative error.
    pub sum_sq_base: f64,
    /// `Σ (w − ŵ)²` — the numerator, `‖·‖_F²` before the root.
    pub sum_sq_delta: f64,
    /// `Σ |w − ŵ|`.
    pub sum_abs_delta: f64,
    /// `max |w − ŵ|`. A max reduction has no rounding excuse, so it is exact on
    /// every backend.
    pub max_abs_delta: f64,
    pub bytes_at_base_precision: u64,
    pub bytes_at_target_precision: u64,
}

/// One layer of the repeated stack.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerEntry {
    pub layer_index: u32,
    pub aggregate: ErrorAggregate,
}

/// One MoE expert, present only where the resolver found experts (`NSIR-003`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpertEntry {
    pub layer_index: u32,
    pub expert_index: u32,
    pub aggregate: ErrorAggregate,
}

/// Share of squared error carried by the largest weights by magnitude
/// (`DIAGNOSTIC_ARCHITECTURE.md` §7.1). Entirely weight-space: a layer whose
/// error concentrates in 0.1 % of its weights is a candidate for an
/// outlier-preserving scheme, which is actionable; nothing here is a statement
/// about what the layer means.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlierAttribution {
    pub top_0_1_percent_share: f64,
    pub top_1_percent_share: f64,
}

/// One examined tensor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TensorEntry {
    /// NSIR canonical address, e.g.
    /// `model.layers[10].self_attention.query_projection.weight`. Unique across
    /// the array (`SRC-006`).
    pub address: String,
    /// `unknown` is a legitimate value. Nothing infers a role from shape.
    #[serde(with = "role_serde")]
    pub role: TensorRole,
    #[serde(with = "dtype_serde")]
    pub dtype: DType,
    /// Shape in axis order, rank at most [`MAX_IMPLEMENTED_RANK`] (ADR-010).
    pub shape: Vec<u64>,
    pub aggregate: ErrorAggregate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outlier_attribution: Option<OutlierAttribution>,
}

/// One row of the fragility ranking.
///
/// `relative_error` appears here — and only here — because it *is* the ordering
/// key. Everywhere else the manifest carries partials and the consumer finishes
/// the arithmetic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankingEntry {
    pub address: String,
    /// `sqrt(sum_sq_delta / sum_sq_base)` for this tensor. A proxy for
    /// sensitivity, ranked; not a statement that a layer is important.
    pub relative_error: f64,
    pub parameter_count: u64,
}

/// One greedy step of the mixed-precision frontier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontierStep {
    /// Canonical addresses kept at base precision at this step, ascending.
    pub keep_set: Vec<String>,
    /// Cumulative bytes the keep set costs, relative to quantising everything.
    pub added_bytes: u64,
    /// Cumulative fraction of total squared error removed. Weight-space only:
    /// not an accuracy prediction.
    pub error_removed_fraction: f64,
}

/// The mixed-precision frontier, with the method that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frontier {
    pub method: FrontierMethod,
    /// Must equal [`FRONTIER_CLAIM`]. Required so that no consumer can render a
    /// frontier without the sentence that says it is greedy.
    pub claim: String,
    pub steps: Vec<FrontierStep>,
}

/// A capability this run could not provide.
///
/// A first-class array, because a consumer must be able to tell "zero" from
/// "not computed" — the failure mode that destroys trust in a diagnostic tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    /// The `STATUS.md` requirement ID, e.g. `EVAL-001`.
    pub requirement_id: String,
    /// What was not provided.
    pub what: String,
    /// Why not, in the wording the governing document requires.
    pub why: String,
}

/// The diagnostics manifest.
///
/// Field order is the serialization order, and the serialization order is part
/// of the artifact: a Git-diffable manifest must not reflow when a number
/// changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub manifest_version: u32,
    pub projection: Projection,
    pub run: Run,
    pub model: Model,
    pub config: QuantConfigRecord,
    /// Whole-model partials.
    pub totals: ErrorAggregate,
    /// Ordered by `layer_index`.
    pub layers: Vec<LayerEntry>,
    /// Ordered by `(layer_index, expert_index)`. Empty where the resolver found
    /// no experts.
    pub experts: Vec<ExpertEntry>,
    /// Ordered by canonical address. Present exactly when `projection` is
    /// `full`; an `O(tensors)` array is not something to push into a browser
    /// wholesale (`ARCHITECTURE.md` §19).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tensors: Option<Vec<TensorEntry>>,
    /// Ordered by `(relative_error desc, parameter_count desc, address asc)`.
    pub ranking: Vec<RankingEntry>,
    pub frontier: Frontier,
    pub fidelity: Fidelity,
    /// Ordered by `(requirement_id, what, why)`. Required — never omitted,
    /// because an absent `refusals` reads as "nothing was refused".
    pub refusals: Vec<Refusal>,
    /// Members this build does not recognise, preserved verbatim so a
    /// read-modify-write never deletes a newer producer's data.
    #[serde(flatten)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl Manifest {
    /// Refuse a version this build does not write, naming both versions.
    ///
    /// The `CAT-002` rule, in JSON: a reader that guesses at an unknown version
    /// produces a plausible wrong answer.
    fn check_version(found: u32) -> Result<()> {
        if found > MANIFEST_VERSION {
            return Err(refuse(format!(
                "manifest_version {found} is newer than this build supports \
                 ({MANIFEST_VERSION}); upgrade Quatricmorph rather than reading a future manifest"
            )));
        }
        if found != MANIFEST_VERSION {
            return Err(refuse(format!(
                "manifest_version {found} is not a version this build knows \
                 ({MANIFEST_VERSION} is the only one); there is no v{found} to migrate from"
            )));
        }
        Ok(())
    }

    /// Every invariant the schema cannot express, plus the ones it can.
    ///
    /// Checked on both read and write: a document that does not describe a real
    /// run is refused rather than handed on.
    pub fn validate(&self) -> Result<()> {
        Self::check_version(self.manifest_version)?;

        require_non_empty("run.run_id", &self.run.run_id)?;
        require_non_empty("run.engine_version", &self.run.engine_version)?;
        if !is_rfc3339_date_time(&self.run.started_at) {
            return Err(refuse(format!(
                "run.started_at {:?} is not an RFC 3339 date-time such as `2026-08-04T00:00:00Z`",
                self.run.started_at
            )));
        }
        require_finite("run.elapsed_seconds", self.run.elapsed_seconds)?;
        require_non_negative("run.elapsed_seconds", self.run.elapsed_seconds)?;
        if self.run.peak_resident_bytes == 0 {
            return Err(refuse(
                "run.peak_resident_bytes is 0; bounded residency is the product's central claim \
                 and a performance number does not exist until it is measured — a run that could \
                 not measure it records a refusal rather than reporting nothing as zero",
            ));
        }

        require_non_empty("model.model_id", &self.model.model_id)?;
        require_non_empty("model.source_uri", &self.model.source_uri)?;
        require_non_empty("model.revision_hash", &self.model.revision_hash)?;
        require_non_empty("model.architecture", &self.model.architecture)?;

        if self.config.block_rows == 0 {
            return Err(refuse(
                "config.block_rows is 0; a block has at least one row",
            ));
        }
        if self.config.block_columns == 0 {
            return Err(refuse(
                "config.block_columns is 0; a block has at least one column",
            ));
        }
        if self.config.resident_ceiling_bytes == 0 {
            return Err(refuse(
                "config.resident_ceiling_bytes is 0; a ceiling of zero admits no run at all",
            ));
        }
        match self.config.granularity.kind {
            GranularityKind::PerGroup => match self.config.granularity.group_size {
                None => {
                    return Err(refuse(
                        "config.granularity.group_size is required when kind is `per_group`",
                    ))
                }
                Some(0) => {
                    return Err(refuse(
                        "config.granularity.group_size is 0; a group holds at least one value",
                    ))
                }
                Some(_) => {}
            },
            GranularityKind::PerTensor | GranularityKind::PerOutputChannel => {
                if self.config.granularity.group_size.is_some() {
                    return Err(refuse(
                        "config.granularity.group_size is set on a granularity that has no \
                         groups; a parameter nobody reads is a parameter that will be believed",
                    ));
                }
            }
        }

        validate_aggregate("totals", &self.totals)?;

        let mut layer_indices = BTreeSet::new();
        for layer in &self.layers {
            validate_aggregate(
                &format!("layers[{}].aggregate", layer.layer_index),
                &layer.aggregate,
            )?;
            if !layer_indices.insert(layer.layer_index) {
                return Err(refuse(format!(
                    "duplicate layer_index {} in layers; the order is fixed by content and a \
                     repeated key leaves it undefined",
                    layer.layer_index
                )));
            }
        }

        let mut expert_keys = BTreeSet::new();
        for expert in &self.experts {
            validate_aggregate(
                &format!(
                    "experts[{},{}].aggregate",
                    expert.layer_index, expert.expert_index
                ),
                &expert.aggregate,
            )?;
            if !expert_keys.insert((expert.layer_index, expert.expert_index)) {
                return Err(refuse(format!(
                    "duplicate expert ({}, {}) in experts",
                    expert.layer_index, expert.expert_index
                )));
            }
        }

        let examined: Option<BTreeSet<&str>> = match (self.projection, &self.tensors) {
            (Projection::Full, None) => {
                return Err(refuse(
                    "projection is `full` but `tensors` is absent; a full projection lists every \
                     examined tensor, and an absent array would be indistinguishable from a summary",
                ))
            }
            (Projection::Summary, Some(_)) => {
                return Err(refuse(
                    "projection is `summary` but `tensors` is present; the summary exists so that \
                     an O(tensors) array is never pushed to a consumer that asked for totals",
                ))
            }
            (Projection::Summary, None) => None,
            (Projection::Full, Some(tensors)) => {
                let mut addresses = BTreeSet::new();
                for entry in tensors {
                    require_non_empty("tensors[].address", &entry.address)?;
                    if entry.shape.len() > MAX_IMPLEMENTED_RANK {
                        return Err(QError::not_implemented(
                            "GRID-007",
                            format!(
                                "tensor {} has rank {}; ADR-010 implements rank <= \
                                 {MAX_IMPLEMENTED_RANK} and refuses above it rather than \
                                 flattening, because a flattened shape invites the reader to see \
                                 adjacency between values that are not adjacent. Record it in \
                                 `refusals` instead",
                                entry.address,
                                entry.shape.len()
                            ),
                        ));
                    }
                    validate_aggregate(
                        &format!("tensors[{}].aggregate", entry.address),
                        &entry.aggregate,
                    )?;
                    if let Some(outliers) = &entry.outlier_attribution {
                        require_fraction(
                            &format!(
                                "tensors[{}].outlier_attribution.top_0_1_percent_share",
                                entry.address
                            ),
                            outliers.top_0_1_percent_share,
                        )?;
                        require_fraction(
                            &format!(
                                "tensors[{}].outlier_attribution.top_1_percent_share",
                                entry.address
                            ),
                            outliers.top_1_percent_share,
                        )?;
                    }
                    if !addresses.insert(entry.address.as_str()) {
                        return Err(refuse(format!(
                            "duplicate canonical address {} in tensors; addresses are unique \
                             (SRC-006)",
                            entry.address
                        )));
                    }
                }
                Some(addresses)
            }
        };

        let mut ranked = BTreeSet::new();
        for entry in &self.ranking {
            require_non_empty("ranking[].address", &entry.address)?;
            let field = format!("ranking[{}].relative_error", entry.address);
            require_finite(&field, entry.relative_error)?;
            require_non_negative(&field, entry.relative_error)?;
            if !ranked.insert(entry.address.as_str()) {
                return Err(refuse(format!(
                    "duplicate canonical address {} in ranking",
                    entry.address
                )));
            }
            if let Some(examined) = &examined {
                if !examined.contains(entry.address.as_str()) {
                    return Err(refuse(format!(
                        "ranking names {}, which is not in `tensors`; a manifest may not rank a \
                         tensor it did not examine",
                        entry.address
                    )));
                }
            }
        }

        if self.frontier.claim != FRONTIER_CLAIM {
            return Err(refuse(format!(
                "frontier.claim must be {FRONTIER_CLAIM:?}; the search is greedy over \
                 error-per-byte and is not proven optimal, and the sentence travels with the \
                 numbers so that no consumer can render one without the other"
            )));
        }
        let mut steps = BTreeSet::new();
        for step in &self.frontier.steps {
            require_fraction(
                &format!(
                    "frontier.steps[added_bytes={}].error_removed_fraction",
                    step.added_bytes
                ),
                step.error_removed_fraction,
            )?;
            if step.keep_set.is_empty() {
                return Err(refuse(
                    "a frontier step has an empty keep_set; a step that keeps nothing is not a step",
                ));
            }
            let mut kept = BTreeSet::new();
            for address in &step.keep_set {
                require_non_empty("frontier.steps[].keep_set[]", address)?;
                if !kept.insert(address.as_str()) {
                    return Err(refuse(format!(
                        "duplicate address {address} in a frontier keep_set"
                    )));
                }
                if let Some(examined) = &examined {
                    if !examined.contains(address.as_str()) {
                        return Err(refuse(format!(
                            "the frontier keeps {address}, which is not in `tensors`; a manifest \
                             may not recommend keeping a tensor it did not examine"
                        )));
                    }
                }
            }
            if !steps.insert((
                step.added_bytes,
                step.error_removed_fraction.to_bits(),
                step.keep_set.clone(),
            )) {
                return Err(refuse(format!(
                    "duplicate frontier step at added_bytes {}",
                    step.added_bytes
                )));
            }
        }

        let mut seen = BTreeSet::new();
        for entry in &self.refusals {
            require_non_empty("refusals[].requirement_id", &entry.requirement_id)?;
            require_non_empty("refusals[].what", &entry.what)?;
            require_non_empty("refusals[].why", &entry.why)?;
            if !seen.insert((&entry.requirement_id, &entry.what, &entry.why)) {
                return Err(refuse(format!(
                    "duplicate entry {} in refusals; the order is fixed by content and a repeated \
                     key leaves it undefined",
                    entry.requirement_id
                )));
            }
        }

        Ok(())
    }

    /// A validated clone with every array in its content-defined total order.
    ///
    /// The order is *imposed* here rather than demanded of the caller, so that
    /// two runs over the same data agree byte for byte no matter what order the
    /// engine happened to visit tensors in. Each key below is total:
    /// `validate` has already refused the duplicates that would leave it
    /// otherwise.
    pub fn canonical(&self) -> Result<Manifest> {
        self.validate()?;
        let mut canonical = self.clone();

        canonical.layers.sort_by_key(|layer| layer.layer_index);
        canonical
            .experts
            .sort_by_key(|expert| (expert.layer_index, expert.expert_index));
        if let Some(tensors) = canonical.tensors.as_mut() {
            tensors.sort_by(|a, b| a.address.cmp(&b.address));
        }
        canonical.ranking.sort_by(|a, b| {
            b.relative_error
                .total_cmp(&a.relative_error)
                .then(b.parameter_count.cmp(&a.parameter_count))
                .then(a.address.cmp(&b.address))
        });
        for step in &mut canonical.frontier.steps {
            step.keep_set.sort();
        }
        // Primary key is cumulative `added_bytes`, as the ordering table
        // requires; the remainder exists only to make the order total.
        canonical.frontier.steps.sort_by(|a, b| {
            a.added_bytes
                .cmp(&b.added_bytes)
                .then(
                    a.error_removed_fraction
                        .total_cmp(&b.error_removed_fraction),
                )
                .then(a.keep_set.cmp(&b.keep_set))
        });
        canonical.refusals.sort_by(|a, b| {
            (&a.requirement_id, &a.what, &a.why).cmp(&(&b.requirement_id, &b.what, &b.why))
        });

        Ok(canonical)
    }

    /// The canonical JSON document, ending in a newline.
    pub fn to_json_string(&self) -> Result<String> {
        let canonical = self.canonical()?;
        let mut json = serde_json::to_string_pretty(&canonical)
            .map_err(|e| QError::json("diagnostics manifest", e))?;
        json.push('\n');
        Ok(json)
    }

    /// Parse a manifest, refusing a version or a shape this build cannot read.
    ///
    /// The version is read first, on its own, so that a future document is
    /// refused *as a future document* rather than producing a confusing
    /// complaint about a field v2 renamed.
    pub fn from_json_str(json: &str) -> Result<Manifest> {
        #[derive(Deserialize)]
        struct VersionProbe {
            manifest_version: u32,
        }

        let probe: VersionProbe =
            serde_json::from_str(json).map_err(|e| QError::json("diagnostics manifest", e))?;
        Self::check_version(probe.manifest_version)?;

        let manifest: Manifest =
            serde_json::from_str(json).map_err(|e| QError::json("diagnostics manifest", e))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// The `--summary` projection: everything except the per-tensor array.
    ///
    /// Totals, layers, experts, ranking and frontier — enough to make the
    /// decision the product exists to support, without the `O(tensors)` array
    /// that `ARCHITECTURE.md` §19 forbids pushing into a browser wholesale.
    pub fn summary(&self) -> Result<Manifest> {
        let mut summary = self.canonical()?;
        summary.projection = Projection::Summary;
        summary.tensors = None;
        summary.validate()?;
        Ok(summary)
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(refuse(format!(
            "{field} is empty; a manifest must describe a real run, and a blank {field} describes \
             nothing"
        )));
    }
    Ok(())
}

fn require_finite(field: &str, value: f64) -> Result<()> {
    if !value.is_finite() {
        return Err(refuse(format!(
            "{field} is {value}, which JSON cannot represent; the manifest refuses rather than \
             writing the `null` serde_json would otherwise emit, because a `null` where a measured \
             number belongs reads as an absence rather than a failure"
        )));
    }
    Ok(())
}

fn require_non_negative(field: &str, value: f64) -> Result<()> {
    require_finite(field, value)?;
    if value < 0.0 {
        return Err(refuse(format!(
            "{field} is {value}, which cannot be negative"
        )));
    }
    Ok(())
}

fn require_fraction(field: &str, value: f64) -> Result<()> {
    require_non_negative(field, value)?;
    if value > 1.0 {
        return Err(refuse(format!(
            "{field} is {value}, which is not a fraction of a whole"
        )));
    }
    Ok(())
}

fn validate_aggregate(path: &str, aggregate: &ErrorAggregate) -> Result<()> {
    require_non_negative(&format!("{path}.sum_sq_base"), aggregate.sum_sq_base)?;
    require_non_negative(&format!("{path}.sum_sq_delta"), aggregate.sum_sq_delta)?;
    require_non_negative(&format!("{path}.sum_abs_delta"), aggregate.sum_abs_delta)?;
    require_non_negative(&format!("{path}.max_abs_delta"), aggregate.max_abs_delta)?;
    Ok(())
}

/// RFC 3339 `date-time` shape: `YYYY-MM-DDTHH:MM:SS`, an optional fractional
/// second, then `Z` or `±HH:MM`.
///
/// Shape only — it does not check that the date exists. Upper-case `T` and `Z`
/// are required even though RFC 3339 permits lower case, because a manifest is
/// compared byte-for-byte and two spellings of one instant would defeat that.
pub(crate) fn is_rfc3339_date_time(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 20 {
        return false;
    }
    let digits = |mut range: std::ops::Range<usize>| range.all(|i| b[i].is_ascii_digit());
    if !(digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10)) {
        return false;
    }
    if b[10] != b'T' {
        return false;
    }
    if !(digits(11..13) && b[13] == b':' && digits(14..16) && b[16] == b':' && digits(17..19)) {
        return false;
    }
    let mut i = 19;
    if b[i] == b'.' {
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    match b.get(i) {
        Some(b'Z') => i + 1 == b.len(),
        Some(b'+') | Some(b'-') => {
            i + 6 == b.len()
                && b[i + 1].is_ascii_digit()
                && b[i + 2].is_ascii_digit()
                && b[i + 3] == b':'
                && b[i + 4].is_ascii_digit()
                && b[i + 5].is_ascii_digit()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small but complete manifest that satisfies every invariant. Tests
    /// mutate one field of it and assert the refusal, so the thing under test
    /// is always exactly the mutation.
    fn a_valid_manifest() -> Manifest {
        Manifest {
            manifest_version: MANIFEST_VERSION,
            projection: Projection::Full,
            run: Run {
                run_id: "3a7f1c9d2b8e4056".into(),
                engine_version: "0.1.0".into(),
                backend: Backend::Cpu,
                started_at: "2026-08-04T00:00:00Z".into(),
                elapsed_seconds: 12.5,
                peak_resident_bytes: 2_883_584,
                bytes_read: 8192,
            },
            model: Model {
                model_id: "fixture-model".into(),
                source_uri: "file:///models/fixture".into(),
                revision_hash: "1f0a9d3c7b2e6540".into(),
                checkpoint_bytes: 8192,
                parameter_count: 2048,
                architecture: "generic".into(),
                resolver_confidence: ResolverConfidence::Unknown,
            },
            config: QuantConfigRecord {
                precision: Precision::Int4,
                granularity: Granularity {
                    kind: GranularityKind::PerGroup,
                    group_size: Some(128),
                },
                zero_point: ZeroPoint::Asymmetric,
                round: RoundMode::NearestEven,
                block_rows: 256,
                block_columns: 256,
                resident_ceiling_bytes: 2_147_483_648,
            },
            totals: an_aggregate(2048, 1024.0, 64.0),
            layers: vec![LayerEntry {
                layer_index: 0,
                aggregate: an_aggregate(2048, 1024.0, 64.0),
            }],
            experts: vec![],
            tensors: Some(vec![TensorEntry {
                address: "model.layers[0].mlp.down_projection.weight".into(),
                role: TensorRole::MlpDownProjection,
                dtype: DType::F32,
                shape: vec![64, 32],
                aggregate: an_aggregate(2048, 1024.0, 64.0),
                outlier_attribution: None,
            }]),
            ranking: vec![RankingEntry {
                address: "model.layers[0].mlp.down_projection.weight".into(),
                relative_error: 0.25,
                parameter_count: 2048,
            }],
            frontier: Frontier {
                method: FrontierMethod::GreedyErrorPerByte,
                claim: FRONTIER_CLAIM.into(),
                steps: vec![FrontierStep {
                    keep_set: vec!["model.layers[0].mlp.down_projection.weight".into()],
                    added_bytes: 6144,
                    error_removed_fraction: 1.0,
                }],
            },
            fidelity: Fidelity::Exact,
            refusals: vec![Refusal {
                requirement_id: "EVAL-001".into(),
                what: "accuracy estimate".into(),
                why: "Weight-space error only. Accuracy impact is not measured.".into(),
            }],
            extensions: BTreeMap::new(),
        }
    }

    fn an_aggregate(count: u64, sum_sq_base: f64, sum_sq_delta: f64) -> ErrorAggregate {
        ErrorAggregate {
            count,
            sum_sq_base,
            sum_sq_delta,
            sum_abs_delta: 128.0,
            max_abs_delta: 0.5,
            bytes_at_base_precision: count * 4,
            bytes_at_target_precision: count / 2,
        }
    }

    fn tensor(address: &str, params: u64) -> TensorEntry {
        TensorEntry {
            address: address.into(),
            role: TensorRole::Unknown,
            dtype: DType::F32,
            shape: vec![params],
            aggregate: an_aggregate(params, 1024.0, 64.0),
            outlier_attribution: None,
        }
    }

    fn ranked(address: &str, relative_error: f64, parameter_count: u64) -> RankingEntry {
        RankingEntry {
            address: address.into(),
            relative_error,
            parameter_count,
        }
    }

    // -- the happy path -----------------------------------------------------

    #[test]
    fn a_complete_manifest_validates() {
        a_valid_manifest().validate().unwrap();
    }

    #[test]
    fn a_serialised_manifest_parses_back_to_an_equal_value() {
        let m = a_valid_manifest();
        let json = m.to_json_string().unwrap();
        assert_eq!(
            Manifest::from_json_str(&json).unwrap(),
            m.canonical().unwrap()
        );
    }

    #[test]
    fn the_round_trip_of_a_produced_manifest_is_byte_identical() {
        let first = a_valid_manifest().to_json_string().unwrap();
        let second = Manifest::from_json_str(&first)
            .unwrap()
            .to_json_string()
            .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn two_runs_over_the_same_data_produce_identical_bytes() {
        assert_eq!(
            a_valid_manifest().to_json_string().unwrap(),
            a_valid_manifest().to_json_string().unwrap()
        );
    }

    #[test]
    fn the_document_ends_in_a_newline() {
        assert!(a_valid_manifest()
            .to_json_string()
            .unwrap()
            .ends_with("}\n"));
    }

    // -- version refusal ----------------------------------------------------

    #[test]
    fn a_future_manifest_version_is_refused_naming_both_versions() {
        let mut m = a_valid_manifest();
        m.manifest_version = 2;
        let json = serde_json::to_string(&m).unwrap();
        let err = Manifest::from_json_str(&json).unwrap_err().to_string();
        assert!(err.contains('2'), "the found version must be named: {err}");
        assert!(
            err.contains('1'),
            "the supported version must be named: {err}"
        );
        assert!(err.contains("newer than this build supports"), "{err}");
    }

    #[test]
    fn a_manifest_version_below_one_is_refused_rather_than_upgraded() {
        let mut m = a_valid_manifest();
        m.manifest_version = 0;
        let json = serde_json::to_string(&m).unwrap();
        let err = Manifest::from_json_str(&json).unwrap_err().to_string();
        assert!(err.contains("manifest_version 0"), "{err}");
        assert!(
            err.contains('1'),
            "the supported version must be named: {err}"
        );
    }

    #[test]
    fn a_future_version_is_refused_before_the_body_is_parsed() {
        // The body is nonsense for v1. The version refusal must fire anyway,
        // rather than a confusing complaint about a missing field.
        let err = Manifest::from_json_str(r#"{"manifest_version": 7, "anything": []}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("manifest_version 7"), "{err}");
    }

    // -- malformed and missing ----------------------------------------------

    #[test]
    fn malformed_json_is_refused_rather_than_partially_parsed() {
        assert!(Manifest::from_json_str("{\"manifest_version\": 1,").is_err());
        assert!(Manifest::from_json_str("not json at all").is_err());
    }

    #[test]
    fn a_missing_required_field_is_refused_naming_it() {
        let mut value = serde_json::to_value(a_valid_manifest()).unwrap();
        value["run"].as_object_mut().unwrap().remove("backend");
        let err = Manifest::from_json_str(&value.to_string())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("backend"),
            "the missing field must be named: {err}"
        );
    }

    #[test]
    fn a_missing_refusals_array_is_refused_rather_than_defaulted_to_empty() {
        let mut value = serde_json::to_value(a_valid_manifest()).unwrap();
        value.as_object_mut().unwrap().remove("refusals");
        let err = Manifest::from_json_str(&value.to_string())
            .unwrap_err()
            .to_string();
        assert!(err.contains("refusals"), "{err}");
    }

    // -- unknown members: preserved at the top, refused inside ---------------

    #[test]
    fn an_unknown_top_level_field_survives_the_round_trip_rather_than_being_dropped() {
        let mut value = serde_json::to_value(a_valid_manifest()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("zz_future_section".into(), serde_json::json!({"k": 1}));
        let parsed = Manifest::from_json_str(&value.to_string()).unwrap();
        assert_eq!(
            parsed.extensions.get("zz_future_section"),
            Some(&serde_json::json!({"k": 1}))
        );
        assert!(parsed
            .to_json_string()
            .unwrap()
            .contains("zz_future_section"));
    }

    #[test]
    fn an_unknown_field_inside_run_is_refused_rather_than_silently_dropped() {
        let mut value = serde_json::to_value(a_valid_manifest()).unwrap();
        value["run"]
            .as_object_mut()
            .unwrap()
            .insert("gpu_name".into(), serde_json::json!("RTX 3090"));
        let err = Manifest::from_json_str(&value.to_string())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("gpu_name"),
            "the unknown field must be named: {err}"
        );
    }

    // -- non-finite floats ---------------------------------------------------

    #[test]
    fn a_non_finite_relative_error_is_refused_rather_than_serialised_as_null() {
        let mut m = a_valid_manifest();
        m.ranking[0].relative_error = f64::NAN;
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("relative_error"), "{err}");
        assert!(err.contains("NaN"), "{err}");
    }

    #[test]
    fn an_infinite_aggregate_is_refused_rather_than_serialised_as_null() {
        let mut m = a_valid_manifest();
        m.totals.sum_sq_delta = f64::INFINITY;
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("totals.sum_sq_delta"), "{err}");
    }

    #[test]
    fn serde_json_would_have_written_null_which_is_why_the_refusal_exists() {
        // Not a test of our code: a test of the hazard our code guards. If this
        // ever stops being true the refusal may be revisited; while it is true
        // the refusal is the only thing standing between a NaN and a `null`
        // that reads as a measured absence.
        assert_eq!(serde_json::to_string(&f64::NAN).unwrap(), "null");
    }

    // -- identity: a manifest must describe real data ------------------------

    #[test]
    fn a_manifest_without_a_revision_hash_is_refused_rather_than_emitted() {
        let mut m = a_valid_manifest();
        m.model.revision_hash = String::new();
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("model.revision_hash"), "{err}");
    }

    #[test]
    fn a_manifest_without_a_run_id_is_refused_rather_than_emitted() {
        let mut m = a_valid_manifest();
        m.run.run_id = "   ".into();
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("run.run_id"), "{err}");
    }

    #[test]
    fn an_unmeasured_peak_residency_is_refused_rather_than_reported_as_zero() {
        let mut m = a_valid_manifest();
        m.run.peak_resident_bytes = 0;
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("peak_resident_bytes"), "{err}");
    }

    #[test]
    fn a_malformed_started_at_is_refused() {
        let mut m = a_valid_manifest();
        m.run.started_at = "yesterday".into();
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("started_at"), "{err}");
    }

    #[test]
    fn rfc3339_shapes_are_accepted_and_others_are_not() {
        assert!(is_rfc3339_date_time("2026-08-04T00:00:00Z"));
        assert!(is_rfc3339_date_time("2026-08-04T12:34:56.789Z"));
        assert!(is_rfc3339_date_time("2026-08-04T12:34:56+02:00"));
        assert!(!is_rfc3339_date_time("2026-08-04 00:00:00Z"));
        assert!(!is_rfc3339_date_time("2026-08-04T00:00:00"));
        assert!(!is_rfc3339_date_time("2026-08-04t00:00:00z"));
        assert!(!is_rfc3339_date_time("2026-08-04T00:00:00.Z"));
        assert!(!is_rfc3339_date_time(""));
    }

    // -- ADR-010: rank above three refuses, it does not flatten --------------

    #[test]
    fn refuses_rank_four_rather_than_flattening() {
        let mut m = a_valid_manifest();
        m.tensors.as_mut().unwrap()[0].shape = vec![32, 4, 128, 128];
        let err = m.to_json_string().unwrap_err();
        assert_eq!(err.requirement_id(), Some("GRID-007"));
        let text = err.to_string();
        assert!(text.contains("rank 4"), "{text}");
        assert!(
            !text.contains("16384"),
            "no flattened extent may appear anywhere: {text}"
        );
    }

    #[test]
    fn rank_three_is_accepted_because_grouped_experts_produce_it() {
        let mut m = a_valid_manifest();
        m.tensors.as_mut().unwrap()[0].shape = vec![4, 512, 256];
        m.validate().unwrap();
    }

    #[test]
    fn a_rank_four_tensor_belongs_in_refusals_not_in_tensors() {
        let mut m = a_valid_manifest();
        m.refusals.push(Refusal {
            requirement_id: "GRID-007".into(),
            what: "rank-4 tensor model.layers[0].router.gate.weight".into(),
            why: "ADR-010 implements rank <= 3 and refuses above it.".into(),
        });
        m.validate().unwrap();
    }

    // -- vocabularies: unknown refuses, `unknown` is a value -----------------

    #[test]
    fn an_unknown_dtype_is_refused_rather_than_guessed() {
        let mut value = serde_json::to_value(a_valid_manifest()).unwrap();
        value["tensors"][0]["dtype"] = serde_json::json!("F4_SECRET");
        let err = Manifest::from_json_str(&value.to_string())
            .unwrap_err()
            .to_string();
        assert!(err.contains("F4_SECRET"), "{err}");
    }

    #[test]
    fn an_unrecognised_role_is_refused_rather_than_coerced_to_unknown() {
        let mut value = serde_json::to_value(a_valid_manifest()).unwrap();
        value["tensors"][0]["role"] = serde_json::json!("attention_query_projeciton");
        let err = Manifest::from_json_str(&value.to_string())
            .unwrap_err()
            .to_string();
        assert!(err.contains("attention_query_projeciton"), "{err}");
    }

    #[test]
    fn an_unidentified_tensor_keeps_the_unknown_role_rather_than_a_shape_inferred_guess() {
        let mut m = a_valid_manifest();
        m.tensors.as_mut().unwrap()[0].role = TensorRole::Unknown;
        // A 64x32 matrix has the shape of a projection. Nothing may upgrade it.
        let json = m.to_json_string().unwrap();
        assert!(json.contains("\"role\": \"unknown\""), "{json}");
        assert_eq!(
            Manifest::from_json_str(&json).unwrap().tensors.unwrap()[0].role,
            TensorRole::Unknown
        );
    }

    #[test]
    fn every_role_in_the_nsir_vocabulary_round_trips_on_the_wire() {
        for role in [
            TensorRole::TokenEmbedding,
            TensorRole::PositionEmbedding,
            TensorRole::AttentionQueryProjection,
            TensorRole::AttentionKeyProjection,
            TensorRole::AttentionValueProjection,
            TensorRole::AttentionOutputProjection,
            TensorRole::AttentionQueryNorm,
            TensorRole::AttentionKeyNorm,
            TensorRole::MlpGateProjection,
            TensorRole::MlpUpProjection,
            TensorRole::MlpDownProjection,
            TensorRole::MoeRouter,
            TensorRole::MoeExpertGateProjection,
            TensorRole::MoeExpertUpProjection,
            TensorRole::MoeExpertDownProjection,
            TensorRole::InputLayerNorm,
            TensorRole::PostAttentionLayerNorm,
            TensorRole::FinalNorm,
            TensorRole::LmHead,
            TensorRole::Bias,
            TensorRole::Unknown,
        ] {
            let mut m = a_valid_manifest();
            m.tensors.as_mut().unwrap()[0].role = role;
            let json = m.to_json_string().unwrap();
            assert!(json.contains(&format!("\"role\": \"{}\"", role.as_str())));
            assert_eq!(
                Manifest::from_json_str(&json).unwrap().tensors.unwrap()[0].role,
                role
            );
        }
    }

    #[test]
    fn every_safetensors_dtype_round_trips_on_the_wire() {
        for dtype in [
            DType::Bool,
            DType::U8,
            DType::I8,
            DType::F8E4M3,
            DType::F8E5M2,
            DType::I16,
            DType::U16,
            DType::F16,
            DType::BF16,
            DType::I32,
            DType::U32,
            DType::F32,
            DType::I64,
            DType::U64,
            DType::F64,
        ] {
            let mut m = a_valid_manifest();
            m.tensors.as_mut().unwrap()[0].dtype = dtype;
            let json = m.to_json_string().unwrap();
            assert!(json.contains(&format!("\"dtype\": \"{}\"", dtype.as_safetensors_str())));
            assert_eq!(
                Manifest::from_json_str(&json).unwrap().tensors.unwrap()[0].dtype,
                dtype
            );
        }
    }

    #[test]
    fn every_result_is_labelled_exact_sampled_or_approximate() {
        for (source, expected) in [
            (ResultFidelity::Exact, "exact"),
            (ResultFidelity::Sampled, "sampled"),
            (ResultFidelity::Approximate, "approximate"),
        ] {
            let mut m = a_valid_manifest();
            m.fidelity = Fidelity::from(source);
            assert_eq!(m.fidelity.as_str(), expected);
            assert_eq!(m.fidelity.as_str(), source.as_str());
            assert!(m
                .to_json_string()
                .unwrap()
                .contains(&format!("\"fidelity\": \"{expected}\"")));
        }
        assert_eq!(Fidelity::ALL.len(), 3);
    }

    #[test]
    fn a_backend_this_build_cannot_have_run_is_refused() {
        let mut value = serde_json::to_value(a_valid_manifest()).unwrap();
        value["run"]["backend"] = serde_json::json!("cuda");
        let err = Manifest::from_json_str(&value.to_string())
            .unwrap_err()
            .to_string();
        assert!(err.contains("cuda"), "{err}");
    }

    // -- ordering: a total order fixed by content ----------------------------

    #[test]
    fn arrays_are_sorted_by_content_not_by_insertion_order() {
        let mut m = a_valid_manifest();
        m.layers = vec![
            LayerEntry {
                layer_index: 2,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
            LayerEntry {
                layer_index: 0,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
            LayerEntry {
                layer_index: 1,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
        ];
        m.experts = vec![
            ExpertEntry {
                layer_index: 1,
                expert_index: 1,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
            ExpertEntry {
                layer_index: 0,
                expert_index: 3,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
            ExpertEntry {
                layer_index: 1,
                expert_index: 0,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
        ];
        m.tensors = Some(vec![
            tensor("z.weight", 4),
            tensor("a.weight", 4),
            tensor("m.weight", 4),
        ]);
        m.ranking = vec![
            ranked("m.weight", 0.5, 4),
            ranked("z.weight", 0.75, 4),
            ranked("a.weight", 0.5, 4),
        ];
        m.refusals = vec![
            Refusal {
                requirement_id: "GRID-007".into(),
                what: "b".into(),
                why: "b".into(),
            },
            Refusal {
                requirement_id: "EVAL-001".into(),
                what: "a".into(),
                why: "a".into(),
            },
        ];
        m.frontier.steps = vec![
            FrontierStep {
                keep_set: vec!["z.weight".into()],
                added_bytes: 20,
                error_removed_fraction: 0.75,
            },
            FrontierStep {
                keep_set: vec!["a.weight".into()],
                added_bytes: 10,
                error_removed_fraction: 0.5,
            },
        ];

        let c = m.canonical().unwrap();
        assert_eq!(
            c.layers.iter().map(|l| l.layer_index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(
            c.experts
                .iter()
                .map(|e| (e.layer_index, e.expert_index))
                .collect::<Vec<_>>(),
            vec![(0, 3), (1, 0), (1, 1)]
        );
        assert_eq!(
            c.tensors
                .unwrap()
                .iter()
                .map(|t| t.address.clone())
                .collect::<Vec<_>>(),
            vec!["a.weight", "m.weight", "z.weight"]
        );
        // relative_error desc, then parameter_count desc, then address asc.
        assert_eq!(
            c.ranking
                .iter()
                .map(|r| r.address.clone())
                .collect::<Vec<_>>(),
            vec!["z.weight", "a.weight", "m.weight"]
        );
        assert_eq!(
            c.refusals
                .iter()
                .map(|r| r.requirement_id.clone())
                .collect::<Vec<_>>(),
            vec!["EVAL-001", "GRID-007"]
        );
        assert_eq!(
            c.frontier
                .steps
                .iter()
                .map(|s| s.added_bytes)
                .collect::<Vec<_>>(),
            vec![10, 20]
        );
    }

    #[test]
    fn the_ranking_tie_break_falls_through_to_the_canonical_address() {
        let mut m = a_valid_manifest();
        m.tensors = Some(vec![tensor("b.weight", 4), tensor("a.weight", 4)]);
        m.ranking = vec![ranked("b.weight", 0.5, 4), ranked("a.weight", 0.5, 4)];
        m.frontier.steps = vec![];
        let c = m.canonical().unwrap();
        assert_eq!(
            c.ranking
                .iter()
                .map(|r| r.address.clone())
                .collect::<Vec<_>>(),
            vec!["a.weight", "b.weight"]
        );
    }

    #[test]
    fn a_shuffled_manifest_serialises_to_the_same_bytes_as_a_sorted_one() {
        let mut sorted = a_valid_manifest();
        sorted.tensors = Some(vec![tensor("a.weight", 4), tensor("b.weight", 4)]);
        sorted.ranking = vec![ranked("a.weight", 0.75, 4), ranked("b.weight", 0.5, 4)];
        sorted.frontier.steps = vec![];
        let mut shuffled = sorted.clone();
        shuffled.tensors.as_mut().unwrap().reverse();
        shuffled.ranking.reverse();
        assert_eq!(
            sorted.to_json_string().unwrap(),
            shuffled.to_json_string().unwrap()
        );
    }

    #[test]
    fn a_keep_set_is_sorted_so_two_runs_agree() {
        let mut m = a_valid_manifest();
        m.tensors = Some(vec![tensor("a.weight", 4), tensor("b.weight", 4)]);
        m.ranking = vec![ranked("a.weight", 0.75, 4), ranked("b.weight", 0.5, 4)];
        m.frontier.steps = vec![FrontierStep {
            keep_set: vec!["b.weight".into(), "a.weight".into()],
            added_bytes: 10,
            error_removed_fraction: 1.0,
        }];
        assert_eq!(
            m.canonical().unwrap().frontier.steps[0].keep_set,
            vec!["a.weight".to_string(), "b.weight".to_string()]
        );
    }

    // -- uniqueness ----------------------------------------------------------

    #[test]
    fn a_duplicate_canonical_address_is_refused() {
        let mut m = a_valid_manifest();
        m.tensors = Some(vec![tensor("a.weight", 4), tensor("a.weight", 4)]);
        m.ranking = vec![ranked("a.weight", 0.5, 4)];
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("a.weight"), "{err}");
        assert!(err.contains("duplicate"), "{err}");
    }

    #[test]
    fn a_duplicate_layer_index_is_refused() {
        let mut m = a_valid_manifest();
        m.layers = vec![
            LayerEntry {
                layer_index: 0,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
            LayerEntry {
                layer_index: 0,
                aggregate: an_aggregate(8, 8.0, 1.0),
            },
        ];
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("layer_index"), "{err}");
    }

    #[test]
    fn a_duplicate_expert_is_refused() {
        let mut m = a_valid_manifest();
        m.experts = vec![
            ExpertEntry {
                layer_index: 1,
                expert_index: 0,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
            ExpertEntry {
                layer_index: 1,
                expert_index: 0,
                aggregate: an_aggregate(4, 4.0, 1.0),
            },
        ];
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("expert"), "{err}");
    }

    #[test]
    fn a_duplicate_refusal_is_refused_so_the_order_stays_total() {
        let mut m = a_valid_manifest();
        let r = m.refusals[0].clone();
        m.refusals.push(r);
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("refusals"), "{err}");
    }

    // -- cross-references ----------------------------------------------------

    #[test]
    fn a_ranking_entry_for_a_tensor_that_was_not_examined_is_refused() {
        let mut m = a_valid_manifest();
        m.ranking
            .push(ranked("model.layers[9].invented.weight", 0.1, 4));
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("model.layers[9].invented.weight"), "{err}");
    }

    #[test]
    fn a_frontier_keeping_a_tensor_that_was_not_examined_is_refused() {
        let mut m = a_valid_manifest();
        m.frontier.steps[0].keep_set = vec!["model.layers[9].invented.weight".into()];
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("model.layers[9].invented.weight"), "{err}");
    }

    // -- the greedy claim ----------------------------------------------------

    #[test]
    fn a_frontier_that_drops_the_not_proven_optimal_claim_is_refused() {
        let mut m = a_valid_manifest();
        m.frontier.claim = "Optimal mixed-precision assignment.".into();
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("frontier.claim"), "{err}");
        assert!(err.contains("not proven optimal"), "{err}");
    }

    #[test]
    fn the_frontier_claim_reaches_the_serialised_document() {
        assert!(a_valid_manifest()
            .to_json_string()
            .unwrap()
            .contains(FRONTIER_CLAIM));
    }

    #[test]
    fn an_error_removed_fraction_above_one_is_refused() {
        let mut m = a_valid_manifest();
        m.frontier.steps[0].error_removed_fraction = 1.5;
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("error_removed_fraction"), "{err}");
    }

    // -- projections ---------------------------------------------------------

    #[test]
    fn the_summary_projection_omits_the_tensor_array() {
        let full = a_valid_manifest();
        let summary = full.summary().unwrap();
        assert_eq!(summary.projection, Projection::Summary);
        assert!(summary.tensors.is_none());
        let json = summary.to_json_string().unwrap();
        assert!(!json.contains("\"tensors\""), "{json}");
        assert!(json.contains("\"projection\": \"summary\""), "{json}");
        // Everything a decision needs is still there.
        assert!(json.contains("\"ranking\"") && json.contains("\"frontier\""));
    }

    #[test]
    fn a_full_projection_without_a_tensor_array_is_refused() {
        let mut m = a_valid_manifest();
        m.tensors = None;
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("projection"), "{err}");
        assert!(err.contains("tensors"), "{err}");
    }

    #[test]
    fn a_summary_projection_that_still_carries_tensors_is_refused() {
        let mut m = a_valid_manifest();
        m.projection = Projection::Summary;
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("tensors"), "{err}");
    }

    #[test]
    fn an_empty_run_is_valid_when_refusals_explain_why() {
        let mut m = a_valid_manifest();
        m.tensors = Some(vec![]);
        m.layers = vec![];
        m.experts = vec![];
        m.ranking = vec![];
        m.frontier.steps = vec![];
        m.totals = ErrorAggregate {
            count: 0,
            sum_sq_base: 0.0,
            sum_sq_delta: 0.0,
            sum_abs_delta: 0.0,
            max_abs_delta: 0.0,
            bytes_at_base_precision: 0,
            bytes_at_target_precision: 0,
        };
        m.refusals = vec![Refusal {
            requirement_id: "QUANT-003".into(),
            what: "every tensor in the checkpoint".into(),
            why: "The run was cancelled before the first tensor completed.".into(),
        }];
        let json = m.to_json_string().unwrap();
        // An empty full projection and a summary are distinguishable.
        assert!(json.contains("\"projection\": \"full\""), "{json}");
        assert!(json.contains("\"tensors\": []"), "{json}");
        assert!(json.contains("QUANT-003"), "{json}");
    }

    #[test]
    fn a_refusal_carries_its_requirement_id() {
        let json = a_valid_manifest().to_json_string().unwrap();
        assert!(json.contains("\"requirement_id\": \"EVAL-001\""), "{json}");
        let parsed = Manifest::from_json_str(&json).unwrap();
        assert_eq!(parsed.refusals[0].requirement_id, "EVAL-001");
    }

    #[test]
    fn refusals_survive_the_summary_projection() {
        let summary = a_valid_manifest().summary().unwrap();
        assert_eq!(summary.refusals[0].requirement_id, "EVAL-001");
    }

    // -- granularity ----------------------------------------------------------

    #[test]
    fn per_group_granularity_without_a_group_size_is_refused() {
        let mut m = a_valid_manifest();
        m.config.granularity.group_size = None;
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("group_size"), "{err}");
    }

    #[test]
    fn a_group_size_on_a_non_group_granularity_is_refused() {
        let mut m = a_valid_manifest();
        m.config.granularity.kind = GranularityKind::PerTensor;
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("group_size"), "{err}");
    }

    #[test]
    fn a_zero_block_dimension_is_refused() {
        let mut m = a_valid_manifest();
        m.config.block_rows = 0;
        let err = m.to_json_string().unwrap_err().to_string();
        assert!(err.contains("block_rows"), "{err}");
    }

    // -- float formatting ------------------------------------------------------

    #[test]
    fn float_values_round_trip_f64_exactly() {
        for value in [
            0.1_f64,
            0.1 + 0.2,
            f64::EPSILON,
            f64::MIN_POSITIVE,
            1.0e300,
            1.0e-300,
            std::f64::consts::PI,
            -0.0,
        ] {
            let mut m = a_valid_manifest();
            m.totals.sum_sq_base = value.abs().max(f64::MIN_POSITIVE);
            m.ranking[0].relative_error = value.abs();
            let json = m.to_json_string().unwrap();
            let back = Manifest::from_json_str(&json).unwrap();
            assert_eq!(
                back.ranking[0].relative_error.to_bits(),
                value.abs().to_bits(),
                "{value} did not survive the round trip"
            );
        }
    }

    #[test]
    fn float_formatting_uses_enough_digits_to_round_trip() {
        // 0.1 + 0.2 is the standard demonstration that binary64 needs all 17
        // significant digits: the nearest double to 0.3 is not the sum, and a
        // shorter rendering would silently substitute it.
        let mut m = a_valid_manifest();
        m.ranking[0].relative_error = 0.1 + 0.2;
        assert!(
            m.to_json_string().unwrap().contains("0.30000000000000004"),
            "the manifest must carry every digit needed to recover the f64"
        );
    }

    #[test]
    fn float_formatting_is_identical_across_repeated_serialisations() {
        let mut m = a_valid_manifest();
        m.totals.sum_sq_base = std::f64::consts::PI;
        m.totals.sum_sq_delta = std::f64::consts::E;
        let first = m.to_json_string().unwrap();
        for _ in 0..8 {
            assert_eq!(m.to_json_string().unwrap(), first);
        }
    }

    #[test]
    fn a_byte_count_above_the_f64_integer_limit_survives_the_round_trip_exactly() {
        // `Manifest` carries `#[serde(flatten)] extensions`, and serde buffers
        // every field of a flattened struct through its self-describing
        // `Content` enum on the way in. If an integer took the `f64` arm of
        // that buffer it would come back rounded, and a checkpoint size is
        // exactly the field where that would go unnoticed: 2^53 is 9.0 PB, but
        // `bytes_read` accumulates and `u64::MAX` is what the type promises.
        let mut m = a_valid_manifest();
        m.model.checkpoint_bytes = 9_007_199_254_740_993; // 2^53 + 1
        m.run.bytes_read = u64::MAX;
        m.run.peak_resident_bytes = 9_007_199_254_740_995; // 2^53 + 3
        m.model.parameter_count = 18_446_744_073_709_551_615;

        let json = m.to_json_string().unwrap();
        assert!(json.contains("9007199254740993"), "{json}");
        assert!(json.contains("18446744073709551615"), "{json}");

        let back = Manifest::from_json_str(&json).unwrap();
        assert_eq!(back.model.checkpoint_bytes, 9_007_199_254_740_993);
        assert_eq!(back.run.bytes_read, u64::MAX);
        assert_eq!(back.run.peak_resident_bytes, 9_007_199_254_740_995);
        assert_eq!(back.model.parameter_count, u64::MAX);
    }

    #[test]
    fn every_non_finite_float_field_is_refused_rather_than_written_as_null() {
        // `## Error Handling`: "NaN or Infinity in a numeric field — refuse at
        // serialization". Enumerated rather than spot-checked, so that a field
        // added later without a guard fails here instead of shipping a `null`
        // that reads as an absence rather than a failure.
        /// One field, and the way to put a non-finite value into it.
        type Poison = (&'static str, Box<dyn Fn(&mut Manifest)>);

        let poison: Vec<Poison> = vec![
            (
                "run.elapsed_seconds",
                Box::new(|m: &mut Manifest| m.run.elapsed_seconds = f64::NAN),
            ),
            (
                "totals.sum_sq_base",
                Box::new(|m: &mut Manifest| m.totals.sum_sq_base = f64::NAN),
            ),
            (
                "totals.sum_sq_delta",
                Box::new(|m: &mut Manifest| m.totals.sum_sq_delta = f64::INFINITY),
            ),
            (
                "totals.sum_abs_delta",
                Box::new(|m: &mut Manifest| m.totals.sum_abs_delta = f64::NEG_INFINITY),
            ),
            (
                "totals.max_abs_delta",
                Box::new(|m: &mut Manifest| m.totals.max_abs_delta = f64::NAN),
            ),
            (
                "layers[0].aggregate.sum_sq_delta",
                Box::new(|m: &mut Manifest| m.layers[0].aggregate.sum_sq_delta = f64::NAN),
            ),
            (
                "experts[0].aggregate.sum_sq_delta",
                Box::new(|m: &mut Manifest| {
                    m.experts = vec![ExpertEntry {
                        layer_index: 0,
                        expert_index: 0,
                        aggregate: an_aggregate(4, 4.0, f64::NAN),
                    }];
                }),
            ),
            (
                "tensors[0].aggregate.sum_sq_base",
                Box::new(|m: &mut Manifest| {
                    m.tensors.as_mut().unwrap()[0].aggregate.sum_sq_base = f64::INFINITY
                }),
            ),
            (
                "tensors[0].outlier_attribution.top_0_1_percent_share",
                Box::new(|m: &mut Manifest| {
                    m.tensors.as_mut().unwrap()[0].outlier_attribution = Some(OutlierAttribution {
                        top_0_1_percent_share: f64::NAN,
                        top_1_percent_share: 0.5,
                    })
                }),
            ),
            (
                "tensors[0].outlier_attribution.top_1_percent_share",
                Box::new(|m: &mut Manifest| {
                    m.tensors.as_mut().unwrap()[0].outlier_attribution = Some(OutlierAttribution {
                        top_0_1_percent_share: 0.5,
                        top_1_percent_share: f64::INFINITY,
                    })
                }),
            ),
            (
                "ranking[0].relative_error",
                Box::new(|m: &mut Manifest| m.ranking[0].relative_error = f64::NAN),
            ),
            (
                "frontier.steps[0].error_removed_fraction",
                Box::new(|m: &mut Manifest| m.frontier.steps[0].error_removed_fraction = f64::NAN),
            ),
        ];

        for (field, poison) in poison {
            let mut m = a_valid_manifest();
            poison(&mut m);
            let err = m.to_json_string().unwrap_err().to_string();
            assert!(
                err.contains(field.rsplit('.').next().unwrap()),
                "{field} must be named in its own refusal: {err}"
            );
            // The refusal must also hold on the validate() path, so that a
            // consumer calling it directly is protected too.
            assert!(
                m.validate().is_err(),
                "{field} must never reach serialization"
            );
        }
    }

    // -- constants -------------------------------------------------------------

    #[test]
    fn the_schema_id_carries_an_explicit_version() {
        assert!(MANIFEST_SCHEMA_ID.ends_with("/v1"));
        assert_eq!(MANIFEST_VERSION, 1);
        assert_eq!(MAX_IMPLEMENTED_RANK, 3);
    }
}
