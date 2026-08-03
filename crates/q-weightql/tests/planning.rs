//! Data plane: **Metadata Plane** (ARCHITECTURE.md §7).
//!
//! Planner tests: reference resolution, ambiguity, shape checking, and the
//! guarantee that a bad expression is rejected *before* anything executes.
//!
//! These run against the checked-in `fixtures/tiny-llama-2shard` catalog but
//! use a **planning-only** engine wherever the point is "no bytes were read" —
//! that engine has no `ModelSource` at all, so the claim is enforced by the
//! type system rather than by inspection.

use q_catalog::Catalog;
use q_nsir::{Registry, ResolvedModel};
use q_safetensors::ingest_local;
use q_source::error::QError;
use q_source::{AccessScale, ModelId, ResultFidelity};
use q_weightql::{parse, QueryEngine, QueryOutcome};
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/tiny-llama-2shard")
        .canonicalize()
        .expect("run fixtures/generate_fixtures.py")
}

fn catalog() -> (Catalog, String) {
    let out = ingest_local(fixture_dir()).unwrap();
    let registry = Registry::builtin().unwrap();
    let resolved = ResolvedModel::build(
        &registry,
        out.manifest.model_type().as_deref(),
        out.manifest.declared_architecture().as_deref(),
        out.descriptors.clone(),
    )
    .unwrap();
    let cat = Catalog::open_in_memory().unwrap();
    cat.upsert_resolved(
        out.model_id,
        &out.manifest.root_uri,
        &out.manifest.source_key,
        "",
        &out.manifest.fingerprint(),
        "llama",
        out.manifest.config_u64("hidden_size").map(|v| v as u32),
        &resolved,
    )
    .unwrap();
    (cat, out.model_id.to_hex())
}

#[test]
fn alias_and_canonical_references_resolve_to_the_same_tensor() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    assert_eq!(engine.resolver_id(), "llama");

    let by_alias = engine.resolve_reference("Q[10]").unwrap();
    let by_canonical = engine
        .resolve_reference("model.layers[10].self_attention.query_projection.weight")
        .unwrap();
    let by_raw = engine
        .resolve_reference("model.layers.10.self_attn.q_proj.weight")
        .unwrap();

    assert_eq!(by_alias.tensor_id, by_canonical.tensor_id);
    assert_eq!(by_alias.tensor_id, by_raw.tensor_id);
    assert_eq!(by_alias.shape, vec![128, 48]);
    assert_eq!(by_alias.role, "attention_query_projection");
}

#[test]
fn ambiguous_alias_is_rejected_with_the_candidate_list() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    match engine.resolve_reference("Att[10]") {
        Err(QError::AmbiguousAlias { candidates, .. }) => {
            assert_eq!(candidates.len(), 4);
            assert!(candidates
                .iter()
                .any(|c| c.contains("query_projection")));
            assert!(candidates.iter().any(|c| c.contains("key_projection")));
        }
        other => panic!("expected AmbiguousAlias, got {other:?}"),
    }
    // The same alias, presented as choices rather than an error.
    let candidates = engine.alias_candidates("Att[10]").unwrap();
    assert_eq!(candidates.len(), 4);
    assert!((candidates[0].confidence - 0.25).abs() < 1e-6);
}

#[test]
fn unknown_alias_and_unknown_address_are_both_rejected() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    assert!(engine.resolve_reference("Zzz[10]").is_err());
    assert!(engine
        .resolve_reference("model.layers[999].self_attention.query_projection.weight")
        .is_err());
}

#[test]
fn shape_mismatch_is_rejected_before_execution() {
    let (cat, model_id) = catalog();
    // A planning-only engine has no ModelSource, so this test cannot read a
    // byte even if the planner tried.
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();

    // Q[10] is [128, 48]; K[10] is [32, 48]. Q @ K has inner dims 48 vs 32.
    let script = parse(
        r#"A = tensor("Q[10]")
           B = tensor("K[10]")
           show A @ B"#,
    )
    .unwrap();
    let err = engine.plan(&script).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("shape mismatch"), "{msg}");
    assert!(msg.contains("48") && msg.contains("32"), "{msg}");
}

#[test]
fn explicit_transpose_makes_the_expression_type_check() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    // Q[10] is [128,48]; transpose(K[10]) is [48,32]; product is [128,32].
    let script = parse(
        r#"A = tensor("Q[10]")
           B = transpose(tensor("K[10]"))
           show A @ B"#,
    )
    .unwrap();
    let plan = engine.plan(&script).unwrap();
    assert_eq!(plan.output_shape, vec![128, 32]);
    assert_eq!(plan.matmul_count, 1);
    assert!(!plan.executable_now);
    assert_eq!(plan.blocked_by.as_deref(), Some("WQL-006"));
    assert!(plan
        .blocked_reason
        .as_ref()
        .unwrap()
        .contains("no GPU or CPU expression backend"));
    assert_eq!(plan.access_scale, AccessScale::SelectedBlockCompute);
}

#[test]
fn the_architecture_md_block_matmul_script_plans() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    // ARCHITECTURE.md §8.1 Tensor Block Mode: A[0:32,0:32] @ B[0:32,0:32].
    let script = parse(
        r#"A = tensor("Q[10][0:32,0:32]")
           B = transpose(tensor("K[10][0:32,0:32]"))
           show A @ B"#,
    )
    .unwrap();
    let plan = engine.plan(&script).unwrap();
    assert_eq!(plan.output_shape, vec![32, 32]);
    // Two 32x32 f32 blocks = 8192 bytes; nowhere near the whole tensors.
    assert_eq!(plan.estimated_read_bytes, 2 * 32 * 32 * 4);
    assert_eq!(plan.references.len(), 2);
    // Plan IDs are deterministic and quotable.
    assert!(plan.plan_id.starts_with("plan:b3:"));
    assert_eq!(plan.plan_id, engine.plan(&script).unwrap().plan_id);
}

#[test]
fn chained_matmul_produces_two_nodes_and_the_documented_shape() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    // Q[10]:[128,48] @ transpose(K[10]):[48,32] -> [128,32]
    //                                    @ V[10]:[32,48] -> [128,48]
    let script = parse(
        r#"show tensor("Q[10]") @ transpose(tensor("K[10]")) @ tensor("V[10]")"#,
    )
    .unwrap();
    let plan = engine.plan(&script).unwrap();
    assert_eq!(plan.matmul_count, 2);
    assert_eq!(plan.output_shape, vec![128, 48]);
}

#[test]
fn planning_only_engine_refuses_to_execute_even_a_pure_read() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    let outcome = engine
        .run(r#"SELECT value FROM tensor("Q[10]") AT [100, 42]"#)
        .unwrap();
    match outcome {
        QueryOutcome::Planned(plan) => {
            assert_eq!(plan.blocked_by.as_deref(), Some("WQL-005"));
            assert!(plan.blocked_reason.unwrap().contains("no ModelSource"));
        }
        other => panic!("planning-only engine must not execute; got {other:?}"),
    }
}

#[test]
fn whole_tensor_reads_are_refused_with_an_explanation() {
    let (cat, model_id) = catalog();
    let src = q_source::LocalFsSource::open(fixture_dir()).unwrap();
    let engine = QueryEngine::with_source(&cat, &model_id, &src).unwrap();
    let err = engine.run(r#"show tensor("Q[10]")"#).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("whole-tensor reads are refused"), "{msg}");
    assert!(msg.contains("Select a region"), "{msg}");
}

#[test]
fn scalar_and_slice_queries_execute_and_are_labelled_exact() {
    let (cat, model_id) = catalog();
    let src = q_source::LocalFsSource::open(fixture_dir()).unwrap();
    let engine = QueryEngine::with_source(&cat, &model_id, &src).unwrap();

    match engine.run(r#"show tensor("Q[10][100,42]")"#).unwrap() {
        QueryOutcome::Scalar { plan, read } => {
            assert_eq!(read.value as f32, f32::from_bits(0x3BD1FB7E));
            assert_eq!(read.bytes_read, 4);
            assert_eq!(plan.fidelity, ResultFidelity::Exact);
            assert_eq!(plan.access_scale, AccessScale::SelectedBlockExact);
            assert_eq!(plan.estimated_read_bytes, 4);
        }
        other => panic!("expected a scalar, got {other:?}"),
    }

    match engine
        .run(r#"SELECT slice FROM tensor("Q[10]") ROWS 100:104 COLUMNS 40:44"#)
        .unwrap()
    {
        QueryOutcome::Slice { plan, read } => {
            assert_eq!((read.rows(), read.columns()), (4, 4));
            assert_eq!(read.get(0, 2).unwrap() as f32, f32::from_bits(0x3BD1FB7E));
            assert_eq!(plan.estimated_read_bytes, 4 * 4 * 4);
        }
        other => panic!("expected a slice, got {other:?}"),
    }
}

#[test]
fn out_of_bounds_selector_is_rejected_at_plan_time() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    let script = parse(r#"show tensor("Q[10][500,0]")"#).unwrap();
    assert!(engine.plan(&script).is_err());
}

#[test]
fn model_that_is_not_in_the_catalog_is_reported() {
    let (cat, _) = catalog();
    assert!(matches!(
        QueryEngine::planning_only(&cat, &ModelId::derive("nope", "", "").to_hex()),
        Err(QError::NotFound(_))
    ));
}

#[test]
fn moe_and_unresolved_names_behave_predictably() {
    let (cat, model_id) = catalog();
    let engine = QueryEngine::planning_only(&cat, &model_id).unwrap();
    // The fixture is dense, so the MoE alias matches nothing — and says so
    // rather than falling back to a dense tensor.
    assert!(matches!(
        engine.resolve_reference("Expert[10,3].up"),
        Err(QError::NotFound(_))
    ));
}
