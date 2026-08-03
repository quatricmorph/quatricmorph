# Phase 04 — Tensor tiles, GLB, and tileset

## Goal

```text
Tensor metadata and block summaries → .qtile → .glb → tileset.json
                                    atomically written, resumable, externally validated
```

## The gap

Three builders exist and **all three refuse**:

* `q_tileset::UnimplementedTilesetBuilder` —
  `the_builder_refuses_rather_than_emitting_a_fake_tileset`
* `q_gltf::UnimplementedGlbBuilder` —
  `the_builder_refuses_rather_than_emitting_a_placeholder_glb`
* No tile pyramid is ever generated (`TILE-004`, Not Started)

Refusing was the right call — a placeholder GLB would have been found six months
later, in production, by a user. This phase replaces the refusals with output.

## Entry conditions

* **G1** passed.
* `QM-0003` fixture, `QM-0031` statistics pass, `QM-0033` job executor complete.
* `ADR-CANDIDATE-008` (explicit tiling), `009` (3D Tiles 1.1), and `017` (GLB
  instancing) decided.

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0040` | LOD ladder and block-layout planner | Implementation | `TILE-010` |
| `QM-0041` | `.qtile` pyramid generation | Implementation | `TILE-004`, `MVP-13` |
| `QM-0042` | Instanced GLB tile builder | Implementation | `GLB-001`, `CESIUM-012`, `MVP-14` |
| `QM-0043` | Feature IDs and structural metadata | Implementation | `GLB-004` |
| `QM-0044` | `tileset.json` generation | Implementation | `CESIUM-001`, `CESIUM-011`, `MVP-15` |
| `QM-0045` | Atomic output and resume manifests | Implementation | `TILE-011`, `MVP-16` |
| `QM-0046` | External artifact validation in CI | Verification | `TILE-012`, `MVP-14`, `MVP-15` |

## Design constraints

* **A GLB is never the only carrier of values.** `GlbTileSpec::validate` requires
  a `qtile_uri`; `a_glb_without_a_qtile_sidecar_is_refused` stays passing.
* **`MAX_INSTANCES_PER_TILE = 262_144`.** Above it a tile **refines**; it never
  grows. Equal to `MAX_WORKSPACE_SPHERES` by design, so no pipeline stage can
  produce something a later stage must reject.
* **Position is never stored per cell.** `.qtile` stores a Morton coordinate;
  the renderer derives `tile_origin + decode_morton(coord) × cell_spacing`.
* **Colour and opacity are applied in the viewer**, not baked into the GLB —
  which is also why `ARCHITECTURE.md` §13.2 excludes the palette from the cache
  key.
* **Bounds are derived from the shared grid layout**, not fitted to geometry, and
  a parent's box contains its children's by construction.
* **Geometric error decreases strictly with depth.** A non-refining child makes
  its entire subtree unreachable — an invisible bug. Two tests already hold this.
* **Every write is temp-file + fsync + atomic rename.** A partially written
  `.qtile` or GLB must never be visible under its final name. `tileset.json` is
  written **last**, after every tile it references exists.

## Exit conditions — **integration gate G2**

1. A tensor from the `QM-0003` fixture converts to a full LOD 0–4 pyramid.
2. `.qtile` files decode byte-exactly and round-trip through the existing tests.
3. GLB tiles pass the Khronos `gltf-validator`.
4. `tileset.json` passes `3d-tiles-validator` and the published 3D Tiles schema.
5. Feature IDs are **stable across regeneration** — regenerating twice yields the
   same IDs for the same blocks.
6. Bounds containment asserted: parent ⊇ children, at every level.
7. A killed conversion leaves **no file under a final name**, and resuming
   produces byte-identical output to an uninterrupted run.
8. A second conversion reports cache hits and skips completed blocks.

**No Phase 05 task that requires real data may start before G2.** `QM-0050`, the
viewer spike, is the exception — it uses a hand-authored tileset precisely so it
can run early.

## Parallelization

`QM-0040` → `QM-0041` → `QM-0042` → `QM-0043` is sequential; each consumes the
previous stage's output. `QM-0044` depends on `QM-0042`. `QM-0045` and `QM-0046`
depend on everything before them.

`QM-0041`/`QM-0042` touch different crates (`q-tiles`, `q-gltf`) and could
overlap once the planner lands, but the GLB builder needs a `.qtile` to
reference, so overlap is limited.

## Risks

| Risk | Mitigation |
| --- | --- |
| R4 — glTF extension support worse than assumed | `QM-0057`'s probe, plus three emission profiles down to a core-glTF-only floor |
| A tile too large to render is emitted | `MAX_INSTANCES_PER_TILE` enforced in `validate`, before emission |
| "Valid GLB" means "valid per the code that wrote it" | `QM-0046` uses **external** validators — the class of bug internal round-trips cannot catch |
