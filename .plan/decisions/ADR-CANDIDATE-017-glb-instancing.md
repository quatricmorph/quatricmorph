# ADR-CANDIDATE-017 — GLB instancing strategy

## Status

`Open`.

## Context

A tile carries up to 262 144 cells. Emitting one mesh per cell is forbidden;
emitting one draw call per cell would be unusable. How are cells encoded in a
GLB, and what happens when the renderer does not support the chosen mechanism?

## Repository evidence

* `crates/q-gltf/src/lib.rs:77` — `MAX_INSTANCES_PER_TILE = 262_144`.
* `GlbBuilder` trait at :80; `UnimplementedGlbBuilder` at :85 —
  `the_builder_refuses_rather_than_emitting_a_placeholder_glb`.
* `cube_per_weight_explosions_are_refused` and
  `a_glb_without_a_qtile_sidecar_is_refused` — both **Verified**.
* `ARCHITECTURE.md` §10.2 names `EXT_mesh_gpu_instancing` **and warns**:
  *"Quatricmorph must check the renderer's actual support level and have its own
  fallback."*
* §11.2 — send only tile origin, extent, quantized values, selection mask, filter
  and normalization parameters; **camera culling, cell placement, and colour
  mapping happen on the GPU**.
* `schemas/visualization/schema.json` — `glb_tile_spec` requires `tile_id`,
  `instance_count`, `value_encoding`, `qtile_uri`.

## Decision required

Which glTF extensions, and what is the fallback ladder?

## Options

| Option | |
| --- | --- |
| **A** | `EXT_mesh_gpu_instancing` + `EXT_mesh_features` + `EXT_structural_metadata` |
| **B** | Instancing only; metadata resolved by a daemon lookup |
| **C** | Merged geometry, one primitive per tile, feature IDs as a vertex attribute |
| **D** | Core glTF only, one mesh per cell |

## Advantages

* **A** — one draw call per tile; per-instance feature IDs; metadata travels in
  the tile; the 3D Tiles 1.1 native path.
* **B** — one extension instead of three; a smaller support surface.
* **C** — no extension at all; **cannot fail for extension reasons**.
* **D** — maximal compatibility.

## Disadvantages

* **A** — three extensions to support, and `ARCHITECTURE.md` warns that support
  is uneven.
* **B** — a network round trip per pick, and picking latency is a stated budget
  (< 50 ms).
* **C** — geometry is duplicated per cell, so a tile is several times larger.
* **D** — **this is the cube-per-weight explosion.** Already refused by a test.
  Disqualifying.

## Size comparison at 65 536 cells

| Profile | Bytes | Note |
| --- | --- | --- |
| **A** | 65 536 × 28 B ≈ **1.8 MB** | translation 12 + scale 12 + feature ID 4 |
| **C** | 65 536 × ~24 vertices × 12 B ≈ **19 MB** | ~10× larger |
| **D** | — | refused |

## Risks

[`RISK_REGISTER.md`](../RISK_REGISTER.md) R4 — extension support is worse than
assumed, and the failure mode may be *silent*: the loader accepts the file and
renders nothing.

## Recommended default

**A, with a probed fallback to B then C.**

```text
Profile A   EXT_mesh_gpu_instancing + EXT_mesh_features + EXT_structural_metadata
Profile B   EXT_mesh_gpu_instancing + a _FEATURE_ID_0 attribute; metadata by lookup
Profile C   merged geometry, one primitive per tile, vertex-attribute feature IDs
```

`QM-0057` probes at viewer start with a 3-instance GLB per extension, records
which **loaded**, which **rendered**, and which **silently produced nothing**, and
selects the emission profile. The active profile is shown in the dev panel.

Profile C is the floor: it uses no extension beyond core glTF 2.0 and therefore
cannot fail for extension reasons. **No profile emits a primitive per scalar.**

Per §11.2, instance transforms carry position and scale only. **Colour and
opacity are applied in the viewer** from the quantized value class plus the user's
palette and normalization — which is also why `ARCHITECTURE.md` §13.2 excludes the
palette from the cache key. Baking colour would force regeneration on every
palette change.

## Tasks affected

`QM-0042` (implements A), `QM-0043` (feature IDs and metadata), `QM-0057`
(probe and fallback), `QM-0046` (validates all emitted profiles).

## Decision deadline

Before `QM-0042`.
