//! `REP-001` — the manifest schema is the contract; these tests are what stops
//! the Rust types drifting from it.
//!
//! ## Why the validator is in here rather than in `Cargo.toml`
//!
//! Nothing in the workspace validates JSON Schema today, the four schemas
//! already in `schemas/` are asserted by nobody, and no JSON Schema validator
//! is installed in the system Python. Pulling a validator crate in for one test
//! would add a dependency tree larger than this crate to a task whose declared
//! `Cargo.toml` scope is "workspace member".
//!
//! So [`draft07`] implements the subset of draft-07 the manifest schema
//! actually uses — and **refuses any schema containing a keyword it does not
//! assert**. That last rule is the whole point: the schema cannot quietly grow
//! a constraint that nothing checks, because
//! `the_schema_uses_only_keywords_this_validator_asserts` turns red the moment
//! it does.
//!
//! An external validator is the belt to those braces: `jsonschema` 4.26.0 in a
//! throwaway virtualenv validated both goldens and the schema's own example
//! against `manifest.v1.json`, and confirmed 24 negative paths, on 2026-08-05
//! (`.plan/evidence/QM-0140.md` `## Validation evidence`). That is recorded as
//! evidence rather than wired into the build — `TEST_STRATEGY.md` §1 keeps
//! artifact validation a CI job, and no `cargo test` may depend on a
//! `pip install`.

use std::fs;
use std::path::{Path, PathBuf};

use q_report::{
    Backend, ErrorAggregate, Fidelity, Frontier, FrontierMethod, FrontierStep, Granularity,
    GranularityKind, LayerEntry, Manifest, Model, Precision, Projection, QuantConfigRecord,
    RankingEntry, Refusal, ResolverConfidence, RoundMode, Run, TensorEntry, ZeroPoint,
    FRONTIER_CLAIM, MANIFEST_SCHEMA_ID, MANIFEST_SCHEMA_PATH, MANIFEST_VERSION,
};
use q_source::{DType, TensorRole};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// A validator for the draft-07 subset this schema uses
// ---------------------------------------------------------------------------

mod draft07 {
    use serde_json::Value;

    /// Keywords this validator turns into assertions.
    const ASSERTED: &[&str] = &[
        "$ref",
        "additionalProperties",
        "allOf",
        "const",
        "else",
        "enum",
        "exclusiveMinimum",
        "format",
        "if",
        "items",
        "maxItems",
        "maximum",
        "minItems",
        "minLength",
        "minimum",
        "not",
        "properties",
        "required",
        "then",
        "type",
    ];

    /// Keywords that carry documentation rather than a constraint.
    const ANNOTATIONS: &[&str] = &[
        "$comment",
        "$id",
        "$schema",
        "default",
        "definitions",
        "description",
        "examples",
        "title",
    ];

    /// The only `format` this validator implements. Draft-07 permits treating
    /// `format` as an annotation; asserting it is the stricter reading, and a
    /// schema naming any other format is refused rather than ignored.
    const SUPPORTED_FORMAT: &str = "date-time";

    #[derive(Debug)]
    pub struct Validator {
        root: Value,
    }

    impl Validator {
        /// Refuses a schema containing a keyword this validator cannot assert.
        pub fn new(root: Value) -> Result<Validator, String> {
            let v = Validator { root };
            v.audit(&v.root, "#")?;
            Ok(v)
        }

        fn audit(&self, schema: &Value, at: &str) -> Result<(), String> {
            let object = schema
                .as_object()
                .ok_or_else(|| format!("{at}: a schema must be an object"))?;

            for key in object.keys() {
                if !ASSERTED.contains(&key.as_str()) && !ANNOTATIONS.contains(&key.as_str()) {
                    return Err(format!(
                        "{at}: keyword {key:?} is not one this validator asserts; \
                         implement it here before using it in the schema"
                    ));
                }
            }

            if let Some(reference) = object.get("$ref") {
                let name = reference
                    .as_str()
                    .and_then(|r| r.strip_prefix("#/definitions/"))
                    .ok_or_else(|| {
                        format!("{at}: only `#/definitions/NAME` references are supported")
                    })?;
                if self.root.pointer(&format!("/definitions/{name}")).is_none() {
                    return Err(format!("{at}: `$ref` target {name:?} does not exist"));
                }
                let siblings: Vec<&String> = object
                    .keys()
                    .filter(|k| k.as_str() != "$ref" && ASSERTED.contains(&k.as_str()))
                    .collect();
                if !siblings.is_empty() {
                    return Err(format!(
                        "{at}: draft-07 ignores keywords beside `$ref`; found {siblings:?}"
                    ));
                }
            }

            if let Some(format) = object.get("format") {
                if format.as_str() != Some(SUPPORTED_FORMAT) {
                    return Err(format!("{at}: unsupported `format` {format}"));
                }
            }

            if let Some(kind) = object.get("type") {
                let names: Vec<&str> = match kind {
                    Value::String(s) => vec![s.as_str()],
                    Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                    _ => return Err(format!("{at}: `type` must be a string or an array")),
                };
                for name in names {
                    if !matches!(
                        name,
                        "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
                    ) {
                        return Err(format!("{at}: unknown type {name:?}"));
                    }
                }
            }

            if let Some(extra) = object.get("additionalProperties") {
                if extra.as_bool() != Some(false) {
                    return Err(format!(
                        "{at}: only `additionalProperties: false` is supported"
                    ));
                }
            }

            for (key, child) in [
                ("items", object.get("items")),
                ("if", object.get("if")),
                ("then", object.get("then")),
                ("else", object.get("else")),
                ("not", object.get("not")),
            ] {
                if let Some(child) = child {
                    self.audit(child, &format!("{at}/{key}"))?;
                }
            }

            for key in ["properties", "definitions"] {
                if let Some(map) = object.get(key).and_then(Value::as_object) {
                    for (name, child) in map {
                        self.audit(child, &format!("{at}/{key}/{name}"))?;
                    }
                }
            }

            if let Some(all) = object.get("allOf").and_then(Value::as_array) {
                for (i, child) in all.iter().enumerate() {
                    self.audit(child, &format!("{at}/allOf/{i}"))?;
                }
            }

            Ok(())
        }

        /// `Ok(())` or every violation found, each carrying a JSON pointer.
        pub fn validate(&self, instance: &Value) -> Result<(), Vec<String>> {
            let mut errors = Vec::new();
            self.check(&self.root, instance, "", &mut errors);
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        }

        fn passes(&self, schema: &Value, instance: &Value) -> bool {
            let mut errors = Vec::new();
            self.check(schema, instance, "", &mut errors);
            errors.is_empty()
        }

        fn check(&self, schema: &Value, instance: &Value, at: &str, out: &mut Vec<String>) {
            let object = match schema.as_object() {
                Some(o) => o,
                None => return,
            };

            if let Some(name) = object
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix("#/definitions/"))
            {
                let target = self.root.pointer(&format!("/definitions/{name}")).unwrap();
                self.check(target, instance, at, out);
                return;
            }

            if let Some(kind) = object.get("type") {
                let names: Vec<&str> = match kind {
                    Value::String(s) => vec![s.as_str()],
                    Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                    _ => vec![],
                };
                if !names.iter().any(|n| type_matches(n, instance)) {
                    out.push(format!(
                        "{at}: expected type {names:?}, found {}",
                        describe(instance)
                    ));
                    return;
                }
            }

            if let Some(allowed) = object.get("enum").and_then(Value::as_array) {
                if !allowed.contains(instance) {
                    out.push(format!(
                        "{at}: {instance} is not one of {}",
                        Value::Array(allowed.clone())
                    ));
                }
            }

            if let Some(expected) = object.get("const") {
                if expected != instance {
                    out.push(format!(
                        "{at}: expected the constant {expected}, found {instance}"
                    ));
                }
            }

            if let Some(text) = instance.as_str() {
                if let Some(min) = object.get("minLength").and_then(Value::as_u64) {
                    if (text.chars().count() as u64) < min {
                        out.push(format!("{at}: {text:?} is shorter than minLength {min}"));
                    }
                }
                if object.get("format").and_then(Value::as_str) == Some(SUPPORTED_FORMAT)
                    && !is_rfc3339_date_time(text)
                {
                    out.push(format!("{at}: {text:?} is not an RFC 3339 date-time"));
                }
            }

            if let Some(number) = instance.as_f64() {
                if let Some(min) = object.get("minimum").and_then(Value::as_f64) {
                    if number < min {
                        out.push(format!("{at}: {number} is below minimum {min}"));
                    }
                }
                if let Some(max) = object.get("maximum").and_then(Value::as_f64) {
                    if number > max {
                        out.push(format!("{at}: {number} is above maximum {max}"));
                    }
                }
                if let Some(min) = object.get("exclusiveMinimum").and_then(Value::as_f64) {
                    if number <= min {
                        out.push(format!(
                            "{at}: {number} is not above exclusiveMinimum {min}"
                        ));
                    }
                }
            }

            if let Some(items) = instance.as_array() {
                if let Some(min) = object.get("minItems").and_then(Value::as_u64) {
                    if (items.len() as u64) < min {
                        out.push(format!(
                            "{at}: {} items is below minItems {min}",
                            items.len()
                        ));
                    }
                }
                if let Some(max) = object.get("maxItems").and_then(Value::as_u64) {
                    if (items.len() as u64) > max {
                        out.push(format!(
                            "{at}: {} items is above maxItems {max}",
                            items.len()
                        ));
                    }
                }
                if let Some(item_schema) = object.get("items") {
                    for (i, item) in items.iter().enumerate() {
                        self.check(item_schema, item, &format!("{at}/{i}"), out);
                    }
                }
            }

            if let Some(members) = instance.as_object() {
                if let Some(required) = object.get("required").and_then(Value::as_array) {
                    for name in required.iter().filter_map(Value::as_str) {
                        if !members.contains_key(name) {
                            out.push(format!("{at}: required member {name:?} is missing"));
                        }
                    }
                }
                let properties = object.get("properties").and_then(Value::as_object);
                if let Some(properties) = properties {
                    for (name, child) in members {
                        if let Some(child_schema) = properties.get(name) {
                            self.check(child_schema, child, &format!("{at}/{name}"), out);
                        }
                    }
                }
                if object.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
                    for name in members.keys() {
                        if !properties.is_some_and(|p| p.contains_key(name)) {
                            out.push(format!("{at}: member {name:?} is not declared in v1"));
                        }
                    }
                }
            }

            if let Some(all) = object.get("allOf").and_then(Value::as_array) {
                for (i, child) in all.iter().enumerate() {
                    self.check(child, instance, &format!("{at}/allOf/{i}"), out);
                }
            }

            if let Some(condition) = object.get("if") {
                let branch = if self.passes(condition, instance) {
                    ("then", object.get("then"))
                } else {
                    ("else", object.get("else"))
                };
                if let (name, Some(child)) = branch {
                    self.check(child, instance, &format!("{at}/{name}"), out);
                }
            }

            if let Some(child) = object.get("not") {
                if self.passes(child, instance) {
                    out.push(format!("{at}: matched a schema it must not match"));
                }
            }
        }
    }

    fn type_matches(name: &str, value: &Value) -> bool {
        match name {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            "number" => value.is_number(),
            "integer" => {
                value.is_i64() || value.is_u64() || value.as_f64().is_some_and(|f| f.fract() == 0.0)
            }
            _ => false,
        }
    }

    fn describe(value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    /// RFC 3339 shape, mirroring the check the Rust types apply.
    fn is_rfc3339_date_time(s: &str) -> bool {
        let b = s.as_bytes();
        if b.len() < 20 {
            return false;
        }
        let digits = |mut range: std::ops::Range<usize>| range.all(|i| b[i].is_ascii_digit());
        if !(digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10)) {
            return false;
        }
        if b[10] != b'T' {
            return false;
        }
        if !(digits(11..13) && b[13] == b':' && digits(14..16) && b[16] == b':' && digits(17..19)) {
            return false;
        }
        let mut i = 19;
        if b[i] == b'.' {
            i += 1;
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i == start {
                return false;
            }
        }
        match b.get(i) {
            Some(b'Z') => i + 1 == b.len(),
            Some(b'+') | Some(b'-') => {
                i + 6 == b.len()
                    && b[i + 1].is_ascii_digit()
                    && b[i + 2].is_ascii_digit()
                    && b[i + 3] == b':'
                    && b[i + 4].is_ascii_digit()
                    && b[i + 5].is_ascii_digit()
            }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    crate_dir().join("..").join("..")
}

fn schema_json() -> Value {
    let path = repo_root().join(MANIFEST_SCHEMA_PATH);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read the schema at {}: {e}", path.display()));
    serde_json::from_str(&text).expect("the schema is not valid JSON")
}

fn validator() -> draft07::Validator {
    draft07::Validator::new(schema_json()).expect("the schema uses only supported keywords")
}

fn golden(name: &str) -> String {
    let path: PathBuf = Path::new(&crate_dir())
        .join("tests")
        .join("golden")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn golden_value() -> Value {
    serde_json::from_str(&golden("manifest.v1.json"))
        .expect("the golden manifest is not valid JSON")
}

fn assert_schema_rejects(instance: &Value, needle: &str) {
    match validator().validate(instance) {
        Ok(()) => panic!("expected the schema to reject this document over {needle:?}"),
        Err(errors) => assert!(
            errors.iter().any(|e| e.contains(needle)),
            "expected an error mentioning {needle:?}, got {errors:#?}"
        ),
    }
}

/// A manifest built from the Rust types, not read from a file — so that
/// "a produced manifest validates" is about the producer, not the fixture.
fn a_produced_manifest() -> Manifest {
    let aggregate = ErrorAggregate {
        count: 4096,
        sum_sq_base: 512.0,
        sum_sq_delta: 32.0,
        sum_abs_delta: 128.0,
        max_abs_delta: 0.25,
        bytes_at_base_precision: 16384,
        bytes_at_target_precision: 2048,
    };
    Manifest {
        manifest_version: MANIFEST_VERSION,
        projection: Projection::Full,
        run: Run {
            run_id: "0c1d2e3f4a5b60710c1d2e3f4a5b6071".into(),
            engine_version: "0.1.0".into(),
            backend: Backend::Cpu,
            started_at: "2026-08-04T09:15:00Z".into(),
            elapsed_seconds: 0.25,
            peak_resident_bytes: 2_883_584,
            bytes_read: 32768,
        },
        model: Model {
            model_id: "distilbert-distilgpt2".into(),
            source_uri: "file:///models/distilbert-distilgpt2".into(),
            revision_hash: "0000000000000000000000000000000000000001".into(),
            checkpoint_bytes: 16384,
            parameter_count: 4096,
            architecture: "generic".into(),
            resolver_confidence: ResolverConfidence::Unknown,
        },
        config: QuantConfigRecord {
            precision: Precision::Int8,
            granularity: Granularity {
                kind: GranularityKind::PerOutputChannel,
                group_size: None,
            },
            zero_point: ZeroPoint::Symmetric,
            round: RoundMode::NearestEven,
            block_rows: 256,
            block_columns: 256,
            resident_ceiling_bytes: 2_147_483_648,
        },
        totals: aggregate,
        layers: vec![LayerEntry {
            layer_index: 0,
            aggregate,
        }],
        experts: vec![],
        tensors: Some(vec![TensorEntry {
            address: "model.layers[0].mlp.down_projection.weight".into(),
            role: TensorRole::Unknown,
            dtype: DType::F32,
            shape: vec![64, 64],
            aggregate,
            outlier_attribution: None,
        }]),
        ranking: vec![RankingEntry {
            address: "model.layers[0].mlp.down_projection.weight".into(),
            relative_error: 0.25,
            parameter_count: 4096,
        }],
        frontier: Frontier {
            method: FrontierMethod::GreedyErrorPerByte,
            claim: FRONTIER_CLAIM.into(),
            steps: vec![FrontierStep {
                keep_set: vec!["model.layers[0].mlp.down_projection.weight".into()],
                added_bytes: 14336,
                error_removed_fraction: 1.0,
            }],
        },
        fidelity: Fidelity::Exact,
        refusals: vec![Refusal {
            requirement_id: "EVAL-001".into(),
            what: "accuracy estimate".into(),
            why: "Weight-space error only. Accuracy impact is not measured.".into(),
        }],
        extensions: Default::default(),
    }
}

// ---------------------------------------------------------------------------
// The schema as an artifact
// ---------------------------------------------------------------------------

#[test]
fn the_schema_declares_draft_07_like_the_four_schemas_beside_it() {
    assert_eq!(
        schema_json()["$schema"],
        json!("http://json-schema.org/draft-07/schema#")
    );
}

#[test]
fn the_schema_id_carries_an_explicit_version_and_matches_the_crate_constant() {
    let id = schema_json()["$id"].as_str().unwrap().to_string();
    assert_eq!(id, MANIFEST_SCHEMA_ID);
    assert!(id.ends_with("/v1"), "SCHEMA-001: {id}");
}

#[test]
fn the_schema_uses_only_keywords_this_validator_asserts() {
    draft07::Validator::new(schema_json()).unwrap();
}

#[test]
fn the_validator_refuses_a_schema_whose_keywords_it_cannot_assert() {
    let err = draft07::Validator::new(json!({
        "type": "array",
        "uniqueItems": true
    }))
    .unwrap_err();
    assert!(err.contains("uniqueItems"), "{err}");
}

#[test]
fn every_property_the_schema_declares_carries_a_description() {
    // The manual verification step is "read the schema descriptions as a
    // stranger would; a field nobody can explain does not belong in v1".
    let schema = schema_json();
    let mut undocumented = Vec::new();
    let mut visit = |scope: &str, node: &Value| {
        if let Some(properties) = node.get("properties").and_then(Value::as_object) {
            for (name, child) in properties {
                if child.get("description").and_then(Value::as_str).is_none() {
                    undocumented.push(format!("{scope}/{name}"));
                }
            }
        }
    };
    visit("#", &schema);
    for (name, definition) in schema["definitions"].as_object().unwrap() {
        visit(&format!("#/definitions/{name}"), definition);
    }
    assert!(
        undocumented.is_empty(),
        "undescribed fields: {undocumented:?}"
    );
}

#[test]
fn every_example_in_the_schema_validates_against_it() {
    let schema = schema_json();
    let examples = schema["examples"]
        .as_array()
        .expect("the schema must carry at least one example");
    assert!(!examples.is_empty());
    for (i, example) in examples.iter().enumerate() {
        validator()
            .validate(example)
            .unwrap_or_else(|e| panic!("examples[{i}] does not validate: {e:#?}"));
    }
}

// ---------------------------------------------------------------------------
// Both sides of the contract
// ---------------------------------------------------------------------------

#[test]
fn the_golden_manifest_validates_against_the_schema() {
    validator().validate(&golden_value()).unwrap();
}

#[test]
fn the_golden_summary_validates_against_the_schema() {
    let value: Value = serde_json::from_str(&golden("manifest.v1.summary.json")).unwrap();
    validator().validate(&value).unwrap();
    assert!(
        value.get("tensors").is_none(),
        "the summary must omit `tensors`"
    );
}

#[test]
fn a_produced_manifest_validates_against_the_schema() {
    let json = a_produced_manifest().to_json_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    validator().validate(&value).unwrap();
}

#[test]
fn a_produced_summary_validates_and_omits_the_tensor_array() {
    let json = a_produced_manifest()
        .summary()
        .unwrap()
        .to_json_string()
        .unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    validator().validate(&value).unwrap();
    assert!(value.get("tensors").is_none());
    assert_eq!(value["projection"], json!("summary"));
}

#[test]
fn a_manifest_serialises_validates_deserialises_and_compares_equal_on_both_sides() {
    // Both sides of the contract in one chain, because the three properties are
    // only worth anything together: bytes that validate but parse back to a
    // different value, or a value that survives the round trip while producing
    // a document no consumer would accept, are each a silent contract breach.
    for original in [
        a_produced_manifest(),
        a_produced_manifest().summary().unwrap(),
    ] {
        // serialize
        let written = original.to_json_string().unwrap();

        // validate — against the schema file, not against the Rust types
        let as_value: Value = serde_json::from_str(&written).unwrap();
        validator()
            .validate(&as_value)
            .unwrap_or_else(|e| panic!("a produced manifest must validate: {e:#?}"));

        // deserialize
        let read_back = Manifest::from_json_str(&written).unwrap();

        // compare, as a value and as bytes
        assert_eq!(read_back, original.canonical().unwrap());
        let rewritten = read_back.to_json_string().unwrap();
        assert_eq!(rewritten, written);

        // and the document that came back out still validates
        let reparsed: Value = serde_json::from_str(&rewritten).unwrap();
        validator().validate(&reparsed).unwrap();
    }
}

#[test]
fn the_golden_manifest_round_trips_byte_identically() {
    let text = golden("manifest.v1.json");
    let parsed = Manifest::from_json_str(&text).unwrap();
    assert_eq!(parsed.to_json_string().unwrap(), text);
}

#[test]
fn the_golden_summary_round_trips_byte_identically() {
    let text = golden("manifest.v1.summary.json");
    let parsed = Manifest::from_json_str(&text).unwrap();
    assert_eq!(parsed.to_json_string().unwrap(), text);
}

#[test]
fn the_summary_projection_of_the_golden_manifest_is_the_golden_summary() {
    let full = Manifest::from_json_str(&golden("manifest.v1.json")).unwrap();
    assert_eq!(
        full.summary().unwrap().to_json_string().unwrap(),
        golden("manifest.v1.summary.json")
    );
}

#[test]
fn the_data_contract_fields_are_all_present_in_a_real_manifest() {
    // The rows QM-0140 calls contract rather than convenience.
    let m = golden_value();
    assert!(m.get("manifest_version").is_some());
    assert!(m["run"].get("backend").is_some());
    assert!(m["run"].get("peak_resident_bytes").is_some());
    assert!(m["model"].get("revision_hash").is_some());
    assert!(m["model"].get("resolver_confidence").is_some());
    assert!(m.get("fidelity").is_some());
    assert!(m.get("refusals").is_some());
    assert!(m["frontier"].get("method").is_some());
}

// ---------------------------------------------------------------------------
// The schema and the Rust vocabularies may not drift
// ---------------------------------------------------------------------------

fn enum_at(pointer: &str) -> Vec<String> {
    schema_json()
        .pointer(pointer)
        .unwrap_or_else(|| panic!("no enum at {pointer}"))
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn the_schema_role_enum_matches_the_nsir_role_vocabulary() {
    let expected: Vec<String> = [
        TensorRole::TokenEmbedding,
        TensorRole::PositionEmbedding,
        TensorRole::AttentionQueryProjection,
        TensorRole::AttentionKeyProjection,
        TensorRole::AttentionValueProjection,
        TensorRole::AttentionOutputProjection,
        TensorRole::AttentionQueryNorm,
        TensorRole::AttentionKeyNorm,
        TensorRole::MlpGateProjection,
        TensorRole::MlpUpProjection,
        TensorRole::MlpDownProjection,
        TensorRole::MoeRouter,
        TensorRole::MoeExpertGateProjection,
        TensorRole::MoeExpertUpProjection,
        TensorRole::MoeExpertDownProjection,
        TensorRole::InputLayerNorm,
        TensorRole::PostAttentionLayerNorm,
        TensorRole::FinalNorm,
        TensorRole::LmHead,
        TensorRole::Bias,
        TensorRole::Unknown,
    ]
    .iter()
    .map(|r| r.as_str().to_string())
    .collect();
    let mut declared = enum_at("/definitions/tensor_entry/properties/role/enum");
    let mut wanted = expected.clone();
    declared.sort();
    wanted.sort();
    assert_eq!(declared, wanted);
    assert!(
        expected.contains(&"unknown".to_string()),
        "`unknown` is a value, not a gap"
    );
}

#[test]
fn the_schema_dtype_enum_matches_the_safetensors_spellings() {
    let expected: Vec<String> = [
        DType::Bool,
        DType::U8,
        DType::I8,
        DType::F8E4M3,
        DType::F8E5M2,
        DType::I16,
        DType::U16,
        DType::F16,
        DType::BF16,
        DType::I32,
        DType::U32,
        DType::F32,
        DType::I64,
        DType::U64,
        DType::F64,
    ]
    .iter()
    .map(|d| d.as_safetensors_str().to_string())
    .collect();
    let mut declared = enum_at("/definitions/tensor_entry/properties/dtype/enum");
    let mut wanted = expected;
    declared.sort();
    wanted.sort();
    assert_eq!(declared, wanted);
}

#[test]
fn the_schema_fidelity_enum_is_exact_sampled_and_approximate() {
    let mut declared = enum_at("/properties/fidelity/enum");
    declared.sort();
    assert_eq!(declared, vec!["approximate", "exact", "sampled"]);
}

#[test]
fn the_schema_backend_enum_cannot_express_a_backend_that_has_never_run() {
    let declared = enum_at("/definitions/run/properties/backend/enum");
    assert_eq!(declared, vec!["cpu", "metal"]);
    assert!(
        !declared.iter().any(|b| b == "cuda"),
        "CUDA-001 is Hardware-Unverified; a manifest that can name it can claim it"
    );
}

#[test]
fn the_schema_caps_tensor_rank_at_the_adr_010_ceiling() {
    assert_eq!(
        schema_json()["definitions"]["tensor_entry"]["properties"]["shape"]["maxItems"],
        json!(q_report::MAX_IMPLEMENTED_RANK)
    );
}

// ---------------------------------------------------------------------------
// Negative paths, asserted against the schema itself
// ---------------------------------------------------------------------------

#[test]
fn the_schema_refuses_an_unknown_top_level_member() {
    let mut m = golden_value();
    m.as_object_mut()
        .unwrap()
        .insert("zz_future_section".into(), json!(1));
    assert_schema_rejects(&m, "zz_future_section");
}

#[test]
fn the_schema_refuses_a_future_manifest_version() {
    let mut m = golden_value();
    m["manifest_version"] = json!(2);
    assert_schema_rejects(&m, "manifest_version");
}

#[test]
fn the_schema_refuses_a_manifest_missing_run_backend() {
    let mut m = golden_value();
    m["run"].as_object_mut().unwrap().remove("backend");
    assert_schema_rejects(&m, "backend");
}

#[test]
fn the_schema_refuses_a_backend_that_has_never_run() {
    let mut m = golden_value();
    m["run"]["backend"] = json!("cuda");
    assert_schema_rejects(&m, "cuda");
}

#[test]
fn the_schema_requires_the_refusals_array() {
    let mut m = golden_value();
    m.as_object_mut().unwrap().remove("refusals");
    assert_schema_rejects(&m, "refusals");
}

#[test]
fn the_schema_refuses_a_summary_that_still_carries_tensors() {
    let mut m = golden_value();
    m["projection"] = json!("summary");
    assert_schema_rejects(&m, "must not match");
}

#[test]
fn the_schema_refuses_a_full_projection_without_a_tensor_array() {
    let mut m = golden_value();
    m.as_object_mut().unwrap().remove("tensors");
    assert_schema_rejects(&m, "tensors");
}

#[test]
fn the_schema_refuses_a_rank_four_shape_rather_than_flattening_it() {
    let mut m = golden_value();
    m["tensors"][0]["shape"] = json!([32, 4, 128, 128]);
    assert_schema_rejects(&m, "maxItems");
}

#[test]
fn the_schema_refuses_an_unknown_dtype_tag() {
    let mut m = golden_value();
    m["tensors"][0]["dtype"] = json!("F4_SECRET");
    assert_schema_rejects(&m, "F4_SECRET");
}

#[test]
fn the_schema_refuses_an_unknown_semantic_role() {
    let mut m = golden_value();
    m["tensors"][0]["role"] = json!("attention_query_projeciton");
    assert_schema_rejects(&m, "attention_query_projeciton");
}

#[test]
fn the_schema_refuses_an_unmeasured_peak_residency() {
    let mut m = golden_value();
    m["run"]["peak_resident_bytes"] = json!(0);
    assert_schema_rejects(&m, "exclusiveMinimum");
}

#[test]
fn the_schema_refuses_a_blank_revision_hash() {
    let mut m = golden_value();
    m["model"]["revision_hash"] = json!("");
    assert_schema_rejects(&m, "minLength");
}

#[test]
fn the_schema_refuses_a_frontier_without_the_not_proven_optimal_claim() {
    let mut m = golden_value();
    m["frontier"]["claim"] = json!("Optimal mixed-precision assignment.");
    assert_schema_rejects(&m, "constant");
}

#[test]
fn the_schema_refuses_a_malformed_started_at() {
    let mut m = golden_value();
    m["run"]["started_at"] = json!("yesterday");
    assert_schema_rejects(&m, "RFC 3339");
}

#[test]
fn the_schema_refuses_per_group_granularity_without_a_group_size() {
    let mut m = golden_value();
    m["config"]["granularity"]
        .as_object_mut()
        .unwrap()
        .remove("group_size");
    assert_schema_rejects(&m, "group_size");
}

#[test]
fn the_schema_refuses_an_error_removed_fraction_above_one() {
    let mut m = golden_value();
    m["frontier"]["steps"][0]["error_removed_fraction"] = json!(1.5);
    assert_schema_rejects(&m, "maximum");
}

#[test]
fn the_schema_refuses_a_negative_sum_of_squares() {
    let mut m = golden_value();
    m["totals"]["sum_sq_delta"] = json!(-1.0);
    assert_schema_rejects(&m, "minimum");
}

#[test]
fn the_schema_refuses_a_refusal_without_a_requirement_id() {
    let mut m = golden_value();
    m["refusals"][0]
        .as_object_mut()
        .unwrap()
        .remove("requirement_id");
    assert_schema_rejects(&m, "requirement_id");
}
