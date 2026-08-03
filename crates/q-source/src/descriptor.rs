//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §4.1, §5.2).
//!
//! [`TensorDescriptor`] — the metadata record for one tensor.
//!
//! This is the type ARCHITECTURE.md §4.1 specifies verbatim. It is small and
//! fixed-size-ish (~200 bytes plus two strings) regardless of how large the
//! tensor it describes is: a 4096×4096 tensor and a 1×1 tensor produce
//! descriptors of the same order of magnitude. That property is what makes
//! trillion-parameter *metadata* tractable while the payload stays on disk.

use crate::dtype::DType;
use crate::error::{QError, Result};
use crate::ids::TensorId;
use crate::role::TensorRole;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TensorDescriptor {
    pub tensor_id: TensorId,
    /// The name exactly as it appears in the SafeTensors header.
    pub raw_name: String,
    /// The NSIR canonical name, e.g.
    /// `model.layers[10].self_attention.query_projection.weight`.
    /// Equal to `raw_name` when no resolver claimed it.
    pub canonical_name: String,
    pub shape: Vec<u64>,
    pub dtype: DType,
    /// Which artifact file holds the payload.
    pub shard_uri: String,
    /// Absolute offset of the first payload byte within `shard_uri`.
    pub byte_start: u64,
    /// Absolute offset one past the last payload byte.
    pub byte_end: u64,
    pub layer_index: Option<u32>,
    pub semantic_role: TensorRole,
}

impl TensorDescriptor {
    pub fn element_count(&self) -> u64 {
        self.shape.iter().copied().product::<u64>().max(
            // A 0-d tensor holds exactly one element; an empty shape is not an
            // empty tensor.
            if self.shape.is_empty() { 1 } else { 0 },
        )
    }

    pub fn byte_length(&self) -> u64 {
        self.byte_end.saturating_sub(self.byte_start)
    }

    /// Check that the declared byte range matches shape × dtype width.
    pub fn validate(&self) -> Result<()> {
        if self.byte_end < self.byte_start {
            return Err(QError::malformed(
                &self.shard_uri,
                format!(
                    "tensor {}: byte_end {} < byte_start {}",
                    self.raw_name, self.byte_end, self.byte_start
                ),
            ));
        }
        let expected = self
            .element_count()
            .checked_mul(self.dtype.size_in_bytes())
            .ok_or_else(|| {
                QError::malformed(
                    &self.shard_uri,
                    format!("tensor {}: shape overflows u64", self.raw_name),
                )
            })?;
        if expected != self.byte_length() {
            return Err(QError::malformed(
                &self.shard_uri,
                format!(
                    "tensor {}: shape {:?} × {} bytes = {expected}, but header declares {} bytes",
                    self.raw_name,
                    self.shape,
                    self.dtype.size_in_bytes(),
                    self.byte_length()
                ),
            ));
        }
        Ok(())
    }

    /// Row-major linear element offset for a logical index.
    ///
    /// SafeTensors stores tensors in C (row-major) order, so the last axis is
    /// contiguous. Returns [`QError::IndexOutOfBounds`] rather than wrapping.
    pub fn linear_index(&self, index: &[u64]) -> Result<u64> {
        if index.len() != self.shape.len() {
            return Err(QError::IndexOutOfBounds {
                tensor: self.canonical_name.clone(),
                index: index.to_vec(),
                shape: self.shape.clone(),
            });
        }
        let mut linear = 0u64;
        for (i, (&idx, &dim)) in index.iter().zip(self.shape.iter()).enumerate() {
            if idx >= dim {
                return Err(QError::IndexOutOfBounds {
                    tensor: self.canonical_name.clone(),
                    index: index.to_vec(),
                    shape: self.shape.clone(),
                });
            }
            let stride: u64 = self.shape[i + 1..].iter().product();
            linear += idx * stride;
        }
        Ok(linear)
    }

    /// Absolute byte offset of one element within its shard.
    pub fn element_byte_offset(&self, index: &[u64]) -> Result<u64> {
        let linear = self.linear_index(index)?;
        Ok(self.byte_start + linear * self.dtype.size_in_bytes())
    }

    /// Absolute byte range of one contiguous run of `count` elements starting
    /// at `index`. The run must not cross the end of the tensor.
    pub fn element_run_range(&self, index: &[u64], count: u64) -> Result<(u64, u64)> {
        let start = self.element_byte_offset(index)?;
        let end = start + count * self.dtype.size_in_bytes();
        if end > self.byte_end {
            return Err(QError::RangeOutOfBounds {
                uri: self.shard_uri.clone(),
                start,
                end,
                length: self.byte_end,
            });
        }
        Ok((start, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ModelId;

    fn desc(shape: Vec<u64>, dtype: DType) -> TensorDescriptor {
        let n: u64 = shape.iter().product();
        TensorDescriptor {
            tensor_id: TensorId::derive(ModelId::derive("m", "", "f"), "t"),
            raw_name: "t".into(),
            canonical_name: "t".into(),
            byte_start: 1000,
            byte_end: 1000 + n * dtype.size_in_bytes(),
            shape,
            dtype,
            shard_uri: "s.safetensors".into(),
            layer_index: None,
            semantic_role: TensorRole::Unknown,
        }
    }

    #[test]
    fn row_major_linear_index() {
        let d = desc(vec![128, 48], DType::F32);
        assert_eq!(d.linear_index(&[0, 0]).unwrap(), 0);
        assert_eq!(d.linear_index(&[0, 1]).unwrap(), 1);
        assert_eq!(d.linear_index(&[1, 0]).unwrap(), 48);
        assert_eq!(d.linear_index(&[100, 42]).unwrap(), 100 * 48 + 42);
    }

    #[test]
    fn element_offset_accounts_for_dtype_width_and_shard_base() {
        let d = desc(vec![128, 48], DType::F32);
        assert_eq!(
            d.element_byte_offset(&[100, 42]).unwrap(),
            1000 + (100 * 48 + 42) * 4
        );
        let d16 = desc(vec![128, 48], DType::BF16);
        assert_eq!(
            d16.element_byte_offset(&[100, 42]).unwrap(),
            1000 + (100 * 48 + 42) * 2
        );
    }

    #[test]
    fn out_of_bounds_index_is_rejected_not_wrapped() {
        let d = desc(vec![128, 48], DType::F32);
        assert!(matches!(
            d.linear_index(&[128, 0]),
            Err(QError::IndexOutOfBounds { .. })
        ));
        assert!(matches!(
            d.linear_index(&[0, 48]),
            Err(QError::IndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn wrong_rank_is_rejected() {
        let d = desc(vec![128, 48], DType::F32);
        assert!(d.linear_index(&[100]).is_err());
        assert!(d.linear_index(&[1, 2, 3]).is_err());
    }

    #[test]
    fn validate_catches_shape_byte_range_mismatch() {
        let mut d = desc(vec![128, 48], DType::F32);
        assert!(d.validate().is_ok());
        d.byte_end -= 4;
        assert!(matches!(d.validate(), Err(QError::MalformedArtifact { .. })));
    }

    #[test]
    fn run_that_overruns_the_tensor_is_rejected() {
        let d = desc(vec![4, 4], DType::F32);
        assert!(d.element_run_range(&[3, 0], 4).is_ok());
        assert!(matches!(
            d.element_run_range(&[3, 1], 4),
            Err(QError::RangeOutOfBounds { .. })
        ));
    }

    #[test]
    fn one_dimensional_tensor_indexes_directly() {
        let d = desc(vec![48], DType::F32);
        assert_eq!(d.linear_index(&[7]).unwrap(), 7);
        assert_eq!(d.element_byte_offset(&[7]).unwrap(), 1000 + 28);
    }
}
