//! # q-source — Artifact Plane
//!
//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1), with the
//! [`TensorDescriptor`] record that bridges into the Metadata Plane.
//!
//! This crate owns the immutable source of truth: `config.json`,
//! `tokenizer.json`, `model.safetensors.index.json`, and the
//! `model-NNNNN-of-MMMMM.safetensors` shards. Artifacts are never rewritten in
//! place.
//!
//! ## The contract every reader here upholds
//!
//! * **Nothing is checkpoint-size-proportional.** Manifests list file names and
//!   lengths. Reads are byte ranges. The only way to get a `Vec<u8>` is
//!   [`ByteStream::read_all_within_budget`], which requires a named
//!   [`MemoryBudget`]. There is no `load_tensor()` and no `load_model()`,
//!   because at trillion scale those cannot exist.
//! * **Unknown means unknown.** [`DType::parse_safetensors`] refuses dtypes it
//!   does not know; [`TensorRole::Unknown`] is a legitimate answer. Nothing
//!   here infers meaning from shape.
//! * **Unbuilt means unbuilt.** [`QError::NotImplemented`] carries a
//!   requirement ID. No stub in this crate returns plausible fake data.
//!
//! ## Scale vocabulary
//!
//! [`AccessScale`] distinguishes the six access modes the architecture treats
//! as fundamentally different. It exists as a type so code and APIs cannot
//! quietly imply that one substitutes for another.

pub mod budget;
pub mod cancel;
pub mod descriptor;
pub mod dtype;
pub mod error;
pub mod http;
pub mod ids;
pub mod local;
pub mod manifest;
pub mod role;

pub use budget::MemoryBudget;
pub use cancel::{Cancellable, CancellationToken, ResumePoint};
pub use descriptor::TensorDescriptor;
pub use dtype::DType;
pub use error::{QError, Result};
pub use http::{HttpByteRange, HttpRangeSource, RangeFetcher};
pub use ids::{content_fingerprint, ModelId, TensorId};
pub use local::LocalFsSource;
pub use manifest::{
    ArtifactKind, ByteStream, ModelManifest, ModelSource, ModelSourceExt, SourceFile,
};
pub use role::{Component, Stack, TensorRole};

use serde::{Deserialize, Serialize};

/// The six access scales the architecture treats as distinct.
///
/// These are not performance tiers; they are *different guarantees*. A caller
/// that asks for [`AccessScale::Metadata`] must never be handed sampled visual
/// data, and a UI showing [`AccessScale::Visualization`] must never claim it is
/// showing exact values. Carrying the scale in the type system is how
/// ARCHITECTURE.md §18 AC-010 ("the UI clearly indicates exact, sampled, or
/// approximate results") is enforced rather than merely intended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessScale {
    /// Names, shapes, dtypes, byte ranges. Bounded memory at any checkpoint
    /// size. No payload bytes are read.
    Metadata,
    /// Downsampled / summarized values for rendering. Approximate by
    /// construction; never an answer to "what is this weight?".
    Visualization,
    /// Exact bytes for a selected block or scalar, read on demand.
    SelectedBlockExact,
    /// Computation over one selected block (statistics, a block matmul).
    SelectedBlockCompute,
    /// Offline conversion of a whole model into tiles. Runs as a job, not a
    /// request; bounded by streaming, not by RAM.
    FullModelOfflineConversion,
    /// Numerical computation over a whole model. Explicitly out of scope for a
    /// single RTX 3090 at trillion scale; requires an explicit cost gate.
    FullModelNumericalCompute,
}

impl AccessScale {
    pub fn as_str(self) -> &'static str {
        match self {
            AccessScale::Metadata => "metadata",
            AccessScale::Visualization => "visualization",
            AccessScale::SelectedBlockExact => "selected_block_exact",
            AccessScale::SelectedBlockCompute => "selected_block_compute",
            AccessScale::FullModelOfflineConversion => "full_model_offline_conversion",
            AccessScale::FullModelNumericalCompute => "full_model_numerical_compute",
        }
    }

    /// Whether results at this scale are exact.
    ///
    /// Used to label query results; a `false` here must reach the user, not be
    /// rounded up to "exact" in a summary.
    pub fn is_exact(self) -> bool {
        matches!(
            self,
            AccessScale::Metadata
                | AccessScale::SelectedBlockExact
                | AccessScale::SelectedBlockCompute
                | AccessScale::FullModelNumericalCompute
        )
    }

    /// Whether this scale requires reading weight payload at all.
    pub fn reads_payload(self) -> bool {
        !matches!(self, AccessScale::Metadata)
    }
}

/// How a returned value was obtained. Surfaced in API responses and the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultFidelity {
    Exact,
    Sampled,
    Approximate,
}

impl ResultFidelity {
    pub fn as_str(self) -> &'static str {
        match self {
            ResultFidelity::Exact => "exact",
            ResultFidelity::Sampled => "sampled",
            ResultFidelity::Approximate => "approximate",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_scale_never_reads_payload() {
        assert!(!AccessScale::Metadata.reads_payload());
        assert!(AccessScale::SelectedBlockExact.reads_payload());
    }

    #[test]
    fn visualization_scale_is_never_exact() {
        assert!(!AccessScale::Visualization.is_exact());
        assert!(AccessScale::SelectedBlockExact.is_exact());
    }

    #[test]
    fn scale_names_are_stable() {
        assert_eq!(AccessScale::Metadata.as_str(), "metadata");
        assert_eq!(
            AccessScale::FullModelNumericalCompute.as_str(),
            "full_model_numerical_compute"
        );
    }
}
