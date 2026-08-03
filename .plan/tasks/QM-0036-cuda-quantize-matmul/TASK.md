# QM-0036 — CUDA quantization, Morton, and matmul verification

## Status

Blocked

Unblocks when `QM-0035` reaches `Complete`. **Requires: RTX 3090.**

## Phase

Phase 03 — Block runtime and compute (Lane E)

## Objective

Verify `quantize.cu` and `matmul.cu` against the CPU reference, and demonstrate
out-of-memory adaptation on real hardware.

## Repository Evidence

* `gpu/cuda/quantize.cu`, `gpu/cuda/matmul.cu` — never compiled or executed
  (`CUDA-004`, `CUDA-005`).
* `q_gpu::CpuBackend` matmul — `hand_computed_matmul_2x3_by_3x2`,
  `hand_computed_matmul_edge_shapes` (`MATMUL-004` Verified).
* `q_tiles::dequantize_i16` (`crates/q-tiles/src/lib.rs:374`);
  `QTile::from_f32_quantized` (`:188`).
* `BlockEncoding::MortonSparseI16` — morton `u32` + quantized `i16` + flags
  `u16`, 8 B/cell.
* `q_cuda::check_workload` — the ceiling, verified without a device.

## Requirements Covered

`CUDA-004`, `CUDA-005`, `CUDA-008`.

## Dependencies

`QM-0035`.

## Blocks

`QM-0083`.

## Parallelization

Lane E. Blocks nothing on the critical path. **Requires: RTX 3090.**

## Program Boundary

`crates/q-cuda`, `gpu/cuda/`.

## Scope

* Quantization: f32 → i16 given min/max, diffed against
  `QTile::from_f32_quantized`.
* Morton encoding, diffed against a CPU implementation.
* Visual classification and normalization.
* Tiled block matmul at 64, 128, 256, 512, diffed against `CpuBackend`.
* Out-of-memory adaptation: force a failure, confirm halving and completion.
* Benchmarks.

## Out of Scope

Reductions and histograms (`QM-0035`) · leak soak (`QM-0083`) · optimization
beyond correctness.

## Files Expected to Change

* `gpu/cuda/quantize.cu`, `gpu/cuda/matmul.cu`
* `crates/q-cuda/src/lib.rs`

## Files Expected to Add

* `crates/q-cuda/tests/differential_quantize_matmul.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

| Operation | Tolerance |
| --- | --- |
| Quantized i16, given identical min/max | **exact** — integer arithmetic after normalization |
| Morton codes | **exact** — bit interleaving |
| Block matmul f32 | relative `1e-5` |

The matmul tolerance is looser than the reduction's because tiled accumulation
order differs and f32 FMA rounds differently — a documented consequence, not a
concession.

## Memory and Performance Constraints

Matmul working set: `(m×k + k×n + m×n) × 4`. At 512³ that is 3 MiB, well inside
`MAX_GPU_INPUT_BYTES`. Determinism per device requires fixed block and grid
dimensions and no float atomics.

## Implementation Plan

1. Wire both kernels into `CudaBackend`.
2. Differential harness reusing `QM-0035`'s structure.
3. Quantization: compare against `QTile::from_f32_quantized` byte for byte, then
   `dequantize_i16` round-trip.
4. Morton: compare against a CPU reference over the full 0..2¹⁶ coordinate space.
5. Matmul: compare against `CpuBackend` at four dimensions, including the edge
   shapes from `hand_computed_matmul_edge_shapes`.
6. OOM adaptation: constrain the budget artificially, confirm halving.
7. Benchmarks with FLOP/s.

## Error Handling

* Quantization divergence → fail naming the cell index and both values.
* Morton divergence → fail naming the coordinate.
* Matmul divergence beyond `1e-5` → fail naming `(i, j)` and both values.
* Shape mismatch → refused **before launch**, matching `WQL-004`'s discipline.
* OOM → halve to a floor of 64×64, then fail naming the budget.

## Acceptance Criteria

1. Quantized output is byte-identical to `QTile::from_f32_quantized`.
2. Dequantization round-trips within the encoding's declared loss.
3. Morton codes match exactly across the full coordinate space.
4. Matmul matches the CPU within `1e-5` at 64, 128, 256, 512.
5. Edge shapes (`m=1`, `n=1`, `k=1`) match.
6. A shape mismatch is refused before any launch.
7. Forced OOM halves the block and completes.
8. 100 runs of one matmul are bit-identical on the same device.
9. Benchmarks recorded with FLOP/s and device details.

## Verification Plan

**Automated** — the differential suite on an RTX 3090.
**Manual** — benchmark table reviewed; OOM demonstration captured.

## Suggested Commands

```bash
cargo test -p q-cuda --features cuda --test differential_quantize_matmul  # new
cargo bench -p q-cuda --features cuda -- matmul                            # new
nvidia-smi --query-gpu=name,driver_version --format=csv
```

## Test Cases

| Input | Expected |
| --- | --- |
| 256×256 f32 → i16 | Byte-identical to the CPU encoder |
| Dequantize round trip | Within declared loss |
| Morton over 0..2¹⁶ | Exact |
| 256³ matmul | Within `1e-5` of CPU |
| 64³, 128³, 512³ | Within `1e-5` |
| `2×3 @ 3×2` | Matches `hand_computed_matmul_2x3_by_3x2` |
| `2×3 @ 2×2` | Refused before launch |
| Budget forced below one block | Halves; completes |
| Budget below 64×64 | Fails naming the budget |
| 100 runs of one matmul | Bit-identical |

## Risks

| Risk | Mitigation |
| --- | --- |
| Tiled matmul is subtly wrong at edges | Edge shapes are explicit test cases |
| OOM adaptation is untestable without exhausting VRAM | The budget is a configuration variable; constrain it artificially |
| Small blocks are slower on GPU than CPU | **Recorded as a finding**; the backend selector gains a size threshold |

## Completion Evidence

* Differential output for all parameterizations.
* Morton exhaustive-comparison result.
* OOM adaptation transcript.
* Benchmark table with FLOP/s and the CPU baseline.
* `nvidia-smi` output.
