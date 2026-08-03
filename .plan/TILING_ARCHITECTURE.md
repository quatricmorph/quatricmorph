# TILING_ARCHITECTURE — LOD, blocks, `.qtile`, GLB, `tileset.json`

## 1. The LOD ladder

Six levels, already a closed enum (`q_tensor_runtime::Lod`, `TILE-001` verified —
`the_ladder_has_exactly_six_levels`).

| LOD | Object | Data at this level | Fidelity | Exact values? |
| --- | --- | --- | --- | --- |
| 0 | Model | Parameter count, total bytes, global value distribution, subsystem bounds | `aggregate` | No |
| 1 | Subsystem | Layer ranges, aggregate norms, module counts | `aggregate` | No |
| 2 | Layer | Tensor counts, layer statistics, anomaly summaries | `aggregate` | No |
| 3 | Tensor | Shape, dtype, histogram, norms, block layout | `aggregate` | No |
| 4 | Tensor block | Block statistics, quantized samples, exact-value availability | `quantized` / `sampled` | No |
| 5 | Scalar region | Selected exact values, selected small slices | `exact` | **Yes, and only here** |

`Lod::carries_exact_values()` returns true only for level 5. This is the
mechanism behind `AC-006` — *zooming out does not load exact values* — because
the traversal cannot even ask for them above LOD 5.

### 1.1 Parent–child relationships

```text
LOD 0 model
 └ LOD 1 subsystem      (embedding · transformer stack · output head · router)
    └ LOD 2 layer       (one per layer index)
       └ LOD 3 tensor   (Q, K, V, O, MLP up/gate/down, norms, experts)
          └ LOD 4 block (256×256 by default; grid over the tensor)
             └ LOD 5 region (selected sub-extent, on demand only)
```

`Lod::parent()` / `Lod::child()` already implement the ladder navigation. LOD 5
tiles are **never pre-generated** — they exist only as a response to a selection
or a query, which is why the pyramid a conversion produces stops at LOD 4.

---

## 2. Block layout

### 2.1 Choosing a block size

Default `256 × 256`; `512 × 512` allowed. The choice is a function, not a
constant, and `QM-0040` implements it:

```text
block_elements     = rows × columns
block_bytes        = block_elements × dtype_width
instances_per_tile = block_elements                (one visual cell per scalar)

constraints:
  block_bytes        ≤ MAX_GPU_STAGING_BYTES / concurrent_blocks
  instances_per_tile ≤ MAX_INSTANCES_PER_TILE  = 262_144
  block_bytes        ≤ MAX_QTILE_PAYLOAD_BYTES = 256 MiB
  rows,columns       ≥ 1 and ≤ the tensor's own extent (BlockExtent::clamped_to)
```

At 256×256 f32: 65 536 cells, 256 KiB raw, 128 KiB quantized — comfortably inside
every ceiling, and equal to the workspace's default sphere count. At 512×512 f32:
262 144 cells, exactly `MAX_INSTANCES_PER_TILE`, 1 MiB raw. **512 is the largest
legal block**, and that is not a coincidence — the two limits were chosen to
coincide so no pipeline stage can produce something a later stage must reject.

Inputs to the decision: dtype width; tensor dimensions (a `[128, 48]` tensor gets
one clamped block, not a padded 256×256 one); the CUDA or CPU memory budget;
desired LOD depth; the GLB size target; Cesium traversal behaviour; picking
granularity; query granularity.

### 2.2 Edge blocks

`BlockExtent::clamped_to(shape)` already handles the ragged edge. Blocks are
**clamped, never padded** — a padded block would put fabricated zeros into a
statistic, and `ARCHITECTURE.md` §19's prohibition on implying data that does not
exist applies as much to a mean as to a picture.

The `mm` blocking code carried the same insight: `viz.js:386-400`'s `grid`
skipped a block whose `start >= max`, with the comment *"dead final block when
size * n - max > size"*. That logic is now
`apps/web/matrix-workspace/src/math/blocking.ts`, tested (`MATMUL-002`).

### 2.3 Byte-range planning

`TensorBlock::plan` derives **one byte run per row** for a row-major tensor,
without reading anything (`block_planning_derives_one_byte_run_per_row`). For a
256-column window of a 4096-column f32 tensor, that is 256 runs of 1 024 bytes
each, at a stride of 16 384 — which is what makes a block read cost 256 KiB
instead of the 4 MiB a naive row-span read would cost.

`SourceByteRanges::total_bytes()` is the I/O cost estimate every plan and job
quotes before doing anything.

---

## 3. Geometric error and bounding volumes

### 3.1 Geometric error

```text
geometric_error(lod) = ROOT_GEOMETRIC_ERROR / 2^lod     ROOT = 1024.0
LOD 0 → 1024   1 → 512   2 → 256   3 → 128   4 → 64   5 → 32
```

Implemented in `q_tileset::GeometricError::for_lod` and **hand-mirrored** in
`apps/web/model-viewer/src/lod-policy.ts:102`. Closing that duplication is
`QM-0004`/`QM-0005`; see [`REPOSITORY_ANALYSIS.md`](REPOSITORY_ANALYSIS.md) §5.

Monotonic decrease is not a style preference. `TilesetNode::validate_refinement`
rejects a child whose error is ≥ its parent's, because such a child never refines
and its entire subtree becomes unreachable — an invisible bug in a hand-built
tileset. Two tests hold this: `geometric_error_halves_down_the_ladder` and
`a_child_that_never_refines_is_rejected`.

### 3.2 Bounding volumes

3D Tiles `box` form: `center: [f64;3]` + `half_axes: [f64;3]`, already the shape
of `q_tileset::BoundingBox` and of `schemas/visualization/schema.json`'s
`bounding_box`.

**Bounds are derived from the shared grid layout**, not fitted to geometry:

```text
tensor bounds  = tensor_anchor ± (extent × cellSize + framePadding) / 2
block bounds   = tensor_anchor + block_origin × cellSize ± (block_extent × cellSize) / 2
layer bounds   = union of its tensors' bounds, snapped outward to majorGridInterval
model bounds   = union of layer bounds
```

A parent's box contains its children's by construction. If it did not, Cesium
would cull a visible child, and the failure would look like a rendering glitch
rather than a layout bug. `QM-0040` asserts containment.

---

## 4. Refinement and traversal

| Property | Choice | Why |
| --- | --- | --- |
| Refinement | `REPLACE` | A block's detail replaces its tensor's summary. `ADD` would draw both, doubling the instance count at every level |
| Tiling | **Explicit** for the MVP | Implicit tiling needs a uniform, complete subdivision; a model's tensor set is neither uniform nor complete. `ADR-CANDIDATE-008`; the node type carries the fields implicit tiling would need, so adopting it later is additive |
| Availability | Every emitted node has content or children | A node with neither is a dead end that still costs a traversal step |
| Content | `.glb` at LOD 1–4; LOD 0 may be metadata-only | A model-level tile has nothing to draw but its bounds and label |

**Prefetch policy** (`ARCHITECTURE.md` §13.3, already encoded in
`lod-policy.ts` and tested): load the current tile → prefetch children →
prefetch sibling metadata → **do not fetch exact values**. Exact ranges are read
only on explicit selection, an exact query, an analysis pass, or a
multiplication that names the block. `never_reads_exact_values_from_camera_movement_alone`
and `reads_exact_values_only_on_an_explicit_selection` are the standing proof.

---

## 5. `.qtile` in the pyramid

The format is done ([`DATA_ARCHITECTURE.md`](DATA_ARCHITECTURE.md) §6). What the
MVP adds is generation.

| LOD | What the `.qtile` holds | Encoding |
| --- | --- | --- |
| 0–2 | Aggregate statistics for the subject; no per-cell payload, or a small histogram | `RawF32` over a tiny cell set |
| 3 | Tensor-level histogram, norms, and a coarse downsample of the tensor | `QuantizedI16` |
| 4 | Per-block statistics and a quantized sample of the block's cells | `QuantizedI16` or `MortonSparseI16` |
| 5 | Exact values for a selected extent, on demand, never pre-generated | `RawF32` |

`MortonSparseI16` is preferred at LOD 4 when a block is sparse enough that
skipping zero cells saves more than the 4-byte coordinate costs — measured, per
block, in `QM-0041`, not assumed. Its 8 bytes per cell versus `QuantizedI16`'s 2
means the break-even is around 25 % density.

---

## 6. GLB tile content

### 6.1 Structure

```text
tile_<lod>_<i>_<j>.glb
├── one shared unit mesh                    (sphere at MVP quality, or a quad for sprites)
├── EXT_mesh_gpu_instancing
│   ├── TRANSLATION   per instance, derived from Morton coordinate × cellSize
│   ├── SCALE         per instance, from |value| via the documented mapping
│   └── _FEATURE_ID_0 per instance
├── EXT_mesh_features                        featureId → tile-local index
├── EXT_structural_metadata                  value class, sign, fidelity, block coords
└── extras: { qtile_uri, tensor_id, tile_id, block_extent, canonical_address }
```

Instance transforms carry position and scale; **colour and opacity are applied in
the viewer**, from the quantized value class plus the user's palette and
normalization. Baking colour into the GLB would put a display choice into a cache
key and force regeneration whenever a palette changes — `ARCHITECTURE.md` §13.2
explicitly excludes the palette from the key for this reason.

### 6.2 Ceilings

```text
MAX_INSTANCES_PER_TILE = 262_144      # q_gltf, enforced by GlbTileSpec::validate
```

Above it the tile **refines into children**; it never grows. Two existing tests
enforce the boundary: `cube_per_weight_explosions_are_refused` and
`a_glb_without_a_qtile_sidecar_is_refused`.

Target GLB size at 256×256: 65 536 instances × (12 B translation + 12 B scale +
4 B feature ID) ≈ **1.8 MB**, before compression. That is a reasonable tile. At
512×512 it is 7 MB, which is why 512 is the ceiling rather than the default.

### 6.3 Extension compatibility

`ARCHITECTURE.md` §10.2 warns against assuming renderer support.
`CESIUM-010` / `QM-0057` adds a **capability probe** at viewer start:

1. Load a 3-instance probe GLB using each extension.
2. Record which loaded, which rendered, and which silently produced nothing.
3. Select the emission profile accordingly, and **show the selected profile in
   the dev panel**.

Fallback ladder, most to least capable:

| Profile | Uses | Cost |
| --- | --- | --- |
| A | `EXT_mesh_gpu_instancing` + `EXT_mesh_features` + `EXT_structural_metadata` | Best; one draw call per tile |
| B | `EXT_mesh_gpu_instancing` + `_FEATURE_ID_0` attribute only | Metadata resolved by daemon lookup instead of in-tile |
| C | Merged geometry, one primitive per tile, vertex-attribute feature IDs | Larger tiles, still one draw call |

Profile C is the floor: it uses no extension beyond core glTF 2.0 and therefore
cannot fail for extension reasons. A tile is never emitted per scalar in any
profile.

---

## 7. `tileset.json`

```jsonc
{
  "asset": { "version": "1.1" },
  "geometricError": 1024.0,
  "root": {
    "boundingVolume": { "box": [ /* center xyz, 3 half-axes */ ] },
    "geometricError": 1024.0,
    "refine": "REPLACE",
    "content": { "uri": "tiles/model.glb" },
    "children": [ /* subsystem → layer → tensor → block */ ]
  }
}
```

Emitted by `q_tileset::TilesetBuilder`, which today refuses
(`the_builder_refuses_rather_than_emitting_a_fake_tileset`). `QM-0044` implements
it. Validation is against the published 3D Tiles JSON schema, not only against
our own round trip (`QM-0046`).

Non-geospatial placement: the model sits in a local ENU frame at a fixed origin.
The viewer disables the globe, terrain, imagery, atmosphere, and the sun.
Cesium's GIS assumptions are the price of its traversal engine; the plan pays it
explicitly rather than fighting it (`ADR-CANDIDATE-009`).

---

## 8. Incremental generation

Conversion of a large model runs for a long time. The pipeline is built to be
interrupted.

### 8.1 Unit of work

**One block.** After each block the job records: block ID, content hash, output
URIs, bytes read, bytes written, and elapsed time. A crash costs at most one
block.

### 8.2 Atomic output

```text
write   → <name>.qtile.tmp.<job_id>
fsync
rename  → <name>.qtile                     atomic on the same filesystem
```

**A partially written `.qtile` or GLB must never be visible under its final
name.** `tileset.json` is written last, after every tile it references exists, so
a tileset on disk is always complete. Orphaned `.tmp.<job_id>` files are swept
when a job resumes or is cancelled (`TILE-011`, `QM-0045`).

### 8.3 Resume

A resumed job re-reads its block manifest, verifies each completed block's
content hash against its output, and skips the ones that match. A mismatch is
re-done, not trusted. This is the same shape as the verified ingestion resume
(`resume_skips_completed_shards`).

### 8.4 Cache reuse

Before converting a block, the executor computes its `CacheKey` and checks L1
then L2. A hit skips the compute and the write entirely. Because the key includes
`algorithm_version` and the encoding, changing either invalidates cleanly instead
of serving a stale artifact (`CACHE-008`, `QM-0032`).

---

## 9. Validation

Three layers, because each catches what the others cannot.

| Layer | Tool | Catches |
| --- | --- | --- |
| Round trip | Our own encode/decode | Our own bugs. Already verified for `.qtile` |
| Schema | 3D Tiles 1.1 JSON schema; `schemas/visualization/schema.json` | Shape drift between producer and consumer |
| External | Khronos `gltf-validator`; `3d-tiles-validator` | Everything we did not think to test — the class of bug that makes a file load in our reader and fail in Cesium |

`QM-0046` wires all three into CI as a job that runs the generator over the
fixture and validates its output. Without the external layer, "valid GLB" means
"valid according to the code that wrote it".

---

## 10. Requirements

| ID | Requirement | Task |
| --- | --- | --- |
| `TILE-009` | Bounded streaming block reader with named budgets | `QM-0030` |
| `TILE-004` | `.qtile` pyramid generation for a model | `QM-0041` |
| `TILE-010` | LOD ladder and block-layout planner; bounds containment asserted | `QM-0040` |
| `TILE-011` | Atomic output and resume manifests | `QM-0045` |
| `TILE-012` | External artifact validation in CI | `QM-0046` |
| `GLB-001` | Instanced GLB tile emission | `QM-0042` |
| `GLB-004` | Feature IDs and structural metadata | `QM-0043` |
| `CESIUM-001` | `tileset.json` generation | `QM-0044` |
| `CESIUM-010` | glTF extension capability probe and fallback profiles | `QM-0057` |
