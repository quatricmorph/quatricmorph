//! `QM-0100` — the real local checkpoint, verified from its header alone.
//!
//! Data plane: **Artifact Plane** header bytes → **Metadata Plane**
//! (ARCHITECTURE.md §2.1, §4.1). No test here reads more than the header, with
//! one deliberate exception: the five exact 4-byte scalar reads in
//! `rank_four_addressing_uses_the_true_shape_rather_than_a_flattened_one`, which
//! are the entire point of that test. The truncation fixtures stream their
//! prefix rather than slurping the file — see `truncated_copy`.
//!
//! ## What this file proves
//!
//! `models/distilbert-distilgpt2/` is the checkpoint the repository owner
//! selected for v1 (`.plan/MASTER_PLAN.md` §4). It is **gitignored**, so the
//! weights are never redistributed by this repository, and it may be deleted at
//! any time to reclaim disk. `fixtures/real-checkpoint-record.json` is the
//! committed, measured record that keeps the plan's numbers auditable after
//! that happens.
//!
//! The tests split deliberately into two groups:
//!
//! * **Record-only** tests run everywhere, including a machine that has never
//!   seen the weights. They assert the committed record is internally
//!   consistent — that its histograms, counts and byte sizes agree with each
//!   other and with the arithmetic they imply.
//! * **Checkpoint-requiring** tests assert the record against the real file.
//!   They cannot run without it. When the file is absent they print a loud
//!   reason and return; set `QM_REQUIRE_REAL_CHECKPOINT=1` to turn that absence
//!   into a hard failure (which is what a machine that is supposed to have the
//!   checkpoint should do).
//!
//! Every checkpoint-requiring test is named in `.plan/evidence/QM-0100.md`.
//!
//! ## Where the expected values come from
//!
//! Not from the code under test. The byte size, header length, tensor count,
//! dtype histogram, rank histogram and parameter count were read out of the file
//! by Python `struct` + `json` + `math.prod`, and the header hash by
//! `shasum -a 256`. Both are recorded in the evidence with their exact commands.
//! The parameter count has an independent corroboration: subtracting the six
//! non-parameter `attn.bias` causal masks (6 × 1 048 576 = 6 291 456) from
//! 88 204 032 leaves 81 912 576 — the "82 million parameters" the model card
//! itself claims.
//!
//! ## What this checkpoint does NOT cover
//!
//! Recorded here as well as in the evidence, because a claim limit that lives
//! only in a plan document is a claim limit that gets forgotten: this file is
//! **single-file** (no sharded read path), **F32 throughout** (no bf16 exact
//! decode), **GPT-2 with no experts** (no MoE expert-keyed aggregation), and
//! **~337 MiB** (no ≥ 24 GB scale claim). See `coverage_not_established` in the
//! record.

use q_safetensors::header::SafeTensorsHeader;
use q_safetensors::ingest::{ingest_local, is_single_file, CheckpointIngestor};
use q_source::error::QError;
use q_source::LocalFsSource;
use q_tensor_runtime::BlockExtent;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Independently measured constants. Source: Python, see the module docs.
// ---------------------------------------------------------------------------

const BYTES_ON_DISK: u64 = 352_824_413;
const HEADER_LENGTH: u64 = 8_277;
/// `8` (the little-endian `u64` length prefix) `+ HEADER_LENGTH`.
const DATA_OFFSET: u64 = 8_285;
const TENSOR_COUNT: usize = 82;
const PARAMETER_COUNT: u64 = 88_204_032;
const RANK4_TENSOR_COUNT: usize = 6;
/// `[1, 1, 1024, 1024]` — 1 048 576 elements each.
const RANK4_SHAPE: [u64; 4] = [1, 1, 1024, 1024];

/// The header must stay under a tenth of a percent of the file. `SRC-007`
/// asserts this property on fixtures; here it is asserted on a real artifact.
const HEADER_FRACTION_CEILING: f64 = 0.001;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<root>/tests`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the integration-test crate always has a parent directory")
        .to_path_buf()
}

fn record_path() -> PathBuf {
    repo_root().join("fixtures/real-checkpoint-record.json")
}

fn record() -> Value {
    let path = record_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the committed record {} must exist and be readable — it is the durable \
             part of QM-0100 and outlives the checkpoint itself: {e}",
            path.display()
        )
    });
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

fn u64_field(rec: &Value, key: &str) -> u64 {
    rec.get(key)
        .unwrap_or_else(|| panic!("the record must carry a `{key}` field"))
        .as_u64()
        .unwrap_or_else(|| panic!("`{key}` must be a non-negative integer"))
}

fn histogram(rec: &Value, key: &str) -> BTreeMap<String, u64> {
    rec.get(key)
        .unwrap_or_else(|| panic!("the record must carry a `{key}` field"))
        .as_object()
        .unwrap_or_else(|| panic!("`{key}` must be a JSON object"))
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.as_u64()
                    .unwrap_or_else(|| panic!("`{key}.{k}` must be a count")),
            )
        })
        .collect()
}

/// The real checkpoint directory, or `None` with a loud reason.
///
/// `models/` is gitignored, so a fresh clone on another machine will not have
/// it. Absence must never read as success: it prints why it is skipping, and
/// `QM_REQUIRE_REAL_CHECKPOINT=1` promotes the skip to a failure.
fn checkpoint_dir() -> Option<PathBuf> {
    let dir = repo_root().join("models/distilbert-distilgpt2");
    if dir.join("model.safetensors").is_file() {
        return Some(dir);
    }
    let reason = format!(
        "SKIPPED — NOT PROVEN: {} is absent. `models/` is gitignored (see .gitignore \
         `/models/`), so the weights are not in the repository and this machine does not \
         have them. This test asserts nothing until the checkpoint is present. Set \
         QM_REQUIRE_REAL_CHECKPOINT=1 to make this absence a hard failure. \
         See .plan/evidence/QM-0100.md.",
        dir.join("model.safetensors").display()
    );
    // Deliberately not `is_some()`: a reviewer who sets the variable to `0` to
    // switch the requirement *off* must not get the opposite of what they asked.
    let required = std::env::var("QM_REQUIRE_REAL_CHECKPOINT")
        .map(|v| {
            !matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false" | "no"
            )
        })
        .unwrap_or(false);
    if required {
        panic!("{reason}");
    }
    eprintln!("{reason}");
    None
}

// ===========================================================================
// Record-only. These run on every machine, checkpoint or not.
// ===========================================================================

#[test]
fn the_record_is_committed_and_names_the_checkpoint_the_owner_selected() {
    let rec = record();
    assert_eq!(
        rec.get("name").and_then(Value::as_str),
        Some("distilbert-distilgpt2"),
        "the record must name the checkpoint `.plan/MASTER_PLAN.md` §4 selects"
    );
    assert_eq!(
        rec.get("architecture").and_then(Value::as_str),
        Some("gpt2"),
        "architecture is read from models/distilbert-distilgpt2/config.json `model_type`"
    );
}

#[test]
fn the_dtype_and_rank_histograms_each_account_for_every_tensor() {
    let rec = record();
    let tensors = u64_field(&rec, "tensor_count");
    let dtypes: u64 = histogram(&rec, "dtypes").values().sum();
    let ranks: u64 = histogram(&rec, "rank_histogram").values().sum();
    assert_eq!(
        dtypes, tensors,
        "the dtype histogram must classify every tensor exactly once"
    );
    assert_eq!(
        ranks, tensors,
        "the rank histogram must classify every tensor exactly once"
    );
}

#[test]
fn the_recorded_parameter_count_is_consistent_with_an_f32_only_payload() {
    let rec = record();
    // Every tensor is F32, so the described payload is exactly 4 bytes per
    // element, and it must fit between the end of the header and the end of
    // the file. This ties parameter_count, dtypes, header_length_bytes and
    // bytes_on_disk together: change any one and this fails.
    let params = u64_field(&rec, "parameter_count");
    let payload = params * 4;
    let data_offset = u64_field(&rec, "data_offset_bytes");
    let size = u64_field(&rec, "bytes_on_disk");
    assert_eq!(
        data_offset + payload,
        size,
        "8 + header_length + 4 bytes per F32 element must account for the whole file"
    );
}

#[test]
fn the_recorded_header_is_under_a_tenth_of_a_percent_of_the_file() {
    let rec = record();
    let header = u64_field(&rec, "data_offset_bytes") as f64;
    let size = u64_field(&rec, "bytes_on_disk") as f64;
    let fraction = header / size;
    assert!(
        fraction < HEADER_FRACTION_CEILING,
        "header is {header} of {size} bytes = {:.6} % of the file, which must stay \
         under {} %",
        fraction * 100.0,
        HEADER_FRACTION_CEILING * 100.0
    );

    // The record stores that percentage as its own field. Recompute it and
    // compare, so the stored value cannot drift away from the byte counts it is
    // supposed to summarise.
    let recorded = rec
        .get("header_fraction_of_file_percent")
        .and_then(Value::as_f64)
        .expect("the record must carry `header_fraction_of_file_percent`");
    assert!(
        (recorded - fraction * 100.0).abs() < 1e-12,
        "the recorded header fraction {recorded} % disagrees with the {:.12} % implied \
         by the recorded byte counts",
        fraction * 100.0
    );
}

#[test]
fn the_declared_data_offset_is_eight_bytes_past_the_header_length() {
    let rec = record();
    assert_eq!(
        u64_field(&rec, "data_offset_bytes"),
        8 + u64_field(&rec, "header_length_bytes"),
        "SafeTensors puts the payload at 8 + N; the record must not disagree with the format"
    );
}

#[test]
fn the_record_declares_a_single_shard_and_no_index_json() {
    let rec = record();
    assert_eq!(
        u64_field(&rec, "shard_count"),
        1,
        "this checkpoint is a single file"
    );
    assert_eq!(
        rec.get("has_index_json").and_then(Value::as_bool),
        Some(false),
        "there is no model.safetensors.index.json — the sharded read path is NOT \
         exercised by this checkpoint"
    );
}

#[test]
fn the_record_states_the_licence_read_from_the_model_card_rather_than_guessing() {
    let rec = record();
    let licence = rec
        .get("licence")
        .and_then(Value::as_str)
        .expect("the record must carry a `licence` field");
    assert_eq!(
        licence, "apache-2.0",
        "read from the `license:` front matter of models/distilbert-distilgpt2/README.md"
    );
    let source = rec
        .get("licence_source")
        .and_then(Value::as_str)
        .expect("the record must say which file the licence came from");
    assert!(
        source.contains("README.md"),
        "the licence must cite the tracked file it was read from, got `{source}`"
    );
}

#[test]
fn unmeasurable_provenance_is_null_rather_than_an_invented_uri_or_hash() {
    let rec = record();
    // Nothing in the tree records where this checkpoint was fetched from or at
    // which revision. A plausible-looking invention would be worse than an
    // honest null, so the record must carry a null (or "not verified") and this
    // test is what stops a later edit from quietly filling one in.
    for key in ["source_uri", "revision"] {
        let value = rec
            .get(key)
            .unwrap_or_else(|| panic!("the record must carry a `{key}` field"));
        let acceptable = value.is_null() || value.as_str() == Some("not verified");
        assert!(
            acceptable,
            "`{key}` is `{value}`. No file in the tree records it, so it must stay \
             null or \"not verified\" — never a plausible-looking invented value"
        );
    }
}

#[test]
fn the_record_says_exactly_which_bytes_the_header_hash_covers() {
    let rec = record();
    let digest = rec
        .get("sha256_of_header")
        .and_then(Value::as_str)
        .expect("the record must carry `sha256_of_header`");
    assert_eq!(
        digest.len(),
        64,
        "a SHA-256 digest is 64 hex characters, got `{digest}`"
    );
    assert!(
        digest.chars().all(|c| c.is_ascii_hexdigit()),
        "`sha256_of_header` must be hex, got `{digest}`"
    );
    let covers = rec
        .get("sha256_of_header_covers")
        .and_then(Value::as_str)
        .expect("the record must say which byte range was hashed — there is no index.json");
    assert!(
        covers.contains("8285"),
        "the hashed range must be stated explicitly, got `{covers}`"
    );
}

#[test]
fn the_record_states_the_coverage_this_checkpoint_does_not_provide() {
    // Acceptance criterion 10, made durable. The MVP concession that swapped a
    // 24 GB sharded MoE bf16 checkpoint for a 337 MiB single-file F32 GPT-2 is
    // only honest if the gap travels with the artifact.
    let rec = record();
    let gaps = rec
        .get("coverage_not_established")
        .and_then(Value::as_array)
        .expect("the record must list the coverage this checkpoint does not provide");
    let text = gaps
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    for expected in ["shard", "bf16", "expert", "24 gb"] {
        assert!(
            text.contains(expected),
            "the record must state plainly that `{expected}` coverage is not established \
             by this checkpoint; got: {text}"
        );
    }
    assert_eq!(
        rec.get("has_experts").and_then(Value::as_bool),
        Some(false),
        "GPT-2 has no experts, so MoE expert-keyed aggregation has no real fixture here"
    );
    assert!(
        !histogram(&rec, "dtypes").contains_key("BF16"),
        "this checkpoint is F32 throughout; the bf16 exact-decode path is not exercised"
    );
}

// ===========================================================================
// Checkpoint-requiring. These assert the record against the real file.
// ===========================================================================

#[test]
fn the_real_checkpoint_header_matches_every_measured_field_of_the_record() {
    let Some(dir) = checkpoint_dir() else { return };
    let rec = record();

    let file = dir.join("model.safetensors");
    let size = std::fs::metadata(&file).expect("checkpoint metadata").len();
    assert_eq!(size, BYTES_ON_DISK, "byte size on disk");
    assert_eq!(
        u64_field(&rec, "bytes_on_disk"),
        size,
        "record vs. file size"
    );

    let source = LocalFsSource::open(&dir).expect("open the checkpoint directory");
    let header = SafeTensorsHeader::read_from(&source, "model.safetensors", size)
        .expect("the real header must parse");

    assert_eq!(header.header_length, HEADER_LENGTH, "header length N");
    assert_eq!(header.data_offset, DATA_OFFSET, "payload begins at 8 + N");
    assert_eq!(header.tensor_count(), TENSOR_COUNT, "tensor count");
    assert_eq!(
        header.metadata.get("format").map(String::as_str),
        Some("pt"),
        "__metadata__ is `{{\"format\": \"pt\"}}`"
    );
    assert_eq!(u64_field(&rec, "header_length_bytes"), header.header_length);
    assert_eq!(u64_field(&rec, "data_offset_bytes"), header.data_offset);
    assert_eq!(
        u64_field(&rec, "tensor_count"),
        header.tensor_count() as u64
    );

    let mut dtypes: BTreeMap<String, u64> = BTreeMap::new();
    let mut ranks: BTreeMap<String, u64> = BTreeMap::new();
    let mut parameters = 0u64;
    for (_, entry) in &header.tensors {
        *dtypes.entry(entry.dtype.clone()).or_default() += 1;
        *ranks.entry(entry.shape.len().to_string()).or_default() += 1;
        parameters += entry.element_count();
    }
    assert_eq!(parameters, PARAMETER_COUNT, "parameter count");
    assert_eq!(u64_field(&rec, "parameter_count"), parameters);
    assert_eq!(dtypes, histogram(&rec, "dtypes"), "dtype histogram");
    assert_eq!(ranks, histogram(&rec, "rank_histogram"), "rank histogram");
    assert_eq!(
        dtypes.get("F32").copied(),
        Some(TENSOR_COUNT as u64),
        "every tensor is F32 — this checkpoint does not exercise bf16"
    );
}

#[test]
fn indexing_the_real_checkpoint_reads_only_its_header_bytes() {
    let Some(dir) = checkpoint_dir() else { return };
    let out = ingest_local(&dir).expect("the real checkpoint must ingest");

    assert_eq!(
        out.bytes_read, DATA_OFFSET,
        "ingestion must read exactly the 8-byte length prefix plus the {HEADER_LENGTH}-byte \
         header and nothing else"
    );
    assert_eq!(out.tensor_count(), TENSOR_COUNT);
    assert_eq!(out.total_parameters(), PARAMETER_COUNT);
    assert_eq!(
        out.described_payload_bytes,
        BYTES_ON_DISK - DATA_OFFSET,
        "the described payload is the whole file minus its header, none of it read"
    );

    let fraction = out.bytes_read as f64 / BYTES_ON_DISK as f64;
    assert!(
        fraction < HEADER_FRACTION_CEILING,
        "read {} of {BYTES_ON_DISK} bytes = {:.6} %, which must stay under {} %",
        out.bytes_read,
        fraction * 100.0,
        HEADER_FRACTION_CEILING * 100.0
    );
}

#[test]
fn the_real_checkpoint_resolves_as_a_single_file_without_an_index_json() {
    let Some(dir) = checkpoint_dir() else { return };
    assert!(
        !dir.join("model.safetensors.index.json").exists(),
        "this checkpoint has no index JSON; if one appears, the Selection table in \
         QM-0100 and the record's shard_count are both wrong"
    );
    let out = ingest_local(&dir).expect("ingest");
    assert!(
        out.shard_index.is_none(),
        "no index JSON means no ShardIndex — the sharded attribution path is NOT \
         exercised by this checkpoint"
    );
    assert!(is_single_file(&out.manifest));
    assert_eq!(out.manifest.shards().count(), 1);
    assert_eq!(
        out.manifest.model_type().as_deref(),
        Some("gpt2"),
        "config.json declares model_type gpt2"
    );
    assert!(
        out.descriptors
            .iter()
            .all(|d| d.shard_uri == "model.safetensors"),
        "every tensor lives in the one file"
    );
}

/// **ADR-010, metadata layer.** ADR-010 places the rank ceiling at the axis
/// binding, block, tile and layout layers — *not* here. It says so explicitly:
/// `q_source::TensorDescriptor::shape: Vec<u64>` is "arbitrary rank already",
/// and "the metadata layer is rank-agnostic". So ingestion, and therefore
/// `q inspect`, must list the six rank-4 causal masks **at their true shape**.
/// Making `inspect` refuse them would contradict ADR-010, not honour it.
///
/// This pins that rank-agnosticism so a later change cannot quietly reshape,
/// collapse, or drop them on the way through.
#[test]
fn inspect_lists_rank_four_tensors_at_their_true_shape() {
    let Some(dir) = checkpoint_dir() else { return };
    let out = ingest_local(&dir).expect("ingest");

    // Nothing was dropped: all 82 arrive, and exactly six of them are rank 4.
    assert_eq!(
        out.tensor_count(),
        TENSOR_COUNT,
        "no tensor may be silently skipped because its rank is inconvenient"
    );
    let rank4: Vec<_> = out
        .descriptors
        .iter()
        .filter(|d| d.shape.len() == 4)
        .collect();
    assert_eq!(
        rank4.len(),
        RANK4_TENSOR_COUNT,
        "the six causal-mask buffers must all survive ingestion as rank 4"
    );

    for d in &rank4 {
        // Not flattened: not collapsed to [1024, 1024], not to [1, 1048576],
        // not to [1048576].
        assert_eq!(
            d.shape.as_slice(),
            &RANK4_SHAPE,
            "{} must keep its declared rank-4 shape; any reshape here would be the \
             flattening ADR-010 forbids",
            d.raw_name
        );
        assert_eq!(d.element_count(), 1_048_576);
        assert_eq!(d.byte_length(), 1_048_576 * 4);
    }

    // Exactly the six the record names, by name.
    let mut names: Vec<&str> = rank4.iter().map(|d| d.raw_name.as_str()).collect();
    names.sort_unstable();
    let expected: Vec<String> = (0..6)
        .map(|n| format!("transformer.h.{n}.attn.bias"))
        .collect();
    assert_eq!(
        names,
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );

    let rec = record();
    let recorded: Vec<&str> = rec
        .get("rank4_tensor_names")
        .and_then(Value::as_array)
        .expect("the record must name the rank-4 tensors")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(recorded, names, "record vs. file, by tensor name");
}

/// The sharpest available proof that nothing flattens: exact scalar addressing
/// against the real rank-4 causal mask.
///
/// `transformer.h.0.attn.bias` is a lower-triangular mask, so it is
/// **asymmetric**: `[0,0,5,3]` is 1.0 and `[0,0,3,5]` is 0.0. A flattened or
/// mis-strided read cannot reproduce that pair, and it cannot land on the right
/// byte offsets either. Both the values and the absolute file offsets below were
/// produced by Python `struct.unpack('<f')` seeking directly into the file — an
/// independent implementation, not this codebase.
///
/// A rank-2 selector against the same tensor is refused rather than broadcast
/// or padded, which is the other half of the property.
#[test]
fn rank_four_addressing_uses_the_true_shape_rather_than_a_flattened_one() {
    let Some(dir) = checkpoint_dir() else { return };
    let out = ingest_local(&dir).expect("ingest");
    let source = LocalFsSource::open(&dir).expect("open");
    let mask = out
        .find("transformer.h.0.attn.bias")
        .expect("the rank-4 causal mask must be present");
    assert_eq!(mask.shape.as_slice(), &RANK4_SHAPE);

    // (index, expected value, expected absolute byte offset) — from Python.
    let cases: [(&[u64], f64, u64); 5] = [
        (&[0, 0, 0, 0], 1.0, 8_285),
        (&[0, 0, 3, 5], 0.0, 20_593),
        (&[0, 0, 5, 3], 1.0, 28_777),
        (&[0, 0, 1023, 1023], 1.0, 4_202_585),
        (&[0, 0, 0, 1023], 0.0, 12_377),
    ];
    for (index, expected_value, expected_offset) in cases {
        let read = q_safetensors::read::read_scalar(&source, mask, index)
            .unwrap_or_else(|e| panic!("exact read at {index:?} must succeed: {e}"));
        assert_eq!(
            read.value, expected_value,
            "value at {index:?} disagrees with an independent Python read of the same file"
        );
        assert_eq!(
            read.byte_offset, expected_offset,
            "byte offset for {index:?} disagrees with the rank-4 row-major stride; a \
             flattened shape would land somewhere else"
        );
        assert_eq!(read.bytes_read, 4, "one F32 element, nothing more");
    }

    // The asymmetry is the point: these two indices differ only by transposition
    // of the last two axes, so equal values would mean the strides are wrong.
    let lower = q_safetensors::read::read_scalar(&source, mask, &[0, 0, 5, 3]).unwrap();
    let upper = q_safetensors::read::read_scalar(&source, mask, &[0, 0, 3, 5]).unwrap();
    assert_ne!(
        lower.value, upper.value,
        "the causal mask is lower-triangular; identical values here would mean the \
         rank-4 tensor was being read as something other than what it is"
    );

    // And a rank-2 selector is refused, not silently padded or broadcast. The
    // refusal reports the tensor's *true* shape back to the caller, which is
    // what makes it actionable rather than merely negative.
    let err = q_safetensors::read::read_scalar(&source, mask, &[0, 0])
        .expect_err("a 2-element index must not address a rank-4 tensor");
    match &err {
        QError::IndexOutOfBounds { shape, index, .. } => {
            assert_eq!(
                shape.as_slice(),
                &RANK4_SHAPE,
                "the refusal must quote the real rank-4 shape, not a flattened one"
            );
            assert_eq!(index.as_slice(), &[0, 0]);
        }
        other => panic!("expected an out-of-bounds refusal naming the shape, got: {other}"),
    }
    // (Through the CLI the NSIR address layer refuses this earlier still, with
    // "2-D selector applied to a rank-4 tensor" — recorded in the evidence.)
}

/// **ADR-010's spirit, at the layers that exist today.** ADR-010's designed
/// refusal — `bindAxes()` returning `NotImplemented` carrying `GRID-007` — is
/// **not implemented anywhere in this tree**; it is owned by `QM-0061` (and
/// `QM-0040` for the block planner), and is outside `QM-0100`'s declared file
/// scope. See `.plan/evidence/QM-0100.md` and `.plan/PLAN_CHANGELOG.md`.
///
/// What *does* exist is a set of rank-2 preconditions on the 2-D read and block
/// paths. They are incidental preconditions rather than the ADR's axis binding,
/// but they carry the property that matters here: presented with a real rank-4
/// tensor, every one of them **refuses with context naming the rank** instead of
/// reinterpreting the buffer as a 2-D matrix. This test pins that, so that no
/// future change "fixes" a rank-4 failure by flattening.
#[test]
fn every_two_dimensional_read_path_refuses_rank_four_rather_than_flattening_it() {
    let Some(dir) = checkpoint_dir() else { return };
    let out = ingest_local(&dir).expect("ingest");
    let source = LocalFsSource::open(&dir).expect("open");

    let rank4: Vec<_> = out
        .descriptors
        .iter()
        .filter(|d| d.shape.len() == 4)
        .collect();
    assert_eq!(rank4.len(), RANK4_TENSOR_COUNT);

    for d in &rank4 {
        let err = BlockExtent::new(0, 4, 0, 4)
            .expect("a 4x4 extent is well formed")
            .clamped_to(&d.shape)
            .expect_err("a rank-4 shape must not be admitted to a 2-D block extent");
        let message = err.to_string();
        assert!(
            matches!(err, QError::QueryRejected(_)),
            "the refusal must be a rejection, not an incidental failure: {message}"
        );
        assert!(
            message.contains("rank-2") && message.contains("rank 4"),
            "the refusal must name what it got and what it needs: {message}"
        );

        let slice_err = q_safetensors::read::read_slice_2d(&source, d, (0, 4), (0, 4))
            .expect_err("a rank-4 tensor must not be sliced as if it were 2-D");
        let slice_message = slice_err.to_string();
        assert!(
            slice_message.contains("rank 4") && slice_message.contains("requires rank 2"),
            "the slice refusal must carry context: {slice_message}"
        );

        let row_err = q_safetensors::read::read_row(&source, d, 0)
            .expect_err("a rank-4 tensor must not be read row-wise as if it were 2-D");
        assert!(row_err.to_string().contains("rank 4"));
    }
}

// ===========================================================================
// Negative paths against copies. The real checkpoint is never modified.
// ===========================================================================

/// Build a checkpoint directory in a temp dir holding the first `bytes` bytes of
/// the real file. The original is opened read-only and never written.
///
/// The prefix is **streamed** through `Read::take` + `io::copy` rather than
/// slurped with `fs::read`. This file's whole thesis is that nothing allocates
/// proportionally to checkpoint size, and that has to hold for its own fixtures
/// too — reading 352 MB into a `Vec` to build a 20 KB fixture would contradict
/// the property the suite exists to prove.
fn truncated_copy(dir: &Path, bytes: u64) -> tempfile::TempDir {
    let source = dir.join("model.safetensors");
    let file = std::fs::File::open(&source).expect("open the real checkpoint read-only");
    let length = file.metadata().expect("checkpoint metadata").len();
    assert!(
        bytes < length,
        "a truncation must actually remove something"
    );

    let temp = tempfile::tempdir().expect("temp dir");
    let mut out =
        std::fs::File::create(temp.path().join("model.safetensors")).expect("create the copy");
    let copied = std::io::copy(&mut file.take(bytes), &mut out).expect("stream the prefix");
    assert_eq!(copied, bytes, "the copy must be exactly the truncation");
    std::fs::copy(dir.join("config.json"), temp.path().join("config.json"))
        .expect("copy config.json");
    temp
}

#[test]
fn a_truncated_copy_of_the_checkpoint_is_refused_with_context() {
    let Some(dir) = checkpoint_dir() else { return };
    // 20 000 bytes: the 8 277-byte header survives intact, so the file parses as
    // far as its declared offsets — which then run past the end of the truncated
    // payload. `SRC-015`.
    let temp = truncated_copy(&dir, 20_000);
    let err = ingest_local(temp.path()).expect_err("a truncated checkpoint must be refused");
    let message = err.to_string();
    assert!(
        matches!(err, QError::RangeOutOfBounds { .. }),
        "truncation must surface as an out-of-bounds range, not a panic or a partial \
         success: {message}"
    );
    assert!(
        message.contains("model.safetensors"),
        "the refusal must name the artifact it refused: {message}"
    );
}

#[test]
fn a_copy_whose_declared_header_length_exceeds_the_file_is_refused_before_allocating() {
    let Some(dir) = checkpoint_dir() else { return };
    // 4 000 bytes: the leading u64 still declares an 8 277-byte header, but the
    // file is shorter than that. The reader must refuse on the declared length
    // alone, before it allocates a buffer for a header that is not there.
    // `SRC-013`.
    let temp = truncated_copy(&dir, 4_000);
    let err = ingest_local(temp.path()).expect_err("an over-declared header must be refused");
    let message = err.to_string();
    assert!(
        matches!(err, QError::MalformedArtifact { .. }),
        "expected a malformed-artifact refusal, got: {message}"
    );
    assert!(
        message.contains("declared header length 8277") && message.contains("4000"),
        "the refusal must state both the declared length and the real file length: {message}"
    );
}

#[test]
fn a_hostile_header_length_is_refused_against_the_named_budget_not_allocated() {
    // No checkpoint needed: this is the allocation-bomb case, and it must be
    // provable on any machine. A file declaring a u64::MAX header must be
    // refused by the `safetensors_header` budget before a buffer is reserved.
    let temp = tempfile::tempdir().expect("temp dir");
    let mut bytes = u64::MAX.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"{}");
    std::fs::write(temp.path().join("model.safetensors"), &bytes).expect("write");
    let source = LocalFsSource::open(temp.path()).expect("open");
    let err = CheckpointIngestor::new(&source)
        .ingest()
        .expect_err("a u64::MAX header length must be refused");
    match err {
        QError::BudgetExceeded {
            budget_name,
            requested,
            limit,
        } => {
            assert_eq!(budget_name, "safetensors_header");
            assert_eq!(requested, u64::MAX);
            assert_eq!(limit, 100 * 1024 * 1024);
        }
        other => panic!("expected a BudgetExceeded refusal, got: {other}"),
    }
}
