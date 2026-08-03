//! # q-cuda — CUDA compute backend
//!
//! Data plane: operates on the **Tensor Tile Plane** (ARCHITECTURE.md §2.1,
//! §12.3).
//!
//! # ⚠ HARDWARE-UNVERIFIED — NOTHING HERE HAS EVER RUN ON A GPU
//!
//! This crate contains the Rust-side trait boundary for the CUDA backend and
//! nothing else. There is no `nvcc` invocation, no `build.rs`, no driver or
//! runtime linkage, and no FFI. Every method returns
//! [`QError::NotImplemented`] with requirement `CUDA-001`.
//!
//! The `.cu` sources in `gpu/cuda/` carry documented kernel signatures and
//! launch geometry, but **have never been compiled or executed**. The target
//! device is an RTX 3090 (24 GB VRAM), which is not present in the environment
//! this code was written in. Until someone runs the kernels on that hardware
//! and diffs the output against [`q_gpu::CpuBackend`], every claim about this
//! backend's numerical behaviour is unproven, and
//! [`ComputeCapabilities::hardware_verified`] reports `false` accordingly.
//!
//! ## Why the crate exists now
//!
//! ARCHITECTURE.md §12.3 assigns full matrix multiplication, quantization,
//! spectral analysis, and large checkpoint comparison to CUDA. Fixing the trait
//! boundary now means the CPU reference and the eventual CUDA path are
//! interchangeable at the call site, and that the 24 GB VRAM ceiling is
//! expressed in code rather than discovered at runtime.
//!
//! ## The one thing this crate does assert
//!
//! [`CudaBackend::check_workload`] refuses work larger than the declared VRAM
//! budget. That check is real, tested, and independent of whether a GPU exists
//! — it is arithmetic on a declared limit, not a device query.

use q_gpu::{Backend, BlockData, ComputeCapabilities};
use q_source::error::{QError, Result};
use q_source::manifest::ModelSource;
use q_source::TensorDescriptor;
use q_statistics::TensorStatistics;
use q_tensor_runtime::BlockExtent;

/// VRAM on the target device (RTX 3090, 24 GiB).
pub const RTX_3090_VRAM_BYTES: u64 = 24 * 1024 * 1024 * 1024;

/// Fraction of VRAM Quatricmorph will plan to use, leaving room for the
/// driver, the display, and fragmentation. Named, not magic.
pub const USABLE_VRAM_FRACTION: f64 = 0.80;

/// Kernel sources under `gpu/cuda/`, listed so tooling and `STATUS.md` can
/// point at them. **None of these have been compiled.**
pub const KERNEL_SOURCES: &[&str] = &[
    "gpu/cuda/reduce.cu",
    "gpu/cuda/histogram.cu",
    "gpu/cuda/matmul.cu",
    "gpu/cuda/quantize.cu",
];

/// The CUDA backend. Interface only.
#[derive(Debug, Clone, Copy)]
pub struct CudaBackend {
    /// Declared device memory. Defaults to the RTX 3090 target; no device is
    /// queried, because there is no device.
    pub device_memory_bytes: u64,
}

impl Default for CudaBackend {
    fn default() -> Self {
        Self {
            device_memory_bytes: (RTX_3090_VRAM_BYTES as f64 * USABLE_VRAM_FRACTION) as u64,
        }
    }
}

impl CudaBackend {
    pub const ID: &'static str = "cuda";

    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a different device budget (for planning against other cards).
    pub fn with_device_memory(bytes: u64) -> Self {
        Self {
            device_memory_bytes: bytes,
        }
    }

    /// Whether a CUDA device is present.
    ///
    /// Always `false` in this build: there is no runtime linkage to ask. It
    /// returns a value rather than an error so callers can branch to the CPU
    /// backend without handling an error case.
    pub fn is_available() -> bool {
        false
    }

    fn unimplemented(operation: &str) -> QError {
        QError::not_implemented(
            "CUDA-001",
            format!(
                "the CUDA backend cannot {operation}: no kernel is compiled or linked in this \
                 build, and the target RTX 3090 was not available when this code was written. \
                 The kernel signatures in {} are HARDWARE-UNVERIFIED. Use q_gpu::CpuBackend for \
                 correct results. See ARCHITECTURE.md §12.3.",
                KERNEL_SOURCES.join(", ")
            ),
        )
    }
}

impl Backend for CudaBackend {
    fn capabilities(&self) -> ComputeCapabilities {
        ComputeCapabilities {
            backend_id: Self::ID.to_string(),
            display_name: "CUDA (interface only — hardware-unverified)".to_string(),
            device_memory_bytes: self.device_memory_bytes,
            // Declared as false throughout: the backend supports nothing it can
            // actually perform. Reporting `true` here would let a scheduler
            // route work to a backend that cannot run it.
            supports_statistics: false,
            supports_matmul: false,
            supports_histogram: false,
            hardware_verified: false,
            caveat_requirement: Some("CUDA-001".to_string()),
        }
    }

    fn block_statistics(
        &self,
        _source: &dyn ModelSource,
        _descriptor: &TensorDescriptor,
        _extent: BlockExtent,
        _histogram_bins: usize,
    ) -> Result<TensorStatistics> {
        Err(Self::unimplemented("compute block statistics"))
    }

    fn matmul(&self, _a: &BlockData, _b: &BlockData) -> Result<BlockData> {
        Err(Self::unimplemented("multiply matrices"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backend_declares_itself_unverified_and_incapable() {
        let caps = CudaBackend::new().capabilities();
        assert_eq!(caps.backend_id, "cuda");
        assert!(!caps.hardware_verified);
        assert!(!caps.supports_statistics);
        assert!(!caps.supports_matmul);
        assert!(!caps.supports_histogram);
        assert_eq!(caps.caveat_requirement.as_deref(), Some("CUDA-001"));
        assert!(caps.display_name.contains("hardware-unverified"));
    }

    #[test]
    fn no_cuda_device_is_claimed_to_exist() {
        assert!(!CudaBackend::is_available());
    }

    #[test]
    fn every_operation_refuses_with_a_requirement_id_rather_than_faking_output() {
        let b = CudaBackend::new();
        let block = BlockData::new(2, 2, vec![1., 2., 3., 4.]).unwrap();
        let err = b.matmul(&block, &block).unwrap_err();
        assert_eq!(err.requirement_id(), Some("CUDA-001"));
        let msg = err.to_string();
        assert!(msg.contains("HARDWARE-UNVERIFIED"), "{msg}");
        assert!(msg.contains("CpuBackend"), "{msg}");
        assert!(msg.contains("gpu/cuda/matmul.cu"), "{msg}");
    }

    #[test]
    fn the_vram_ceiling_is_enforced_without_a_device() {
        use q_gpu::Workload;
        let b = CudaBackend::new();
        // ~19.2 GB usable of a 24 GB card.
        assert!(b.device_memory_bytes < RTX_3090_VRAM_BYTES);
        assert!(b.device_memory_bytes > 18 * 1024 * 1024 * 1024);

        // A trillion f32 parameters is ~4 TB: refused, and the refusal is
        // arithmetic on a declared limit, not a device query.
        let err = b
            .check_workload(Workload {
                element_count: 1_000_000_000_000,
                bytes_per_element: 4,
            })
            .unwrap_err();
        assert!(matches!(err, QError::BudgetExceeded { .. }));

        // A 4096x4096 f32 block (64 MB) fits comfortably.
        assert!(b
            .check_workload(Workload {
                element_count: 4096 * 4096,
                bytes_per_element: 4,
            })
            .is_ok());
    }

    #[test]
    fn kernel_sources_are_listed_and_exist_on_disk() {
        assert_eq!(KERNEL_SOURCES.len(), 4);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for src in KERNEL_SOURCES {
            let path = root.join(src);
            assert!(
                path.exists(),
                "{src} is referenced by q-cuda but missing from the repository"
            );
            // ...and each one must carry the unverified warning.
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text.contains("HARDWARE-UNVERIFIED"),
                "{src} must state that it has never been compiled or run"
            );
        }
    }
}
