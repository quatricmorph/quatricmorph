//! Data plane: **Metadata Plane** (ARCHITECTURE.md §18 AC-003).
//!
//! Cancellation and resume for multi-step ingestion.
//!
//! ARCHITECTURE.md acceptance criterion 3 requires metadata import to be
//! cancellable and resumable. Header-only ingestion is a single cheap pass per
//! shard, so both are implemented for real here: [`CancellationToken`] is
//! checked at every shard boundary, and [`ResumePoint`] records which shards
//! already completed so a re-run skips them.

use crate::error::{QError, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cooperative cancellation shared across an ingestion pass.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Return `Err(QError::Cancelled)` if cancellation was requested.
    /// `checkpoint` names where we stopped so the caller can resume there.
    pub fn check(&self, checkpoint: impl Into<String>) -> Result<()> {
        if self.is_cancelled() {
            return Err(QError::Cancelled {
                checkpoint: checkpoint.into(),
            });
        }
        Ok(())
    }
}

/// Work that can be interrupted between units and picked up again.
pub trait Cancellable {
    /// The unit of progress, e.g. a shard file name.
    type Unit;

    /// Units already finished; a resumed run skips these.
    fn completed_units(&self) -> &[Self::Unit];
}

/// Durable record of how far an ingestion got.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResumePoint {
    /// Shard URIs whose headers were parsed and persisted.
    pub completed_shards: Vec<String>,
    /// Shard URI we stopped inside, if cancellation landed mid-file.
    pub interrupted_at: Option<String>,
}

impl ResumePoint {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_complete(&self, shard_uri: &str) -> bool {
        self.completed_shards.iter().any(|s| s == shard_uri)
    }

    pub fn mark_complete(&mut self, shard_uri: impl Into<String>) {
        let uri = shard_uri.into();
        if !self.is_complete(&uri) {
            self.completed_shards.push(uri);
        }
        self.interrupted_at = None;
    }

    pub fn mark_interrupted(&mut self, shard_uri: impl Into<String>) {
        self.interrupted_at = Some(shard_uri.into());
    }
}

impl Cancellable for ResumePoint {
    type Unit = String;

    fn completed_units(&self) -> &[String] {
        &self.completed_shards
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_starts_live_and_flips_once_cancelled() {
        let t = CancellationToken::new();
        assert!(t.check("start").is_ok());
        t.cancel();
        assert!(t.is_cancelled());
        match t.check("shard-2") {
            Err(QError::Cancelled { checkpoint }) => assert_eq!(checkpoint, "shard-2"),
            other => panic!("expected Cancelled, got {other:?}"),
        }
    }

    #[test]
    fn token_clones_share_one_flag() {
        let a = CancellationToken::new();
        let b = a.clone();
        b.cancel();
        assert!(a.is_cancelled());
    }

    #[test]
    fn resume_point_skips_completed_and_is_idempotent() {
        let mut r = ResumePoint::new();
        r.mark_complete("shard-1");
        r.mark_complete("shard-1");
        assert_eq!(r.completed_units().len(), 1);
        assert!(r.is_complete("shard-1"));
        assert!(!r.is_complete("shard-2"));
        r.mark_interrupted("shard-2");
        assert_eq!(r.interrupted_at.as_deref(), Some("shard-2"));
        r.mark_complete("shard-2");
        assert!(r.interrupted_at.is_none());
    }
}
