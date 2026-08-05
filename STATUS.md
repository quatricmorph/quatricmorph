# STATUS — requirement traceability

Generated from a real test run, not from intent. Every row marked `Verified`
cites a test file that exists and passed in the run recorded below.

**Test run (2026-08-03):**

```bash
cargo test --workspace          # 290 passed, 0 failed, 0 ignored
cd apps/web && npx vitest run   # 101 passed, 0 failed  (12 files)
cargo fmt --all --check         # clean
cargo clippy --workspace --all-targets -- -D warnings   # clean
```

Total: **391 passing tests, 0 failing.**

## Status vocabulary

| Status | Meaning |
| --- | --- |
| `Verified` | Implemented **and** covered by a passing automated test cited in the row |
| `Implemented` | Code exists and works; not covered by a dedicated test |
| `Partial` | A working vertical slice; feature coverage deliberately incomplete |
| `Stub` | Real types/traits exist; every operation returns `NotImplemented` with this ID |
| `Hardware-Unverified` | Code exists, has **never** been executed on the hardware it targets |
| `Not Started` | — |

## Requirement ID prefixes

This document uses the fine-grained prefixes the task specifies (`SRC-`,
`NSIR-`, `CAT-`, …). The repository's own checklists use coarser IDs — `TILE-*`
in `docs/requirements/VIZ_MVP.md` and `PLAT-*` in
`docs/requirements/MVP_REQUIREMENTS.md`. The **Maps to** column links the two so
neither numbering is orphaned.

---

## SRC — SafeTensors ingestion and the Artifact Plane

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| SRC-001 | Parse a single SafeTensors header (length prefix, JSON, offsets) | Verified | TILE-02 | `crates/q-safetensors/src/header.rs` | `header::tests::parses_a_minimal_header` |
| SRC-002 | `__metadata__` is not counted as a tensor | Verified | TILE-02 | `crates/q-safetensors/src/header.rs` | `header::tests::metadata_key_is_not_counted_as_a_tensor` |
| SRC-003 | Sharded checkpoints via `model.safetensors.index.json` | Verified | PLAT-P0-INGEST | `crates/q-safetensors/src/index.rs`, `ingest.rs` | `ingest::tests::ingests_a_sharded_checkpoint` |
| SRC-004 | Single-file checkpoints | Verified | TILE-02 | `crates/q-safetensors/src/ingest.rs` | `ingest::tests::ingests_a_single_file_checkpoint` |
| SRC-005 | Memory-mapped local byte-range reads | Verified | PLAT-P0-INGEST | `crates/q-source/src/local.rs` | `local::tests::range_read_returns_exactly_the_window` |
| SRC-006 | Stable `model_id` / `tensor_id` across reopen | Verified | PLAT-P0-LOOKUP | `crates/q-source/src/ids.rs` | `ingest::tests::tensor_ids_are_stable_across_reopen`, `ids::tests::*` |
| SRC-007 | Metadata import allocates nothing proportional to checkpoint size | Verified | TILE-01, AC-001 | `crates/q-safetensors/src/ingest.rs`, `crates/q-source/src/budget.rs` | `ingest::tests::ingestion_reads_only_headers_not_payload`, `the_whole_slice_reads_a_negligible_fraction_of_the_checkpoint` |
| SRC-008 | HTTP Range reads for remote checkpoints | **Stub** | PLAT-P0-INGEST | `crates/q-source/src/http.rs` | `http::tests::offline_source_refuses_with_a_requirement_id` (range arithmetic is Verified; transport is not built) |
| SRC-009 | Ingestion is cancellable | Verified | TILE-03, AC-003 | `crates/q-source/src/cancel.rs` | `ingest::tests::cancellation_stops_at_a_shard_boundary` |
| SRC-010 | Ingestion is resumable | Verified | TILE-03, AC-003 | `crates/q-source/src/cancel.rs` | `ingest::tests::resume_skips_completed_shards` |
| SRC-011 | Missing shard named by the index is reported | Verified | PLAT-P0-INGEST | `crates/q-safetensors/src/index.rs` | `ingest::tests::missing_shard_named_by_the_index_is_reported` |
| SRC-012 | Duplicate tensor name rejected (within and across shards) | Verified | PLAT-P0-INGEST | `crates/q-safetensors/src/header.rs`, `ingest.rs` | `header::tests::duplicate_tensor_name_is_rejected`, `ingest::tests::duplicate_tensor_across_shards_is_rejected` |
| SRC-013 | Corrupt header rejected with context | Verified | PLAT-P0-INGEST | `crates/q-safetensors/src/header.rs` | `header::tests::corrupt_json_is_rejected_with_context`, `absurd_header_length_is_refused_before_allocating` |
| SRC-014 | Unsupported dtype rejected, never guessed | Verified | PLAT-P0-INGEST | `crates/q-source/src/dtype.rs` | `dtype::tests::unknown_dtype_is_rejected_not_guessed`, `fp8_refuses_rather_than_approximates` |
| SRC-015 | Invalid byte offset rejected | Verified | PLAT-P0-INGEST | `crates/q-source/src/local.rs`, `descriptor.rs` | `local::tests::range_past_end_of_file_is_rejected`, `descriptor::tests::run_that_overruns_the_tensor_is_rejected` |
| SRC-016 | Exact dtype decoding (f32/bf16/f16 incl. subnormals) | Verified | AC-005 | `crates/q-source/src/dtype.rs` | `dtype::tests::f32_decode_is_exact`, `bf16_is_high_half_of_f32`, `f16_handles_normal_subnormal_and_inf` |
| SRC-017 | Named, enforced memory budgets | Verified | AC-001 | `crates/q-source/src/budget.rs` | `budget::tests::*`, `ingest::tests::a_tight_metadata_budget_is_enforced` |
| SRC-018 | Access scale is a type, not a comment | Verified | AC-010 | `crates/q-source/src/lib.rs` | `tests::metadata_scale_never_reads_payload`, `visualization_scale_is_never_exact` |

## NSIR — canonical addressing and semantic resolution

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| NSIR-001 | Generic resolver returns `unknown` for names it was not taught | Verified | PLAT-P0-ADAPTER | `crates/q-nsir/src/resolver.rs`, `architectures/generic/plugin.toml` | `resolver::tests::generic_resolver_returns_unknown_for_names_it_was_not_taught` |
| NSIR-002 | Llama-family resolver (the one named architecture this pass) | Verified | PLAT-P0-ADAPTER | `architectures/llama/plugin.toml`, `crates/q-nsir/src/resolver.rs` | `resolver::tests::llama_resolves_the_architecture_md_example` |
| NSIR-003 | MoE expert addressing | Verified | PLAT-P0-ADAPTER | `crates/q-nsir/src/resolver.rs` | `resolver::tests::llama_resolves_moe_expert_tensors` |
| NSIR-004 | Canonical address construction and stability | Verified | TILE-06 | `crates/q-nsir/src/address.rs`, `resolver.rs` | `address::tests::round_trips_through_display`, `resolver::tests::canonical_names_are_stable_across_resolution_runs` |
| NSIR-005 | Contextual alias grammar (`Q[10][100,42]`, `MLP.down[24][:]`, `Expert[12,37].up[0:128,:]`) | Verified | PLAT-P1-WQL | `crates/q-nsir/src/alias.rs` | `alias::tests::parses_the_five_architecture_md_forms` |
| NSIR-006 | Qwen / Kimi / DeepSeek resolvers | **Not Started** | PLAT-P0-ADAPTER | `architectures/{qwen,kimi,deepseek}/plugin.toml` (declared, `implemented = false`) | `q_architecture::tests::unimplemented_plugins_are_declared_and_never_claim` (asserts they never claim a model) |
| NSIR-007 | Ambiguous alias returns candidates, never a silent pick | Verified | PLAT-P1-WQL | `crates/q-nsir/src/resolver.rs` | `resolver::tests::ambiguous_alias_returns_candidates_not_a_silent_pick` |
| NSIR-008 | Architecture plugin registry with priority selection and generic fallback | Verified | PLAT-P0-ADAPTER | `crates/q-architecture/src/lib.rs` | `tests::llama_is_selected_by_model_type_and_by_architecture`, `unknown_model_falls_back_to_generic` |
| NSIR-009 | Invalid address / alias syntax rejected | Verified | PLAT-P1-WQL | `crates/q-nsir/src/{address,alias}.rs` | `address::tests::invalid_syntax_is_rejected_not_guessed`, `alias::tests::invalid_syntax_is_rejected` |
| NSIR-010 | Model-level metadata typed from `config.json`; declared, never inferred | Verified | MVP-06 | `crates/q-architecture/src/lib.rs` (`ModelConfigMetadata`) | `q_architecture::tests::config_metadata_parses_every_declared_field_of_the_fixture`, `a_field_of_the_wrong_type_is_none_and_the_rest_still_load`, `config_torch_dtype_is_recorded_as_declared_not_used_to_infer_tensor_dtype`, `malformed_config_json_is_refused_with_context` |

## CAT — metadata catalog

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| CAT-001 | Schema for models/tensors/blocks/statistics/tiles/jobs | Verified | PLAT-P0-CATALOG | `crates/q-catalog/src/schema.rs` | `schema::tests::migrations_apply_cleanly_to_an_empty_database` |
| CAT-002 | Versioned migrations, idempotent, future-schema refused | Verified | PLAT-P0-CATALOG | `crates/q-catalog/src/schema.rs` | `schema::tests::migrations_are_idempotent`, `a_future_schema_is_refused_rather_than_corrupted` |
| CAT-003 | Hierarchy browsing (model → layer → tensor) | Verified | PLAT-P0-CATALOG | `crates/q-catalog/src/lib.rs` | `tests::hierarchy_browse_returns_one_summary_per_layer` |
| CAT-004 | Canonical-address lookup with raw-name fallback | Verified | PLAT-P0-LOOKUP | `crates/q-catalog/src/lib.rs` | `tests::canonical_address_lookup_and_raw_name_fallback` |
| CAT-005 | Role / component / dtype / rank / layer filters | Verified | PLAT-P0-CATALOG | `crates/q-catalog/src/lib.rs` | `tests::shape_dtype_and_resolution_filters_work`, `role_and_layer_filters_drive_alias_resolution` |
| CAT-006 | **Trillion-parameter metadata under a bounded memory budget** | Verified | PLAT-P0-CATALOG | `crates/q-catalog/tests/trillion_scale_manifest.rs` | `trillion_parameter_manifest_indexes_and_queries_within_a_bounded_budget` — 47 278 tensors, 1.048×10¹² parameters, 2.10 TB of *described* payload, indexed and queried with **35.7 MB peak allocation** (56 040:1), opening no artifact |
| CAT-007 | Byte-range resolution is pure metadata arithmetic | Verified | PLAT-P0-LOOKUP | `crates/q-catalog/src/lib.rs` | `tests::byte_range_resolution_is_pure_metadata_arithmetic` |
| CAT-008 | Catalog survives close and reopen | Verified | AC-008 | `crates/q-catalog/src/lib.rs` | `tests::catalog_survives_close_and_reopen` |
| CAT-009 | Re-import is idempotent | Verified | PLAT-P0-CATALOG | `crates/q-catalog/src/lib.rs` | `tests::reimporting_the_same_model_is_idempotent` |
| CAT-010 | DuckDB/Arrow/Parquet backend (ARCHITECTURE.md §5) | **Not Started** | PLAT-P0-CATALOG | — | Departure recorded in `docs/decisions/ADR-003-catalog-sqlite.md` |
| CAT-011 | `models.hidden_size` / `layer_count` / `parameter_count` filled from `config.json` and the manifest; absent means `NULL`, never `0` | Verified | MVP-06 | `crates/q-catalog/src/lib.rs`, `crates/q-safetensors/src/ingest.rs`, `crates/q-daemon/src/lib.rs`, `crates/q-cli/src/main.rs` | `q_catalog::tests::config_declared_hidden_size_persists_and_reloads`, `observed_layer_count_wins_over_a_disagreeing_declared_one`, `declared_layer_count_fills_in_only_when_none_was_observed`, `a_model_without_a_config_persists_null_columns_never_zero`, `persisted_parameter_count_is_summed_from_descriptors_not_from_config_arithmetic`, `q_daemon::tests::the_model_route_carries_config_metadata_and_a_metadata_fidelity` |

## WQL — WeightQL

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| WQL-001 | Tokenizer | Verified | PLAT-P1-WQL | `crates/q-weightql/src/lexer.rs` | `lexer::tests::*` (9 tests) |
| WQL-002 | Parser → AST (assignment, `show`, `SELECT value`, `SELECT slice`) | Verified | PLAT-P1-WQL | `crates/q-weightql/src/parser.rs` | `parser::tests::parses_the_architecture_md_matmul_script`, `parses_the_sql_scalar_form`, `parses_the_sql_slice_form` |
| WQL-003 | Reference resolution (canonical, raw, alias) against the catalog | Verified | PLAT-P1-WQL | `crates/q-weightql/src/plan.rs` | `tests/planning.rs::alias_and_canonical_references_resolve_to_the_same_tensor` |
| WQL-004 | **Shape mismatch rejected before execution** | Verified | AC-009, PLAT-P1-EXPR | `crates/q-expression/src/lib.rs`, `crates/q-weightql/src/plan.rs` | `tests/planning.rs::shape_mismatch_is_rejected_before_execution` (planning-only engine has no `ModelSource`, so it *cannot* read) |
| WQL-005 | Scalar / slice queries execute via byte-range reads | Verified | AC-005, PLAT-P0-LOOKUP | `crates/q-weightql/src/plan.rs`, `crates/q-safetensors/src/read.rs` | `tests/planning.rs::scalar_and_slice_queries_execute_and_are_labelled_exact` |
| WQL-006 | Matrix-multiplication **execution** | **Stub** | PLAT-P1-EXPR | `crates/q-weightql/src/plan.rs` | Planning is Verified (`explicit_transpose_makes_the_expression_type_check`); no compute backend exists, and the plan says so |
| WQL-007 | Statistical `SELECT` (`GROUP BY layer_index`, ARCHITECTURE.md §7.3) | **Not Started** | PLAT-P1-WQL | — | Parser rejects it by name with this ID (`parser::tests::unsupported_select_target_is_named_with_its_requirement`) |
| WQL-008 | Stacked slice composition (`A[0:64][0:8]`) | **Stub** | — | `crates/q-weightql/src/plan.rs` | Returns `NotImplemented` rather than approximating |
| WQL-009 | **No arbitrary code execution** | Verified | SEC | `crates/q-expression/src/lib.rs` (closed enum), `crates/q-weightql/src/parser.rs` | `parser::tests::arbitrary_code_execution_constructs_are_rejected`, `unknown_function_error_names_the_closed_function_set` |
| WQL-010 | I/O cost estimate on every plan | Verified | PLAT-P1-EXPR | `crates/q-weightql/src/plan.rs` | `tests/planning.rs::the_architecture_md_block_matmul_script_plans` |
| WQL-011 | Whole-tensor reads refused | Verified | AC-001 | `crates/q-weightql/src/plan.rs` | `tests/planning.rs::whole_tensor_reads_are_refused_with_an_explanation` |
| WQL-012 | Deterministic, quotable plan IDs | Verified | — | `crates/q-weightql/src/plan.rs` | `tests/planning.rs::the_architecture_md_block_matmul_script_plans` |

## STAT — statistics

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| STAT-001 | CPU reference statistics (min/max/mean/variance/L1/L2/ratios/histogram) | Verified | PLAT-P0-CATALOG | `crates/q-statistics/src/lib.rs` | `tests::hand_computed_moments_on_a_small_fixture`, `hand_computed_ratios_with_signs_and_zeros`, `hand_computed_histogram_binning` — all expected values computed by hand |
| STAT-002 | Statistics persisted and served over the API | **Stub** | PLAT-P0-API | `crates/q-daemon/src/lib.rs` | `tests::unbuilt_routes_return_501_with_a_requirement_id` (501 + this ID) |
| STAT-003 | Numerically stable variance (Welford) | Verified | — | `crates/q-statistics/src/lib.rs` | `tests::welford_stays_accurate_where_the_naive_formula_collapses` |
| STAT-004 | Streaming accumulation equals batch computation | Verified | — | `crates/q-statistics/src/lib.rs` | `tests::streaming_in_chunks_equals_computing_at_once` |
| STAT-005 | Sampled results are labelled approximate | Verified | AC-010 | `crates/q-statistics/src/lib.rs` | `tests::approximate_results_are_labelled` |
| STAT-006 | Comparison metrics (cosine similarity, relative L2) | Verified | PLAT-P1-EXPR | `crates/q-statistics/src/lib.rs` | `tests::hand_computed_cosine_similarity_and_relative_l2` |
| STAT-007 | Block statistics over a real checkpoint block | Verified | — | `crates/q-gpu/src/lib.rs` | `tests::block_statistics_stream_a_real_fixture_block` |

## TILE — tensor tiles and the `.qtile` format

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| TILE-001 | LOD ladder (0–5) as a closed enum | Verified | TILE-04 | `crates/q-tensor-runtime/src/lib.rs` | `tests::the_ladder_has_exactly_six_levels`, `only_the_finest_level_carries_exact_values` |
| TILE-002 | `TensorBlock` byte-range planning (no reads) | Verified | TILE-04 | `crates/q-tensor-runtime/src/lib.rs` | `tests::block_planning_derives_one_byte_run_per_row` |
| TILE-003 | Stable, extent- and LOD-sensitive tile IDs | Verified | TILE-04 | `crates/q-tensor-runtime/src/lib.rs` | `tests::tile_ids_are_stable_and_sensitive_to_extent_and_lod` |
| TILE-004 | **Tile pyramid generation** (building `.qtile` files for a model) | **Not Started** | TILE-04 | — | Daemon returns 501 with this ID |
| TILE-005 | `.qtile` v1 encode/decode round trip, byte-exact | Verified | TILE-04 | `crates/q-tiles/src/lib.rs` | `tests::round_trip_preserves_header_and_payload_byte_for_byte`, `round_trip_preserves_exact_f32_values` |
| TILE-006 | `.qtile` rejects corrupt and hostile files | Verified | — | `crates/q-tiles/src/lib.rs` | `tests::corrupt_and_hostile_files_are_rejected` (8 distinct corruptions) |
| TILE-007 | Little-endian regardless of host | Verified | — | `crates/q-tiles/src/lib.rs` | `tests::encoding_is_little_endian_regardless_of_host` |
| TILE-008 | Quantized encoding declares itself lossy | Verified | AC-010 | `crates/q-tiles/src/lib.rs` | `tests::quantized_tiles_are_half_the_size_and_declare_themselves_lossy` |

## GLB / CESIUM — Visualization Plane

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| GLB-001 | GLB tile-content generation | **Stub** | TILE-04 | `crates/q-gltf/src/lib.rs` | `tests::the_builder_refuses_rather_than_emitting_a_placeholder_glb` |
| GLB-002 | Cube-per-weight explosions refused | Verified | ARCH §19 | `crates/q-gltf/src/lib.rs` | `tests::cube_per_weight_explosions_are_refused` |
| GLB-003 | A GLB may never be the only carrier of tensor values | Verified | ARCH §10.1 | `crates/q-gltf/src/lib.rs` | `tests::a_glb_without_a_qtile_sidecar_is_refused` |
| CESIUM-001 | `tileset.json` generation | **Stub** | TILE-05 | `crates/q-tileset/src/lib.rs` | `tests::the_builder_refuses_rather_than_emitting_a_fake_tileset` |
| CESIUM-002 | LOD loading policy; exact values only on selection | Verified | TILE-08, AC-006, AC-007 | `apps/web/model-viewer/src/lod-policy.ts` | `never_reads_exact_values_from_camera_movement_alone`, `reads_exact_values_only_on_an_explicit_selection` |
| CESIUM-003 | Daemon client; 501 is a value, not a retryable failure | Verified | PLAT-P0-API | `apps/web/model-viewer/src/tile-client.ts` | `treats_a_501_as_a_declared_gap_not_a_failure_to_retry` |
| CESIUM-004 | Geometric error decreases monotonically with depth | Verified | TILE-05 | `crates/q-tileset/src/lib.rs` | `tests::geometric_error_halves_down_the_ladder`, `a_child_that_never_refines_is_rejected` |
| CESIUM-005 | CesiumJS viewer actually rendering a tileset | **Not Started** | TILE-05, PLAT-P0-UX | `apps/web/model-viewer/` (shell only) | — |

## GRID / MATMUL — matrix workspace

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| GRID-001 | `GridRuler3D` — one spatial authority, ten layout parameters | Verified | TILE-06 | `apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts` | `exposes_every_layout_parameter_the_workspace_needs`, `every_operand_placement_it_produces_is_on_grid` |
| GRID-002 | Grid snap invariant within a documented tolerance (1e-6) | Verified | — | `apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts` | `snaps_positions_to_cellSize_multiples`, `tolerates_float_accumulation_within_the_documented_tolerance` |
| GRID-003 | `TensorGridFrame` — boundary, margins, labels, deterministic anchor | Implemented | — | `apps/web/quatricmorph-workspace/src/layout/tensor-frame.ts` | `layout/__tests__/grid-ruler.test.ts` |
| GRID-004 | Tensor-block adapter (real checkpoint data into the workspace) | **Stub** | — | `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts` | `the_daemon_source_refuses_rather_than_returning_plausible_zeros` |
| GRID-005 | Block-request ceiling; no whole-tensor transfer to the browser | Verified | ARCH §19 | `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts` | `refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser` |
| MATMUL-001 | Pure matmul separated from Three.js scene state | Verified | PLAT-P1-MATMUL-VIZ | `apps/web/quatricmorph-workspace/src/math/matmul.ts`, `blocking.ts` | `math/__tests__/matmul.test.ts` (2×3@3×2, 3×3@3×1, 1×3@3×2, 1×3@3×1, 1×1@1×1, 2×3@2×2 rejected) |
| MATMUL-002 | Block decomposition extracted as pure index math | Verified | — | `apps/web/quatricmorph-workspace/src/math/blocking.ts` | `math/__tests__/blocking.test.ts` (7 tests) |
| MATMUL-003 | Animation schedule extracted as a pure state machine | Verified | PLAT-P1-MATMUL-VIZ | `apps/web/quatricmorph-workspace/src/math/animation-schedule.ts` | `math/__tests__/animation-schedule.test.ts` (7 tests) |
| MATMUL-004 | CPU reference matmul (Rust) | Verified | — | `crates/q-gpu/src/lib.rs` | `tests::hand_computed_matmul_2x3_by_3x2`, `hand_computed_matmul_edge_shapes` |
| MATMUL-005 | Demo still runs after extraction (regression bar) | Verified | — | `apps/web/quatricmorph-workspace/` | 74 workspace tests pass; `npx vite build` succeeds |

## CACHE

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| CACHE-001 | §13.2 cache key; every component affects the digest | Verified | PLAT-P0-CACHE | `crates/q-cache/src/lib.rs` | `tests::every_key_component_changes_the_digest`, `length_prefixing_prevents_field_boundary_collisions` |
| CACHE-002 | L1 in-process LRU: write, read, evict (by count and by bytes) | Verified | PLAT-P0-CACHE | `crates/q-cache/src/lib.rs` | `tests::l1_write_read_and_stats`, `l1_evicts_by_entry_count`, `l1_evicts_by_byte_budget` |
| CACHE-003 | L2 content-addressed disk cache; size limit and eviction | Verified | PLAT-P0-CACHE | `crates/q-cache/src/lib.rs` | `tests::l2_write_read_and_content_addressed_layout`, `l2_evicts_to_stay_under_its_budget` |
| CACHE-004 | **Cache reused after reopening** | Verified | TILE-09, AC-008 | `crates/q-cache/src/lib.rs` | `tests::l2_is_reused_after_reopen`, `layered_cache_survives_reopen_of_its_l2` |
| CACHE-005 | L0 GPU-resident cache | **Not Started** | — | — | — |
| CACHE-006 | L3 browser cache (Cache Storage / IndexedDB) | **Stub** | — | `crates/q-cache/src/lib.rs` | `tests::l3_and_l4_refuse_rather_than_missing_silently` |
| CACHE-007 | L4 remote object storage / CDN | **Stub** | — | `crates/q-cache/src/lib.rs` | `tests::l3_and_l4_refuse_rather_than_missing_silently` |
| CACHE-008 | Cache is wired into the query path | **Not Started** | PLAT-P0-CACHE | — | The cache works; nothing calls it yet |

## CUDA / GPU — ⚠ Hardware-Unverified

**No code in this section has ever run on a GPU.** The target is an RTX 3090
(24 GB VRAM), which was not available in the environment this code was written
in. `crates/q-cuda` compiles no kernels, links no driver, and returns
`NotImplemented` for every operation. The `.cu` sources carry documented
signatures and launch geometry and have **never been compiled**. See
`gpu/cuda/README.md`.

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| GPU-001 | Compute-backend trait | Verified | — | `crates/q-gpu/src/lib.rs` | `tests::cpu_backend_declares_itself_verified_and_capable` |
| GPU-002 | CPU reference backend (the ground truth for all others) | Verified | — | `crates/q-gpu/src/lib.rs` | 7 tests |
| GPU-003 | wgpu / Metal backends | **Metal: Implemented, Not Verified** (QM-0126) · **wgpu: Not Started** | QM-0126 | Metal: `crates/q-gpu/src/metal.rs`, `crates/q-gpu/build.rs`, `gpu/metal/paired_reduction.metal`, `gpu/metal/qm_metal_shim.m`. wgpu: `gpu/wgsl/compute.wgsl` (placeholder shader), `gpu/metal/compute.metal` (placeholder shader) | Metal: 9 tests behind the off-by-default `metal` feature. The shader compiles and the kernel was dispatched on a real Apple M3 Pro; it has **not** been diffed against `CpuBackend`, so `hardware_verified` is `false` and stays so until `QM-0127`. Evidence: `.plan/evidence/QM-0126.md`. wgpu: **none — not started** |
| CUDA-001 | CUDA backend implements the compute trait | **Hardware-Unverified** | — | `crates/q-cuda/src/lib.rs` | `tests::every_operation_refuses_with_a_requirement_id_rather_than_faking_output` — refusal is tested; execution is not, and cannot be here |
| CUDA-002 | Reduction kernels (min/max/sum/sum-of-squares) | **Hardware-Unverified** | — | `gpu/cuda/reduce.cu` | **none — never compiled or executed** |
| CUDA-003 | Histogram kernel | **Hardware-Unverified** | — | `gpu/cuda/histogram.cu` | **none — never compiled or executed** |
| CUDA-004 | Tiled block matmul kernels | **Hardware-Unverified** | — | `gpu/cuda/matmul.cu` | **none — never compiled or executed** |
| CUDA-005 | Quantization / Morton packing kernels | **Hardware-Unverified** | — | `gpu/cuda/quantize.cu` | **none — never compiled or executed** |
| CUDA-006 | VRAM ceiling enforced before a launch | Verified | — | `crates/q-cuda/src/lib.rs` | `tests::the_vram_ceiling_is_enforced_without_a_device` (arithmetic on a declared limit, not a device query) |

## SURF — v1 diagnostic surface

The v1 surface is the diagnostics heat map over layer x channel, fed by the
report manifest. It is not the deferred Cesium viewer or matrix workspace.

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| SURF-001 | Heat-map surface over layer x channel, fed by the manifest | Verified | `QM-0150` | `apps/web/diagnostics/src/heatmap.ts`, `src/render.ts` | `heatmap.test.ts`, `artifacts.test.ts` |
| SURF-002 | Degradation to an aggregate above a rendering ceiling, stated in the UI | Verified | `QM-0153` | `apps/web/diagnostics/src/heatmap.ts`, `src/render.ts`, `src/app.ts` | `degradation.test.ts` — 24 tests, including the AC5 channel-coverage assertions shown by mutation to fail under both a gross truncation and a one-group off-by-one |

`MAX_HEATMAP_CELLS` is 250 000; above it, columns aggregate **by maximum, never by
mean** — a mean would hide one catastrophic channel inside a healthy group, which is
the finding the tool exists to surface. Aggregation carries a persistent marker
legible without hover and in greyscale, and `sampled` (engine-side coarseness) is
rendered and captioned distinctly from `aggregated` (renderer-side).

**Not claimed:** no screenshot was taken — there is no browser in the build
environment. Committed SVG artifacts from the same draw plan stand in and are
asserted byte-for-byte; they are not screenshots. The 2-D canvas painter draws
neither mark (it never drew `QM-0150`'s dash either); the marked SVG is placed on the
page beside the canvas.

## API — local daemon

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| API-001 | `GET /v1/models`, `/v1/models/{id}`, `/v1/models/{id}/layers` | Verified | PLAT-P0-API | `crates/q-daemon/src/lib.rs` | `tests::models_and_layers_are_served_from_the_catalog` |
| API-002 | `GET /v1/tensors/{id}`, `/value` (exact) | Verified | PLAT-P0-API, AC-005 | `crates/q-daemon/src/lib.rs` | `tests::exact_value_route_returns_the_golden_scalar` |
| API-003 | `GET /v1/tensors/{id}/blocks` (exact window) | Verified | PLAT-P0-API | `crates/q-daemon/src/lib.rs` | `tests::block_route_returns_only_the_requested_window` |
| API-004 | `POST /v1/query` (scalar/slice execute; matmul plans) | Verified | PLAT-P0-API | `crates/q-daemon/src/lib.rs` | `tests::query_route_executes_scalars_and_plans_matmuls` |
| API-005 | 501 routes carry a requirement ID and an explanation | Verified | — | `crates/q-daemon/src/lib.rs` | `tests::unbuilt_routes_return_501_with_a_requirement_id` |
| API-006 | Shape mismatch is a 400, before any read | Verified | AC-009 | `crates/q-daemon/src/lib.rs` | `tests::a_shape_mismatch_is_a_400_before_any_read` |
| API-007 | Ambiguous alias is a 409 carrying its candidates | Verified | PLAT-P1-WQL | `crates/q-daemon/src/lib.rs` | `tests::an_ambiguous_alias_is_a_409_carrying_its_candidates` |
| API-008 | Startup ingests metadata only | Verified | AC-001 | `crates/q-daemon/src/lib.rs` | `tests::bootstrap_ingests_metadata_only` |

## SEC — security boundaries

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| SEC-001 | Local file access confined to configured model roots | Verified | — | `crates/q-source/src/local.rs`, `crates/q-daemon/src/lib.rs` | `local::tests::path_traversal_is_refused`, `daemon::tests::a_traversal_attempt_never_escapes_a_root`, `the_model_root_boundary_is_enforced` |
| SEC-002 | No arbitrary code execution in WeightQL (Rust) | Verified | — | `crates/q-weightql/src/parser.rs` | `parser::tests::arbitrary_code_execution_constructs_are_rejected` |
| SEC-003 | No arbitrary code execution in WeightQL (browser) | Verified | — | `apps/web/query-interface/src/weightql.ts` | `rejects_arbitrary_code_execution_constructs` |
| SEC-004 | `mm`'s `eval` path is not carried forward | Verified | — | `docs/CURRENT_ARCHITECTURE.md` §5, `docs/decisions/ADR-006-*.md` | Absence of any `eval` in `apps/web/`; SEC-003 |
| SEC-005 | No SQL injection surface in catalog filters | Verified | — | `crates/q-catalog/src/lib.rs` | Every caller-supplied filter value is a bound parameter; the only interpolated strings are enum-derived `&'static str`. Exercised by `tests::shape_dtype_and_resolution_filters_work` |

## CHAT — query interface

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| CHAT-001 | Chat assistant (plan + I/O estimate before execution) | **Not Started** | PLAT-P1-CHAT | — | Deliberately not built; see `apps/web/query-interface/README.md` |
| CHAT-002 | Client-side WeightQL parser matching the Rust grammar | Verified | — | `apps/web/query-interface/src/weightql.ts` | 8 tests in `__tests__/weightql.test.ts` |
| CHAT-003 | KaTeX preview of the parsed AST | Verified | — | `apps/web/query-interface/src/katex-preview.ts` | `renders_the_grouping_the_parser_chose_not_the_source_order` |
| CHAT-004 | Error caret at the offending character | Verified | — | `apps/web/query-interface/src/app.ts` | `reports_a_caret_under_the_offending_character` |

## JOB — conversion jobs

| ID | Description | Status | Maps to | Files | Test(s) |
| --- | --- | --- | --- | --- | --- |
| JOB-001 | Job state machine, with illegal transitions rejected | Verified | — | `crates/q-catalog/src/job.rs` | `job::tests::legal_transitions_are_accepted`, `illegal_transitions_are_rejected`, `failed_and_cancelled_jobs_can_resume` |
| JOB-002 | Job runner (anything that actually executes a job) | **Stub** | — | `crates/q-daemon/src/lib.rs` | 501 with this ID |
| JOB-003 | Job persistence and reload | Verified | — | `crates/q-catalog/src/lib.rs` | `tests::jobs_persist_and_reload_with_their_state` |

## AC — ARCHITECTURE.md §18 acceptance criteria

| ID | Criterion | Status | Test(s) |
| --- | --- | --- | --- |
| AC-001 | Do not load the entire checkpoint into RAM | Verified | `the_whole_slice_reads_a_negligible_fraction_of_the_checkpoint`, `ingestion_reads_only_headers_not_payload`, `CAT-006` |
| AC-002 | Successfully parse sharded SafeTensors | Verified | `ingest::tests::ingests_a_sharded_checkpoint` |
| AC-003 | Metadata import can be cancelled and resumed | Verified | `cancellation_stops_at_a_shard_boundary`, `resume_skips_completed_shards` |
| AC-004 | Clicking a visual cell returns the correct tensor address | **Partial** | Addressing is Verified (`GRID-001`, `NSIR-004`); there is no viewer to click (`CESIUM-005`) |
| AC-005 | **The exact scalar matches the Python SafeTensors reference** | Verified | `tests/tests/end_to_end_scalar_slice.rs` — 4 tests over 6 golden scalars, 2 golden slices, and 2 bf16 tensors |
| AC-006 | Zooming out does not load exact values | Verified | `never_reads_exact_values_from_camera_movement_alone` (policy is tested; no renderer exercises it yet) |
| AC-007 | Zooming in only reads the necessary byte ranges | Verified | `scalar_read_touches_only_dtype_width_bytes`, `slice_read_matches_golden_and_reads_only_the_window` |
| AC-008 | The cache is reused after reopening | Verified | `l2_is_reused_after_reopen`, `catalog_survives_close_and_reopen`, `the_slice_is_reproducible_across_a_full_reopen` |
| AC-009 | A shape-mismatched expression is rejected before execution | Verified | `shape_mismatch_is_rejected_before_execution`, `a_shape_mismatch_is_a_400_before_any_read` |
| AC-010 | The UI clearly indicates exact / sampled / approximate | **Partial** | The *data model* carries fidelity end to end and is Verified (`SRC-018`, `STAT-005`, `TILE-008`, API responses); no UI renders it yet |

---

## Summary

133 requirement rows:

| Status | Count |
| --- | --- |
| Verified | 106 |
| Stub (returns `NotImplemented` carrying its own ID) | 10 |
| Not Started | 8 |
| Hardware-Unverified | 5 |
| Implemented (works, no dedicated test) | 1 |
| Partial | 2 |
| Split status (`GPU-003` — Metal implemented and unverified, wgpu not started) | 1 |

106 + 10 + 8 + 5 + 1 + 2 + 1 = 133. `GPU-003` is counted on its own line rather than
folded into either neighbour: its two backends are at genuinely different stages, and
filing the row under one of them would overstate the other.

## What a reader should not be surprised by

* **Nothing renders.** There is no tileset, no GLB, and no CesiumJS viewer.
* **No CUDA kernel has ever run.** The `.cu` files have not been compiled.
* **Matrix multiplication does not execute.** It parses, resolves, shape-checks,
  and estimates I/O — then stops, and says why.
* **Statistics are computed but not stored.** `q-statistics` and
  `q_gpu::CpuBackend` work; nothing has run a statistics pass, so
  `tensor_statistics` is empty and the API returns 501.
* **The cache is not wired in.** L1 and L2 work and are tested; no query path
  calls them yet.
* **Trillion-scale means metadata.** `CAT-006` proves that a 10¹²-parameter
  *manifest* indexes and queries in 35.7 MB. It proves nothing about loading
  weights, because that is not possible and is not claimed anywhere.
