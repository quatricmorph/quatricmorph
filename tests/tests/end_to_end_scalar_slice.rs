//! # Section 7 — the end-to-end vertical slice
//!
//! Requirements: `AC-005` / `TILE-07` / `PLAT-P0-LOOKUP`.
//!
//! This is the load-bearing test for the whole architecture. It exercises, in
//! one pass and with no mocking:
//!
//! ```text
//! sharded SafeTensors fixture
//!   -> q-safetensors  parses headers and the shard index
//!   -> q-nsir         resolves raw names to canonical addresses
//!   -> q-catalog      stores and queries the resulting metadata
//!   -> q-weightql     parses the scalar query  Q[10][100,42]
//!   -> q-catalog      resolves it to a canonical tensor + byte offset
//!   -> q-safetensors  range-reads the exact scalar
//!   -> assert         the value equals a Python `safetensors` reference read
//! ```
//!
//! The reference values live in `fixtures/tiny-llama-2shard/golden.json`, which
//! `fixtures/generate_fixtures.py` produced by reading the same fixture back
//! through the official Python `safetensors` library. Comparing against the
//! checked-in golden file keeps this test hermetic (no Python in CI) while
//! keeping every asserted number traceable to a real reference implementation.
//!
//! Comparison is on **exact f32 bit patterns**, not an epsilon. A range read of
//! a stored f32 either returns the stored value or the addressing is wrong;
//! there is no legitimate rounding in this path.

use q_catalog::Catalog;
use q_nsir::{Registry, ResolvedModel};
use q_safetensors::{ingest_local, read_scalar};
use q_source::{AccessScale, LocalFsSource, ResultFidelity};
use q_weightql::{QueryEngine, QueryOutcome};
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/tiny-llama-2shard")
        .canonicalize()
        .expect(
            "fixtures/tiny-llama-2shard is missing. Regenerate it with:\n  \
             python3 fixtures/generate_fixtures.py",
        )
}

fn golden() -> serde_json::Value {
    let text = std::fs::read_to_string(fixture_dir().join("golden.json"))
        .expect("golden.json must accompany the fixture");
    serde_json::from_str(&text).expect("golden.json must be valid JSON")
}

/// Parse `"0x3BD1FB7E"` into the f32 it denotes.
fn f32_from_hex_bits(s: &str) -> f32 {
    let stripped = s.trim_start_matches("0x");
    f32::from_bits(u32::from_str_radix(stripped, 16).expect("hex bit pattern"))
}

/// Run the full ingest → resolve → catalog pipeline against the fixture.
fn open_model() -> (LocalFsSource, Catalog, String) {
    let dir = fixture_dir();

    // 1. Ingest metadata. Headers and the shard index only — no payload.
    let ingested = ingest_local(&dir).expect("metadata ingestion");

    // 2. Resolve names to canonical addresses through the architecture plugin.
    let registry = Registry::builtin().expect("builtin architecture registry");
    let resolved = ResolvedModel::build(
        &registry,
        ingested.manifest.model_type().as_deref(),
        ingested.manifest.declared_architecture().as_deref(),
        ingested.descriptors.clone(),
    )
    .expect("NSIR resolution");
    assert_eq!(
        resolved.resolver_id, "llama",
        "the fixture declares model_type=llama, so the llama plugin must claim it"
    );

    // 3. Persist to the catalog.
    let catalog = Catalog::open_in_memory().expect("catalog");
    catalog
        .upsert_resolved(
            ingested.model_id,
            &ingested.manifest.root_uri,
            &ingested.manifest.source_key,
            "",
            &ingested.manifest.fingerprint(),
            "llama",
            ingested
                .manifest
                .config_u64("hidden_size")
                .map(|v| v as u32),
            &resolved,
        )
        .expect("catalog upsert");

    let source = LocalFsSource::open(&dir).expect("local source");
    (source, catalog, ingested.model_id.to_hex())
}

#[test]
fn section_7_vertical_slice_scalar_matches_python_safetensors_reference() {
    let golden = golden();
    let (source, catalog, model_id) = open_model();

    // Sanity: the fixture the golden file describes is the fixture we loaded.
    assert_eq!(
        catalog.tensor_count(&model_id).unwrap(),
        golden["tensor_count"].as_u64().unwrap(),
        "catalog tensor count must match the count the Python reference saw"
    );

    let engine = QueryEngine::with_source(&catalog, &model_id, &source).expect("query engine");

    // The query of Section 7, verbatim.
    let outcome = engine
        .run(r#"show tensor("Q[10][100,42]")"#)
        .expect("Q[10][100,42] must resolve, plan, and execute");

    let (plan, read) = match outcome {
        QueryOutcome::Scalar { plan, read } => (plan, read),
        other => panic!("expected an exact scalar, got {other:?}"),
    };

    // The alias resolved to the canonical address, and the canonical address is
    // the one ARCHITECTURE.md §6.1 specifies.
    assert_eq!(
        read.canonical_name,
        "model.layers[10].self_attention.query_projection.weight"
    );
    assert_eq!(plan.references.len(), 1);
    assert_eq!(
        plan.references[0].raw_name,
        "model.layers.10.self_attn.q_proj.weight"
    );

    // Find the matching reference entry in the golden file.
    let expected = golden["scalars"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| {
            s["tensor"] == "model.layers.10.self_attn.q_proj.weight"
                && s["index"][0] == 100
                && s["index"][1] == 42
        })
        .expect("golden.json must contain Q[10][100,42]");

    let want = f32_from_hex_bits(expected["value_f32_bits"].as_str().unwrap());
    assert_eq!(
        read.value as f32, want,
        "exact scalar must equal the Python `safetensors` reference read \
         (reference library: {})",
        golden["reference_library"]
    );

    // It came from the second shard — so shard selection, not a lucky guess at
    // the first file, is what resolved the address.
    assert_eq!(read.shard_uri, expected["shard"].as_str().unwrap());
    assert_eq!(read.shard_uri, "model-00002-of-00002.safetensors");

    // And it was a four-byte read of a 1.2 MB checkpoint.
    assert_eq!(read.bytes_read, 4);
    assert_eq!(read.fidelity, ResultFidelity::Exact);
    assert_eq!(plan.access_scale, AccessScale::SelectedBlockExact);

    // The byte offset is quotable and reproducible.
    let (shard, start, end) = catalog
        .resolve_byte_range(
            &model_id,
            "model.layers[10].self_attention.query_projection.weight",
            &[100, 42],
        )
        .expect("byte-range resolution");
    assert_eq!(shard, read.shard_uri);
    assert_eq!(start, read.byte_offset);
    assert_eq!(end - start, 4);
}

#[test]
fn every_golden_scalar_matches_through_the_full_pipeline() {
    let golden = golden();
    let (source, catalog, model_id) = open_model();
    let engine = QueryEngine::with_source(&catalog, &model_id, &source).unwrap();

    for entry in golden["scalars"].as_array().unwrap() {
        let raw_name = entry["tensor"].as_str().unwrap();
        let index: Vec<u64> = entry["index"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let want = f32_from_hex_bits(entry["value_f32_bits"].as_str().unwrap());

        // Route A: the SQL scalar form of ARCHITECTURE.md §7.1, by raw name.
        let index_list = index
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let query =
            format!(r#"SELECT value FROM tensor("{raw_name}") AT [{index_list}]"#);
        match engine.run(&query).unwrap() {
            QueryOutcome::Scalar { read, .. } => assert_eq!(
                read.value as f32,
                want,
                "SELECT value mismatch for {raw_name}{index:?}"
            ),
            other => panic!("expected a scalar for {query}, got {other:?}"),
        }

        // Route B: the low-level reader, straight from a catalog descriptor.
        let row = catalog
            .get_by_canonical_name(&model_id, raw_name)
            .unwrap()
            .unwrap_or_else(|| panic!("{raw_name} missing from catalog"));
        let descriptor = row.to_descriptor().unwrap();
        let direct = read_scalar(&source, &descriptor, &index).unwrap();
        assert_eq!(
            direct.value as f32, want,
            "direct read mismatch for {raw_name}{index:?}"
        );

        // Both routes must agree, or the query layer is adding meaning of its
        // own — which it must not.
        assert_eq!(direct.byte_offset % descriptor.dtype.size_in_bytes(), 0);
    }
}

#[test]
fn every_golden_slice_matches_through_the_query_layer() {
    let golden = golden();
    let (source, catalog, model_id) = open_model();
    let engine = QueryEngine::with_source(&catalog, &model_id, &source).unwrap();

    for entry in golden["slices"].as_array().unwrap() {
        let raw_name = entry["tensor"].as_str().unwrap();
        let rows = entry["rows"].as_array().unwrap();
        let cols = entry["columns"].as_array().unwrap();
        let (r0, r1) = (rows[0].as_u64().unwrap(), rows[1].as_u64().unwrap());
        let (c0, c1) = (cols[0].as_u64().unwrap(), cols[1].as_u64().unwrap());

        let query = format!(
            r#"SELECT slice FROM tensor("{raw_name}") ROWS {r0}:{r1} COLUMNS {c0}:{c1}"#
        );
        let read = match engine.run(&query).unwrap() {
            QueryOutcome::Slice { read, .. } => read,
            other => panic!("expected a slice for {query}, got {other:?}"),
        };

        let want: Vec<f32> = entry["values_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| f32_from_hex_bits(v.as_str().unwrap()))
            .collect();
        assert_eq!(read.values.len(), want.len());
        for (i, (got, expected)) in read.values.iter().zip(want.iter()).enumerate() {
            assert_eq!(
                *got as f32, *expected,
                "slice element {i} of {raw_name} differs from the Python reference"
            );
        }
        // Exactly the window was read: rows x cols x dtype width.
        assert_eq!(
            read.bytes_read,
            (r1 - r0) * (c1 - c0) * read.dtype.size_in_bytes()
        );
    }
}

#[test]
fn bf16_tensors_are_described_exactly_as_the_reference_sees_them() {
    let golden = golden();
    let (source, catalog, model_id) = open_model();

    for entry in golden["bf16"].as_array().unwrap() {
        let raw_name = entry["tensor"].as_str().unwrap();
        let row = catalog
            .get_by_canonical_name(&model_id, raw_name)
            .unwrap()
            .unwrap();
        assert_eq!(row.dtype, entry["dtype"].as_str().unwrap());
        assert_eq!(row.shard_uri, entry["shard"].as_str().unwrap());
        assert_eq!(row.byte_length, entry["byte_length"].as_u64().unwrap());

        // The first stored u16 must be the bit pattern the reference recorded.
        let descriptor = row.to_descriptor().unwrap();
        let first = read_scalar(&source, &descriptor, &[0, 0]).unwrap();
        let want_bits =
            u16::from_str_radix(entry["first_u16_le"].as_str().unwrap().trim_start_matches("0x"), 16)
                .unwrap();
        assert_eq!(
            (first.value as f32).to_bits(),
            (want_bits as u32) << 16,
            "bf16 decode for {raw_name} must be the high half of the f32"
        );
    }
}

#[test]
fn the_whole_slice_reads_a_negligible_fraction_of_the_checkpoint() {
    // AC-001 / TILE-01: do not load the entire checkpoint into RAM.
    //
    // The fixture is small enough that loading it would be harmless — which is
    // exactly why the assertion is on the *ratio*, not on an absolute size. The
    // same code path against a 600 GB checkpoint reads the same four bytes.
    let dir = fixture_dir();
    let ingested = ingest_local(&dir).unwrap();
    let (source, catalog, model_id) = open_model();
    let engine = QueryEngine::with_source(&catalog, &model_id, &source).unwrap();

    let read = match engine.run(r#"show tensor("Q[10][100,42]")"#).unwrap() {
        QueryOutcome::Scalar { read, .. } => read,
        other => panic!("{other:?}"),
    };

    let payload = ingested.described_payload_bytes;
    assert!(payload > 1_000_000, "fixture payload should be ~1.2 MB");
    assert_eq!(read.bytes_read, 4);
    assert!(
        read.bytes_read * 100_000 < payload,
        "a scalar read touched {} of {payload} payload bytes",
        read.bytes_read
    );

    // Metadata ingestion itself read only headers.
    assert!(
        ingested.bytes_read < payload / 10,
        "ingestion read {} bytes to describe {payload}",
        ingested.bytes_read
    );
}

#[test]
fn the_slice_is_reproducible_across_a_full_reopen() {
    // AC-008 / TILE-09 in spirit: identifiers and addresses survive reopen, so
    // a saved query still means the same thing in a later session.
    let (source_a, catalog_a, model_a) = open_model();
    let (source_b, catalog_b, model_b) = open_model();
    assert_eq!(model_a, model_b, "model_id must be stable across reopen");

    let a = QueryEngine::with_source(&catalog_a, &model_a, &source_a).unwrap();
    let b = QueryEngine::with_source(&catalog_b, &model_b, &source_b).unwrap();

    let qa = a.run(r#"show tensor("Q[10][100,42]")"#).unwrap();
    let qb = b.run(r#"show tensor("Q[10][100,42]")"#).unwrap();
    assert_eq!(qa.plan().plan_id, qb.plan().plan_id);
    assert_eq!(qa, qb);
}
