# QM-0063 — Sphere-block cells with value→opacity

## Status

Blocked

Unblocks when `QM-0060` reaches `Complete`.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Render each scalar as a sphere block whose **size, colour, and opacity** encode
its value — and decide sprites versus instanced meshes **by measurement**.

## Repository Evidence

* `apps/web/matrix-workspace/src/viz/material.ts:4` — one shared `ShaderMaterial`;
  `TEXTURE = TextureLoader().load('/assets/ball.png')`. Vertex shader sizes points
  by `mag * pointSize / -mvPosition.z` — **distance-derived, not grid-derived**.
* `viz/mat.ts:51` `emptyPoints`; `:110` `sizeFromData`; `:144` `colorFromData`
  (HSL with configurable zero hue, hue gap, hue spread).
* **Nothing anywhere drives alpha from data.** The opacity channel does not exist.
* `viz/mat.ts:403` — picking via `raycaster.params.Points.threshold`.
* `ADR-CANDIDATE-015` — sprites default, measure `InstancedMesh` behind a flag.

## Requirements Covered

`GRID-009`, `PERF-003`, `MVP-27`.

## Dependencies

`QM-0060`.

## Blocks

`QM-0064`, `QM-0067`, `QM-0068`.

## Parallelization

Parallel with `QM-0061`, `QM-0062`. Touches `viz/`.

## Program Boundary

`apps/web/matrix-workspace/src/viz`.

## Scope

* Add the value→opacity channel to the fragment shader and to
  `setColorsAndSizes`.
* Clamp radius so `r_max ≤ 0.5 × cellSize` — closing the overflow tension with
  the grid invariant.
* Add an `InstancedMesh` sphere path behind a flag.
* **Measure both** at 65 536 and 262 144 cells; set the default from the numbers.
* Selectable magnitude mapping: linear, log, signed-percentile.

## Out of Scope

The sphere budget and degradation (`QM-0064`) · hover metadata (`QM-0068`) ·
changing the picking mechanism.

## Files Expected to Change

* `apps/web/matrix-workspace/src/viz/material.ts`
* `apps/web/matrix-workspace/src/viz/mat.ts`
* `apps/web/matrix-workspace/src/viz/sizing.ts`
* `apps/web/matrix-workspace/src/gui/mvp-gui.ts`

## Files Expected to Add

* `apps/web/matrix-workspace/src/viz/instanced-spheres.ts`
* `apps/web/matrix-workspace/src/viz/__tests__/encoding.test.ts`
* `apps/web/matrix-workspace/e2e/render-budget.spec.ts`

## Files Expected to Remove or Deprecate

None. The sprite path stays regardless of the measurement's outcome.

## Data Contracts

```text
s      = clamp(|v| / absmax, 0, 1)
radius = cellSize × (r_min + (r_max − r_min) × f(s)),   r_max ≤ 0.5 × cellSize
colour = sign → { negative, zero, positive } palette
alpha  = a_min + (1 − a_min) × s,                        a_min ≥ 0.15
```

Three rules:

1. **A zero keeps its cell** — rendered at `r_min`, `a_min`, never absent. An
   absent sphere must keep meaning *no data*.
2. **Magnitude survives losing colour or opacity.** Scale is primary; the others
   reinforce. §18 forbids conveying state by colour alone, and opacity is
   unreliable on projectors and in screenshots.
3. **A sphere never crosses its cell boundary**, by the `r_max` clamp.

## Memory and Performance Constraints

```text
sprites:  1 vertex/cell  → 65 536 verts at 256×256
meshes:   ~80 tris/cell  → 5.2 M tris at 65 536; 21 M at 262 144
budget:   65 536  → < 100 ms initial, < 16 ms/frame
          262 144 → < 400 ms initial, < 33 ms/frame
```

Both paths measured on the reference machine; the numbers go in
`Completion Evidence`.

## Implementation Plan

1. Add an `alpha` attribute; extend the fragment shader to multiply the texture's
   alpha by it, with correct blending and `depthWrite` handling.
2. Add the `r_max` clamp to `sizeFromData`.
3. Add the three magnitude mappings; wire to the GUI.
4. Implement `instanced-spheres.ts` with the same encoding.
5. Add a renderer flag.
6. Measure both at both counts.
7. Set the default from the measurement; **record the numbers**.

## Error Handling

* `absmax = 0` (an all-zero block) → every cell at `r_min`, `a_min`; **not a
  division by zero, and not an empty render**.
* NaN or Inf → rendered in a distinct "non-finite" colour and counted; never
  silently skipped.
* Instanced path unsupported → fall back to sprites with a status-bar note.

## Acceptance Criteria

1. Size, colour, **and opacity** all vary with value.
2. `v = 0` renders visibly at `r_min`, `a_min`.
3. No sphere exceeds `0.5 × cellSize`; neighbours never overlap.
4. Negative, zero, and positive are distinguishable.
5. Magnitude is legible in a **greyscale** screenshot with opacity disabled.
6. All three magnitude mappings work.
7. Both renderer paths produce the same encoding.
8. Budgets met at 65 536; measured at 262 144.
9. An all-zero block renders all cells, no division by zero.
10. Non-finite values are distinct and counted.

## Verification Plan

**Automated** — vitest for the encoding functions and clamps; Playwright for
render timing at both counts.
**Manual** — screenshots: colour, greyscale, opacity-off, all-zero block.

## Suggested Commands

```bash
cd apps/web && npx vitest run encoding                                   # new
npx playwright test apps/web/matrix-workspace/e2e/render-budget.spec.ts   # new
npm run dev --workspace matrix-workspace
```

## Test Cases

| Input | Expected |
| --- | --- |
| `v = 0` | `r_min`, `a_min`, visible |
| `v = absmax` | `r_max ≤ 0.5 × cellSize`, alpha 1 |
| `v = -absmax` | Negative palette, same radius as `+absmax` |
| Block with `absmax = 0` | All cells at `r_min`; no NaN |
| Block containing NaN | Distinct colour; counted |
| Greyscale + opacity off | Magnitude still legible |
| 65 536 cells, sprites | < 100 ms initial, < 16 ms/frame |
| 65 536 cells, instanced | Measured and recorded |
| 262 144 cells, both paths | Measured and recorded |
| Adjacent max-value cells | No overlap |

## Risks

| Risk | Mitigation |
| --- | --- |
| Instanced spheres too slow | Sprites stay the default; the decision follows the measurement |
| Alpha blending breaks depth ordering | Documented; sprites are camera-facing so ordering is simpler |
| Opacity becomes load-bearing for magnitude | Greyscale + opacity-off is an acceptance criterion |

## Completion Evidence

* **The measurement table**: sprites vs instanced × 65 536 vs 262 144, initial
  render and frame time.
* Screenshots: colour, greyscale, opacity-off, all-zero, non-finite.
* The chosen default and the reasoning from the numbers.
* Encoding test output.
