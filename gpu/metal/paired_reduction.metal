// gpu/metal/paired_reduction.metal — the paired block reduction as a Metal
// compute kernel.
//
// Data plane: **Tensor Tile Plane** (ARCHITECTURE.md §2.1, §12.3).
// Requirement: `GPU-003`. Specified by
// `.plan/tasks/QM-0126-metal-backend-build/TASK.md` and
// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.
//
// This is the device half of `q_gpu::metal::MetalBackend`. The CPU half of the
// same reduction is `q_gpu::CpuBackend::reduce_paired_blocks`, which is the
// numerical reference; nothing here claims to agree with it. Proving that
// agreement is `QM-0127`, and until it passes
// `ComputeCapabilities::hardware_verified` reports `false`.
//
// ## Reduction order — fixed, and this is the statement of it
//
// Determinism is a requirement (`V1-13`: byte-identical output across runs),
// so the order below is part of the contract, not an implementation detail.
//
//   1. One **threadgroup per channel**. Threadgroup `g` reduces exactly the
//      elements of channel `g` and touches no other element, so channels never
//      interact and there is no cross-threadgroup accumulation of any kind.
//      There are no atomics anywhere in this file.
//   2. Within a threadgroup, `QM_THREADS` (256, a compile-time constant)
//      threads. Thread `t` visits its channel's elements at positions
//      `t, t + 256, t + 512, …` in **strictly increasing** position order and
//      accumulates them into private registers.
//   3. The 256 private accumulators are then combined by a **fixed binary tree**:
//      `for (stride = 128; stride > 0; stride >>= 1) lane[t] += lane[t + stride]`,
//      with a threadgroup barrier between levels. Lane `0` holds the result.
//
// Steps 2 and 3 depend on the threadgroup size, so the host **must** dispatch
// with exactly 256 threads per threadgroup, via
// `dispatchThreadgroups:threadsPerThreadgroup:` — never `dispatchThreads:`,
// which permits the driver to reshape the final threadgroup and would change
// the tree. `qm_metal_paired_reduction` refuses a device whose
// `maxTotalThreadsPerThreadgroup` is below 256 rather than silently shrinking.
//
// ## Precision — f32 on device, and the delta is formed in f32
//
// Metal has **no double precision**. Every accumulator here is `float`, and
// crucially `delta = base − counterpart` is subtracted in f32, whereas
// `CpuBackend` widens both operands to f64 *before* subtracting. That
// difference — not the reduction order — is the dominant source of divergence
// from the reference, and `QM-0127`'s tolerance is set against it rather than
// the other way round.
//
// The host widens each channel's five f32 outputs to f64 once, on readback.
// The whole-block partials are **not** re-summed from the per-channel results:
// the host issues a second dispatch of this same kernel over a single channel
// spanning the whole block, so the whole-block figures are independent of which
// channel axis was requested — the property
// `q_gpu::paired` names
// `the_whole_block_partials_are_bit_identical_whichever_axis_is_requested`.

#include <metal_stdlib>
using namespace metal;

/// Threads per threadgroup. Fixed: the reduction order above is only defined
/// for this value, and the host asserts the dispatch matches it.
constant uint QM_THREADS = 256;

/// Slots per channel in the output buffer, in this order.
constant uint QM_SLOTS = 5;

/// How to walk one channel. The host computes these so that one kernel serves
/// both channel axes and the whole-block pass:
///
/// | pass                | channel_count | elements_per_channel | element_stride | channel_stride |
/// | ------------------- | ------------- | -------------------- | -------------- | -------------- |
/// | `ChannelAxis::Rows` | rows          | columns              | 1              | columns        |
/// | `ChannelAxis::Columns` | columns    | rows                 | columns        | 1              |
/// | whole block         | 1             | rows × columns       | 1              | 0              |
struct QmPairedParams {
    uint channel_count;
    uint elements_per_channel;
    uint element_stride;
    uint channel_stride;
};

kernel void qm_paired_channel_reduction(
    device const float *base [[buffer(0)]],
    device const float *counterpart [[buffer(1)]],
    constant QmPairedParams &params [[buffer(2)]],
    device float *out [[buffer(3)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]])
{
    threadgroup float sum_sq_base[QM_THREADS];
    threadgroup float sum_sq_delta[QM_THREADS];
    threadgroup float sum_abs_delta[QM_THREADS];
    threadgroup float max_abs_delta[QM_THREADS];
    threadgroup float max_abs_base[QM_THREADS];

    float acc_sq_base = 0.0f;
    float acc_sq_delta = 0.0f;
    float acc_abs_delta = 0.0f;
    float acc_max_delta = 0.0f;
    float acc_max_base = 0.0f;

    // Step 2: strictly increasing position order within this thread's stripe.
    const uint origin = group * params.channel_stride;
    for (uint i = tid; i < params.elements_per_channel; i += QM_THREADS) {
        const uint index = origin + i * params.element_stride;
        const float b = base[index];
        const float delta = b - counterpart[index];
        const float abs_delta = fabs(delta);
        const float abs_base = fabs(b);
        acc_sq_base += b * b;
        acc_sq_delta += delta * delta;
        acc_abs_delta += abs_delta;
        acc_max_delta = max(acc_max_delta, abs_delta);
        acc_max_base = max(acc_max_base, abs_base);
    }

    sum_sq_base[tid] = acc_sq_base;
    sum_sq_delta[tid] = acc_sq_delta;
    sum_abs_delta[tid] = acc_abs_delta;
    max_abs_delta[tid] = acc_max_delta;
    max_abs_base[tid] = acc_max_base;

    // Step 3: fixed binary tree. The barrier is outside the `if`, because every
    // thread in the threadgroup must reach every barrier or the dispatch hangs.
    for (uint stride = QM_THREADS / 2; stride > 0; stride >>= 1) {
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (tid < stride) {
            sum_sq_base[tid] += sum_sq_base[tid + stride];
            sum_sq_delta[tid] += sum_sq_delta[tid + stride];
            sum_abs_delta[tid] += sum_abs_delta[tid + stride];
            max_abs_delta[tid] = max(max_abs_delta[tid], max_abs_delta[tid + stride]);
            max_abs_base[tid] = max(max_abs_base[tid], max_abs_base[tid + stride]);
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (tid == 0) {
        const uint slot = group * QM_SLOTS;
        out[slot + 0] = sum_sq_base[0];
        out[slot + 1] = sum_sq_delta[0];
        out[slot + 2] = sum_abs_delta[0];
        out[slot + 3] = max_abs_delta[0];
        out[slot + 4] = max_abs_base[0];
    }
}
