// Quatricmorph — block reduction kernels (min / max / sum / sum-of-squares).
//
// HARDWARE-UNVERIFIED: this file has never been compiled or executed. The
// target is an RTX 3090 (sm_86, 24 GB VRAM), which was unavailable when this
// was written. See gpu/cuda/README.md and requirement CUDA-001 in STATUS.md.
// The reference implementation is q_statistics::StatisticsAccumulator; any
// divergence between this and that is a bug here, by definition.
//
// Data plane: Tensor Tile Plane (ARCHITECTURE.md §2.1, §5.4, §12.3).

#include <cuda_runtime.h>
#include <cfloat>

// Partial results one block writes back. The host performs the final
// combination so that a reduction over a tensor larger than VRAM is a stream of
// these, never a single resident buffer.
struct QmPartialStats {
    float min_value;
    float max_value;
    double sum;          // double: f32 accumulation drifts badly past ~10^7 terms
    double sum_squares;
    unsigned long long count;
    unsigned long long zeros;
    unsigned long long positives;
    unsigned long long negatives;
};

// Reduce `count` f32 elements into one QmPartialStats per thread block.
//
//   input   device pointer to `count` contiguous f32 values (one selected block)
//   count   number of elements; must be <= the host's declared VRAM budget
//   out     device array of at least gridDim.x QmPartialStats
//
// Launch geometry: <<< ceil(count / (threads * ELEMENTS_PER_THREAD)), 256 >>>
// with shared memory = 256 * sizeof(QmPartialStats).
//
// Contract: a grid-stride loop, so any grid size is correct; the size only
// affects occupancy. Never launched with count == 0 (the host rejects empty
// blocks before reaching here).
extern "C" __global__ void qm_reduce_stats_f32(
    const float* __restrict__ input,
    unsigned long long count,
    QmPartialStats* __restrict__ out);

// Same reduction over bf16 storage, widened to f32 on load.
//
//   input   device pointer to `count` contiguous 16-bit bf16 values
//
// bf16 is the high half of an f32, so the widening is a shift and is exact —
// matching q_source::dtype::bf16_bits_to_f32.
extern "C" __global__ void qm_reduce_stats_bf16(
    const unsigned short* __restrict__ input,
    unsigned long long count,
    QmPartialStats* __restrict__ out);

// Min/max only, for the range pass that precedes histogram binning.
//
//   out_min, out_max   device arrays of at least gridDim.x floats
extern "C" __global__ void qm_reduce_minmax_f32(
    const float* __restrict__ input,
    unsigned long long count,
    float* __restrict__ out_min,
    float* __restrict__ out_max);
