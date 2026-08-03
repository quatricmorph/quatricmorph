//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §14.5, §17).
//!
//! The conversion-job state machine.
//!
//! **Scope in this pass: types and persistence only.** No CUDA job, no tile
//! compiler, and no worker pool is wired to these records — see requirement
//! `JOB-002` in STATUS.md. The state machine exists now because the offline
//! conversion path (`AccessScale::FullModelOfflineConversion`) is inherently
//! long-running, interruptible, and resumable, and retrofitting those semantics
//! later means retrofitting them into every caller.

use q_source::error::{QError, Result};
use serde::{Deserialize, Serialize};

/// What a job is converting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    /// Ingest metadata (headers only).
    MetadataImport,
    /// Compute tensor/block statistics.
    StatisticsPass,
    /// Build the `.qtile` multiresolution pyramid.
    TilePyramid,
    /// Emit `tileset.json` + GLB tile content.
    VisualizationArtifacts,
}

impl JobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            JobKind::MetadataImport => "metadata_import",
            JobKind::StatisticsPass => "statistics_pass",
            JobKind::TilePyramid => "tile_pyramid",
            JobKind::VisualizationArtifacts => "visualization_artifacts",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "metadata_import" => JobKind::MetadataImport,
            "statistics_pass" => JobKind::StatisticsPass,
            "tile_pyramid" => JobKind::TilePyramid,
            "visualization_artifacts" => JobKind::VisualizationArtifacts,
            other => return Err(QError::Catalog(format!("unknown job kind `{other}`"))),
        })
    }
}

/// Job lifecycle.
///
/// ```text
///  Pending ──> Running ──> Succeeded
///     │           │  │
///     │           │  └──> Cancelled ──> Running   (resume)
///     │           └─────> Failed    ──> Running   (retry)
///     └─────────────────> Cancelled
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            JobState::Pending => "pending",
            JobState::Running => "running",
            JobState::Succeeded => "succeeded",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "pending" => JobState::Pending,
            "running" => JobState::Running,
            "succeeded" => JobState::Succeeded,
            "failed" => JobState::Failed,
            "cancelled" => JobState::Cancelled,
            other => return Err(QError::Catalog(format!("unknown job state `{other}`"))),
        })
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, JobState::Succeeded)
    }

    /// Whether `self -> next` is a legal transition.
    ///
    /// `Succeeded` is final: a job that completed cannot be restarted, only
    /// superseded by a new job. That keeps the `algorithm_version` in cache
    /// keys meaningful.
    pub fn can_transition_to(self, next: JobState) -> bool {
        use JobState::*;
        matches!(
            (self, next),
            (Pending, Running)
                | (Pending, Cancelled)
                | (Running, Succeeded)
                | (Running, Failed)
                | (Running, Cancelled)
                | (Failed, Running)
                | (Cancelled, Running)
        )
    }
}

/// A persisted conversion job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversionJob {
    pub job_id: String,
    pub model_id: String,
    pub kind: JobKind,
    pub state: JobState,
    pub created_at: i64,
    pub updated_at: i64,
    pub units_total: u64,
    pub units_done: u64,
    /// Opaque resume state, e.g. a serialized `q_source::ResumePoint`.
    pub resume_token: Option<String>,
    pub error_message: Option<String>,
    /// Requirement ID explaining why this job kind cannot run yet, if it can't.
    pub requirement: Option<String>,
}

impl ConversionJob {
    pub fn new(job_id: impl Into<String>, model_id: impl Into<String>, kind: JobKind) -> Self {
        let now = crate::schema::now_unix();
        Self {
            job_id: job_id.into(),
            model_id: model_id.into(),
            kind,
            state: JobState::Pending,
            created_at: now,
            updated_at: now,
            units_total: 0,
            units_done: 0,
            resume_token: None,
            error_message: None,
            requirement: None,
        }
    }

    pub fn progress(&self) -> f32 {
        if self.units_total == 0 {
            return 0.0;
        }
        (self.units_done as f32 / self.units_total as f32).clamp(0.0, 1.0)
    }

    /// Move to `next`, rejecting illegal transitions rather than silently
    /// overwriting state.
    pub fn transition(&mut self, next: JobState) -> Result<()> {
        if !self.state.can_transition_to(next) {
            return Err(QError::Catalog(format!(
                "illegal job transition {} -> {} for job {}",
                self.state.as_str(),
                next.as_str(),
                self.job_id
            )));
        }
        self.state = next;
        self.updated_at = crate::schema::now_unix();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_transitions_are_accepted() {
        let mut j = ConversionJob::new("j1", "m1", JobKind::TilePyramid);
        assert_eq!(j.state, JobState::Pending);
        j.transition(JobState::Running).unwrap();
        j.transition(JobState::Succeeded).unwrap();
        assert!(j.state.is_terminal());
    }

    #[test]
    fn illegal_transitions_are_rejected() {
        let mut j = ConversionJob::new("j1", "m1", JobKind::StatisticsPass);
        assert!(j.transition(JobState::Succeeded).is_err());
        j.transition(JobState::Running).unwrap();
        j.transition(JobState::Succeeded).unwrap();
        // Succeeded is final.
        assert!(j.transition(JobState::Running).is_err());
    }

    #[test]
    fn failed_and_cancelled_jobs_can_resume() {
        let mut j = ConversionJob::new("j1", "m1", JobKind::TilePyramid);
        j.transition(JobState::Running).unwrap();
        j.transition(JobState::Failed).unwrap();
        assert!(j.transition(JobState::Running).is_ok());
        j.transition(JobState::Cancelled).unwrap();
        assert!(j.transition(JobState::Running).is_ok());
    }

    #[test]
    fn progress_is_bounded() {
        let mut j = ConversionJob::new("j1", "m1", JobKind::MetadataImport);
        assert_eq!(j.progress(), 0.0);
        j.units_total = 64;
        j.units_done = 16;
        assert_eq!(j.progress(), 0.25);
        j.units_done = 999;
        assert_eq!(j.progress(), 1.0);
    }

    #[test]
    fn kind_and_state_strings_round_trip() {
        for k in [
            JobKind::MetadataImport,
            JobKind::StatisticsPass,
            JobKind::TilePyramid,
            JobKind::VisualizationArtifacts,
        ] {
            assert_eq!(JobKind::parse(k.as_str()).unwrap(), k);
        }
        for s in [
            JobState::Pending,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed,
            JobState::Cancelled,
        ] {
            assert_eq!(JobState::parse(s.as_str()).unwrap(), s);
        }
        assert!(JobKind::parse("teleport").is_err());
        assert!(JobState::parse("vibing").is_err());
    }
}
