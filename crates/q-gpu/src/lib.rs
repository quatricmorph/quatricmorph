//! # q-gpu — compute backend boundary
//!
//! Data plane: operates on the **Tensor Tile Plane**, reading from the
//! **Artifact Plane** (ARCHITECTURE.md §2.1, §12.3).
//!
//! The trait every compute backend implements, plus the CPU reference
//! implementation that defines correct behaviour.
//!
//! ARCHITECTURE.md §12.3 splits the work deliberately:
//!
//! ```text
//! rendering            -> wgpu / WebGPU / Metal / Vulkan
//! large tensor compute -> CUDA, Metal Performance Shaders, CPU SIMD/BLAS
//! ```
//!
//! This crate is the *compute* side. It never renders.
//!
//! ## Backends in this build
//!
//! | backend           | status                                                    |
//! |-------------------|-----------------------------------------------------------|
//! | [`CpuBackend`]    | **Implemented and tested** — the reference for all others |
//! | `q_cuda::CudaBackend` | Interface only; **Hardware-Unverified** (`CUDA-001`)  |
//! | [`metal::MetalBackend`] | Behind the off-by-default `metal` feature. One kernel — the paired reduction — compiled and dispatched on a real Apple GPU, but **not yet diffed against [`CpuBackend`]** (`GPU-003`; `QM-0127` is the diff) |
//! | wgpu              | Not started; `gpu/wgsl/compute.wgsl` remains a placeholder |
//!
//! Selection is explicit. [`default_backend`] returns [`CpuBackend`] in every
//! build, feature on or off; no code path picks a GPU on a caller's behalf.
//!
//! ## What a backend may promise
//!
//! [`ComputeCapabilities`] states, per backend, what it can actually do and how
//! much memory it has. The RTX 3090 target has 24 GB of VRAM; nothing in this
//! API lets a caller ask a backend to hold a trillion-parameter tensor, and
//! [`Backend::check_workload`] refuses a workload that would not fit rather
//! than discovering it mid-kernel.

#[cfg(feature = "metal")]
pub mod metal;
pub mod paired;

#[cfg(feature = "metal")]
pub use metal::MetalBackend;
pub use paired::{ChannelAxis, ChannelPartials, PairedPartials};

use q_source::error::{QError, Result};
use q_source::manifest::{ModelSource, ModelSourceExt};
use q_source::{MemoryBudget, TensorDescriptor};
use q_statistics::{StatisticsAccumulator, TensorStatistics, DEFAULT_HISTOGRAM_BINS};
use q_tensor_runtime::{BlockExtent, TensorBlock};
use serde::{Deserialize, Serialize};

/// What a backend can do and how much memory it has.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputeCapabilities {
    pub backend_id: String,
    pub display_name: String,
    /// Bytes the backend can hold at once. For a GPU this is VRAM.
    pub device_memory_bytes: u64,
    pub supports_statistics: bool,
    pub supports_matmul: bool,
    pub supports_histogram: bool,
    /// `false` when the code exists but has never been run on the hardware it
    /// targets. Consumers must surface this rather than treating the backend as
    /// proven.
    pub hardware_verified: bool,
    /// Requirement ID covering the unbuilt or unverified parts.
    pub caveat_requirement: Option<String>,
}

/// A described unit of work, so a backend can refuse before starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Workload {
    pub element_count: u64,
    pub bytes_per_element: u64,
}

impl Workload {
    pub fn bytes(&self) -> u64 {
        self.element_count.saturating_mul(self.bytes_per_element)
    }

    /// The workload of a **pair** of `rows × columns` `f32` blocks.
    ///
    /// A paired reduction holds both operands resident at once, so charging the
    /// budget for one of them would let a pair through at twice the declared
    /// limit. `.plan/DIAGNOSTIC_ARCHITECTURE.md` §5 counts "base block +
    /// counterpart block" for exactly this reason.
    ///
    /// Saturating throughout: a shape whose product overflows `u64` must reduce
    /// to a refused budget, never to a small number that passes.
    pub fn for_paired_blocks(rows: usize, columns: usize) -> Workload {
        Workload {
            element_count: (rows as u64)
                .saturating_mul(columns as u64)
                .saturating_mul(2),
            bytes_per_element: 4,
        }
    }
}

/// A compute backend.
pub trait Backend: Send + Sync {
    fn capabilities(&self) -> ComputeCapabilities;

    /// Statistics over one block, read through `source`.
    fn block_statistics(
        &self,
        source: &dyn ModelSource,
        descriptor: &TensorDescriptor,
        extent: BlockExtent,
        histogram_bins: usize,
    ) -> Result<TensorStatistics>;

    /// Dense matrix multiply of two already-materialized blocks.
    fn matmul(&self, a: &BlockData, b: &BlockData) -> Result<BlockData>;

    /// Reduce a base block against a counterpart block of the same shape,
    /// producing whole-block and per-channel partials.
    ///
    /// The counterpart is **any** second block. This method does not know, and
    /// must not be told, where it came from: a simulated reconstruction, a
    /// second checkpoint's block, or a sibling expert's block are all the same
    /// call. See [`paired`] for the reasoning and for what the partials are.
    ///
    /// The default refuses, so a backend that has not implemented it says so
    /// instead of returning zeroes that look like a perfect match.
    fn paired_block_reduction(
        &self,
        base: &BlockData,
        counterpart: &BlockData,
        axis: ChannelAxis,
    ) -> Result<PairedPartials> {
        let _ = (base, counterpart, axis);
        Err(QError::NotImplemented {
            requirement: "QUANT-002",
            detail: format!(
                "backend {} does not implement the paired block reduction",
                self.capabilities().backend_id
            ),
        })
    }

    /// Refuse a workload larger than this backend can hold.
    fn check_workload(&self, workload: Workload) -> Result<()> {
        let caps = self.capabilities();
        if workload.bytes() > caps.device_memory_bytes {
            return Err(QError::BudgetExceeded {
                budget_name: "device_memory",
                requested: workload.bytes(),
                limit: caps.device_memory_bytes,
            });
        }
        Ok(())
    }
}

/// A materialized dense block.
///
/// Re-exported from [`q_tensor_runtime`], which is where it now lives so that
/// the streaming block reader (`q_tensor_runtime::stream`) can produce one
/// without depending on this crate — `q-gpu` already depends on
/// `q-tensor-runtime`, so the reverse edge would be a dependency cycle. This
/// alias keeps `q_gpu::BlockData` a valid path for every existing caller.
pub use q_tensor_runtime::BlockData;

/// The CPU reference backend.
///
/// This is what every other backend is validated against. It is correct and
/// slow, in that order of priority — a divergence between this and a GPU kernel
/// is a bug in the kernel, by definition.
#[derive(Debug, Default, Clone, Copy)]
pub struct CpuBackend;

impl CpuBackend {
    pub const ID: &'static str = "cpu-reference";
}

impl Backend for CpuBackend {
    fn capabilities(&self) -> ComputeCapabilities {
        ComputeCapabilities {
            backend_id: Self::ID.to_string(),
            display_name: "CPU reference implementation".to_string(),
            // Bounded by the single-read budget, not by system RAM: this
            // backend operates on selected blocks, never whole checkpoints.
            device_memory_bytes: q_source::budget::MAX_SINGLE_READ_BYTES,
            supports_statistics: true,
            supports_matmul: true,
            supports_histogram: true,
            hardware_verified: true,
            caveat_requirement: None,
        }
    }

    fn block_statistics(
        &self,
        source: &dyn ModelSource,
        descriptor: &TensorDescriptor,
        extent: BlockExtent,
        histogram_bins: usize,
    ) -> Result<TensorStatistics> {
        let block = TensorBlock::plan(descriptor, q_tensor_runtime::Lod::Block, extent)?;
        self.check_workload(Workload {
            element_count: block.extent.element_count(),
            bytes_per_element: descriptor.dtype.size_in_bytes(),
        })?;

        // Two passes over the block's byte runs: one for the range, one for the
        // histogram. Both stream run by run, so peak memory is one row, not one
        // block — let alone one tensor.
        let budget = MemoryBudget::single_read();
        let mut range = StatisticsAccumulator::new();
        for (start, end) in &block.source_byte_ranges.0 {
            let bytes =
                source.read_range_buffered(&descriptor.shard_uri, *start, end - start, &budget)?;
            range.push_bytes(&bytes, descriptor.dtype)?;
        }
        let range = range.finish(Self::ID)?;

        let mut acc = StatisticsAccumulator::new().with_histogram(
            range.min_value,
            range.max_value,
            histogram_bins.max(1),
        );
        for (start, end) in &block.source_byte_ranges.0 {
            let bytes =
                source.read_range_buffered(&descriptor.shard_uri, *start, end - start, &budget)?;
            acc.push_bytes(&bytes, descriptor.dtype)?;
        }
        acc.finish(Self::ID)
    }

    fn matmul(&self, a: &BlockData, b: &BlockData) -> Result<BlockData> {
        if a.columns != b.rows {
            return Err(QError::QueryRejected(format!(
                "shape mismatch: [{}, {}] @ [{}, {}] — inner dimensions {} and {} differ",
                a.rows, a.columns, b.rows, b.columns, a.columns, b.rows
            )));
        }
        self.check_workload(Workload {
            element_count: (a.rows * b.columns) as u64,
            bytes_per_element: 4,
        })?;
        let mut out = vec![0f32; a.rows * b.columns];
        for i in 0..a.rows {
            for k in 0..a.columns {
                let aik = a.values[i * a.columns + k];
                if aik == 0.0 {
                    continue;
                }
                for j in 0..b.columns {
                    out[i * b.columns + j] += aik * b.values[k * b.columns + j];
                }
            }
        }
        BlockData::new(a.rows, b.columns, out)
    }

    fn paired_block_reduction(
        &self,
        base: &BlockData,
        counterpart: &BlockData,
        axis: ChannelAxis,
    ) -> Result<PairedPartials> {
        self.reduce_paired_blocks(base, counterpart, axis)
    }
}

/// The backend used when the caller has not named one.
///
/// Always [`CpuBackend`], in every build. This function exists so that
/// "nothing selects a GPU implicitly" is a fact with a test attached
/// (`the_default_backend_is_the_cpu_reference_whatever_features_are_enabled`)
/// rather than an absence someone has to audit for.
///
/// A GPU backend is opted into by constructing it — `MetalBackend::probe()` —
/// and by handling the `None` it returns where there is no device. That is the
/// only way work reaches one, and it stays that way until a backend has been
/// diffed against this reference (`QM-0127` for Metal, `CUDA-001` for CUDA).
pub fn default_backend() -> CpuBackend {
    CpuBackend
}

/// Convenience: statistics over a block with the default histogram resolution.
pub fn block_statistics_default(
    backend: &dyn Backend,
    source: &dyn ModelSource,
    descriptor: &TensorDescriptor,
    extent: BlockExtent,
) -> Result<TensorStatistics> {
    backend.block_statistics(source, descriptor, extent, DEFAULT_HISTOGRAM_BINS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_source::LocalFsSource;
    use std::path::{Path, PathBuf};

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/tiny-llama-2shard")
            .canonicalize()
            .expect("run fixtures/generate_fixtures.py")
    }

    #[test]
    fn cpu_backend_declares_itself_verified_and_capable() {
        let caps = CpuBackend.capabilities();
        assert_eq!(caps.backend_id, "cpu-reference");
        assert!(caps.hardware_verified);
        assert!(caps.supports_statistics && caps.supports_matmul && caps.supports_histogram);
        assert!(caps.caveat_requirement.is_none());
    }

    #[test]
    fn the_default_backend_is_the_cpu_reference_whatever_features_are_enabled() {
        // This test compiles identically with and without `--features metal`,
        // which is the point: enabling a GPU backend must not change what an
        // unopinionated caller gets. QM-0126 acceptance criterion 8.
        let caps = default_backend().capabilities();
        assert_eq!(caps.backend_id, CpuBackend::ID);
        assert!(caps.hardware_verified);
    }

    #[test]
    fn hand_computed_matmul_2x3_by_3x2() {
        // A = [[1,2,3],[4,5,6]]  B = [[7,8],[9,10],[11,12]]
        // AB = [[1*7+2*9+3*11, 1*8+2*10+3*12], [4*7+5*9+6*11, 4*8+5*10+6*12]]
        //    = [[58, 64], [139, 154]]
        let a = BlockData::new(2, 3, vec![1., 2., 3., 4., 5., 6.]).unwrap();
        let b = BlockData::new(3, 2, vec![7., 8., 9., 10., 11., 12.]).unwrap();
        let c = CpuBackend.matmul(&a, &b).unwrap();
        assert_eq!((c.rows, c.columns), (2, 2));
        assert_eq!(c.values, vec![58., 64., 139., 154.]);
    }

    #[test]
    fn hand_computed_matmul_edge_shapes() {
        // 3x3 @ 3x1 -> 3x1: identity times a column is the column.
        let i3 = BlockData::new(3, 3, vec![1., 0., 0., 0., 1., 0., 0., 0., 1.]).unwrap();
        let col = BlockData::new(3, 1, vec![2., 3., 4.]).unwrap();
        let r = CpuBackend.matmul(&i3, &col).unwrap();
        assert_eq!((r.rows, r.columns), (3, 1));
        assert_eq!(r.values, vec![2., 3., 4.]);

        // 1x3 @ 3x2 -> 1x2: [1,2,3] @ [[1,0],[0,1],[1,1]] = [1+0+3, 0+2+3] = [4,5]
        let row = BlockData::new(1, 3, vec![1., 2., 3.]).unwrap();
        let m = BlockData::new(3, 2, vec![1., 0., 0., 1., 1., 1.]).unwrap();
        let r = CpuBackend.matmul(&row, &m).unwrap();
        assert_eq!((r.rows, r.columns), (1, 2));
        assert_eq!(r.values, vec![4., 5.]);

        // 1x3 @ 3x1 -> 1x1 (dot product): 1*4 + 2*5 + 3*6 = 32
        let v = BlockData::new(3, 1, vec![4., 5., 6.]).unwrap();
        let r = CpuBackend.matmul(&row, &v).unwrap();
        assert_eq!((r.rows, r.columns), (1, 1));
        assert_eq!(r.values, vec![32.]);

        // 1x1 @ 1x1 -> 1x1
        let s = BlockData::new(1, 1, vec![7.]).unwrap();
        let r = CpuBackend.matmul(&s, &s).unwrap();
        assert_eq!(r.values, vec![49.]);
    }

    #[test]
    fn matmul_shape_mismatch_is_rejected() {
        // 2x3 @ 2x2 is invalid.
        let a = BlockData::new(2, 3, vec![1., 2., 3., 4., 5., 6.]).unwrap();
        let b = BlockData::new(2, 2, vec![1., 2., 3., 4.]).unwrap();
        let err = CpuBackend.matmul(&a, &b).unwrap_err();
        assert!(err.to_string().contains("shape mismatch"));
    }

    #[test]
    fn block_data_rejects_a_ragged_value_count() {
        assert!(BlockData::new(2, 3, vec![1., 2.]).is_err());
        let a = BlockData::new(2, 2, vec![1., 2., 3., 4.]).unwrap();
        assert_eq!(a.get(1, 1), Some(4.0));
        assert_eq!(a.get(2, 0), None);
    }

    #[test]
    fn workloads_larger_than_device_memory_are_refused_up_front() {
        let err = CpuBackend
            .check_workload(Workload {
                element_count: 1_000_000_000_000,
                bytes_per_element: 4,
            })
            .unwrap_err();
        assert!(matches!(err, QError::BudgetExceeded { .. }));
    }

    #[test]
    fn block_statistics_stream_a_real_fixture_block() {
        // Descriptors come from the real ingestion path (q-safetensors), not a
        // second header parser written for this test. A re-implementation would
        // be an untested copy of SRC-001..SRC-004 that silently drifts.
        let src = LocalFsSource::open(fixture_dir()).unwrap();
        let ingested = q_safetensors::ingest_local(fixture_dir()).unwrap();
        let d = ingested
            .find("model.layers.10.self_attn.q_proj.weight")
            .expect("fixture tensor");

        let stats = block_statistics_default(
            &CpuBackend,
            &src,
            d,
            BlockExtent::new(100, 104, 40, 44).unwrap(),
        )
        .unwrap();

        assert_eq!(stats.count, 16);
        assert!(!stats.approximate);
        assert_eq!(stats.backend, "cpu-reference");
        assert_eq!(stats.histogram.total(), 16);
        // The golden scalar at (100, 42) lies inside this block, so it must lie
        // inside the block's range.
        let known = f32::from_bits(0x3BD1FB7E) as f64;
        assert!(stats.min_value <= known && known <= stats.max_value);
        assert!(stats.l2_norm > 0.0);
        assert_eq!(
            stats.zero_ratio + stats.positive_ratio + stats.negative_ratio,
            1.0
        );
    }
}
