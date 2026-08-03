//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §5).
//!
//! Requirement `CAT-006` — trillion-parameter *metadata* under bounded memory.
//!
//! # What this test proves, and what it does not
//!
//! It proves that Quatricmorph can index, persist, and query the metadata of a
//! checkpoint with ~10^12 parameters while its resident allocation stays a
//! vanishing fraction of the checkpoint's payload size, and that doing so opens
//! **no weight payload at all**.
//!
//! It does **not** prove — and nothing in this repository proves — that a
//! trillion-parameter checkpoint can be loaded into RAM, VRAM, or a browser.
//! It cannot. The synthetic manifest here contains descriptors only; there are
//! no `.safetensors` files behind it, and any code path that tried to read one
//! would fail with `NotFound` rather than quietly succeeding.
//!
//! # How memory is measured
//!
//! A counting `#[global_allocator]` wraps the system allocator and tracks
//! live bytes and a high-water mark for the whole test binary. That is exact
//! and deterministic, unlike sampling RSS, which is why it is used here.

use q_catalog::{Catalog, TensorFilter};
use q_nsir::{Registry, ResolvedModel};
use q_source::role::TensorRole;
use q_source::{DType, ModelId, TensorDescriptor, TensorId};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

// --- counting allocator ------------------------------------------------------

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            bump(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, new_size);
        if !p.is_null() {
            if new_size >= layout.size() {
                bump(new_size - layout.size());
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        p
    }
}

fn bump(n: usize) {
    let live = LIVE_BYTES.fetch_add(n, Ordering::Relaxed) + n;
    PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
}

#[global_allocator]
static ALLOC: CountingAllocator = CountingAllocator;

fn peak_bytes() -> usize {
    PEAK_BYTES.load(Ordering::Relaxed)
}

fn reset_peak() {
    PEAK_BYTES.store(LIVE_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
}

// --- the synthetic manifest --------------------------------------------------

/// Shape of a plausible trillion-parameter sparse-MoE checkpoint.
///
/// Roughly the class of Kimi-K2 / DeepSeek-V3: a deep stack whose parameter
/// count is dominated by many small experts rather than a few huge dense
/// tensors. This matters because it is the *descriptor count* that stresses the
/// catalog, and MoE maximizes descriptors per parameter.
const LAYERS: u32 = 61;
const EXPERTS_PER_LAYER: u32 = 256;
const HIDDEN: u64 = 7168;
const EXPERT_INTERMEDIATE: u64 = 3072;
const ATTENTION_HEADS_DIM: u64 = 8192;

/// Declared memory ceiling for indexing and querying a trillion-parameter
/// manifest. Named, not magic: ~50 k descriptors at ~1 KB of transient
/// allocation each (descriptor + JSON shape + SQL binding), plus SQLite's page
/// cache, with headroom.
const MANIFEST_MEMORY_BUDGET_BYTES: usize = 384 * 1024 * 1024;

/// bf16 storage for the payload this manifest *describes* but never touches.
const BYTES_PER_PARAMETER: u64 = 2;

fn synthetic_descriptors(model_id: ModelId) -> Vec<TensorDescriptor> {
    let mut out = Vec::new();
    let mut offset = 0u64;
    let mut shard = 0u32;

    let push = |out: &mut Vec<TensorDescriptor>, name: String, shape: Vec<u64>, offset: &mut u64, shard: u32| {
        let n: u64 = shape.iter().product();
        let len = n * BYTES_PER_PARAMETER;
        out.push(TensorDescriptor {
            tensor_id: TensorId::derive(model_id, &name),
            raw_name: name.clone(),
            canonical_name: name,
            shape,
            dtype: DType::BF16,
            // A real checkpoint of this size is thousands of shards. The URIs
            // are strings; no file behind them exists or is opened.
            shard_uri: format!("model-{:05}-of-04096.safetensors", shard + 1),
            byte_start: *offset,
            byte_end: *offset + len,
            layer_index: None,
            semantic_role: TensorRole::Unknown,
        });
        *offset += len;
    };

    for layer in 0..LAYERS {
        // Roughly 512 GB per shard boundary; irrelevant to the assertions, but
        // keeps shard URIs realistic.
        if offset > 512 * 1024 * 1024 * 1024 {
            shard += 1;
            offset = 0;
        }
        let p = format!("model.layers.{layer}.");
        push(&mut out, format!("{p}self_attn.q_proj.weight"), vec![ATTENTION_HEADS_DIM, HIDDEN], &mut offset, shard);
        push(&mut out, format!("{p}self_attn.k_proj.weight"), vec![ATTENTION_HEADS_DIM, HIDDEN], &mut offset, shard);
        push(&mut out, format!("{p}self_attn.v_proj.weight"), vec![ATTENTION_HEADS_DIM, HIDDEN], &mut offset, shard);
        push(&mut out, format!("{p}self_attn.o_proj.weight"), vec![HIDDEN, ATTENTION_HEADS_DIM], &mut offset, shard);
        push(&mut out, format!("{p}input_layernorm.weight"), vec![HIDDEN], &mut offset, shard);
        push(&mut out, format!("{p}post_attention_layernorm.weight"), vec![HIDDEN], &mut offset, shard);
        push(&mut out, format!("{p}mlp.gate.weight"), vec![EXPERTS_PER_LAYER as u64, HIDDEN], &mut offset, shard);
        for expert in 0..EXPERTS_PER_LAYER {
            let e = format!("{p}mlp.experts.{expert}.");
            push(&mut out, format!("{e}gate_proj.weight"), vec![EXPERT_INTERMEDIATE, HIDDEN], &mut offset, shard);
            push(&mut out, format!("{e}up_proj.weight"), vec![EXPERT_INTERMEDIATE, HIDDEN], &mut offset, shard);
            push(&mut out, format!("{e}down_proj.weight"), vec![HIDDEN, EXPERT_INTERMEDIATE], &mut offset, shard);
        }
    }
    push(&mut out, "model.embed_tokens.weight".into(), vec![163_840, HIDDEN], &mut offset, shard);
    push(&mut out, "model.norm.weight".into(), vec![HIDDEN], &mut offset, shard);
    push(&mut out, "lm_head.weight".into(), vec![163_840, HIDDEN], &mut offset, shard);
    out
}

#[test]
fn trillion_parameter_manifest_indexes_and_queries_within_a_bounded_budget() {
    reset_peak();
    let baseline = peak_bytes();

    let model_id = ModelId::derive("synthetic:trillion-moe", "v1", "no-artifacts");
    let descriptors = synthetic_descriptors(model_id);

    // Every shard URI names a file that does not exist. Nothing below opens
    // one; the fact that the whole pipeline succeeds anyway is the proof that
    // metadata ingestion never touches payload.
    let shards: std::collections::BTreeSet<&str> =
        descriptors.iter().map(|d| d.shard_uri.as_str()).collect();
    assert!(!shards.is_empty());
    for s in &shards {
        assert!(
            !std::path::Path::new(s).exists(),
            "{s} unexpectedly exists on disk"
        );
    }

    let total_parameters: u64 = descriptors.iter().map(|d| d.element_count()).sum();
    let described_payload_bytes: u64 = descriptors.iter().map(|d| d.byte_length()).sum();

    // The manifest really is trillion-scale.
    assert!(
        total_parameters >= 1_000_000_000_000,
        "synthetic manifest describes only {total_parameters} parameters; \
         this test is meaningless below 10^12"
    );
    // ...and its payload is measured in terabytes, which is exactly what we are
    // never going to read.
    assert!(described_payload_bytes > 1_000_000_000_000);

    // Descriptor count is the thing that actually stresses the catalog.
    assert!(
        descriptors.len() > 40_000,
        "expected ~47k descriptors, got {}",
        descriptors.len()
    );

    let registry = Registry::builtin().unwrap();
    let resolved = ResolvedModel::build(&registry, Some("llama"), None, descriptors).unwrap();

    // MoE expert tensors resolve through the llama plugin's expert rules.
    let expert_sample = resolved
        .by_raw_name("model.layers.7.mlp.experts.42.down_proj.weight")
        .expect("expert tensor present");
    assert_eq!(
        expert_sample.canonical_name,
        "model.layers[7].moe.experts[42].down_projection.weight"
    );
    assert_eq!(
        expert_sample.semantic_role,
        TensorRole::MoeExpertDownProjection
    );

    let catalog = Catalog::open_in_memory().unwrap();
    let row = catalog
        .upsert_resolved(
            model_id,
            "synthetic://trillion-moe",
            "synthetic:trillion-moe",
            "v1",
            "no-artifacts",
            "llama",
            Some(HIDDEN as u32),
            &resolved,
        )
        .unwrap();

    assert_eq!(row.parameter_count, total_parameters);
    assert_eq!(row.layer_count, Some(LAYERS));

    let model_hex = model_id.to_hex();

    // --- hierarchy navigation ------------------------------------------------
    let layers = catalog.list_layers(&model_hex).unwrap();
    assert_eq!(layers.len() as u32, LAYERS);
    assert_eq!(
        layers[0].tensor_count as u32,
        7 + EXPERTS_PER_LAYER * 3,
        "each layer has 7 non-expert tensors plus 3 per expert"
    );

    // --- targeted lookup in the middle of a 47k-tensor model -----------------
    let t = catalog
        .get_by_canonical_name(
            &model_hex,
            "model.layers[30].self_attention.query_projection.weight",
        )
        .unwrap()
        .expect("layer 30 query projection");
    assert_eq!(t.shape, vec![ATTENTION_HEADS_DIM, HIDDEN]);

    // --- byte-range resolution: metadata arithmetic, no artifact touched -----
    let (shard, start, end) = catalog
        .resolve_byte_range(
            &model_hex,
            "model.layers[30].self_attention.query_projection.weight",
            &[4096, 512],
        )
        .unwrap();
    assert!(shard.ends_with(".safetensors"));
    assert_eq!(end - start, DType::BF16.size_in_bytes());
    // The shard file does not exist; resolution succeeded purely from metadata.
    assert!(
        !std::path::Path::new(&shard).exists(),
        "the synthetic manifest must not be backed by real files"
    );

    // --- role filter across the whole model ---------------------------------
    let all_q = catalog
        .find_by_role(&model_hex, TensorRole::AttentionQueryProjection, None)
        .unwrap();
    assert_eq!(all_q.len() as u32, LAYERS);

    let experts_in_layer_3 = catalog
        .list_tensors(
            &model_hex,
            &TensorFilter {
                layer_index: Some(3),
                role: Some(TensorRole::MoeExpertUpProjection),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(experts_in_layer_3.len() as u32, EXPERTS_PER_LAYER);

    // --- the bounded-memory claim -------------------------------------------
    let used = peak_bytes().saturating_sub(baseline);
    assert!(
        used < MANIFEST_MEMORY_BUDGET_BYTES,
        "peak allocation {used} bytes exceeded the declared budget of \
         {MANIFEST_MEMORY_BUDGET_BYTES} bytes"
    );
    // The real claim: memory used is a vanishing fraction of the checkpoint it
    // describes. Anything close to 1:1 would mean we had loaded the model.
    let ratio = described_payload_bytes / used.max(1) as u64;
    assert!(
        ratio > 10_000,
        "used {used} bytes to describe {described_payload_bytes} payload bytes \
         (ratio {ratio}:1); metadata must be orders of magnitude smaller than payload"
    );

    eprintln!(
        "CAT-006: {} tensors, {} parameters, {:.2} TB of described payload, \
         indexed and queried with {:.1} MB peak allocation ({}:1)",
        row.tensor_count,
        total_parameters,
        described_payload_bytes as f64 / 1e12,
        used as f64 / (1024.0 * 1024.0),
        ratio,
    );
}

// NOTE: there is deliberately exactly one `#[test]` in this file.
//
// The counting allocator is process-wide, so a second test running on another
// thread would allocate inside the first one's measurement window and make the
// budget assertion nondeterministic. The "opens no artifact" check is folded
// into the single test above rather than split out.
