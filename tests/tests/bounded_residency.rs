//! `QM-0101` — bounded residency against a **configured** ceiling. Gate `G1`.
//!
//! Data plane: **Artifact Plane** payload → **Tensor Tile Plane** blocks
//! (ARCHITECTURE.md §2.1, §9.3, §14.2). Nothing here writes an artifact and
//! nothing here interprets a weight.
//!
//! # The one thing this file exists to prevent
//!
//! `QM-0100` measured peak RSS honestly and then derived its ceiling from that
//! measurement: `C = R / 1.25`. Its reviewer ruled the result *"honest, though
//! near-tautological, and … not a G1 pass"*, because `peak ≤ 1.25 × C` with
//! `C = R/1.25` is an identity. **A ceiling derived from a measurement cannot test
//! that measurement.**
//!
//! So every ceiling asserted here is declared somewhere a reader can check,
//! *before* any run:
//!
//! | `C` | Declared in |
//! | --- | --- |
//! | 2 GiB | `q_source::budget::MAX_RESIDENT_BYTES`, the compiled default |
//! | 512 MiB, 64 MiB | `.plan/tasks/QM-0101-…/TASK.md`, `## Test Cases` rows 1 and 4 |
//! | 3 528 244 B | `.plan/DEFINITION_OF_DONE.md` `V1-04`: *"Against 352,824,413 bytes, `N ≥ 100` requires `C ≤ ~3.4 MB`"* |
//!
//! The last one decides `G1`. `N = 100` is true **by construction** at that `C`
//! — the falsifiable content is the measured peak, and
//! `the_mapped_read_mode_exceeds_the_same_ceiling_so_the_gate_is_seen_to_fail`
//! proves the comparison is not vacuous by showing the *same* ceiling being
//! breached, 75× over, by a real configuration of this same binary.
//!
//! # Where the expected values come from
//!
//! Not from the code under test. Peak RSS comes from `/usr/bin/time -l`, a
//! program outside this repository, and is recorded in
//! `fixtures/residency-measurements.json` with the exact command that produced
//! it. Checkpoint size, header length, tensor and rank counts come from
//! `fixtures/real-checkpoint-record.json`, which `QM-0100` derived with Python
//! `struct`. The fixture byte counts are arithmetic over the fixtures' own
//! shapes.
//!
//! # Labels
//!
//! * **exact** — byte counts, block counts, checksums, the accounted residency,
//!   and the counting-allocator heap peak in `residency_peak_allocation.rs`.
//! * **approximate** — every peak RSS figure. RSS is sampled by the kernel and
//!   includes the binary's text, its stacks, and the allocator's arenas.
//! * Nothing here is **sampled** in the statistical sense, and nothing here is a
//!   GPU, throughput, or deployment claim.

use q_source::budget::{resident_tolerance_bytes, MemoryBudget, MAX_RESIDENT_BYTES};
use q_source::config::{BudgetFlags, BudgetOrigin, EmptyEnv, StreamingBudgets};
use q_source::error::QError;
use q_source::{LocalFsSource, ModelSource};
use q_tensor_runtime::residency::{self, ResidencyRequest};
use q_tensor_runtime::stream::BlockStreamConfig;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Independently declared constants.
// ---------------------------------------------------------------------------

/// `fixtures/real-checkpoint-record.json`, measured by `QM-0100` with Python.
const CHECKPOINT_BYTES: u64 = 352_824_413;

/// `.plan/DEFINITION_OF_DONE.md` `V1-04`. The largest ceiling that satisfies the
/// plan's `N ≥ 100` on this checkpoint: `⌊352 824 413 / 100⌋`.
///
/// Derived from the **checkpoint size**, which was known before any pass ran —
/// never from a measured peak.
const G1_CEILING_BYTES: u64 = 3_528_244;

/// `.plan/MASTER_PLAN.md` §4: `N ≥ 100`.
const REQUIRED_RATIO_N: f64 = 100.0;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the integration-test crate always has a parent directory")
        .to_path_buf()
}

fn record_path() -> PathBuf {
    repo_root().join("fixtures/residency-measurements.json")
}

fn record() -> Value {
    let path = record_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the committed record {} must exist and be readable — it is the durable part of \
             QM-0101 and outlives both the checkpoint and the machine the numbers were taken \
             on: {e}",
            path.display()
        )
    });
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

fn rows() -> Vec<Value> {
    record()
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| panic!("the record must carry a `rows` array of measurements"))
}

fn u64_of(row: &Value, key: &str) -> u64 {
    row.get(key)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("row {row:?} must carry a non-negative integer `{key}`"))
}

fn str_of(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("row {row:?} must carry a string `{key}`"))
        .to_string()
}

fn row_named(label: &str) -> Value {
    rows()
        .into_iter()
        .find(|r| r.get("label").and_then(Value::as_str) == Some(label))
        .unwrap_or_else(|| panic!("the record must carry a row labelled `{label}`"))
}

/// The real checkpoint directory, or `None` with a loud reason.
///
/// `models/` is gitignored, so a fresh clone will not have it. Absence must never
/// read as success: it prints why it is skipping, and `QM_REQUIRE_REAL_CHECKPOINT=1`
/// promotes the skip to a failure. Pattern taken from `QM-0100`'s
/// `real_checkpoint_record.rs`, deliberately unchanged so a reader learns it once.
fn checkpoint_dir() -> Option<PathBuf> {
    let dir = repo_root().join("models/distilbert-distilgpt2");
    if dir.join("model.safetensors").is_file() {
        return Some(dir);
    }
    let reason = format!(
        "SKIPPED — NOT PROVEN: {} is absent. `models/` is gitignored, so the weights are not \
         in the repository and this machine does not have them. This test asserts nothing \
         until the checkpoint is present. Set QM_REQUIRE_REAL_CHECKPOINT=1 to make this \
         absence a hard failure. See .plan/evidence/QM-0101.md.",
        dir.join("model.safetensors").display()
    );
    if env_flag("QM_REQUIRE_REAL_CHECKPOINT") {
        panic!("{reason}");
    }
    eprintln!("{reason}");
    None
}

/// The release binary, or `None` with a loud reason.
///
/// `.plan/tasks/QM-0101-…/TASK.md` makes a **release** build an acceptance
/// criterion, and `cargo test` builds debug. A debug binary's RSS is a different
/// number and must not be substituted, so a missing release binary skips rather
/// than measuring the wrong thing.
fn release_binary() -> Option<PathBuf> {
    let path = repo_root().join("target/release/q");
    if path.is_file() {
        return Some(path);
    }
    let reason = format!(
        "SKIPPED — NOT PROVEN: {} is absent. QM-0101's acceptance criterion 2 requires a \
         RELEASE build, and measuring the debug binary instead would report a different \
         number under the same name. Run `cargo build --release -p q-cli` first. Set \
         QM_REQUIRE_RELEASE_BINARY=1 to make this absence a hard failure.",
        path.display()
    );
    if env_flag("QM_REQUIRE_RELEASE_BINARY") {
        panic!("{reason}");
    }
    eprintln!("{reason}");
    None
}

/// Deliberately not `is_some()`: a reviewer who sets the variable to `0` to switch
/// the requirement *off* must not get the opposite of what they asked for.
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

fn fixture(name: &str) -> PathBuf {
    repo_root()
        .join("fixtures")
        .join(name)
        .canonicalize()
        .expect("run fixtures/generate_fixtures.py")
}

/// Run one full pass over a checkpoint directory and hand back the outcome plus
/// the described payload the refusal reconciliation is checked against.
fn pass_over(
    dir: &Path,
    config: BlockStreamConfig,
    request: fn(BlockStreamConfig) -> ResidencyRequest,
) -> (residency::ResidencyOutcome, u64) {
    let ingested = q_safetensors::ingest_local(dir).expect("ingest headers");
    let source = LocalFsSource::open_without_mapping(dir).expect("open the checkpoint");
    let outcome = residency::run(&source, &ingested.descriptors, &request(config))
        .expect("the pass must complete");
    (outcome, ingested.described_payload_bytes)
}

fn plain(config: BlockStreamConfig) -> ResidencyRequest {
    ResidencyRequest {
        config,
        ..Default::default()
    }
}

/// `/usr/bin/time -l <binary> stream …`, returning the reported maximum resident
/// set size in bytes.
///
/// The measurement comes from a program outside this repository, which is the
/// point: nothing in this workspace can influence what `time` reports.
fn peak_rss_of_stream(binary: &Path, dir: &Path, ceiling: u64, io: &str) -> u64 {
    let output = Command::new("/usr/bin/time")
        .arg("-l")
        .arg(binary)
        .arg("stream")
        .arg(dir)
        .arg("--resident-ceiling")
        .arg(ceiling.to_string())
        .arg("--io")
        .arg(io)
        .output()
        .expect("/usr/bin/time -l must be available; it is macOS's and BSD's own binary");
    assert!(
        output.status.success(),
        "the streaming pass itself must exit 0; it exited {:?} with stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    // `time -l` writes its table to stderr.
    let text = String::from_utf8_lossy(&output.stderr);
    parse_max_rss(&text).unwrap_or_else(|| {
        panic!("no `maximum resident set size` line in /usr/bin/time -l output:\n{text}")
    })
}

/// Pull `maximum resident set size` out of `/usr/bin/time -l` output.
///
/// macOS reports it in **bytes**; the line is `<value>  maximum resident set
/// size`. Parsed rather than assumed positional so a future `time` that reorders
/// its table does not silently return the wrong field.
fn parse_max_rss(text: &str) -> Option<u64> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_suffix("maximum resident set size") {
            return value.trim().parse().ok();
        }
    }
    None
}

// ===========================================================================
// The committed record. These run on every machine, checkpoint or not.
// ===========================================================================

#[test]
fn the_record_is_committed_and_states_how_every_figure_was_obtained() {
    let rec = record();
    assert_eq!(
        rec.get("task").and_then(Value::as_str),
        Some("QM-0101"),
        "the record must name the task that produced it"
    );
    assert_eq!(rec.get("gate").and_then(Value::as_str), Some("G1"));
    // A peak RSS with no command beside it is an anecdote.
    let method = rec
        .get("measurement_method")
        .and_then(Value::as_str)
        .expect("the record must state how peak RSS was measured");
    assert!(
        method.contains("/usr/bin/time -l"),
        "measurement_method was {method:?}"
    );
    assert!(
        method.contains("maximum resident set size"),
        "measurement_method was {method:?}"
    );
    // RSS is approximate and the record must say so rather than leaving a reader
    // to assume the number is exact.
    assert_eq!(
        rec.get("peak_rss_label").and_then(Value::as_str),
        Some("approximate"),
        "peak RSS is sampled by the kernel and includes binary text and stacks"
    );
    assert_eq!(
        rec.get("build_profile").and_then(Value::as_str),
        Some("release"),
        "acceptance criterion 2 requires a release build"
    );
    assert!(!rows().is_empty(), "the record must carry measurements");
    for row in rows() {
        for key in [
            "label",
            "checkpoint",
            "io_mode",
            "expected",
            "command",
            "ceiling_declared_in",
        ] {
            assert!(
                row.get(key).and_then(Value::as_str).is_some(),
                "row {} is missing the string field `{key}`",
                row.get("label").unwrap_or(&Value::Null)
            );
        }
        for key in [
            "checkpoint_bytes",
            "resident_ceiling_bytes",
            "resident_tolerance_bytes",
            "peak_resident_set_size_bytes",
            "accounted_resident_bytes",
            "bytes_streamed",
        ] {
            u64_of(&row, key);
        }
    }
}

/// The tolerance is `.plan/MASTER_PLAN.md` §4's `1.25 ×`, and it is asserted
/// against the *code's* arithmetic rather than recomputed here — otherwise a
/// change to one could drift past the other unnoticed.
#[test]
fn every_recorded_tolerance_is_exactly_five_quarters_of_its_own_recorded_ceiling() {
    for row in rows() {
        let ceiling = u64_of(&row, "resident_ceiling_bytes");
        let tolerance = u64_of(&row, "resident_tolerance_bytes");
        assert_eq!(
            tolerance,
            resident_tolerance_bytes(ceiling),
            "row {}: tolerance {tolerance} is not 1.25 x ceiling {ceiling}",
            str_of(&row, "label")
        );
        assert_eq!(tolerance, ceiling * 5 / 4);
    }
}

#[test]
fn every_recorded_ratio_is_the_recorded_checkpoint_size_over_the_recorded_ceiling() {
    for row in rows() {
        let checkpoint = u64_of(&row, "checkpoint_bytes");
        let ceiling = u64_of(&row, "resident_ceiling_bytes");
        let recorded = row
            .get("ratio_n")
            .and_then(Value::as_f64)
            .unwrap_or_else(|| panic!("row {row:?} must carry `ratio_n`"));
        let expected = checkpoint as f64 / ceiling as f64;
        assert!(
            (recorded - expected).abs() < 1e-6,
            "row {}: recorded N {recorded} != {checkpoint}/{ceiling} = {expected}",
            str_of(&row, "label")
        );
    }
}

/// The heart of the record: each row says whether it was expected to hold or to
/// breach, and the recorded peak must agree. A row whose `expected` disagrees with
/// its own numbers is the failure this guards.
#[test]
fn every_row_holds_or_breaches_its_ceiling_exactly_as_the_row_declares() {
    let mut within = 0;
    let mut exceeds = 0;
    for row in rows() {
        let label = str_of(&row, "label");
        let peak = u64_of(&row, "peak_resident_set_size_bytes");
        let tolerance = u64_of(&row, "resident_tolerance_bytes");
        match str_of(&row, "expected").as_str() {
            "within" => {
                assert!(
                    peak <= tolerance,
                    "row {label} claims `within` but its peak {peak} exceeds 1.25 x C = \
                     {tolerance}"
                );
                within += 1;
            }
            "exceeds" => {
                assert!(
                    peak > tolerance,
                    "row {label} claims `exceeds` but its peak {peak} is within 1.25 x C = \
                     {tolerance}. A gate that is never seen to fail is not a gate"
                );
                exceeds += 1;
            }
            other => panic!("row {label}: `expected` must be `within` or `exceeds`, got {other:?}"),
        }
    }
    assert!(within > 0, "no row demonstrates the ceiling holding");
    assert!(
        exceeds > 0,
        "no row demonstrates the ceiling being breached, so nothing here shows the \
         comparison can fail"
    );
}

/// Every row reports the **conservative** end of its own observed range.
///
/// Peak RSS moves by tens of kilobytes between identical runs, so a single number
/// is a choice. A row claiming the ceiling holds must report its *largest* peak,
/// and the row claiming a breach must report its *smallest* — otherwise each row
/// would be quoting the flattering end of its own sample, which is how a
/// measurement becomes an advertisement.
#[test]
fn every_row_reports_the_conservative_end_of_its_own_observed_range() {
    for row in rows() {
        let label = str_of(&row, "label");
        let range = row
            .get("observed_peak_range_bytes")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("row {label} must record the range it observed"));
        assert_eq!(range.len(), 2, "row {label}: the range is [lo, hi]");
        let lo = range[0].as_u64().unwrap();
        let hi = range[1].as_u64().unwrap();
        assert!(lo <= hi, "row {label}: range {lo}..{hi} is inverted");
        let sample = u64_of(&row, "sample_size");
        assert!(
            sample >= 3,
            "row {label}: {sample} run(s) is not a sample; TASK.md's Verification Plan takes \
             repeated measurements"
        );
        let reported = u64_of(&row, "peak_resident_set_size_bytes");
        match str_of(&row, "expected").as_str() {
            "within" => assert_eq!(
                reported, hi,
                "row {label} claims the ceiling holds, so it must report the LARGEST peak it \
                 saw ({hi}), not {reported}"
            ),
            "exceeds" => assert_eq!(
                reported, lo,
                "row {label} claims a breach, so it must report the SMALLEST peak it saw \
                 ({lo}), not {reported}"
            ),
            other => panic!("row {label}: unexpected `expected` {other:?}"),
        }
        // And for a breach, *every* run in the sample must have breached — one
        // run over the line inside a sample that mostly held would not
        // demonstrate that the configuration cannot hold the ceiling.
        if str_of(&row, "expected") == "exceeds" {
            assert!(
                lo > u64_of(&row, "resident_tolerance_bytes"),
                "row {label}: the smallest observed peak {lo} is within the tolerance, so not \
                 every run in the sample breached"
            );
        }
    }
}

/// The anti-tautology assertion, and the one that answers `QM-0100`'s reviewer.
#[test]
fn the_g1_ceiling_is_derived_from_the_checkpoint_size_and_never_from_a_measured_peak() {
    let row = row_named("g1-headline");
    let ceiling = u64_of(&row, "resident_ceiling_bytes");
    let checkpoint = u64_of(&row, "checkpoint_bytes");
    let peak = u64_of(&row, "peak_resident_set_size_bytes");

    assert_eq!(checkpoint, CHECKPOINT_BYTES);
    assert_eq!(ceiling, G1_CEILING_BYTES);
    // Derived from the checkpoint: C = floor(checkpoint / 100).
    assert_eq!(
        ceiling,
        checkpoint / 100,
        "the G1 ceiling must be the largest one satisfying N >= 100 on this checkpoint"
    );
    // NOT derived from the measurement. `QM-0100` used C = R/1.25; if this
    // ceiling equalled that, the comparison below would be an identity.
    assert_ne!(
        ceiling,
        peak * 4 / 5,
        "the ceiling equals the measured peak / 1.25 — that is the tautology QM-0100's \
         reviewer rejected, and it would make `peak <= 1.25 x C` unfalsifiable"
    );
    // And the citation for where the rule comes from is in the record itself.
    let declared = str_of(&row, "ceiling_declared_in");
    assert!(
        declared.contains("DEFINITION_OF_DONE") && declared.contains("V1-04"),
        "the G1 ceiling must cite where it is declared; got {declared:?}"
    );
}

#[test]
fn the_g1_row_clears_every_conjunct_the_plan_requires_at_that_ceiling() {
    let row = row_named("g1-headline");
    let ceiling = u64_of(&row, "resident_ceiling_bytes");
    let peak = u64_of(&row, "peak_resident_set_size_bytes");
    let tolerance = u64_of(&row, "resident_tolerance_bytes");
    let ratio = row.get("ratio_n").and_then(Value::as_f64).unwrap();

    // `.plan/MASTER_PLAN.md` §4, all three rows of the table.
    assert!(ceiling <= 2 * 1024 * 1024 * 1024, "C must be <= 2 GiB");
    assert!(
        ratio >= REQUIRED_RATIO_N,
        "N = {ratio} does not reach the plan's 100"
    );
    assert!(
        peak <= tolerance,
        "peak RSS {peak} exceeds 1.25 x C = {tolerance}"
    );
    // The pass must have actually streamed, or the residency figure is about a
    // program that did nothing.
    assert!(u64_of(&row, "bytes_streamed") > 300_000_000);
    assert_eq!(str_of(&row, "io_mode"), "pread");
    assert_eq!(str_of(&row, "expected"), "within");
}

/// The gate seen to fail, made permanent. Without this row the `within`
/// assertions above would be consistent with a comparison that cannot fail.
#[test]
fn the_record_carries_the_same_ceiling_being_breached_so_the_gate_is_falsifiable() {
    let held = row_named("g1-headline");
    let breached = row_named("g1-ceiling-breached-by-mmap");
    assert_eq!(
        u64_of(&held, "resident_ceiling_bytes"),
        u64_of(&breached, "resident_ceiling_bytes"),
        "the breach must be against the SAME ceiling, or it demonstrates nothing about it"
    );
    assert_eq!(
        u64_of(&held, "checkpoint_bytes"),
        u64_of(&breached, "checkpoint_bytes"),
        "and over the same checkpoint"
    );
    assert_eq!(str_of(&breached, "io_mode"), "mmap");
    assert_eq!(str_of(&breached, "expected"), "exceeds");
    let peak = u64_of(&breached, "peak_resident_set_size_bytes");
    let tolerance = u64_of(&breached, "resident_tolerance_bytes");
    assert!(peak > tolerance * 10, "the breach is {peak} vs {tolerance}");
    // Same work, same bytes, same checksum — only the read mode differs. That is
    // what makes this a measurement failure rather than a different program.
    assert_eq!(
        str_of(&held, "checksum"),
        str_of(&breached, "checksum"),
        "the breaching run must have done the identical work"
    );
    assert_eq!(
        u64_of(&held, "bytes_streamed"),
        u64_of(&breached, "bytes_streamed")
    );
}

/// `V1-05`'s **exact** half, guarded so it cannot drift once recorded.
///
/// The RSS rows corroborate flatness; this block carries it, because RSS at these
/// magnitudes is dominated by the binary's own text and would look flat whatever
/// the streaming code did.
#[test]
fn the_recorded_exact_heap_figures_are_flat_and_come_from_the_counting_allocator() {
    let block = record()
        .get("exact_heap_flatness")
        .cloned()
        .expect("the record must carry the exact counting-allocator figures");
    assert_eq!(
        block.get("metric").and_then(Value::as_str),
        Some("peak heap high-water, counting #[global_allocator], EXACT"),
        "the exact figure must say what it measures and that it is exact"
    );
    let measured_by = block.get("measured_by").and_then(Value::as_str).unwrap();
    assert!(
        measured_by.contains("residency_peak_allocation.rs"),
        "the record must name the test that produces these figures; got {measured_by:?}"
    );
    let peaks: Vec<u64> = [
        "tiny_llama_single_bytes",
        "tiny_llama_2shard_bytes",
        "distilbert_distilgpt2_bytes",
    ]
    .iter()
    .map(|k| {
        block
            .get(*k)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("the exact block must carry `{k}`"))
    })
    .collect();
    let ceiling = block
        .get("declared_ceiling_bytes")
        .and_then(Value::as_u64)
        .unwrap();
    let tolerance = block
        .get("tolerance_bytes")
        .and_then(Value::as_u64)
        .unwrap();
    // The same two constants `residency_peak_allocation.rs` declares from
    // `.plan/MEMORY_BUDGET.md` §4's formula, asserted here so the record and the
    // test cannot disagree about what the ceiling was.
    assert_eq!(ceiling, 2 * 1024 * 1024);
    assert_eq!(tolerance, 64 * 1024);
    for peak in &peaks {
        assert!(*peak <= ceiling, "exact heap peak {peak} exceeds {ceiling}");
    }
    let spread = peaks.iter().max().unwrap() - peaks.iter().min().unwrap();
    assert_eq!(
        spread,
        block.get("spread_bytes").and_then(Value::as_u64).unwrap(),
        "the recorded spread must be the spread of the recorded figures"
    );
    assert!(
        spread <= tolerance,
        "exact heap moved by {spread} B across the recorded span, tolerance {tolerance} B"
    );
    // And the sensitivity check, without which flatness could be a fixed floor.
    let sensitivity = block
        .get("sensitivity_check")
        .and_then(Value::as_str)
        .expect("a flatness figure needs a sensitivity check beside it");
    assert!(sensitivity.contains("512x512"), "{sensitivity}");
}

/// `V1-05` — peak residency is flat in checkpoint size. Stated over the sizes
/// that actually exist on this machine, and the span is recorded rather than
/// implied.
#[test]
fn peak_resident_is_flat_across_the_recorded_span_of_checkpoint_sizes() {
    let flat: Vec<Value> = rows()
        .into_iter()
        .filter(|r| r.get("flatness_series").and_then(Value::as_bool) == Some(true))
        .collect();
    assert!(
        flat.len() >= 3,
        "V1-05 needs at least three sizes; the record has {}",
        flat.len()
    );
    let ceilings: Vec<u64> = flat
        .iter()
        .map(|r| u64_of(r, "resident_ceiling_bytes"))
        .collect();
    assert!(
        ceilings.windows(2).all(|w| w[0] == w[1]),
        "flatness means the SAME ceiling holds across sizes; ceilings were {ceilings:?}"
    );
    let sizes: Vec<u64> = flat.iter().map(|r| u64_of(r, "checkpoint_bytes")).collect();
    let peaks: Vec<u64> = flat
        .iter()
        .map(|r| u64_of(r, "peak_resident_set_size_bytes"))
        .collect();
    let smallest = *sizes.iter().min().unwrap();
    let largest = *sizes.iter().max().unwrap();
    assert!(
        largest / smallest >= 1000,
        "the span must be worth calling flat; it is {}x",
        largest / smallest
    );
    // Every row under one tolerance, and the peaks within a factor of two of each
    // other. Two rather than a tight epsilon because RSS includes the binary's
    // own text and stacks, which dominate at these magnitudes — the *exact* heap
    // statement is `residency_peak_allocation.rs`, and this row corroborates it
    // rather than carrying it.
    let tolerance = u64_of(&flat[0], "resident_tolerance_bytes");
    for (row, peak) in flat.iter().zip(&peaks) {
        assert!(
            *peak <= tolerance,
            "row {}: peak {peak} exceeds the shared tolerance {tolerance}",
            str_of(row, "label")
        );
    }
    let min_peak = *peaks.iter().min().unwrap();
    let max_peak = *peaks.iter().max().unwrap();
    assert!(
        max_peak <= min_peak * 2,
        "peak RSS moved from {min_peak} to {max_peak} across a {}x span of checkpoint size; \
         that is not flat",
        largest / smallest
    );
    // Recorded, so a reader does not have to recompute it.
    let span = record()
        .get("flatness_span")
        .and_then(Value::as_f64)
        .expect("the record must state the span flatness was demonstrated across");
    assert!((span - (largest as f64 / smallest as f64)).abs() < 1.0);
}

#[test]
fn the_record_states_plainly_what_this_measurement_does_not_establish() {
    let rec = record();
    let limits = rec
        .get("coverage_not_established")
        .and_then(Value::as_array)
        .expect("the record must list the coverage it does not provide");
    let joined = limits
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" | ")
        .to_ascii_lowercase();
    for needle in [
        "24 gb",
        "shard",
        "throughput",
        "gpu",
        "mmap",
        "page",
        "rank",
    ] {
        assert!(
            joined.contains(needle),
            "the claim limits must mention `{needle}`; they are {joined:?}"
        );
    }
}

#[test]
fn the_recorded_verdict_names_each_conjunct_of_g1_and_its_outcome() {
    let verdict = record()
        .get("g1_verdict")
        .cloned()
        .expect("the record must carry an explicit G1 verdict");
    assert_eq!(
        verdict.get("passes").and_then(Value::as_bool),
        Some(true),
        "if G1 does not pass, this test must be updated to expect `false` together with the \
         evidence — never left asserting a pass that is not there"
    );
    let conjuncts = verdict
        .get("conjuncts")
        .and_then(Value::as_array)
        .expect("the verdict must break down into the plan's conjuncts");
    let names: Vec<String> = conjuncts
        .iter()
        .map(|c| {
            c.get("requirement")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    for required in ["C <= 2 GiB", "N >= 100", "peak <= 1.25 x C"] {
        assert!(
            names.iter().any(|n| n == required),
            "the verdict must state `{required}`; it states {names:?}"
        );
    }
    for c in conjuncts {
        assert_eq!(
            c.get("met").and_then(Value::as_bool),
            Some(true),
            "conjunct {c:?} is not met, so `passes: true` above is wrong"
        );
    }
    // And the one thing a reader must not mistake: which conjunct is true by
    // construction rather than by measurement.
    let by_construction = verdict
        .get("true_by_construction")
        .and_then(Value::as_array)
        .expect("the verdict must say which conjuncts are true by construction");
    assert!(
        by_construction
            .iter()
            .filter_map(Value::as_str)
            .any(|s| s.contains("N >= 100")),
        "N >= 100 at C = checkpoint/100 is arithmetic, not a measurement, and the record \
         must say so"
    );
}

// ===========================================================================
// The pass itself, over checked-in fixtures. Deterministic, no checkpoint.
// ===========================================================================

#[test]
fn a_full_pass_over_the_two_shard_fixture_reconciles_every_described_byte() {
    let (outcome, described) = pass_over(
        &fixture("tiny-llama-2shard"),
        BlockStreamConfig::default(),
        plain,
    );
    // fixtures/tiny-llama-2shard: 111 tensors, 1 196 736 payload bytes
    // (scripts/baseline.json `cli_golden.inspect_payload`). 25 are rank-1 norms
    // of 48 f32 each = 4 800 bytes, refused by ADR-010's rank rule.
    assert_eq!(described, 1_196_736);
    assert_eq!(outcome.tensors_total, 111);
    assert_eq!(outcome.tensors_refused, 25);
    assert_eq!(outcome.refused_payload_bytes, 25 * 48 * 4);
    assert_eq!(outcome.bytes_streamed, 1_196_736 - 4_800);
    assert!(outcome.is_complete());
    assert!(
        outcome.reconciles_against(described),
        "streamed {} + refused {} != described {described}",
        outcome.bytes_streamed,
        outcome.refused_payload_bytes
    );
}

#[test]
fn two_passes_over_the_same_fixture_produce_the_identical_checksum() {
    let dir = fixture("tiny-llama-2shard");
    let (first, _) = pass_over(&dir, BlockStreamConfig::default(), plain);
    let (second, _) = pass_over(&dir, BlockStreamConfig::default(), plain);
    assert_eq!(first.checksum, second.checksum);
    assert_eq!(first.bytes_streamed, second.bytes_streamed);
    assert_ne!(first.checksum, 0, "a zero checksum would prove nothing");
}

/// Stronger than "two runs agree", which any deterministic function satisfies:
/// the fold does not depend on how the tensors were cut up, so the same bytes
/// read through different grids must produce the same total.
#[test]
fn the_checksum_is_identical_however_the_tensors_are_cut_into_blocks() {
    let dir = fixture("tiny-llama-2shard");
    let mut checksums = Vec::new();
    let mut blocks = Vec::new();
    for edge in [256u64, 128, 64, 32] {
        let (outcome, described) = pass_over(
            &dir,
            BlockStreamConfig::default().with_block(edge, edge),
            plain,
        );
        assert!(outcome.reconciles_against(described));
        checksums.push(outcome.checksum);
        blocks.push(outcome.blocks_planned);
    }
    assert!(
        checksums.windows(2).all(|w| w[0] == w[1]),
        "the checksum moved with the block size: {checksums:?}"
    );
    assert!(
        blocks.windows(2).any(|w| w[0] != w[1]),
        "every block size produced the same grid, so nothing was varied: {blocks:?}"
    );
}

#[test]
fn both_read_modes_produce_the_identical_checksum_over_the_same_fixture() {
    // The read mode is a *measurement* choice. If it changed the bytes it would
    // be a correctness change wearing a performance flag's clothes.
    let dir = fixture("tiny-llama-2shard");
    let ingested = q_safetensors::ingest_local(&dir).unwrap();
    let mut results = Vec::new();
    for mapped in [true, false] {
        let source: Box<dyn ModelSource> = if mapped {
            Box::new(LocalFsSource::open(&dir).unwrap())
        } else {
            Box::new(LocalFsSource::open_without_mapping(&dir).unwrap())
        };
        let outcome = residency::run(
            &*source,
            &ingested.descriptors,
            &plain(BlockStreamConfig::default()),
        )
        .unwrap();
        results.push((outcome.checksum, outcome.bytes_streamed));
    }
    assert_eq!(
        results[0], results[1],
        "mmap and pread disagree on the bytes"
    );
}

/// The admission check, at the CLI's own default configuration. This is the
/// mechanism that makes `G1` a gate: it refuses **before any read**.
#[test]
fn a_ceiling_below_the_passes_own_buffers_is_refused_naming_max_resident_before_any_read() {
    let dir = fixture("tiny-llama-2shard");
    let ingested = q_safetensors::ingest_local(&dir).unwrap();
    let source = LocalFsSource::open_without_mapping(&dir).unwrap();
    let base = BlockStreamConfig::default();
    let needed = base.accounted_resident_bytes();
    // At defaults: (min(64,4) + 2) x 256 x 256 x 4 + 256 x 8 = 1 574 912 bytes.
    assert_eq!(needed, 6 * 256 * 256 * 4 + 256 * 8);

    let err = residency::run(
        &source,
        &ingested.descriptors,
        &plain(base.with_max_resident_bytes(needed - 1)),
    )
    .unwrap_err();
    match &err {
        QError::BudgetExceeded {
            budget_name,
            requested,
            limit,
        } => {
            assert_eq!(*budget_name, MemoryBudget::resident().name);
            assert_eq!(*budget_name, "max_resident");
            assert_eq!((*requested, *limit), (needed, needed - 1));
        }
        other => panic!("expected BudgetExceeded naming max_resident, got {other:?}"),
    }
    // One byte more is admitted, so this is a boundary and not a blanket refusal.
    assert!(residency::run(
        &source,
        &ingested.descriptors,
        &plain(base.with_max_resident_bytes(needed))
    )
    .is_ok());
}

/// `.plan/MEMORY_BUDGET.md` §11's chain has to reach the *streaming* path, not
/// merely exist. A precedence chain nothing consults is decoration.
#[test]
fn the_configured_ceiling_reaches_the_streaming_configuration_through_the_precedence_chain() {
    let dir = tempfile::tempdir().unwrap();
    let config_file = dir.path().join("quatricmorph.toml");
    std::fs::write(
        &config_file,
        "[budgets]\nmax_resident_bytes = \"3MiB\"\nblock_rows = 64\nblock_columns = 64\n",
    )
    .unwrap();

    // Config file only: all three values arrive from the file.
    let budgets =
        StreamingBudgets::resolve(&BudgetFlags::default(), &EmptyEnv, Some(&config_file)).unwrap();
    let config = BlockStreamConfig::from_budgets(&budgets);
    assert_eq!(config.max_resident_bytes, 3 * 1024 * 1024);
    assert_eq!((config.block_rows, config.block_columns), (64, 64));
    assert_eq!(budgets.max_resident_bytes.origin, BudgetOrigin::ConfigFile);

    // A flag overrides the file, and the streaming config follows.
    let budgets = StreamingBudgets::resolve(
        &BudgetFlags {
            max_resident_bytes: Some(G1_CEILING_BYTES),
            ..Default::default()
        },
        &EmptyEnv,
        Some(&config_file),
    )
    .unwrap();
    let config = BlockStreamConfig::from_budgets(&budgets);
    assert_eq!(config.max_resident_bytes, G1_CEILING_BYTES);
    assert_eq!(budgets.max_resident_bytes.origin, BudgetOrigin::CliFlag);
    // Block size still came from the file — the links are per variable, not
    // all-or-nothing.
    assert_eq!(config.block_rows, 64);
    assert_eq!(budgets.block_rows.origin, BudgetOrigin::ConfigFile);

    // And with nothing set at all, the compiled 2 GiB default is what a pass runs
    // against. That is the ceiling that exists without any operator action.
    let defaults = BlockStreamConfig::from_budgets(&StreamingBudgets::compiled_defaults());
    assert_eq!(defaults.max_resident_bytes, MAX_RESIDENT_BYTES);
    assert_eq!(defaults.max_resident_bytes, 2 * 1024 * 1024 * 1024);
}

#[test]
fn the_fixtures_rank_one_norms_are_refused_rather_than_flattened_into_a_matrix() {
    let (outcome, _) = pass_over(
        &fixture("tiny-llama-2shard"),
        BlockStreamConfig::default(),
        plain,
    );
    assert_eq!(outcome.tensors_refused, 25);
    for refusal in &outcome.refusals {
        assert_eq!(refusal.shape.len(), 1, "{refusal:?}");
        assert!(
            refusal.reason.contains("rank 1"),
            "a rank-1 refusal must say so: {}",
            refusal.reason
        );
        assert!(
            refusal.reason.contains("2-D extent"),
            "and say what it needed instead: {}",
            refusal.reason
        );
        assert_eq!(refusal.payload_bytes, 48 * 4);
    }
}

#[test]
fn cancellation_and_resume_rejoin_into_exactly_one_uninterrupted_pass_over_a_real_fixture() {
    let dir = fixture("tiny-llama-2shard");
    let config = BlockStreamConfig::default();
    let (whole, described) = pass_over(&dir, config, plain);
    assert!(whole.reconciles_against(described));

    let ingested = q_safetensors::ingest_local(&dir).unwrap();
    let source = LocalFsSource::open_without_mapping(&dir).unwrap();
    for stop in [1u64, 7, 40, 85] {
        let first = residency::run(
            &source,
            &ingested.descriptors,
            &ResidencyRequest {
                config,
                stop_after_blocks: Some(stop),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(first.blocks_streamed, stop);
        assert!(
            first.stopped_at.is_some(),
            "stop at {stop} recorded nothing"
        );

        let second = residency::run(
            &source,
            &ingested.descriptors,
            &ResidencyRequest {
                config,
                resume_from_block: Some(first.next_block_index),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            first.bytes_streamed + second.bytes_streamed,
            whole.bytes_streamed,
            "stop at {stop}: bytes do not rejoin"
        );
        assert_eq!(
            first.checksum.wrapping_add(second.checksum),
            whole.checksum,
            "stop at {stop}: the resumed pass did not read exactly what the interrupted one \
             missed"
        );
    }
}

#[test]
fn a_checkpoint_directory_that_is_not_there_is_refused_naming_it() {
    let missing = repo_root().join("fixtures/there-is-no-such-checkpoint");
    let err = q_safetensors::ingest_local(&missing).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("there-is-no-such-checkpoint"),
        "the refusal must name the directory; it said {msg}"
    );
}

#[test]
fn a_directory_with_no_shard_is_refused_rather_than_reporting_a_perfect_empty_pass() {
    // `fixtures/tiny-qwen-single` holds a config and a golden vector but no
    // `.safetensors`. A pass over it would have flawless residency and mean
    // nothing, so ingestion refuses first.
    let err = q_safetensors::ingest_local(fixture("tiny-qwen-single")).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("no .safetensors shards found"),
        "message was {msg}"
    );
}

#[test]
fn a_truncated_shard_stops_the_pass_naming_the_overrunning_byte_range() {
    let dir = tempfile::tempdir().unwrap();
    let source_dir = fixture("tiny-llama-single");
    // Copy every artifact, then truncate the shard. The fixture itself is never
    // modified.
    for entry in std::fs::read_dir(&source_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let bytes = std::fs::read(entry.path()).unwrap();
        let out = if name.to_string_lossy().ends_with(".safetensors") {
            bytes[..bytes.len() / 2].to_vec()
        } else {
            bytes
        };
        std::fs::write(dir.path().join(name), out).unwrap();
    }
    let ingested = q_safetensors::ingest_local(dir.path());
    // Either ingestion refuses on the declared offsets, or the pass refuses on
    // the read. Both are correct; what must never happen is a zero-filled block.
    let err = match ingested {
        Err(e) => e,
        Ok(ingested) => {
            let source = LocalFsSource::open_without_mapping(dir.path()).unwrap();
            residency::run(
                &source,
                &ingested.descriptors,
                &plain(BlockStreamConfig::default()),
            )
            .expect_err("a truncated shard must not stream")
        }
    };
    assert!(
        matches!(
            err,
            QError::RangeOutOfBounds { .. } | QError::MalformedArtifact { .. }
        ),
        "expected a range or malformed refusal, got {err:?}"
    );
}

// ===========================================================================
// The real checkpoint and the release binary. Skipped loudly when absent.
// ===========================================================================

#[test]
fn the_real_checkpoint_pass_reconciles_and_reproduces_the_recorded_checksum() {
    let Some(dir) = checkpoint_dir() else { return };
    let row = row_named("g1-headline");
    let (outcome, described) = pass_over(
        &dir,
        BlockStreamConfig::default().with_max_resident_bytes(G1_CEILING_BYTES),
        plain,
    );
    assert_eq!(described, u64_of(&row, "described_payload_bytes"));
    assert_eq!(outcome.bytes_streamed, u64_of(&row, "bytes_streamed"));
    assert_eq!(
        outcome.refused_payload_bytes,
        u64_of(&row, "refused_payload_bytes")
    );
    assert_eq!(outcome.blocks_planned, u64_of(&row, "blocks"));
    assert!(
        outcome.reconciles_against(described),
        "streamed {} + refused {} != described {described}",
        outcome.bytes_streamed,
        outcome.refused_payload_bytes
    );
    assert_eq!(
        format!("{:#018x}", outcome.checksum),
        str_of(&row, "checksum"),
        "the recorded checksum no longer reproduces, so either the record or the pass changed"
    );
    // 82 tensors, 56 of them not rank-2 (50 rank-1, 6 rank-4) —
    // fixtures/real-checkpoint-record.json's rank histogram.
    assert_eq!(outcome.tensors_total, 82);
    assert_eq!(outcome.tensors_refused, 56);
    assert_eq!(
        outcome
            .refusals
            .iter()
            .filter(|r| r.shape.len() == 4)
            .count(),
        6,
        "the six rank-4 causal masks must be refused, never flattened"
    );
    for mask in outcome.refusals.iter().filter(|r| r.shape.len() == 4) {
        assert_eq!(mask.shape, vec![1, 1, 1024, 1024]);
        assert_eq!(mask.requirement_id.as_deref(), Some("GRID-007"));
        assert!(
            mask.reason.contains("refused rather than flattened"),
            "reason was {}",
            mask.reason
        );
    }
}

/// **The G1 measurement, re-taken by the test rather than trusted from the
/// record.** Runs the release binary under `/usr/bin/time -l` and compares the
/// reported maximum resident set size against `1.25 × C`.
#[test]
fn the_release_binary_holds_the_configured_ceiling_over_the_real_checkpoint() {
    let Some(dir) = checkpoint_dir() else { return };
    let Some(binary) = release_binary() else {
        return;
    };
    let tolerance = resident_tolerance_bytes(G1_CEILING_BYTES);
    assert_eq!(tolerance, 4_410_305);
    let peak = peak_rss_of_stream(&binary, &dir, G1_CEILING_BYTES, "pread");
    assert!(
        peak <= tolerance,
        "GATE G1 FAILED: peak RSS {peak} B exceeds 1.25 x C = {tolerance} B at the \
         configured ceiling C = {G1_CEILING_BYTES} B. .plan/EXECUTION_ORDER.md §7 says to \
         halt the engine lane and bisect per stage — do not raise C to make this pass"
    );
    // Sanity: a measurement far below the process's own footprint would mean
    // `time` reported something other than this program.
    assert!(
        peak > 1_000_000,
        "peak RSS {peak} B is implausibly small for a release binary; the measurement is \
         probably not of the streaming process"
    );
}

/// The gate seen to fail, re-taken rather than trusted. Same binary, same
/// checkpoint, same ceiling — only `--io mmap` differs, and the measurement
/// breaches by roughly 75×.
#[test]
fn the_mapped_read_mode_breaches_the_same_ceiling_so_the_gate_is_seen_to_fail() {
    let Some(dir) = checkpoint_dir() else { return };
    let Some(binary) = release_binary() else {
        return;
    };
    let tolerance = resident_tolerance_bytes(G1_CEILING_BYTES);
    let peak = peak_rss_of_stream(&binary, &dir, G1_CEILING_BYTES, "mmap");
    assert!(
        peak > tolerance,
        "the mapped pass peaked at {peak} B, within 1.25 x C = {tolerance} B. That would be \
         good news, but it would also mean this test no longer demonstrates that the \
         comparison can fail — and a gate never seen to fail is not a gate. Investigate \
         before deleting: the likely cause is that `time` measured the wrong process"
    );
    // The mapped pass pulls the checkpoint's pages into RSS, so the breach should
    // be on the order of the file rather than a marginal overshoot.
    assert!(
        peak > CHECKPOINT_BYTES / 2,
        "the mapped breach is {peak} B, far less than the {CHECKPOINT_BYTES} B checkpoint; \
         mapped-page accounting is not what is being demonstrated"
    );
}
