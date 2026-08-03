//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1, §13.3, §14.3).
//!
//! Exact scalar and slice reads by byte range.
//!
//! This is the "select or inspect → range-read exact bytes from SafeTensors"
//! step of ARCHITECTURE.md §9.3, and the read that ARCHITECTURE.md §18 AC-005
//! requires to match a Python `safetensors` reference.
//!
//! Every function here reads *only* the bytes its result needs:
//!
//! * [`read_scalar`] reads `dtype.size_in_bytes()` bytes. For an F32 tensor
//!   that is four bytes, no matter how large the tensor is.
//! * [`read_slice_2d`] reads one contiguous run per row. A 4×4 window of a
//!   4096×4096 tensor costs four 16-byte reads, not 64 MB.

use q_source::budget::{MemoryBudget, MAX_QUERY_RESULT_ELEMENTS};
use q_source::error::{QError, Result};
use q_source::manifest::{ModelSource, ModelSourceExt};
use q_source::{ResultFidelity, TensorDescriptor};
use serde::{Deserialize, Serialize};

/// One exactly-read scalar, carrying its provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalarRead {
    pub canonical_name: String,
    pub index: Vec<u64>,
    pub value: f64,
    /// Absolute byte offset the value came from — quotable in a report.
    pub byte_offset: u64,
    pub shard_uri: String,
    pub dtype: q_source::DType,
    pub fidelity: ResultFidelity,
    /// Bytes actually read to produce this value.
    pub bytes_read: u64,
}

/// A rectangular window of a 2-D tensor, read exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceRead {
    pub canonical_name: String,
    pub row_start: u64,
    pub row_end: u64,
    pub column_start: u64,
    pub column_end: u64,
    /// Row-major, `(row_end - row_start) * (column_end - column_start)` values.
    pub values: Vec<f64>,
    pub dtype: q_source::DType,
    pub fidelity: ResultFidelity,
    pub bytes_read: u64,
}

impl SliceRead {
    pub fn rows(&self) -> u64 {
        self.row_end - self.row_start
    }

    pub fn columns(&self) -> u64 {
        self.column_end - self.column_start
    }

    pub fn get(&self, i: u64, j: u64) -> Option<f64> {
        if i >= self.rows() || j >= self.columns() {
            return None;
        }
        self.values.get((i * self.columns() + j) as usize).copied()
    }
}

/// Read one scalar exactly.
pub fn read_scalar(
    source: &dyn ModelSource,
    descriptor: &TensorDescriptor,
    index: &[u64],
) -> Result<ScalarRead> {
    if !descriptor.dtype.supports_exact_scalar_read() {
        return Err(QError::UnsupportedDType {
            dtype: descriptor.dtype.as_safetensors_str().to_string(),
            operation: format!("exact scalar read of {}", descriptor.canonical_name),
        });
    }
    let offset = descriptor.element_byte_offset(index)?;
    let width = descriptor.dtype.size_in_bytes();
    let bytes = source.read_range_buffered(
        &descriptor.shard_uri,
        offset,
        width,
        &MemoryBudget::single_read(),
    )?;
    Ok(ScalarRead {
        canonical_name: descriptor.canonical_name.clone(),
        index: index.to_vec(),
        value: descriptor.dtype.decode_scalar(&bytes)?,
        byte_offset: offset,
        shard_uri: descriptor.shard_uri.clone(),
        dtype: descriptor.dtype,
        fidelity: ResultFidelity::Exact,
        bytes_read: width,
    })
}

/// Read a rectangular window of a 2-D tensor exactly.
///
/// Ranges are half-open: `rows = (0, 256)` is rows 0..255.
pub fn read_slice_2d(
    source: &dyn ModelSource,
    descriptor: &TensorDescriptor,
    rows: (u64, u64),
    columns: (u64, u64),
) -> Result<SliceRead> {
    if descriptor.shape.len() != 2 {
        return Err(QError::QueryRejected(format!(
            "{} has rank {}; 2-D slice requires rank 2",
            descriptor.canonical_name,
            descriptor.shape.len()
        )));
    }
    if !descriptor.dtype.supports_exact_scalar_read() {
        return Err(QError::UnsupportedDType {
            dtype: descriptor.dtype.as_safetensors_str().to_string(),
            operation: format!("exact slice read of {}", descriptor.canonical_name),
        });
    }
    let (r0, r1) = rows;
    let (c0, c1) = columns;
    if r1 <= r0 || c1 <= c0 {
        return Err(QError::QueryRejected(format!(
            "empty slice [{r0}:{r1}, {c0}:{c1}] on {}",
            descriptor.canonical_name
        )));
    }
    if r1 > descriptor.shape[0] || c1 > descriptor.shape[1] {
        return Err(QError::IndexOutOfBounds {
            tensor: descriptor.canonical_name.clone(),
            index: vec![r1.saturating_sub(1), c1.saturating_sub(1)],
            shape: descriptor.shape.clone(),
        });
    }

    let n_rows = r1 - r0;
    let n_cols = c1 - c0;
    let total = n_rows * n_cols;
    if total > MAX_QUERY_RESULT_ELEMENTS {
        return Err(QError::BudgetExceeded {
            budget_name: "query_result_elements",
            requested: total,
            limit: MAX_QUERY_RESULT_ELEMENTS,
        });
    }

    let width = descriptor.dtype.size_in_bytes();
    let budget = MemoryBudget::single_read();
    let mut values = Vec::with_capacity(total as usize);
    let mut bytes_read = 0u64;

    // One contiguous read per row: the last axis is contiguous in row-major
    // order, so a column window is a run, but successive rows are not adjacent.
    for r in r0..r1 {
        let (start, end) = descriptor.element_run_range(&[r, c0], n_cols)?;
        let bytes = source.read_range_buffered(
            &descriptor.shard_uri,
            start,
            end - start,
            &budget,
        )?;
        bytes_read += bytes.len() as u64;
        values.extend(descriptor.dtype.decode_run(&bytes)?);
    }
    debug_assert_eq!(values.len() as u64, total);
    debug_assert_eq!(bytes_read, total * width);

    Ok(SliceRead {
        canonical_name: descriptor.canonical_name.clone(),
        row_start: r0,
        row_end: r1,
        column_start: c0,
        column_end: c1,
        values,
        dtype: descriptor.dtype,
        fidelity: ResultFidelity::Exact,
        bytes_read,
    })
}

/// Read a whole row of a 2-D tensor (the `Q[10][100]` / `MLP.down[24][:]` form).
pub fn read_row(
    source: &dyn ModelSource,
    descriptor: &TensorDescriptor,
    row: u64,
) -> Result<SliceRead> {
    if descriptor.shape.len() != 2 {
        return Err(QError::QueryRejected(format!(
            "{} has rank {}; row read requires rank 2",
            descriptor.canonical_name,
            descriptor.shape.len()
        )));
    }
    read_slice_2d(source, descriptor, (row, row + 1), (0, descriptor.shape[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::ingest_local;
    use std::path::{Path, PathBuf};

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/tiny-llama-2shard")
            .canonicalize()
            .expect("run fixtures/generate_fixtures.py")
    }

    fn setup() -> (q_source::LocalFsSource, crate::ingest::IngestOutcome) {
        let src = q_source::LocalFsSource::open(fixture_dir()).unwrap();
        let out = ingest_local(fixture_dir()).unwrap();
        (src, out)
    }

    #[test]
    fn scalar_read_touches_only_dtype_width_bytes() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.self_attn.q_proj.weight").unwrap();
        let s = read_scalar(&src, d, &[100, 42]).unwrap();
        assert_eq!(s.bytes_read, 4);
        assert_eq!(s.fidelity, ResultFidelity::Exact);
        // Golden value from fixtures/tiny-llama-2shard/golden.json (0x3BD1FB7E).
        assert_eq!(s.value as f32, f32::from_bits(0x3BD1FB7E));
    }

    #[test]
    fn scalar_reads_match_golden_at_corners() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.self_attn.q_proj.weight").unwrap();
        assert_eq!(
            read_scalar(&src, d, &[0, 0]).unwrap().value as f32,
            f32::from_bits(0x3D02A4B7)
        );
        assert_eq!(
            read_scalar(&src, d, &[127, 47]).unwrap().value as f32,
            f32::from_bits(0x3CD9B444)
        );
    }

    #[test]
    fn first_shard_tensor_resolves_to_its_own_shard() {
        let (src, out) = setup();
        let d = out.find("model.layers.3.self_attn.q_proj.weight").unwrap();
        assert_eq!(d.shard_uri, "model-00001-of-00002.safetensors");
        assert_eq!(
            read_scalar(&src, d, &[100, 42]).unwrap().value as f32,
            f32::from_bits(0x3BD54A14)
        );
    }

    #[test]
    fn one_dimensional_tensor_scalar_read() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.input_layernorm.weight").unwrap();
        assert_eq!(
            read_scalar(&src, d, &[7]).unwrap().value as f32,
            f32::from_bits(0xBC291DFE)
        );
    }

    #[test]
    fn slice_read_matches_golden_and_reads_only_the_window() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.self_attn.q_proj.weight").unwrap();
        let s = read_slice_2d(&src, d, (100, 104), (40, 44)).unwrap();
        assert_eq!((s.rows(), s.columns()), (4, 4));
        assert_eq!(s.bytes_read, 4 * 4 * 4);
        let golden = [
            0x3C6AC97Eu32, 0x3CF61617, 0x3BD1FB7E, 0xBC11D8DF, 0x3BDB7F5B, 0xBBCE95A9, 0xBC831466,
            0x3C22237C, 0x3B93C26B, 0xBCD65E80, 0x3CC4ACFC, 0xBBCD0478, 0xBD2C3367, 0xBCA7EE40,
            0xBC95E7AA, 0xBD00D0AA,
        ];
        for (got, want) in s.values.iter().zip(golden.iter()) {
            assert_eq!(*got as f32, f32::from_bits(*want));
        }
        // Cross-check: the slice's (0,2) entry is the Section 7 scalar.
        assert_eq!(s.get(0, 2).unwrap() as f32, f32::from_bits(0x3BD1FB7E));
    }

    #[test]
    fn slice_on_a_second_tensor_matches_golden() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.self_attn.k_proj.weight").unwrap();
        let s = read_slice_2d(&src, d, (0, 2), (0, 3)).unwrap();
        let golden = [0xBB94AA1Fu32, 0xBC92CC80, 0x3C77372C, 0x3C546F02, 0xBC53405E, 0xBD1C9EFD];
        for (got, want) in s.values.iter().zip(golden.iter()) {
            assert_eq!(*got as f32, f32::from_bits(*want));
        }
    }

    #[test]
    fn row_read_returns_the_whole_row() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.self_attn.q_proj.weight").unwrap();
        let s = read_row(&src, d, 100).unwrap();
        assert_eq!((s.rows(), s.columns()), (1, 48));
        assert_eq!(s.get(0, 42).unwrap() as f32, f32::from_bits(0x3BD1FB7E));
    }

    #[test]
    fn out_of_bounds_index_is_rejected_before_any_read() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.self_attn.q_proj.weight").unwrap();
        assert!(matches!(
            read_scalar(&src, d, &[128, 0]),
            Err(QError::IndexOutOfBounds { .. })
        ));
        assert!(matches!(
            read_slice_2d(&src, d, (0, 129), (0, 4)),
            Err(QError::IndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn empty_and_inverted_slices_are_rejected() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.self_attn.q_proj.weight").unwrap();
        assert!(read_slice_2d(&src, d, (4, 4), (0, 4)).is_err());
        assert!(read_slice_2d(&src, d, (4, 2), (0, 4)).is_err());
    }

    #[test]
    fn bf16_scalar_read_decodes_from_the_high_half() {
        let (src, out) = setup();
        let d = out.find("model.layers.0.mlp.gate_proj.weight").unwrap();
        let s = read_scalar(&src, d, &[0, 0]).unwrap();
        assert_eq!(s.bytes_read, 2);
        // Golden bf16 bit pattern 0x3D1E from golden.json.
        assert_eq!(s.value as f32, f32::from_bits(0x3D1E_0000));
    }

    #[test]
    fn slice_of_a_1d_tensor_is_rejected_rather_than_reshaped() {
        let (src, out) = setup();
        let d = out.find("model.layers.10.input_layernorm.weight").unwrap();
        assert!(matches!(
            read_slice_2d(&src, d, (0, 1), (0, 4)),
            Err(QError::QueryRejected(_))
        ));
    }
}
