// Quatricmorph — fixed-range histogram kernel.
//
// HARDWARE-UNVERIFIED: never compiled, never executed. Target RTX 3090
// (sm_86). See gpu/cuda/README.md and requirement CUDA-002 in STATUS.md.
// Reference: q_statistics::Histogram::bin_of.
//
// Data plane: Tensor Tile Plane (ARCHITECTURE.md §2.1, §5.4).

#include <cuda_runtime.h>

// Maximum bins a single kernel launch supports, bounded by shared memory:
// 256 bins x 4 bytes = 1 KB per block, comfortably inside 48 KB.
#define QM_MAX_HISTOGRAM_BINS 256

// Bin `count` f32 values into `bins` buckets spanning [min_value, max_value].
//
//   input       device pointer to `count` contiguous f32 values
//   count       element count
//   min_value   inclusive lower edge (from the preceding range pass)
//   max_value   inclusive upper edge
//   bins        1 <= bins <= QM_MAX_HISTOGRAM_BINS
//   out         device array of `bins` u32 counters, zeroed by the host
//
// Launch geometry: <<< ceil(count / 256), 256, bins * sizeof(unsigned int) >>>
//
// Binning contract, identical to the CPU reference:
//   * t = (x - min) / (max - min), bin = floor(t * bins)
//   * a value exactly equal to max_value lands in the LAST bin, not out of range
//   * values outside [min, max] clamp to the first or last bin
//   * max_value <= min_value puts everything in bin 0 (no division by zero)
//
// Per-block shared-memory accumulation, one global atomicAdd per bin per block.
extern "C" __global__ void qm_histogram_f32(
    const float* __restrict__ input,
    unsigned long long count,
    float min_value,
    float max_value,
    unsigned int bins,
    unsigned int* __restrict__ out);

// bf16 variant; widening to f32 is exact (a 16-bit shift).
extern "C" __global__ void qm_histogram_bf16(
    const unsigned short* __restrict__ input,
    unsigned long long count,
    float min_value,
    float max_value,
    unsigned int bins,
    unsigned int* __restrict__ out);
