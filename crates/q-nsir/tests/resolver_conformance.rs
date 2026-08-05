//! `QM-0011` — cross-family resolver conformance.
//!
//! Requirements: `NSIR-001` (an untaught name stays `unknown`), `NSIR-006`
//! (per-family resolvers), `NSIR-008` (registry priority selection and generic
//! fallback). `TASK.md` also names `MVP-08`; this suite does **not**
//! independently witness it — no per-criterion text for `MVP-08` exists in the
//! repository, and `.plan/REQUIREMENT_TRACEABILITY.md:184` maps it to `QM-0010`,
//! whose work this suite guards. See `.plan/evidence/QM-0011.md` §Claim limits.
//!
//! One table-driven suite over `architectures/conformance.json`, asserting that
//! **every architecture plugin resolves what it claims and returns `unknown`
//! for everything else**.
//!
//! ## Where the expected values come from
//!
//! Never from the code under test. Every `canonical` in the corpus was written
//! by hand from the address rule fixed by `ARCHITECTURE.md` §6.1
//! (`model.layers[10].self_attention.query_projection.weight`) and the
//! operation/axis vocabulary fixed by §4.2, together with the **declared**
//! `architectures/*/plugin.toml` rule tables, which are the resolver's input
//! rather than its output. Every Qwen row that also appears in
//! `fixtures/tiny-qwen-single/golden.json` — the checked-in golden `QM-0010`
//! generated — is cross-checked against it, so a transcription slip in either
//! file fails the suite. No test here touches the network.
//!
//! ## Why none of this passes vacuously
//!
//! `QM-0010` volunteered that a conformance test can pass against an **empty**
//! rule table, because an empty table answers `unknown` to everything. Every
//! negative row in this corpus therefore names a `control`: a raw name in the
//! same family that must *still resolve*. Against an empty rule table the
//! negative row would still say `unknown` — and its control would stop
//! resolving, so the assertion fails. The two unimplemented families (`kimi`,
//! `deepseek`) are the deliberate exception and are documented as such at
//! [`an_unimplemented_family_never_claims_a_model_and_the_generic_fallback_answers_instead`].
//!
//! ## What this suite does not establish
//!
//! Nothing about what any tensor means. A name pattern is not a concept
//! (`ARCHITECTURE.md` §19); `query_projection` is the checkpoint author's word
//! propagated through a declared rule table. Resolution is **exact** — a
//! deterministic function of a raw name and a declared table, nothing sampled
//! and nothing estimated — and that is the only claim made.

use q_architecture::{ArchitecturePlugin, MatchKind, Registry};
use q_nsir::{canonical_name, NsirRecord, NsirResolver, ResolvedModel};
use q_source::error::QError;
use q_source::role::{Component, Stack, TensorRole};
use q_source::{DType, ModelId, TensorDescriptor, TensorId};
use serde::Deserialize;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// One corpus row. A positive row carries a `canonical`; a negative row carries
/// `canonical: null` and — for an implemented family — a `control`.
#[derive(Debug, Clone, Deserialize)]
struct Row {
    raw: String,
    canonical: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    component: Option<String>,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    parameter: Option<String>,
    #[serde(default)]
    axes: Option<Vec<String>>,
    #[serde(default)]
    layer: Option<u32>,
    #[serde(default)]
    expert: Option<u32>,
    #[serde(default)]
    control: Option<String>,
    #[serde(default)]
    why: Option<String>,
}

impl Row {
    fn is_positive(&self) -> bool {
        self.canonical.is_some()
    }
}

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../architectures/conformance.json")
}

fn corpus() -> serde_json::Value {
    let path = corpus_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

/// Top-level keys that name a family. Keys beginning with `_` are metadata and
/// are never a family — the repository already spells provenance that way in
/// `scripts/baseline.json` and `fixtures/tiny-qwen-single/golden.json`.
fn family_names(corpus: &serde_json::Value) -> Vec<String> {
    corpus
        .as_object()
        .expect("the corpus must be a JSON object")
        .keys()
        .filter(|k| !k.starts_with('_'))
        .cloned()
        .collect()
}

fn rows_of(corpus: &serde_json::Value, family: &str) -> Vec<Row> {
    serde_json::from_value(corpus[family].clone())
        .unwrap_or_else(|e| panic!("family `{family}` has a malformed row: {e}"))
}

/// Every family key the registry does not carry, named.
///
/// `TASK.md` §Error Handling: *"A corpus row referencing a non-existent family →
/// failure naming it."*
fn families_the_registry_does_not_carry(
    registry: &Registry,
    corpus: &serde_json::Value,
) -> Vec<String> {
    family_names(corpus)
        .into_iter()
        .filter(|f| registry.get(f).is_none())
        .collect()
}

// ---------------------------------------------------------------------------
// The one comparison every conformance test is built from
// ---------------------------------------------------------------------------

/// Every disagreement between `rows` and what `plugin` actually resolves, one
/// human-readable line each. An empty vector means the family conforms.
///
/// Both directions are checked, and a negative row's `control` is checked with
/// it, because a negative row on its own would pass against an empty rule
/// table.
fn conformance_failures(plugin: &ArchitecturePlugin, rows: &[Row]) -> Vec<String> {
    let resolver = NsirResolver::new(plugin);
    let id = plugin.id();
    let mut failures = Vec::new();

    for row in rows {
        let got = resolver.resolve_name(&row.raw);
        match &row.canonical {
            Some(expected) => {
                if !got.resolved {
                    failures.push(format!(
                        "{id}: `{}` did not resolve; the corpus expects `{expected}`",
                        row.raw
                    ));
                    continue;
                }
                let address = canonical_name(&got).expect("a resolved record has an address");
                if address != *expected {
                    failures.push(format!(
                        "{id}: `{}` resolved to `{address}`; the corpus expects `{expected}`",
                        row.raw
                    ));
                }
                check(
                    &mut failures,
                    id,
                    &row.raw,
                    "stack",
                    got.stack,
                    Stack::Language,
                );
                if let Some(want) = &row.role {
                    check(
                        &mut failures,
                        id,
                        &row.raw,
                        "role",
                        got.role.as_str(),
                        want.as_str(),
                    );
                }
                if let Some(want) = &row.component {
                    check(
                        &mut failures,
                        id,
                        &row.raw,
                        "component",
                        got.component.as_str(),
                        want.as_str(),
                    );
                }
                if let Some(want) = &row.operation {
                    check(
                        &mut failures,
                        id,
                        &row.raw,
                        "operation",
                        &got.operation,
                        want,
                    );
                }
                if let Some(want) = &row.parameter {
                    check(
                        &mut failures,
                        id,
                        &row.raw,
                        "parameter",
                        &got.parameter,
                        want,
                    );
                }
                if let Some(want) = &row.axes {
                    check(&mut failures, id, &row.raw, "axes", &got.axes, want);
                }
                check(&mut failures, id, &row.raw, "layer", got.layer, row.layer);
                check(
                    &mut failures,
                    id,
                    &row.raw,
                    "expert",
                    got.expert,
                    row.expert,
                );
                check(
                    &mut failures,
                    id,
                    &row.raw,
                    "resolver_id",
                    got.resolver_id.as_str(),
                    id,
                );
            }
            None => {
                if got.resolved {
                    failures.push(format!(
                        "{id}: `{}` claimed to resolve, to `{}`; the corpus expects `unknown`{}",
                        row.raw,
                        canonical_name(&got).unwrap_or_default(),
                        why(row),
                    ));
                }
                check(
                    &mut failures,
                    id,
                    &row.raw,
                    "role",
                    got.role,
                    TensorRole::Unknown,
                );
                check(
                    &mut failures,
                    id,
                    &row.raw,
                    "component",
                    got.component,
                    Component::Unknown,
                );
                if !got.operation.is_empty() {
                    failures.push(format!(
                        "{id}: `{}` invented the operation `{}`",
                        row.raw, got.operation
                    ));
                }
                if !got.axes.is_empty() {
                    failures.push(format!(
                        "{id}: `{}` invented the axis labels {:?}",
                        row.raw, got.axes
                    ));
                }
                if canonical_name(&got).is_some() {
                    failures.push(format!(
                        "{id}: `{}` produced a canonical address it has not earned",
                        row.raw
                    ));
                }
                // The anti-vacuity check. Without it this row would pass
                // against an empty rule table.
                if let Some(control) = &row.control {
                    let c = resolver.resolve_name(control);
                    if !c.resolved {
                        failures.push(format!(
                            "{id}: the control `{control}` for negative row `{}` did not resolve, \
                             so that row proves nothing — an empty rule table would satisfy it",
                            row.raw
                        ));
                    }
                }
            }
        }
    }
    failures
}

fn check<T: PartialEq + std::fmt::Debug>(
    failures: &mut Vec<String>,
    id: &str,
    raw: &str,
    field: &str,
    got: T,
    want: T,
) {
    if got != want {
        failures.push(format!(
            "{id}: `{raw}` {field} is {got:?}; the corpus expects {want:?}"
        ));
    }
}

fn why(row: &Row) -> String {
    match &row.why {
        Some(w) => format!(" ({w})"),
        None => String::new(),
    }
}

fn report(failures: &[String]) -> String {
    format!("\n  {}\n", failures.join("\n  "))
}

fn descriptor(raw: &str, shape: Vec<u64>) -> TensorDescriptor {
    let model = ModelId::derive("qm-0011-conformance", "", "names-only");
    let elements: u64 = shape.iter().product();
    TensorDescriptor {
        tensor_id: TensorId::derive(model, raw),
        raw_name: raw.to_string(),
        canonical_name: raw.to_string(),
        shape,
        dtype: DType::F32,
        shard_uri: "conformance.safetensors".into(),
        byte_start: 0,
        byte_end: elements * 4,
        layer_index: None,
        semantic_role: TensorRole::Unknown,
    }
}

// ---------------------------------------------------------------------------
// §Test Cases row 1 · AC 1 · NSIR-006 — the corpus itself
// ---------------------------------------------------------------------------

#[test]
fn every_corpus_row_resolves_exactly_as_the_corpus_says_it_must() {
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    let mut failures = Vec::new();
    let mut rows_checked = 0usize;

    for family in family_names(&corpus) {
        let plugin = registry
            .get(&family)
            .unwrap_or_else(|| panic!("the corpus names a family the registry lacks: `{family}`"));
        let rows = rows_of(&corpus, &family);
        assert!(!rows.is_empty(), "family `{family}` has no corpus rows");
        rows_checked += rows.len();
        failures.extend(conformance_failures(plugin, &rows));
    }

    assert!(failures.is_empty(), "{}", report(&failures));
    // The count is asserted so that a corpus emptied by a bad edit fails here
    // rather than passing with nothing to check.
    assert!(
        rows_checked >= 100,
        "only {rows_checked} corpus rows were checked; the corpus has shrunk"
    );
}

#[test]
fn the_corpus_declares_exact_fidelity_because_resolution_samples_and_estimates_nothing() {
    // Every result this repository shows must be labelled exact, sampled, or
    // approximate. A resolved address is a deterministic function of a raw name
    // and a declared rule table, so it is exact — and the corpus says so rather
    // than leaving a reader to assume it.
    let corpus = corpus();
    assert_eq!(corpus["_fidelity"], serde_json::json!("exact"));
    assert!(
        corpus["_canonical_address_rule"]
            .as_str()
            .unwrap_or_default()
            .contains("section 6.1"),
        "the corpus must cite the address rule it was written from"
    );
}

#[test]
fn every_corpus_family_names_a_plugin_the_registry_actually_carries() {
    let registry = Registry::builtin().unwrap();
    let unknown = families_the_registry_does_not_carry(&registry, &corpus());
    assert!(
        unknown.is_empty(),
        "the corpus names {unknown:?}, which the registry does not carry"
    );
}

#[test]
fn a_corpus_family_the_registry_does_not_carry_is_named_in_the_failure() {
    // §Error Handling, second bullet, exercised rather than asserted: the check
    // above would pass silently if the detector were dead.
    let registry = Registry::builtin().unwrap();
    let bogus = serde_json::json!({
        "_comment": "metadata keys are not families",
        "qwen": [],
        "mistral": [{ "raw": "model.layers.10.self_attn.q_proj.weight", "canonical": null }],
    });
    let unknown = families_the_registry_does_not_carry(&registry, &bogus);
    assert_eq!(unknown, vec!["mistral".to_string()]);
}

#[test]
fn the_corpus_covers_every_family_the_registry_carries() {
    // A family with no rows is a family nothing guards.
    let registry = Registry::builtin().unwrap();
    let covered = family_names(&corpus());
    for plugin in registry.plugins() {
        assert!(
            covered.iter().any(|f| f == plugin.id()),
            "the registry carries `{}` and the corpus has no rows for it",
            plugin.id()
        );
    }
}

#[test]
fn no_family_lists_the_same_raw_name_twice() {
    // A duplicate row is a row whose failure would be reported twice and whose
    // count would flatter the corpus.
    let corpus = corpus();
    let mut checked = 0usize;
    for family in family_names(&corpus) {
        let rows = rows_of(&corpus, &family);
        for (i, row) in rows.iter().enumerate() {
            let first = rows.iter().position(|r| r.raw == row.raw).unwrap();
            assert_eq!(first, i, "family `{family}` lists `{}` twice", row.raw);
            checked += 1;
        }
    }
    assert!(
        checked >= 100,
        "only {checked} rows were checked for duplicates"
    );
}

#[test]
fn every_corpus_row_resolves_identically_on_a_second_run() {
    // `NSIR-004`: a canonical address is stable, so it is safe to store in a
    // catalog, a report, or an annotation.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    let mut compared = 0usize;
    for family in family_names(&corpus) {
        let resolver = NsirResolver::new(registry.get(&family).unwrap());
        for row in rows_of(&corpus, &family) {
            let first = resolver.resolve_name(&row.raw);
            let second = resolver.resolve_name(&row.raw);
            assert_eq!(first, second, "{family}: `{}` record", row.raw);
            assert_eq!(
                canonical_name(&first),
                canonical_name(&second),
                "{family}: `{}` address",
                row.raw
            );
            compared += 1;
        }
    }
    assert!(compared >= 100, "only {compared} rows were resolved twice");
}

// ---------------------------------------------------------------------------
// AC 1 · §Error Handling — every declared pattern is exercised
// ---------------------------------------------------------------------------

/// Declared patterns of `plugin` that no row in `rows` depends on.
///
/// Determined by deletion rather than by re-implementing the matcher: remove
/// one rule, resolve every row again, and see whether anything changed. If
/// nothing changed, no row needed that rule. This is exact, uses only the
/// public API, and cannot drift from how matching actually works.
fn patterns_no_corpus_row_depends_on(plugin: &ArchitecturePlugin, rows: &[Row]) -> Vec<String> {
    let resolver = NsirResolver::new(plugin);
    let before: Vec<NsirRecord> = rows.iter().map(|r| resolver.resolve_name(&r.raw)).collect();
    let mut uncovered = Vec::new();

    for (i, rule) in plugin.rules.iter().enumerate() {
        let mut without = plugin.clone();
        without.rules.remove(i);
        let mutated = NsirResolver::new(&without);
        let after: Vec<NsirRecord> = rows.iter().map(|r| mutated.resolve_name(&r.raw)).collect();
        if before == after {
            uncovered.push(format!(
                "{}: rule `{}` ({:?}) has no corpus row — deleting it changes nothing",
                plugin.id(),
                rule.name,
                rule.match_kind
            ));
        }
    }
    uncovered
}

#[test]
fn every_declared_pattern_of_every_family_is_exercised_by_at_least_one_corpus_row() {
    // Acceptance criterion 1, and §Error Handling's first bullet: an uncovered
    // declared pattern fails the suite *naming the pattern*.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    let mut uncovered = Vec::new();
    let mut patterns_checked = 0usize;

    for family in family_names(&corpus) {
        let plugin = registry.get(&family).unwrap();
        patterns_checked += plugin.rules.len();
        uncovered.extend(patterns_no_corpus_row_depends_on(
            plugin,
            &rows_of(&corpus, &family),
        ));
    }

    assert!(uncovered.is_empty(), "{}", report(&uncovered));
    // Anti-vacuity: with no declared patterns anywhere there would be nothing
    // to cover and this test would pass having checked nothing.
    assert!(
        patterns_checked >= 54,
        "only {patterns_checked} declared patterns were checked; \
         generic declares 12, llama 21 and qwen 21"
    );
}

#[test]
fn a_declared_pattern_with_no_corpus_row_is_named_in_the_failure() {
    // The detector above, driven against a manifest carrying a pattern the
    // corpus does not exercise. Without this, `every_declared_pattern_…` would
    // pass silently if the deletion probe stopped working.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    let mut plugin = registry.get("qwen").unwrap().clone();
    let mut orphan = plugin.rules[0].clone();
    orphan.name = "self_attn.sink_proj.weight".to_string();
    orphan.match_kind = MatchKind::Suffix;
    plugin.rules.push(orphan);

    let uncovered = patterns_no_corpus_row_depends_on(&plugin, &rows_of(&corpus, "qwen"));
    assert_eq!(uncovered.len(), 1, "{}", report(&uncovered));
    assert!(
        uncovered[0].contains("self_attn.sink_proj.weight"),
        "the failure must name the uncovered pattern: {}",
        uncovered[0]
    );
}

// ---------------------------------------------------------------------------
// AC 6 · §Verification Plan (manual) — deleting a pattern fails the suite
// ---------------------------------------------------------------------------

#[test]
fn removing_a_declared_pattern_from_a_manifest_makes_the_suite_fail_naming_the_row() {
    // Acceptance criterion 6, and the §Verification Plan's manual step, run
    // automatically: delete one Qwen pattern and confirm the suite names what
    // stopped resolving. The mutation is in memory, so no manifest on disk is
    // touched — and it drives the *same* `conformance_failures` the green test
    // uses, so the demonstration is of the real check rather than of a copy.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    let rows = rows_of(&corpus, "qwen");
    let qwen = registry.get("qwen").unwrap();
    assert!(
        conformance_failures(qwen, &rows).is_empty(),
        "the unmutated manifest must conform, or this demonstration proves nothing"
    );

    let victim = "self_attn.q_norm.weight";
    let mut without = qwen.clone();
    let before = without.rules.len();
    without.rules.retain(|r| r.name != victim);
    assert_eq!(
        without.rules.len(),
        before - 1,
        "`{victim}` was not declared"
    );

    let failures = conformance_failures(&without, &rows);
    assert!(!failures.is_empty(), "deleting `{victim}` changed nothing");
    assert!(
        failures
            .iter()
            .any(|f| f.contains("model.layers.10.self_attn.q_norm.weight")),
        "the failure must name the row that stopped resolving: {}",
        report(&failures)
    );
    assert!(
        failures
            .iter()
            .any(|f| { f.contains("model.layers[10].self_attention.query_normalization.weight") }),
        "the failure must name the address the corpus expects: {}",
        report(&failures)
    );
}

#[test]
fn changing_a_declared_operation_makes_the_suite_fail_naming_the_wrong_address() {
    // The other half of AC 6: a pattern that is still declared but declares a
    // different label must fail too, or the suite would only guard existence.
    let registry = Registry::builtin().unwrap();
    let rows = rows_of(&corpus(), "llama");
    let mut mutated = registry.get("llama").unwrap().clone();
    let rule = mutated
        .rules
        .iter_mut()
        .find(|r| r.name == "self_attn.q_proj.weight" && r.match_kind == MatchKind::Suffix)
        .expect("llama declares the ARCHITECTURE.md §6.1 pattern");
    rule.operation = "query_proj".to_string();

    let failures = conformance_failures(&mutated, &rows);
    assert!(
        failures
            .iter()
            .any(|f| f.contains("model.layers[10].self_attention.query_proj.weight")),
        "{}",
        report(&failures)
    );
}

// ---------------------------------------------------------------------------
// AC 2 · NSIR-001 — negatives, and why they are not vacuous
// ---------------------------------------------------------------------------

#[test]
fn every_implemented_family_carries_at_least_ten_negative_rows() {
    // Acceptance criterion 2. Counted per family so a rich `llama` list cannot
    // cover for an empty `generic` one.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    for family in family_names(&corpus) {
        let plugin = registry.get(&family).unwrap();
        if !plugin.is_implemented() {
            continue;
        }
        let negatives = rows_of(&corpus, &family)
            .into_iter()
            .filter(|r| !r.is_positive())
            .count();
        assert!(
            negatives >= 10,
            "family `{family}` has only {negatives} negative rows; the criterion is 10"
        );
    }
}

#[test]
fn an_untaught_name_stays_unknown_and_is_never_answered_with_a_nearest_guess() {
    // `NSIR-001`, over every negative row of every implemented family: not
    // resolved, role `Unknown`, component `Unknown`, no invented operation, no
    // invented axis labels, and no canonical address.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    let mut failures = Vec::new();
    let mut negatives = 0usize;

    for family in family_names(&corpus) {
        let plugin = registry.get(&family).unwrap();
        let rows: Vec<Row> = rows_of(&corpus, &family)
            .into_iter()
            .filter(|r| !r.is_positive())
            .collect();
        negatives += rows.len();
        failures.extend(conformance_failures(plugin, &rows));
    }
    assert!(failures.is_empty(), "{}", report(&failures));
    assert!(
        negatives >= 30,
        "only {negatives} negative rows were checked"
    );
}

#[test]
fn every_negative_row_of_an_implemented_family_names_a_control_that_still_resolves() {
    // This is what stops the negative rows above from passing vacuously. A
    // negative row alone would be satisfied by an empty rule table; a control
    // that must resolve would not be.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    for family in family_names(&corpus) {
        let plugin = registry.get(&family).unwrap();
        if !plugin.is_implemented() {
            continue;
        }
        let rows = rows_of(&corpus, &family);
        let resolver = NsirResolver::new(plugin);
        for row in rows.iter().filter(|r| !r.is_positive()) {
            let control = row
                .control
                .as_ref()
                .unwrap_or_else(|| panic!("{family}: negative row `{}` names no control", row.raw));
            assert!(
                rows.iter().any(|r| r.is_positive() && r.raw == *control),
                "{family}: the control `{control}` for `{}` is not a positive row of the \
                 same family, so it is not independently checked",
                row.raw
            );
            assert!(
                resolver.resolve_name(control).resolved,
                "{family}: the control `{control}` for `{}` does not resolve",
                row.raw
            );
        }
    }
}

#[test]
fn an_empty_rule_table_fails_this_suite_rather_than_satisfying_its_negative_rows() {
    // The vacuity question, answered directly. Strip every rule from each
    // implemented family and confirm the corpus rejects the result. If this
    // ever passes, the negative rows have stopped proving anything.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    for family in ["generic", "llama", "qwen"] {
        let mut empty = registry.get(family).unwrap().clone();
        empty.rules.clear();
        let failures = conformance_failures(&empty, &rows_of(&corpus, family));
        assert!(
            !failures.is_empty(),
            "an empty `{family}` rule table satisfied the whole corpus"
        );
        assert!(
            failures.iter().any(|f| f.contains("the control")),
            "an empty `{family}` rule table must break the negative rows' controls, \
             not merely the positive rows: {}",
            report(&failures)
        );
    }
}

#[test]
fn a_layer_index_that_is_not_a_number_leaves_the_layer_absent_and_the_whole_name_unknown() {
    // §Test Cases row 3, asserted directly rather than only through the corpus
    // loop, plus the structural half the corpus cannot express: the layer index
    // must be *absent*, never defaulted to 0.
    let registry = Registry::builtin().unwrap();
    for family in ["generic", "llama", "qwen"] {
        let resolver = NsirResolver::new(registry.get(family).unwrap());
        for raw in [
            "model.layers.abc.self_attn.q_proj.weight",
            "model.layers.4294967296.self_attn.q_proj.weight",
            "model.layers.-1.self_attn.q_proj.weight",
            "model.layers..self_attn.q_proj.weight",
        ] {
            let got = resolver.resolve_name(raw);
            assert_eq!(got.layer, None, "{family}: `{raw}` invented a layer index");
            assert!(
                !got.resolved,
                "{family}: `{raw}` resolved without a layer index"
            );
            assert_eq!(got.role, TensorRole::Unknown, "{family}: `{raw}` role");
        }
        // The in-range neighbour resolves, so the four refusals are about the
        // index and not about the rest of the name.
        let ok = resolver.resolve_name("model.layers.4294967295.self_attn.q_proj.weight");
        assert_eq!(ok.layer, Some(u32::MAX), "{family}: u32::MAX layer");
        assert!(ok.resolved, "{family}: the in-range neighbour must resolve");
    }
}

// ---------------------------------------------------------------------------
// AC 3 · §Test Cases rows 4 and 5 · ARCHITECTURE.md §4.2 — shape independence
// ---------------------------------------------------------------------------

#[test]
fn two_identically_shaped_tensors_get_different_roles_from_their_names_alone() {
    // §Test Cases row 4: two `[4096, 4096]` tensors named `q_proj` and `o_proj`
    // must get different roles. `resolve_name` takes a `&str`, so a shape is not
    // merely unused here — it is unavailable, which is the strongest available
    // form of ARCHITECTURE.md §4.2's prohibition. `annotate` is used as well,
    // because that is the entry point a shape can even reach.
    let registry = Registry::builtin().unwrap();
    let shape = vec![4096u64, 4096];
    for family in ["generic", "llama", "qwen"] {
        let resolver = NsirResolver::new(registry.get(family).unwrap());
        let mut q = descriptor("model.layers.10.self_attn.q_proj.weight", shape.clone());
        let mut o = descriptor("model.layers.10.self_attn.o_proj.weight", shape.clone());
        assert_eq!(q.shape, o.shape, "the premise of this test");

        let rq = resolver.annotate(&mut q);
        let ro = resolver.annotate(&mut o);
        assert_eq!(rq.role, TensorRole::AttentionQueryProjection, "{family}");
        assert_eq!(ro.role, TensorRole::AttentionOutputProjection, "{family}");
        assert_ne!(rq.role, ro.role, "{family}");
        assert_eq!(
            q.canonical_name, "model.layers[10].self_attention.query_projection.weight",
            "{family}"
        );
        assert_eq!(
            o.canonical_name, "model.layers[10].self_attention.output_projection.weight",
            "{family}"
        );
    }
}

#[test]
fn an_untaught_name_stays_unknown_however_familiar_its_shape() {
    // §Test Cases row 5. The same `[4096, 4096]` that resolved above, under a
    // name no table declares: still `unknown`, and its raw name is still a
    // perfectly good address — just an unresolved one.
    let registry = Registry::builtin().unwrap();
    let shape = vec![4096u64, 4096];
    for family in ["generic", "llama", "qwen"] {
        let resolver = NsirResolver::new(registry.get(family).unwrap());
        for raw in [
            "model.layers.10.self_attn.qkv_proj.weight",
            "model.layers.10.self_attn.q_proj_v2.weight",
            "visual.blocks.3.attn.qkv.weight",
        ] {
            let mut d = descriptor(raw, shape.clone());
            let got = resolver.annotate(&mut d);
            assert!(
                !got.resolved,
                "{family}: `{raw}` resolved on a familiar shape"
            );
            assert_eq!(got.role, TensorRole::Unknown, "{family}: `{raw}`");
            assert_eq!(d.semantic_role, TensorRole::Unknown, "{family}: `{raw}`");
            assert_eq!(d.canonical_name, raw, "{family}: `{raw}` address");
        }
    }
}

#[test]
fn changing_only_the_shape_changes_nothing_about_the_resolution() {
    // The converse of the two tests above, and the sharpest statement of §4.2:
    // hold the name fixed, vary the shape across five ranks including rank 0,
    // and every field of the record — role, component, operation, axes, address
    // — is unmoved.
    let registry = Registry::builtin().unwrap();
    let resolver = NsirResolver::new(registry.get("qwen").unwrap());
    let raw = "model.layers.10.self_attn.q_proj.weight";
    let mut addresses = Vec::new();
    for shape in [
        vec![],
        vec![4096u64],
        vec![4096, 4096],
        vec![32, 128, 128],
        vec![2, 32, 128, 128],
    ] {
        let mut d = descriptor(raw, shape.clone());
        let got = resolver.annotate(&mut d);
        assert!(got.resolved, "shape {shape:?}");
        assert_eq!(
            got.role,
            TensorRole::AttentionQueryProjection,
            "shape {shape:?}"
        );
        assert_eq!(
            got.axes,
            vec!["output_channel", "input_channel"],
            "shape {shape:?}"
        );
        addresses.push(d.canonical_name.clone());
    }
    assert!(
        addresses
            .iter()
            .all(|a| a == "model.layers[10].self_attention.query_projection.weight"),
        "the address moved with the shape: {addresses:?}"
    );
}

#[test]
fn no_corpus_row_declares_more_axes_than_the_implemented_rank_ceiling() {
    // ADR-010 implements rank ≤ 3 and refuses above it. Rank is not expressible
    // in a *name* resolver — nothing here sees a shape — so the nearest
    // expressible surface is the number of axis labels a row declares. A row
    // declaring four would record a rank the rest of the system refuses to
    // render. This is the corpus-side companion to
    // `q_architecture::tests::no_plugin_rule_declares_more_axes_than_the_implemented_rank_ceiling`,
    // which guards the manifests.
    let corpus = corpus();
    let mut checked = 0usize;
    for family in family_names(&corpus) {
        for row in rows_of(&corpus, &family) {
            if let Some(axes) = &row.axes {
                assert!(
                    axes.len() <= 3,
                    "{family}: `{}` declares {} axes; ADR-010 implements rank <= 3",
                    row.raw,
                    axes.len()
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 54, "only {checked} rows declared axes at all");
}

// ---------------------------------------------------------------------------
// AC 4 · AC 5 · NSIR-008 — registry selection, priority, generic fallback
// ---------------------------------------------------------------------------

#[test]
fn a_named_architecture_outranks_the_generic_fallback_and_is_selected_for_every_key_it_declares() {
    // Acceptance criterion 5, first half. Every declared `model_type` and every
    // declared `architectures` string of every implemented plugin is asserted
    // individually rather than sampled.
    let registry = Registry::builtin().unwrap();
    let generic_priority = registry.get("generic").unwrap().plugin.priority;
    let mut keys_checked = 0usize;

    for plugin in registry.plugins() {
        if !plugin.is_implemented() || plugin.id() == "generic" {
            continue;
        }
        assert!(
            plugin.plugin.priority > generic_priority,
            "`{}` does not outrank the generic fallback",
            plugin.id()
        );
        for model_type in &plugin.match_spec.model_types {
            let sel = registry.select(Some(model_type), None).unwrap();
            assert_eq!(sel.id(), plugin.id(), "model_type `{model_type}`");
            assert!(sel.matched, "model_type `{model_type}`");
            keys_checked += 1;
        }
        for architecture in &plugin.match_spec.architectures {
            let sel = registry.select(None, Some(architecture)).unwrap();
            assert_eq!(sel.id(), plugin.id(), "architecture `{architecture}`");
            assert!(sel.matched, "architecture `{architecture}`");
            keys_checked += 1;
        }
    }
    assert!(
        keys_checked >= 9,
        "only {keys_checked} selection keys were checked"
    );
}

#[test]
fn no_two_plugins_claim_the_same_model_type_or_architecture_so_priority_never_breaks_a_tie() {
    // Acceptance criterion 5's premise. `Registry::select` keeps the first
    // highest-priority claimant, so a tie would make selection depend on
    // manifest load order. Today no tie exists; this says so, and fails if one
    // is ever introduced.
    let registry = Registry::builtin().unwrap();
    for (i, a) in registry.plugins().iter().enumerate() {
        for b in registry.plugins().iter().skip(i + 1) {
            for key in &a.match_spec.model_types {
                assert!(
                    !b.match_spec.model_types.contains(key),
                    "`{}` and `{}` both claim model_type `{key}`",
                    a.id(),
                    b.id()
                );
            }
            for key in &a.match_spec.architectures {
                assert!(
                    !b.match_spec.architectures.contains(key),
                    "`{}` and `{}` both claim architecture `{key}`",
                    a.id(),
                    b.id()
                );
            }
        }
    }
}

#[test]
fn an_unclaimed_model_falls_back_to_generic_which_resolves_only_what_it_was_taught() {
    // Acceptance criterion 5, second half. The fallback is a correct outcome,
    // not a failure — and it is a real fallback, not a silent success: the
    // generic table resolves the universal convention and answers `unknown` for
    // the family-specific names, in the same run.
    let registry = Registry::builtin().unwrap();
    let sel = registry
        .select(Some("some_new_family"), Some("XForCausalLM"))
        .unwrap();
    assert_eq!(sel.id(), "generic");
    assert!(
        !sel.matched,
        "the fallback must report that nothing claimed the model"
    );

    let model = ResolvedModel::build(
        &registry,
        Some("some_new_family"),
        Some("XForCausalLM"),
        vec![
            descriptor("model.layers.4.mlp.down_proj.weight", vec![48, 64]),
            descriptor(
                "model.layers.4.mlp.experts.0.down_proj.weight",
                vec![48, 64],
            ),
            descriptor("model.layers.4.self_attn.q_norm.weight", vec![16]),
            descriptor("visual.blocks.3.attn.qkv.weight", vec![4, 4]),
        ],
    )
    .unwrap();
    assert_eq!(model.resolver_id, "generic");
    assert_eq!(
        model
            .by_raw_name("model.layers.4.mlp.down_proj.weight")
            .unwrap()
            .canonical_name,
        "model.layers[4].mlp.down_projection.weight"
    );
    // Three names the generic table was not taught: unresolved, each keeping
    // its raw name as its address.
    assert_eq!(model.unresolved_count(), 3);
    for raw in [
        "model.layers.4.mlp.experts.0.down_proj.weight",
        "model.layers.4.self_attn.q_norm.weight",
        "visual.blocks.3.attn.qkv.weight",
    ] {
        let d = model.by_raw_name(raw).unwrap();
        assert_eq!(d.semantic_role, TensorRole::Unknown, "{raw}");
        assert_eq!(d.canonical_name, raw, "{raw}");
    }
}

#[test]
fn an_unimplemented_family_never_claims_a_model_and_the_generic_fallback_answers_instead() {
    // Acceptance criterion 4. `kimi` and `deepseek` are declared so the gap is
    // visible rather than merely missing, and they claim nothing.
    //
    // THE ONE VACUITY IN THIS SUITE, STATED PLAINLY: the `kimi` and `deepseek`
    // corpus rows would be satisfied by an empty rule table — because that is
    // exactly what these plugins have, and asserting it is the point. They
    // therefore carry no control. What makes the pair non-vacuous is the second
    // half below: the same raw name that `kimi` refuses is shown resolving
    // through the `generic` plugin the registry selects in its place, so the
    // corpus rows cannot be passing merely because resolution is broken.
    let registry = Registry::builtin().unwrap();
    let mut unimplemented = registry.declared_but_unimplemented();
    unimplemented.sort();
    assert_eq!(unimplemented, vec!["deepseek", "kimi"]);

    let corpus = corpus();
    for family in &unimplemented {
        let plugin = registry.get(family).unwrap();
        assert!(!plugin.is_implemented(), "{family}");
        assert!(plugin.rules.is_empty(), "{family} has a rule table");
        assert!(
            !plugin.match_spec.model_types.is_empty(),
            "{family} declares nothing, so the gap is invisible"
        );
        let rows = rows_of(&corpus, family);
        assert!(
            rows.iter().all(|r| !r.is_positive()),
            "{family} is unimplemented; no corpus row may expect it to resolve"
        );
        assert!(conformance_failures(plugin, &rows).is_empty(), "{family}");

        for model_type in &plugin.match_spec.model_types {
            assert!(
                !plugin.claims(Some(model_type), None),
                "{family} claimed {model_type}"
            );
            let sel = registry.select(Some(model_type), None).unwrap();
            assert_eq!(sel.id(), "generic", "{family}/{model_type}");
            assert!(!sel.matched, "{family}/{model_type}");
        }
        for architecture in &plugin.match_spec.architectures {
            assert!(!plugin.claims(None, Some(architecture)), "{family}");
            assert_eq!(
                registry.select(None, Some(architecture)).unwrap().id(),
                "generic",
                "{family}/{architecture}"
            );
        }
    }

    // The non-vacuity half: a name `kimi` and `deepseek` answer `unknown` is
    // resolved by the plugin the registry actually selects for their models.
    let raw = "model.layers.10.self_attn.q_proj.weight";
    for family in &unimplemented {
        assert!(
            !NsirResolver::new(registry.get(family).unwrap())
                .resolve_name(raw)
                .resolved
        );
    }
    let model = ResolvedModel::build(
        &registry,
        Some("kimi"),
        Some("KimiForCausalLM"),
        vec![descriptor(raw, vec![4096, 4096])],
    )
    .unwrap();
    assert_eq!(model.resolver_id, "generic");
    assert_eq!(
        model.by_raw_name(raw).unwrap().canonical_name,
        "model.layers[10].self_attention.query_projection.weight"
    );
}

// ---------------------------------------------------------------------------
// Cross-family agreement — the canonical address is the universal join key
// ---------------------------------------------------------------------------

#[test]
fn the_llama_and_qwen_corpora_agree_on_every_raw_name_they_share() {
    // A canonical address must not depend on which family produced the tensor
    // (.plan/DATA_ARCHITECTURE.md §4). Asserted first between the two corpora —
    // catching a transcription slip in this file — and then between the two
    // resolvers, catching a divergence between the two manifests.
    let registry = Registry::builtin().unwrap();
    let corpus = corpus();
    let llama_rows = rows_of(&corpus, "llama");
    let qwen_rows = rows_of(&corpus, "qwen");
    let llama = NsirResolver::new(registry.get("llama").unwrap());
    let qwen = NsirResolver::new(registry.get("qwen").unwrap());
    let mut shared = 0usize;

    for l in &llama_rows {
        let Some(q) = qwen_rows.iter().find(|q| q.raw == l.raw) else {
            continue;
        };
        assert_eq!(
            l.canonical, q.canonical,
            "corpus rows disagree on `{}`",
            l.raw
        );
        assert_eq!(l.role, q.role, "corpus rows disagree on `{}` role", l.raw);
        assert_eq!(l.axes, q.axes, "corpus rows disagree on `{}` axes", l.raw);

        let rl = llama.resolve_name(&l.raw);
        let rq = qwen.resolve_name(&l.raw);
        assert_eq!(
            canonical_name(&rl),
            canonical_name(&rq),
            "`{}` address",
            l.raw
        );
        assert_eq!(rl.role, rq.role, "`{}` role", l.raw);
        assert_eq!(rl.component, rq.component, "`{}` component", l.raw);
        assert_eq!(rl.axes, rq.axes, "`{}` axes", l.raw);
        assert_eq!(rl.layer, rq.layer, "`{}` layer", l.raw);
        assert_eq!(rl.expert, rq.expert, "`{}` expert", l.raw);
        shared += 1;
    }
    assert!(
        shared >= 40,
        "only {shared} raw names are shared between the llama and qwen corpora"
    );
}

#[test]
fn the_corpus_agrees_with_the_checked_in_qwen_fixture_golden_on_every_shared_raw_name() {
    // `fixtures/tiny-qwen-single/golden.json` is the checked-in golden QM-0010
    // generated, hand-written from the same §6.1 rule. Two independently typed
    // records of the same expectation: if either drifts, this names the row.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/tiny-qwen-single/golden.json");
    let text = std::fs::read_to_string(&path).expect("run fixtures/generate_fixtures.py");
    let golden: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(golden["resolver_id"], serde_json::json!("qwen"));

    let corpus = corpus();
    let rows = rows_of(&corpus, "qwen");
    let mut compared = 0usize;
    let mut absent: Vec<&str> = Vec::new();

    for g in golden["resolved"].as_array().unwrap() {
        let raw = g["raw_name"].as_str().unwrap();
        let Some(row) = rows.iter().find(|r| r.raw == raw) else {
            // Collected and named below, never skipped: a cross-check that
            // quietly covers less than it claims is worse than one that fails.
            absent.push(raw);
            continue;
        };
        assert_eq!(
            row.canonical.as_deref(),
            g["canonical_name"].as_str(),
            "`{raw}` canonical address"
        );
        assert_eq!(row.role.as_deref(), g["role"].as_str(), "`{raw}` role");
        assert_eq!(
            row.component.as_deref(),
            g["component"].as_str(),
            "`{raw}` component"
        );
        assert_eq!(
            row.operation.as_deref(),
            g["operation"].as_str(),
            "`{raw}` operation"
        );
        assert_eq!(
            serde_json::to_value(&row.axes).unwrap(),
            g["axes"],
            "`{raw}` axes"
        );
        compared += 1;
    }
    assert!(
        compared >= 21,
        "only {compared} of the golden's rows appear in the corpus"
    );

    // The golden's untaught names must be negative rows here too. A name one
    // file refuses and the other resolves would be a silent contradiction —
    // and a name the golden lists that the corpus simply does not carry is a
    // gap in this cross-check, so it is NAMED rather than skipped.
    let mut untaught_compared = 0usize;
    for u in golden["untaught"].as_array().unwrap() {
        let raw = u["raw_name"].as_str().unwrap();
        match rows.iter().find(|r| r.raw == raw) {
            Some(row) => {
                assert!(
                    !row.is_positive(),
                    "the fixture calls `{raw}` untaught and the corpus expects it to resolve"
                );
                untaught_compared += 1;
            }
            None => absent.push(raw),
        }
    }

    // Both directions, and neither of them partial. Every row of the golden —
    // 21 resolved and 6 untaught — has a counterpart here, or this names the
    // ones that do not.
    assert!(
        absent.is_empty(),
        "the golden lists {absent:?}, which the qwen corpus does not carry, so those \
         rows were not cross-checked"
    );
    assert_eq!(
        untaught_compared,
        golden["untaught"].as_array().unwrap().len(),
        "not every untaught row of the golden was cross-checked"
    );
    assert!(
        untaught_compared >= 6,
        "only {untaught_compared} untaught rows"
    );
}

// ---------------------------------------------------------------------------
// NSIR-007 — an ambiguous alias returns candidates, never a single pick
// ---------------------------------------------------------------------------

/// A model built from a family's own positive corpus rows. Shapes are arbitrary
/// and are never read by the resolver; they exist because `TensorDescriptor` has
/// the field.
fn model_from_corpus(registry: &Registry, family: &str, model_type: &str) -> ResolvedModel {
    let rows = rows_of(&corpus(), family);
    let descriptors = rows
        .iter()
        .filter(|r| r.is_positive())
        .map(|r| descriptor(&r.raw, vec![16, 48]))
        .collect();
    let model = ResolvedModel::build(registry, Some(model_type), None, descriptors).unwrap();
    assert_eq!(model.resolver_id, family, "the wrong plugin was selected");
    model
}

#[test]
fn an_ambiguous_alias_returns_candidates_rather_than_one_silent_pick_in_every_family() {
    // `NSIR-007` and ARCHITECTURE.md §6.2: if `Att` could refer to Q, K, V or O,
    // the query returns a candidate list rather than choosing.
    //
    // Over this corpus `Att[10]` has **seven** candidates, not four, because the
    // corpus carries the declared q/k/v projection *biases* alongside their
    // weights and an alias names a *role* — a role that covers more than one
    // tensor. That is asserted rather than avoided: it is the same rule doing
    // more work, and a suite that quietly picked a model without biases would be
    // testing a weaker claim than the one that matters.
    let registry = Registry::builtin().unwrap();
    for (family, model_type) in [("llama", "llama"), ("qwen", "qwen3")] {
        let model = model_from_corpus(&registry, family, model_type);
        let att = model.resolve_alias("Att[10]").unwrap();
        assert!(att.is_ambiguous(), "{family}: `Att[10]` was not ambiguous");
        assert_eq!(att.candidates.len(), 7, "{family}");
        assert_eq!(att.confidence, 1.0 / 7.0, "{family}");
        // Candidate order follows the manifest's declared role order — Q, K, V,
        // O — which is the order a user sees in the ambiguity message.
        let addresses: Vec<&str> = att
            .candidates
            .iter()
            .map(|c| c.canonical_name.as_str())
            .collect();
        assert_eq!(
            addresses,
            vec![
                "model.layers[10].self_attention.query_projection.weight",
                "model.layers[10].self_attention.query_projection.bias",
                "model.layers[10].self_attention.key_projection.weight",
                "model.layers[10].self_attention.key_projection.bias",
                "model.layers[10].self_attention.value_projection.weight",
                "model.layers[10].self_attention.value_projection.bias",
                "model.layers[10].self_attention.output_projection.weight",
            ],
            "{family}"
        );
        match att.unique() {
            Err(QError::AmbiguousAlias { candidates, .. }) => {
                assert_eq!(candidates.len(), 7, "{family}");
                assert!(
                    candidates
                        .iter()
                        .any(|c| c == "model.layers[10].self_attention.query_projection.weight"),
                    "{family}: the ambiguity message must name the candidates"
                );
            }
            other => panic!("{family}: expected AmbiguousAlias, got {other:?}"),
        }

        // Even the *unambiguous-looking* `Q` is ambiguous here, and the resolver
        // says so instead of assuming a user who types `Q` means the weight.
        let q = model.resolve_alias("Q[10]").unwrap();
        assert!(
            q.is_ambiguous(),
            "{family}: `Q[10]` collapsed to one tensor"
        );
        assert_eq!(q.candidates.len(), 2, "{family}");
        assert!(
            matches!(q.unique(), Err(QError::AmbiguousAlias { .. })),
            "{family}"
        );

        // An alias that really does name one tensor resolves to one, so the
        // ambiguity above is a property of the alias and the model rather than a
        // resolver that never commits.
        let down = model.resolve_alias("MLP.down[10]").unwrap();
        assert!(!down.is_ambiguous(), "{family}");
        assert_eq!(down.confidence, 1.0, "{family}");
        assert_eq!(
            down.unique().unwrap().canonical_name,
            "model.layers[10].mlp.down_projection.weight",
            "{family}"
        );
        let expert = model.resolve_alias("Expert[10,37].up").unwrap();
        assert_eq!(
            expert.unique().unwrap().canonical_name,
            "model.layers[10].moe.experts[37].up_projection.weight",
            "{family}"
        );

        // An alias for a layer this model does not carry matches nothing, and
        // says so with confidence 0.0 rather than returning the nearest layer.
        let missing = model.resolve_alias("Q[99]").unwrap();
        assert!(missing.candidates.is_empty(), "{family}");
        assert_eq!(missing.confidence, 0.0, "{family}");
        assert!(
            matches!(missing.unique(), Err(QError::NotFound(_))),
            "{family}"
        );

        // An alias no manifest declares is rejected with an explanation, not
        // answered with the nearest match.
        let err = model.resolve_alias("Zzz[10]").unwrap_err();
        assert!(err.to_string().contains("unknown alias"), "{family}: {err}");
    }

    // `QKNorm` is Qwen's own ambiguous alias, covering the two per-head norms
    // Llama checkpoints do not carry. Also ambiguous, also candidates.
    let qwen = model_from_corpus(&registry, "qwen", "qwen3");
    let norms = qwen.resolve_alias("QKNorm[10]").unwrap();
    assert!(norms.is_ambiguous());
    assert_eq!(norms.candidates.len(), 2);
    assert!(matches!(norms.unique(), Err(QError::AmbiguousAlias { .. })));
    let router = qwen.resolve_alias("Router[5]").unwrap();
    assert_eq!(
        router.unique().unwrap().canonical_name,
        "model.layers[5].router.expert_routing.weight"
    );
    // Llama's manifest declares no `Router` alias, so the same input is refused
    // there rather than answered from another family's vocabulary.
    let llama = model_from_corpus(&registry, "llama", "llama");
    assert!(llama
        .resolve_alias("Router[5]")
        .unwrap_err()
        .to_string()
        .contains("unknown alias"));
}

#[test]
fn a_family_that_declares_no_aliases_rejects_every_alias_with_an_explanation() {
    // The generic plugin declares no aliases at all. That is not a gap to paper
    // over with a best-effort match: an alias it does not declare is refused,
    // and the refusal says which plugin resolved the model.
    let registry = Registry::builtin().unwrap();
    assert!(registry.get("generic").unwrap().aliases.is_empty());
    let model = ResolvedModel::build(
        &registry,
        Some("some_new_family"),
        None,
        vec![descriptor(
            "model.layers.4.self_attn.q_proj.weight",
            vec![16, 48],
        )],
    )
    .unwrap();
    assert_eq!(model.resolver_id, "generic");
    let err = model.resolve_alias("Q[4]").unwrap_err();
    let message = err.to_string();
    assert!(message.contains("unknown alias"), "{message}");
    assert!(message.contains("generic"), "{message}");
}
