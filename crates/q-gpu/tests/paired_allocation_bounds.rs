//! Acceptance criterion 6 — *"allocation is proportional to channel count, not
//! element count"* — **measured**, not asserted.
//!
//! `TASK.md` §Memory and Performance Constraints:
//!
//! ```text
//! allocation = per_channel.len() × size_of::<ChannelPartials>()
//!            = channels × 48 B
//! ```
//!
//! ## Why this binary holds exactly one test
//!
//! A `#[global_allocator]` counter sees every allocation in the whole test
//! binary, and `cargo test` runs a binary's tests on several threads. Two tests
//! here would race on the counter and the failure would be intermittent. So:
//! one test, one binary, and the counter is snapshotted immediately before each
//! measured call and read immediately after, with nothing allocating in between.
//!
//! ## Why the measurement is a difference rather than an absolute
//!
//! `Backend::check_workload` calls `capabilities()`, which builds two `String`s
//! every time it is asked. That is a genuine constant cost of the call and it is
//! deliberately not hidden here. Criterion 6 is a statement about what the
//! allocation *scales with*, so the assertions below isolate the scaling term:
//! doubling the elements at a fixed channel count must change the total by
//! **exactly zero bytes**, and doubling the channels at a fixed element count
//! must change it by **exactly `channels × 48`**. A constant term cannot pass
//! either test, and neither can a term proportional to elements.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use q_gpu::{Backend, BlockData, ChannelAxis, ChannelPartials, CpuBackend};

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

/// Reduce one pair and return `(allocations, bytes)` for that call alone.
fn measure(rows: usize, columns: usize, axis: ChannelAxis) -> (u64, usize) {
    // Every input allocation happens here, before the measurement window.
    let values: Vec<f32> = (0..rows * columns).map(|k| (k % 17) as f32 - 8.0).collect();
    let counterpart_values: Vec<f32> = values.iter().map(|v| v * 0.5).collect();
    let base = BlockData::new(rows, columns, values).unwrap();
    let counterpart = BlockData::new(rows, columns, counterpart_values).unwrap();

    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_before = BYTES.load(Ordering::Relaxed);
    let out = CpuBackend.paired_block_reduction(&base, &counterpart, axis);
    let allocations_after = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_after = BYTES.load(Ordering::Relaxed);

    let out = out.expect("the reduction succeeds");
    assert_eq!(out.per_channel.len(), axis.channel_count(rows, columns));
    assert_eq!(out.count as usize, rows * columns);
    (
        allocations_after - allocations_before,
        bytes_after - bytes_before,
    )
}

#[test]
fn paired_reduction_allocates_per_channel_and_never_per_element() {
    // The size the constraint is written in terms of.
    assert_eq!(
        std::mem::size_of::<ChannelPartials>(),
        48,
        "TASK.md states channels x 48 B; a change to ChannelPartials must change \
         that statement too"
    );

    // Warm the call once so no lazy one-off inside it lands in a measurement.
    let _ = measure(8, 8, ChannelAxis::Columns);

    // -- 1. Same channel count, 16x the elements: identical allocation. -------
    let (small_allocations, small_bytes) = measure(64, 64, ChannelAxis::Columns);
    let (large_allocations, large_bytes) = measure(1024, 64, ChannelAxis::Columns);
    assert_eq!(
        small_bytes, large_bytes,
        "4096 elements allocated {small_bytes} B and 65536 elements allocated \
         {large_bytes} B over the same 64 channels; the allocation is scaling \
         with element count"
    );
    assert_eq!(small_allocations, large_allocations);

    // -- 2. Twice the channels, same elements: exactly `channels x 48` more. --
    let (_, sixty_four_channels) = measure(128, 64, ChannelAxis::Columns);
    let (_, one_two_eight_channels) = measure(64, 128, ChannelAxis::Columns);
    assert_eq!(
        one_two_eight_channels - sixty_four_channels,
        64 * std::mem::size_of::<ChannelPartials>(),
        "going from 64 to 128 channels over the same 8192 elements moved the \
         allocation by {} B; expected exactly 64 x 48",
        one_two_eight_channels - sixty_four_channels
    );

    // -- 3. The same holds along the other axis. ------------------------------
    let (_, rows_64) = measure(64, 1024, ChannelAxis::Rows);
    let (_, rows_128) = measure(128, 512, ChannelAxis::Rows);
    assert_eq!(
        rows_128 - rows_64,
        64 * std::mem::size_of::<ChannelPartials>()
    );

    // -- 4. The absolute figure, against the block it reduced. ----------------
    // A 1024x64 block is 65536 f32 values, 262144 bytes. The reduction over 64
    // channels must be a small fraction of that, and must equal the per-channel
    // term plus the constant `capabilities()` overhead measured at step 1.
    let element_bytes = 1024 * 64 * std::mem::size_of::<f32>();
    assert!(
        large_bytes < element_bytes / 50,
        "the reduction allocated {large_bytes} B against a block of \
         {element_bytes} B; that is not independent of tensor size"
    );
    // The per-channel term is exactly 64 x 48 = 3072 B of the total, and the
    // remainder is the constant overhead, not an element-proportional term.
    let per_channel_term = 64 * std::mem::size_of::<ChannelPartials>();
    assert!(
        large_bytes >= per_channel_term,
        "{large_bytes} B is less than the {per_channel_term} B of partials it \
         returned, which cannot be"
    );
    let constant_overhead = large_bytes - per_channel_term;
    assert!(
        constant_overhead < 512,
        "the constant, non-scaling overhead is {constant_overhead} B, which is \
         larger than a handful of short strings and deserves an explanation"
    );

    // -- 5. And a 256-column block is the ~12 KB TASK.md predicts. ------------
    let (_, columns_256) = measure(256, 256, ChannelAxis::Columns);
    assert_eq!(columns_256 - constant_overhead, 256 * 48);
    assert!(
        columns_256 < 13_000,
        "TASK.md predicts ~12 KB for a 256-column block; measured {columns_256} B"
    );
}
