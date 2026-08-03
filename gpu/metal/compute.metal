#include <metal_stdlib>
using namespace metal;

kernel void compute_kernel(
    device float* data [[buffer(0)]],
    uint idx [[thread_position_in_grid]]) {
    data[idx] = data[idx] * 2.0;
}
