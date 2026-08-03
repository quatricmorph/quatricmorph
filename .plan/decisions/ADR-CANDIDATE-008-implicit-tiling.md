# ADR-CANDIDATE-008 — Implicit versus explicit tiling

## Status

`Open`.

## Context

3D Tiles 1.1 offers *implicit tiling*: a quadtree or octree subdivision described
by a rule plus availability bitstreams, instead of an explicit node per tile.
Which does the generator emit?

## Repository evidence

* `crates/q-tileset/src/lib.rs:60` — `TilesetNode` with explicit `children:
  Vec<TilesetNode>`; `TILES_VERSION = "1.1"`.
* `node_count()` walks the tree; `validate_refinement()` checks every parent/child
  pair.
* `schemas/visualization/schema.json` — `tileset_node` is recursive and explicit.
* `crates/q-catalog/src/schema.rs:111` — `visual_tiles` has `parent_tile_id` and
  `child_count`, an explicit parent-child model.
* `q_tensor_runtime::Lod` — a **6-level ladder whose levels are semantic**
  (model, subsystem, layer, tensor, block, region), not a uniform spatial
  subdivision.

## Decision required

Explicit nodes, implicit subdivision, or a hybrid?

## Options

| Option | |
| --- | --- |
| **A** | Explicit tiles throughout |
| **B** | Implicit subdivision throughout |
| **C** | Explicit for LOD 0–3 (semantic levels), implicit for LOD 4 blocks within a tensor |

## Advantages

* **A** — matches the data model exactly; every node is inspectable; already
  implemented and validated; no availability bitstream to get wrong.
* **B** — smaller `tileset.json` for very large models; Cesium can compute
  subdivision without downloading the tree.
* **C** — the block grid within a tensor genuinely *is* a uniform quadtree, which
  is where implicit tiling fits naturally.

## Disadvantages

* **A** — `tileset.json` grows with tile count. At 1 000 000 nodes it is large,
  which is why `MAX_TILESET_NODES` exists.
* **B** — **the hierarchy is not a uniform subdivision.** A model has 32 layers,
  each with 7–9 tensors of different shapes; there is no quadtree rule that
  produces that. Forcing one would mean fabricating empty nodes, and
  availability bitstreams would then encode mostly absence.
* **C** — two code paths, two validation strategies, for a benefit that only
  appears at model sizes the MVP does not reach.

## Risks

* **A** — a very large model produces a very large tileset. Mitigation:
  `MAX_TILESET_NODES = 1 000 000`, and conversion is scoped — the MVP converts a
  *selected hierarchy*, not necessarily a whole model.
* **B** — silently wrong availability produces invisible missing tiles, the
  hardest class of bug in a tileset.

## Recommended default

**A**, with the seam preserved.

`TilesetNode` already carries the fields implicit tiling would need — `lod`,
`bounding_box`, `geometric_error` — so adopting **C** later is additive rather
than a rewrite. `CESIUM-011` records the seam.

The decisive argument is that the LOD ladder's levels are **semantic, not
spatial**. Implicit tiling describes uniform subdivision of space; a transformer
is not uniform, and pretending otherwise moves complexity from the tileset into
fabricated nodes.

## Tasks affected

`QM-0044` (implements), `QM-0046` (validates).

## Decision deadline

Before `QM-0044`.
