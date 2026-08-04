# QM-0062 — Ruled-grid rendering

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Draw the grid. "Aligned to the grid" is an unverifiable claim while the grid is
invisible.

## Repository Evidence

* `grid-ruler.ts:25-37` — `minorGridSpacing`, `majorGridInterval`, `axisMargin`,
  `labelMargin`, `origin` are all config values that **nothing renders**.
* `mm/util.js:124-138` — `lineSeg`, `axes()` drawing 128-unit RGB axes.
* `mm/util.js:140-162` — `rowGuide` at a stride of `(h-1)/denom`.
* Ported to `apps/web/quatricmorph-workspace/src/util/geometry.ts`.
* `apps/web/quatricmorph-workspace/src/layout/tensor-frame.ts` — `buildTensorFrame`,
  `frameContainsPoint`.
* Task specification §25 requires independent toggles for major grid, minor
  grid, labels, and hierarchy frames.

## Requirements Covered

`GRID-008`.

## Dependencies

`QM-0060`.

## Blocks

`QM-0065`.

## Parallelization

Parallel with `QM-0061`, `QM-0065`.

## Program Boundary

`apps/web/quatricmorph-workspace`.

## Scope

* Minor lines every `minorGridSpacing`; major lines every `majorGridInterval`
  minor lines, heavier and labelled with the logical index.
* A distinct origin marker.
* Axis labels: `I`, `J`, `K` for operands; NSIR axis names for tensors where
  known.
* Independent toggles for major, minor, labels, and frames.
* **One buffer of line segments per plane**, not one object per line.

## Out of Scope

Grid rendering in the Cesium viewer, which uses tile bounds instead ·
tensor frames themselves (`QM-0065`) · cell rendering (`QM-0063`).

## Files Expected to Change

* `apps/web/quatricmorph-workspace/src/util/geometry.ts`
* `apps/web/quatricmorph-workspace/src/app/scene.ts`
* `apps/web/quatricmorph-workspace/src/gui/mvp-gui.ts`

## Files Expected to Add

* `apps/web/quatricmorph-workspace/src/layout/grid-lines.ts`
* `apps/web/quatricmorph-workspace/src/layout/__tests__/grid-lines.test.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

All geometry derives from the grid config in `apps/web/core`. Every vertex is
snapped and `assertVecSnapped` is called at the **plane** level, not per line —
per-line assertion would cost more than the drawing.

Major-line labels show the **logical index**, not a world coordinate: a user
reading "1024" should be reading a tensor row, not a scene unit.

## Memory and Performance Constraints

* A 512×512 grid is 1 026 lines per plane. As one `BufferGeometry` per plane
  that is 3 draw calls, not 3 078.
* Labels only at major intervals — at `majorGridInterval = 5` and 512 cells that
  is ~103 labels per axis, drawn into **one shared texture**, never one DOM node
  each.
* Rebuild only when the grid config or extent changes, never per frame.

## Implementation Plan

1. `buildGridLines(extent, config)` returning one merged geometry per plane, with
   minor and major as separate materials.
2. Origin marker as a distinct small geometry.
3. Axis labels via the existing `util/text.ts` path, into a shared texture.
4. Major-line index labels at the same intervals.
5. Toggles wired into `mvp-gui.ts`.
6. Tests: line counts, snapping, label positions, rebuild-on-change only.

## Error Handling

* `cellSize = 0` → error; an infinite line count is not a rendering problem, it
  is a configuration error.
* An extent producing more than 10 000 lines per plane → reduce to major lines
  only, and say so in the status bar.
* Font loading failure → lines still render; labels are skipped with a warning.

## Acceptance Criteria

1. Minor and major lines render at the configured intervals.
2. Major lines are visually heavier and labelled with the logical index.
3. The origin marker is distinct from both.
4. Axis labels show `I`, `J`, `K` for operands and NSIR names for tensors.
5. Each of the four toggles works independently.
6. **3 draw calls for grid lines**, not one per line — asserted via
   `renderer.info.render.calls`.
7. Every grid vertex is snapped within `1e-6`.
8. Labels use one shared texture; **no DOM node per label**.
9. The grid rebuilds only when config or extent changes.

## Verification Plan

**Automated** — vitest for line counts, snapping, and label positions; a draw-call
assertion.
**Manual** — screenshot at several zoom levels; confirm alignment against cell
centres.

## Suggested Commands

```bash
cd apps/web && npx vitest run grid-lines               # introduced here
npm run dev --workspace quatricmorph-workspace
```

## Test Cases

| Input | Expected |
| --- | --- |
| 256×256 extent, `minorGridSpacing 1` | 257 lines per direction |
| `majorGridInterval 5` | Every 5th line heavy, labelled |
| Major label at index 1024 | Reads "1024", not a world unit |
| Toggle minor off | Only major lines remain |
| Toggle labels off | Lines remain, labels gone |
| Draw calls for grid geometry | 3 |
| Every vertex | Snapped within `1e-6` |
| DOM node count | Unchanged by label rendering |
| Extent forcing > 10 000 lines | Major only; status-bar note |
| Config unchanged across 100 frames | No rebuild |

## Risks

| Risk | Mitigation |
| --- | --- |
| Line count explodes at large extents | The 10 000-line degradation, with a visible note |
| Labels become DOM nodes by habit | Shared texture; DOM count asserted |
| Grid rebuilt every frame | Rebuild-on-change asserted |

## Completion Evidence

* Screenshots at three zoom levels showing minor, major, origin, and labels.
* Draw-call count.
* Snapping test output.
* DOM-node count before and after enabling labels.
