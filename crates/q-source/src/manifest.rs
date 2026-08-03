//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1, §4.1).
//!
//! The `ModelSource` trait and the manifest it produces.
//!
//! A manifest lists artifact *files* — names, lengths, kinds — and never their
//! contents. Opening a 600 GB checkpoint produces a manifest of a few kilobytes.

use crate::budget::MemoryBudget;
use crate::error::{QError, Result};
use crate::ids::{content_fingerprint, ModelId};
use serde::{Deserialize, Serialize};
use std::io::Read;

/// What an artifact file is, decided by name, not by opening it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactKind {
    /// `model.safetensors` or `model-NNNNN-of-MMMMM.safetensors`
    SafeTensorsShard,
    /// `model.safetensors.index.json`
    ShardIndex,
    /// `config.json`
    Config,
    /// `tokenizer.json`, `tokenizer_config.json`, …
    Tokenizer,
    Other,
}

impl ArtifactKind {
    /// Classify by file name alone.
    pub fn classify(file_name: &str) -> ArtifactKind {
        if file_name == "model.safetensors.index.json" || file_name.ends_with(".index.json") {
            ArtifactKind::ShardIndex
        } else if file_name.ends_with(".safetensors") {
            ArtifactKind::SafeTensorsShard
        } else if file_name == "config.json" {
            ArtifactKind::Config
        } else if file_name.starts_with("tokenizer") || file_name == "vocab.json" {
            ArtifactKind::Tokenizer
        } else {
            ArtifactKind::Other
        }
    }
}

/// One artifact file. `length` comes from directory metadata, not from reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFile {
    /// URI relative to the manifest root, e.g. `model-00002-of-00002.safetensors`.
    pub uri: String,
    pub length: u64,
    pub kind: ArtifactKind,
}

/// The immutable artifact set backing one model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelManifest {
    /// Stable logical name, e.g. `local:tiny-llama-2shard` or `hf:org/model`.
    pub source_key: String,
    /// Where the files live (a directory path or a base URL).
    pub root_uri: String,
    /// Revision/commit if the source has one; empty for plain local directories.
    pub revision: String,
    pub files: Vec<SourceFile>,
    /// Parsed `config.json` if present. Kept as raw JSON: architecture
    /// resolvers read what they need and the rest is preserved verbatim.
    pub config: Option<serde_json::Value>,
}

impl ModelManifest {
    pub fn shards(&self) -> impl Iterator<Item = &SourceFile> {
        self.files
            .iter()
            .filter(|f| f.kind == ArtifactKind::SafeTensorsShard)
    }

    pub fn shard_index(&self) -> Option<&SourceFile> {
        self.files.iter().find(|f| f.kind == ArtifactKind::ShardIndex)
    }

    pub fn file(&self, uri: &str) -> Option<&SourceFile> {
        self.files.iter().find(|f| f.uri == uri)
    }

    /// Cheap fingerprint over file names and lengths — see
    /// [`crate::ids::content_fingerprint`] for what this does and does not
    /// detect.
    pub fn fingerprint(&self) -> String {
        let pairs: Vec<(String, u64)> = self
            .files
            .iter()
            .map(|f| (f.uri.clone(), f.length))
            .collect();
        content_fingerprint(&pairs)
    }

    pub fn model_id(&self) -> ModelId {
        ModelId::derive(&self.source_key, &self.revision, &self.fingerprint())
    }

    /// `architectures[0]` from `config.json`, if present.
    pub fn declared_architecture(&self) -> Option<String> {
        self.config
            .as_ref()?
            .get("architectures")?
            .as_array()?
            .first()?
            .as_str()
            .map(str::to_string)
    }

    pub fn model_type(&self) -> Option<String> {
        self.config
            .as_ref()?
            .get("model_type")?
            .as_str()
            .map(str::to_string)
    }

    pub fn config_u64(&self, key: &str) -> Option<u64> {
        self.config.as_ref()?.get(key)?.as_u64()
    }
}

/// A bounded reader over one byte range of one artifact.
///
/// This is the `ByteStream` of ARCHITECTURE.md §4.1. It is a *stream*, not a
/// buffer, so that the caller decides how much to materialize;
/// [`ByteStream::read_all_within_budget`] is the only way to get a `Vec` and it
/// requires an explicit [`MemoryBudget`].
pub struct ByteStream {
    len: u64,
    inner: Box<dyn Read + Send>,
}

impl std::fmt::Debug for ByteStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ByteStream").field("len", &self.len).finish()
    }
}

impl ByteStream {
    pub fn new(len: u64, inner: Box<dyn Read + Send>) -> Self {
        Self { len, inner }
    }

    pub fn from_vec(v: Vec<u8>) -> Self {
        Self {
            len: v.len() as u64,
            inner: Box::new(std::io::Cursor::new(v)),
        }
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Materialize the whole range, but only if the budget allows it.
    pub fn read_all_within_budget(mut self, budget: &MemoryBudget) -> Result<Vec<u8>> {
        budget.check(self.len)?;
        let mut buf = Vec::with_capacity(self.len as usize);
        self.inner.read_to_end(&mut buf)?;
        if buf.len() as u64 != self.len {
            return Err(QError::malformed(
                "byte stream",
                format!("expected {} bytes, read {}", self.len, buf.len()),
            ));
        }
        Ok(buf)
    }

    /// Stream to a writer using a fixed-size chunk, regardless of range size.
    pub fn copy_to(mut self, w: &mut dyn std::io::Write) -> Result<u64> {
        let mut chunk = vec![0u8; crate::budget::STREAM_CHUNK_BYTES];
        let mut total = 0u64;
        loop {
            let n = self.inner.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            w.write_all(&chunk[..n])?;
            total += n as u64;
        }
        Ok(total)
    }
}

impl Read for ByteStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

/// An immutable checkpoint source (ARCHITECTURE.md §4.1).
///
/// Implementations must be lazy: `manifest` never reads payload, and
/// `read_range` reads exactly the requested window and nothing more.
pub trait ModelSource: Send + Sync {
    fn manifest(&self) -> Result<ModelManifest>;

    fn read_range(&self, uri: &str, offset: u64, length: u64) -> Result<ByteStream>;
}

/// Convenience layered on any [`ModelSource`].
pub trait ModelSourceExt: ModelSource {
    /// Read exactly `length` bytes, refusing anything over `budget`.
    fn read_range_buffered(
        &self,
        uri: &str,
        offset: u64,
        length: u64,
        budget: &MemoryBudget,
    ) -> Result<Vec<u8>> {
        budget.check(length)?;
        self.read_range(uri, offset, length)?
            .read_all_within_budget(budget)
    }
}

impl<T: ModelSource + ?Sized> ModelSourceExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_kinds_are_classified_by_name() {
        assert_eq!(
            ArtifactKind::classify("model.safetensors.index.json"),
            ArtifactKind::ShardIndex
        );
        assert_eq!(
            ArtifactKind::classify("model-00002-of-00064.safetensors"),
            ArtifactKind::SafeTensorsShard
        );
        assert_eq!(ArtifactKind::classify("config.json"), ArtifactKind::Config);
        assert_eq!(
            ArtifactKind::classify("tokenizer.json"),
            ArtifactKind::Tokenizer
        );
        assert_eq!(ArtifactKind::classify("README.md"), ArtifactKind::Other);
    }

    #[test]
    fn byte_stream_refuses_to_exceed_budget() {
        let s = ByteStream::from_vec(vec![0u8; 4096]);
        let tight = MemoryBudget::new("tight", 1024);
        assert!(matches!(
            s.read_all_within_budget(&tight),
            Err(QError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn byte_stream_round_trips_within_budget() {
        let s = ByteStream::from_vec(vec![7u8; 16]);
        let out = s
            .read_all_within_budget(&MemoryBudget::new("ok", 1024))
            .unwrap();
        assert_eq!(out, vec![7u8; 16]);
    }
}
