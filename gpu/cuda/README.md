# CUDA kernels — HARDWARE-UNVERIFIED

Every `.cu` file in this directory is **source only**. None has been compiled,
linked, or executed. There is no `nvcc` step in the build, no `build.rs`, and no
FFI binding in `crates/q-cuda` — that crate returns `NotImplemented` for every
operation (requirement `CUDA-001`).

The target device is an **RTX 3090, 24 GB VRAM**, which was not available when
these files were written. Treat every performance or numerical claim below as an
intention, not a measurement.

## How to make these verified

1. Build with `nvcc -arch=sm_86` (Ampere / RTX 3090).
2. Add a `build.rs` to `crates/q-cuda` that compiles and links them.
3. Implement `q_gpu::Backend` for `CudaBackend` against the real launches.
4. Diff every kernel's output against `q_gpu::CpuBackend` on
   `fixtures/tiny-llama-2shard`. The CPU backend is the reference; a divergence
   is a bug in the kernel.
5. Only then may `STATUS.md` move `CUDA-*` from `Hardware-Unverified`.

## VRAM discipline

No kernel here may assume a whole tensor is resident. A trillion-parameter
checkpoint is terabytes; 24 GB holds a few thousand 4096×4096 f32 blocks at most.
Every kernel is written to operate on **one selected block**, streamed in by the
host. `q_cuda::CudaBackend::check_workload` enforces the ceiling before a launch
would be attempted.
