//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1, §4.1).
//!
//! `model.safetensors.index.json` — the shard index.
//!
//! The index maps each tensor name to the shard file that holds it. It is the
//! only thing that must be read in full to know the layout of a sharded
//! checkpoint, and it is small: one short string pair per tensor. A 64-shard,
//! trillion-parameter checkpoint has an index measured in megabytes, not
//! terabytes.

use q_source::error::{QError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShardIndexMetadata {
    #[serde(default)]
    pub total_size: Option<u64>,
}

/// Parsed `model.safetensors.index.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShardIndex {
    #[serde(default)]
    pub metadata: ShardIndexMetadata,
    /// tensor name -> shard file name
    pub weight_map: BTreeMap<String, String>,
}

impl ShardIndex {
    pub fn parse(uri: &str, bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(|e| QError::json(uri.to_string(), e))
    }

    /// Distinct shard file names, in sorted order.
    pub fn shard_files(&self) -> Vec<String> {
        let mut v: Vec<String> = self.weight_map.values().cloned().collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn shard_for(&self, tensor_name: &str) -> Option<&str> {
        self.weight_map.get(tensor_name).map(String::as_str)
    }

    pub fn tensor_count(&self) -> usize {
        self.weight_map.len()
    }

    /// Verify every shard the index names is actually present.
    ///
    /// A missing shard is reported as [`QError::MissingShard`] rather than
    /// discovered later as a confusing IO error at query time.
    pub fn verify_shards_present(&self, available: &[String], root: &str) -> Result<()> {
        for shard in self.shard_files() {
            if !available.iter().any(|a| a == &shard) {
                return Err(QError::MissingShard {
                    shard,
                    root: root.to_string(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "metadata": {"total_size": 1196736},
      "weight_map": {
        "model.embed_tokens.weight": "model-00001-of-00002.safetensors",
        "model.layers.10.self_attn.q_proj.weight": "model-00002-of-00002.safetensors",
        "lm_head.weight": "model-00002-of-00002.safetensors"
      }
    }"#;

    #[test]
    fn parses_weight_map_and_total_size() {
        let idx = ShardIndex::parse("index", SAMPLE.as_bytes()).unwrap();
        assert_eq!(idx.tensor_count(), 3);
        assert_eq!(idx.metadata.total_size, Some(1196736));
        assert_eq!(
            idx.shard_for("model.layers.10.self_attn.q_proj.weight"),
            Some("model-00002-of-00002.safetensors")
        );
        assert_eq!(idx.shard_for("nope"), None);
    }

    #[test]
    fn shard_files_are_deduplicated_and_sorted() {
        let idx = ShardIndex::parse("index", SAMPLE.as_bytes()).unwrap();
        assert_eq!(
            idx.shard_files(),
            vec![
                "model-00001-of-00002.safetensors".to_string(),
                "model-00002-of-00002.safetensors".to_string()
            ]
        );
    }

    #[test]
    fn missing_shard_is_named_explicitly() {
        let idx = ShardIndex::parse("index", SAMPLE.as_bytes()).unwrap();
        let err = idx
            .verify_shards_present(&["model-00001-of-00002.safetensors".into()], "/models/x")
            .unwrap_err();
        match err {
            QError::MissingShard { shard, .. } => {
                assert_eq!(shard, "model-00002-of-00002.safetensors")
            }
            other => panic!("expected MissingShard, got {other:?}"),
        }
    }

    #[test]
    fn index_without_metadata_still_parses() {
        let idx = ShardIndex::parse("i", br#"{"weight_map":{"a":"s.safetensors"}}"#).unwrap();
        assert_eq!(idx.metadata.total_size, None);
        assert_eq!(idx.tensor_count(), 1);
    }

    #[test]
    fn malformed_index_is_rejected_with_context() {
        let err = ShardIndex::parse("model.safetensors.index.json", b"{ nope").unwrap_err();
        assert!(err.to_string().contains("model.safetensors.index.json"));
    }
}
