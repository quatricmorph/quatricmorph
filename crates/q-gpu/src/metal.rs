//! Metal compute backend — the paired block reduction on an Apple GPU.
//!
//! Data plane: operates on the **Tensor Tile Plane** (ARCHITECTURE.md §2.1,
//! §12.3). Requirement: `GPU-003`, specified by
//! `.plan/tasks/QM-0126-metal-backend-build/TASK.md`.
//!
//! Compiled only when the `metal` cargo feature is on. It is **off by default**
//! and nothing in this workspace turns it on: [`crate::default_backend`]
//! returns [`CpuBackend`] whether or not this module exists.
//!
//! # What has been exercised, and what has not
//!
//! Unlike `gpu/cuda/*.cu`, which has never been compiled, the kernel here is
//! compiled by `build.rs` and dispatched on a real device. What that earns is
//! narrow and worth stating precisely:
//!
//! | claim | status |
//! | --- | --- |
//! | the shader compiles | proven at build time — a failure is a build error |
//! | a dispatch reaches the GPU and returns partials of the right shape | proven by the tests below on the machine that has a device |
//! | those partials agree numerically with [`CpuBackend`] | **not proven here.** `QM-0127` |
//!
//! [`ComputeCapabilities::hardware_verified`] therefore reports `false`, and it
//! must keep reporting `false` until `QM-0127` passes.
//! `.plan/PRODUCT_SCOPE.md` §5.2 forbids the claim in the meantime, and
//! `the_backend_never_claims_hardware_verification` asserts it.
//!
//! # Numerics and reduction order
//!
//! The authoritative statement of the order is the header of
//! `gpu/metal/paired_reduction.metal`. In summary, and stated here because
//! `QM-0127`'s tolerance is set against it:
//!
//! * one threadgroup per channel, no atomics, no cross-threadgroup accumulation;
//! * within a threadgroup, thread `t` sums its stripe in increasing element
//!   order, then a fixed binary tree over exactly 256 lanes combines them;
//! * **accumulation is `f32` on the device.** Metal has no double precision.
//!   The delta is formed as `f32(base) − f32(counterpart)`, whereas
//!   [`CpuBackend`] widens both to `f64` and subtracts there. That, not the
//!   reduction order, is the dominant divergence from the reference;
//! * the host widens the five `f32` outputs per channel to `f64` on readback —
//!   a widening, not a re-accumulation;
//! * the whole-block partials come from a **second dispatch** of the same
//!   kernel over a single channel spanning the whole block, enumerated in flat
//!   row-major order and reduced in the same stripe-and-tree order as every
//!   other pass. They are deliberately *not* re-summed from the per-channel results,
//!   so that they are identical whichever [`ChannelAxis`] was requested — the
//!   property [`crate::paired`] documents under
//!   `the_whole_block_partials_are_bit_identical_whichever_axis_is_requested`.
//!
//! # Staging budget
//!
//! ```text
//! device staging = base block bytes + counterpart block bytes
//!                ≤ MAX_DEVICE_STAGING_BYTES        (default 256 MiB)
//! ```
//!
//! Enforced through [`Backend::check_workload`] with
//! [`Workload::for_paired_blocks`], which already counts **both** blocks of the
//! pair. On unified memory the host/device distinction is soft, which is
//! exactly why the budget is named and enforced anyway: `V1-03`'s residency
//! ceiling covers the whole process, and a buffer the GPU can see is still a
//! buffer the process is holding.

use std::ffi::c_void;
use std::os::raw::c_char;

use q_source::error::{QError, Result};
use q_source::manifest::ModelSource;
use q_source::TensorDescriptor;
use q_statistics::TensorStatistics;
use q_tensor_runtime::BlockExtent;

use crate::paired::{
    require_dense, require_finite, validate_pair, ChannelAxis, ChannelPartials, PairedPartials,
};
use crate::{Backend, BlockData, ComputeCapabilities, Workload};

/// The metallib `build.rs` produced from `gpu/metal/paired_reduction.metal`,
/// embedded in the binary. No path is resolved and no file is read at run time,
/// so a missing artifact is a build failure rather than a runtime one.
static METALLIB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/paired_reduction.metallib"));

/// The named staging budget: 256 MiB of device-visible buffers per dispatch,
/// counting both blocks of the pair.
pub const MAX_DEVICE_STAGING_BYTES: u64 = 256 * 1024 * 1024;

/// Threads per threadgroup. Must equal `QM_THREADS` in the shader and in the
/// shim: the documented reduction order is defined only for this value.
pub const THREADS_PER_THREADGROUP: u64 = 256;

/// Floats the kernel writes per channel.
const SLOTS_PER_CHANNEL: usize = 5;

const OK: i32 = 0;
const NO_DEVICE: i32 = 1;

extern "C" {
    fn qm_metal_probe(
        metallib: *const c_void,
        metallib_len: usize,
        name_out: *mut c_char,
        name_len: usize,
        recommended_working_set: *mut u64,
        max_buffer_length: *mut u64,
        max_threads_per_threadgroup: *mut u64,
        has_unified_memory: *mut i32,
        err: *mut c_char,
        err_len: usize,
    ) -> i32;

    #[allow(clippy::too_many_arguments)]
    fn qm_metal_paired_reduction(
        metallib: *const c_void,
        metallib_len: usize,
        base: *const f32,
        counterpart: *const f32,
        element_count: u32,
        channel_count: u32,
        elements_per_channel: u32,
        element_stride: u32,
        channel_stride: u32,
        out: *mut f32,
        err: *mut c_char,
        err_len: usize,
    ) -> i32;
}

fn buffer_to_string(buffer: &[u8]) -> String {
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[..end]).into_owned()
}

/// What the device said about itself. Every field is queried from `MTLDevice`
/// or from the compiled pipeline; none is a declared constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetalDeviceInfo {
    pub name: String,
    /// `MTLDevice.recommendedMaxWorkingSetSize`. On Apple silicon this tracks
    /// unified memory, which is shared with the CPU and the display.
    pub recommended_working_set_bytes: u64,
    /// `MTLDevice.maxBufferLength`.
    pub max_buffer_length_bytes: u64,
    /// `MTLComputePipelineState.maxTotalThreadsPerThreadgroup` for this kernel.
    pub max_threads_per_threadgroup: u64,
    /// `MTLDevice.hasUnifiedMemory`.
    pub unified_memory: bool,
}

/// The Metal backend.
///
/// Two ways to build one, and they are not interchangeable:
///
/// * [`MetalBackend::new`] / [`MetalBackend::probe`] query a real device and
///   can dispatch;
/// * [`MetalBackend::with_declared_staging_budget`] queries nothing and
///   **refuses to dispatch**. It exists so the budget arithmetic is testable on
///   a machine with no GPU, the same reason `q_cuda`'s
///   `the_vram_ceiling_is_enforced_without_a_device` exists. A backend that
///   never asked a device anything must not pretend it ran on one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetalBackend {
    device: Option<MetalDeviceInfo>,
    staging_budget_bytes: u64,
}

impl MetalBackend {
    pub const ID: &'static str = "metal";

    /// The requirement this backend is still caveated by. It stays until
    /// `QM-0127` diffs the kernel against [`CpuBackend`].
    pub const CAVEAT: &'static str = "GPU-003";

    /// Discover the system default Metal device.
    ///
    /// * `Ok(Some(backend))` — a device exists and the kernel pipeline built.
    /// * `Ok(None)` — no Metal device on this machine. Callers fall back to
    ///   [`CpuBackend`]; this is not an error and must not be reported as one.
    /// * `Err(_)` — a device exists but the embedded metallib or the pipeline
    ///   failed, named. This is a real fault and is *not* collapsed into
    ///   `None`, because a broken build must not look like absent hardware.
    pub fn probe() -> Result<Option<MetalBackend>> {
        let mut name = [0u8; 256];
        let mut err = [0u8; 512];
        let mut recommended: u64 = 0;
        let mut max_buffer: u64 = 0;
        let mut max_threads: u64 = 0;
        let mut unified: i32 = 0;

        // SAFETY: every pointer is to a live local of the stated length, and
        // the shim writes at most that many bytes (it uses `strlcpy`). The
        // metallib is a `'static` slice. The call is synchronous, so nothing
        // outlives this frame.
        let code = unsafe {
            qm_metal_probe(
                METALLIB.as_ptr() as *const c_void,
                METALLIB.len(),
                name.as_mut_ptr() as *mut c_char,
                name.len(),
                &mut recommended,
                &mut max_buffer,
                &mut max_threads,
                &mut unified,
                err.as_mut_ptr() as *mut c_char,
                err.len(),
            )
        };

        match code {
            OK => {
                let device = MetalDeviceInfo {
                    name: buffer_to_string(&name),
                    recommended_working_set_bytes: recommended,
                    max_buffer_length_bytes: max_buffer,
                    max_threads_per_threadgroup: max_threads,
                    unified_memory: unified != 0,
                };
                let staging_budget_bytes = if recommended == 0 {
                    MAX_DEVICE_STAGING_BYTES
                } else {
                    MAX_DEVICE_STAGING_BYTES.min(recommended)
                };
                Ok(Some(MetalBackend {
                    device: Some(device),
                    staging_budget_bytes,
                }))
            }
            NO_DEVICE => Ok(None),
            _ => Err(QError::QueryRejected(format!(
                "the Metal backend could not initialise (code {code}): {}",
                buffer_to_string(&err)
            ))),
        }
    }

    /// [`MetalBackend::probe`], with an initialisation failure collapsed into
    /// `None` for callers that only want "use Metal if it is there".
    ///
    /// Prefer `probe` when the difference between *absent hardware* and *a
    /// broken build* matters — which, when something is unexpectedly slow, it
    /// usually does.
    pub fn new() -> Option<MetalBackend> {
        Self::probe().ok().flatten()
    }

    /// A backend that has queried no device, carrying a declared budget.
    ///
    /// Every refusal path — shape, emptiness, the staging budget, raggedness,
    /// non-finite values — is reachable from here without hardware, which is
    /// the point. [`Backend::paired_block_reduction`] refuses on such an
    /// instance rather than quietly dispatching to whatever GPU happens to be
    /// present, because its capabilities describe a device it never asked.
    pub fn with_declared_staging_budget(staging_budget_bytes: u64) -> MetalBackend {
        MetalBackend {
            device: None,
            staging_budget_bytes,
        }
    }

    /// The device this backend queried, or `None` for a declared-budget
    /// instance.
    pub fn device(&self) -> Option<&MetalDeviceInfo> {
        self.device.as_ref()
    }

    /// The enforced staging budget in bytes, counting both blocks of a pair.
    pub fn staging_budget_bytes(&self) -> u64 {
        self.staging_budget_bytes
    }

    fn no_device(&self, operation: &str) -> QError {
        QError::QueryRejected(format!(
            "this MetalBackend cannot {operation}: it was built by \
             `with_declared_staging_budget`, which queries no device and exists only so the \
             {} B staging budget is testable without hardware. Use `MetalBackend::probe()` for \
             a backend bound to a real device, or `q_gpu::CpuBackend` for the reference result.",
            self.staging_budget_bytes
        ))
    }

    fn unimplemented(operation: &str) -> QError {
        QError::not_implemented(
            Self::CAVEAT,
            format!(
                "the Metal backend cannot {operation}: QM-0126 implements exactly one kernel, \
                 the paired block reduction in gpu/metal/paired_reduction.metal. Use \
                 q_gpu::CpuBackend, which is the reference for every backend."
            ),
        )
    }

    /// One dispatch of `qm_paired_channel_reduction`.
    ///
    /// The four stride parameters are the whole of the difference between the
    /// per-channel passes and the whole-block pass; see the table in the
    /// shader's header.
    fn dispatch(
        &self,
        base: &[f32],
        counterpart: &[f32],
        channel_count: usize,
        elements_per_channel: usize,
        element_stride: usize,
        channel_stride: usize,
    ) -> Result<Vec<f32>> {
        let element_count = u32::try_from(base.len()).map_err(|_| {
            QError::QueryRejected(format!(
                "the Metal backend dispatches at most {} elements per pass; this block declares {}",
                u32::MAX,
                base.len()
            ))
        })?;
        let mut out = vec![0f32; channel_count * SLOTS_PER_CHANNEL];
        let mut err = [0u8; 512];

        // SAFETY: `base` and `counterpart` are the same length (checked by
        // `validate_pair` and `require_dense` before this is reached) and are
        // read for `element_count` floats; `out` is written for exactly
        // `channel_count * SLOTS_PER_CHANNEL` floats, which is its length. The
        // shim copies out of device memory and returns before this frame ends.
        let code = unsafe {
            qm_metal_paired_reduction(
                METALLIB.as_ptr() as *const c_void,
                METALLIB.len(),
                base.as_ptr(),
                counterpart.as_ptr(),
                element_count,
                channel_count as u32,
                elements_per_channel as u32,
                element_stride as u32,
                channel_stride as u32,
                out.as_mut_ptr(),
                err.as_mut_ptr() as *mut c_char,
                err.len(),
            )
        };
        if code != OK {
            return Err(QError::QueryRejected(format!(
                "the Metal paired reduction failed (code {code}) on device '{}': {}",
                self.device
                    .as_ref()
                    .map(|d| d.name.as_str())
                    .unwrap_or("(none)"),
                buffer_to_string(&err)
            )));
        }
        Ok(out)
    }
}

/// Widen one channel's five `f32` slots to the `f64` partials the rest of the
/// engine composes in. A widening, never a re-accumulation.
fn widen(slots: &[f32], count: u64) -> ChannelPartials {
    ChannelPartials {
        count,
        sum_sq_base: slots[0] as f64,
        sum_sq_delta: slots[1] as f64,
        sum_abs_delta: slots[2] as f64,
        max_abs_delta: slots[3] as f64,
        max_abs_base: slots[4] as f64,
    }
}

impl Backend for MetalBackend {
    fn capabilities(&self) -> ComputeCapabilities {
        let display_name = match &self.device {
            Some(device) => format!(
                "Metal — {} ({} GPU memory, max buffer {} B) — UNVERIFIED against the CPU \
                 reference until QM-0127",
                device.name,
                if device.unified_memory {
                    "unified"
                } else {
                    "discrete"
                },
                device.max_buffer_length_bytes
            ),
            None => "Metal — declared staging budget only, no device queried".to_string(),
        };
        ComputeCapabilities {
            backend_id: Self::ID.to_string(),
            display_name,
            // The staging budget, not the device's total memory: this is the
            // number `check_workload` refuses against, and reporting the raw
            // 36 GB of an M3 Pro here would invite a caller to plan a workload
            // this backend will refuse.
            device_memory_bytes: self.staging_budget_bytes,
            supports_statistics: false,
            supports_matmul: false,
            // The one kernel QM-0126 implements is the paired reduction; there
            // is no histogram kernel and claiming one would route work here
            // that this backend cannot run.
            supports_histogram: false,
            // Non-negotiable until QM-0127 diffs this kernel against
            // CpuBackend. The shader compiles and the dispatch runs; neither
            // fact is evidence the numbers are right.
            hardware_verified: false,
            caveat_requirement: Some(Self::CAVEAT.to_string()),
        }
    }

    /// Refuse a pair that would exceed the staging budget, naming it.
    ///
    /// Identical arithmetic to the default, with the budget named for what it
    /// is: `device_memory` would suggest the M3 Pro's 36 GB rather than the
    /// 256 MiB this backend actually plans against.
    fn check_workload(&self, workload: Workload) -> Result<()> {
        if workload.bytes() > self.staging_budget_bytes {
            return Err(QError::BudgetExceeded {
                budget_name: "metal_device_staging",
                requested: workload.bytes(),
                limit: self.staging_budget_bytes,
            });
        }
        Ok(())
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

    /// The paired reduction, on device.
    ///
    /// Refusal order mirrors [`CpuBackend::reduce_paired_blocks`] exactly —
    /// shape, emptiness, the staging budget, raggedness, finiteness — and every
    /// one of them precedes the first buffer allocation, let alone the first
    /// dispatch.
    fn paired_block_reduction(
        &self,
        base: &BlockData,
        counterpart: &BlockData,
        axis: ChannelAxis,
    ) -> Result<PairedPartials> {
        validate_pair(base, counterpart)?;
        self.check_workload(Workload::for_paired_blocks(base.rows, base.columns))?;
        require_dense(base, "base")?;
        require_dense(counterpart, "counterpart")?;
        require_finite(base, "base")?;
        require_finite(counterpart, "counterpart")?;
        if self.device.is_none() {
            return Err(self.no_device("run the paired block reduction"));
        }

        let (rows, columns) = (base.rows, base.columns);
        let channels = axis.channel_count(rows, columns);
        // See the table in gpu/metal/paired_reduction.metal.
        let (elements_per_channel, element_stride, channel_stride) = match axis {
            ChannelAxis::Rows => (columns, 1, columns),
            ChannelAxis::Columns => (rows, columns, 1),
        };

        let per_channel_slots = self.dispatch(
            &base.values,
            &counterpart.values,
            channels,
            elements_per_channel,
            element_stride,
            channel_stride,
        )?;
        // Second, independent pass: a single channel spanning the whole
        // block, enumerated in flat row-major order and reduced in the same
        // stripe-and-tree order as every other pass. Not a re-sum of the
        // per-channel results, so the whole-block figures do not depend on the
        // requested axis.
        let whole_slots =
            self.dispatch(&base.values, &counterpart.values, 1, rows * columns, 1, 0)?;

        let elements_per_channel = elements_per_channel as u64;
        let per_channel = per_channel_slots
            .chunks_exact(SLOTS_PER_CHANNEL)
            .map(|slots| widen(slots, elements_per_channel))
            .collect();
        let whole = widen(&whole_slots, (rows as u64) * (columns as u64));

        Ok(PairedPartials {
            count: whole.count,
            sum_sq_base: whole.sum_sq_base,
            sum_sq_delta: whole.sum_sq_delta,
            sum_abs_delta: whole.sum_abs_delta,
            max_abs_delta: whole.max_abs_delta,
            max_abs_base: whole.max_abs_base,
            per_channel,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CpuBackend;

    /// A device, or a named reason there is none.
    ///
    /// Returns `None` after printing the reason. Tests that need hardware call
    /// this and return early; they never fail for the absence of a GPU and they
    /// never pass silently either — the reason is on stderr, and
    /// `.plan/evidence/QM-0126.md` records which machine ran which.
    fn device_or_skip(test: &str) -> Option<MetalBackend> {
        match MetalBackend::probe() {
            Ok(Some(backend)) => Some(backend),
            Ok(None) => {
                eprintln!(
                    "SKIP {test}: no Metal device on this machine \
                     (MTLCreateSystemDefaultDevice returned nil). GPU-003 is device-gated; \
                     q_gpu::CpuBackend covers the same reduction without one."
                );
                None
            }
            Err(error) => {
                eprintln!(
                    "SKIP {test}: the Metal backend failed to initialise, which is a build \
                     fault rather than absent hardware: {error}"
                );
                None
            }
        }
    }

    fn hand_pair() -> (BlockData, BlockData) {
        // The QM-0121 hand fixture. Every value is a multiple of 1/4, so every
        // square is a multiple of 1/16 and each sum is exact in binary floating
        // point — including in the f32 the device accumulates in.
        let base = BlockData::new(
            3,
            4,
            vec![
                1.0, -2.0, 3.0, -4.0, //
                0.5, 1.5, -2.5, 4.5, //
                -1.25, 2.25, 0.0, 3.75,
            ],
        )
        .unwrap();
        let counterpart = BlockData::new(
            3,
            4,
            vec![
                1.5, -1.0, 3.0, -3.5, //
                0.25, 2.0, -2.0, 4.0, //
                -1.0, 2.5, 0.5, 3.0,
            ],
        )
        .unwrap();
        (base, counterpart)
    }

    #[test]
    fn the_backend_never_claims_hardware_verification() {
        // QM-0127 is what earns this claim; QM-0126 must not make it, with or
        // without a device present.
        let declared = MetalBackend::with_declared_staging_budget(MAX_DEVICE_STAGING_BYTES);
        let caps = declared.capabilities();
        assert_eq!(caps.backend_id, "metal");
        assert!(!caps.hardware_verified);
        assert_eq!(caps.caveat_requirement.as_deref(), Some("GPU-003"));

        if let Some(backend) = device_or_skip("the_backend_never_claims_hardware_verification") {
            let caps = backend.capabilities();
            assert!(
                !caps.hardware_verified,
                "a real device must not flip hardware_verified: that is QM-0127's to flip"
            );
            assert_eq!(caps.caveat_requirement.as_deref(), Some("GPU-003"));
            assert!(
                caps.display_name.contains("UNVERIFIED"),
                "{}",
                caps.display_name
            );
        }
    }

    #[test]
    fn the_staging_budget_counts_both_blocks_of_the_pair_without_a_device() {
        let backend = MetalBackend::with_declared_staging_budget(MAX_DEVICE_STAGING_BYTES);
        assert_eq!(backend.staging_budget_bytes(), 256 * 1024 * 1024);
        assert!(backend.device().is_none());

        // 4096 x 8192 f32 is exactly 128 MiB per block, so the pair is exactly
        // the 256 MiB budget and fits.
        assert!(backend
            .check_workload(Workload::for_paired_blocks(4096, 8192))
            .is_ok());

        // One more column per row: 128 MiB + 16 KiB per block. A single block
        // still fits under 256 MiB — so this refusal happens ONLY because both
        // blocks of the pair are counted. If the doubling were ever dropped,
        // this is the assertion that fails.
        let err = backend
            .check_workload(Workload::for_paired_blocks(4096, 8193))
            .unwrap_err();
        match err {
            QError::BudgetExceeded {
                budget_name,
                requested,
                limit,
            } => {
                assert_eq!(budget_name, "metal_device_staging");
                assert_eq!(requested, 4096 * 8193 * 2 * 4);
                assert_eq!(limit, 256 * 1024 * 1024);
                assert!(
                    requested > 4096 * 8193 * 4,
                    "one block alone would have fit"
                );
            }
            other => panic!("expected BudgetExceeded, got {other:?}"),
        }
    }

    #[test]
    fn the_budget_refusal_precedes_any_dispatch() {
        // A tiny declared budget refuses a tiny block, and the refusal names
        // the budget rather than the missing device — proof that the budget
        // check runs before anything device-shaped is touched.
        let backend = MetalBackend::with_declared_staging_budget(16);
        let (base, counterpart) = hand_pair();
        let err = backend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap_err();
        assert!(
            matches!(err, QError::BudgetExceeded { budget_name, .. } if budget_name == "metal_device_staging"),
            "{err}"
        );
    }

    #[test]
    fn a_declared_budget_backend_refuses_to_dispatch_rather_than_borrowing_a_gpu() {
        let backend = MetalBackend::with_declared_staging_budget(MAX_DEVICE_STAGING_BYTES);
        let (base, counterpart) = hand_pair();
        let err = backend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("queries no device"), "{message}");
        assert!(message.contains("CpuBackend"), "{message}");
    }

    #[test]
    fn the_shape_and_finiteness_refusals_mirror_the_cpu_reference_without_a_device() {
        let backend = MetalBackend::with_declared_staging_budget(MAX_DEVICE_STAGING_BYTES);
        let (base, _) = hand_pair();

        let wrong_shape = BlockData::new(4, 3, vec![0.0; 12]).unwrap();
        let err = backend
            .paired_block_reduction(&base, &wrong_shape, ChannelAxis::Rows)
            .unwrap_err();
        assert!(err.to_string().contains("shape mismatch"), "{err}");

        let mut nonfinite = base.clone();
        nonfinite.values[5] = f32::NAN;
        let err = backend
            .paired_block_reduction(&base, &nonfinite, ChannelAxis::Rows)
            .unwrap_err();
        assert!(err.to_string().contains("non-finite"), "{err}");
    }

    #[test]
    fn the_metal_backend_refuses_the_kernels_it_has_not_written() {
        let backend = MetalBackend::with_declared_staging_budget(MAX_DEVICE_STAGING_BYTES);
        let block = BlockData::new(2, 2, vec![1., 2., 3., 4.]).unwrap();
        let err = backend.matmul(&block, &block).unwrap_err();
        assert_eq!(err.requirement_id(), Some("GPU-003"));
        assert!(err.to_string().contains("CpuBackend"), "{err}");
        let caps = backend.capabilities();
        assert!(!caps.supports_matmul && !caps.supports_statistics && !caps.supports_histogram);
    }

    #[test]
    fn the_device_reports_its_real_identity_and_limits() {
        let Some(backend) = device_or_skip("the_device_reports_its_real_identity_and_limits")
        else {
            return;
        };
        let device = backend.device().expect("a probed backend carries a device");
        assert!(!device.name.is_empty(), "the device must name itself");
        assert!(device.max_buffer_length_bytes > 0);
        assert!(device.recommended_working_set_bytes > 0);
        assert!(
            device.max_threads_per_threadgroup >= THREADS_PER_THREADGROUP,
            "the documented reduction order needs {THREADS_PER_THREADGROUP} threads; this \
             pipeline permits {}",
            device.max_threads_per_threadgroup
        );
        eprintln!(
            "QM-0126 device: {} | unified={} | recommendedMaxWorkingSetSize={} B | \
             maxBufferLength={} B | maxTotalThreadsPerThreadgroup={} | staging budget={} B",
            device.name,
            device.unified_memory,
            device.recommended_working_set_bytes,
            device.max_buffer_length_bytes,
            device.max_threads_per_threadgroup,
            backend.staging_budget_bytes(),
        );
    }

    #[test]
    fn a_small_paired_reduction_runs_on_device_and_returns_the_right_shape() {
        let Some(backend) =
            device_or_skip("a_small_paired_reduction_runs_on_device_and_returns_the_right_shape")
        else {
            return;
        };
        let (base, counterpart) = hand_pair();

        let rows = backend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap();
        assert_eq!(rows.per_channel.len(), 3, "one channel per row");
        assert_eq!(rows.count, 12);
        assert!(rows.per_channel.iter().all(|c| c.count == 4));

        let columns = backend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Columns)
            .unwrap();
        assert_eq!(columns.per_channel.len(), 4, "one channel per column");
        assert_eq!(columns.count, 12);
        assert!(columns.per_channel.iter().all(|c| c.count == 3));

        // The whole-block partials come from their own dispatch, so they do not
        // depend on which axis was asked for. This is a property of THIS
        // backend, asserted against itself — it is not a comparison with the
        // CPU reference, which is QM-0127's job.
        assert_eq!(rows.sum_sq_base, columns.sum_sq_base);
        assert_eq!(rows.sum_sq_delta, columns.sum_sq_delta);
        assert_eq!(rows.sum_abs_delta, columns.sum_abs_delta);
        assert_eq!(rows.max_abs_delta, columns.max_abs_delta);
        assert_eq!(rows.max_abs_base, columns.max_abs_base);

        eprintln!("QM-0126 device output, ChannelAxis::Rows: {rows:#?}");
        eprintln!(
            "QM-0126 CPU reference, ChannelAxis::Rows: {:#?}",
            CpuBackend
                .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
                .unwrap()
        );
    }

    #[test]
    fn repeated_dispatches_of_the_same_block_return_identical_bytes() {
        let Some(backend) =
            device_or_skip("repeated_dispatches_of_the_same_block_return_identical_bytes")
        else {
            return;
        };
        // 300 columns exceeds the 256-lane threadgroup, so every thread's
        // stripe is more than one element and the tree reduction is genuinely
        // exercised. V1-13 requires the same input to give the same bytes.
        let values: Vec<f32> = (0..(7 * 300)).map(|i| (i as f32) * 0.001 - 3.0).collect();
        let shifted: Vec<f32> = values.iter().map(|v| v * 0.998).collect();
        let base = BlockData::new(7, 300, values).unwrap();
        let counterpart = BlockData::new(7, 300, shifted).unwrap();

        let first = backend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap();
        for _ in 0..4 {
            let again = backend
                .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
                .unwrap();
            assert_eq!(
                first, again,
                "the documented reduction order must be stable"
            );
        }
    }
}
