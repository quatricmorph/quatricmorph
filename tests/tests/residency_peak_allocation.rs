//! `QM-0101` — the **exact** half of the residency claim. Gate `G1`, `V1-05`.
//!
//! Data plane: **Artifact Plane** payload → **Tensor Tile Plane** blocks
//! (ARCHITECTURE.md §2.1, §9.3).
//!
//! # Why this file exists alongside `bounded_residency.rs`
//!
//! Peak RSS is the metric `.plan/MASTER_PLAN.md` §4 and `TASK.md` name, and it is
//! the right one for a *product* claim — it is what the operating system says the
//! process cost. But it is **approximate**, it includes the binary's text and
//! stacks, and on this machine it is dominated by them: `QM-0100` measured
//! ~3.4 MB of RSS for a process that indexed 8 285 bytes. A flatness claim
//! carried on RSS across three checkpoint sizes would therefore be nearly
//! uninformative — it would be flat because the binary is the same binary.
//!
//! So the flatness property is carried here instead, on an **exact** figure: a
//! counting `#[global_allocator]` that reports the heap high-water mark of the
//! pass itself. `bounded_residency.rs`'s RSS rows corroborate this; they do not
//! substitute for it.
//!
//! The idiom, and the one-test-per-binary constraint below, come from
//! `crates/q-catalog/tests/trillion_scale_manifest.rs` (`CAT-006`) and
//! `crates/q-tensor-runtime/tests/bounded_residency.rs` (`QM-0030`).
//!
//! # What this measures, and what it cannot
//!
//! `LocalFsSource::open_without_mapping` is used deliberately. On a mapped source
//! the counting allocator would be **blind**: a memory map never reaches
//! `GlobalAlloc`, so "streamed one block at a time" and "mapped the whole file and
//! copied one block at a time" would both report a small heap. Reading by `seek` +
//! `read` means every byte the streamer receives passes through a buffer this
//! allocator can see.
//!
//! It still cannot see mapped pages, stacks, or the binary's text. Those are the
//! RSS measurement's business, and neither figure is presented as the other.

use q_safetensors::ingest_local;
use q_source::LocalFsSource;
use q_tensor_runtime::residency::{self, ResidencyRequest};
use q_tensor_runtime::stream::BlockStreamConfig;
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// Measure the peak allocation `body` adds above whatever is already live.
fn measure<T>(body: impl FnOnce() -> T) -> (T, usize) {
    PEAK_BYTES.store(LIVE_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
    let baseline = peak_bytes();
    let value = body();
    (value, peak_bytes().saturating_sub(baseline))
}

// --- the declared ceiling ----------------------------------------------------

/// The heap a bounded pass may peak at, at default budgets.
///
/// **Declared from the formula, not from a result.** `.plan/MEMORY_BUDGET.md` §4
/// gives `host_staging_bytes = N × E × 4` = 4 × 256 × 256 × 4 = 1 MiB, and
/// `BlockStreamConfig::accounted_resident_bytes` adds the two blocks that can be
/// live beyond the queue's capacity plus one run buffer, giving 1 574 912 B. This
/// ceiling is that figure rounded up to 2 MiB for the harness's own bookkeeping —
/// the same 2 MiB `QM-0030` declared for the single-tensor case, which is where
/// the number comes from rather than from anything measured here.
const PEAK_CEILING_BYTES: usize = 2 * 1024 * 1024;

/// How far the peak may move between the smallest and largest checkpoint before
/// the claim "flat in checkpoint size" stops being true.
///
/// A quarter of one decoded 256×256 f32 block. Loose enough for allocator
/// bookkeeping across two different checkpoints, and far tighter than any
/// implementation whose residency tracked checkpoint size could pass: the largest
/// checkpoint here is 3 331× the smallest, so a size-proportional peak would miss
/// this by six orders of magnitude.
const FLATNESS_TOLERANCE_BYTES: usize = 64 * 1024;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the integration-test crate always has a parent directory")
        .to_path_buf()
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            !matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false" | "no"
            )
        })
        .unwrap_or(false)
}

/// The real checkpoint, or `None` with a loud reason. `models/` is gitignored.
fn checkpoint_dir() -> Option<PathBuf> {
    let dir = repo_root().join("models/distilbert-distilgpt2");
    if dir.join("model.safetensors").is_file() {
        return Some(dir);
    }
    let reason = format!(
        "SKIPPED (one phase only) — NOT PROVEN: {} is absent. `models/` is gitignored, so the \
         weights are not in the repository. The two fixture phases below still run and still \
         assert flatness across them; only the 337 MiB row is missing. Set \
         QM_REQUIRE_REAL_CHECKPOINT=1 to make this absence a hard failure. See \
         .plan/evidence/QM-0101.md.",
        dir.join("model.safetensors").display()
    );
    if env_flag("QM_REQUIRE_REAL_CHECKPOINT") {
        panic!("{reason}");
    }
    eprintln!("{reason}");
    None
}

/// One measured phase: the checkpoint, its size, and the pass's exact heap peak.
struct Phase {
    label: String,
    checkpoint_bytes: u64,
    bytes_streamed: u64,
    blocks: u64,
    peak: usize,
}

/// Measure one full pass. Ingestion and source construction are measured
/// **separately** from the streaming loop, because folding them together would
/// hide the loop's cost behind the descriptor vector — which *is* allowed to grow
/// with tensor count (`.plan/MEMORY_BUDGET.md` §2, `O(tensor count)`), while the
/// loop is not allowed to grow with anything.
fn measure_pass(label: &str, dir: &Path, config: BlockStreamConfig) -> Phase {
    let ingested = ingest_local(dir).expect("ingest headers");
    let checkpoint_bytes: u64 = ingested
        .manifest
        .files
        .iter()
        .filter(|f| f.kind == q_source::ArtifactKind::SafeTensorsShard)
        .map(|f| f.length)
        .sum();
    let source = LocalFsSource::open_without_mapping(dir).expect("open without mapping");
    let described = ingested.described_payload_bytes;

    let (outcome, peak) = measure(|| {
        residency::run(
            &source,
            &ingested.descriptors,
            &ResidencyRequest {
                config,
                ..Default::default()
            },
        )
        .expect("the pass must complete")
    });

    assert!(
        outcome.reconciles_against(described),
        "{label}: streamed {} + refused {} != described {described}",
        outcome.bytes_streamed,
        outcome.refused_payload_bytes
    );
    assert!(outcome.is_complete(), "{label}: the pass did not complete");
    assert_ne!(
        outcome.checksum, 0,
        "{label}: a zero checksum means no byte was folded, and a pass that read nothing has \
         excellent residency for the wrong reason"
    );
    Phase {
        label: label.to_string(),
        checkpoint_bytes,
        bytes_streamed: outcome.bytes_streamed,
        blocks: outcome.blocks_planned,
        peak,
    }
}

#[test]
fn peak_heap_is_bounded_by_block_size_and_does_not_grow_with_checkpoint_size() {
    let config = BlockStreamConfig::default();
    // The formula, asserted before anything is measured, so the ceiling below is
    // anchored to `.plan/MEMORY_BUDGET.md` §4 rather than to a result.
    assert_eq!(config.host_staging_bytes(), 1024 * 1024);
    assert_eq!(
        config.accounted_resident_bytes(),
        6 * 256 * 256 * 4 + 256 * 8
    );
    assert!(config.accounted_resident_bytes() as usize <= PEAK_CEILING_BYTES);

    let mut phases = vec![
        measure_pass(
            "tiny-llama-single",
            &repo_root().join("fixtures/tiny-llama-single"),
            config,
        ),
        measure_pass(
            "tiny-llama-2shard",
            &repo_root().join("fixtures/tiny-llama-2shard"),
            config,
        ),
    ];
    let real = checkpoint_dir().map(|dir| measure_pass("distilbert-distilgpt2", &dir, config));
    let had_real = real.is_some();
    if let Some(phase) = real {
        phases.push(phase);
    }

    // Every phase under the declared ceiling.
    for phase in &phases {
        assert!(
            phase.peak <= PEAK_CEILING_BYTES,
            "{}: peak heap {} B exceeded the declared ceiling {PEAK_CEILING_BYTES} B over {} \
             blocks and {} bytes streamed",
            phase.label,
            phase.peak,
            phase.blocks,
            phase.bytes_streamed
        );
    }

    // The load-bearing assertion: the peak does not track checkpoint size.
    let smallest = phases
        .iter()
        .min_by_key(|p| p.checkpoint_bytes)
        .expect("at least one phase");
    let largest = phases
        .iter()
        .max_by_key(|p| p.checkpoint_bytes)
        .expect("at least one phase");
    let span = largest.checkpoint_bytes as f64 / smallest.checkpoint_bytes as f64;
    // max − min over **every** phase, not just the smallest and largest
    // checkpoints. The stricter of the two definitions, and the one
    // `fixtures/residency-measurements.json` records: a middle phase that spiked
    // would slip past a smallest-vs-largest comparison entirely.
    let lowest = phases.iter().map(|p| p.peak).min().expect("a phase");
    let highest = phases.iter().map(|p| p.peak).max().expect("a phase");
    let spread = highest - lowest;
    assert!(
        spread <= FLATNESS_TOLERANCE_BYTES,
        "peak heap moved by {spread} B across the measured phases ({lowest} B to {highest} B) \
         over a {span:.0}x span of checkpoint size ({} at {} B to {} at {} B). Residency must \
         be a function of block size, never of checkpoint size; this is the failure gate G1 \
         exists to catch (.plan/EXECUTION_ORDER.md §7: halt the engine lane and bisect per \
         stage)",
        smallest.label,
        smallest.checkpoint_bytes,
        largest.label,
        largest.checkpoint_bytes,
    );

    // A larger block costs proportionally more, and that is the *point*: it shows
    // the peak tracks the configured block size rather than being a fixed floor
    // that would look flat whatever the code did.
    let bigger_blocks = measure_pass(
        "tiny-llama-2shard @ 512x512",
        &repo_root().join("fixtures/tiny-llama-2shard"),
        BlockStreamConfig::default().with_block(512, 512),
    );
    let baseline = phases
        .iter()
        .find(|p| p.label == "tiny-llama-2shard")
        .expect("the 2-shard phase ran");
    assert!(
        bigger_blocks.peak > baseline.peak,
        "quadrupling the block edge did not raise the peak ({} B vs {} B), so this measurement \
         is not sensitive to the block size and its flatness result proves nothing",
        bigger_blocks.peak,
        baseline.peak
    );

    eprintln!(
        "QM-0101 PERF-002 (exact, counting allocator): peak heap {}{}; span {span:.0}x; \
         spread {spread} B, tolerance {FLATNESS_TOLERANCE_BYTES} B; ceiling \
         {PEAK_CEILING_BYTES} B; 512x512 block peak {} B",
        phases
            .iter()
            .map(|p| format!("{} {} B", p.label, p.peak))
            .collect::<Vec<_>>()
            .join(" / "),
        if had_real {
            ""
        } else {
            " (real checkpoint ABSENT — fixtures only)"
        },
        bigger_blocks.peak,
    );
}

// NOTE: there is deliberately exactly one `#[test]` in this file.
//
// The counting allocator is process-wide, so a second test running on another
// thread would allocate inside this one's measurement window and make the ceiling
// assertion nondeterministic. Every phase is folded into the single test above —
// the same constraint `crates/q-catalog/tests/trillion_scale_manifest.rs` records
// for `CAT-006` and `crates/q-tensor-runtime/tests/bounded_residency.rs` for
// `QM-0030`.
