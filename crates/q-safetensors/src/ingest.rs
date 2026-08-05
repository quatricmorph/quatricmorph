//! Data plane: **Artifact Plane** → **Metadata Plane** (ARCHITECTURE.md §4.1).
//!
//! Metadata ingestion: manifest → shard headers → [`TensorDescriptor`]s.
//!
//! This implements the import process of §4.1 up to (not including) semantic
//! resolution — `q-nsir` annotates the descriptors afterwards, matching the
//! architecture's own pipeline order (Ingestion → Architecture Resolver → NSIR).
//!
//! ## Bounded memory
//!
//! Per shard, this reads `8 + header_length` bytes and drops them. The only
//! thing that grows with checkpoint size is the descriptor vector, and a
//! descriptor is ~200 bytes regardless of the tensor's size. The declared
//! ceiling is [`MAX_INGEST_METADATA_BYTES`]; exceeding it is an error, not a
//! silent OOM.
//!
//! ## Cancel and resume
//!
//! A [`CancellationToken`] is checked at every shard boundary and a
//! [`ResumePoint`] records completed shards, satisfying ARCHITECTURE.md §18
//! AC-003. Because header reads are a single cheap pass, mid-shard resume buys
//! nothing and is deliberately not implemented.

use crate::header::SafeTensorsHeader;
use crate::index::ShardIndex;
use q_source::budget::{MemoryBudget, MAX_INGEST_METADATA_BYTES};
use q_source::cancel::{CancellationToken, ResumePoint};
use q_source::error::{QError, Result};
use q_source::ids::ModelId;
use q_source::manifest::{ArtifactKind, ModelManifest, ModelSource, ModelSourceExt};
use q_source::role::TensorRole;
use q_source::{TensorDescriptor, TensorId};
use std::collections::BTreeMap;

/// Rough resident size of one descriptor, used to enforce the ingest budget.
/// Two heap strings (raw + canonical name) plus a small shape vector plus the
/// fixed struct.
const APPROX_DESCRIPTOR_BYTES: u64 = 256;

/// Result of one metadata ingestion pass.
#[derive(Debug, Clone)]
pub struct IngestOutcome {
    pub model_id: ModelId,
    pub manifest: ModelManifest,
    /// Descriptors with `canonical_name == raw_name` and role `Unknown`;
    /// `q-nsir` fills those in.
    pub descriptors: Vec<TensorDescriptor>,
    /// Shard index if the checkpoint is sharded; `None` for single-file.
    pub shard_index: Option<ShardIndex>,
    pub resume: ResumePoint,
    /// Total payload bytes described. Nothing of this size was read.
    pub described_payload_bytes: u64,
    /// Bytes actually read from disk during ingestion (headers only).
    pub bytes_read: u64,
}

impl IngestOutcome {
    pub fn tensor_count(&self) -> usize {
        self.descriptors.len()
    }

    /// Summed element count over every descriptor.
    ///
    /// This is the only honest way to get it. Dividing
    /// `described_payload_bytes` by a single dtype width assumes a uniform
    /// checkpoint, and real ones are mixed — `tiny-llama-2shard` alone is F32
    /// with two BF16 tensors, so the shortcut is wrong by 3 072 elements.
    /// Config arithmetic over `hidden_size` and `num_hidden_layers` is an
    /// estimate for the same reason and worse ones.
    pub fn total_parameters(&self) -> u64 {
        self.descriptors.iter().map(|d| d.element_count()).sum()
    }

    /// How many distinct shards the descriptors were drawn from.
    ///
    /// Counted over the descriptors rather than the manifest so that a resumed
    /// pass reports the shards it actually described, not the shards that
    /// exist on disk.
    pub fn shard_count(&self) -> usize {
        self.descriptors
            .iter()
            .map(|d| d.shard_uri.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }

    pub fn find(&self, raw_name: &str) -> Option<&TensorDescriptor> {
        self.descriptors.iter().find(|d| d.raw_name == raw_name)
    }
}

/// Reads checkpoint metadata from any [`ModelSource`].
pub struct CheckpointIngestor<'a> {
    source: &'a dyn ModelSource,
    cancel: CancellationToken,
    resume: ResumePoint,
    budget: MemoryBudget,
}

impl<'a> CheckpointIngestor<'a> {
    pub fn new(source: &'a dyn ModelSource) -> Self {
        Self {
            source,
            cancel: CancellationToken::new(),
            resume: ResumePoint::new(),
            budget: MemoryBudget::ingest_metadata(),
        }
    }

    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancel = token;
        self
    }

    /// Resume a previously interrupted pass, skipping already-completed shards.
    pub fn with_resume(mut self, resume: ResumePoint) -> Self {
        self.resume = resume;
        self
    }

    pub fn with_budget(mut self, budget: MemoryBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Run one metadata ingestion pass.
    pub fn ingest(mut self) -> Result<IngestOutcome> {
        let manifest = self.source.manifest()?;
        let model_id = manifest.model_id();
        let mut bytes_read = 0u64;

        let shard_index = match manifest.shard_index() {
            Some(f) => {
                let raw = self.source.read_range_buffered(
                    &f.uri,
                    0,
                    f.length,
                    &MemoryBudget::new("shard_index", MAX_INGEST_METADATA_BYTES),
                )?;
                bytes_read += raw.len() as u64;
                let idx = ShardIndex::parse(&f.uri, &raw)?;
                let available: Vec<String> = manifest.shards().map(|s| s.uri.clone()).collect();
                idx.verify_shards_present(&available, &manifest.root_uri)?;
                Some(idx)
            }
            None => None,
        };

        // Shard order: index order when sharded, manifest order otherwise. Both
        // are deterministic, so descriptor order (and therefore catalog
        // insertion order) is reproducible across runs.
        let shard_uris: Vec<String> = match &shard_index {
            Some(idx) => idx.shard_files(),
            None => manifest.shards().map(|s| s.uri.clone()).collect(),
        };
        if shard_uris.is_empty() {
            return Err(QError::malformed(
                &manifest.root_uri,
                "no .safetensors shards found",
            ));
        }

        let mut descriptors: Vec<TensorDescriptor> = Vec::new();
        let mut first_seen: BTreeMap<String, String> = BTreeMap::new();
        let mut described_payload_bytes = 0u64;

        // True when this pass skipped a shard because a prior pass finished it.
        // The index-completeness check below only holds for a full pass.
        let mut skipped_completed_shard = false;

        for uri in &shard_uris {
            if self.resume.is_complete(uri) {
                skipped_completed_shard = true;
                continue;
            }
            if self.cancel.is_cancelled() {
                self.resume.mark_interrupted(uri);
                return Err(QError::Cancelled {
                    checkpoint: uri.clone(),
                });
            }

            let file = manifest.file(uri).ok_or_else(|| QError::MissingShard {
                shard: uri.clone(),
                root: manifest.root_uri.clone(),
            })?;
            let header = SafeTensorsHeader::read_from(self.source, uri, file.length)?;
            bytes_read += 8 + header.header_length;

            self.budget.check(
                (descriptors.len() as u64 + header.tensor_count() as u64) * APPROX_DESCRIPTOR_BYTES,
            )?;

            for (name, entry) in &header.tensors {
                if let Some(prev) = first_seen.get(name) {
                    return Err(QError::DuplicateTensorName {
                        name: name.clone(),
                        first_uri: prev.clone(),
                        second_uri: uri.clone(),
                    });
                }
                first_seen.insert(name.clone(), uri.clone());

                // If an index exists, it is authoritative about placement; a
                // header that disagrees with it means the checkpoint is
                // internally inconsistent.
                if let Some(idx) = &shard_index {
                    match idx.shard_for(name) {
                        Some(expected) if expected == uri => {}
                        Some(expected) => {
                            return Err(QError::malformed(
                                uri,
                                format!(
                                "tensor {name} is in {uri} but the index places it in {expected}"
                            ),
                            ))
                        }
                        None => {
                            return Err(QError::malformed(
                                uri,
                                format!(
                                "tensor {name} is present in the shard but absent from the index"
                            ),
                            ))
                        }
                    }
                }

                let (byte_start, byte_end) = header.absolute_range(entry);
                let descriptor = TensorDescriptor {
                    tensor_id: TensorId::derive(model_id, name),
                    raw_name: name.clone(),
                    canonical_name: name.clone(),
                    shape: entry.shape.clone(),
                    dtype: entry.parsed_dtype()?,
                    shard_uri: uri.clone(),
                    byte_start,
                    byte_end,
                    layer_index: None,
                    semantic_role: TensorRole::Unknown,
                };
                descriptor.validate()?;
                described_payload_bytes += descriptor.byte_length();
                descriptors.push(descriptor);
            }

            self.resume.mark_complete(uri);
        }

        // Every tensor the index promised must have been found — but only a
        // full pass sees every shard. A resumed pass legitimately omits the
        // tensors of shards a prior pass already persisted.
        if let (Some(idx), false) = (&shard_index, skipped_completed_shard) {
            for name in idx.weight_map.keys() {
                if !first_seen.contains_key(name) {
                    return Err(QError::malformed(
                        &manifest.root_uri,
                        format!("index lists tensor {name} but no shard header declares it"),
                    ));
                }
            }
        }

        Ok(IngestOutcome {
            model_id,
            manifest,
            descriptors,
            shard_index,
            resume: self.resume,
            described_payload_bytes,
            bytes_read,
        })
    }
}

/// Convenience: ingest metadata from a local directory.
pub fn ingest_local(dir: impl AsRef<std::path::Path>) -> Result<IngestOutcome> {
    let src = q_source::LocalFsSource::open(dir)?;
    CheckpointIngestor::new(&src).ingest()
}

/// Single-file checkpoints have no index; this is just a manifest shape check.
pub fn is_single_file(manifest: &ModelManifest) -> bool {
    manifest.shard_index().is_none()
        && manifest
            .files
            .iter()
            .filter(|f| f.kind == ArtifactKind::SafeTensorsShard)
            .count()
            == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn fixtures(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .canonicalize()
            .expect("run fixtures/generate_fixtures.py")
    }

    /// `fixtures/tiny-llama-2shard/golden.json` — reference totals read back
    /// with the official Python `safetensors` library, checked in so the Rust
    /// suite stays hermetic.
    fn golden() -> serde_json::Value {
        let text = std::fs::read_to_string(fixtures("tiny-llama-2shard").join("golden.json"))
            .expect("run fixtures/generate_fixtures.py");
        serde_json::from_str(&text).unwrap()
    }

    /// One valid single-tensor shard plus whatever `config.json` text is given.
    fn checkpoint_with_config(config: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let mut bytes = (json.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(json.as_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        std::fs::write(dir.path().join("model.safetensors"), &bytes).unwrap();
        if let Some(text) = config {
            std::fs::write(dir.path().join("config.json"), text).unwrap();
        }
        dir
    }

    #[test]
    fn ingests_a_sharded_checkpoint() {
        let out = ingest_local(fixtures("tiny-llama-2shard")).unwrap();
        assert_eq!(out.tensor_count(), 111);
        assert_eq!(out.shard_index.as_ref().unwrap().shard_files().len(), 2);
        assert_eq!(out.described_payload_bytes, 1_196_736);
        let q10 = out.find("model.layers.10.self_attn.q_proj.weight").unwrap();
        assert_eq!(q10.shape, vec![128, 48]);
        assert_eq!(q10.dtype, q_source::DType::F32);
        assert_eq!(q10.shard_uri, "model-00002-of-00002.safetensors");
    }

    #[test]
    fn ingests_a_single_file_checkpoint() {
        let out = ingest_local(fixtures("tiny-llama-single")).unwrap();
        assert!(out.shard_index.is_none());
        assert!(is_single_file(&out.manifest));
        assert!(out.find("model.layers.0.self_attn.q_proj.weight").is_some());
        assert!(out
            .find("model.layers.10.self_attn.q_proj.weight")
            .is_none());
    }

    #[test]
    fn ingestion_reads_only_headers_not_payload() {
        let out = ingest_local(fixtures("tiny-llama-2shard")).unwrap();
        // Headers + index only. The payload is 1.19 MB; we read far less.
        assert!(
            out.bytes_read < out.described_payload_bytes / 10,
            "read {} bytes to describe {} payload bytes",
            out.bytes_read,
            out.described_payload_bytes
        );
    }

    #[test]
    fn described_totals_match_the_golden_file() {
        let out = ingest_local(fixtures("tiny-llama-2shard")).unwrap();
        let g = golden();
        assert_eq!(
            out.tensor_count() as u64,
            g["tensor_count"].as_u64().unwrap()
        );
        assert_eq!(
            out.described_payload_bytes,
            g["total_size_bytes"].as_u64().unwrap()
        );
        assert_eq!(out.shard_count() as u64, g["shard_count"].as_u64().unwrap());
    }

    #[test]
    fn total_parameters_is_the_summed_element_count_not_bytes_divided_by_a_uniform_width() {
        let out = ingest_local(fixtures("tiny-llama-2shard")).unwrap();
        // 302 256, computed independently by reading the fixture with Python
        // `safetensors==0.8.0` (command and output recorded in
        // `.plan/evidence/QM-0012.md` § Research).
        assert_eq!(out.total_parameters(), 302_256);
        // Deliberately *not* described_payload_bytes / 4: two tensors are BF16,
        // so a uniform-width shortcut is wrong by 3 072 elements. Summing the
        // descriptors is the only way to get this right.
        assert_eq!(out.described_payload_bytes / 4, 299_184);
        assert_ne!(out.total_parameters(), out.described_payload_bytes / 4);
    }

    #[test]
    fn shard_count_counts_the_distinct_shards_described() {
        assert_eq!(
            ingest_local(fixtures("tiny-llama-2shard"))
                .unwrap()
                .shard_count(),
            2
        );
        assert_eq!(
            ingest_local(fixtures("tiny-llama-single"))
                .unwrap()
                .shard_count(),
            1
        );
    }

    #[test]
    fn a_checkpoint_without_a_config_json_still_ingests() {
        let dir = checkpoint_with_config(None);
        let out = ingest_local(dir.path()).unwrap();
        assert_eq!(out.tensor_count(), 1);
        assert!(out.manifest.config.is_none());
        // Absent, not zero.
        assert_eq!(out.manifest.config_u64("hidden_size"), None);
        assert_eq!(out.manifest.model_type(), None);
    }

    #[test]
    fn a_corrupt_config_json_is_rejected_with_context() {
        let dir = checkpoint_with_config(Some("{ not json"));
        let err = ingest_local(dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("config.json"), "message lacked context: {msg}");
        assert!(matches!(err, QError::Json { .. }), "{err:?}");
    }

    #[test]
    fn a_config_field_of_the_wrong_type_does_not_fail_ingestion() {
        let dir =
            checkpoint_with_config(Some(r#"{"hidden_size": "big", "num_hidden_layers": 12}"#));
        let out = ingest_local(dir.path()).unwrap();
        assert_eq!(out.tensor_count(), 1);
        assert_eq!(out.manifest.config_u64("hidden_size"), None);
        assert_eq!(out.manifest.config_u64("num_hidden_layers"), Some(12));
    }

    #[test]
    fn tensor_ids_are_stable_across_reopen() {
        let a = ingest_local(fixtures("tiny-llama-2shard")).unwrap();
        let b = ingest_local(fixtures("tiny-llama-2shard")).unwrap();
        assert_eq!(a.model_id, b.model_id);
        for (x, y) in a.descriptors.iter().zip(b.descriptors.iter()) {
            assert_eq!(x.tensor_id, y.tensor_id);
            assert_eq!(x.raw_name, y.raw_name);
        }
    }

    #[test]
    fn bf16_tensors_are_described_with_the_right_width() {
        let out = ingest_local(fixtures("tiny-llama-2shard")).unwrap();
        let d = out.find("model.layers.0.mlp.gate_proj.weight").unwrap();
        assert_eq!(d.dtype, q_source::DType::BF16);
        assert_eq!(d.shape, vec![64, 48]);
        assert_eq!(d.byte_length(), 64 * 48 * 2);
    }

    #[test]
    fn cancellation_stops_at_a_shard_boundary() {
        let src = q_source::LocalFsSource::open(fixtures("tiny-llama-2shard")).unwrap();
        let token = CancellationToken::new();
        token.cancel();
        let err = CheckpointIngestor::new(&src)
            .with_cancellation(token)
            .ingest()
            .unwrap_err();
        assert!(matches!(err, QError::Cancelled { .. }));
    }

    #[test]
    fn resume_skips_completed_shards() {
        let src = q_source::LocalFsSource::open(fixtures("tiny-llama-2shard")).unwrap();
        let mut resume = ResumePoint::new();
        resume.mark_complete("model-00001-of-00002.safetensors");
        let out = CheckpointIngestor::new(&src)
            .with_resume(resume)
            .ingest()
            .unwrap();
        // Only the second shard's tensors were re-read.
        assert!(out
            .descriptors
            .iter()
            .all(|d| d.shard_uri == "model-00002-of-00002.safetensors"));
        assert!(out
            .find("model.layers.10.self_attn.q_proj.weight")
            .is_some());
        assert!(out.find("model.layers.0.self_attn.q_proj.weight").is_none());
    }

    #[test]
    fn a_tight_metadata_budget_is_enforced() {
        let src = q_source::LocalFsSource::open(fixtures("tiny-llama-2shard")).unwrap();
        let err = CheckpointIngestor::new(&src)
            .with_budget(MemoryBudget::new("tiny", 1024))
            .ingest()
            .unwrap_err();
        assert!(matches!(err, QError::BudgetExceeded { .. }));
    }

    #[test]
    fn missing_shard_named_by_the_index_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("model.safetensors.index.json"),
            br#"{"weight_map":{"a":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}}"#,
        )
        .unwrap();
        // Only shard 1 exists.
        let json = r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let mut bytes = (json.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(json.as_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        std::fs::write(dir.path().join("model-00001-of-00002.safetensors"), &bytes).unwrap();

        let src = q_source::LocalFsSource::open(dir.path()).unwrap();
        assert!(matches!(
            CheckpointIngestor::new(&src).ingest(),
            Err(QError::MissingShard { .. })
        ));
    }

    #[test]
    fn duplicate_tensor_across_shards_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let mut bytes = (json.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(json.as_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        for shard in [
            "model-00001-of-00002.safetensors",
            "model-00002-of-00002.safetensors",
        ] {
            std::fs::write(dir.path().join(shard), &bytes).unwrap();
        }
        // No index: both shards are scanned and both declare "a".
        let src = q_source::LocalFsSource::open(dir.path()).unwrap();
        assert!(matches!(
            CheckpointIngestor::new(&src).ingest(),
            Err(QError::DuplicateTensorName { .. })
        ));
    }

    #[test]
    fn directory_without_shards_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.json"), b"{}").unwrap();
        let src = q_source::LocalFsSource::open(dir.path()).unwrap();
        assert!(matches!(
            CheckpointIngestor::new(&src).ingest(),
            Err(QError::MalformedArtifact { .. })
        ));
    }
}
