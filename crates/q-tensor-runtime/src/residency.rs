//! Data plane: **Tensor Tile Plane**, reading the **Artifact Plane**
//! (ARCHITECTURE.md §2.1, §14.2).
//!
//! One bounded pass over every streamable tensor of a checkpoint — `QM-0101`,
//! gate **G1**, `PERF-002`, `V1-03`…`V1-05`.
//!
//! # What this module is for
//!
//! [`stream::BlockStream`](crate::stream::BlockStream) bounds one *tensor*.
//! `QM-0030` proved that: 268,355 B of peak heap, identical at 1024², 2048²,
//! 4096² and a 65536² descriptor. What it deliberately did **not** claim is gate
//! `G1`, because `G1` is about a *checkpoint* and about a **configured ceiling**
//! that did not exist in this tree.
//!
//! This module is the checkpoint-level pass, and the ceiling now exists:
//! [`q_source::budget::MAX_RESIDENT_BYTES`], resolved through
//! `.plan/MEMORY_BUDGET.md` §11's precedence chain and enforced by
//! [`BlockStreamConfig::validate`] **before any byte is read**.
//!
//! # Why every byte is folded into a checksum
//!
//! `.plan/tasks/QM-0101-…/TASK.md`, `## Data Contracts`: *"without it, a
//! sufficiently clever compiler or a bug that skips blocks produces an excellent
//! residency number for the wrong reason."* A pass that reads nothing has
//! superb residency.
//!
//! The fold here is stronger than that requirement in two ways, and both are
//! load-bearing rather than decorative:
//!
//! * it is **position-sensitive**, so a duplicated block, a swapped pair of
//!   values, or a block read at the wrong offset changes the result — a plain sum
//!   of values notices none of those;
//! * it is **order-independent** (contributions combine by wrapping addition), so
//!   the total does not depend on the block size or the visiting order. That is
//!   what makes `checksum(256×256) == checksum(64×64)` assertable, and what makes
//!   `checksum(interrupted) + checksum(resumed) == checksum(whole)` an exact
//!   identity rather than an approximation.
//!
//! # What is refused rather than flattened
//!
//! A real checkpoint is mostly not rank-2. `models/distilbert-distilgpt2` holds
//! 50 rank-1 tensors and 6 rank-4 causal masks; `fixtures/tiny-llama-2shard`
//! holds 25 rank-1 norms. `ADR-010` requires rank above 3 to **refuse rather than
//! flatten**, and `BlockStream` needs a 2-D extent, so those tensors cannot
//! stream.
//!
//! They are therefore **recorded, counted, and reconciled** — never skipped
//! silently. [`ResidencyOutcome::reconciles_against`] is the tie:
//!
//! ```text
//! bytes_streamed + refused_payload_bytes == described_payload_bytes
//! ```
//!
//! A pass that dropped a tensor on the floor fails that equality, so "every byte
//! of every streamable tensor, exactly once" is a checked claim rather than an
//! assertion of good intent.

use crate::stream::{BlockStream, BlockStreamConfig};
use q_source::error::Result;
use q_source::{CancellationToken, ModelSource, TensorDescriptor};
use serde::Serialize;

/// One tensor the pass refused, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TensorRefusal {
    pub canonical_name: String,
    pub shape: Vec<u64>,
    pub dtype: String,
    /// Payload bytes this tensor describes, and which the pass therefore did not
    /// read. Summed into [`ResidencyOutcome::refused_payload_bytes`].
    pub payload_bytes: u64,
    pub reason: String,
    /// The requirement covering the gap, when the refusal carries one —
    /// `GRID-007` for rank above 2, for instance.
    pub requirement_id: Option<String>,
}

/// What one pass was asked to do.
///
/// `Default` is derived, so the default request is exactly
/// `BlockStreamConfig::default()` with no early stop, no resume, and refusals
/// reported rather than fatal.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResidencyRequest {
    pub config: BlockStreamConfig,
    /// Stop at a block boundary once this many blocks have been emitted.
    ///
    /// A count rather than a signal so the cancellation path is reproducible: a
    /// `SIGINT` race cannot be asserted.
    pub stop_after_blocks: Option<u64>,
    /// Start at this absolute block index in pass order.
    pub resume_from_block: Option<u64>,
    /// Turn a refusal into a hard failure.
    ///
    /// Off by default because it must be: a real checkpoint's rank-1 and rank-4
    /// tensors are refused *correctly*, and a pass that demanded every tensor
    /// would fail on `ADR-010` behaving as designed.
    pub require_all_tensors: bool,
}

/// What one pass actually did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResidencyOutcome {
    /// Payload bytes read and decoded, each exactly once.
    pub bytes_streamed: u64,
    pub blocks_streamed: u64,
    /// Blocks in the grids of every streamable tensor, whether or not this pass
    /// reached them. Arithmetic over the shapes; costs no I/O.
    pub blocks_planned: u64,
    /// Position- and value-sensitive fold over every decoded element.
    pub checksum: u64,
    pub tensors_total: usize,
    /// Tensors this pass read at least one block from.
    pub tensors_streamed: usize,
    /// Tensors passed over because they lie entirely before the resume point.
    pub tensors_skipped_before_resume: usize,
    pub tensors_refused: usize,
    pub refused_payload_bytes: u64,
    /// `Some(where)` when the pass stopped early at a block boundary.
    pub stopped_at: Option<String>,
    /// Absolute block index a resumed pass must start from.
    pub next_block_index: u64,
    pub refusals: Vec<TensorRefusal>,
}

impl ResidencyOutcome {
    /// Every block of every streamable tensor was read.
    pub fn is_complete(&self) -> bool {
        self.stopped_at.is_none()
            && self.blocks_streamed + self.skipped_blocks() == self.blocks_planned
    }

    /// Blocks the resume point passed over without reading.
    fn skipped_blocks(&self) -> u64 {
        self.next_block_index.saturating_sub(self.blocks_streamed)
    }

    /// The tie that proves nothing was silently dropped:
    /// `bytes_streamed + refused_payload_bytes == described_payload_bytes`.
    ///
    /// Only meaningful for a pass that ran to completion from the start; an
    /// interrupted or resumed pass has read less than it planned by design, which
    /// is why this takes the described total rather than asserting on its own.
    pub fn reconciles_against(&self, described_payload_bytes: u64) -> bool {
        self.stopped_at.is_none()
            && self.next_block_index == self.blocks_planned
            && self.blocks_streamed == self.blocks_planned
            && self.bytes_streamed + self.refused_payload_bytes == described_payload_bytes
    }
}

/// A stable per-tensor key, so the checksum does not depend on the order tensors
/// are visited in. FNV-1a over the raw header name.
pub fn tensor_key(raw_name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in raw_name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// One element's contribution to the checksum. See the module documentation for
/// why it is position-sensitive and order-independent.
///
/// Not a cryptographic hash, and not presented as one.
pub fn fold_element(key: u64, row: u64, column: u64, bits: u32) -> u64 {
    let mut w = key ^ row.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    w ^= column.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    w ^= (bits as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    // splitmix64's finalizer: cheap, and it diffuses every input bit, so one
    // flipped mantissa bit changes the fold.
    w = (w ^ (w >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    w = (w ^ (w >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    w ^ (w >> 31)
}

/// The pass order: derived from the artifact, not from header iteration order.
///
/// `--resume-from-block N` has to name the same block on every run, on every
/// machine, so the order is `(shard, byte offset, name)` — all three from the
/// descriptor itself. A `HashMap` iteration order would make resume a lottery.
pub fn pass_order(descriptors: &[TensorDescriptor]) -> Vec<TensorDescriptor> {
    let mut out = descriptors.to_vec();
    out.sort_by(|a, b| {
        a.shard_uri
            .cmp(&b.shard_uri)
            .then(a.byte_start.cmp(&b.byte_start))
            .then(a.raw_name.cmp(&b.raw_name))
    });
    out
}

/// The pass plan: which tensors stream, how many blocks each contributes, and
/// which are refused — all of it **arithmetic over the shapes, reading nothing**.
///
/// Computed as its own phase so that `resume_from_block` can be validated against
/// the real block count *before* any I/O. Without it, an out-of-range resume index
/// walks the whole pass reading nothing and then reports a byte total that fails
/// to reconcile — which looks like a dropped tensor rather than like the bad
/// argument it is.
pub struct PassPlan {
    /// Descriptors that will stream, in pass order, with each one's block count.
    pub streamable: Vec<(TensorDescriptor, u64)>,
    pub refusals: Vec<TensorRefusal>,
    pub refused_payload_bytes: u64,
    pub blocks_planned: u64,
    pub tensors_total: usize,
}

/// Build the plan. Reads nothing: [`BlockStream::new`] validates and derives the
/// grid from metadata alone (`TILE-002`).
pub fn plan(
    source: &dyn ModelSource,
    descriptors: &[TensorDescriptor],
    request: &ResidencyRequest,
) -> Result<PassPlan> {
    request.config.validate()?;
    let ordered = pass_order(descriptors);
    let mut plan = PassPlan {
        streamable: Vec::new(),
        refusals: Vec::new(),
        refused_payload_bytes: 0,
        blocks_planned: 0,
        tensors_total: ordered.len(),
    };
    for descriptor in ordered {
        match BlockStream::new(source, descriptor.clone(), request.config) {
            Ok(stream) => {
                let count = stream.block_count();
                plan.blocks_planned += count;
                plan.streamable.push((descriptor, count));
            }
            Err(e) => {
                if request.require_all_tensors {
                    return Err(e);
                }
                plan.refused_payload_bytes += descriptor.byte_length();
                plan.refusals.push(TensorRefusal {
                    canonical_name: descriptor.canonical_name.clone(),
                    shape: descriptor.shape.clone(),
                    dtype: descriptor.dtype.as_safetensors_str().to_string(),
                    payload_bytes: descriptor.byte_length(),
                    reason: e.to_string(),
                    requirement_id: e.requirement_id().map(str::to_string),
                });
            }
        }
    }
    Ok(plan)
}

/// Stream every streamable tensor's blocks through the bounded reader.
///
/// The order of operations is the substance of gate `G1`:
///
/// 1. **admit the configuration against the configured resident ceiling `C`** —
///    [`BlockStreamConfig::validate`], before a single payload byte is read, so
///    an over-budget run costs no I/O at all and refuses naming `max_resident`;
/// 2. build the plan and validate the resume index against it — still no I/O;
/// 3. stream, folding the checksum, holding at most one decoded block.
///
/// Step 1 is why this can fail. A ceiling below what the configured block size
/// and concurrency need is refused, which is what makes `G1` a gate rather than a
/// report.
pub fn run(
    source: &dyn ModelSource,
    descriptors: &[TensorDescriptor],
    request: &ResidencyRequest,
) -> Result<ResidencyOutcome> {
    // Admission and planning first. Nothing below this line has read a byte.
    let PassPlan {
        streamable,
        refusals,
        refused_payload_bytes,
        blocks_planned,
        tensors_total,
    } = plan(source, descriptors, request)?;

    let resume_from = request.resume_from_block.unwrap_or(0);
    if resume_from > blocks_planned {
        return Err(q_source::error::QError::QueryRejected(format!(
            "cannot resume at block {resume_from}: this pass plans {blocks_planned} blocks \
             over {} streamable tensor(s). An out-of-range resume index is refused rather \
             than read as an empty pass, which would report zero bytes streamed and look \
             like a dropped tensor",
            streamable.len()
        )));
    }
    if let Some(0) = request.stop_after_blocks {
        return Err(q_source::error::QError::QueryRejected(
            "cannot stop after 0 blocks: a pass that is cancelled before its first block \
             cannot be told from one that read nothing because it was broken"
                .to_string(),
        ));
    }

    let cancel = CancellationToken::new();
    let mut checksum = 0u64;
    let mut bytes_streamed = 0u64;
    let mut blocks_streamed = 0u64;
    let mut global_index = 0u64;
    let mut tensors_streamed = 0usize;
    let mut tensors_skipped_before_resume = 0usize;
    let mut stopped_at: Option<String> = None;

    for (descriptor, count) in &streamable {
        let count = *count;
        if stopped_at.is_some() {
            continue;
        }
        // Whole tensors before the resume point are passed over by arithmetic —
        // the grid is a pure function of shape and block size, so no I/O is
        // needed to know how many blocks lie behind the resume point.
        if global_index + count <= resume_from {
            global_index += count;
            tensors_skipped_before_resume += 1;
            continue;
        }
        let stream = BlockStream::new(source, descriptor.clone(), request.config)?;
        let start_within = resume_from.saturating_sub(global_index);
        let mut stream = if start_within > 0 {
            stream.resume_from(start_within)?
        } else {
            stream
        };
        stream = stream.with_cancellation(cancel.clone());

        let key = tensor_key(&descriptor.raw_name);
        let stop_after = request.stop_after_blocks;
        let outcome = stream.drive(|block| {
            let columns = block.extent.columns() as usize;
            for (offset, value) in block.data.values.iter().enumerate() {
                checksum = checksum.wrapping_add(fold_element(
                    key,
                    block.extent.row_start + (offset / columns) as u64,
                    block.extent.column_start + (offset % columns) as u64,
                    value.to_bits(),
                ));
            }
            bytes_streamed += block.bytes_read;
            blocks_streamed += 1;
            if let Some(limit) = stop_after {
                if blocks_streamed >= limit {
                    // Cancellation is checked *between* blocks, so the block just
                    // handed over is whole and the stream stops at its boundary.
                    cancel.cancel();
                }
            }
            Ok(())
        })?;

        // `next_block_index` is absolute within this tensor's grid — the resume
        // point seeds it and every emitted block advances it — so adding it to
        // the blocks of preceding tensors gives the pass's absolute resume point.
        global_index += outcome.next_block_index;
        if let Some(at) = &outcome.cancelled_at {
            stopped_at = Some(format!("{} ({at})", descriptor.canonical_name));
        }
        tensors_streamed += 1;
    }

    Ok(ResidencyOutcome {
        bytes_streamed,
        blocks_streamed,
        blocks_planned,
        checksum,
        tensors_total,
        tensors_streamed,
        tensors_skipped_before_resume,
        tensors_refused: refusals.len(),
        refused_payload_bytes,
        stopped_at,
        next_block_index: global_index,
        refusals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_source::budget::MemoryBudget;
    use q_source::error::QError;
    use q_source::manifest::{ByteStream, ModelManifest};
    use q_source::role::TensorRole;
    use q_source::{DType, ModelId, TensorId};

    const HEADER: u64 = 8;

    /// A shard whose bytes are generated on demand, so a "tensor" of any
    /// declared size costs no disk. Defined here so expected values never come
    /// from the code under test.
    struct SyntheticShard {
        length: u64,
    }

    impl SyntheticShard {
        fn byte_at(offset: u64) -> u8 {
            (offset.wrapping_mul(2_654_435_761) >> 11) as u8
        }
    }

    impl ModelSource for SyntheticShard {
        fn manifest(&self) -> Result<ModelManifest> {
            Err(QError::NotFound(
                "the residency harness has no manifest".into(),
            ))
        }

        fn read_range(&self, uri: &str, offset: u64, length: u64) -> Result<ByteStream> {
            let end = offset
                .checked_add(length)
                .ok_or_else(|| QError::RangeOutOfBounds {
                    uri: uri.to_string(),
                    start: offset,
                    end: u64::MAX,
                    length: self.length,
                })?;
            if end > self.length {
                return Err(QError::RangeOutOfBounds {
                    uri: uri.to_string(),
                    start: offset,
                    end,
                    length: self.length,
                });
            }
            let mut out = vec![0u8; length as usize];
            for (i, b) in out.iter_mut().enumerate() {
                *b = Self::byte_at(offset + i as u64);
            }
            Ok(ByteStream::from_vec(out))
        }
    }

    fn descriptor(name: &str, start: u64, shape: Vec<u64>, dtype: DType) -> TensorDescriptor {
        let elements: u64 = shape.iter().product();
        TensorDescriptor {
            tensor_id: TensorId::derive(ModelId::derive("residency", "", "f"), name),
            raw_name: name.into(),
            canonical_name: name.into(),
            byte_start: start,
            byte_end: start + elements * dtype.size_in_bytes(),
            shape,
            dtype,
            shard_uri: "model.safetensors".into(),
            layer_index: None,
            semantic_role: TensorRole::Unknown,
        }
    }

    /// Two rank-2 tensors, one rank-1, one rank-4 — the shape of a real
    /// checkpoint in miniature.
    fn mixed_rank_checkpoint() -> (Vec<TensorDescriptor>, SyntheticShard, u64) {
        let a = descriptor("a.weight", HEADER, vec![32, 32], DType::F32);
        let b = descriptor("b.weight", a.byte_end, vec![16, 48], DType::F32);
        let norm = descriptor("c.norm", b.byte_end, vec![32], DType::F32);
        let mask = descriptor("d.mask", norm.byte_end, vec![1, 1, 8, 8], DType::F32);
        let described = a.byte_length() + b.byte_length() + norm.byte_length() + mask.byte_length();
        let shard = SyntheticShard {
            length: mask.byte_end,
        };
        (vec![a, b, norm, mask], shard, described)
    }

    fn small_config() -> BlockStreamConfig {
        BlockStreamConfig::default().with_block(8, 8)
    }

    #[test]
    fn a_full_pass_streams_every_streamable_byte_and_reconciles_the_refused_ones() {
        let (descriptors, shard, described) = mixed_rank_checkpoint();
        let outcome = run(
            &shard,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                ..Default::default()
            },
        )
        .unwrap();

        // Hand-computed: 32×32 is 4×4 blocks of 8×8, 16×48 is 2×6.
        assert_eq!(outcome.blocks_planned, 16 + 12);
        assert_eq!(outcome.blocks_streamed, 28);
        assert_eq!(outcome.bytes_streamed, (32 * 32 + 16 * 48) * 4);
        assert_eq!(outcome.tensors_total, 4);
        assert_eq!(outcome.tensors_streamed, 2);
        assert_eq!(outcome.tensors_refused, 2);
        // The rank-1 norm (32 × 4 B) and the rank-4 mask (64 × 4 B).
        assert_eq!(outcome.refused_payload_bytes, 32 * 4 + 64 * 4);
        assert!(outcome.is_complete());
        assert!(
            outcome.reconciles_against(described),
            "streamed {} + refused {} != described {described}",
            outcome.bytes_streamed,
            outcome.refused_payload_bytes
        );
        assert_ne!(outcome.checksum, 0);
    }

    #[test]
    fn a_rank_four_tensor_is_refused_carrying_grid_007_rather_than_flattened() {
        let (descriptors, shard, _) = mixed_rank_checkpoint();
        let outcome = run(
            &shard,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                ..Default::default()
            },
        )
        .unwrap();
        let mask = outcome
            .refusals
            .iter()
            .find(|r| r.canonical_name == "d.mask")
            .expect("the rank-4 mask must be recorded, not dropped");
        assert_eq!(mask.shape, vec![1, 1, 8, 8]);
        assert_eq!(mask.requirement_id.as_deref(), Some("GRID-007"));
        assert!(
            mask.reason.contains("refused rather than flattened"),
            "reason was {}",
            mask.reason
        );
        assert!(
            mask.reason.contains("ADR-010"),
            "reason was {}",
            mask.reason
        );
        // A rank-1 tensor is refused too, and for a different, honest reason: it
        // has no second axis to bind, which is `QM-0061`'s work, not a flattening.
        let norm = outcome
            .refusals
            .iter()
            .find(|r| r.canonical_name == "c.norm")
            .expect("the rank-1 norm must be recorded");
        assert!(norm.reason.contains("rank 1"), "reason was {}", norm.reason);
    }

    #[test]
    fn require_all_tensors_turns_a_correct_refusal_into_a_hard_failure() {
        let (descriptors, shard, _) = mixed_rank_checkpoint();
        let err = run(
            &shard,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                require_all_tensors: true,
                ..Default::default()
            },
        )
        .unwrap_err();
        // Pass order is (shard, offset, name), so the rank-1 norm comes first.
        assert!(err.to_string().contains("rank 1"), "error was {err}");
    }

    /// The property that makes the checksum a real check: it does not depend on
    /// how the tensors were cut into blocks, only on which bytes were read and
    /// where they were.
    #[test]
    fn the_checksum_is_identical_at_every_block_size_that_divides_the_pass() {
        let (descriptors, shard, described) = mixed_rank_checkpoint();
        let mut checksums = Vec::new();
        let mut block_counts = Vec::new();
        for (rows, columns) in [(8u64, 8u64), (4, 4), (16, 16), (32, 48)] {
            let outcome = run(
                &shard,
                &descriptors,
                &ResidencyRequest {
                    config: BlockStreamConfig::default().with_block(rows, columns),
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(outcome.reconciles_against(described));
            assert_eq!(outcome.bytes_streamed, (32 * 32 + 16 * 48) * 4);
            checksums.push(outcome.checksum);
            block_counts.push(outcome.blocks_planned);
        }
        assert_eq!(
            checksums
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            1,
            "the checksum moved with the block size: {checksums:?}"
        );
        // And the block counts really did differ, so the invariance was tested
        // rather than accidentally satisfied by four identical passes.
        assert!(
            block_counts
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1,
            "every block size produced the same grid, so nothing was varied: {block_counts:?}"
        );
    }

    #[test]
    fn a_flipped_bit_anywhere_in_the_payload_changes_the_checksum() {
        // Not a claim about collision resistance — a check that the fold actually
        // depends on the values, which a fold that summed nothing would not.
        let base = fold_element(7, 3, 5, 0x3F80_0000);
        assert_ne!(base, fold_element(7, 3, 5, 0x3F80_0001), "mantissa bit");
        assert_ne!(base, fold_element(7, 3, 6, 0x3F80_0000), "column");
        assert_ne!(base, fold_element(7, 4, 5, 0x3F80_0000), "row");
        assert_ne!(base, fold_element(8, 3, 5, 0x3F80_0000), "tensor");
        // Position sensitivity is what catches a transposed read: two values
        // swapped between (3,5) and (5,3) must not cancel out.
        let straight = fold_element(7, 3, 5, 1).wrapping_add(fold_element(7, 5, 3, 2));
        let swapped = fold_element(7, 3, 5, 2).wrapping_add(fold_element(7, 5, 3, 1));
        assert_ne!(straight, swapped);
    }

    #[test]
    fn cancellation_stops_at_a_block_boundary_and_names_where_to_resume() {
        let (descriptors, shard, _) = mixed_rank_checkpoint();
        let request = ResidencyRequest {
            config: small_config(),
            stop_after_blocks: Some(10),
            ..Default::default()
        };
        let outcome = run(&shard, &descriptors, &request).unwrap();
        assert_eq!(outcome.blocks_streamed, 10);
        assert_eq!(outcome.next_block_index, 10);
        assert_eq!(outcome.bytes_streamed, 10 * 8 * 8 * 4);
        assert!(!outcome.is_complete());
        let at = outcome
            .stopped_at
            .expect("a stop must say where it stopped");
        assert!(at.contains("a.weight"), "stopped_at was {at}");
        assert!(at.contains("block 10"), "stopped_at was {at}");
        // `blocks_planned` still describes the whole pass, not only the part that
        // ran — otherwise a stopped pass would look complete.
        assert_eq!(outcome.blocks_planned, 28);
    }

    /// The strongest statement the pass can make about resume, and it is an
    /// exact identity rather than a tolerance: an interrupted pass plus its
    /// resumption equals one uninterrupted pass, in bytes **and** in checksum.
    #[test]
    fn an_interrupted_pass_plus_its_resumption_equals_one_uninterrupted_pass_exactly() {
        let (descriptors, shard, described) = mixed_rank_checkpoint();
        let whole = run(
            &shard,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(whole.reconciles_against(described));

        for stop in [1u64, 10, 16, 27] {
            let first = run(
                &shard,
                &descriptors,
                &ResidencyRequest {
                    config: small_config(),
                    stop_after_blocks: Some(stop),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(first.next_block_index, stop);

            let second = run(
                &shard,
                &descriptors,
                &ResidencyRequest {
                    config: small_config(),
                    resume_from_block: Some(first.next_block_index),
                    ..Default::default()
                },
            )
            .unwrap();

            assert_eq!(
                first.blocks_streamed + second.blocks_streamed,
                whole.blocks_streamed,
                "stop at {stop}: block counts do not rejoin"
            );
            assert_eq!(
                first.bytes_streamed + second.bytes_streamed,
                whole.bytes_streamed,
                "stop at {stop}: byte counts do not rejoin"
            );
            assert_eq!(
                first.checksum.wrapping_add(second.checksum),
                whole.checksum,
                "stop at {stop}: the resumed pass did not read exactly the blocks the \
                 interrupted one missed"
            );
            assert_eq!(second.next_block_index, whole.blocks_planned);
        }
    }

    #[test]
    fn resuming_exactly_at_the_end_reads_nothing_and_resuming_past_it_is_refused() {
        let (descriptors, shard, _) = mixed_rank_checkpoint();
        let at_end = run(
            &shard,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                resume_from_block: Some(28),
                ..Default::default()
            },
        )
        .unwrap();
        // Resuming at exactly the end is legitimate — it is what a completed pass
        // reports as its resume point — and it reads nothing.
        assert_eq!(at_end.blocks_streamed, 0);
        assert_eq!(at_end.bytes_streamed, 0);
        assert_eq!(at_end.checksum, 0);
        assert_eq!(at_end.next_block_index, 28);
        assert_eq!(at_end.tensors_skipped_before_resume, 2);

        // One past the end is an out-of-range index and is refused naming both
        // the index and the real block count, rather than reported as an empty
        // pass whose byte total then fails to reconcile.
        let err = run(
            &shard,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                resume_from_block: Some(29),
                ..Default::default()
            },
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("block 29"), "message was {msg}");
        assert!(msg.contains("28 blocks"), "message was {msg}");
        assert!(msg.contains("out-of-range"), "message was {msg}");
    }

    #[test]
    fn stopping_after_zero_blocks_is_refused_rather_than_reporting_an_empty_pass() {
        let (descriptors, shard, _) = mixed_rank_checkpoint();
        let err = run(
            &shard,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                stop_after_blocks: Some(0),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("stop after 0 blocks"), "{err}");
    }

    /// The plan is pure arithmetic over the shapes and must cost no I/O — that is
    /// what lets an out-of-range resume index be refused before a byte is read.
    #[test]
    fn the_plan_derives_every_block_count_without_reading_anything() {
        let (descriptors, shard, _) = mixed_rank_checkpoint();
        let counting = CountingShard {
            inner: shard,
            reads: std::sync::atomic::AtomicUsize::new(0),
        };
        let p = plan(
            &counting,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(p.blocks_planned, 28);
        assert_eq!(p.streamable.len(), 2);
        assert_eq!(p.refusals.len(), 2);
        assert_eq!(p.tensors_total, 4);
        assert_eq!(p.refused_payload_bytes, 32 * 4 + 64 * 4);
        assert_eq!(
            counting.reads.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "planning must read nothing; the grid is arithmetic over the shape"
        );
        // Pass order: the two rank-2 tensors, by byte offset.
        assert_eq!(p.streamable[0].0.raw_name, "a.weight");
        assert_eq!(p.streamable[1].0.raw_name, "b.weight");
        assert_eq!((p.streamable[0].1, p.streamable[1].1), (16, 12));
    }

    /// Counts `read_range` calls so "reads nothing" is measured, not assumed.
    struct CountingShard {
        inner: SyntheticShard,
        reads: std::sync::atomic::AtomicUsize,
    }

    impl ModelSource for CountingShard {
        fn manifest(&self) -> Result<ModelManifest> {
            self.inner.manifest()
        }
        fn read_range(&self, uri: &str, offset: u64, length: u64) -> Result<ByteStream> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.read_range(uri, offset, length)
        }
    }

    /// The admission check, and the reason `G1` is a gate. It happens before any
    /// read, so an over-budget run costs no I/O.
    #[test]
    fn a_resident_ceiling_below_the_passes_own_buffers_is_refused_naming_max_resident() {
        let (descriptors, shard, _) = mixed_rank_checkpoint();
        let config = BlockStreamConfig::default();
        let needed = config.accounted_resident_bytes();
        let request = ResidencyRequest {
            config: config.with_max_resident_bytes(needed - 1),
            ..Default::default()
        };
        let err = run(&shard, &descriptors, &request).unwrap_err();
        match &err {
            QError::BudgetExceeded {
                budget_name,
                requested,
                limit,
            } => {
                assert_eq!(*budget_name, "max_resident");
                assert_eq!(*requested, needed);
                assert_eq!(*limit, needed - 1);
            }
            other => panic!("expected BudgetExceeded, got {other:?}"),
        }
        assert_eq!(MemoryBudget::resident().name, "max_resident");
        // And one byte more is admitted, so the refusal is a boundary rather than
        // a blanket.
        assert!(run(
            &shard,
            &descriptors,
            &ResidencyRequest {
                config: config.with_max_resident_bytes(needed),
                ..Default::default()
            }
        )
        .is_ok());
    }

    #[test]
    fn a_truncated_shard_stops_the_pass_naming_the_range_rather_than_zero_filling() {
        let (descriptors, _, _) = mixed_rank_checkpoint();
        // Two rows short of what the first tensor's grid needs.
        let truncated = SyntheticShard {
            length: HEADER + 30 * 32 * 4,
        };
        let err = run(
            &truncated,
            &descriptors,
            &ResidencyRequest {
                config: small_config(),
                ..Default::default()
            },
        )
        .unwrap_err();
        match &err {
            QError::RangeOutOfBounds { end, length, .. } => {
                assert!(*end > *length, "the refused range must overrun the shard");
                assert_eq!(*length, HEADER + 30 * 32 * 4);
            }
            other => panic!("expected RangeOutOfBounds, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_dtype_is_refused_rather_than_guessed_and_its_bytes_are_still_reconciled() {
        let good = descriptor("a.weight", HEADER, vec![8, 8], DType::F32);
        let exotic = descriptor("b.weight", good.byte_end, vec![8, 8], DType::F8E4M3);
        let described = good.byte_length() + exotic.byte_length();
        let shard = SyntheticShard {
            length: exotic.byte_end,
        };
        let outcome = run(
            &shard,
            &[good, exotic],
            &ResidencyRequest {
                config: small_config(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(outcome.tensors_refused, 1);
        assert_eq!(outcome.refusals[0].dtype, "F8_E4M3");
        assert!(
            outcome.refusals[0].reason.contains("F8_E4M3"),
            "reason was {}",
            outcome.refusals[0].reason
        );
        assert!(outcome.reconciles_against(described));
    }

    #[test]
    fn the_pass_order_is_derived_from_the_artifact_so_resume_names_the_same_block_every_run() {
        let (descriptors, _, _) = mixed_rank_checkpoint();
        let forward = pass_order(&descriptors);
        let mut reversed = descriptors.clone();
        reversed.reverse();
        let from_reversed = pass_order(&reversed);
        assert_eq!(
            forward
                .iter()
                .map(|d| d.raw_name.clone())
                .collect::<Vec<_>>(),
            from_reversed
                .iter()
                .map(|d| d.raw_name.clone())
                .collect::<Vec<_>>()
        );
        // Byte offset, not name, is the ordering key after the shard.
        assert_eq!(forward.iter().map(|d| d.byte_start).collect::<Vec<_>>(), {
            let mut s: Vec<u64> = descriptors.iter().map(|d| d.byte_start).collect();
            s.sort_unstable();
            s
        });
    }

    #[test]
    fn a_checkpoint_with_no_streamable_tensor_reports_zero_rather_than_claiming_success() {
        let norm = descriptor("only.norm", HEADER, vec![64], DType::F32);
        let described = norm.byte_length();
        let shard = SyntheticShard {
            length: norm.byte_end,
        };
        let outcome = run(
            &shard,
            &[norm],
            &ResidencyRequest {
                config: small_config(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(outcome.blocks_planned, 0);
        assert_eq!(outcome.bytes_streamed, 0);
        assert_eq!(outcome.checksum, 0);
        assert_eq!(outcome.tensors_refused, 1);
        // It still reconciles: every described byte is accounted for as refused.
        assert!(outcome.reconciles_against(described));
    }
}
