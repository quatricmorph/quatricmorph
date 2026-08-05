//! Data plane: **Tensor Tile Plane**, reading from the **Artifact Plane**
//! (ARCHITECTURE.md §2.1, §9.3, §14.2).
//!
//! Bounded streaming block reader — `TILE-009`, `MVP-09`, `PERF-001`.
//!
//! # The one property this module exists to hold
//!
//! ```text
//! peak decoded residency = live_blocks × block_rows × block_columns × 4
//!                        ≤ (MAX_CONCURRENT_BLOCKS + 2) × E × 4
//! ```
//!
//! **Independent of tensor size, and independent of file size.** A 4096×4096
//! f32 tensor and a 65536×65536 one stream through buffers of exactly the same
//! size; only the block *count* differs. That is `PERF-001`, and
//! [`.plan/PERFORMANCE_PLAN.md`] §4 states it as the single scaling invariant
//! the architecture rests on: *nothing may scale as O(model bytes) in memory*.
//!
//! It is asserted as a test, not measured as a benchmark, because a regression
//! here does not make the system slow — it makes the design false.
//!
//! # How a block is read
//!
//! [`TensorBlock::plan`] derives one byte run per block row from metadata alone
//! (`TILE-002`, no reads). This module then reads those runs, one at a time,
//! into a **single reusable scratch buffer** the width of one run, decodes each
//! into the block's `f32` values, and emits a [`StreamedBlock`].
//!
//! For a 256-column window of a 4096-column f32 tensor that is 256 runs of
//! 1 KiB at a 16 KiB stride — 256 KiB of I/O, not the 4 MiB a row-span read
//! would cost (`.plan/MEMORY_BUDGET.md` §3).
//!
//! # Ordering, and why it is fixed
//!
//! Blocks are visited **row-major over the block grid**: index
//! `i` is grid row `i / grid_columns`, grid column `i % grid_columns`. The order
//! is a pure function of the tensor shape and the effective block size, so a
//! resumed pass and a fresh pass visit blocks identically and
//! [`BlockStream::resume_from`] is exact rather than approximate.
//!
//! Adaptive halving is therefore resolved **once, at construction**, before the
//! grid exists. Halving mid-stream would silently change the grid under a
//! consumer and break both determinism and resume.
//!
//! # What this module refuses
//!
//! | Situation | Answer |
//! | --- | --- |
//! | Configured staging exceeds the budget | [`QError::BudgetExceeded`] naming `host_staging`, before any read |
//! | Accounted residency exceeds the configured ceiling `C` | [`QError::BudgetExceeded`] naming `max_resident`, before any read (`QM-0101`) |
//! | Shape × dtype ≠ declared byte range | [`QError::MalformedArtifact`], before any read |
//! | Rank ≠ 2 | Refused, never flattened — ADR-010 |
//! | A dtype that cannot widen into `f32` exactly | [`QError::UnsupportedDType`], never rounded |
//! | A run that returns fewer bytes than asked | Error naming the block and the byte range; **never zero-filled** |
//! | Allocation failure down to the 64×64 floor | Refused naming `host_staging` |
//!
//! **A block is never silently skipped.** A failing block surfaces its error
//! once and stops the stream rather than advancing past it.

use crate::{BlockData, BlockExtent, Lod, TensorBlock, TileId};
use q_source::budget::{
    MemoryBudget, DEFAULT_BLOCK_DIMENSION, MAX_CONCURRENT_BLOCKS, MAX_HOST_STAGING_BYTES,
    MAX_OUTPUT_QUEUE_DEPTH, MAX_RESIDENT_BYTES, MIN_BLOCK_DIMENSION,
};
use q_source::dtype::{bf16_bits_to_f32, f16_bits_to_f32};
use q_source::error::{QError, Result};
use q_source::manifest::ByteStream;
use q_source::{CancellationToken, DType, ModelSource, TensorDescriptor};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

/// Decoded blocks are always `f32` (`.plan/MEMORY_BUDGET.md` §4).
const DECODED_BYTES_PER_ELEMENT: u64 = 4;

/// Decoded blocks that can be live **beyond** the bounded queue's capacity.
///
/// Two, and both terms are real: one the reader holds while its `send` blocks on
/// a full queue, and one the consumer holds. [`BlockStream::drive_bounded`]'s
/// own documentation states the bound and
/// `a_full_output_queue_blocks_the_reader_instead_of_growing` measures it — it
/// was first written as `+ 1` and the guard run caught that at high water 3 with
/// capacity 1.
///
/// It is a named constant here because [`BlockStreamConfig::accounted_resident_bytes`]
/// needs the same number, and a residency ceiling checked against a different
/// bound than the one the streaming loop actually holds would be checking the
/// wrong thing.
pub const LIVE_BLOCKS_OVER_QUEUE_CAPACITY: u64 = 2;

/// Widest storage dtype, in bytes, for bounding the one run buffer before the
/// tensor's dtype is known.
///
/// `F64`/`I64`/`U64` are 8 bytes. `BlockStream` refuses all three
/// (`streams_exactly_into_f32`), so 8 is strictly conservative for anything that
/// can actually stream — which is the right direction for a ceiling.
const WIDEST_DTYPE_BYTES: u64 = 8;

/// How long [`BlockStream::drive_bounded`] waits on an empty queue before
/// declaring a deadlock.
///
/// Backpressure is supposed to *stall* the reader, never to wedge it. A bounded
/// queue with a bug in its handshake would otherwise hang a test runner
/// indefinitely; this makes that failure loud and finite
/// (`.plan/tasks/QM-0030-…/TASK.md`, Risks: "Backpressure deadlocks").
pub const BOUNDED_QUEUE_TIMEOUT: Duration = Duration::from_secs(60);

/// Configuration for one streaming pass.
///
/// Every field is a named budget from `.plan/MEMORY_BUDGET.md` §4–§5; there are
/// no bare numbers at the call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockStreamConfig {
    pub block_rows: u64,
    pub block_columns: u64,
    /// Ceiling on decoded host staging: `max_concurrent_blocks × E × 4`.
    pub max_host_staging_bytes: u64,
    /// Decoded blocks that may be live at once.
    pub max_concurrent_blocks: usize,
    /// Bounded output queue depth. A full queue blocks the reader.
    pub max_output_queue_depth: usize,
    /// Adaptive halving floor. Halving never produces an edge below this.
    pub min_block_dimension: u64,
    /// The **process** resident ceiling `C` this pass is admitted against
    /// (`.plan/MASTER_PLAN.md` §4, `q_source::budget::MAX_RESIDENT_BYTES`).
    ///
    /// Different in kind from `max_host_staging_bytes`, and the difference is
    /// the point of `QM-0101`: staging caps *the decoded blocks*, while this caps
    /// everything the pass accounts for as resident. A configuration whose
    /// accounted residency exceeds it is refused before any read, naming
    /// `max_resident`.
    pub max_resident_bytes: u64,
}

impl Default for BlockStreamConfig {
    fn default() -> Self {
        Self {
            block_rows: DEFAULT_BLOCK_DIMENSION,
            block_columns: DEFAULT_BLOCK_DIMENSION,
            max_host_staging_bytes: MAX_HOST_STAGING_BYTES,
            max_concurrent_blocks: MAX_CONCURRENT_BLOCKS,
            max_output_queue_depth: MAX_OUTPUT_QUEUE_DEPTH,
            min_block_dimension: MIN_BLOCK_DIMENSION,
            max_resident_bytes: MAX_RESIDENT_BYTES,
        }
    }
}

impl BlockStreamConfig {
    pub fn with_block(mut self, rows: u64, columns: u64) -> Self {
        self.block_rows = rows;
        self.block_columns = columns;
        self
    }

    pub fn with_max_concurrent_blocks(mut self, blocks: usize) -> Self {
        self.max_concurrent_blocks = blocks;
        self
    }

    pub fn with_max_output_queue_depth(mut self, depth: usize) -> Self {
        self.max_output_queue_depth = depth;
        self
    }

    pub fn with_max_host_staging_bytes(mut self, bytes: u64) -> Self {
        self.max_host_staging_bytes = bytes;
        self
    }

    /// Set the configured resident ceiling `C`.
    pub fn with_max_resident_bytes(mut self, bytes: u64) -> Self {
        self.max_resident_bytes = bytes;
        self
    }

    /// Apply `.plan/MEMORY_BUDGET.md` §11's resolved budgets to this
    /// configuration.
    ///
    /// The single point where the precedence chain reaches the streaming path,
    /// so a run's block size, concurrency, queue depth, staging ceiling and
    /// resident ceiling all come from the same resolution and are all reportable
    /// with their provenance.
    pub fn from_budgets(budgets: &q_source::config::StreamingBudgets) -> Self {
        Self {
            block_rows: budgets.block_rows.value,
            block_columns: budgets.block_columns.value,
            max_host_staging_bytes: budgets.max_host_staging_bytes.value,
            max_concurrent_blocks: budgets.concurrent_blocks(),
            max_output_queue_depth: budgets.output_queue_depth(),
            min_block_dimension: MIN_BLOCK_DIMENSION,
            max_resident_bytes: budgets.max_resident_bytes.value,
        }
    }

    /// Elements in a full (unclamped) block.
    pub fn block_elements(&self) -> u64 {
        self.block_rows.saturating_mul(self.block_columns)
    }

    /// Bytes one decoded block costs. Always `f32` after decode, whatever the
    /// storage dtype was.
    pub fn decoded_block_bytes(&self) -> u64 {
        self.block_elements()
            .saturating_mul(DECODED_BYTES_PER_ELEMENT)
    }

    /// `.plan/MEMORY_BUDGET.md` §4: `host_staging_bytes = N × E × 4`.
    pub fn host_staging_bytes(&self) -> u64 {
        self.decoded_block_bytes()
            .saturating_mul(self.max_concurrent_blocks as u64)
    }

    /// The named budget a staging failure reports.
    pub fn host_staging_budget(&self) -> MemoryBudget {
        MemoryBudget::new(
            MemoryBudget::host_staging().name,
            self.max_host_staging_bytes,
        )
    }

    /// The named budget a resident-ceiling failure reports.
    pub fn resident_budget(&self) -> MemoryBudget {
        MemoryBudget::resident_at(self.max_resident_bytes)
    }

    /// Capacity of the bounded output queue this configuration produces.
    ///
    /// `min(max_output_queue_depth, max_concurrent_blocks)`, and the `min` is the
    /// load-bearing part — see [`BlockStream::queue_capacity`], which delegates
    /// here so the admission arithmetic and the running loop cannot disagree.
    pub fn queue_capacity(&self) -> usize {
        self.max_output_queue_depth
            .min(self.max_concurrent_blocks)
            .max(1)
    }

    /// Bytes this configuration accounts for as resident during one pass.
    ///
    /// ```text
    /// accounted = (queue_capacity + 2) × decoded_block_bytes     decoded blocks live at once
    ///           + block_columns × 8                             the one reusable run buffer
    /// ```
    ///
    /// Three things it deliberately does **not** count, each stated so the number
    /// is not mistaken for a process RSS:
    ///
    /// * the process itself — binary text, stacks, the allocator's own arenas;
    /// * memory-mapped source pages, which never reach the allocator at all
    ///   (`.plan/MEMORY_BUDGET.md` §3) and whose page-level residency this
    ///   repository does not measure;
    /// * anything a *consumer* of the blocks allocates.
    ///
    /// So this is the pass's **own** accounted residency, exact for what it
    /// accounts for, and it is what a configuration is admitted against. The
    /// process's peak RSS is a separate, external, approximate measurement
    /// (`/usr/bin/time -l`), and `.plan/evidence/QM-0101.md` keeps the two
    /// apart.
    pub fn accounted_resident_bytes(&self) -> u64 {
        let live_blocks =
            (self.queue_capacity() as u64).saturating_add(LIVE_BLOCKS_OVER_QUEUE_CAPACITY);
        let decoded = self.decoded_block_bytes().saturating_mul(live_blocks);
        let run_buffer = self.block_columns.saturating_mul(WIDEST_DTYPE_BYTES);
        decoded.saturating_add(run_buffer)
    }

    /// Reject a configuration that could not be bounded, before any read.
    pub fn validate(&self) -> Result<()> {
        if self.block_rows == 0 || self.block_columns == 0 {
            return Err(QError::QueryRejected(format!(
                "block stream needs a non-empty block; got {}x{}",
                self.block_rows, self.block_columns
            )));
        }
        if self.max_concurrent_blocks == 0 {
            return Err(QError::QueryRejected(
                "block stream needs max_concurrent_blocks >= 1".to_string(),
            ));
        }
        if self.max_output_queue_depth == 0 {
            return Err(QError::QueryRejected(
                "block stream needs max_output_queue_depth >= 1; a zero-depth \
                 queue cannot apply backpressure, it can only deadlock"
                    .to_string(),
            ));
        }
        if self.min_block_dimension == 0 {
            return Err(QError::QueryRejected(
                "block stream needs min_block_dimension >= 1".to_string(),
            ));
        }
        self.host_staging_budget()
            .check(self.host_staging_bytes())?;
        // The resident ceiling is checked last, and against `C` itself rather
        // than `1.25 × C`: the 25 % of `.plan/MASTER_PLAN.md` §4 is a tolerance
        // on the *measurement*, not headroom the planner may spend. Admitting a
        // configuration that needs 1.2 × C and then reporting that the peak came
        // in under 1.25 × C would be using the tolerance twice.
        self.resident_budget()
            .check(self.accounted_resident_bytes())
    }

    /// Both edges halved, or `None` at the floor.
    ///
    /// The floor is absolute: halving never returns an edge below
    /// `min_block_dimension`, so a degenerate 1×1 block can never be reached by
    /// stepping down (`.plan/MEMORY_BUDGET.md` §5).
    pub fn halved(&self) -> Option<BlockStreamConfig> {
        let block_rows = self.block_rows / 2;
        let block_columns = self.block_columns / 2;
        if block_rows < self.min_block_dimension || block_columns < self.min_block_dimension {
            return None;
        }
        Some(Self {
            block_rows,
            block_columns,
            ..*self
        })
    }
}

/// Whether decoded host staging of `bytes` can be reserved right now.
///
/// Exists as a trait for one reason: adaptive halving is a *failure* path, and a
/// failure path that cannot be provoked deterministically is untested. The
/// production implementation is [`SystemStagingProbe`]; tests substitute a probe
/// that refuses a chosen size.
pub trait StagingProbe {
    fn can_reserve(&self, bytes: u64) -> bool;
}

/// Asks the real allocator, then gives the memory straight back.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemStagingProbe;

impl StagingProbe for SystemStagingProbe {
    fn can_reserve(&self, bytes: u64) -> bool {
        let Ok(elements) = usize::try_from(bytes / DECODED_BYTES_PER_ELEMENT) else {
            return false;
        };
        let mut probe: Vec<f32> = Vec::new();
        probe.try_reserve_exact(elements).is_ok()
    }
}

/// The block grid of one tensor at one block size.
///
/// Pure arithmetic over the shape: constructing a grid reads nothing, so
/// `block_count()` is known for a tensor of any size at no I/O cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockGrid {
    /// Rows of blocks — `ceil(tensor_rows / block_rows)`.
    pub grid_rows: u64,
    /// Columns of blocks — `ceil(tensor_columns / block_columns)`.
    pub grid_columns: u64,
    /// Block edges this grid was generated with, after any adaptive halving.
    pub block_rows: u64,
    pub block_columns: u64,
}

impl BlockGrid {
    pub fn block_count(&self) -> u64 {
        self.grid_rows.saturating_mul(self.grid_columns)
    }
}

/// One block, read and decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamedBlock {
    pub extent: BlockExtent,
    pub block_id: TileId,
    pub data: BlockData,
    /// Storage bytes actually read for this block —
    /// `extent.rows() × extent.columns() × dtype_width`, never more.
    pub bytes_read: u64,
}

/// How a pass ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamOutcome {
    pub blocks_emitted: u64,
    pub bytes_read: u64,
    /// Blocks in the whole grid, whether or not this pass reached them.
    pub block_count: u64,
    /// Where a resumed pass must start. Equals `block_count` on a full pass.
    pub next_block_index: u64,
    /// `Some(checkpoint)` when the pass stopped at a block boundary because the
    /// cancellation token was set.
    pub cancelled_at: Option<String>,
    pub block_rows: u64,
    pub block_columns: u64,
    /// Times the block was halved to satisfy the staging budget.
    pub halvings: u32,
    /// Capacity of the bounded output queue this pass used, if any.
    pub queue_capacity: usize,
    /// Most blocks simultaneously handed to the queue. Bounded, by construction,
    /// by `queue_capacity + 2`.
    pub queue_high_water: usize,
}

impl StreamOutcome {
    pub fn is_complete(&self) -> bool {
        self.cancelled_at.is_none() && self.next_block_index == self.block_count
    }
}

/// Whether every stored value of `dtype` widens into `f32` **exactly**.
///
/// `f32` carries 24 significand bits, so `I32`/`U32`/`I64`/`U64`/`F64` values
/// above 2^24 would be silently rounded. [`BlockData`] is `f32` by
/// construction, so those dtypes are **refused** rather than approximated —
/// the same rule `SRC-014` applies to unknown dtypes and ADR-010 applies to
/// rank. An approximate value presented as a weight is worse than no value.
pub fn streams_exactly_into_f32(dtype: DType) -> bool {
    matches!(
        dtype,
        DType::Bool
            | DType::U8
            | DType::I8
            | DType::I16
            | DType::U16
            | DType::F16
            | DType::BF16
            | DType::F32
    )
}

fn decode_element(dtype: DType, bytes: &[u8]) -> Result<f32> {
    let widen = |n: usize| -> Result<()> {
        if bytes.len() == n {
            Ok(())
        } else {
            Err(QError::malformed(
                "block decode",
                format!("dtype {dtype:?} needs {n} bytes, got {}", bytes.len()),
            ))
        }
    };
    Ok(match dtype {
        // Decoded straight from the bit pattern: no f64 round trip, so the
        // emitted f32 is bit-for-bit the stored value.
        DType::F32 => {
            widen(4)?;
            f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        }
        DType::BF16 => {
            widen(2)?;
            bf16_bits_to_f32(u16::from_le_bytes([bytes[0], bytes[1]]))
        }
        DType::F16 => {
            widen(2)?;
            f16_bits_to_f32(u16::from_le_bytes([bytes[0], bytes[1]]))
        }
        // Bool, U8, I8, I16, U16 — every value is below 2^24, so the f64 the
        // audited scalar decoder returns narrows to f32 losslessly.
        other if streams_exactly_into_f32(other) => other.decode_scalar(bytes)? as f32,
        other => {
            return Err(QError::UnsupportedDType {
                dtype: other.as_safetensors_str().to_string(),
                operation: "exact f32 block stream".into(),
            })
        }
    })
}

/// A tensor's blocks, read one at a time through a [`ModelSource`].
///
/// Pull-based: nothing is read until a block is requested, which makes the
/// simple path inherently backpressured — at most one decoded block is live.
/// [`BlockStream::drive_bounded`] adds a bounded queue for a producer/consumer
/// split, where backpressure becomes explicit instead of implicit.
pub struct BlockStream<'a> {
    source: &'a dyn ModelSource,
    descriptor: TensorDescriptor,
    /// Effective configuration, i.e. after adaptive halving.
    config: BlockStreamConfig,
    /// Configuration as requested, kept so a consumer can see what changed.
    requested: BlockStreamConfig,
    grid: BlockGrid,
    halvings: u32,
    next_index: u64,
    blocks_emitted: u64,
    bytes_read: u64,
    cancel: CancellationToken,
    cancelled_at: Option<String>,
    /// Set once a block failed. The stream stops rather than skipping it.
    failed: bool,
    /// One run's worth of storage bytes, reused across every run of every
    /// block. This is the *only* I/O buffer the pass allocates.
    scratch: Vec<u8>,
}

impl std::fmt::Debug for BlockStream<'_> {
    /// Hand-written because a `&dyn ModelSource` is not `Debug`. Reports the
    /// stream's position and its effective block size, which is what a failed
    /// assertion needs to be readable.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockStream")
            .field("tensor", &self.descriptor.canonical_name)
            .field("shape", &self.descriptor.shape)
            .field("dtype", &self.descriptor.dtype)
            .field("grid", &self.grid)
            .field("halvings", &self.halvings)
            .field("next_index", &self.next_index)
            .field("blocks_emitted", &self.blocks_emitted)
            .field("bytes_read", &self.bytes_read)
            .field("cancelled_at", &self.cancelled_at)
            .field("failed", &self.failed)
            .finish()
    }
}

impl<'a> BlockStream<'a> {
    /// Validate against the budgets, resolve the effective block size, and
    /// generate the grid. Reads nothing.
    pub fn new(
        source: &'a dyn ModelSource,
        descriptor: TensorDescriptor,
        config: BlockStreamConfig,
    ) -> Result<Self> {
        Self::with_staging_probe(source, descriptor, config, &SystemStagingProbe)
    }

    /// As [`BlockStream::new`], with the allocation probe supplied.
    pub fn with_staging_probe(
        source: &'a dyn ModelSource,
        descriptor: TensorDescriptor,
        config: BlockStreamConfig,
        probe: &dyn StagingProbe,
    ) -> Result<Self> {
        // Order matters: every check below happens before the first byte is
        // read, so a rejected stream costs no I/O at all.
        config.validate()?;
        descriptor.validate()?;
        let shape = Self::require_streamable_rank(&descriptor)?;
        if !streams_exactly_into_f32(descriptor.dtype) {
            return Err(QError::UnsupportedDType {
                dtype: descriptor.dtype.as_safetensors_str().to_string(),
                operation: format!(
                    "exact f32 block stream of tensor {}",
                    descriptor.canonical_name
                ),
            });
        }

        // `config` is kept as requested; `effective` is what halving produced.
        let (effective, halvings) = Self::resolve_block_size(config, probe)?;
        let grid = BlockGrid {
            grid_rows: shape.0.div_ceil(effective.block_rows),
            grid_columns: shape.1.div_ceil(effective.block_columns),
            block_rows: effective.block_rows,
            block_columns: effective.block_columns,
        };

        Ok(Self {
            source,
            descriptor,
            config: effective,
            requested: config,
            grid,
            halvings,
            next_index: 0,
            blocks_emitted: 0,
            bytes_read: 0,
            cancel: CancellationToken::new(),
            cancelled_at: None,
            failed: false,
            scratch: Vec::new(),
        })
    }

    /// Rank gate. ADR-010 caps the implemented visualization rank at 3 and
    /// requires anything above it to **refuse rather than flatten**; a
    /// `[32, 128, 128]` tensor shown as `[32, 16384]` is a confidently wrong
    /// picture, which is worse than no picture.
    ///
    /// [`BlockExtent`] is 2-D today, so rank 3 also refuses here — its depth
    /// extent is `QM-0040`'s work. Both refusals carry `GRID-007` so a caller
    /// can tell a declared gap from a bug.
    fn require_streamable_rank(descriptor: &TensorDescriptor) -> Result<(u64, u64)> {
        let rank = descriptor.shape.len();
        match rank {
            2 => Ok((descriptor.shape[0], descriptor.shape[1])),
            3 => Err(QError::not_implemented(
                "GRID-007",
                format!(
                    "tensor {} has rank 3 {:?}; block streaming needs a 2-D extent and the \
                     rank-3 depth extent of ADR-010 lands with QM-0040. Refused rather than \
                     flattened",
                    descriptor.canonical_name, descriptor.shape
                ),
            )),
            r if r > 3 => Err(QError::not_implemented(
                "GRID-007",
                format!(
                    "tensor {} has rank {r} {:?}; ADR-010 caps the implemented rank at 3, so \
                     this is refused rather than flattened into a 2-D grid",
                    descriptor.canonical_name, descriptor.shape
                ),
            )),
            r => Err(QError::QueryRejected(format!(
                "tensor {} has rank {r} {:?}; block streaming needs a 2-D extent",
                descriptor.canonical_name, descriptor.shape
            ))),
        }
    }

    /// Halve until the staging reservation succeeds, or refuse at the floor.
    ///
    /// Resolved here — before the grid exists — so the block order a consumer
    /// sees never changes underneath it.
    fn resolve_block_size(
        config: BlockStreamConfig,
        probe: &dyn StagingProbe,
    ) -> Result<(BlockStreamConfig, u32)> {
        let mut effective = config;
        let mut halvings = 0u32;
        loop {
            if probe.can_reserve(effective.host_staging_bytes()) {
                return Ok((effective, halvings));
            }
            match effective.halved() {
                Some(next) => {
                    effective = next;
                    halvings += 1;
                }
                None => {
                    return Err(QError::QueryRejected(format!(
                        "budget {}: could not reserve {} bytes of decoded staging for {} \
                         concurrent {}x{} blocks after {halvings} halving(s); \
                         min_block_dimension {} is the floor of .plan/MEMORY_BUDGET.md §5",
                        MemoryBudget::host_staging().name,
                        effective.host_staging_bytes(),
                        effective.max_concurrent_blocks,
                        effective.block_rows,
                        effective.block_columns,
                        effective.min_block_dimension,
                    )))
                }
            }
        }
    }

    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancel = token;
        self
    }

    /// Restart this stream at `next_block_index`.
    ///
    /// Exact rather than approximate because the visiting order is a pure
    /// function of shape and block size: the blocks a resumed pass emits are
    /// precisely the ones a full pass would have emitted from that index on.
    pub fn resume_from(mut self, next_block_index: u64) -> Result<Self> {
        if next_block_index > self.grid.block_count() {
            return Err(QError::QueryRejected(format!(
                "cannot resume tensor {} at block {next_block_index}: the grid has {} blocks",
                self.descriptor.canonical_name,
                self.grid.block_count()
            )));
        }
        self.next_index = next_block_index;
        self.blocks_emitted = 0;
        self.bytes_read = 0;
        self.cancelled_at = None;
        self.failed = false;
        Ok(self)
    }

    pub fn grid(&self) -> BlockGrid {
        self.grid
    }

    pub fn block_count(&self) -> u64 {
        self.grid.block_count()
    }

    /// Effective configuration, after any adaptive halving.
    pub fn config(&self) -> &BlockStreamConfig {
        &self.config
    }

    /// Configuration as requested. Differs from [`BlockStream::config`] only
    /// when halving occurred.
    pub fn requested_config(&self) -> &BlockStreamConfig {
        &self.requested
    }

    pub fn halvings(&self) -> u32 {
        self.halvings
    }

    pub fn next_block_index(&self) -> u64 {
        self.next_index
    }

    pub fn blocks_emitted(&self) -> u64 {
        self.blocks_emitted
    }

    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    pub fn cancelled_at(&self) -> Option<&str> {
        self.cancelled_at.as_deref()
    }

    pub fn descriptor(&self) -> &TensorDescriptor {
        &self.descriptor
    }

    /// Capacity of the bounded output queue.
    ///
    /// `min(max_output_queue_depth, max_concurrent_blocks)`, and the `min` is
    /// the load-bearing part. `MAX_OUTPUT_QUEUE_DEPTH` is 64 because
    /// `.plan/CUDA_ARCHITECTURE.md` sizes it for *compact* per-block output —
    /// statistics records, not decoded blocks. This queue carries decoded
    /// blocks, so the residency bound `MAX_CONCURRENT_BLOCKS` binds first: 64
    /// decoded 256×256 blocks would be 16 MiB, which would break the peak this
    /// module exists to hold. When `QM-0031` puts compact results on the queue
    /// instead, the depth ceiling is the one that applies.
    pub fn queue_capacity(&self) -> usize {
        self.config.queue_capacity()
    }

    /// Extent of block `index`, row-major over the grid, **clamped** to the
    /// tensor shape — an edge block is smaller, never padded.
    pub fn extent_at(&self, index: u64) -> Result<BlockExtent> {
        if index >= self.grid.block_count() {
            return Err(QError::QueryRejected(format!(
                "block index {index} is outside the {}-block grid of tensor {}",
                self.grid.block_count(),
                self.descriptor.canonical_name
            )));
        }
        let grid_row = index / self.grid.grid_columns;
        let grid_column = index % self.grid.grid_columns;
        let row_start = grid_row * self.config.block_rows;
        let column_start = grid_column * self.config.block_columns;
        BlockExtent::new(
            row_start,
            row_start + self.config.block_rows,
            column_start,
            column_start + self.config.block_columns,
        )?
        .clamped_to(&self.descriptor.shape)
    }

    /// Read and decode one block. The only method here that touches bytes.
    fn read_block(&mut self, index: u64) -> Result<StreamedBlock> {
        let extent = self.extent_at(index)?;
        let block = TensorBlock::plan(&self.descriptor, Lod::Block, extent)?;
        let extent = block.extent;
        let width = self.descriptor.dtype.size_in_bytes() as usize;
        let run_bytes = extent.columns() as usize * width;

        // Named budget on the single range read, so the per-run allocation is
        // checked rather than assumed small.
        MemoryBudget::single_read().check(run_bytes as u64)?;

        let elements = extent.element_count();
        let decoded_bytes = elements.saturating_mul(DECODED_BYTES_PER_ELEMENT);
        let mut values: Vec<f32> = Vec::new();
        if values
            .try_reserve_exact(usize::try_from(elements).unwrap_or(usize::MAX))
            .is_err()
        {
            return Err(QError::QueryRejected(format!(
                "budget {}: could not reserve {decoded_bytes} bytes for block {} of tensor {}",
                MemoryBudget::host_staging().name,
                block.block_id,
                self.descriptor.canonical_name
            )));
        }

        if self.scratch.len() != run_bytes {
            self.scratch = vec![0u8; run_bytes];
        }

        let mut bytes_read = 0u64;
        for (run, &(start, end)) in block.source_byte_ranges.0.iter().enumerate() {
            let length = end - start;
            let mut stream = self
                .source
                .read_range(&self.descriptor.shard_uri, start, length)?;
            let filled = fill_exactly(&mut stream, &mut self.scratch)?;
            if filled != run_bytes {
                return Err(QError::malformed(
                    &self.descriptor.shard_uri,
                    format!(
                        "short read in block {} run {run} of tensor {}: byte range \
                         {start}..{end} returned {filled} of {run_bytes} bytes. A short read \
                         is refused, never zero-filled",
                        block.block_id, self.descriptor.canonical_name
                    ),
                ));
            }
            for element in self.scratch.chunks_exact(width) {
                values.push(decode_element(self.descriptor.dtype, element)?);
            }
            bytes_read += length;
        }

        let data = BlockData::new(extent.rows() as usize, extent.columns() as usize, values)?;
        Ok(StreamedBlock {
            extent,
            block_id: block.block_id,
            data,
            bytes_read,
        })
    }

    /// Run to completion on this thread, handing each block to `sink`.
    ///
    /// At most one decoded block is live, so peak decoded residency is one
    /// block regardless of grid size.
    pub fn drive<F>(&mut self, mut sink: F) -> Result<StreamOutcome>
    where
        F: FnMut(StreamedBlock) -> Result<()>,
    {
        for block in self.by_ref() {
            sink(block?)?;
        }
        Ok(self.outcome(0, 0))
    }

    /// Run with a **bounded** output queue: the reader produces on a worker
    /// thread, `sink` consumes on this one, and a full queue blocks the reader.
    ///
    /// The queue is never grown. Growing it would trade a throughput problem for
    /// an out-of-memory crash, which is strictly worse because it destroys the
    /// completed work a stalled pipeline preserves
    /// (`.plan/MEMORY_BUDGET.md` §4).
    ///
    /// Live decoded blocks are bounded by `queue_capacity() + 2`: at most
    /// `capacity` queued, one held by the reader while its `send` blocks, and
    /// one held by `sink`.
    pub fn drive_bounded<F>(&mut self, mut sink: F) -> Result<StreamOutcome>
    where
        F: FnMut(StreamedBlock) -> Result<()>,
    {
        let capacity = self.queue_capacity();
        let live = Arc::new(AtomicUsize::new(0));
        let high_water = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::sync_channel::<Result<StreamedBlock>>(capacity);

        let producer_live = Arc::clone(&live);
        let producer_high = Arc::clone(&high_water);
        let reader = &mut *self;

        let consumed: Result<()> = std::thread::scope(|scope| {
            let handle = scope.spawn(move || {
                for item in reader {
                    let inflight = producer_live.fetch_add(1, Ordering::SeqCst) + 1;
                    producer_high.fetch_max(inflight, Ordering::SeqCst);
                    if tx.send(item).is_err() {
                        // The consumer is gone; stop rather than spin.
                        producer_live.fetch_sub(1, Ordering::SeqCst);
                        break;
                    }
                }
            });

            let mut failure: Option<QError> = None;
            loop {
                match rx.recv_timeout(BOUNDED_QUEUE_TIMEOUT) {
                    Ok(item) => {
                        live.fetch_sub(1, Ordering::SeqCst);
                        match item.and_then(&mut sink) {
                            Ok(()) => {}
                            Err(e) => {
                                failure = Some(e);
                                break;
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {
                        failure = Some(QError::QueryRejected(format!(
                            "bounded output queue (capacity {capacity}) produced nothing for \
                             {:?}; backpressure must stall the reader, never deadlock it",
                            BOUNDED_QUEUE_TIMEOUT
                        )));
                        break;
                    }
                }
            }
            // Dropping the receiver unblocks a reader parked in `send`, so the
            // join below cannot hang on an abandoned consumer.
            drop(rx);
            if handle.join().is_err() {
                return Err(QError::QueryRejected(
                    "the block reader thread panicked".to_string(),
                ));
            }
            match failure {
                Some(e) => Err(e),
                None => Ok(()),
            }
        });
        consumed?;

        Ok(self.outcome(capacity, high_water.load(Ordering::SeqCst)))
    }

    fn outcome(&self, queue_capacity: usize, queue_high_water: usize) -> StreamOutcome {
        StreamOutcome {
            blocks_emitted: self.blocks_emitted,
            bytes_read: self.bytes_read,
            block_count: self.grid.block_count(),
            next_block_index: self.next_index,
            cancelled_at: self.cancelled_at.clone(),
            block_rows: self.config.block_rows,
            block_columns: self.config.block_columns,
            halvings: self.halvings,
            queue_capacity,
            queue_high_water,
        }
    }
}

/// Fill `buf` completely, reporting how many bytes were actually available.
///
/// Returns short rather than erroring so the caller can name the block and the
/// byte range in the message; `Read::read_exact` cannot.
fn fill_exactly(stream: &mut ByteStream, buf: &mut [u8]) -> Result<usize> {
    let mut filled = 0usize;
    while filled < buf.len() {
        match stream.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(QError::BareIo(e)),
        }
    }
    Ok(filled)
}

impl Iterator for BlockStream<'_> {
    type Item = Result<StreamedBlock>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.next_index >= self.grid.block_count() {
            return None;
        }
        // Cancellation is checked *between* blocks: a block in progress always
        // finishes, so what a consumer received is always whole.
        if self.cancel.is_cancelled() {
            if self.cancelled_at.is_none() {
                self.cancelled_at = Some(format!(
                    "tensor {} block {}",
                    self.descriptor.canonical_name, self.next_index
                ));
            }
            return None;
        }
        let index = self.next_index;
        match self.read_block(index) {
            Ok(block) => {
                self.next_index += 1;
                self.blocks_emitted += 1;
                self.bytes_read += block.bytes_read;
                Some(Ok(block))
            }
            Err(e) => {
                // The index is deliberately not advanced. A failing block is
                // reported once and stops the stream; it is never skipped.
                self.failed = true;
                Some(Err(e))
            }
        }
    }
}

/// Total storage bytes a full pass over `descriptor` will read at `config`.
///
/// Metadata arithmetic — it opens nothing. Useful for the I/O estimate
/// ARCHITECTURE.md §19 requires before a large expression runs.
pub fn planned_bytes(descriptor: &TensorDescriptor, config: &BlockStreamConfig) -> Result<u64> {
    let shape = BlockStream::require_streamable_rank(descriptor)?;
    config.validate()?;
    Ok(shape
        .0
        .saturating_mul(shape.1)
        .saturating_mul(descriptor.dtype.size_in_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_source::manifest::{ModelManifest, ModelSource};
    use q_source::role::TensorRole;
    use q_source::{ModelId, TensorId};
    use std::sync::atomic::AtomicU64;

    const SHARD: &str = "model.safetensors";
    const BASE: u64 = 8; // a plausible header length; nothing here parses one

    /// An in-memory shard whose bytes are generated on demand.
    ///
    /// `read_range` synthesizes exactly the requested window, so a "tensor" of
    /// any declared size costs no memory and no disk. That is what makes the
    /// residency assertions meaningful: nothing but the streamer's own buffers
    /// can be holding the data.
    struct SyntheticShard {
        payload_bytes: u64,
        /// Bytes to withhold from every read, to provoke a short read.
        withhold: usize,
        /// Pretend the file is this long, to provoke a range refusal.
        declared_length: Option<u64>,
        reads: AtomicUsize,
        bytes_served: AtomicU64,
    }

    impl SyntheticShard {
        fn new(payload_bytes: u64) -> Self {
            Self {
                payload_bytes,
                withhold: 0,
                declared_length: None,
                reads: AtomicUsize::new(0),
                bytes_served: AtomicU64::new(0),
            }
        }

        fn withholding(mut self, bytes: usize) -> Self {
            self.withhold = bytes;
            self
        }

        fn truncated_to(mut self, length: u64) -> Self {
            self.declared_length = Some(length);
            self
        }

        fn reads(&self) -> usize {
            self.reads.load(Ordering::SeqCst)
        }

        fn bytes_served(&self) -> u64 {
            self.bytes_served.load(Ordering::SeqCst)
        }

        /// Deterministic byte at an absolute file offset. Defined here, in the
        /// test, so expected values never come from the code under test.
        fn byte_at(offset: u64) -> u8 {
            (offset.wrapping_mul(2_654_435_761) >> 11) as u8
        }

        fn file_length(&self) -> u64 {
            self.declared_length.unwrap_or(BASE + self.payload_bytes)
        }
    }

    impl ModelSource for SyntheticShard {
        fn manifest(&self) -> Result<ModelManifest> {
            Ok(ModelManifest {
                source_key: "synthetic:stream".into(),
                root_uri: "synthetic://stream".into(),
                revision: String::new(),
                files: Vec::new(),
                config: None,
            })
        }

        fn read_range(&self, uri: &str, offset: u64, length: u64) -> Result<ByteStream> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let end = offset
                .checked_add(length)
                .ok_or_else(|| QError::RangeOutOfBounds {
                    uri: uri.to_string(),
                    start: offset,
                    end: u64::MAX,
                    length: self.file_length(),
                })?;
            if end > self.file_length() {
                return Err(QError::RangeOutOfBounds {
                    uri: uri.to_string(),
                    start: offset,
                    end,
                    length: self.file_length(),
                });
            }
            let served = (length as usize).saturating_sub(self.withhold);
            self.bytes_served.fetch_add(served as u64, Ordering::SeqCst);
            let mut out = Vec::with_capacity(served);
            for i in 0..served {
                out.push(Self::byte_at(offset + i as u64));
            }
            // The declared length stays the full window: a source that shrinks
            // its promise as well as its payload would not exercise the short
            // read path at all.
            Ok(ByteStream::new(length, Box::new(std::io::Cursor::new(out))))
        }
    }

    fn descriptor(shape: Vec<u64>, dtype: DType) -> TensorDescriptor {
        let elements: u64 = shape.iter().product();
        TensorDescriptor {
            tensor_id: TensorId::derive(ModelId::derive("m", "", "f"), "t"),
            raw_name: "t".into(),
            canonical_name: "t".into(),
            byte_start: BASE,
            byte_end: BASE + elements * dtype.size_in_bytes(),
            shape,
            dtype,
            shard_uri: SHARD.into(),
            layer_index: None,
            semantic_role: TensorRole::Unknown,
        }
    }

    fn shard_for(descriptor: &TensorDescriptor) -> SyntheticShard {
        SyntheticShard::new(descriptor.byte_end - BASE)
    }

    /// Hand-computed expectation for one element: the test's own generator plus
    /// `f32::from_le_bytes`, never the streamer's decoder.
    fn expected_f32(descriptor: &TensorDescriptor, row: u64, column: u64) -> f32 {
        let offset = descriptor.byte_start + (row * descriptor.shape[1] + column) * 4;
        let mut bytes = [0u8; 4];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = SyntheticShard::byte_at(offset + i as u64);
        }
        f32::from_le_bytes(bytes)
    }

    fn collect(stream: &mut BlockStream<'_>) -> Vec<StreamedBlock> {
        let mut out = Vec::new();
        for item in stream.by_ref() {
            out.push(item.expect("block"));
        }
        out
    }

    // --- the grid ------------------------------------------------------------

    #[test]
    fn a_four_thousand_ninety_six_square_tensor_streams_as_two_hundred_fifty_six_blocks() {
        let d = descriptor(vec![4096, 4096], DType::F32);
        let shard = shard_for(&d);
        let stream = BlockStream::new(&shard, d, BlockStreamConfig::default()).unwrap();
        assert_eq!(stream.grid().grid_rows, 16);
        assert_eq!(stream.grid().grid_columns, 16);
        assert_eq!(stream.block_count(), 256);
        assert_eq!(
            (stream.config().block_rows, stream.config().block_columns),
            (256, 256)
        );
        // The grid is arithmetic: constructing it read nothing.
        assert_eq!(shard.reads(), 0);
    }

    #[test]
    fn edge_blocks_are_clamped_rather_than_padded() {
        // 4000 = 15 * 256 + 160, so the last row and column of blocks are 160
        // wide and the corner block is 160x160.
        let d = descriptor(vec![4000, 4000], DType::F32);
        let shard = shard_for(&d);
        let stream = BlockStream::new(&shard, d, BlockStreamConfig::default()).unwrap();
        assert_eq!(stream.block_count(), 16 * 16);

        let first = stream.extent_at(0).unwrap();
        assert_eq!((first.rows(), first.columns()), (256, 256));

        let last = stream.extent_at(stream.block_count() - 1).unwrap();
        assert_eq!((last.rows(), last.columns()), (160, 160));
        assert_eq!(last.element_count(), 160 * 160);
        assert_eq!(
            (last.row_end, last.column_end),
            (4000, 4000),
            "a clamped block ends at the tensor edge; a padded one would overrun it"
        );

        // Element counts sum to the tensor exactly: no padding, no gaps.
        let total: u64 = (0..stream.block_count())
            .map(|i| stream.extent_at(i).unwrap().element_count())
            .sum();
        assert_eq!(total, 4000 * 4000);
    }

    #[test]
    fn block_order_is_row_major_over_the_grid_and_identical_across_runs() {
        let d = descriptor(vec![24, 20], DType::F32);
        let shard = shard_for(&d);
        let config = BlockStreamConfig::default().with_block(8, 5);

        let mut first = BlockStream::new(&shard, d.clone(), config).unwrap();
        let mut second = BlockStream::new(&shard, d, config).unwrap();
        let a = collect(&mut first);
        let b = collect(&mut second);

        assert_eq!(a.len(), 3 * 4);
        let ids_a: Vec<TileId> = a.iter().map(|b| b.block_id).collect();
        let ids_b: Vec<TileId> = b.iter().map(|b| b.block_id).collect();
        assert_eq!(ids_a, ids_b, "two runs must visit blocks in the same order");

        // Row-major: the first four blocks walk across grid row 0.
        let starts: Vec<(u64, u64)> = a
            .iter()
            .take(5)
            .map(|b| (b.extent.row_start, b.extent.column_start))
            .collect();
        assert_eq!(starts, vec![(0, 0), (0, 5), (0, 10), (0, 15), (8, 0)]);
    }

    #[test]
    fn a_block_index_past_the_grid_is_refused() {
        let d = descriptor(vec![16, 16], DType::F32);
        let shard = shard_for(&d);
        let stream =
            BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8)).unwrap();
        assert_eq!(stream.block_count(), 4);
        assert!(stream.extent_at(3).is_ok());
        let err = stream.extent_at(4).unwrap_err();
        assert!(
            err.to_string().contains("outside the 4-block grid"),
            "message was {err}"
        );
    }

    // --- values and I/O cost -------------------------------------------------

    #[test]
    fn decoded_values_match_an_independently_computed_reference() {
        let d = descriptor(vec![12, 10], DType::F32);
        let shard = shard_for(&d);
        let mut stream = BlockStream::new(
            &shard,
            d.clone(),
            BlockStreamConfig::default().with_block(4, 5),
        )
        .unwrap();
        let blocks = collect(&mut stream);
        assert_eq!(blocks.len(), 3 * 2);

        for block in &blocks {
            for r in 0..block.extent.rows() {
                for c in 0..block.extent.columns() {
                    let want = expected_f32(
                        &d,
                        block.extent.row_start + r,
                        block.extent.column_start + c,
                    );
                    let got = block.data.get(r as usize, c as usize).unwrap();
                    assert_eq!(
                        got.to_bits(),
                        want.to_bits(),
                        "block {} element [{r},{c}]",
                        block.block_id
                    );
                }
            }
        }
    }

    #[test]
    fn a_block_reads_exactly_its_own_bytes_and_one_run_per_row() {
        let d = descriptor(vec![4096, 4096], DType::F32);
        let shard = shard_for(&d);
        let mut stream = BlockStream::new(&shard, d.clone(), BlockStreamConfig::default()).unwrap();

        let block = stream.next().unwrap().unwrap();
        // 256 x 256 f32 = 256 KiB, in 256 runs of 1 KiB — not the 4 MiB a
        // row-span read of a 4096-column tensor would cost.
        assert_eq!(block.bytes_read, 256 * 256 * 4);
        assert_eq!(block.bytes_read, 256 * 1024);
        assert_eq!(shard.reads(), 256, "one range read per block row");
        assert_eq!(shard.bytes_served(), 256 * 1024);

        let planned = planned_bytes(&d, &BlockStreamConfig::default()).unwrap();
        assert_eq!(planned, 4096 * 4096 * 4);
        assert!(
            block.bytes_read * 256 == planned,
            "256 blocks of {} bytes must account for the whole tensor",
            block.bytes_read
        );
    }

    #[test]
    fn bf16_and_f16_blocks_decode_to_hand_computed_ieee_754_values() {
        // Hand-computed, from the bit patterns alone:
        //   bf16 0x3F80 -> f32 0x3F800000 = 1.0
        //   bf16 0xC000 -> f32 0xC0000000 = -2.0
        //   f16  0x3C00 -> 1.0 ; 0xC000 -> -2.0 ; 0x0001 -> 2^-24
        struct Fixed(Vec<u8>);
        impl ModelSource for Fixed {
            fn manifest(&self) -> Result<ModelManifest> {
                Err(QError::NotFound("no manifest".into()))
            }
            fn read_range(&self, _uri: &str, offset: u64, length: u64) -> Result<ByteStream> {
                let s = offset as usize;
                let e = s + length as usize;
                Ok(ByteStream::from_vec(self.0[s..e].to_vec()))
            }
        }

        for (dtype, patterns, expected) in [
            (
                DType::BF16,
                [0x3F80u16, 0xC000, 0x3F80, 0xC000],
                [1.0f32, -2.0, 1.0, -2.0],
            ),
            (
                DType::F16,
                [0x3C00u16, 0xC000, 0x0001, 0x3C00],
                [1.0f32, -2.0, 2f32.powi(-24), 1.0],
            ),
        ] {
            let mut bytes = vec![0u8; BASE as usize];
            for p in patterns {
                bytes.extend_from_slice(&p.to_le_bytes());
            }
            let source = Fixed(bytes);
            let d = descriptor(vec![2, 2], dtype);
            let mut stream =
                BlockStream::new(&source, d, BlockStreamConfig::default().with_block(2, 2))
                    .unwrap();
            let block = stream.next().unwrap().unwrap();
            assert_eq!(block.data.values, expected, "dtype {dtype:?}");
            assert_eq!(block.bytes_read, 8, "4 elements x 2 bytes");
        }
    }

    // --- refusals, all before any read --------------------------------------

    #[test]
    fn refuses_rank_four_rather_than_flattening_it() {
        let d = descriptor(vec![1, 1, 1024, 1024], DType::F32);
        let shard = shard_for(&d);
        let err = BlockStream::new(&shard, d, BlockStreamConfig::default()).unwrap_err();
        assert_eq!(err.requirement_id(), Some("GRID-007"));
        let msg = err.to_string();
        assert!(msg.contains("rank 4"), "message was {msg}");
        assert!(
            msg.contains("refused rather than flattened"),
            "message was {msg}"
        );
        assert!(msg.contains("ADR-010"), "message was {msg}");
        assert_eq!(shard.reads(), 0, "the refusal must cost no I/O");
    }

    #[test]
    fn refuses_rank_three_naming_the_task_that_implements_it() {
        let d = descriptor(vec![4, 8, 8], DType::F32);
        let shard = shard_for(&d);
        let err = BlockStream::new(&shard, d, BlockStreamConfig::default()).unwrap_err();
        assert_eq!(err.requirement_id(), Some("GRID-007"));
        let msg = err.to_string();
        assert!(msg.contains("rank 3"), "message was {msg}");
        assert!(msg.contains("QM-0040"), "message was {msg}");
        assert_eq!(shard.reads(), 0);
    }

    #[test]
    fn refuses_rank_one_rather_than_inventing_a_second_axis() {
        let d = descriptor(vec![48], DType::F32);
        let shard = shard_for(&d);
        let err = BlockStream::new(&shard, d, BlockStreamConfig::default()).unwrap_err();
        assert!(err.to_string().contains("rank 1"), "message was {err}");
        assert_eq!(shard.reads(), 0);
    }

    #[test]
    fn refuses_an_unknown_dtype_rather_than_guessing_it() {
        let d = descriptor(vec![8, 8], DType::F8E4M3);
        let shard = shard_for(&d);
        let err =
            BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8)).unwrap_err();
        match &err {
            QError::UnsupportedDType { dtype, .. } => assert_eq!(dtype, "F8_E4M3"),
            other => panic!("expected UnsupportedDType, got {other:?}"),
        }
        assert_eq!(shard.reads(), 0);
    }

    #[test]
    fn refuses_dtypes_that_would_lose_precision_in_an_f32_block_rather_than_rounding() {
        // 2^24 + 1 is the first integer f32 cannot represent, so an I32 or I64
        // tensor cannot be streamed into an f32 block exactly.
        for dtype in [DType::I32, DType::U32, DType::I64, DType::U64, DType::F64] {
            assert!(!streams_exactly_into_f32(dtype), "{dtype:?}");
            let d = descriptor(vec![8, 8], dtype);
            let shard = shard_for(&d);
            let err = BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8))
                .unwrap_err();
            assert!(
                matches!(err, QError::UnsupportedDType { .. }),
                "{dtype:?} gave {err:?}"
            );
            assert_eq!(shard.reads(), 0);
        }
        for dtype in [
            DType::Bool,
            DType::U8,
            DType::I8,
            DType::I16,
            DType::U16,
            DType::F16,
            DType::BF16,
            DType::F32,
        ] {
            assert!(streams_exactly_into_f32(dtype), "{dtype:?}");
        }
    }

    #[test]
    fn a_shape_that_disagrees_with_the_declared_byte_range_is_refused_before_execution() {
        let mut d = descriptor(vec![64, 64], DType::F32);
        d.byte_end -= 4; // one element short of what the shape declares
        let shard = shard_for(&d);
        let err = BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(64, 64))
            .unwrap_err();
        assert!(
            matches!(err, QError::MalformedArtifact { .. }),
            "got {err:?}"
        );
        assert_eq!(shard.reads(), 0, "shape mismatch must be caught before I/O");
    }

    #[test]
    fn a_staging_budget_smaller_than_one_block_grid_is_refused_naming_the_budget() {
        let d = descriptor(vec![512, 512], DType::F32);
        let shard = shard_for(&d);
        let config = BlockStreamConfig::default().with_max_host_staging_bytes(512 * 1024);
        let err = BlockStream::new(&shard, d, config).unwrap_err();
        match &err {
            QError::BudgetExceeded {
                budget_name,
                requested,
                limit,
            } => {
                assert_eq!(*budget_name, "host_staging");
                assert_eq!((*requested, *limit), (1024 * 1024, 512 * 1024));
            }
            other => panic!("expected BudgetExceeded, got {other:?}"),
        }
        assert_eq!(shard.reads(), 0);
    }

    #[test]
    fn an_empty_block_and_a_zero_depth_queue_are_refused() {
        assert!(BlockStreamConfig::default()
            .with_block(0, 8)
            .validate()
            .is_err());
        assert!(BlockStreamConfig::default()
            .with_block(8, 0)
            .validate()
            .is_err());
        assert!(BlockStreamConfig::default()
            .with_max_output_queue_depth(0)
            .validate()
            .is_err());
        assert!(BlockStreamConfig::default()
            .with_max_concurrent_blocks(0)
            .validate()
            .is_err());
    }

    #[test]
    fn a_truncated_shard_is_refused_with_the_byte_range_that_overran_it() {
        let d = descriptor(vec![16, 16], DType::F32);
        // Declare the file two rows shorter than the tensor needs.
        let shard = shard_for(&d).truncated_to(BASE + 14 * 16 * 4);
        let mut stream =
            BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(16, 16)).unwrap();
        let err = stream.next().unwrap().unwrap_err();
        match &err {
            QError::RangeOutOfBounds {
                start, end, length, ..
            } => {
                assert_eq!(*length, BASE + 14 * 16 * 4);
                assert!(*end > *length, "the refused range must overrun the file");
                assert!(*start >= BASE);
            }
            other => panic!("expected RangeOutOfBounds, got {other:?}"),
        }
        // A failed block is reported once and the stream stops; it is not
        // skipped and the next block is not silently attempted.
        assert!(stream.next().is_none());
        assert_eq!(stream.blocks_emitted(), 0);
    }

    #[test]
    fn a_short_read_is_refused_naming_the_block_and_the_byte_range_never_zero_filled() {
        let d = descriptor(vec![8, 8], DType::F32);
        let shard = shard_for(&d).withholding(4);
        let mut stream =
            BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8)).unwrap();
        let err = stream.next().unwrap().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("short read"), "message was {msg}");
        assert!(msg.contains("block "), "message was {msg}");
        assert!(msg.contains("returned 28 of 32 bytes"), "message was {msg}");
        assert!(msg.contains("never zero-filled"), "message was {msg}");
        assert!(stream.next().is_none(), "a failed block is never skipped");
    }

    #[test]
    fn a_missing_shard_is_reported_rather_than_read_as_zeros() {
        struct Absent;
        impl ModelSource for Absent {
            fn manifest(&self) -> Result<ModelManifest> {
                Err(QError::NotFound("no manifest".into()))
            }
            fn read_range(&self, uri: &str, _o: u64, _l: u64) -> Result<ByteStream> {
                Err(QError::NotFound(uri.to_string()))
            }
        }
        let d = descriptor(vec![8, 8], DType::F32);
        let mut stream =
            BlockStream::new(&Absent, d, BlockStreamConfig::default().with_block(8, 8)).unwrap();
        assert!(matches!(
            stream.next().unwrap().unwrap_err(),
            QError::NotFound(_)
        ));
        assert_eq!(stream.blocks_emitted(), 0);
    }

    // --- cancellation and resume --------------------------------------------

    #[test]
    fn cancellation_stops_at_a_block_boundary_and_keeps_the_blocks_already_read() {
        let d = descriptor(vec![64, 64], DType::F32);
        let shard = shard_for(&d);
        let token = CancellationToken::new();
        let mut stream = BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8))
            .unwrap()
            .with_cancellation(token.clone());
        assert_eq!(stream.block_count(), 64);

        let mut kept = Vec::new();
        for item in stream.by_ref() {
            kept.push(item.unwrap());
            if kept.len() == 10 {
                token.cancel();
            }
        }
        assert_eq!(kept.len(), 10, "exactly the completed blocks are returned");
        assert_eq!(stream.blocks_emitted(), 10);
        assert_eq!(stream.next_block_index(), 10);
        // Every kept block is whole: cancellation landed on a boundary.
        for block in &kept {
            assert_eq!(block.data.values.len(), 64);
            assert_eq!(block.bytes_read, 8 * 8 * 4);
        }
        let checkpoint = stream.cancelled_at().expect("checkpoint recorded");
        assert!(
            checkpoint.contains("block 10"),
            "checkpoint was {checkpoint}"
        );
    }

    #[test]
    fn a_resumed_pass_visits_exactly_the_blocks_a_cancelled_one_did_not() {
        let d = descriptor(vec![64, 64], DType::F32);
        let shard = shard_for(&d);
        let config = BlockStreamConfig::default().with_block(8, 8);

        let whole: Vec<TileId> = {
            let mut s = BlockStream::new(&shard, d.clone(), config).unwrap();
            collect(&mut s).into_iter().map(|b| b.block_id).collect()
        };

        let token = CancellationToken::new();
        let mut interrupted = BlockStream::new(&shard, d.clone(), config)
            .unwrap()
            .with_cancellation(token.clone());
        let mut first: Vec<TileId> = Vec::new();
        for item in interrupted.by_ref() {
            first.push(item.unwrap().block_id);
            if first.len() == 10 {
                token.cancel();
            }
        }
        let resume_at = interrupted.next_block_index();
        assert_eq!(resume_at, 10);

        let mut resumed = BlockStream::new(&shard, d, config)
            .unwrap()
            .resume_from(resume_at)
            .unwrap();
        let second: Vec<TileId> = collect(&mut resumed)
            .into_iter()
            .map(|b| b.block_id)
            .collect();

        let mut rejoined = first;
        rejoined.extend(second);
        assert_eq!(
            rejoined, whole,
            "interrupted + resumed must equal one uninterrupted pass, in order"
        );
        assert_eq!(rejoined.len(), 64);
    }

    #[test]
    fn resuming_past_the_end_of_the_grid_is_refused() {
        let d = descriptor(vec![16, 16], DType::F32);
        let shard = shard_for(&d);
        let stream =
            BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8)).unwrap();
        assert!(stream.resume_from(65).is_err());
    }

    // --- backpressure --------------------------------------------------------

    #[test]
    fn a_full_output_queue_blocks_the_reader_instead_of_growing() {
        let d = descriptor(vec![128, 128], DType::F32);
        let shard = shard_for(&d);
        // Depth 1 with concurrency 1: the queue holds one block.
        let config = BlockStreamConfig::default()
            .with_block(8, 8)
            .with_max_concurrent_blocks(1)
            .with_max_output_queue_depth(1);
        let mut stream = BlockStream::new(&shard, d, config).unwrap();
        assert_eq!(stream.queue_capacity(), 1);
        let total = stream.block_count();
        assert_eq!(total, 256);

        let mut seen = 0u64;
        let outcome = stream
            .drive_bounded(|_block| {
                seen += 1;
                // A consumer slower than the reader is what makes the queue
                // fill; without it the reader is never asked to wait.
                std::thread::sleep(Duration::from_micros(200));
                Ok(())
            })
            .unwrap();

        assert_eq!(seen, total);
        assert_eq!(outcome.blocks_emitted, total);
        assert_eq!(outcome.queue_capacity, 1);
        // `capacity + 2`, and every term is real: `capacity` blocks buffered in
        // the channel, one the reader holds while its `send` blocks, and one the
        // consumer holds. The counter is decremented after `recv` returns, so a
        // consumer descheduled between `recv` and its decrement is the third
        // term — which is also a genuinely live decoded block, not a counting
        // artefact. This bound was first written as `+ 1` and the guard run
        // caught it at high water 3 with capacity 1; the residency arithmetic in
        // `drive_bounded`'s own documentation always said `+ 2`.
        assert!(
            outcome.queue_high_water <= outcome.queue_capacity + 2,
            "queue high water {} exceeded capacity {} + 2",
            outcome.queue_high_water,
            outcome.queue_capacity
        );
        // The assertion that matters: an unbounded queue would reach `total`.
        assert!(
            outcome.queue_high_water < total as usize,
            "the queue grew without bound"
        );
        assert!(
            outcome.queue_high_water * 256 * 4 <= stream.config().max_host_staging_bytes as usize,
            "the live blocks exceeded the staging budget"
        );
        assert!(outcome.is_complete());
    }

    #[test]
    fn the_output_queue_never_exceeds_the_concurrency_budget_even_at_depth_sixty_four() {
        let d = descriptor(vec![64, 64], DType::F32);
        let shard = shard_for(&d);
        let mut stream =
            BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8)).unwrap();
        // MAX_OUTPUT_QUEUE_DEPTH is 64, MAX_CONCURRENT_BLOCKS is 4. Because
        // this queue carries *decoded blocks*, the residency budget binds.
        assert_eq!(stream.queue_capacity(), MAX_CONCURRENT_BLOCKS);
        let outcome = stream.drive_bounded(|_| Ok(())).unwrap();
        assert!(
            outcome.queue_high_water <= MAX_CONCURRENT_BLOCKS + 2,
            "queue high water {} exceeded MAX_CONCURRENT_BLOCKS + 2",
            outcome.queue_high_water
        );
        assert_eq!(outcome.blocks_emitted, 64);
    }

    #[test]
    fn a_sink_error_stops_the_bounded_pass_rather_than_draining_the_queue() {
        let d = descriptor(vec![64, 64], DType::F32);
        let shard = shard_for(&d);
        let mut stream =
            BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8)).unwrap();
        let mut seen = 0;
        let err = stream
            .drive_bounded(|_| {
                seen += 1;
                if seen == 3 {
                    Err(QError::QueryRejected("sink stopped".into()))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(err.to_string().contains("sink stopped"));
        assert!(seen == 3, "the sink saw {seen} blocks");
    }

    #[test]
    fn driving_on_one_thread_visits_every_block_in_grid_order() {
        let d = descriptor(vec![32, 24], DType::F32);
        let shard = shard_for(&d);
        let mut stream =
            BlockStream::new(&shard, d, BlockStreamConfig::default().with_block(8, 8)).unwrap();
        let mut order = Vec::new();
        let outcome = stream
            .drive(|block| {
                order.push((block.extent.row_start, block.extent.column_start));
                Ok(())
            })
            .unwrap();
        assert_eq!(outcome.blocks_emitted, 4 * 3);
        assert_eq!(outcome.bytes_read, 32 * 24 * 4);
        assert!(outcome.is_complete());
        assert_eq!(order[0], (0, 0));
        assert_eq!(order[1], (0, 8));
        assert_eq!(order[3], (8, 0));
    }

    // --- adaptive halving ----------------------------------------------------

    /// Refuses any reservation at or above `refuse_at_or_above`.
    struct FailingProbe {
        refuse_at_or_above: u64,
    }

    impl StagingProbe for FailingProbe {
        fn can_reserve(&self, bytes: u64) -> bool {
            bytes < self.refuse_at_or_above
        }
    }

    #[test]
    fn allocation_failure_halves_the_block_and_completes() {
        let d = descriptor(vec![512, 512], DType::F32);
        let shard = shard_for(&d);
        // 4 x 256 x 256 x 4 = 1 MiB is refused; 4 x 128 x 128 x 4 = 256 KiB
        // is not, so the pass must land on 128x128.
        let probe = FailingProbe {
            refuse_at_or_above: 1024 * 1024,
        };
        let mut stream =
            BlockStream::with_staging_probe(&shard, d, BlockStreamConfig::default(), &probe)
                .unwrap();
        assert_eq!(stream.halvings(), 1);
        assert_eq!(
            (stream.config().block_rows, stream.config().block_columns),
            (128, 128)
        );
        assert_eq!(
            (
                stream.requested_config().block_rows,
                stream.requested_config().block_columns
            ),
            (256, 256),
            "the requested size is still reportable"
        );
        assert_eq!(stream.block_count(), 16);
        let outcome = stream.drive(|_| Ok(())).unwrap();
        assert!(outcome.is_complete());
        assert_eq!(outcome.blocks_emitted, 16);
        assert_eq!(outcome.bytes_read, 512 * 512 * 4);
        assert_eq!(outcome.halvings, 1);
    }

    #[test]
    fn allocation_failure_below_the_floor_fails_naming_the_budget() {
        let d = descriptor(vec![512, 512], DType::F32);
        let shard = shard_for(&d);
        let probe = FailingProbe {
            refuse_at_or_above: 1,
        };
        let err = BlockStream::with_staging_probe(&shard, d, BlockStreamConfig::default(), &probe)
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("host_staging"), "message was {msg}");
        assert!(msg.contains("64x64"), "message was {msg}");
        assert!(msg.contains("min_block_dimension 64"), "message was {msg}");
        assert!(msg.contains("2 halving(s)"), "message was {msg}");
        assert_eq!(shard.reads(), 0, "the refusal must cost no I/O");
    }

    #[test]
    fn halving_stops_at_the_floor_rather_than_reaching_a_degenerate_block() {
        let mut config = BlockStreamConfig::default();
        let mut edges = vec![(config.block_rows, config.block_columns)];
        while let Some(next) = config.halved() {
            config = next;
            edges.push((config.block_rows, config.block_columns));
        }
        assert_eq!(edges, vec![(256, 256), (128, 128), (64, 64)]);
        assert_eq!(config.block_rows, MIN_BLOCK_DIMENSION);
    }

    #[test]
    fn the_real_allocator_probe_admits_the_default_staging_and_refuses_the_absurd() {
        let probe = SystemStagingProbe;
        assert!(probe.can_reserve(BlockStreamConfig::default().host_staging_bytes()));
        assert!(!probe.can_reserve(u64::MAX));
    }

    // --- the residency arithmetic itself ------------------------------------

    // --- the configured resident ceiling `C` (`QM-0101`, gate G1) ------------

    /// The accounted residency is the *formula*, not a measurement, and it must
    /// match the bound `drive_bounded` actually holds. If the two ever disagree
    /// the admission check would be admitting against the wrong number.
    #[test]
    fn accounted_residency_is_the_live_block_bound_the_bounded_queue_actually_holds() {
        let config = BlockStreamConfig::default();
        // (min(64, 4) + 2) x 256 x 256 x 4 + 256 x 8.
        assert_eq!(config.queue_capacity(), MAX_CONCURRENT_BLOCKS);
        assert_eq!(
            config.accounted_resident_bytes(),
            (MAX_CONCURRENT_BLOCKS as u64 + LIVE_BLOCKS_OVER_QUEUE_CAPACITY) * 256 * 256 * 4
                + 256 * WIDEST_DTYPE_BYTES
        );
        assert_eq!(config.accounted_resident_bytes(), 1_574_912);
        // It exceeds `host_staging_bytes` by exactly the two extra live blocks
        // plus the run buffer, which is what makes it a *different* budget rather
        // than a renaming of the same one.
        assert_eq!(
            config.accounted_resident_bytes() - config.host_staging_bytes(),
            2 * 256 * 256 * 4 + 256 * WIDEST_DTYPE_BYTES
        );
        // Halving the block edge quarters the block area, so residency tracks
        // block size — the property the whole design rests on.
        let small = config.with_block(64, 64);
        assert_eq!(
            small.accounted_resident_bytes(),
            6 * 64 * 64 * 4 + 64 * WIDEST_DTYPE_BYTES
        );
        assert!(small.accounted_resident_bytes() * 15 < config.accounted_resident_bytes());
        // And it is independent of any tensor: no descriptor was involved above.
    }

    #[test]
    fn the_bounded_queues_high_water_never_exceeds_what_the_accounted_residency_assumed() {
        let d = descriptor(vec![256, 256], DType::F32);
        let shard = shard_for(&d);
        let config = BlockStreamConfig::default().with_block(32, 32);
        let mut stream = BlockStream::new(&shard, d, config).unwrap();
        let outcome = stream
            .drive_bounded(|_| {
                std::thread::sleep(Duration::from_micros(50));
                Ok(())
            })
            .unwrap();
        let assumed = outcome.queue_capacity as u64 + LIVE_BLOCKS_OVER_QUEUE_CAPACITY;
        assert!(
            outcome.queue_high_water as u64 <= assumed,
            "high water {} exceeded the {assumed} live blocks the residency arithmetic \
             assumed, so accounted_resident_bytes understates the real peak",
            outcome.queue_high_water
        );
        assert_eq!(outcome.blocks_emitted, 64);
    }

    #[test]
    fn a_resident_ceiling_below_the_accounted_residency_is_refused_naming_max_resident() {
        let d = descriptor(vec![512, 512], DType::F32);
        let shard = shard_for(&d);
        let needed = BlockStreamConfig::default().accounted_resident_bytes();
        let config = BlockStreamConfig::default().with_max_resident_bytes(needed - 1);
        let err = BlockStream::new(&shard, d.clone(), config).unwrap_err();
        match &err {
            QError::BudgetExceeded {
                budget_name,
                requested,
                limit,
            } => {
                assert_eq!(*budget_name, "max_resident");
                assert_eq!((*requested, *limit), (needed, needed - 1));
            }
            other => panic!("expected BudgetExceeded naming max_resident, got {other:?}"),
        }
        assert_eq!(shard.reads(), 0, "the refusal must cost no I/O");
        // Exactly the needed amount is admitted: a boundary, not a blanket.
        assert!(BlockStream::new(
            &shard,
            d,
            BlockStreamConfig::default().with_max_resident_bytes(needed)
        )
        .is_ok());
    }

    /// The staging budget is checked *before* the resident ceiling, so a
    /// configuration that breaks both reports the tighter, more specific one.
    #[test]
    fn the_staging_budget_is_reported_before_the_resident_ceiling_when_both_are_exceeded() {
        let config = BlockStreamConfig::default()
            .with_max_host_staging_bytes(512 * 1024)
            .with_max_resident_bytes(1024);
        match config.validate() {
            Err(QError::BudgetExceeded { budget_name, .. }) => {
                assert_eq!(budget_name, "host_staging")
            }
            other => panic!("expected the host_staging refusal first, got {other:?}"),
        }
    }

    /// The default ceiling is the compiled 2 GiB, so an existing caller that
    /// never heard of `QM-0101` keeps working unchanged.
    #[test]
    fn the_default_configuration_carries_the_compiled_two_gibibyte_ceiling_and_validates() {
        let config = BlockStreamConfig::default();
        assert_eq!(config.max_resident_bytes, MAX_RESIDENT_BYTES);
        assert_eq!(config.resident_budget().name, "max_resident");
        assert!(config.validate().is_ok());
        assert!(config.accounted_resident_bytes() < config.max_resident_bytes);
    }

    /// `.plan/MEMORY_BUDGET.md` §11's chain has to reach this configuration, or
    /// it is a chain nothing consults.
    #[test]
    fn a_configuration_built_from_resolved_budgets_carries_every_one_of_them() {
        use q_source::config::{BudgetFlags, EmptyEnv, StreamingBudgets};
        let budgets = StreamingBudgets::resolve(
            &BudgetFlags {
                max_resident_bytes: Some(3_528_244),
                max_concurrent_blocks: Some(2),
                block_rows: Some(128),
                block_columns: Some(64),
                max_output_queue_depth: Some(8),
                max_host_staging_bytes: Some(4 * 1024 * 1024),
            },
            &EmptyEnv,
            None,
        )
        .unwrap();
        let config = BlockStreamConfig::from_budgets(&budgets);
        assert_eq!(config.max_resident_bytes, 3_528_244);
        assert_eq!(config.max_concurrent_blocks, 2);
        assert_eq!((config.block_rows, config.block_columns), (128, 64));
        assert_eq!(config.max_output_queue_depth, 8);
        assert_eq!(config.max_host_staging_bytes, 4 * 1024 * 1024);
        // The halving floor is not a §11 variable; it stays the compiled one.
        assert_eq!(config.min_block_dimension, MIN_BLOCK_DIMENSION);
        assert_eq!(
            config.queue_capacity(),
            2,
            "concurrency binds below depth 8"
        );
        assert!(config.validate().is_ok());

        let defaults = BlockStreamConfig::from_budgets(&StreamingBudgets::compiled_defaults());
        assert_eq!(defaults, BlockStreamConfig::default());
    }

    #[test]
    fn peak_residency_is_a_function_of_block_size_not_tensor_size() {
        let config = BlockStreamConfig::default();
        assert_eq!(config.decoded_block_bytes(), 256 * 1024);
        assert_eq!(config.host_staging_bytes(), 1024 * 1024);

        // Four tensors spanning 4096x in element count. Same buffers.
        let mut counts = Vec::new();
        for edge in [1024u64, 2048, 4096, 65536] {
            let d = descriptor(vec![edge, edge], DType::F32);
            let shard = shard_for(&d);
            let stream = BlockStream::new(&shard, d, config).unwrap();
            assert_eq!(stream.config().block_rows, 256);
            assert_eq!(stream.config().host_staging_bytes(), 1024 * 1024);
            counts.push(stream.block_count());
            assert_eq!(shard.reads(), 0);
        }
        // Only the block count grows, and it grows as the area.
        assert_eq!(counts, vec![16, 64, 256, 65536]);
    }
}
