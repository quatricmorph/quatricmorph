# Quatricmorph Visualization Architecture (MVP)

Companion to [SYSTEM_ARCHITECTURE.md](SYSTEM_ARCHITECTURE.md) and [requirements/VIZ_MVP.md](requirements/VIZ_MVP.md). Describes the **implemented** Track A stack in `quatricmorph/`.

## Data flow

```text
User Input (GUI / URL)
    → Validation (A.cols === B.rows)
    → Tensor init (Array2D + valuesText presets)
    → MatMul (dotprod → C)
    → GridRuledLines3D / MarginGrid3D placement
    → Three.js scene (Mat point sprites + guides)
    → Interaction (hover, C selection, animation)
    → Renderer + shareable URL
```

## Module map

| Layer | Path | Role |
| --- | --- | --- |
| Math | `src/math/` | Pure validate, matmul, parse, presets, shape kinds |
| Layout | `src/layout/` | `GridRuledLines3D` (alias MarginGrid3D), tensor frames, camera presets |
| Interaction | `src/interaction/` | Animation SM helpers, C-path selection |
| Viz | `src/viz/` | `Array2D`, `Mat`, `MatMul`, materials, sizing (from mm) |
| App | `src/app/` | Scene, URL, defaults, `createApp` lifecycle |
| UI | `src/gui/mvp-gui.ts` | Quatricmorph MVP panel (research GUI: `research-gui.ts`) |

## Scene graph (conceptual)

```text
Scene
 └─ MatMul.group (centered, rotated for Y-up display)
     ├─ A / left Mat (I×K plane)
     ├─ B / right Mat (K×J plane)
     ├─ C / result Mat (I×J plane)
     ├─ flow guides
     └─ animation intermediates (during play)
```

## Coordinate convention

- World **X → J**, **Y → I**, **Z → K**
- Planes: **A** on I×K, **B** on K×J, **C** on I×J
- Placement derived from `placeOperands()` + `operandGap` / `cellSize` (not hard-coded per shape)

## Animation state machine

Canonical algorithm in MVP UI: **output-cell dot product** (`dotprod (row major)`).

Controls: Play, Pause, Step, Previous Step (reset+pause), Reset Calculation. Animation status is separate from matrix data; advancing play clears sticky C selection.

## URL format

- Uncompressed: `?params=<JSON>`
- Compressed: flattened+renamed query keys (`util/params`)
- Includes A/B shapes + `valuesText`, layout/display, camera; not C (recomputed), not Three objects / timers
- Invalid JSON → keep prior fields / fall back to defaults with message

## Disposal

`MatMul.disposeAll()` → `util.disposeAndClear` on the group before replacing the scene object. `initObj` validates first and skips construction on bad dims to avoid partial GPU objects.

## Extension points (MVP 2+)

- Full previous-step micro-rewind without reset
- Ruled minor/major grid meshes as first-class scene nodes
- Remove `eval` / sync XHR paths from non-MVP entry entirely
- Headless visual fixtures

## Related

- Deep design draft: [SYSTEM_ARCHITECTURE.md](SYSTEM_ARCHITECTURE.md)
- Product vision: [PRODUCT_ARCHITECTURE_v1.md](PRODUCT_ARCHITECTURE_v1.md)
