//! Acceptance criterion 6 — *"`simulate` allocates nothing proportional to
//! tensor size"* — **measured**, not asserted.
//!
//! `TASK.md` §Memory and Performance Constraints: *"Allocation is `O(unit size)`,
//! never `O(tensor)`. `simulate` writes into a caller-provided buffer where the
//! caller has one, so the streaming pass reuses buffers across blocks."*
//!
//! ## Why this binary holds exactly one test
//!
//! A `#[global_allocator]` counter sees every allocation in the whole test
//! binary, and `cargo test` runs a binary's tests on several threads. Two tests
//! here would race on the counter and the failure would be intermittent. So:
//! one test, one binary, and the counter is snapshotted immediately before the
//! measured call and read immediately after, with nothing allocating in between.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use q_quant::{simulate, simulate_into, Precision, QuantConfig, QuantParams, ZeroPoint};

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The unit size `TASK.md` §Test Cases row 9 names, and the tensor it must NOT
/// scale with: a 4096×4096 tensor is 16 777 216 values, 67 108 864 bytes of f32.
const UNIT: usize = 4096;
const TENSOR_VALUES: usize = 4096 * 4096;

#[test]
fn simulating_a_4096_element_unit_allocates_per_unit_and_never_per_tensor() {
    let config = QuantConfig::per_tensor(Precision::Int8, ZeroPoint::Symmetric);
    let params = QuantParams::new(1.0 / 127.0, 0);

    // Everything that allocates happens up front, outside every measurement.
    let values: Vec<f32> = (0..UNIT)
        .map(|i| (i as f32 - UNIT as f32 / 2.0) / UNIT as f32)
        .collect();
    let mut buffer = vec![0.0f32; UNIT];
    // Warm the call once so no lazy one-off inside it lands in a measurement.
    simulate_into(&values, &params, &config, &mut buffer).expect("warm-up");

    // --- 1. `simulate_into` must allocate NOTHING at all. -------------------
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let result = simulate_into(&values, &params, &config, &mut buffer);
    let after = ALLOCATIONS.load(Ordering::Relaxed);
    result.expect("simulate_into succeeds");
    assert_eq!(
        after - before,
        0,
        "simulate_into performed {} allocation(s); the streaming pass in QM-0122 \
         reuses one buffer across every block and cannot afford any",
        after - before
    );

    // --- 2. Ten more blocks through the same buffer, still nothing. ---------
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for _ in 0..10 {
        simulate_into(&values, &params, &config, &mut buffer).expect("block");
    }
    let after = ALLOCATIONS.load(Ordering::Relaxed);
    assert_eq!(
        after - before,
        0,
        "streaming 10 blocks through one buffer allocated {} time(s)",
        after - before
    );

    // --- 3. `simulate` allocates exactly one buffer, sized by the UNIT. -----
    let bytes_before = BYTES.load(Ordering::Relaxed);
    let allocs_before = ALLOCATIONS.load(Ordering::Relaxed);
    let out = simulate(&values, &params, &config);
    let allocs_after = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_after = BYTES.load(Ordering::Relaxed);
    let out = out.expect("simulate succeeds");
    assert_eq!(out.len(), UNIT);

    let allocations = allocs_after - allocs_before;
    let bytes = bytes_after - bytes_before;
    assert_eq!(
        allocations, 1,
        "simulate performed {allocations} allocations; it must allocate exactly \
         one output buffer"
    );
    assert_eq!(
        bytes,
        UNIT * std::mem::size_of::<f32>(),
        "simulate allocated {bytes} bytes for a {UNIT}-value unit; expected \
         exactly the output buffer"
    );

    // The claim acceptance criterion 6 actually makes: the allocation is a
    // function of the UNIT, not of the tensor the unit came from.
    let tensor_bytes = TENSOR_VALUES * std::mem::size_of::<f32>();
    assert!(
        bytes < tensor_bytes / 1000,
        "simulate allocated {bytes} bytes, which is not negligible against the \
         {tensor_bytes} bytes of a 4096x4096 f32 tensor"
    );
}
