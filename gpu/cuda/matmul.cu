// Quatricmorph — tiled block matrix multiplication.
//
// HARDWARE-UNVERIFIED: never compiled, never executed. Target RTX 3090
// (sm_86). See gpu/cuda/README.md and requirement CUDA-003 in STATUS.md.
// Reference: q_gpu::CpuBackend::matmul.
//
// Scope note (ARCHITECTURE.md §8): Quatricmorph does NOT multiply an entire
// matrix by default. This kernel operates on a SELECTED BLOCK — the
// `A[0:256, 0:256] @ B[0:256, 0:256]` of §8.1 Tensor Block Mode. Full Compute
// Mode runs only on explicit user request after a cost estimate is shown.
//
// Data plane: Tensor Tile Plane (ARCHITECTURE.md §2.1, §12.3).

#include <cuda_runtime.h>

// Shared-memory tile edge. 32x32 f32 = 4 KB per operand tile, 8 KB per block,
// which keeps occupancy reasonable on sm_86's 100 KB of shared memory per SM.
#define QM_MATMUL_TILE 32

// C = A @ B for row-major blocks.
//
//   a    device pointer to m*k f32, row-major
//   b    device pointer to k*n f32, row-major
//   c    device pointer to m*n f32, row-major, written (not accumulated)
//   m    rows of A and C
//   k    inner dimension; A's columns must equal B's rows
//   n    columns of B and C
//
// Launch geometry:
//   dim3 threads(QM_MATMUL_TILE, QM_MATMUL_TILE);
//   dim3 grid(ceil(n / QM_MATMUL_TILE), ceil(m / QM_MATMUL_TILE));
//
// Contract:
//   * the host has already shape-checked k; this kernel does not validate
//     (WeightQL rejects mismatches before execution — ARCHITECTURE.md §7.4)
//   * m*n*4 bytes must fit the declared VRAM budget; the host checks via
//     q_cuda::CudaBackend::check_workload
//   * edge tiles are zero-padded in shared memory, so non-multiples of
//     QM_MATMUL_TILE are correct without a separate epilogue kernel
extern "C" __global__ void qm_matmul_f32(
    const float* __restrict__ a,
    const float* __restrict__ b,
    float* __restrict__ c,
    unsigned int m,
    unsigned int k,
    unsigned int n);

// C = A @ B^T, for the attention-style Q @ transpose(K) case.
//
// A separate kernel rather than a transpose pass: materializing B^T would
// double the VRAM footprint of the operand for no arithmetic benefit.
// ARCHITECTURE.md §7.4 requires transposes to be *explicitly declared* in the
// query, which is what selects this kernel over qm_matmul_f32.
extern "C" __global__ void qm_matmul_f32_bt(
    const float* __restrict__ a,
    const float* __restrict__ b,
    float* __restrict__ c,
    unsigned int m,
    unsigned int k,
    unsigned int n);
