// gpu/metal/qm_metal_shim.m — the Objective-C host side of the Metal backend.
//
// Data plane: **Tensor Tile Plane** (ARCHITECTURE.md §2.1, §12.3).
// Requirement: `GPU-003`. Compiled only by `crates/q-gpu/build.rs` when the
// `metal` cargo feature is on; absent from every default build.
//
// ## Why an Objective-C shim and not a crates.io binding
//
// The Metal API is Objective-C only. The alternative was a third-party Rust
// binding, which would add packages to `Cargo.lock` for every consumer of this
// workspace whether or not they enable the feature, and would put an
// unaudited transitive tree in front of `scripts/license-audit.sh`. This file
// is ~200 lines of Objective-C against Apple's own SDK, compiled by `clang`
// straight from `build.rs`, and it leaves `Cargo.lock` byte-identical. The
// deviation from the task's `Files Expected to Add` is recorded in
// `.plan/evidence/QM-0126.md`.
//
// ## Shape of the interface
//
// Deliberately **stateless and C-flat**: no opaque handle is returned across
// the FFI boundary. Under `-fobjc-arc` a `malloc`'d C struct may not hold
// `__strong` object pointers, and there is exactly one system default device,
// so the device, the library and the pipeline are file-scope statics built
// once under `dispatch_once`. The Rust side holds no pointer it must free.
//
// Every entry point returns one of the `QM_METAL_*` codes below and writes a
// human-readable reason into `err`. `QM_METAL_NO_DEVICE` is the one code that
// is *not* a failure: it is the machine honestly reporting that it has no
// Metal GPU, and `MetalBackend::new` turns it into `None`.

#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <dispatch/dispatch.h>
#include <stdint.h>
#include <string.h>

#define QM_METAL_OK 0
#define QM_METAL_NO_DEVICE 1
#define QM_METAL_LIBRARY_FAILED 2
#define QM_METAL_THREADGROUP_TOO_SMALL 3
#define QM_METAL_ALLOCATION_FAILED 4
#define QM_METAL_DISPATCH_FAILED 5
#define QM_METAL_BAD_ARGUMENT 6

/// Threads per threadgroup. Must equal `QM_THREADS` in
/// `gpu/metal/paired_reduction.metal`: the kernel's documented reduction order
/// is defined only for this value.
#define QM_THREADS 256

/// Floats written per channel by the kernel.
#define QM_SLOTS 5

static id<MTLDevice> g_device = nil;
static id<MTLCommandQueue> g_queue = nil;
static id<MTLComputePipelineState> g_pipeline = nil;
static NSString *g_setup_error = nil;
static int32_t g_setup_code = QM_METAL_OK;

static void qm_copy_error(char *err, size_t err_len, NSString *message) {
    if (err == NULL || err_len == 0) {
        return;
    }
    const char *utf8 = message ? [message UTF8String] : "unknown Metal error";
    if (utf8 == NULL) {
        utf8 = "unknown Metal error";
    }
    strlcpy(err, utf8, err_len);
}

/// Build the device, queue and pipeline exactly once.
///
/// `metallib` is the metallib `build.rs` produced at compile time, embedded in
/// the Rust binary with `include_bytes!` — there is no path lookup and no file
/// read at run time, so a moved or missing artifact is a build failure rather
/// than a runtime one.
static int32_t qm_metal_setup(const void *metallib, size_t metallib_len, char *err, size_t err_len) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        @autoreleasepool {
            g_device = MTLCreateSystemDefaultDevice();
            if (g_device == nil) {
                g_setup_code = QM_METAL_NO_DEVICE;
                g_setup_error = @"no Metal device: MTLCreateSystemDefaultDevice() returned nil";
                return;
            }
            if (metallib == NULL || metallib_len == 0) {
                g_setup_code = QM_METAL_LIBRARY_FAILED;
                g_setup_error = @"the embedded metallib is empty";
                return;
            }
            dispatch_data_t data = dispatch_data_create(
                metallib, metallib_len, dispatch_get_main_queue(), DISPATCH_DATA_DESTRUCTOR_DEFAULT);
            NSError *error = nil;
            id<MTLLibrary> library = [g_device newLibraryWithData:data error:&error];
            if (library == nil) {
                g_setup_code = QM_METAL_LIBRARY_FAILED;
                g_setup_error = [NSString
                    stringWithFormat:@"newLibraryWithData failed for the metallib built from "
                                     @"gpu/metal/paired_reduction.metal: %@",
                                     error ? [error localizedDescription] : @"(no NSError)"];
                return;
            }
            id<MTLFunction> function = [library newFunctionWithName:@"qm_paired_channel_reduction"];
            if (function == nil) {
                g_setup_code = QM_METAL_LIBRARY_FAILED;
                g_setup_error = @"gpu/metal/paired_reduction.metal defines no kernel named "
                                @"qm_paired_channel_reduction";
                return;
            }
            g_pipeline = [g_device newComputePipelineStateWithFunction:function error:&error];
            if (g_pipeline == nil) {
                g_setup_code = QM_METAL_LIBRARY_FAILED;
                g_setup_error = [NSString
                    stringWithFormat:@"newComputePipelineStateWithFunction failed for "
                                     @"qm_paired_channel_reduction: %@",
                                     error ? [error localizedDescription] : @"(no NSError)"];
                return;
            }
            g_queue = [g_device newCommandQueue];
            if (g_queue == nil) {
                g_setup_code = QM_METAL_ALLOCATION_FAILED;
                g_setup_error = @"newCommandQueue returned nil";
                return;
            }
        }
    });
    if (g_setup_code != QM_METAL_OK) {
        qm_copy_error(err, err_len, g_setup_error);
    }
    return g_setup_code;
}

/// Report the real device. Every figure is queried from `MTLDevice`; nothing
/// here is a declared constant standing in for hardware.
int32_t qm_metal_probe(const void *metallib,
                       size_t metallib_len,
                       char *name_out,
                       size_t name_len,
                       uint64_t *recommended_working_set,
                       uint64_t *max_buffer_length,
                       uint64_t *max_threads_per_threadgroup,
                       int32_t *has_unified_memory,
                       char *err,
                       size_t err_len) {
    int32_t code = qm_metal_setup(metallib, metallib_len, err, err_len);
    if (code != QM_METAL_OK) {
        return code;
    }
    @autoreleasepool {
        if (name_out != NULL && name_len > 0) {
            strlcpy(name_out, [[g_device name] UTF8String], name_len);
        }
        if (recommended_working_set != NULL) {
            *recommended_working_set = (uint64_t)[g_device recommendedMaxWorkingSetSize];
        }
        if (max_buffer_length != NULL) {
            *max_buffer_length = (uint64_t)[g_device maxBufferLength];
        }
        if (max_threads_per_threadgroup != NULL) {
            *max_threads_per_threadgroup = (uint64_t)[g_pipeline maxTotalThreadsPerThreadgroup];
        }
        if (has_unified_memory != NULL) {
            *has_unified_memory = [g_device hasUnifiedMemory] ? 1 : 0;
        }
    }
    return QM_METAL_OK;
}

/// Run one pass of `qm_paired_channel_reduction`.
///
/// `out` must hold `channel_count * QM_SLOTS` floats. The caller has already
/// enforced the staging budget; this function allocates exactly the two input
/// buffers and one output buffer and nothing else.
int32_t qm_metal_paired_reduction(const void *metallib,
                                  size_t metallib_len,
                                  const float *base,
                                  const float *counterpart,
                                  uint32_t element_count,
                                  uint32_t channel_count,
                                  uint32_t elements_per_channel,
                                  uint32_t element_stride,
                                  uint32_t channel_stride,
                                  float *out,
                                  char *err,
                                  size_t err_len) {
    int32_t code = qm_metal_setup(metallib, metallib_len, err, err_len);
    if (code != QM_METAL_OK) {
        return code;
    }
    if (base == NULL || counterpart == NULL || out == NULL || element_count == 0 ||
        channel_count == 0 || elements_per_channel == 0) {
        qm_copy_error(err, err_len, @"null buffer or zero extent passed to the paired reduction");
        return QM_METAL_BAD_ARGUMENT;
    }
    if ([g_pipeline maxTotalThreadsPerThreadgroup] < QM_THREADS) {
        qm_copy_error(
            err, err_len,
            [NSString stringWithFormat:
                          @"this device permits only %lu threads per threadgroup; the documented "
                          @"reduction order in gpu/metal/paired_reduction.metal is defined only "
                          @"for %d, and shrinking it silently would change the result",
                          (unsigned long)[g_pipeline maxTotalThreadsPerThreadgroup], QM_THREADS]);
        return QM_METAL_THREADGROUP_TOO_SMALL;
    }

    @autoreleasepool {
        const size_t input_bytes = (size_t)element_count * sizeof(float);
        const size_t output_bytes = (size_t)channel_count * QM_SLOTS * sizeof(float);
        id<MTLBuffer> base_buffer = [g_device newBufferWithBytes:base
                                                          length:input_bytes
                                                         options:MTLResourceStorageModeShared];
        id<MTLBuffer> counterpart_buffer =
            [g_device newBufferWithBytes:counterpart
                                  length:input_bytes
                                 options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_buffer = [g_device newBufferWithLength:output_bytes
                                                        options:MTLResourceStorageModeShared];
        if (base_buffer == nil || counterpart_buffer == nil || out_buffer == nil) {
            qm_copy_error(err, err_len,
                          @"MTLDevice could not allocate the staging buffers for this dispatch");
            return QM_METAL_ALLOCATION_FAILED;
        }

        uint32_t params[4] = {channel_count, elements_per_channel, element_stride, channel_stride};

        id<MTLCommandBuffer> command_buffer = [g_queue commandBuffer];
        id<MTLComputeCommandEncoder> encoder = [command_buffer computeCommandEncoder];
        [encoder setComputePipelineState:g_pipeline];
        [encoder setBuffer:base_buffer offset:0 atIndex:0];
        [encoder setBuffer:counterpart_buffer offset:0 atIndex:1];
        [encoder setBytes:params length:sizeof(params) atIndex:2];
        [encoder setBuffer:out_buffer offset:0 atIndex:3];
        // dispatchThreadgroups, never dispatchThreads: the latter lets the
        // driver reshape the final threadgroup, which would change the tree
        // reduction the kernel documents.
        [encoder dispatchThreadgroups:MTLSizeMake(channel_count, 1, 1)
                threadsPerThreadgroup:MTLSizeMake(QM_THREADS, 1, 1)];
        [encoder endEncoding];
        [command_buffer commit];
        [command_buffer waitUntilCompleted];

        if ([command_buffer status] != MTLCommandBufferStatusCompleted) {
            NSError *error = [command_buffer error];
            qm_copy_error(err, err_len,
                          [NSString stringWithFormat:
                                        @"the command buffer on device '%@' ended with status %ld: "
                                        @"%@ — no partial output is returned",
                                        [g_device name], (long)[command_buffer status],
                                        error ? [error localizedDescription] : @"(no NSError)"]);
            return QM_METAL_DISPATCH_FAILED;
        }
        memcpy(out, [out_buffer contents], output_bytes);
    }
    return QM_METAL_OK;
}
