//! # q-tensor-runtime — Metadata Plane
//!
//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §5.3, §9).
//!
//! Tensor blocks, tile identity, the LOD ladder, and the bounded streaming
//! block reader.
//!
//! **Scope of this module (`lib.rs`): types only.** These are the addressing
//! primitives the tile compiler and the compute backends share; nothing in
//! *this file* executes, reads bytes, or produces tiles (`TILE-004`). What it
//! *does* do is make the LOD ladder of ARCHITECTURE.md §9.1 a closed enum, so
//! no code can invent a seventh level or conflate "block statistics" with
//! "exact values".
//!
//! **[`stream`] is the one part of this crate that reads bytes** (`TILE-009`).
//! It still produces no tiles: it walks a tensor's block grid, range-reads each
//! block through a [`q_source::ModelSource`], and decodes it into a
//! [`BlockData`] under buffers whose size depends on the block and never on the
//! tensor. See that module for the residency contract.
//!
//! ## The ladder (§9.1–9.2)
//!
//! | LOD | Object        | Data                                          |
//! | --- | ------------- | --------------------------------------------- |
//! | 0   | Model         | parameter count, bytes, global distributions  |
//! | 1   | Subsystem     | layer ranges, aggregate norms                 |
//! | 2   | Layer         | tensor count, mean norm, anomaly score        |
//! | 3   | Tensor        | shape, dtype, histogram, spectrum summary     |
//! | 4   | Block         | block statistics, quantized samples           |
//! | 5   | Scalar region | exact or sampled weight values                |
//!
//! Only LOD 5 may carry exact values, and only on demand. Everything above it
//! is summary data, labelled as such via [`q_source::AccessScale`].

pub mod residency;
pub mod stream;

pub use residency::{ResidencyOutcome, ResidencyRequest, TensorRefusal};
pub use stream::{BlockGrid, BlockStream, BlockStreamConfig, StreamOutcome, StreamedBlock};

use q_source::error::{QError, Result};
use q_source::{AccessScale, TensorDescriptor, TensorId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The six level-of-detail tiers of ARCHITECTURE.md §9.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Lod {
    Model = 0,
    Subsystem = 1,
    Layer = 2,
    Tensor = 3,
    Block = 4,
    ScalarRegion = 5,
}

impl Lod {
    pub const ALL: [Lod; 6] = [
        Lod::Model,
        Lod::Subsystem,
        Lod::Layer,
        Lod::Tensor,
        Lod::Block,
        Lod::ScalarRegion,
    ];

    pub fn level(self) -> u8 {
        self as u8
    }

    pub fn from_level(level: u8) -> Result<Self> {
        Ok(match level {
            0 => Lod::Model,
            1 => Lod::Subsystem,
            2 => Lod::Layer,
            3 => Lod::Tensor,
            4 => Lod::Block,
            5 => Lod::ScalarRegion,
            other => {
                return Err(QError::malformed(
                    "lod",
                    format!("level {other} is outside the 0..=5 ladder of ARCHITECTURE.md §9.1"),
                ))
            }
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Lod::Model => "model",
            Lod::Subsystem => "subsystem",
            Lod::Layer => "layer",
            Lod::Tensor => "tensor",
            Lod::Block => "block",
            Lod::ScalarRegion => "scalar_region",
        }
    }

    /// Whether this level may carry exact weight values.
    ///
    /// ARCHITECTURE.md §9.3: exact bytes are range-read only on select or
    /// inspect. Anything coarser is a summary and must never be presented as an
    /// exact answer.
    pub fn carries_exact_values(self) -> bool {
        self == Lod::ScalarRegion
    }

    /// The access scale a consumer of this level is operating at.
    pub fn access_scale(self) -> AccessScale {
        match self {
            Lod::ScalarRegion => AccessScale::SelectedBlockExact,
            _ => AccessScale::Visualization,
        }
    }

    pub fn child(self) -> Option<Lod> {
        Lod::from_level(self.level() + 1).ok()
    }

    pub fn parent(self) -> Option<Lod> {
        if self.level() == 0 {
            None
        } else {
            Lod::from_level(self.level() - 1).ok()
        }
    }
}

impl fmt::Display for Lod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LOD{} ({})", self.level(), self.as_str())
    }
}

/// A half-open 2-D window of a tensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockExtent {
    pub row_start: u64,
    pub row_end: u64,
    pub column_start: u64,
    pub column_end: u64,
}

impl BlockExtent {
    pub fn new(row_start: u64, row_end: u64, column_start: u64, column_end: u64) -> Result<Self> {
        if row_end <= row_start || column_end <= column_start {
            return Err(QError::QueryRejected(format!(
                "empty block extent [{row_start}:{row_end}, {column_start}:{column_end}]"
            )));
        }
        Ok(Self {
            row_start,
            row_end,
            column_start,
            column_end,
        })
    }

    pub fn rows(&self) -> u64 {
        self.row_end - self.row_start
    }

    pub fn columns(&self) -> u64 {
        self.column_end - self.column_start
    }

    pub fn element_count(&self) -> u64 {
        self.rows() * self.columns()
    }

    pub fn contains(&self, row: u64, column: u64) -> bool {
        row >= self.row_start
            && row < self.row_end
            && column >= self.column_start
            && column < self.column_end
    }

    /// Clamp to a tensor's shape, or reject if it starts outside it.
    pub fn clamped_to(&self, shape: &[u64]) -> Result<BlockExtent> {
        if shape.len() != 2 {
            return Err(QError::QueryRejected(format!(
                "block extents apply to rank-2 tensors; got rank {}",
                shape.len()
            )));
        }
        if self.row_start >= shape[0] || self.column_start >= shape[1] {
            return Err(QError::IndexOutOfBounds {
                tensor: "<block>".into(),
                index: vec![self.row_start, self.column_start],
                shape: shape.to_vec(),
            });
        }
        BlockExtent::new(
            self.row_start,
            self.row_end.min(shape[0]),
            self.column_start,
            self.column_end.min(shape[1]),
        )
    }
}

/// The byte ranges backing one block.
///
/// A row-major block is *not* contiguous unless it spans whole rows, so a block
/// is a list of runs. Storing them explicitly means the tile compiler and the
/// cache both see the true I/O cost of a block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceByteRanges(pub Vec<(u64, u64)>);

impl SourceByteRanges {
    pub fn total_bytes(&self) -> u64 {
        self.0.iter().map(|(s, e)| e.saturating_sub(*s)).sum()
    }

    pub fn run_count(&self) -> usize {
        self.0.len()
    }
}

/// A block of a tensor at some LOD — the `tensor_blocks` row of §5.3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TensorBlock {
    pub block_id: TileId,
    pub tensor_id: TensorId,
    pub lod: Lod,
    pub extent: BlockExtent,
    pub source_byte_ranges: SourceByteRanges,
    /// Set once statistics have been computed for this block.
    pub statistics_id: Option<String>,
    /// Content hash for cache keying (ARCHITECTURE.md §13.2).
    pub content_hash: String,
}

impl TensorBlock {
    /// Derive the byte ranges for a block of a descriptor.
    ///
    /// Pure arithmetic: it computes *where* the bytes are and reads none of
    /// them. This boundary is what the whole design rests on.
    pub fn plan(
        descriptor: &TensorDescriptor,
        lod: Lod,
        extent: BlockExtent,
    ) -> Result<TensorBlock> {
        let extent = extent.clamped_to(&descriptor.shape)?;
        let mut ranges = Vec::with_capacity(extent.rows() as usize);
        for row in extent.row_start..extent.row_end {
            let (start, end) =
                descriptor.element_run_range(&[row, extent.column_start], extent.columns())?;
            ranges.push((start, end));
        }
        let block_id = TileId::for_block(descriptor.tensor_id, lod, &extent);
        Ok(TensorBlock {
            content_hash: block_id.content_hash(),
            block_id,
            tensor_id: descriptor.tensor_id,
            lod,
            extent,
            source_byte_ranges: SourceByteRanges(ranges),
            statistics_id: None,
        })
    }

    pub fn byte_cost(&self) -> u64 {
        self.source_byte_ranges.total_bytes()
    }
}

/// A tile's identity, stable across runs.
///
/// Derived from `(tensor, lod, extent)` so the same block always gets the same
/// ID — which is what lets the L2 cache and the tileset agree with each other
/// across sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileId(pub [u8; 16]);

impl TileId {
    pub fn for_block(tensor: TensorId, lod: Lod, extent: &BlockExtent) -> TileId {
        let mut h = blake3::Hasher::new();
        h.update(b"quatricmorph/tile/v1");
        h.update(tensor.as_bytes());
        h.update(&[lod.level()]);
        for v in [
            extent.row_start,
            extent.row_end,
            extent.column_start,
            extent.column_end,
        ] {
            h.update(&v.to_le_bytes());
        }
        let mut out = [0u8; 16];
        out.copy_from_slice(&h.finalize().as_bytes()[..16]);
        TileId(out)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn content_hash(&self) -> String {
        format!("b3:{}", self.to_hex())
    }
}

impl fmt::Display for TileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// A materialized dense block. Deliberately `f32`: this is a *selected block*,
/// never a whole tensor.
///
/// This type lives here rather than in `q-gpu` because it is the value
/// container the block *runtime* produces — [`stream::BlockStream`] emits one
/// per block — and `q-gpu` already depends on this crate, so the reverse edge
/// would be a dependency cycle. `q_gpu::BlockData` re-exports this type, so
/// every existing path keeps working.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockData {
    pub rows: usize,
    pub columns: usize,
    pub values: Vec<f32>,
}

impl BlockData {
    pub fn new(rows: usize, columns: usize, values: Vec<f32>) -> Result<Self> {
        if values.len() != rows * columns {
            return Err(QError::malformed(
                "block",
                format!(
                    "{} values supplied for a {rows}x{columns} block",
                    values.len()
                ),
            ));
        }
        Ok(Self {
            rows,
            columns,
            values,
        })
    }

    pub fn get(&self, i: usize, j: usize) -> Option<f32> {
        if i >= self.rows || j >= self.columns {
            return None;
        }
        self.values.get(i * self.columns + j).copied()
    }
}

/// How a block's values are encoded in a `.qtile` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum BlockEncoding {
    /// Exact f32 values.
    RawF32 = 0,
    /// Values quantized to i16 against the tile's declared min/max.
    QuantizedI16 = 1,
    /// Morton-ordered sparse cells (ARCHITECTURE.md §10.3 / §11.1).
    MortonSparseI16 = 2,
}

impl BlockEncoding {
    pub fn code(self) -> u16 {
        self as u16
    }

    pub fn from_code(code: u16) -> Result<Self> {
        Ok(match code {
            0 => BlockEncoding::RawF32,
            1 => BlockEncoding::QuantizedI16,
            2 => BlockEncoding::MortonSparseI16,
            other => {
                return Err(QError::malformed(
                    "qtile",
                    format!("unknown block encoding {other}"),
                ))
            }
        })
    }

    /// Whether values in this encoding are exact.
    pub fn is_lossless(self) -> bool {
        matches!(self, BlockEncoding::RawF32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_source::role::TensorRole;
    use q_source::{DType, ModelId};

    fn descriptor(shape: Vec<u64>) -> TensorDescriptor {
        let model = ModelId::derive("m", "", "f");
        let n: u64 = shape.iter().product();
        TensorDescriptor {
            tensor_id: TensorId::derive(model, "t"),
            raw_name: "t".into(),
            canonical_name: "t".into(),
            shape,
            dtype: DType::F32,
            shard_uri: "s.safetensors".into(),
            byte_start: 1000,
            byte_end: 1000 + n * 4,
            layer_index: None,
            semantic_role: TensorRole::Unknown,
        }
    }

    #[test]
    fn the_ladder_has_exactly_six_levels() {
        assert_eq!(Lod::ALL.len(), 6);
        for (i, lod) in Lod::ALL.iter().enumerate() {
            assert_eq!(lod.level() as usize, i);
            assert_eq!(Lod::from_level(i as u8).unwrap(), *lod);
        }
        assert!(Lod::from_level(6).is_err());
    }

    #[test]
    fn only_the_finest_level_carries_exact_values() {
        for lod in Lod::ALL {
            assert_eq!(lod.carries_exact_values(), lod == Lod::ScalarRegion);
        }
        assert_eq!(Lod::Block.access_scale(), AccessScale::Visualization);
        assert_eq!(
            Lod::ScalarRegion.access_scale(),
            AccessScale::SelectedBlockExact
        );
        assert!(!Lod::Block.access_scale().is_exact());
    }

    #[test]
    fn ladder_navigation_terminates_at_both_ends() {
        assert_eq!(Lod::Model.parent(), None);
        assert_eq!(Lod::ScalarRegion.child(), None);
        assert_eq!(Lod::Layer.child(), Some(Lod::Tensor));
        assert_eq!(Lod::Layer.parent(), Some(Lod::Subsystem));
        assert_eq!(Lod::Block.to_string(), "LOD4 (block)");
    }

    #[test]
    fn empty_and_inverted_extents_are_rejected() {
        assert!(BlockExtent::new(0, 0, 0, 4).is_err());
        assert!(BlockExtent::new(4, 2, 0, 4).is_err());
        assert!(BlockExtent::new(0, 4, 0, 4).is_ok());
    }

    #[test]
    fn extents_clamp_to_the_tensor_shape() {
        let e = BlockExtent::new(0, 512, 0, 512).unwrap();
        let c = e.clamped_to(&[128, 48]).unwrap();
        assert_eq!((c.rows(), c.columns()), (128, 48));
        assert!(BlockExtent::new(999, 1000, 0, 4)
            .unwrap()
            .clamped_to(&[128, 48])
            .is_err());
    }

    #[test]
    fn block_planning_derives_one_byte_run_per_row() {
        let d = descriptor(vec![128, 48]);
        let block =
            TensorBlock::plan(&d, Lod::Block, BlockExtent::new(100, 104, 40, 44).unwrap()).unwrap();
        assert_eq!(block.source_byte_ranges.run_count(), 4);
        assert_eq!(block.byte_cost(), 4 * 4 * 4);
        assert_eq!(block.source_byte_ranges.0[0].0, 1000 + (100 * 48 + 40) * 4);
        assert_eq!(block.lod, Lod::Block);
        assert!(block.statistics_id.is_none());
    }

    #[test]
    fn a_full_width_block_has_abutting_runs() {
        // Rows are contiguous; consecutive rows abut only when the block spans
        // the full width. The run count is the honest I/O cost either way.
        let d = descriptor(vec![8, 4]);
        let block =
            TensorBlock::plan(&d, Lod::Block, BlockExtent::new(0, 8, 0, 4).unwrap()).unwrap();
        assert_eq!(block.source_byte_ranges.run_count(), 8);
        assert_eq!(block.byte_cost(), 8 * 4 * 4);
        for w in block.source_byte_ranges.0.windows(2) {
            assert_eq!(w[0].1, w[1].0, "full-width runs should abut");
        }
    }

    #[test]
    fn tile_ids_are_stable_and_sensitive_to_extent_and_lod() {
        let d = descriptor(vec![128, 48]);
        let a = TensorBlock::plan(&d, Lod::Block, BlockExtent::new(0, 4, 0, 4).unwrap()).unwrap();
        let b = TensorBlock::plan(&d, Lod::Block, BlockExtent::new(0, 4, 0, 4).unwrap()).unwrap();
        let c = TensorBlock::plan(&d, Lod::Block, BlockExtent::new(0, 4, 4, 8).unwrap()).unwrap();
        assert_eq!(a.block_id, b.block_id);
        assert_ne!(a.block_id, c.block_id);
        let s = TensorBlock::plan(&d, Lod::ScalarRegion, BlockExtent::new(0, 4, 0, 4).unwrap())
            .unwrap();
        assert_ne!(a.block_id, s.block_id);
        assert!(a.content_hash.starts_with("b3:"));
        assert_eq!(a.block_id.to_hex().len(), 32);
    }

    #[test]
    fn block_containment_test() {
        let e = BlockExtent::new(100, 104, 40, 44).unwrap();
        assert!(e.contains(100, 40));
        assert!(e.contains(103, 43));
        assert!(!e.contains(104, 40));
        assert!(!e.contains(100, 44));
        assert_eq!(e.element_count(), 16);
    }

    #[test]
    fn encodings_declare_whether_they_are_lossless() {
        assert!(BlockEncoding::RawF32.is_lossless());
        assert!(!BlockEncoding::QuantizedI16.is_lossless());
        assert!(!BlockEncoding::MortonSparseI16.is_lossless());
        for e in [
            BlockEncoding::RawF32,
            BlockEncoding::QuantizedI16,
            BlockEncoding::MortonSparseI16,
        ] {
            assert_eq!(BlockEncoding::from_code(e.code()).unwrap(), e);
        }
        assert!(BlockEncoding::from_code(99).is_err());
    }

    #[test]
    fn rank_mismatch_is_rejected() {
        let d = descriptor(vec![48]);
        assert!(TensorBlock::plan(&d, Lod::Block, BlockExtent::new(0, 4, 0, 4).unwrap()).is_err());
    }
}
