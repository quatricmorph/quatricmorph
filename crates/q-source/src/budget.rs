//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1).
//!
//! Explicit, named memory budgets.
//!
//! Every constant here exists so that no call site contains a bare magic
//! number, and so that "this path is bounded" is a checkable property rather
//! than a comment. A function that would allocate proportionally to total
//! checkpoint size is a bug; [`MemoryBudget::check`] is how that bug is caught
//! before the allocation happens.

use crate::error::{QError, Result};

/// SafeTensors caps its JSON header at 100 MB. Anything larger is either a
/// corrupt file or an attempt to make the parser allocate unboundedly, so the
/// header reader refuses it before allocating.
pub const MAX_HEADER_BYTES: u64 = 100 * 1024 * 1024;

/// Largest single tensor payload read a default-configured process will
/// materialize in RAM. Selected-block reads must stay under this; whole-tensor
/// reads at checkpoint scale never will, which is the point.
pub const MAX_SINGLE_READ_BYTES: u64 = 64 * 1024 * 1024;

/// Chunk size for streaming copies. Streaming paths allocate this much, once,
/// regardless of how large the source range is.
pub const STREAM_CHUNK_BYTES: usize = 1024 * 1024;

/// Ceiling on the metadata a single ingestion pass keeps resident. Tensor
/// *descriptors* are small (~200 bytes each), so this bounds a checkpoint's
/// metadata working set without bounding the checkpoint.
pub const MAX_INGEST_METADATA_BYTES: u64 = 512 * 1024 * 1024;

/// Largest slice a WeightQL scalar/slice query will return over the local API.
pub const MAX_QUERY_RESULT_ELEMENTS: u64 = 1024 * 1024;

// --- streaming block reader (`.plan/MEMORY_BUDGET.md` §4) --------------------
//
// These four are the budgets a bounded streaming pass is configured against.
// They are counts and byte ceilings over *decoded host blocks*, which are
// always `f32` after decode, so `decoded_block_bytes = block_rows ×
// block_columns × 4` regardless of the storage dtype.
//
// The property they exist to protect: peak residency is a function of block
// size and concurrency and **never of tensor size**. A configuration whose
// `MAX_CONCURRENT_BLOCKS × decoded_block_bytes` exceeds
// `MAX_HOST_STAGING_BYTES` is refused before the first byte is read.

/// Default block edge. `.plan/MEMORY_BUDGET.md` §4: 256 × 256 f32 = 256 KiB
/// decoded.
pub const DEFAULT_BLOCK_DIMENSION: u64 = 256;

/// Ceiling on the decoded host staging a streaming pass keeps resident.
/// `.plan/MEMORY_BUDGET.md` §4 requires this to be at least
/// `MAX_CONCURRENT_BLOCKS × block_elements × 4`; at defaults that is 1 MiB, so
/// the ceiling is 512× the default working set.
pub const MAX_HOST_STAGING_BYTES: u64 = 512 * 1024 * 1024;

/// Decoded blocks a streaming pass may hold at once.
/// `.plan/MEMORY_BUDGET.md` §4.
pub const MAX_CONCURRENT_BLOCKS: usize = 4;

/// Depth of the bounded output queue. A full queue **blocks the reader**; it is
/// never grown, because growing it converts a throughput problem into an
/// out-of-memory crash (`.plan/MEMORY_BUDGET.md` §4).
pub const MAX_OUTPUT_QUEUE_DEPTH: usize = 64;

/// Floor for adaptive block halving. On allocation failure both block
/// dimensions halve and the pass retries; below this edge it fails naming the
/// budget rather than trying to stream a degenerate block
/// (`.plan/MEMORY_BUDGET.md` §5).
pub const MIN_BLOCK_DIMENSION: u64 = 64;

// --- the process resident ceiling `C` (`.plan/MASTER_PLAN.md` §4) ------------

/// **The configured resident ceiling `C`.** Compiled default: 2 GiB.
///
/// Every other budget in this module caps *one request* — the largest header a
/// parser will allocate for, the largest single range read, the decoded staging
/// one streaming pass keeps live. This one is different in kind: it is the
/// ceiling on what the **whole process** may hold resident while streaming a
/// checkpoint, and it is the `C` of `.plan/MASTER_PLAN.md` §4:
///
/// ```text
/// peak resident ≤ 1.25 × C,  with C ≤ 2 GiB
/// N = checkpoint_bytes / C ≥ 100
/// ```
///
/// **2 GiB is not derived from any measurement.** It is the figure
/// `.plan/MASTER_PLAN.md` §4 states as v1's target and
/// `.plan/tasks/QM-0101-bounded-residency-proof/TASK.md`'s Implementation Plan
/// step 1 prescribes as this constant's default. That independence is the whole
/// point of declaring it here: a ceiling back-solved from a measured peak cannot
/// test that peak, because the comparison can never fail. This one can — set it
/// below what a pass actually needs and the pass is refused
/// (`.plan/evidence/QM-0101.md`, `## G1`).
///
/// Settable per run through `.plan/MEMORY_BUDGET.md` §11's precedence chain —
/// CLI flag, then `QM_MAX_RESIDENT_BYTES`, then a config file, then this
/// constant. See [`crate::config`].
pub const MAX_RESIDENT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// The tolerance `.plan/MASTER_PLAN.md` §4 allows over `C` for allocator
/// overhead and mmap accounting, as a numerator over
/// [`RESIDENT_TOLERANCE_DENOMINATOR`].
///
/// A ratio rather than a float so the arithmetic is exact at every magnitude:
/// `1.25 × C` computed in floating point on a multi-gigabyte `C` is not a
/// number a refusal should depend on.
pub const RESIDENT_TOLERANCE_NUMERATOR: u64 = 5;

/// Denominator of [`RESIDENT_TOLERANCE_NUMERATOR`]. `5 / 4 = 1.25`.
pub const RESIDENT_TOLERANCE_DENOMINATOR: u64 = 4;

/// The measured-peak allowance for a ceiling of `ceiling_bytes`: `1.25 × C`,
/// computed exactly.
///
/// This is the number a measured peak RSS is compared against. It is
/// deliberately **not** the number a configuration is admitted against — a pass
/// is admitted against `C` itself, so the 25 % is a tolerance on the
/// *measurement*, never extra room the planner is allowed to spend.
pub const fn resident_tolerance_bytes(ceiling_bytes: u64) -> u64 {
    // Saturating so a ceiling near `u64::MAX` yields `u64::MAX` rather than
    // wrapping to a ceiling smaller than the one asked for.
    match ceiling_bytes.checked_mul(RESIDENT_TOLERANCE_NUMERATOR) {
        Some(scaled) => scaled / RESIDENT_TOLERANCE_DENOMINATOR,
        None => u64::MAX,
    }
}

/// Parse a byte size written the way a human writes one: `4096`, `64MiB`,
/// `2 GiB`, `512mib`.
///
/// Binary suffixes (`KiB`, `MiB`, `GiB`, `TiB`) are powers of 1024; decimal
/// suffixes (`KB`, `MB`, `GB`, `TB`) are powers of 1000. Both are accepted
/// because both appear in this repository's plan documents — `C ≤ 2 GB` in
/// `.plan/MASTER_PLAN.md` §4 and `--resident-ceiling 2GiB` in `TASK.md`'s
/// Suggested Commands — and silently treating one as the other would move a
/// ceiling by 7 %.
///
/// A malformed size is **refused naming the input**, never coerced to a default:
/// a mistyped ceiling that quietly became 2 GiB would turn this gate off.
pub fn parse_byte_size(text: &str) -> Result<u64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(QError::QueryRejected(
            "a byte size cannot be empty; write it as `4096`, `64MiB`, or `2GiB`".to_string(),
        ));
    }
    let digits_end = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (digits, suffix) = trimmed.split_at(digits_end);
    if digits.is_empty() {
        return Err(QError::QueryRejected(format!(
            "`{text}` is not a byte size: it must start with a decimal digit, as in \
             `4096`, `64MiB`, or `2GiB`"
        )));
    }
    let magnitude: u64 = digits.parse().map_err(|_| {
        QError::QueryRejected(format!(
            "`{text}` is not a byte size: `{digits}` does not fit in a 64-bit count of bytes"
        ))
    })?;
    let multiplier = match suffix.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1u64,
        "kib" | "k" => 1024,
        "mib" | "m" => 1024 * 1024,
        "gib" | "g" => 1024 * 1024 * 1024,
        "tib" | "t" => 1024u64 * 1024 * 1024 * 1024,
        "kb" => 1_000,
        "mb" => 1_000_000,
        "gb" => 1_000_000_000,
        "tb" => 1_000_000_000_000,
        other => {
            return Err(QError::QueryRejected(format!(
                "`{text}` is not a byte size: `{other}` is not a unit. Use a bare byte \
                 count, a binary unit (KiB, MiB, GiB, TiB) or a decimal unit (KB, MB, \
                 GB, TB)"
            )))
        }
    };
    magnitude.checked_mul(multiplier).ok_or_else(|| {
        QError::QueryRejected(format!(
            "`{text}` overflows a 64-bit byte count ({digits} × {multiplier})"
        ))
    })
}

/// A named allocation ceiling threaded through read paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBudget {
    pub name: &'static str,
    pub limit_bytes: u64,
}

impl MemoryBudget {
    pub const fn new(name: &'static str, limit_bytes: u64) -> Self {
        Self { name, limit_bytes }
    }

    /// Budget for reading a SafeTensors header.
    pub const fn header() -> Self {
        Self::new("safetensors_header", MAX_HEADER_BYTES)
    }

    /// Budget for a single tensor-payload read.
    pub const fn single_read() -> Self {
        Self::new("single_range_read", MAX_SINGLE_READ_BYTES)
    }

    /// Budget for an ingestion pass's resident metadata.
    pub const fn ingest_metadata() -> Self {
        Self::new("ingest_metadata", MAX_INGEST_METADATA_BYTES)
    }

    /// Budget for the decoded host blocks a streaming pass keeps resident.
    ///
    /// The name is what a failure reports, so a caller that hits this ceiling
    /// learns *which* budget it exceeded rather than only that something was
    /// too large.
    pub const fn host_staging() -> Self {
        Self::new("host_staging", MAX_HOST_STAGING_BYTES)
    }

    /// The **process** resident ceiling `C` at its compiled default.
    ///
    /// Use [`MemoryBudget::resident_at`] for a run whose ceiling came from
    /// `.plan/MEMORY_BUDGET.md` §11's precedence chain; this is the last link in
    /// that chain, not a shortcut past it.
    pub const fn resident() -> Self {
        Self::new("max_resident", MAX_RESIDENT_BYTES)
    }

    /// The process resident ceiling at a configured value.
    ///
    /// The name is fixed so every refusal from this budget is attributable to
    /// the same variable regardless of where the value came from.
    pub const fn resident_at(ceiling_bytes: u64) -> Self {
        Self::new("max_resident", ceiling_bytes)
    }

    /// Fail if `requested` bytes would exceed this budget.
    pub fn check(&self, requested: u64) -> Result<()> {
        if requested > self.limit_bytes {
            return Err(QError::BudgetExceeded {
                budget_name: self.name,
                requested,
                limit: self.limit_bytes,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_admits_under_limit_and_refuses_over() {
        let b = MemoryBudget::new("test", 100);
        assert!(b.check(100).is_ok());
        let err = b.check(101).unwrap_err();
        match err {
            QError::BudgetExceeded {
                budget_name,
                requested,
                limit,
            } => {
                assert_eq!(budget_name, "test");
                assert_eq!((requested, limit), (101, 100));
            }
            other => panic!("expected BudgetExceeded, got {other:?}"),
        }
    }

    #[test]
    fn header_budget_matches_safetensors_spec_cap() {
        assert_eq!(MemoryBudget::header().limit_bytes, 100 * 1024 * 1024);
    }

    /// Every number here is quoted in `.plan/MEMORY_BUDGET.md` §4–§5. If the
    /// document and the code disagree, one of them is wrong and a reader cannot
    /// tell which, so the agreement is asserted rather than assumed.
    #[test]
    fn streaming_budgets_match_the_memory_budget_document() {
        assert_eq!(DEFAULT_BLOCK_DIMENSION, 256);
        assert_eq!(MAX_HOST_STAGING_BYTES, 512 * 1024 * 1024);
        assert_eq!(MAX_CONCURRENT_BLOCKS, 4);
        assert_eq!(MAX_OUTPUT_QUEUE_DEPTH, 64);
        assert_eq!(MIN_BLOCK_DIMENSION, 64);
        assert_eq!(MemoryBudget::host_staging().name, "host_staging");
        assert_eq!(
            MemoryBudget::host_staging().limit_bytes,
            MAX_HOST_STAGING_BYTES
        );
    }

    /// `.plan/MEMORY_BUDGET.md` §4: `host_staging_bytes = N × E × 4` = 1 MiB at
    /// defaults, and `MAX_HOST_STAGING_BYTES` must be at least that.
    #[test]
    fn the_default_block_grid_costs_one_mebibyte_of_decoded_staging() {
        let elements = DEFAULT_BLOCK_DIMENSION * DEFAULT_BLOCK_DIMENSION;
        let decoded_block_bytes = elements * 4;
        assert_eq!(decoded_block_bytes, 256 * 1024);
        let staging = MAX_CONCURRENT_BLOCKS as u64 * decoded_block_bytes;
        assert_eq!(staging, 1024 * 1024);
        assert!(MemoryBudget::host_staging().check(staging).is_ok());
    }

    #[test]
    fn a_tight_host_staging_budget_refuses_the_default_block_grid() {
        let elements = DEFAULT_BLOCK_DIMENSION * DEFAULT_BLOCK_DIMENSION;
        let staging = MAX_CONCURRENT_BLOCKS as u64 * elements * 4;
        let tight = MemoryBudget::new("host_staging", 512 * 1024);
        match tight.check(staging) {
            Err(QError::BudgetExceeded {
                budget_name,
                requested,
                limit,
            }) => {
                assert_eq!(budget_name, "host_staging");
                assert_eq!((requested, limit), (1024 * 1024, 512 * 1024));
            }
            other => panic!("expected BudgetExceeded, got {other:?}"),
        }
    }

    // --- the resident ceiling `C` (`QM-0101`, gate G1) ----------------------

    /// The compiled default is 2 GiB, which is `.plan/MASTER_PLAN.md` §4's
    /// `C ≤ 2 GB` target and `TASK.md`'s Implementation Plan step 1. Asserted so
    /// a later edit cannot move the one ceiling a residency claim is measured
    /// against without a test noticing.
    #[test]
    fn the_resident_ceiling_default_is_two_gibibytes_and_is_a_named_budget() {
        assert_eq!(MAX_RESIDENT_BYTES, 2 * 1024 * 1024 * 1024);
        assert_eq!(MAX_RESIDENT_BYTES, 2_147_483_648);
        assert_eq!(MemoryBudget::resident().name, "max_resident");
        assert_eq!(MemoryBudget::resident().limit_bytes, MAX_RESIDENT_BYTES);
        assert_eq!(MemoryBudget::resident_at(4096).name, "max_resident");
        assert_eq!(MemoryBudget::resident_at(4096).limit_bytes, 4096);
        // It is a *process* ceiling and must not be confused with the
        // per-request caps. All three differ, so a refusal names which was hit.
        assert_ne!(MAX_RESIDENT_BYTES, MAX_HEADER_BYTES);
        assert_ne!(MAX_RESIDENT_BYTES, MAX_SINGLE_READ_BYTES);
        assert_ne!(MAX_RESIDENT_BYTES, MAX_HOST_STAGING_BYTES);
    }

    /// The 1.25 × tolerance is exact integer arithmetic at every magnitude. A
    /// float `1.25 * C` on a multi-gigabyte `C` is not a number a refusal should
    /// depend on.
    #[test]
    fn the_resident_tolerance_is_exactly_five_quarters_and_never_wraps() {
        assert_eq!(RESIDENT_TOLERANCE_NUMERATOR, 5);
        assert_eq!(RESIDENT_TOLERANCE_DENOMINATOR, 4);
        assert_eq!(resident_tolerance_bytes(4), 5);
        assert_eq!(resident_tolerance_bytes(1024), 1280);
        // The G1 ceiling and tolerance of `fixtures/residency-measurements.json`,
        // computed here rather than copied from it.
        assert_eq!(resident_tolerance_bytes(3_528_244), 4_410_305);
        assert_eq!(resident_tolerance_bytes(MAX_RESIDENT_BYTES), 2_684_354_560);
        // Saturating rather than wrapping: a ceiling near the top of the range
        // must not yield a tolerance *below* the ceiling it came from.
        assert_eq!(resident_tolerance_bytes(u64::MAX), u64::MAX);
        assert!(resident_tolerance_bytes(u64::MAX - 1) >= u64::MAX - 1);
        // Zero stays zero, so a zero ceiling admits nothing rather than
        // everything.
        assert_eq!(resident_tolerance_bytes(0), 0);
    }

    /// Every form `TASK.md`'s Suggested Commands and `.plan/MASTER_PLAN.md` §4
    /// use, plus the refusals. Binary and decimal units differ by 7 % at the
    /// gigabyte, so conflating them would move a ceiling.
    #[test]
    fn byte_sizes_parse_binary_and_decimal_units_and_refuse_everything_else() {
        for (text, expected) in [
            ("0", 0u64),
            ("4096", 4096),
            ("4096B", 4096),
            ("64MiB", 67_108_864),
            ("2GiB", 2_147_483_648),
            ("512MiB", 536_870_912),
            ("1KiB", 1024),
            ("1TiB", 1_099_511_627_776),
            ("  2 GiB  ", 2_147_483_648),
            ("2gib", 2_147_483_648),
            ("2G", 2_147_483_648),
            ("2GB", 2_000_000_000),
            ("2MB", 2_000_000),
            ("2KB", 2_000),
            ("2TB", 2_000_000_000_000),
            ("3528244", 3_528_244),
        ] {
            assert_eq!(parse_byte_size(text).unwrap(), expected, "input {text:?}");
        }
        // `GB` is 10^9 and `GiB` is 2^30. They are different ceilings.
        assert_ne!(
            parse_byte_size("2GB").unwrap(),
            parse_byte_size("2GiB").unwrap()
        );

        for bad in [
            "",
            "   ",
            "GiB",
            "-1",
            "1.5GiB",
            "two",
            "1e9",
            "1XiB",
            "1 Gib B",
            "18446744073709551616",
        ] {
            let err = parse_byte_size(bad).unwrap_err();
            assert!(
                matches!(err, QError::QueryRejected(_)),
                "{bad:?} gave {err:?}"
            );
            // The refusal quotes what it was given, so an operator can see the
            // typo rather than only that something was wrong.
            let msg = err.to_string();
            if !bad.trim().is_empty() {
                assert!(
                    msg.contains(bad.trim()) || msg.contains(bad),
                    "message was {msg}"
                );
            }
        }
        // Overflow is refused rather than wrapped to a small ceiling.
        let err = parse_byte_size("18446744073709551615TiB").unwrap_err();
        assert!(err.to_string().contains("overflows"), "message was {err}");
    }

    /// The halving ladder of `.plan/MEMORY_BUDGET.md` §5 terminates: 256 → 128
    /// → 64 → refuse. A floor that a halving sequence could step over would let
    /// a degenerate block through.
    #[test]
    fn the_halving_ladder_lands_exactly_on_the_block_dimension_floor() {
        let mut edge = DEFAULT_BLOCK_DIMENSION;
        let mut steps = 0;
        while edge > MIN_BLOCK_DIMENSION {
            edge /= 2;
            steps += 1;
            assert!(
                edge >= MIN_BLOCK_DIMENSION,
                "halving stepped over the floor"
            );
        }
        assert_eq!((edge, steps), (MIN_BLOCK_DIMENSION, 2));
    }
}
