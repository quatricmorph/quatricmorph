//! Data plane: **Tensor Tile Plane** (ARCHITECTURE.md §2.1, §9.3).
//!
//! `PERF-001` / `TILE-009` — peak residency is bounded by **block size**, never
//! by tensor size.
//!
//! # What this test proves, and what it does not
//!
//! It proves that streaming a tensor's blocks through
//! [`q_tensor_runtime::stream::BlockStream`] allocates an amount of memory that
//! is a function of the configured block size and concurrency, and that the
//! amount is **unchanged** across tensors spanning a 4 096× range in element
//! count — including one whose payload would be 16 GiB.
//!
//! It does **not** prove gate `G1`. `G1` is `QM-0101`'s, and it needs a
//! configured residency ceiling that does not exist in this tree yet. It also
//! proves nothing about throughput: no time is measured here.
//!
//! # How memory is measured
//!
//! A counting `#[global_allocator]` wrapping the system allocator, tracking live
//! bytes and a high-water mark for the whole test binary. Exact and
//! deterministic, unlike sampling RSS. The pattern — and the caveat below about
//! there being exactly one `#[test]` — is taken from
//! `crates/q-catalog/tests/trillion_scale_manifest.rs` (`CAT-006`), which
//! measures the metadata plane the same way.
//!
//! **Why a synthetic source rather than a file.** The local source is
//! `mmap`-backed, and a memory map never reaches `GlobalAlloc`. On a mapped file
//! this allocator could not tell "streamed one block at a time" from "mapped the
//! whole file and copied one block at a time" — both report a small heap. The
//! source here hands every requested window over as a heap `Vec`, so every byte
//! the streamer receives is counted. `real_fixture_blocks.rs` covers the mapped
//! path for correctness; this file covers residency.

use q_source::error::{QError, Result};
use q_source::manifest::{ByteStream, ModelManifest, ModelSource};
use q_source::role::TensorRole;
use q_source::{DType, ModelId, TensorDescriptor, TensorId};
use q_tensor_runtime::stream::{BlockStream, BlockStreamConfig};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

// --- counting allocator ------------------------------------------------------

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            bump(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, new_size);
        if !p.is_null() {
            if new_size >= layout.size() {
                bump(new_size - layout.size());
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        p
    }
}

fn bump(n: usize) {
    let live = LIVE_BYTES.fetch_add(n, Ordering::Relaxed) + n;
    PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
}

#[global_allocator]
static ALLOC: CountingAllocator = CountingAllocator;

fn peak_bytes() -> usize {
    PEAK_BYTES.load(Ordering::Relaxed)
}

fn reset_peak() {
    PEAK_BYTES.store(LIVE_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
}

/// Measure the peak allocation `body` adds, above whatever is already live.
fn measure<T>(body: impl FnOnce() -> T) -> (T, usize) {
    reset_peak();
    let baseline = peak_bytes();
    let value = body();
    let peak = peak_bytes().saturating_sub(baseline);
    (value, peak)
}

// --- the declared ceiling ----------------------------------------------------

/// `.plan/tasks/QM-0030-…/TASK.md` acceptance criterion 2: peak allocation at
/// defaults must be ≤ 2 MiB, and unchanged at 1024², 2048² and 4096².
///
/// `.plan/MEMORY_BUDGET.md` §4 predicts the working set: four decoded 256×256
/// f32 blocks is 1 MiB. This ceiling is that prediction plus headroom for the
/// one 1 KiB run buffer and the harness's own bookkeeping — not a number chosen
/// after seeing the result.
const PEAK_CEILING_BYTES: usize = 2 * 1024 * 1024;

const HEADER_BYTES: u64 = 8;

// --- a source that materializes only the window asked for --------------------

struct SyntheticShard {
    payload_bytes: u64,
    reads: AtomicUsize,
    bytes_served: AtomicU64,
}

impl SyntheticShard {
    fn new(payload_bytes: u64) -> Self {
        Self {
            payload_bytes,
            reads: AtomicUsize::new(0),
            bytes_served: AtomicU64::new(0),
        }
    }

    /// Deterministic byte at an absolute offset. Defined in the test, so the
    /// expected values below never come from the code under test.
    fn byte_at(offset: u64) -> u8 {
        (offset.wrapping_mul(2_654_435_761) >> 11) as u8
    }
}

impl ModelSource for SyntheticShard {
    fn manifest(&self) -> Result<ModelManifest> {
        Err(QError::NotFound(
            "the residency harness describes a tensor, it does not publish a manifest".into(),
        ))
    }

    fn read_range(&self, uri: &str, offset: u64, length: u64) -> Result<ByteStream> {
        let file_length = HEADER_BYTES + self.payload_bytes;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| QError::RangeOutOfBounds {
                uri: uri.to_string(),
                start: offset,
                end: u64::MAX,
                length: file_length,
            })?;
        if end > file_length {
            return Err(QError::RangeOutOfBounds {
                uri: uri.to_string(),
                start: offset,
                end,
                length: file_length,
            });
        }
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.bytes_served.fetch_add(length, Ordering::Relaxed);
        // A heap buffer, exactly the size of the window, so the counting
        // allocator sees every byte the streamer is handed.
        let mut out = vec![0u8; length as usize];
        for (i, b) in out.iter_mut().enumerate() {
            *b = Self::byte_at(offset + i as u64);
        }
        Ok(ByteStream::from_vec(out))
    }
}

fn descriptor(edge: u64) -> TensorDescriptor {
    let elements = edge * edge;
    TensorDescriptor {
        tensor_id: TensorId::derive(ModelId::derive("synthetic:residency", "", "none"), "t"),
        raw_name: "t".into(),
        canonical_name: format!("synthetic.tensor[{edge}x{edge}]"),
        shape: vec![edge, edge],
        dtype: DType::F32,
        shard_uri: "model.safetensors".into(),
        byte_start: HEADER_BYTES,
        byte_end: HEADER_BYTES + elements * 4,
        layer_index: None,
        semantic_role: TensorRole::Unknown,
    }
}

#[test]
fn peak_allocation_is_bounded_by_block_size_and_does_not_grow_with_tensor_size() {
    let config = BlockStreamConfig::default();
    assert_eq!((config.block_rows, config.block_columns), (256, 256));
    assert_eq!(config.host_staging_bytes(), 1024 * 1024);

    // --- acceptance criterion 1 and 2: three full passes ---------------------
    //
    // 1024², 2048², 4096² — a 16× span in element count. The third is the
    // 4096×4096 f32 tensor of criterion 1: 256 blocks of 256×256.
    let mut measured: Vec<(u64, u64, u64, usize)> = Vec::new();
    for edge in [1024u64, 2048, 4096] {
        let d = descriptor(edge);
        let described_payload = d.byte_length();
        let shard = SyntheticShard::new(described_payload);

        // Construction is measured separately from the pass. `BlockStream::new`
        // probes the allocator for the declared staging budget (1 MiB at
        // defaults) and gives it straight back, so folding that into the
        // streaming window would hide the streaming loop's own cost behind it.
        let (mut stream, construction_peak) =
            measure(|| BlockStream::new(&shard, d.clone(), config).expect("stream"));
        assert!(
            construction_peak >= config.host_staging_bytes() as usize,
            "{edge}²: construction peak {construction_peak} did not include the \
             {} byte staging probe",
            config.host_staging_bytes()
        );
        assert_eq!(
            shard.reads.load(Ordering::Relaxed),
            0,
            "construction reads nothing"
        );

        let (totals, peak) = measure(|| {
            let mut blocks = 0u64;
            let mut elements = 0u64;
            let mut bytes = 0u64;
            let outcome = stream
                .drive(|block| {
                    blocks += 1;
                    elements += block.data.values.len() as u64;
                    bytes += block.bytes_read;
                    // Every block is exactly its own bytes and no more: 256
                    // rows × 256 columns × 4 bytes for an interior block.
                    assert_eq!(
                        block.bytes_read,
                        block.extent.element_count() * 4,
                        "a block must read exactly its own bytes"
                    );
                    Ok(())
                })
                .expect("full pass");
            assert!(outcome.is_complete());
            (blocks, elements, bytes)
        });

        let (blocks, elements, bytes) = totals;
        assert_eq!(
            elements,
            edge * edge,
            "every element of {edge}² was visited"
        );
        assert_eq!(bytes, described_payload, "exactly the payload was read");
        assert_eq!(
            shard.bytes_served.load(Ordering::Relaxed),
            described_payload
        );
        measured.push((edge, blocks, bytes, peak));
    }

    assert_eq!(
        measured.iter().map(|m| m.1).collect::<Vec<_>>(),
        vec![16, 64, 256],
        "block counts are ceil(edge/256)²"
    );

    for (edge, blocks, bytes, peak) in &measured {
        assert!(
            *peak <= PEAK_CEILING_BYTES,
            "{edge}²: peak allocation {peak} bytes exceeded the declared ceiling of \
             {PEAK_CEILING_BYTES} bytes over {blocks} blocks and {bytes} bytes read"
        );
    }

    // The load-bearing assertion: the peak does not track tensor size. A 16×
    // larger tensor must not cost meaningfully more resident memory. The
    // tolerance is 64 KiB — a quarter of one decoded block — which is loose
    // enough for allocator bookkeeping and far tighter than any size-dependent
    // implementation could pass.
    let smallest = measured[0].3;
    let largest = measured[2].3;
    let spread = largest.abs_diff(smallest);
    assert!(
        spread <= 64 * 1024,
        "peak allocation moved by {spread} bytes between 1024² ({smallest}) and \
         4096² ({largest}); it must be independent of tensor size"
    );

    // --- residency is independent of *declared* size too ---------------------
    //
    // A 65536² f32 tensor describes 16 GiB of payload. Streaming its first 16
    // blocks costs the same as streaming 16 blocks of anything else, because
    // the grid is arithmetic and the buffers are per block.
    let huge = descriptor(65_536);
    let huge_payload = huge.byte_length();
    assert_eq!(huge_payload, 16 * 1024 * 1024 * 1024);
    let shard = SyntheticShard::new(huge_payload);
    let mut stream = BlockStream::new(&shard, huge.clone(), config).expect("stream");
    assert_eq!(stream.block_count(), 65_536);
    let (partial, huge_peak) = measure(|| {
        let mut bytes = 0u64;
        for item in stream.by_ref().take(16) {
            bytes += item.expect("block").bytes_read;
        }
        bytes
    });
    assert_eq!(partial, 16 * 256 * 256 * 4);
    assert!(
        huge_peak <= PEAK_CEILING_BYTES,
        "streaming 16 blocks of a 16 GiB tensor peaked at {huge_peak} bytes"
    );

    // --- the bounded output queue stays inside the same ceiling --------------
    //
    // The consumer has to be slower than the reader for the queue to fill at all,
    // and the queue filling is what makes this the worst case: `capacity` decoded
    // blocks queued, one held by the reader while its `send` blocks, one held by
    // the sink.
    //
    // The stall is **front-loaded** — one long sleep on the first block, none
    // afterwards — rather than spread as a per-block sleep. A per-block sleep only
    // fills the queue if the reader can produce a second block inside one sleep,
    // which makes the phase fail on a *slow* machine: exactly the loaded CI runner
    // where a flake is least welcome. One 200 ms stall gives the reader (~8 ms per
    // 256×256 block in a debug build) room to queue all `capacity` blocks and park
    // in `send` with roughly 6× margin, and costs less wall time than the
    // 16 × 25 ms it replaces.
    let d = descriptor(1024);
    let shard = SyntheticShard::new(d.byte_length());
    let mut stream = BlockStream::new(&shard, d.clone(), config).expect("stream");
    let (outcome, queued_peak) = measure(|| {
        let mut seen = 0u32;
        stream
            .drive_bounded(|_| {
                if seen == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                seen += 1;
                Ok(())
            })
            .expect("bounded pass")
    });
    assert_eq!(outcome.blocks_emitted, 16);
    // `capacity` queued + one in the reader's hand while `send` blocks + one in
    // the sink's. That is the worst-case live decoded block count, and the peak
    // measured below is exactly that many blocks.
    assert!(
        outcome.queue_high_water <= outcome.queue_capacity + 2,
        "queue high water {} exceeded capacity {} + 2",
        outcome.queue_high_water,
        outcome.queue_capacity
    );
    assert!(
        outcome.queue_high_water > 1,
        "the queue never filled, so this phase did not measure the worst case \
         (high water {})",
        outcome.queue_high_water
    );
    assert!(
        queued_peak <= PEAK_CEILING_BYTES,
        "the bounded-queue pass peaked at {queued_peak} bytes, ceiling {PEAK_CEILING_BYTES}"
    );

    eprintln!(
        "PERF-001 (exact, counting allocator): peak allocation \
         1024² {} B / 2048² {} B / 4096² {} B; \
         16 blocks of a 65536² (16 GiB) tensor {} B; \
         bounded-queue 1024² with a stalled consumer {} B \
         (queue high water {} of capacity {}); ceiling {} B",
        measured[0].3,
        measured[1].3,
        measured[2].3,
        huge_peak,
        queued_peak,
        outcome.queue_high_water,
        outcome.queue_capacity,
        PEAK_CEILING_BYTES,
    );
}

// NOTE: there is deliberately exactly one `#[test]` in this file.
//
// The counting allocator is process-wide, so a second test running on another
// thread would allocate inside this one's measurement window and make the
// ceiling assertion nondeterministic. Every phase of the residency claim is
// folded into the single test above rather than split out — the same constraint
// `crates/q-catalog/tests/trillion_scale_manifest.rs` records for `CAT-006`.
