//! Differential test for the paired block reduction against an independent
//! NumPy reference.
//!
//! `.plan/TEST_STRATEGY.md` §0 rule 2: *"Expected values are computed by hand or
//! by an independent implementation, not by the code under test."*
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` §9 names the golden level: *"a committed
//! Python/NumPy script under `python/`, run in CI-equivalent form."*
//! `.plan/EXECUTION_ORDER.md` §7 makes it gate **G2**: *"the engine is the
//! product; a wrong number is worse than no number."*
//!
//! Every expected value here comes from
//! `python/reference/paired_reduction_reference.py`, which was written from
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.1's contract block and run **before**
//! any Rust implementation of it was read. Regenerate with:
//!
//! ```text
//! python3 python/reference/paired_reduction_reference.py \
//!     --emit-goldens crates/q-gpu/tests/goldens/
//! ```
//!
//! and the file must come back byte-identical. Provenance — interpreter
//! version, NumPy version, and the golden's SHA-256 — is in
//! `.plan/evidence/QM-0121.md`.
//!
//! ## Scope: the specified contract, and nothing else
//!
//! This file drives **only** the surface
//! `.plan/tasks/QM-0121-paired-block-reduction/TASK.md` §Data Contracts
//! specifies — `Backend::paired_block_reduction`, `ChannelAxis::{Rows, Columns}`
//! and the six partial fields of `PairedPartials` and `ChannelPartials`. It uses
//! no convenience method beyond that, so it verifies **any** implementation of
//! the contract rather than one particular design of it.
//!
//! ## Why bit equality and not a tolerance
//!
//! Both implementations widen `f32` to `f64` (exact), subtract, square and take
//! absolute values with single correctly-rounded IEEE-754 binary64 operations,
//! and accumulate **sequentially in flat row-major index order** — the order
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` §4.2 fixes so that `V1-13`'s
//! byte-identical output is achievable at all. Identical operations in an
//! identical order produce identical bits, so a tolerance here would only hide a
//! real disagreement. Every comparison below is on `to_bits()`.
//!
//! Where a tolerance *would* be needed — composing two half-block reductions,
//! which regroups the sequential sum — the golden records the **measured** ULP
//! distance and this test asserts that exact number rather than a guessed bound.
//!
//! The golden is embedded with `include_str!`, so this test performs no file I/O
//! and touches no network.

use q_gpu::BlockData;
use q_gpu::{Backend, ChannelAxis, ChannelPartials, CpuBackend, PairedPartials};
use serde_json::Value;

const GOLDENS: &str = include_str!("goldens/paired-reduction-goldens.json");

fn goldens() -> Value {
    serde_json::from_str(GOLDENS).expect("the golden file is valid JSON")
}

/// The normative field for an input is its f32 bit pattern, so no decimal round
/// trip is involved in any comparison here.
fn f32_of(v: &Value) -> f32 {
    let s = v.as_str().expect("bit patterns are strings");
    let hex = s.strip_prefix("0x").expect("bit patterns are 0x-prefixed");
    f32::from_bits(u32::from_str_radix(hex, 16).expect("bit patterns are hex"))
}

/// The normative field for a partial is its f64 bit pattern.
fn f64_of(v: &Value) -> f64 {
    let s = v.as_str().expect("bit patterns are strings");
    let hex = s.strip_prefix("0x").expect("bit patterns are 0x-prefixed");
    f64::from_bits(u64::from_str_radix(hex, 16).expect("bit patterns are hex"))
}

fn f32_values(entry: &Value, key: &str) -> Vec<f32> {
    entry[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} is an array"))
        .iter()
        .map(f32_of)
        .collect()
}

fn block(entry: &Value, key: &str, rows: usize, columns: usize) -> BlockData {
    BlockData::new(rows, columns, f32_values(entry, key))
        .expect("the golden declares a valid block")
}

fn axis_of(name: &str) -> ChannelAxis {
    match name {
        "rows" => ChannelAxis::Rows,
        "columns" => ChannelAxis::Columns,
        other => panic!("the reference emitted an unknown axis {other:?}"),
    }
}

fn usize_of(entry: &Value, key: &str) -> usize {
    entry[key]
        .as_u64()
        .unwrap_or_else(|| panic!("{key} is an integer")) as usize
}

/// The six partial fields, flattened so the whole-block struct and the
/// per-channel struct compare through one routine. `TASK.md` §Data Contracts
/// declares exactly these, in this order, on both types.
type Six = (u64, f64, f64, f64, f64, f64);

fn six_of_whole(p: &PairedPartials) -> Six {
    (
        p.count,
        p.sum_sq_base,
        p.sum_sq_delta,
        p.sum_abs_delta,
        p.max_abs_delta,
        p.max_abs_base,
    )
}

fn six_of_channel(c: &ChannelPartials) -> Six {
    (
        c.count,
        c.sum_sq_base,
        c.sum_sq_delta,
        c.sum_abs_delta,
        c.max_abs_delta,
        c.max_abs_base,
    )
}

fn six_of_golden(v: &Value) -> Six {
    (
        v["count"].as_u64().expect("count"),
        f64_of(&v["sum_sq_base"]),
        f64_of(&v["sum_sq_delta"]),
        f64_of(&v["sum_abs_delta"]),
        f64_of(&v["max_abs_delta"]),
        f64_of(&v["max_abs_base"]),
    )
}

const FIELD_NAMES: [&str; 5] = [
    "sum_sq_base",
    "sum_sq_delta",
    "sum_abs_delta",
    "max_abs_delta",
    "max_abs_base",
];

/// Bit-for-bit comparison of all six fields, reporting the first divergence with
/// both patterns, both decimal renderings, and the exact ULP distance — never a
/// rounded-off "close enough".
#[track_caller]
fn assert_six(what: &str, expected: Six, actual: Six) {
    assert_eq!(expected.0, actual.0, "{what}: count");
    let e = [expected.1, expected.2, expected.3, expected.4, expected.5];
    let a = [actual.1, actual.2, actual.3, actual.4, actual.5];
    for (i, name) in FIELD_NAMES.iter().enumerate() {
        if e[i].to_bits() != a[i].to_bits() {
            let ulps = (e[i].to_bits() as i128 - a[i].to_bits() as i128).abs();
            panic!(
                "{what}.{name} disagrees with the NumPy reference.\n  \
                 reference 0x{:016x} ({:e})\n  \
                 q-gpu     0x{:016x} ({:e})\n  \
                 distance  {ulps} ULP (bit-pattern difference)",
                e[i].to_bits(),
                e[i],
                a[i].to_bits(),
                a[i]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The differential test itself
// ---------------------------------------------------------------------------

/// Acceptance criterion 1 and `TASK.md` §Test Cases rows 1–5, over every case in
/// the golden set, at both channel axes, on every field of the whole block and
/// of every channel.
#[test]
fn every_golden_case_matches_the_numpy_reference_bit_for_bit() {
    let doc = goldens();
    assert_eq!(doc["schema"], "quatricmorph/paired-reduction-goldens/v1");
    assert_eq!(doc["requirement"], "QUANT-002");

    let cases = doc["cases"].as_array().expect("cases");
    assert!(!cases.is_empty(), "the golden set is empty");
    let mut channels_checked = 0usize;

    for entry in cases {
        let name = entry["name"].as_str().expect("name");
        let rows = usize_of(entry, "rows");
        let columns = usize_of(entry, "columns");
        let base = block(entry, "base_bits", rows, columns);
        let counterpart = block(entry, "counterpart_bits", rows, columns);

        for axis_name in ["rows", "columns"] {
            let axis = axis_of(axis_name);
            let expected = &entry["axes"][axis_name];
            let actual = CpuBackend
                .paired_block_reduction(&base, &counterpart, axis)
                .unwrap_or_else(|e| panic!("{name}/{axis_name} was refused: {e}"));

            assert_eq!(
                usize_of(expected, "channels"),
                actual.per_channel.len(),
                "{name}/{axis_name}: channel count"
            );
            assert_six(
                &format!("{name}/{axis_name}/whole_block"),
                six_of_golden(&expected["whole_block"]),
                six_of_whole(&actual),
            );
            let per = expected["per_channel"].as_array().expect("per_channel");
            assert_eq!(per.len(), actual.per_channel.len());
            for (c, (e, a)) in per.iter().zip(&actual.per_channel).enumerate() {
                assert_six(
                    &format!("{name}/{axis_name}/channel[{c}]"),
                    six_of_golden(e),
                    six_of_channel(a),
                );
                channels_checked += 1;
            }
        }
    }
    // A silent zero here would mean the loop never ran.
    assert!(
        channels_checked > 0,
        "no per-channel partials were compared at all"
    );
}

/// The `QM-0120` lesson, made mechanical.
///
/// `.plan/DIAGNOSTIC_ARCHITECTURE.md` §3.1, amended 2026-08-05: *"Agreement with
/// a reference proves the arithmetic matches on the values you chose. It does
/// not prove you chose values that can distinguish two candidate formulas. A
/// golden set needs inputs selected to **discriminate**, not merely to cover."*
///
/// The reference evaluates each plausible **wrong** formula on the same inputs
/// and records whether it lands somewhere else. This test asserts that
///
/// 1. every wrong formula is ruled out by at least one case, and
/// 2. on each case that claims to rule one out, the backend's own answer really
///    does differ from the wrong one.
///
/// Without (2) the coverage map would be a claim about the reference alone.
#[test]
fn every_wrong_formula_is_ruled_out_by_a_case_that_can_actually_tell_it_apart() {
    let doc = goldens();
    let coverage = doc["discrimination_coverage"]
        .as_object()
        .expect("discrimination_coverage");
    assert!(
        !coverage.is_empty(),
        "the golden set names no wrong formulas at all"
    );
    for (key, cases) in coverage {
        assert!(
            !cases.as_array().expect("case list").is_empty(),
            "no golden case can distinguish the correct formula from {key:?} — the \
             set covers without discriminating, which is the QM-0120 failure mode"
        );
    }

    // Which whole-block field each wrong formula would have replaced.
    fn field_of(key: &str) -> &'static str {
        match key {
            "abs_of_sum_delta" | "f32_sum_abs_delta" | "numpy_pairwise_sum_abs_delta" => {
                "sum_abs_delta"
            }
            "max_signed_delta" => "max_abs_delta",
            "max_signed_base" => "max_abs_base",
            "sum_sq_counterpart" | "f32_sum_sq_base" | "numpy_pairwise_sum_sq_base" => {
                "sum_sq_base"
            }
            "f32_sum_sq_delta" | "numpy_pairwise_sum_sq_delta" => "sum_sq_delta",
            other => panic!("unmapped discriminator {other:?}"),
        }
    }

    let mut proven = std::collections::BTreeSet::new();
    for entry in doc["cases"].as_array().expect("cases") {
        let name = entry["name"].as_str().expect("name");
        let rows = usize_of(entry, "rows");
        let columns = usize_of(entry, "columns");
        let base = block(entry, "base_bits", rows, columns);
        let counterpart = block(entry, "counterpart_bits", rows, columns);
        let got = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .expect("accepted");

        for (key, flag) in entry["discriminates"].as_object().expect("discriminates") {
            if !flag.as_bool().expect("boolean") || key.starts_with("transposed_per_channel") {
                // The transposition discriminators are structural rather than
                // scalar; `orientation_*` below owns them.
                continue;
            }
            let wrong = f64_of(&entry["discriminators"][key]);
            let ours = match field_of(key) {
                "sum_sq_base" => got.sum_sq_base,
                "sum_sq_delta" => got.sum_sq_delta,
                "sum_abs_delta" => got.sum_abs_delta,
                "max_abs_delta" => got.max_abs_delta,
                "max_abs_base" => got.max_abs_base,
                other => panic!("unmapped field {other}"),
            };
            assert_ne!(
                wrong.to_bits(),
                ours.to_bits(),
                "{name}: the backend's {} agrees with the WRONG formula {key:?} \
                 (0x{:016x}) — this case cannot tell them apart after all",
                field_of(key),
                wrong.to_bits()
            );
            proven.insert(key.clone());
        }
    }
    let named: std::collections::BTreeSet<String> = coverage
        .keys()
        .filter(|k| !k.starts_with("transposed_per_channel"))
        .cloned()
        .collect();
    assert_eq!(
        proven, named,
        "some wrong formula was never exercised against the backend"
    );
}

/// Acceptance criterion 1, written out by hand in this test rather than read
/// from the golden — so the golden and the hand computation corroborate each
/// other instead of the test trusting one source.
///
/// ```text
/// base                     counterpart              delta = base - counterpart
///  1     -2     0.5   4      1.5   -2     0     4.25    -0.5   0     0.5  -0.25
/// -8      0.25  3    -1.5   -8      0.75  3.5  -1.5      0    -0.5  -0.5   0
///  2     -0.75  6     0.125  2      0.25  6     0.625    0    -1     0    -0.5
///
/// count         = 12
/// sum_sq_base   = 1 + 4 + 0.25 + 16
///               + 64 + 0.0625 + 9 + 2.25
///               + 4 + 0.5625 + 36 + 0.015625                 = 137.140625
/// sum_sq_delta  = 0.25 + 0.25 + 0.0625 + 0.25 + 0.25 + 1 + 0.25
///                                                            =   2.3125
/// sum_abs_delta = 0.5 + 0.5 + 0.25 + 0.5 + 0.5 + 1 + 0.5      =   3.75
/// max_abs_delta = |-1| at (2,1)                               =   1
/// max_abs_base  = |-8| at (1,0)                               =   8
/// ```
///
/// Every value is a dyadic rational with a small exponent, so each square and
/// each partial sum is exact in binary64 and the equalities above are exact
/// rather than rounded.
#[test]
fn the_hand_computed_3x4_case_matches_arithmetic_written_out_in_this_test() {
    #[rustfmt::skip]
    let base = BlockData::new(3, 4, vec![
         1.0, -2.0,  0.5,  4.0,
        -8.0,  0.25, 3.0, -1.5,
         2.0, -0.75, 6.0,  0.125,
    ]).unwrap();
    #[rustfmt::skip]
    let counterpart = BlockData::new(3, 4, vec![
         1.5, -2.0,  0.0,  4.25,
        -8.0,  0.75, 3.5, -1.5,
         2.0,  0.25, 6.0,  0.625,
    ]).unwrap();

    let p = CpuBackend
        .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
        .unwrap();
    assert_six(
        "hand/rows/whole",
        (12, 137.140625, 2.3125, 3.75, 1.0, 8.0),
        six_of_whole(&p),
    );

    // The fixture was chosen so the wrong formulas land elsewhere:
    // Σ|d| = 3.75 is not |Σd| = 2.75; max|d| = 1 is not max d = 0.5;
    // max|w| = 8 is not max w = 6; Σ w² = 137.140625 is not Σ ŵ² = 143.828125.
    assert_ne!(p.sum_abs_delta.to_bits(), 2.75f64.to_bits());
    assert_ne!(p.max_abs_delta.to_bits(), 0.5f64.to_bits());
    assert_ne!(p.max_abs_base.to_bits(), 6.0f64.to_bits());
    assert_ne!(p.sum_sq_base.to_bits(), 143.828125f64.to_bits());

    // Per row: [1,-2,0.5,4], [-8,0.25,3,-1.5], [2,-0.75,6,0.125].
    let rows: Vec<Six> = p.per_channel.iter().map(six_of_channel).collect();
    assert_eq!(
        rows,
        vec![
            (4, 21.25, 0.5625, 1.25, 0.5, 4.0),
            (4, 75.3125, 0.5, 1.0, 0.5, 8.0),
            (4, 40.578125, 1.25, 1.5, 1.0, 6.0),
        ]
    );

    // Per column: [1,-8,2], [-2,0.25,-0.75], [0.5,3,6], [4,-1.5,0.125].
    let q = CpuBackend
        .paired_block_reduction(&base, &counterpart, ChannelAxis::Columns)
        .unwrap();
    let columns: Vec<Six> = q.per_channel.iter().map(six_of_channel).collect();
    assert_eq!(
        columns,
        vec![
            (3, 69.0, 0.25, 0.5, 0.5, 8.0),
            (3, 4.625, 1.25, 1.5, 1.0, 2.0),
            (3, 45.25, 0.5, 1.0, 0.5, 6.0),
            (3, 18.265625, 0.3125, 0.75, 0.5, 4.0),
        ]
    );

    // The whole-block fields do not depend on the axis: the axis chooses how the
    // channels are cut, not what is summed.
    assert_six("hand/columns/whole", six_of_whole(&p), six_of_whole(&q));
}

/// `TASK.md` §Test Cases rows 2 and 3, asserted as relationships rather than as
/// literals, so they hold for the reasons stated rather than by coincidence.
#[test]
fn identical_blocks_have_zero_delta_and_a_zero_counterpart_makes_delta_equal_base() {
    #[rustfmt::skip]
    let base = BlockData::new(3, 4, vec![
         1.0, -2.0,  0.5,  4.0,
        -8.0,  0.25, 3.0, -1.5,
         2.0, -0.75, 6.0,  0.125,
    ]).unwrap();
    let zeros = BlockData::new(3, 4, vec![0.0; 12]).unwrap();

    // Row 2: identical base and counterpart — every delta field exactly zero,
    // sum_sq_base unchanged.
    let same = CpuBackend
        .paired_block_reduction(&base, &base, ChannelAxis::Rows)
        .unwrap();
    assert_eq!(same.sum_sq_delta.to_bits(), 0.0f64.to_bits());
    assert_eq!(same.sum_abs_delta.to_bits(), 0.0f64.to_bits());
    assert_eq!(same.max_abs_delta.to_bits(), 0.0f64.to_bits());
    assert_eq!(same.sum_sq_base.to_bits(), 137.140625f64.to_bits());
    for c in &same.per_channel {
        assert_eq!(c.sum_sq_delta.to_bits(), 0.0f64.to_bits());
        assert_eq!(c.sum_abs_delta.to_bits(), 0.0f64.to_bits());
        assert_eq!(c.max_abs_delta.to_bits(), 0.0f64.to_bits());
    }

    // Row 3: counterpart all zeros — the delta IS the base, so sum_sq_delta must
    // equal sum_sq_base bit for bit (identical terms, identical order), and
    // max_abs_delta must equal max_abs_base.
    let zeroed = CpuBackend
        .paired_block_reduction(&base, &zeros, ChannelAxis::Rows)
        .unwrap();
    assert_eq!(
        zeroed.sum_sq_delta.to_bits(),
        zeroed.sum_sq_base.to_bits(),
        "with a zero counterpart the delta is the base itself"
    );
    assert_eq!(
        zeroed.max_abs_delta.to_bits(),
        zeroed.max_abs_base.to_bits()
    );
    for c in &zeroed.per_channel {
        assert_eq!(c.sum_sq_delta.to_bits(), c.sum_sq_base.to_bits());
        assert_eq!(c.max_abs_delta.to_bits(), c.max_abs_base.to_bits());
    }
}

/// Acceptance criterion 2 — orientation, in the form that can actually fail.
///
/// On a **non-square** block a transposed implementation is caught by the
/// channel count alone. On a **square** one the count matches, the answer is
/// plausibly shaped, and only the per-channel values can catch it. Both are
/// asserted, and the square fixture is deliberately not symmetric under
/// transposition — a symmetric one would prove nothing.
#[test]
fn orientation_reducing_over_the_wrong_axis_gives_a_different_answer() {
    let doc = goldens();
    let cases = doc["cases"].as_array().expect("cases");

    for name in ["asymmetric_2x5", "square_asymmetric_3x3"] {
        let entry = cases
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("golden case {name} is missing"));
        let rows = usize_of(entry, "rows");
        let columns = usize_of(entry, "columns");
        let base = block(entry, "base_bits", rows, columns);
        let counterpart = block(entry, "counterpart_bits", rows, columns);

        let by_rows = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .unwrap();
        let by_columns = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Columns)
            .unwrap();

        assert_eq!(
            by_rows.per_channel.len(),
            rows,
            "{name}: Rows must give one channel per row"
        );
        assert_eq!(
            by_columns.per_channel.len(),
            columns,
            "{name}: Columns must give one channel per column"
        );

        let r: Vec<Six> = by_rows.per_channel.iter().map(six_of_channel).collect();
        let c: Vec<Six> = by_columns.per_channel.iter().map(six_of_channel).collect();
        assert_ne!(
            r, c,
            "{name}: the two axes produced the SAME per-channel partials, so this \
             fixture cannot prove the axis was not silently transposed"
        );
    }

    // The square case is the load-bearing one: equal channel counts, so a
    // transposed implementation still type-checks and still returns 3 channels.
    let square = cases
        .iter()
        .find(|c| c["name"] == "square_asymmetric_3x3")
        .expect("square case");
    assert_eq!(usize_of(square, "rows"), usize_of(square, "columns"));
    assert!(
        square["discriminates"]["transposed_per_channel_at_equal_channel_count"]
            .as_bool()
            .expect("flag"),
        "the square fixture is symmetric under transposition and proves nothing"
    );
}

/// Acceptance criterion 3 — partials compose.
///
/// Split at a row boundary; sum the additive fields, take the maximum of the
/// `max_abs_*` fields. Both halves are also compared against the **reference's**
/// own halves, so this is a differential composition check rather than the code
/// agreeing with itself.
///
/// Composition is bit-exact only where every partial sum is exact: regrouping a
/// sequential sum at the split point is a different association. The golden
/// records which fixtures compose exactly and, where they do not, the measured
/// ULP distance — asserted here as an exact number, not a guessed bound.
#[test]
fn partials_compose_when_a_block_is_reduced_in_two_halves() {
    let doc = goldens();
    let splits = doc["splits"].as_array().expect("splits");
    assert!(!splits.is_empty());

    for entry in splits {
        let name = entry["name"].as_str().expect("name");
        let rows = usize_of(entry, "rows");
        let columns = usize_of(entry, "columns");
        let split = usize_of(entry, "split_row");
        let base = block(entry, "base_bits", rows, columns);
        let counterpart = block(entry, "counterpart_bits", rows, columns);

        let cut = |b: &BlockData, from: usize, to: usize| {
            BlockData::new(
                to - from,
                columns,
                b.values[from * columns..to * columns].to_vec(),
            )
            .unwrap()
        };
        let (bt, ct) = (cut(&base, 0, split), cut(&counterpart, 0, split));
        let (bb, cb) = (cut(&base, split, rows), cut(&counterpart, split, rows));

        for axis_name in ["rows", "columns"] {
            let axis = axis_of(axis_name);
            let expected = &entry["axes"][axis_name];

            let whole = CpuBackend
                .paired_block_reduction(&base, &counterpart, axis)
                .unwrap();
            let top = CpuBackend.paired_block_reduction(&bt, &ct, axis).unwrap();
            let bottom = CpuBackend.paired_block_reduction(&bb, &cb, axis).unwrap();

            assert_six(
                &format!("{name}/{axis_name}/whole"),
                six_of_golden(&expected["whole_block"]),
                six_of_whole(&whole),
            );
            assert_six(
                &format!("{name}/{axis_name}/top"),
                six_of_golden(&expected["top_half"]),
                six_of_whole(&top),
            );
            assert_six(
                &format!("{name}/{axis_name}/bottom"),
                six_of_golden(&expected["bottom_half"]),
                six_of_whole(&bottom),
            );

            // Compose: the additive fields add, the max fields take a maximum.
            let composed: Six = (
                top.count + bottom.count,
                top.sum_sq_base + bottom.sum_sq_base,
                top.sum_sq_delta + bottom.sum_sq_delta,
                top.sum_abs_delta + bottom.sum_abs_delta,
                top.max_abs_delta.max(bottom.max_abs_delta),
                top.max_abs_base.max(bottom.max_abs_base),
            );

            // The composed value must equal the REFERENCE's composed value bit
            // for bit — the reference regroups the same way.
            assert_six(
                &format!("{name}/{axis_name}/composed"),
                six_of_golden(&expected["composed"]),
                composed,
            );

            // count and both maxima compose EXACTLY, always.
            assert_eq!(
                composed.0, whole.count,
                "{name}/{axis_name}: count composes"
            );
            assert_eq!(
                composed.4.to_bits(),
                whole.max_abs_delta.to_bits(),
                "{name}/{axis_name}: max_abs_delta composes by maximum, exactly"
            );
            assert_eq!(
                composed.5.to_bits(),
                whole.max_abs_base.to_bits(),
                "{name}/{axis_name}: max_abs_base composes by maximum, exactly"
            );

            // And the divergence of the three sums from the undivided reduction
            // is the measured one, exactly — not a guessed tolerance.
            let ulp = &expected["composition_ulp"];
            let distance =
                |a: f64, b: f64| (a.to_bits() as i128 - b.to_bits() as i128).unsigned_abs();
            for (field, got, want) in [
                ("sum_sq_base", composed.1, whole.sum_sq_base),
                ("sum_sq_delta", composed.2, whole.sum_sq_delta),
                ("sum_abs_delta", composed.3, whole.sum_abs_delta),
            ] {
                assert_eq!(
                    distance(got, want),
                    ulp[field].as_u64().expect("ulp") as u128,
                    "{name}/{axis_name}: {field} composition divergence is not the \
                     distance the reference measured"
                );
            }
            if expected["composition_is_bit_exact"]
                .as_bool()
                .expect("flag")
            {
                assert_eq!(composed.1.to_bits(), whole.sum_sq_base.to_bits());
                assert_eq!(composed.2.to_bits(), whole.sum_sq_delta.to_bits());
                assert_eq!(composed.3.to_bits(), whole.sum_abs_delta.to_bits());
            }

            // Per-channel composition, where a row split preserves the channels.
            if axis == ChannelAxis::Columns {
                let per = expected["composed_per_channel"]
                    .as_array()
                    .expect("composed_per_channel");
                assert_eq!(per.len(), top.per_channel.len());
                for (c, e) in per.iter().enumerate() {
                    let t = &top.per_channel[c];
                    let b = &bottom.per_channel[c];
                    let merged: Six = (
                        t.count + b.count,
                        t.sum_sq_base + b.sum_sq_base,
                        t.sum_sq_delta + b.sum_sq_delta,
                        t.sum_abs_delta + b.sum_abs_delta,
                        t.max_abs_delta.max(b.max_abs_delta),
                        t.max_abs_base.max(b.max_abs_base),
                    );
                    assert_six(
                        &format!("{name}/{axis_name}/composed.channel[{c}]"),
                        six_of_golden(e),
                        merged,
                    );
                }
            }
        }
    }
}

/// Acceptance criterion 4 — two runs are bit-identical.
///
/// `V1-13` requires byte-identical output across runs, which is why §4.2 fixes
/// the reduction order and forbids parallel accumulation.
#[test]
fn two_runs_over_the_same_blocks_are_bit_identical() {
    let doc = goldens();
    let mut compared = 0usize;
    for entry in doc["cases"].as_array().expect("cases") {
        let rows = usize_of(entry, "rows");
        let columns = usize_of(entry, "columns");
        let base = block(entry, "base_bits", rows, columns);
        let counterpart = block(entry, "counterpart_bits", rows, columns);
        for axis in [ChannelAxis::Rows, ChannelAxis::Columns] {
            let a = CpuBackend
                .paired_block_reduction(&base, &counterpart, axis)
                .unwrap();
            let b = CpuBackend
                .paired_block_reduction(&base, &counterpart, axis)
                .unwrap();
            // Compared through bit patterns, so two NaNs could not pass as equal
            // and +0.0 could not pass as -0.0.
            assert_eq!(six_of_whole(&a), six_of_whole(&b));
            let pa: Vec<Six> = a.per_channel.iter().map(six_of_channel).collect();
            let pb: Vec<Six> = b.per_channel.iter().map(six_of_channel).collect();
            assert_eq!(pa, pb);
            compared += 1;
        }
    }
    assert!(compared > 0);
}

/// Acceptance criterion 5 — every refusal in the golden set is refused, for the
/// reason the reference gives, carrying the content `TASK.md` §Error Handling
/// requires the message to name.
///
/// The required content is asserted as **numbers and role names**, not as the
/// reference's exact wording: §Error Handling requires shape mismatch to name
/// *"both shapes"* and a non-finite value to name *"the position"*, and any
/// phrasing carrying those is conformant.
#[test]
fn every_refusal_in_the_golden_set_is_refused_with_the_reason_named() {
    let doc = goldens();
    let refusals = doc["refusals"].as_array().expect("refusals");
    assert!(!refusals.is_empty());
    let mut seen_kinds = std::collections::BTreeSet::new();

    for entry in refusals {
        let name = entry["name"].as_str().expect("name");
        let kind = entry["kind"].as_str().expect("kind");
        // `ragged_value_count` needs a block whose value count disagrees with its
        // declared shape, which `BlockData::new` cannot build; it has its own
        // test below.
        if kind == "ragged_block" {
            continue;
        }
        seen_kinds.insert(kind.to_string());

        let br = usize_of(entry, "base_rows");
        let bc = usize_of(entry, "base_columns");
        let cr = usize_of(entry, "counterpart_rows");
        let cc = usize_of(entry, "counterpart_columns");
        let base = BlockData::new(br, bc, f32_values(entry, "base_bits")).unwrap();
        let counterpart = BlockData::new(cr, cc, f32_values(entry, "counterpart_bits")).unwrap();

        let err = CpuBackend
            .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
            .err()
            .unwrap_or_else(|| panic!("{name} must be refused, and was not"));
        let message = err.to_string();
        let detail = entry["detail"].as_str().expect("detail");

        match kind {
            "shape_mismatch" => {
                // "naming both shapes" — every one of the four extents.
                for n in [br, bc, cr, cc] {
                    assert!(
                        message.contains(&n.to_string()),
                        "{name}: the refusal must name both shapes ([{br}, {bc}] and \
                         [{cr}, {cc}])\n  reference: {detail}\n  q-gpu:     {message}"
                    );
                }
            }
            "empty_block" => {
                assert!(
                    message.to_lowercase().contains("empty"),
                    "{name}: the refusal must say the block is empty\n  \
                     reference: {detail}\n  q-gpu:     {message}"
                );
            }
            "non_finite" => {
                // "naming the position" — the row and the column, plus which of
                // the two operands held it.
                let role = detail.split_whitespace().next().expect("role");
                let row: usize = detail
                    .split("at row ")
                    .nth(1)
                    .and_then(|s| s.split(',').next())
                    .and_then(|s| s.trim().parse().ok())
                    .expect("the reference names a row");
                let column: usize = detail
                    .split("column ")
                    .nth(1)
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|s| s.trim_end_matches([',', ';']).parse().ok())
                    .expect("the reference names a column");
                assert!(
                    message.contains(role),
                    "{name}: the refusal must name which block held it ({role})\n  \
                     reference: {detail}\n  q-gpu:     {message}"
                );
                assert!(
                    message.contains(&format!("row {row}"))
                        && message.contains(&format!("column {column}")),
                    "{name}: the refusal must name the position (row {row}, column \
                     {column})\n  reference: {detail}\n  q-gpu:     {message}"
                );
            }
            other => panic!("unmapped refusal kind {other:?}"),
        }
    }

    // Every refusal class the reference exercises must have been reached.
    let expected: std::collections::BTreeSet<String> =
        ["empty_block", "non_finite", "shape_mismatch"]
            .into_iter()
            .map(String::from)
            .collect();
    assert_eq!(seen_kinds, expected);
}

/// Acceptance criterion 7 — the signature mentions neither quantisation nor any
/// specific second-operand provenance.
///
/// `TASK.md` §Design note: *"The counterpart is **any** second block, not 'this
/// block's own quantisation'."* Checked two ways: the type names carry no
/// quantisation vocabulary, and the kernel is driven with an **unrelated**
/// second operand — `DIFF-001`'s checkpoint-diff shape — against the value the
/// reference computed for it.
#[test]
fn the_signature_is_neutral_about_where_the_counterpart_came_from() {
    for name in [
        std::any::type_name::<PairedPartials>(),
        std::any::type_name::<ChannelPartials>(),
        std::any::type_name::<ChannelAxis>(),
    ] {
        let lower = name.to_lowercase();
        for forbidden in ["quant", "dequant", "scale", "zero_point", "int8", "int4"] {
            assert!(
                !lower.contains(forbidden),
                "{name} names {forbidden:?}, which specialises this kernel to \
                 quantisation and would cost DIFF-001 and MOE-001 a rewrite"
            );
        }
    }

    // The same kernel over two independently generated blocks.
    let doc = goldens();
    let entry = doc["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|c| c["name"] == "lcg_independent_counterpart_64x64")
        .expect("the checkpoint-diff-shaped case");
    let rows = usize_of(entry, "rows");
    let columns = usize_of(entry, "columns");
    let base = block(entry, "base_bits", rows, columns);
    let counterpart = block(entry, "counterpart_bits", rows, columns);
    let got = CpuBackend
        .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
        .unwrap();
    assert_six(
        "lcg_independent_counterpart_64x64/rows/whole_block",
        six_of_golden(&entry["axes"]["rows"]["whole_block"]),
        six_of_whole(&got),
    );
}

/// Acceptance criterion 6 — the partials held are proportional to the channel
/// count, not to the element count.
///
/// `TASK.md` §Memory and Performance Constraints:
/// `allocation = per_channel.len() × size_of::<ChannelPartials>()`, with
/// *"nothing proportional to tensor size"*.
#[test]
fn the_partials_held_track_channel_count_and_not_element_count() {
    // Six fields: one u64 and five f64.
    assert_eq!(std::mem::size_of::<ChannelPartials>(), 48);

    // Same channel count (8 columns), 8× the elements.
    let small = BlockData::new(4, 8, vec![0.5; 32]).unwrap();
    let large = BlockData::new(32, 8, vec![0.5; 256]).unwrap();
    let zeros_small = BlockData::new(4, 8, vec![0.0; 32]).unwrap();
    let zeros_large = BlockData::new(32, 8, vec![0.0; 256]).unwrap();

    let a = CpuBackend
        .paired_block_reduction(&small, &zeros_small, ChannelAxis::Columns)
        .unwrap();
    let b = CpuBackend
        .paired_block_reduction(&large, &zeros_large, ChannelAxis::Columns)
        .unwrap();

    assert_eq!(a.per_channel.len(), 8);
    assert_eq!(
        a.per_channel.len(),
        b.per_channel.len(),
        "an 8× larger block with the same channel count must hold the same partials"
    );

    // And it does scale with the channel count.
    let wide = BlockData::new(4, 64, vec![0.5; 256]).unwrap();
    let zeros_wide = BlockData::new(4, 64, vec![0.0; 256]).unwrap();
    let c = CpuBackend
        .paired_block_reduction(&wide, &zeros_wide, ChannelAxis::Columns)
        .unwrap();
    assert_eq!(c.per_channel.len(), 64);

    // The 256-column block TASK.md sizes at ~12 KB.
    assert_eq!(256 * std::mem::size_of::<ChannelPartials>(), 12_288);
}

/// A block whose value count disagrees with its declared shape.
///
/// `BlockData::new` refuses to build one, but the fields are `pub`, so the
/// kernel is reachable with one. `TASK.md` §Error Handling's *"refuse before
/// arithmetic"* and `.plan/DIAGNOSTIC_ARCHITECTURE.md` §8's refuse-rather-than-
/// fabricate rule both say the answer is a refusal — never an out-of-bounds
/// index, and never partials computed over whatever values happen to be there.
#[test]
fn a_block_whose_value_count_disagrees_with_its_shape_is_refused() {
    let mut base = BlockData::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let mut counterpart = BlockData::new(2, 3, vec![0.0; 6]).unwrap();
    // Six values, now declared 2x4.
    base.columns = 4;
    counterpart.columns = 4;

    let err = CpuBackend
        .paired_block_reduction(&base, &counterpart, ChannelAxis::Rows)
        .expect_err(
            "a block declaring 2x4 while holding 6 values must be refused, not indexed \
             out of bounds and not reduced over whatever is there",
        );
    let message = err.to_string();
    assert!(
        message.contains('6') && message.contains('4'),
        "the refusal must name the value count and the declared shape: {message}"
    );
}
