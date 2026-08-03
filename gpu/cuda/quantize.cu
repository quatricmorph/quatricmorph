// Quatricmorph — value quantization for .qtile payloads.
//
// HARDWARE-UNVERIFIED: never compiled, never executed. Target RTX 3090
// (sm_86). See gpu/cuda/README.md and requirement CUDA-004 in STATUS.md.
// Reference: q_tiles::quantize_i16 / q_tiles::dequantize_i16.
//
// Data plane: Tensor Tile Plane (ARCHITECTURE.md §2.1, §10.3, §11.1).

#include <cuda_runtime.h>

// Map `count` f32 values in [min_value, max_value] onto the full i16 range.
//
//   input       device pointer to `count` f32 values
//   out         device pointer to `count` i16 values
//   min_value   lower edge of the tile's declared range
//   max_value   upper edge
//
// Launch geometry: <<< ceil(count / 256), 256 >>>
//
// Contract, identical to the CPU reference:
//   * t = clamp((x - min) / (max - min), 0, 1)
//   * q = round(t * 65535) - 32768, clamped to [-32768, 32767]
//   * max_value <= min_value yields 0 for every element
//
// This is LOSSY. Anything produced here must be labelled
// ResultFidelity::Approximate; it may never be presented as an exact weight
// value (ARCHITECTURE.md §18 AC-010).
extern "C" __global__ void qm_quantize_i16(
    const float* __restrict__ input,
    short* __restrict__ out,
    unsigned long long count,
    float min_value,
    float max_value);

// Pack cells into the Morton-ordered sparse layout of ARCHITECTURE.md §11.1:
//
//   struct VisualCell { u32 morton_coordinate; i16 quantized_value; u16 flags; u32 local_id; }
//
// Positions are NOT stored: the shader recomputes them as
//   position = tile_origin + decode_morton(morton_coordinate) * cell_spacing
// which is why ARCHITECTURE.md §19 forbids storing absolute positions per scalar.
//
//   values        device pointer to `count` f32 values, row-major within the tile
//   tile_rows     rows in the tile
//   tile_columns  columns in the tile
//   out_*         parallel output arrays, each of length `count`
extern "C" __global__ void qm_pack_morton_cells(
    const float* __restrict__ values,
    unsigned int tile_rows,
    unsigned int tile_columns,
    float min_value,
    float max_value,
    unsigned int* __restrict__ out_morton,
    short* __restrict__ out_quantized,
    unsigned short* __restrict__ out_flags,
    unsigned int* __restrict__ out_local_id);
