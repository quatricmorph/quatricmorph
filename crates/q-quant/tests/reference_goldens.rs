//! Differential test against the independent NumPy reference.
//!
//! `.plan/TEST_STRATEGY.md` §0 rule 2: *"Expected values are computed by hand or
//! by an independent implementation, not by the code under test."*
//! `.plan/DIAGNOSTIC_ARCHITECTURE.md` §9 names the golden level: *"Full
//! quantise-and-reduce over checked-in fixture tensors — a committed Python/NumPy
//! script under `python/`, run in CI-equivalent form."*
//!
//! Every expected value here comes from `python/reference/quantise_reference.py`,
//! which was written from §3.1's formula block and run **before** `q-quant`
//! existed. Regenerate with:
//!
//! ```text
//! python3 python/reference/quantise_reference.py --emit-goldens crates/q-quant/tests/goldens/
//! ```
//!
//! and the file must come back byte-identical. Provenance, including the exact
//! interpreter and NumPy versions, is in `.plan/evidence/QM-0120.md`.
//!
//! ## Why bit equality and not a tolerance
//!
//! Both implementations perform the same `f32` operations in the same order, and
//! IEEE-754 single-precision divide, multiply, subtract and
//! `roundToIntegralTiesToEven` are all correctly rounded — so agreement is
//! **exact**, and a tolerance would only hide a real disagreement. The
//! comparison is therefore on `to_bits()` throughout. There is no toleranced
//! comparison anywhere in this file.
//!
//! The golden is embedded with `include_str!`, so this test performs no file I/O
//! at runtime — the same boundary the crate itself keeps.

use q_quant::{
    derive_params_named, group_extents, round_half_to_even, simulate, simulate_into,
    simulate_per_group_into, Granularity, GroupExtents, Precision, QuantConfig, QuantFidelity,
    QuantParams, RoundMode, UnitId, ZeroPoint,
};
use serde_json::Value;

const GOLDENS: &str = include_str!("goldens/quant-goldens.json");

fn goldens() -> Value {
    serde_json::from_str(GOLDENS).expect("the golden file is valid JSON")
}

/// The normative field in every golden is the f32 bit pattern, so no decimal
/// round trip is involved in any comparison here.
fn f32_of(bits: &Value) -> f32 {
    let s = bits.as_str().expect("bit patterns are strings");
    let hex = s.strip_prefix("0x").expect("bit patterns are 0x-prefixed");
    f32::from_bits(u32::from_str_radix(hex, 16).expect("bit patterns are hex"))
}

fn f32s_of(entry: &Value, key: &str) -> Vec<f32> {
    entry[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} is an array"))
        .iter()
        .map(f32_of)
        .collect()
}

fn precision_of(s: &str) -> Precision {
    match s {
        "int8" => Precision::Int8,
        "int4" => Precision::Int4,
        other => panic!("the reference emitted an unknown precision {other:?}"),
    }
}

fn zero_point_of(s: &str) -> ZeroPoint {
    match s {
        "symmetric" => ZeroPoint::Symmetric,
        "asymmetric" => ZeroPoint::Asymmetric,
        other => panic!("the reference emitted an unknown zero point {other:?}"),
    }
}

/// Build a config from a golden entry. Numeric cases record the rounding mode and
/// it is asserted; refusal entries record only precision and zero point, because
/// they never reach a rounding step.
fn config_of(entry: &Value, granularity: Granularity) -> QuantConfig {
    let cfg = &entry["config"];
    if let Some(round) = cfg["round"].as_str() {
        assert_eq!(
            round, "nearest_even",
            "the reference must only ever emit nearest-even rounding"
        );
    }
    QuantConfig {
        precision: precision_of(cfg["precision"].as_str().expect("precision")),
        granularity,
        zero_point: zero_point_of(cfg["zero_point"].as_str().expect("zero point")),
        round: RoundMode::NearestEven,
    }
}

fn params_of(entry: &Value) -> QuantParams {
    QuantParams::new(
        f32_of(&entry["params"]["scale_bits"]),
        entry["params"]["zero"].as_i64().expect("zero point") as i32,
    )
}

/// Bit-for-bit comparison, reporting the first divergence with both patterns and
/// the exact ULP distance — never a rounded-off "close enough".
fn assert_bits_equal(name: &str, expected: &[f32], actual: &[f32]) {
    assert_eq!(
        expected.len(),
        actual.len(),
        "{name}: length disagreement — expected {} values, got {}",
        expected.len(),
        actual.len()
    );
    for (i, (e, a)) in expected.iter().zip(actual).enumerate() {
        if e.to_bits() != a.to_bits() {
            let ulps = (e.to_bits() as i64 - a.to_bits() as i64).abs();
            panic!(
                "{name}: value {i} disagrees with the NumPy reference.\n  \
                 reference 0x{:08x} ({e:e})\n  q-quant   0x{:08x} ({a:e})\n  \
                 distance  {ulps} ULP (bit-pattern difference)",
                e.to_bits(),
                a.to_bits()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Acceptance criterion 2 — the rounding mode
// ---------------------------------------------------------------------------

#[test]
fn round_half_to_even_agrees_with_numpy_rint_bit_for_bit_at_every_tie() {
    let g = goldens();
    let rows = g["round_half_to_even"].as_array().expect("boundary rows");
    assert!(
        rows.len() >= 4,
        "the reference must cover at least 0.5, 1.5, -0.5 and 2.5"
    );
    let mut checked = 0usize;
    for row in rows {
        let x = f32_of(&row["input_bits"]);
        let expected = f32_of(&row["output_bits"]);
        let actual = round_half_to_even(x);
        // Bits, not value: the reference rounds -0.5 to NEGATIVE zero, and
        // `-0.0 == 0.0` would let a sign error through.
        assert_eq!(
            expected.to_bits(),
            actual.to_bits(),
            "round_half_to_even({x:e}): reference 0x{:08x} ({expected:e}), \
             q-quant 0x{:08x} ({actual:e})",
            expected.to_bits(),
            actual.to_bits()
        );
        checked += 1;
    }
    // The four ties acceptance criterion 2 names, spelled out so a regenerated
    // golden that dropped them fails here rather than passing vacuously.
    for (x, want) in [(0.5f32, 0.0f32), (1.5, 2.0), (-0.5, -0.0), (2.5, 2.0)] {
        assert_eq!(
            round_half_to_even(x).to_bits(),
            want.to_bits(),
            "round_half_to_even({x}) must be {want}, not the half-away-from-zero answer"
        );
    }
    assert_eq!(checked, rows.len());
}

// ---------------------------------------------------------------------------
// Acceptance criterion 1 — the goldens
// ---------------------------------------------------------------------------

#[test]
fn deriving_parameters_reproduces_the_reference_scale_and_zero_point_bit_exactly() {
    let g = goldens();
    let mut derived = 0usize;
    for entry in g["cases"].as_array().expect("cases") {
        if !entry["params_derived_from_this_unit"]
            .as_bool()
            .expect("derivation flag")
        {
            continue;
        }
        let name = entry["name"].as_str().expect("name");
        let config = config_of(entry, Granularity::PerTensor);
        let input = f32s_of(entry, "input_bits");
        let expected = params_of(entry);
        let actual = derive_params_named(&input, &config, UnitId::new(name, 0))
            .unwrap_or_else(|e| panic!("{name}: derive_params refused: {e}"));
        assert_eq!(
            expected.scale.to_bits(),
            actual.scale.to_bits(),
            "{name}: scale disagrees — reference 0x{:08x} ({:e}), q-quant 0x{:08x} ({:e})",
            expected.scale.to_bits(),
            expected.scale,
            actual.scale.to_bits(),
            actual.scale
        );
        assert_eq!(
            expected.zero, actual.zero,
            "{name}: zero point disagrees — reference {}, q-quant {}",
            expected.zero, actual.zero
        );
        derived += 1;
    }
    assert!(
        derived >= 8,
        "only {derived} cases derived their own parameters; the golden set shrank"
    );
}

#[test]
fn dequantising_every_golden_unit_reproduces_the_reference_bit_exactly() {
    let g = goldens();
    let cases = g["cases"].as_array().expect("cases");
    assert!(!cases.is_empty(), "the golden set is empty");
    let mut values_compared = 0usize;
    for entry in cases {
        let name = entry["name"].as_str().expect("name");
        let config = config_of(entry, Granularity::PerTensor);
        let input = f32s_of(entry, "input_bits");
        let expected = f32s_of(entry, "output_bits");
        let params = params_of(entry);

        let actual = simulate(&input, &params, &config)
            .unwrap_or_else(|e| panic!("{name}: simulate refused: {e}"));
        assert_bits_equal(name, &expected, &actual);

        // The caller-buffer path must produce the identical bits, not merely a
        // close answer: the streaming pass in `QM-0122` uses only this one.
        let mut buffer = vec![f32::NAN; input.len()];
        simulate_into(&input, &params, &config, &mut buffer)
            .unwrap_or_else(|e| panic!("{name}: simulate_into refused: {e}"));
        assert_bits_equal(
            &format!("{name} (into a caller buffer)"),
            &expected,
            &buffer,
        );

        // Fidelity is derived from bit equality, and the reference recorded the
        // same fact independently.
        let expect_exact = entry["round_trip_is_bit_exact"]
            .as_bool()
            .expect("exactness flag");
        let fidelity = QuantFidelity::of_round_trip(&input, &actual).expect("equal lengths");
        assert_eq!(
            fidelity == QuantFidelity::Exact,
            expect_exact,
            "{name}: the reference says bit-exact={expect_exact}, q-quant labelled it {}",
            fidelity.as_str()
        );

        values_compared += input.len();
    }
    assert!(
        values_compared >= 8_000,
        "only {values_compared} values were compared; the 4096-element units are missing"
    );
}

#[test]
fn per_group_simulation_reproduces_the_reference_bit_exactly_with_a_clamped_final_group() {
    let g = goldens();
    let cases = g["grouped_cases"].as_array().expect("grouped cases");
    assert!(!cases.is_empty(), "the grouped golden set is empty");
    for entry in cases {
        let name = entry["name"].as_str().expect("name");
        let size = entry["group_size"].as_u64().expect("group size") as u32;
        let cfg = &entry["config"];
        let config = QuantConfig {
            precision: precision_of(cfg["precision"].as_str().expect("precision")),
            granularity: Granularity::PerGroup { size },
            zero_point: zero_point_of(cfg["zero_point"].as_str().expect("zero point")),
            round: RoundMode::NearestEven,
        };
        let input = f32s_of(entry, "input_bits");
        let expected = f32s_of(entry, "output_bits");
        let groups = entry["groups"].as_array().expect("groups");

        // The extents themselves, before any arithmetic: the final group is
        // clamped, never padded.
        let extents: Vec<_> = group_extents(input.len(), size)
            .expect("valid group size")
            .collect();
        assert_eq!(
            extents.len(),
            groups.len(),
            "{name}: reference has {} groups, q-quant produced {}",
            groups.len(),
            extents.len()
        );
        assert_eq!(
            GroupExtents::count_of(input.len(), size).expect("valid group size"),
            groups.len(),
            "{name}: count_of disagrees with the reference group count"
        );
        for (extent, group) in extents.iter().zip(groups) {
            assert_eq!(
                extent.start,
                group["start"].as_u64().expect("start") as usize,
                "{name}: group start disagrees"
            );
            assert_eq!(
                extent.end,
                group["end"].as_u64().expect("end") as usize,
                "{name}: group end disagrees"
            );
            assert_eq!(
                extent.len(),
                group["count"].as_u64().expect("count") as usize,
                "{name}: group element count disagrees — the final group must be \
                 clamped, never padded"
            );
            // Each group derives its own parameters from its own values.
            let derived = derive_params_named(
                &input[extent.clone()],
                &config,
                UnitId::new(name, extent.start as u64),
            )
            .unwrap_or_else(|e| panic!("{name}: group {extent:?} refused: {e}"));
            assert_eq!(
                f32_of(&group["scale_bits"]).to_bits(),
                derived.scale.to_bits(),
                "{name}: group {extent:?} scale disagrees with the reference"
            );
            assert_eq!(
                group["zero"].as_i64().expect("zero") as i32,
                derived.zero,
                "{name}: group {extent:?} zero point disagrees with the reference"
            );
        }

        let mut out = vec![f32::NAN; input.len()];
        simulate_per_group_into(&input, &config, &mut out, UnitId::new(name, 0))
            .unwrap_or_else(|e| panic!("{name}: simulate_per_group_into refused: {e}"));
        assert_bits_equal(name, &expected, &out);
    }
}

// ---------------------------------------------------------------------------
// Acceptance criterion 3 and the negative paths
// ---------------------------------------------------------------------------

#[test]
fn every_unit_the_reference_refuses_is_refused_with_the_same_kind() {
    let g = goldens();
    let refusals = g["refusals"].as_array().expect("refusals");
    assert!(refusals.len() >= 10, "the refusal golden set shrank");
    for entry in refusals {
        let name = entry["name"].as_str().expect("name");
        let expected_kind = entry["kind"].as_str().expect("kind");
        let entry_point = entry["entry_point"].as_str().expect("entry point");
        let input = f32s_of(entry, "input_bits");
        let unit = UnitId::new(name, 4096);

        let err = match entry_point {
            "derive_params" => {
                let config = config_of(entry, Granularity::PerTensor);
                derive_params_named(&input, &config, unit)
                    .map(|p| format!("{p:?}"))
                    .expect_err(&format!("{name}: derive_params must refuse"))
            }
            "simulate" => {
                let config = config_of(entry, Granularity::PerTensor);
                let params = params_of(entry);
                let mut out = vec![0.0f32; input.len()];
                q_quant::simulate_into_named(&input, &params, &config, &mut out, unit)
                    .expect_err(&format!("{name}: simulate must refuse"))
            }
            "derive_then_simulate" => {
                // The realistic path: parameters derived from this very unit, and
                // the refusal only becomes visible when they are applied.
                let config = config_of(entry, Granularity::PerTensor);
                let params = derive_params_named(&input, &config, unit).unwrap_or_else(|e| {
                    panic!("{name}: derive_params must SUCCEED here; it refused: {e}")
                });
                assert_eq!(
                    params.scale.to_bits(),
                    f32_of(&entry["params"]["scale_bits"]).to_bits(),
                    "{name}: the derived scale must match the reference before the \
                     refusal is compared"
                );
                let mut out = vec![0.0f32; input.len()];
                q_quant::simulate_into_named(&input, &params, &config, &mut out, unit)
                    .expect_err(&format!("{name}: simulate must refuse"))
            }
            "group_extents" => {
                let size = entry["group_size"].as_u64().expect("group size") as u32;
                group_extents(input.len(), size)
                    .map(|g| format!("{g:?}"))
                    .expect_err(&format!("{name}: group_extents must refuse"))
            }
            other => panic!("{name}: unknown entry point {other:?}"),
        };

        assert_eq!(
            err.kind(),
            expected_kind,
            "{name}: the reference refused with {expected_kind:?}, q-quant with \
             {:?} — message: {err}",
            err.kind()
        );

        // §3.1 requires the refused unit to be NAMED. Config-level refusals
        // precede any unit, so they are exempt and say so.
        if expected_kind != "config_rejected" {
            let message = err.to_string();
            assert!(
                message.contains(name) && message.contains("4096"),
                "{name}: the refusal must name the tensor and the offset; got {message:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The divergence, measured rather than assumed
// ---------------------------------------------------------------------------

#[test]
fn the_measured_divergence_from_the_reference_is_zero_ulp_across_every_golden_value() {
    // Requirement: record the ACTUAL ULP divergence, not a comfortable bound.
    // This test measures it over every value in the golden set and asserts it is
    // exactly zero, printing the totals so the figure in
    // `.plan/evidence/QM-0120.md` is a measurement and not a claim. Run with
    // `cargo test -p q-quant -- --nocapture` to see it.
    let g = goldens();
    let mut compared = 0usize;
    let mut max_ulp = 0i64;
    let mut worst = String::new();

    let mut record = |name: &str, expected: &[f32], actual: &[f32]| {
        for (i, (e, a)) in expected.iter().zip(actual).enumerate() {
            let ulp = (e.to_bits() as i64 - a.to_bits() as i64).abs();
            if ulp > max_ulp {
                max_ulp = ulp;
                worst = format!("{name}[{i}]");
            }
            compared += 1;
        }
    };

    for entry in g["cases"].as_array().expect("cases") {
        let name = entry["name"].as_str().expect("name");
        let config = config_of(entry, Granularity::PerTensor);
        let input = f32s_of(entry, "input_bits");
        let expected = f32s_of(entry, "output_bits");
        let actual = simulate(&input, &params_of(entry), &config).expect("simulate");
        record(name, &expected, &actual);
    }
    for entry in g["grouped_cases"].as_array().expect("grouped cases") {
        let name = entry["name"].as_str().expect("name");
        let size = entry["group_size"].as_u64().expect("group size") as u32;
        let cfg = &entry["config"];
        let config = QuantConfig {
            precision: precision_of(cfg["precision"].as_str().expect("precision")),
            granularity: Granularity::PerGroup { size },
            zero_point: zero_point_of(cfg["zero_point"].as_str().expect("zero point")),
            round: RoundMode::NearestEven,
        };
        let input = f32s_of(entry, "input_bits");
        let expected = f32s_of(entry, "output_bits");
        let mut actual = vec![f32::NAN; input.len()];
        simulate_per_group_into(&input, &config, &mut actual, UnitId::new(name, 0))
            .expect("per-group simulate");
        record(name, &expected, &actual);
    }
    // Scales and zero points too, not only reconstructions.
    let mut params_compared = 0usize;
    for entry in g["cases"].as_array().expect("cases") {
        if !entry["params_derived_from_this_unit"].as_bool().unwrap() {
            continue;
        }
        let name = entry["name"].as_str().expect("name");
        let config = config_of(entry, Granularity::PerTensor);
        let input = f32s_of(entry, "input_bits");
        let derived = derive_params_named(&input, &config, UnitId::new(name, 0)).expect("derive");
        let expected = params_of(entry);
        let ulp = (expected.scale.to_bits() as i64 - derived.scale.to_bits() as i64).abs();
        if ulp > max_ulp {
            max_ulp = ulp;
            worst = format!("{name}.scale");
        }
        params_compared += 1;
    }

    println!(
        "measured against python/reference/quantise_reference.py: {compared} reconstructed \
         values and {params_compared} derived scales compared; maximum divergence {max_ulp} ULP\
         {}",
        if max_ulp == 0 {
            String::new()
        } else {
            format!(" (worst at {worst})")
        }
    );
    assert_eq!(
        max_ulp, 0,
        "the maximum divergence from the NumPy reference is {max_ulp} ULP at {worst}; \
         it must be zero, because both implementations perform the same correctly rounded \
         f32 operations in the same order"
    );
    assert!(
        compared >= 8_000,
        "only {compared} values were compared; the 4096-element units are missing"
    );
}

// ---------------------------------------------------------------------------
// A guard on the golden set itself
// ---------------------------------------------------------------------------

#[test]
fn the_golden_set_still_covers_every_case_the_task_enumerates() {
    let g = goldens();
    assert_eq!(
        g["schema"].as_str(),
        Some("quatricmorph/quant-goldens/v1"),
        "the golden schema changed"
    );
    assert_eq!(g["requirement"].as_str(), Some("QUANT-001"));

    let mut names: Vec<&str> = Vec::new();
    for key in ["cases", "grouped_cases", "refusals"] {
        for entry in g[key].as_array().expect(key) {
            names.push(entry["name"].as_str().expect("name"));
        }
    }
    // One name per `TASK.md` §Test Cases row, so a regenerated golden that
    // dropped a row fails here instead of passing with fewer comparisons.
    for required in [
        "minus_one_zero_one_int8_symmetric",                   // row 1
        "tenths_int4_asymmetric",                              // row 2
        "all_zero_int8_symmetric",                             // row 3
        "subnormal_scale_int8_symmetric",                      // row 4
        "nan_in_unit_int8_symmetric",                          // row 5
        "group_130_of_128_int8_symmetric",                     // row 6
        "exactly_representable_int8_symmetric",                // row 7
        "lcg_4096_int8_symmetric",                             // row 9
        "boundary_halves_scale_one_int8_symmetric",            // AC 2, ties
        "clamping_boundaries_int4_symmetric",                  // AC 2, clamping
        "clamping_boundaries_int4_asymmetric",                 // AC 2, clamping
        "constant_ones_int4_asymmetric",                       // zero dynamic range, non-zero
        "constant_half_int4_asymmetric",                       // the case s = 1 gets WRONG
        "constant_negative_three_tenths_int8_asymmetric",      // its negative sign
        "constant_irrational_int4_asymmetric",                 // no short binary expansion
        "constant_irrational_int4_symmetric",                  // the two modes now agree
        "constant_subnormal_int4_asymmetric",                  // refuses on the subnormal row
        "zero_point_outside_i32_int8_asymmetric",              // zero point out of range
        "large_but_valid_zero_point_int8_asymmetric",          // the largest that fits
        "scale_overflows_to_infinity_int8_asymmetric",         // never emit infinities: scale
        "reconstruction_overflows_to_infinity_int8_symmetric", // ... and product
    ] {
        assert!(
            names.contains(&required),
            "the golden set no longer covers {required:?}"
        );
    }
    // Row 8 (`Precision::Nf4`) is a scheme refusal with no numeric golden; it is
    // covered by `q_quant`'s own `requesting_nf4_refuses_naming_quant_011`.
    assert!(
        !names.contains(&"nf4"),
        "NF4 has no arithmetic and must not acquire a numeric golden"
    );
}
