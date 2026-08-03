# QM-0042 — Instanced GLB tile builder

## Status

Blocked

Unblocks when `QM-0041` reaches `Complete`.

## Phase

Phase 04 — Tensor tiles, GLB, and tileset

## Objective

Emit GLB tile content using GPU instancing over one shared mesh — never a
primitive per scalar.

## Repository Evidence

* `crates/q-gltf/src/lib.rs:85` — `UnimplementedGlbBuilder`;
  `the_builder_refuses_rather_than_emitting_a_placeholder_glb`.
* `:33` `GlbTileSpec` with `validate()`; `:77` `MAX_INSTANCES_PER_TILE = 262_144`.
* `cube_per_weight_explosions_are_refused` and
  `a_glb_without_a_qtile_sidecar_is_refused` — both Verified.
* `ARCHITECTURE.md` §10.2 — `EXT_mesh_gpu_instancing`, **with a warning to check
  actual renderer support and have a fallback**.
* §11.2 — send only tile origin, extent, quantized values, selection mask, filter
  and normalization parameters; placement and colour on the GPU.
* `ADR-CANDIDATE-017` — profiles A → B → C.

## Requirements Covered

`GLB-001`, `CESIUM-012`, `MVP-14`.

## Dependencies

`QM-0041`.

## Blocks

`QM-0043`, `QM-0044`, `QM-0046`.

## Parallelization

Lane A, sequential after `QM-0041`.

## Program Boundary

`crates/q-gltf`, `crates/q-daemon`.

## Scope

* `InstancedGlbBuilder` emitting profile A: one shared unit mesh +
  `EXT_mesh_gpu_instancing` with `TRANSLATION`, `SCALE`, `_FEATURE_ID_0`.
* Profiles B and C as selectable emission modes.
* Translation from the Morton coordinate × `cellSize`; scale from `|value|`.
* **Colour and opacity are not baked** — the viewer applies them.
* `extras`: `qtile_uri`, `tensor_id`, `tile_id`, `block_extent`,
  `canonical_address`.
* Serve `GET …/tiles/{tileId}.glb`.

## Out of Scope

`EXT_mesh_features` / `EXT_structural_metadata` (`QM-0043`) · the capability
probe (`QM-0057`) · `tileset.json` (`QM-0044`) · external validation
(`QM-0046`).

## Files Expected to Change

* `crates/q-gltf/src/lib.rs`
* `crates/q-daemon/src/lib.rs`

## Files Expected to Add

* `crates/q-gltf/src/instanced.rs`
* `crates/q-gltf/tests/glb_emission.rs`

## Files Expected to Remove or Deprecate

* `q_daemon::glb_tile_501` — replaced. `UnimplementedGlbBuilder` **stays** as the
  profile-unavailable fallback and keeps its refusal test.

## Data Contracts

```text
tile_<lod>_<i>_<j>.glb
├── one shared unit mesh                    (sphere at MVP quality)
├── EXT_mesh_gpu_instancing
│   ├── TRANSLATION   f32×3 per instance    from Morton × cellSize
│   ├── SCALE         f32×3 per instance    from |value|
│   └── _FEATURE_ID_0 u32   per instance
└── extras { qtile_uri, tensor_id, tile_id, block_extent, canonical_address }
```

**A GLB may never be emitted without a `qtile_uri`.** `GlbTileSpec::validate`
enforces it; the test stays.

## Memory and Performance Constraints

```text
bytes ≈ instances × 28 + shared mesh + JSON chunk
      ≈ 1.8 MB at 65 536 instances
      ≈ 7.3 MB at 262 144 (the ceiling)
MAX_GLB_BUFFER_BYTES = 64 MiB   — refuse rather than assemble something huge
```

Target: < 50 ms to build a 65 536-instance tile. One tile assembled in memory at
a time.

## Implementation Plan

1. Build the shared unit mesh once per model, referenced by every tile.
2. Read the `.qtile`; decode Morton coordinates and quantized values.
3. Fill `TRANSLATION` from `tile_origin + decode_morton(c) × cellSize`, taking
   `cellSize` from the spatial contract.
4. Fill `SCALE` from `|value|` via the documented mapping, clamped so
   `r_max ≤ 0.5 × cellSize`.
5. Assign sequential `_FEATURE_ID_0`, matching the `.qtile` cell order — so
   `local_index` is the same number in both artifacts.
6. Write `extras`; serialize GLB.
7. Add profile B and C emission paths.

## Error Handling

* `instance_count > MAX_INSTANCES_PER_TILE` → refuse; the planner should have
  subdivided, so this is a planner bug worth surfacing loudly.
* Missing `.qtile` → refuse (`GLB-003`).
* Buffer exceeding `MAX_GLB_BUFFER_BYTES` → refuse naming the budget.
* An unknown profile → refuse; never fall back silently, because a silent
  fallback changes what feature IDs mean.

## Acceptance Criteria

1. A 256×256 block emits a valid GLB of roughly 1.8 MB with 65 536 instances.
2. **One** mesh and one primitive per tile, regardless of instance count.
3. Feature IDs are sequential and match `.qtile` cell order.
4. `TRANSLATION` values are grid-snapped within `1e-6`.
5. `SCALE` never exceeds `0.5 × cellSize`.
6. No colour or opacity data is present in the GLB.
7. A GLB without a `qtile_uri` is refused.
8. 262 145 instances are refused.
9. Profiles B and C emit and are selectable.
10. Build time < 50 ms at 65 536 instances.

## Verification Plan

**Automated** — `glb_emission.rs`: instance counts, primitive count, snapping,
scale clamp, absence of colour, refusals.
**Manual** — open a tile in a glTF viewer and confirm it renders.

## Suggested Commands

```bash
cargo test -p q-gltf                                          # verified today
cargo run -p q-cli -- convert … --emit glb --profile A        # introduced here
npx gltf-validator out/<model>/tiles/<tile>.glb               # QM-0046 wires this into CI
```

## Test Cases

| Input | Expected |
| --- | --- |
| 256×256 block | 65 536 instances, 1 primitive, ~1.8 MB |
| Primitive count | 1, at any instance count |
| Feature ID *k* | Corresponds to `.qtile` cell *k* |
| Every `TRANSLATION` | Multiple of `cellSize` within `1e-6` |
| Max `SCALE` | ≤ `0.5 × cellSize` |
| Colour/opacity accessors | **Absent** |
| No `qtile_uri` | Refused |
| 262 145 instances | Refused |
| Profile C | Emits, no extensions used |

## Risks

| Risk | Mitigation |
| --- | --- |
| Feature IDs drift from `.qtile` order | Sequential by construction; asserted for corners |
| Baking colour creeps in for convenience | Test asserts colour accessors are absent |
| Extension unsupported by the renderer | Profiles B and C; probed in `QM-0057` |

## Completion Evidence

* File size and instance count for a generated tile.
* Primitive-count assertion.
* Snapping and scale-clamp test output.
* A screenshot from an external glTF viewer.
* Build timing.
