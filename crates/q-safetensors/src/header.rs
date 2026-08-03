//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1, §4).
//!
//! SafeTensors header parsing.
//!
//! File layout:
//!
//! ```text
//! [0..8)        u64 little-endian  header_length (N)
//! [8..8+N)      UTF-8 JSON header
//! [8+N..)       data buffer; `data_offsets` are relative to this point
//! ```
//!
//! The header is the whole reason SafeTensors works for this project: it gives
//! name, dtype, shape, and byte offsets without touching a single weight. This
//! parser reads exactly `8 + N` bytes per shard and never more, so opening a
//! 64-shard, 600 GB checkpoint costs a few megabytes of JSON.
//!
//! `__metadata__` is a reserved JSON key holding free-form strings, **not** a
//! tensor. It is separated out here so nothing downstream counts it as one.

use q_source::budget::{MemoryBudget, MAX_HEADER_BYTES};
use q_source::error::{QError, Result};
use q_source::manifest::{ModelSource, ModelSourceExt};
use q_source::DType;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::fmt;

/// The reserved header key that is metadata, not a tensor.
pub const METADATA_KEY: &str = "__metadata__";

/// One tensor's entry in the header.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HeaderEntry {
    pub dtype: String,
    pub shape: Vec<u64>,
    /// `[start, end)` **relative to the start of the data buffer**.
    pub data_offsets: [u64; 2],
}

impl HeaderEntry {
    pub fn parsed_dtype(&self) -> Result<DType> {
        DType::parse_safetensors(&self.dtype)
    }

    pub fn element_count(&self) -> u64 {
        if self.shape.is_empty() {
            1
        } else {
            self.shape.iter().copied().product()
        }
    }

    pub fn declared_len(&self) -> u64 {
        self.data_offsets[1].saturating_sub(self.data_offsets[0])
    }
}

/// A parsed SafeTensors header.
#[derive(Debug, Clone, PartialEq)]
pub struct SafeTensorsHeader {
    /// Tensor entries in header order (SafeTensors headers are conventionally
    /// sorted, but order is preserved rather than assumed).
    pub tensors: Vec<(String, HeaderEntry)>,
    /// Contents of `__metadata__`, if present.
    pub metadata: BTreeMap<String, String>,
    /// Absolute file offset where the data buffer begins (`8 + N`).
    pub data_offset: u64,
    /// The header's own JSON length, `N`.
    pub header_length: u64,
}

impl SafeTensorsHeader {
    pub fn get(&self, name: &str) -> Option<&HeaderEntry> {
        self.tensors.iter().find(|(n, _)| n == name).map(|(_, e)| e)
    }

    pub fn tensor_count(&self) -> usize {
        self.tensors.len()
    }

    /// Absolute file range of a tensor's payload.
    pub fn absolute_range(&self, entry: &HeaderEntry) -> (u64, u64) {
        (
            self.data_offset + entry.data_offsets[0],
            self.data_offset + entry.data_offsets[1],
        )
    }

    /// Parse from the leading bytes of a file.
    ///
    /// `bytes` must contain at least `8 + N`; `file_length` is the total file
    /// size, used to validate that declared offsets fit.
    pub fn parse(uri: &str, bytes: &[u8], file_length: u64) -> Result<Self> {
        if bytes.len() < 8 {
            return Err(QError::malformed(
                uri,
                format!(
                    "file is {} bytes; a SafeTensors header needs at least 8",
                    bytes.len()
                ),
            ));
        }
        let header_length = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        if header_length > MAX_HEADER_BYTES {
            // Refuse *before* allocating. This is the bounded-memory contract
            // applied to a hostile or corrupt file.
            return Err(QError::BudgetExceeded {
                budget_name: "safetensors_header",
                requested: header_length,
                limit: MAX_HEADER_BYTES,
            });
        }
        let end = 8u64
            .checked_add(header_length)
            .ok_or_else(|| QError::malformed(uri, "header length overflows u64"))?;
        if end > file_length {
            return Err(QError::malformed(
                uri,
                format!("declared header length {header_length} exceeds file length {file_length}"),
            ));
        }
        if (bytes.len() as u64) < end {
            return Err(QError::malformed(
                uri,
                format!(
                    "only {} bytes supplied but the header needs {end}",
                    bytes.len()
                ),
            ));
        }

        let json = &bytes[8..end as usize];
        let raw: RawHeader =
            serde_json::from_slice(json).map_err(|e| QError::json(format!("{uri} header"), e))?;

        let mut tensors: Vec<(String, HeaderEntry)> = Vec::new();
        let mut metadata = BTreeMap::new();
        let mut seen: BTreeMap<String, ()> = BTreeMap::new();

        for (key, value) in raw.0 {
            if key == METADATA_KEY {
                if let serde_json::Value::Object(map) = value {
                    for (k, v) in map {
                        if let Some(s) = v.as_str() {
                            metadata.insert(k, s.to_string());
                        }
                    }
                }
                continue;
            }
            if seen.insert(key.clone(), ()).is_some() {
                return Err(QError::DuplicateTensorName {
                    name: key,
                    first_uri: uri.to_string(),
                    second_uri: uri.to_string(),
                });
            }
            let entry: HeaderEntry = serde_json::from_value(value)
                .map_err(|e| QError::json(format!("{uri} tensor {key}"), e))?;
            tensors.push((key, entry));
        }

        let header = SafeTensorsHeader {
            tensors,
            metadata,
            data_offset: end,
            header_length,
        };
        header.validate(uri, file_length)?;
        Ok(header)
    }

    /// Structural checks that do not require reading payload.
    fn validate(&self, uri: &str, file_length: u64) -> Result<()> {
        let data_len = file_length - self.data_offset;
        for (name, entry) in &self.tensors {
            let [start, end] = entry.data_offsets;
            if end < start {
                return Err(QError::malformed(
                    uri,
                    format!("tensor {name}: data_offsets [{start}, {end}] are inverted"),
                ));
            }
            if end > data_len {
                return Err(QError::RangeOutOfBounds {
                    uri: format!("{uri} (tensor {name})"),
                    start: self.data_offset + start,
                    end: self.data_offset + end,
                    length: file_length,
                });
            }
            let dtype = entry.parsed_dtype()?;
            let expected = entry
                .element_count()
                .checked_mul(dtype.size_in_bytes())
                .ok_or_else(|| {
                    QError::malformed(uri, format!("tensor {name}: shape overflows u64"))
                })?;
            if expected != entry.declared_len() {
                return Err(QError::malformed(
                    uri,
                    format!(
                        "tensor {name}: shape {:?} × {} bytes = {expected}, but data_offsets span {}",
                        entry.shape,
                        dtype.size_in_bytes(),
                        entry.declared_len()
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Read and parse a header from a [`ModelSource`], touching only the header
    /// bytes.
    ///
    /// Two reads: the 8-byte length prefix, then exactly `N` bytes of JSON.
    /// Payload is never read.
    pub fn read_from(source: &dyn ModelSource, uri: &str, file_length: u64) -> Result<Self> {
        let budget = MemoryBudget::header();
        let prefix = source.read_range_buffered(uri, 0, 8.min(file_length), &budget)?;
        if prefix.len() < 8 {
            return Err(QError::malformed(uri, "file too short for a header"));
        }
        let n = u64::from_le_bytes(prefix[..8].try_into().unwrap());
        if n > MAX_HEADER_BYTES {
            return Err(QError::BudgetExceeded {
                budget_name: "safetensors_header",
                requested: n,
                limit: MAX_HEADER_BYTES,
            });
        }
        if 8 + n > file_length {
            return Err(QError::malformed(
                uri,
                format!("declared header length {n} exceeds file length {file_length}"),
            ));
        }
        let json = source.read_range_buffered(uri, 8, n, &budget)?;
        let mut bytes = Vec::with_capacity(8 + json.len());
        bytes.extend_from_slice(&prefix);
        bytes.extend_from_slice(&json);
        Self::parse(uri, &bytes, file_length)
    }
}

/// Deserializes a JSON object *without* collapsing duplicate keys, so that a
/// checkpoint declaring the same tensor twice is detected rather than silently
/// last-write-wins.
struct RawHeader(Vec<(String, serde_json::Value)>);

impl<'de> Deserialize<'de> for RawHeader {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = RawHeader;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a SafeTensors header object")
            }

            fn visit_map<M: MapAccess<'de>>(
                self,
                mut m: M,
            ) -> std::result::Result<RawHeader, M::Error> {
                let mut out = Vec::with_capacity(m.size_hint().unwrap_or(16));
                while let Some((k, v)) = m.next_entry::<String, serde_json::Value>()? {
                    out.push((k, v));
                }
                Ok(RawHeader(out))
            }
        }
        d.deserialize_map(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(json: &str, data_len: usize) -> (Vec<u8>, u64) {
        let mut v = Vec::new();
        v.extend_from_slice(&(json.len() as u64).to_le_bytes());
        v.extend_from_slice(json.as_bytes());
        v.extend(std::iter::repeat(0u8).take(data_len));
        let len = v.len() as u64;
        (v, len)
    }

    #[test]
    fn parses_a_minimal_header() {
        let json = r#"{"a":{"dtype":"F32","shape":[2,2],"data_offsets":[0,16]}}"#;
        let (bytes, len) = build(json, 16);
        let h = SafeTensorsHeader::parse("t", &bytes, len).unwrap();
        assert_eq!(h.tensor_count(), 1);
        assert_eq!(h.data_offset, 8 + json.len() as u64);
        let e = h.get("a").unwrap();
        assert_eq!(e.parsed_dtype().unwrap(), DType::F32);
        assert_eq!(h.absolute_range(e), (h.data_offset, h.data_offset + 16));
    }

    #[test]
    fn metadata_key_is_not_counted_as_a_tensor() {
        let json = r#"{"__metadata__":{"format":"pt"},"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let (bytes, len) = build(json, 4);
        let h = SafeTensorsHeader::parse("t", &bytes, len).unwrap();
        assert_eq!(h.tensor_count(), 1);
        assert!(h.get(METADATA_KEY).is_none());
        assert_eq!(h.metadata.get("format").map(String::as_str), Some("pt"));
    }

    #[test]
    fn duplicate_tensor_name_is_rejected() {
        let json = r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]},"a":{"dtype":"F32","shape":[1],"data_offsets":[4,8]}}"#;
        let (bytes, len) = build(json, 8);
        assert!(matches!(
            SafeTensorsHeader::parse("t", &bytes, len),
            Err(QError::DuplicateTensorName { .. })
        ));
    }

    #[test]
    fn shape_dtype_byte_length_mismatch_is_rejected() {
        // [2,2] F32 needs 16 bytes, header claims 12.
        let json = r#"{"a":{"dtype":"F32","shape":[2,2],"data_offsets":[0,12]}}"#;
        let (bytes, len) = build(json, 12);
        let err = SafeTensorsHeader::parse("t", &bytes, len).unwrap_err();
        assert!(err.to_string().contains("data_offsets span"));
    }

    #[test]
    fn offsets_past_end_of_data_buffer_are_rejected() {
        let json = r#"{"a":{"dtype":"F32","shape":[2,2],"data_offsets":[0,16]}}"#;
        let (bytes, _) = build(json, 16);
        // Lie about the file length: claim the file ends before the payload.
        let short_len = 8 + json.len() as u64 + 8;
        assert!(matches!(
            SafeTensorsHeader::parse("t", &bytes, short_len),
            Err(QError::RangeOutOfBounds { .. })
        ));
    }

    #[test]
    fn absurd_header_length_is_refused_before_allocating() {
        let mut bytes = u64::MAX.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"{}");
        let err = SafeTensorsHeader::parse("t", &bytes, bytes.len() as u64).unwrap_err();
        assert!(matches!(err, QError::BudgetExceeded { .. }));
    }

    #[test]
    fn truncated_file_is_rejected() {
        assert!(SafeTensorsHeader::parse("t", &[0u8; 4], 4).is_err());
    }

    #[test]
    fn corrupt_json_is_rejected_with_context() {
        let json = r#"{"a":{"dtype":"F32","shape":[2,2],"data_offsets":[0,16]"#;
        let (bytes, len) = build(json, 16);
        let err = SafeTensorsHeader::parse("shard.safetensors", &bytes, len).unwrap_err();
        assert!(err.to_string().contains("shard.safetensors header"));
    }

    #[test]
    fn unsupported_dtype_is_rejected() {
        let json = r#"{"a":{"dtype":"F4_MYSTERY","shape":[2,2],"data_offsets":[0,16]}}"#;
        let (bytes, len) = build(json, 16);
        assert!(matches!(
            SafeTensorsHeader::parse("t", &bytes, len),
            Err(QError::UnsupportedDType { .. })
        ));
    }

    #[test]
    fn inverted_offsets_are_rejected() {
        let json = r#"{"a":{"dtype":"F32","shape":[2,2],"data_offsets":[16,0]}}"#;
        let (bytes, len) = build(json, 16);
        let err = SafeTensorsHeader::parse("t", &bytes, len).unwrap_err();
        assert!(err.to_string().contains("inverted"));
    }

    #[test]
    fn scalar_tensor_with_empty_shape_holds_one_element() {
        let json = r#"{"s":{"dtype":"F32","shape":[],"data_offsets":[0,4]}}"#;
        let (bytes, len) = build(json, 4);
        let h = SafeTensorsHeader::parse("t", &bytes, len).unwrap();
        assert_eq!(h.get("s").unwrap().element_count(), 1);
    }
}
