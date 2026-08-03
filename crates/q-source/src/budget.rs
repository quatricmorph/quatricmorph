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
}
