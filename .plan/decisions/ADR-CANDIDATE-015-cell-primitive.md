# ADR-CANDIDATE-015 — Sphere-block rendering primitive

## Status

`Open`.

## Context

The product requirement: *each matrix visualizes as sphere blocks; each sphere is
one scalar; size, colour, and opacity are determined by the value.*

The port renders cells as camera-facing point sprites with a ball texture. They
already *look* like spheres. Whether they *are* spheres matters for three
properties: grid-derived size, occlusion, and opacity.

## Repository evidence

* `apps/web/matrix-workspace/src/viz/material.ts:4` — one shared
  `ShaderMaterial`; `TEXTURE = new THREE.TextureLoader().load('/assets/ball.png')`.
* Its vertex shader sizes points by `mag * pointSize / -mvPosition.z` —
  **distance-derived, not grid-derived**.
* `viz/mat.ts:51` — `this.points = emptyPoints(this.H, this.W, info)`;
  `sizeFromData` at :110, `colorFromData` at :144 (HSL, configurable zero hue,
  hue gap, hue spread).
* **Nothing drives alpha from data anywhere.** The opacity channel does not exist.
* `viz/mat.ts:403` — picking uses `raycaster.params.Points.threshold`.
* `q_gltf::MAX_INSTANCES_PER_TILE = 262_144`.

## Decision required

Point sprites or `InstancedMesh` spheres — and how is opacity added?

## Options

| Option | |
| --- | --- |
| **A** | Keep point sprites; add a value→alpha channel in the fragment shader |
| **B** | `THREE.InstancedMesh` with a low-poly sphere |
| **C** | Sprites by default, instanced spheres behind a flag, decided by measurement |
| **D** | Instanced quads with a signed-distance-field sphere in the fragment shader |

## Comparison

| | Sprites | InstancedMesh |
| --- | --- | --- |
| Cost per cell | 1 vertex | ~80–320 triangles |
| Screen size | Distance-derived — **not grid-derived** | Grid-derived; a cell is a cell |
| Occlusion / lighting | None; always camera-facing | Real |
| Opacity | Straightforward in the fragment shader | Needs depth sorting or order-independent blending |
| Picking | `Points.threshold`, already working | Standard instanced raycast |
| At 65 536 cells | Proven by the `mm` port | Unmeasured |

## Advantages

* **A** — 74 workspace tests keep passing; one shader change; proven at scale.
* **B** — genuinely satisfies "sphere"; size is grid-derived, which is the
  property the grid invariant cares about; depth reads correctly.
* **C** — decides on evidence rather than on the word in the requirement.
* **D** — sprite cost with sphere appearance; more shader work.

## Disadvantages

* **A** — distance-derived sizing is **in tension with the grid invariant at close
  range**: a sprite can visually overflow its cell even though its position is
  exactly on grid.
* **B** — 65 536 × ~80 triangles is 5.2 M triangles; at the 262 144 ceiling it is
  21 M. Unmeasured, and possibly unviable on integrated GPUs.
* **C** — two render paths to maintain.
* **D** — transparency sorting is still needed, and it is the hard part.

## Risks

[`RISK_REGISTER.md`](../RISK_REGISTER.md) R5. Also: replacing a proven renderer to
satisfy a word in a requirement risks the alignment guarantees that matter more
than the primitive does.

## Recommended default

**C**, defaulting to **A**.

1. `QM-0063` adds the opacity channel to the existing sprite shader — the genuine
   gap — and clamps size so `r_max ≤ 0.5 × cellSize`, closing the overflow
   tension.
2. The same task adds an `InstancedMesh` path behind a flag.
3. **Measure both** at 65 536 and 262 144 cells: initial render, frame time, heap.
4. The default follows the measurement, and the number is recorded in the task's
   `Completion Evidence`.

Value→channel encoding is fixed regardless of primitive
([`GRID_ARCHITECTURE.md`](../GRID_ARCHITECTURE.md) §5.1):

```text
s      = clamp(|v| / absmax, 0, 1)
radius = cellSize × (r_min + (r_max − r_min) × f(s)),   r_max ≤ 0.5 × cellSize
colour = sign → {negative, zero, positive} palette
alpha  = a_min + (1 − a_min) × s,                        a_min ≥ 0.15
```

`a_min ≥ 0.15` exists so a near-zero weight fades **without vanishing** — an
absent sphere must keep meaning *no data*. And magnitude must survive the loss of
colour or opacity, since §18 forbids conveying state by colour alone; scale is the
primary channel.

## Tasks affected

`QM-0063` (implements and measures), `QM-0064` (budget and degradation).

## Decision deadline

The default (**A** + opacity) is needed before `QM-0063`. The sprite-versus-mesh
choice is made **by** `QM-0063`'s measurement.
