# DATA_ARCHITECTURE — planes, identities, exactness, versioning

## 1. The four planes

`ARCHITECTURE.md` §2.1 is implemented, not aspirational: every module declares
its plane in a top-of-file doc comment, and `q-gltf` enforces the boundary with a
test rather than a convention.

```text
┌ ARTIFACT PLANE ─────────────────────────────────── immutable, never rewritten ┐
│ config.json · tokenizer.json · model.safetensors.index.json                   │
│ model-0000N-of-XXXXX.safetensors                                              │
│ Access: SafeTensors header parse, mmap, byte range. Read-only, always.        │
│ Crates: q-source, q-safetensors                                               │
└──────────────────────────────────────────────────────────────────────────────┘
        │ header bytes only — ~20 KB of a 1.2 MB checkpoint
        ▼
┌ METADATA PLANE ─────────────────────────────────────── small, queryable, SQL ┐
│ models · tensors · tensor_blocks · tensor_statistics · visual_tiles           │
│ conversion_jobs · (cache entries, on disk) · expressions · query plans        │
│ Crates: q-architecture, q-nsir, q-catalog, q-tensor-runtime,                  │
│         q-expression, q-weightql                                             │
└──────────────────────────────────────────────────────────────────────────────┘
        │ block plans (byte ranges), never payload
        ▼
┌ TENSOR TILE PLANE ──────────────────────── multiresolution, tensor-native ────┐
│ *.qtile — global/layer/tensor/block statistics, quantized samples,            │
│ exact selected blocks, Morton coordinates, logical extents                    │
│ Crates: q-tiles, q-statistics.  THIS is the authoritative tensor sidecar.     │
└──────────────────────────────────────────────────────────────────────────────┘
        │ visual encoding only
        ▼
┌ VISUALIZATION PLANE ──────────────────────────────────── render-only ─────────┐
│ tileset.json · *.glb · bounding volumes · geometric errors                    │
│ instance transforms · feature IDs · quantized visual classes · camera presets │
│ Crates: q-tileset, q-gltf.  NEVER the authoritative store for values.         │
└──────────────────────────────────────────────────────────────────────────────┘
```

**The load-bearing rule:** a GLB may never be the only carrier of tensor values.
`q_gltf::GlbTileSpec::validate` requires a `qtile_uri`, and
`a_glb_without_a_qtile_sidecar_is_refused` fails the build if that is relaxed.
The reason is not purity: a GLB is a rendering container whose contents are
lossy, reordered, and re-encoded by tooling. Anything reproducible must live in
the tile plane, where the format is ours and the round trip is byte-exact.

---

## 2. Artifact plane

| Concern | Implementation |
| --- | --- |
| Source abstraction | `q_source::ModelSource` — `manifest()` + `read_range(uri, offset, len)` |
| Local access | `crates/q-source/src/local.rs`, memory-mapped, root-confined |
| Remote access | `crates/q-source/src/http.rs` — **range arithmetic verified, transport refuses** (`SRC-008`, extension point) |
| Shard resolution | `crates/q-safetensors/src/index.rs` from `model.safetensors.index.json` |
| Header parsing | `crates/q-safetensors/src/header.rs`; `__metadata__` is not a tensor |
| Identity | `source_uri`, `source_revision`, `source_hash` on the `models` row |
| Validation | Absurd header length refused before allocating; offsets bounds-checked against file length and against the tensor's own extent |

**Never**: rewritten, normalized, re-serialized, or copied into another plane.
Conversion outputs go beside the checkpoint or into a cache directory, never over
it.

---

## 3. Metadata plane

SQLite (`ADR-003`), `CURRENT_SCHEMA_VERSION = 1`, six tables plus
`schema_migrations`, with indices already in place.

| Table | Key columns | Indices |
| --- | --- | --- |
| `models` | `model_id`, `source_uri`, `source_revision`, `source_hash`, `architecture`, `parameter_count`, `layer_count`, `hidden_size`, `imported_at` | — |
| `tensors` | `tensor_id`, `model_id`, `raw_name`, `canonical_name`, `layer_index`, `component`, `role`, `shape`, `dtype`, `shard_uri`, `byte_start`, `byte_length`, `parameter_count` | `(model_id, layer_index)`, `(model_id, canonical_name)`, `(model_id, role, layer_index)` |
| `tensor_blocks` | `block_id`, `tensor_id`, `lod`, row/column extents, `source_byte_ranges`, `statistics_id`, `content_hash` | `(tensor_id, lod)` |
| `tensor_statistics` | `statistics_id`, `subject_id`, count, min, max, mean, variance, L1, L2, ratios, `histogram`, **`approximate`**, `algorithm_version` | `(subject_id)` |
| `visual_tiles` | `tile_id`, `parent_tile_id`, `model_id`, `tensor_id`, `lod`, `bounds`, `geometric_error`, `qtile_uri`, `glb_uri`, `child_count` | `(model_id, lod)` |
| `conversion_jobs` | `job_id`, `model_id`, state, phase, cursor, counters, timestamps | `(model_id, state)` |

Matches `ARCHITECTURE.md` §5 field for field.

### 3.1 What is missing, and which task fills it

| Gap | Task |
| --- | --- |
| `tensor_statistics` is never written | `QM-0020` |
| `visual_tiles` is never written; no tile↔tensor resolution query | `QM-0021` |
| `tensor_blocks` is never populated by a conversion | `QM-0022` |
| `models.hidden_size` / `layer_count` / `parameter_count` not filled from `config.json` | `QM-0012` |

### 3.2 Migration policy

`migrate()` is idempotent and **refuses a database newer than the build**
(`a_future_schema_is_refused_rather_than_corrupted`). Every schema change in this
plan is a numbered migration appended to `crates/q-catalog/src/schema.rs` with a
version bump; no task alters an existing migration, because a shipped migration
has already run on someone's disk.

---

## 4. NSIR — canonical identity

Raw name → semantic record → canonical address.

```text
model.layers.10.self_attn.q_proj.weight
        ↓ architecture resolver (generic | llama | qwen)
{ stack: "language", layer: 10, component: "attention",
  operation: "query_projection", parameter: "weight",
  axes: ["output_channel", "input_channel"] }
        ↓
model.layers[10].self_attention.query_projection.weight
```

Three rules, all already enforced:

1. **A resolver may return `unknown`** and must, when it was not taught the name
   (`generic_resolver_returns_unknown_for_names_it_was_not_taught`).
2. **A resolver never infers a role from shape.** Two tensors with identical
   shapes are not thereby the same role.
3. **An ambiguous alias returns candidates**, never a silent pick
   (`ambiguous_alias_returns_candidates_not_a_silent_pick`; the daemon surfaces
   this as `409` with the candidate list).

The canonical address is the **universal join key** — catalog rows, URLs,
WeightQL, chat responses, Cesium feature metadata, workspace selections, logs,
cache keys, and query plans all use the same string.

Alias forms supported (`NSIR-005`): `Q[10]`, `Q[10][100,42]`,
`K[10][0:256,0:256]`, `MLP.down[24][:]`, `Expert[12,37].up[0:128,:]`.
Contextual selectors like `layer[0][10].attention[1].Q[0]` are accepted at the UI
boundary but **must resolve to a canonical address before execution**.

---

## 5. Identities

| ID | Type | Derivation | Stability guarantee |
| --- | --- | --- | --- |
| `ModelId` | 16 bytes | Hash of source identity | Stable across reopen (`SRC-006`) |
| `TensorId` | 16 bytes | Hash of `(model_id, raw_name)` | Stable across reopen; independent of shard layout |
| `TileId` | 16 bytes | `TileId::for_block(tensor, lod, extent)` | Stable; sensitive to extent and LOD (`TILE-003`) |
| `BlockId` | derived | `(tensor_id, lod, extent)` | Same inputs as `TileId`; one is the catalog key, one is the artifact key |
| `StatisticsId` | derived | `(subject_id, algorithm_version)` | Changing the algorithm changes the ID rather than overwriting history |
| `PlanId` | string | Deterministic hash of the resolved plan | Quotable in logs and chat (`WQL-012`) |
| `FeatureId` | u32, tile-local | Index within a GLB tile | Resolves to `(TileId, local index)` → block → tensor |

**Why `TileId` is extent-sensitive:** if two different blockings of the same
tensor produced the same tile ID, a cache hit would serve the wrong geometry.
The test `tile_ids_are_stable_and_sensitive_to_extent_and_lod` exists for exactly
this failure mode.

---

## 6. Tensor tile plane — `.qtile`

Implemented, v1, `TILE-005`…`TILE-008` verified.

```text
magic    "QTILE\0\0\0"        8 bytes
header                        72 bytes total, little-endian regardless of host
  version u16                 1
  encoding u16                RawF32 | QuantizedI16 | MortonSparseI16
  lod u8                      0..5
  dimensions u8               rank of the covered region (2 for a matrix block)
  count u32                   cells in the payload
  tensor_id [u8;16]
  origin [u32;3]              logical [row, column, depth] within the tensor
  extent [u32;3]              logical [rows, columns, depth]
  min_value f32
  max_value f32
payload                       count × bytes_per_cell
  RawF32           4 B/cell   exact
  QuantizedI16     2 B/cell   lossy — declares itself so
  MortonSparseI16  8 B/cell   morton u32 + quantized i16 + flags u16
```

Guarantees already tested: byte-exact round trip; exact f32 preservation;
little-endian on any host; 8 distinct corruption classes rejected; a payload
claim above `MAX_QTILE_PAYLOAD_BYTES = 256 MiB` refused as corrupt or hostile.

**Position is never stored.** `MortonSparseI16` stores a Morton coordinate, from
which the renderer derives position:
`position = tile_origin + decode_morton(coord) × cell_spacing`. Four bytes
instead of twelve, and no possibility of drift between the stored position and
the logical index.

**MVP delta:** nothing about the format. What is missing is a *pyramid* —
something that generates the LOD 0–5 set for a model (`TILE-004`, `QM-0041`).

---

## 7. Visualization plane

| Artifact | Contents | Never contains |
| --- | --- | --- |
| `tileset.json` | 3D Tiles 1.1 hierarchy, bounding volumes, geometric errors, content URIs, refinement | Values |
| `*.glb` | One shared unit mesh; instance transforms; quantized visual class; feature IDs; tile-local metadata; a reference to its `.qtile` | Full FP16/BF16 weights; one mesh per parameter; duplicated cube geometry; the authoritative catalog; reproducible analysis results |

Guardrails in force today: `MAX_INSTANCES_PER_TILE = 262_144`;
`cube_per_weight_explosions_are_refused`; `a_glb_without_a_qtile_sidecar_is_refused`;
`geometric_error_halves_down_the_ladder`; `a_child_that_never_refines_is_rejected`.

glTF extensions under evaluation — `EXT_mesh_gpu_instancing`,
`EXT_mesh_features`, `EXT_structural_metadata` — are **capability-probed at
runtime with a fallback path** (`CESIUM-010`, `QM-0057`). `ARCHITECTURE.md`
§10.2 explicitly warns not to assume the renderer supports them.

---

## 8. Exactness — a type, not a label

The single most important cross-cutting property. `AC-010` requires the UI to
distinguish these, and `SRC-018` already makes access scale a **type**, so the
compiler participates.

| Fidelity | Meaning | Produced by |
| --- | --- | --- |
| `metadata` | Shape, dtype, address, byte range. No values were read | Catalog queries, header parse |
| `aggregate` | A statistic over a region, computed from all its values | Full-block statistics pass |
| `sampled` | A statistic or preview computed from a subset | Sampled statistics; `mark_approximate` |
| `quantized` | Values present but lossily encoded | `QuantizedI16` / `MortonSparseI16` tiles |
| `exact` | The values as stored in the checkpoint | Byte-range reads; `RawF32` tiles |

Rules:

* Every API response carries its fidelity. Every UI surface renders a badge.
* Fidelity **degrades monotonically** through a pipeline: a statistic over
  quantized data is at best `sampled`, never `aggregate`.
* `Lod::carries_exact_values()` is true **only at the finest level**
  (`only_the_finest_level_carries_exact_values`).
* `AccessScale` makes it a compile-time error for a metadata-scale operation to
  read payload, and visualization scale is never exact.
* A sampled tile must never be displayed in a way that implies completeness —
  the task specification §14's explicit prohibition.

---

## 9. Cache keys

`ARCHITECTURE.md` §13.2, implemented as `q_cache::CacheKey` with **length-prefixed
components** so field boundaries cannot collide (a real hazard: `("ab","c")` and
`("a","bc")` must not hash alike).

```text
key = blake3(
    source_model_hash ‖ tensor_id ‖ logical_slice ‖ lod
  ‖ statistics_algorithm ‖ algorithm_version
  ‖ quantization_encoding ‖ visualization_encoding
)
```

**Excluded deliberately:** colour palette, normalization range, and any purely
visual parameter the browser shader can apply dynamically. Including them would
multiply the cache by the number of palettes for no benefit.

Tiers: L0 GPU (extension point) · L1 in-process LRU, by count and by bytes ·
L2 content-addressed on disk with an 8 GiB default and eviction · L3 browser
(extension point) · L4 remote (extension point). L1 and L2 survive reopen
(`CACHE-004`, `AC-008`). **Nothing calls them yet** — `CACHE-008`, task
`QM-0032`.

---

## 10. Versioning

| Artifact | Version carrier | Compatibility rule |
| --- | --- | --- |
| Catalog | `schema_migrations` + `CURRENT_SCHEMA_VERSION` | Idempotent forward migration; a newer database is refused, never opened |
| `.qtile` | `QTILE_VERSION` in the header | A newer version is refused with a clear error; readers never guess a layout |
| `tileset.json` | `TILES_VERSION = "1.1"` | Emitted version is explicit |
| JSON schemas | `$id` ending `/v1` | A breaking change mints `/v2`; both may exist during migration |
| Statistics | `algorithm_version` on every row | A change mints new rows and new cache keys; old results stay readable and comparable |
| HTTP API | `/v1/` path prefix | Additive changes only within a major version |
| Spatial contract | version field added by `QM-0004` | Rust and TypeScript conformance tests fail on mismatch |

**No artifact is silently upgraded.** Every one of these refuses rather than
reinterpreting, which is the same discipline `SRC-014` applies to unknown dtypes
and `NSIR-001` applies to unknown roles.
