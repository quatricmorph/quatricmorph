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
