# ADR-CANDIDATE-009 — 3D Tiles version and non-geospatial use

## Status

`Open`. Highest-risk candidate in the set.

## Context

Two coupled questions. Which 3D Tiles version does the generator emit? And can
CesiumJS — a geospatial engine — render a tensor at all?

The second question is untested. **Nothing has ever been rendered in this
repository.** `ARCHITECTURE.md` §12.1 is candid: Cesium *"still carries many GIS
and geospatial rendering assumptions."*

## Repository evidence

* `crates/q-tileset/src/lib.rs:30` — `TILES_VERSION = "1.1"`, already chosen.
* `ARCHITECTURE.md` §9.3 — *"3D Tiles 1.1 allows glTF to be used directly as tile
  content, along with structured metadata and implicit tiling."*
* §11.3 warns that Cesium's `CustomShader` *"is currently marked experimental …
  should not become a long-term core dependency."*
* `apps/web/model-viewer/package.json` — **no `cesium` dependency yet**.
* `STATUS.md` `CESIUM-005` — **Not Started**, `apps/web/model-viewer/` shell only.

## Decision required

1. 3D Tiles 1.0 (b3dm/i3dm) or 1.1 (glTF directly)?
2. How is a non-geospatial model placed, and what is the fallback if Cesium
   cannot render it acceptably?

## Options — version

| Option | |
| --- | --- |
| **A** | 3D Tiles 1.1, glTF/GLB tile content |
| **B** | 3D Tiles 1.0, b3dm/i3dm containers |

**A** — glTF directly, no legacy container, validated by the standard
`gltf-validator`, and the version `q-tileset` already declares. **B** — wider
historical support, but wraps glTF in a deprecated container and needs a separate
validator. **A** wins on evidence; the code already says `1.1`.

## Options — placement and fallback

| Option | |
| --- | --- |
| **P1** | Local ENU frame at a fixed origin, globe disabled |
| **P2** | `Cesium.Scene` in a custom coordinate system |
| **P3** | Not Cesium — Three.js with a custom LOD traversal |

## Advantages

* **P1** — Cesium's traversal, culling, camera, and picking come free; the
  transform happens once at the tileset root; the globe, imagery, terrain,
  atmosphere, sun, and every GIS widget are switched off in one place.
* **P2** — no geospatial baggage at all.
* **P3** — full control; the workspace already uses Three.js, so one renderer
  instead of two.

## Disadvantages

* **P1** — WGS84 coordinates are large, and floating-point precision at model
  scale is a real risk; the Cesium bundle is ~3 MB gzipped.
* **P2** — Cesium's traversal assumes an ellipsoid in enough places that this is
  effectively a fork.
* **P3** — **rewrites tile traversal, screen-space-error refinement, request
  scheduling, and culling.** That is the engine, and it is why Cesium was chosen.

## Risks

**This is [`RISK_REGISTER.md`](../RISK_REGISTER.md) R1**, the only risk whose
fallback costs a phase rather than a task. Mitigated by making `QM-0050` a
**small, early spike**: a hand-written 3-tile tileset rendered before any
generator work depends on it.

## Recommended default

**A + P1.** Emit 3D Tiles 1.1 with glTF content; place the model in a local ENU
frame at a fixed origin with every geospatial feature disabled.

If the `QM-0050` spike fails, fall back to **P3** — **and the tile format does not
change.** `.qtile`, GLB, and `tileset.json` are all renderer-independent; only
`apps/web/model-viewer` would be rewritten. That property is why the artifact
pipeline can proceed in parallel with the viewer spike rather than waiting on it.

`CustomShader` is used only behind a flag, never as a core dependency, per §11.3.

## Tasks affected

`QM-0044`, `QM-0050`, `QM-0051`, `QM-0052`, `QM-0057`.

## Decision deadline

Two deadlines, because this candidate answers two coupled questions.

* **Placement (P1 / P2 / P3) — before `QM-0050`**, the earliest task in `Tasks
  affected` (Wave 1). The spike cannot test a placement it has not been given;
  `QM-0050`'s Repository Evidence already names *"`ADR-CANDIDATE-009` (local ENU
  frame)"* as an input. P1 is the **hypothesis the spike tests**, and the P1/P3
  fallback is then decided **on `QM-0050`'s evidence**.
* **Version (1.1 versus 1.0) — before `QM-0044`** (Wave 3), which is where the
  generator must know its target version.

Corrected from a single `QM-0044` deadline, which named only the second of the
two. See `README.md` §"How a deadline is derived".
