# CESIUM_VIEWER_ARCHITECTURE — the model browser

## 0. State

`apps/web/model-viewer/` is three source files: `index.ts`, `lod-policy.ts`,
`tile-client.ts`. Its own `package.json` says *"app shell only; tileset rendering
is not built (CESIUM-001)"*, and it declares no `cesium` dependency.

What **is** built and tested (10 tests):

* `lod-policy.ts` — the ladder, distance thresholds, and the load decision.
  `never_reads_exact_values_from_camera_movement_alone` and
  `reads_exact_values_only_on_an_explicit_selection` are the two policy
  guarantees that make `AC-006` and `AC-007` provable independently of a
  renderer.
* `tile-client.ts` — the daemon client, which
  `treats_a_501_as_a_declared_gap_not_a_failure_to_retry`.

What is missing is the viewer. Eight tasks, `QM-0050`…`QM-0057`.

---

## 1. Components

```text
┌ Header ──────────────────────────────────────────────────────────────────────┐
│ Quatricmorph — Trillion-Scale Tensor Visualization │ model ▾ │ open │ convert │
├──────────────┬───────────────────────────────────────────┬───────────────────┤
│ Hierarchy    │                                           │ Inspector         │
│  · search    │                                           │  canonical addr   │
│  · model     │            Cesium viewport                │  alias · shape    │
│    · layers  │                                           │  dtype · role     │
│      · mods  │        breadcrumbs overlay (top-left)      │  statistics       │
│        · T   │        LOD + exactness (bottom-right)      │  ▣ FIDELITY BADGE │
│              │                                           │  [open in matrix] │
├──────────────┴──────────────┬────────────────────────────┴───────────────────┤
│                             │  chat / WeightQL input, KaTeX preview           │
│                             │  cost estimate · [execute] [cancel] · history   │
├─────────────────────────────┴─────────────────────────────────────────────────┤
│ status: tiles 42/128 · cache 91% · daemon ok · backend cpu-reference          │
└──────────────────────────────────────────────────────────────────────────────┘
```

The query box is **centre-bottom** as the task specification §21 requires. The
optional matrix workspace opens as a panel or a separate route sharing the same
selection state.

### Module layout

```text
apps/web/model-viewer/src/
├── index.ts                 entry, mounts the shell
├── shell/                   header, panels, layout, status bar
├── cesium/
│   ├── viewer.ts            initialization and disposal
│   ├── tileset.ts           load, unload, error states
│   ├── picking.ts           feature pick → address
│   └── camera.ts            fit, reset, presets, URL state
├── hierarchy/               tree, search, breadcrumbs
├── inspector/               metadata, statistics, fidelity badges
├── lod-policy.ts            EXISTING — moves its constants to apps/web/core
└── tile-client.ts           EXISTING — daemon client
```

Shared spatial and fidelity types come from `apps/web/core`
([`TARGET_ARCHITECTURE.md`](TARGET_ARCHITECTURE.md) §2), not from local copies.

---

## 2. Cesium initialization

CesiumJS is a geospatial engine. The plan uses it for **tile traversal, camera,
and picking**, and disables everything else. `ARCHITECTURE.md` §12.1 is candid
that "Cesium still carries many GIS and geospatial rendering assumptions"; the
answer is to switch them off explicitly, in one place, and document why.

| Setting | Value | Why |
| --- | --- | --- |
| `globe` | `false` | There is no Earth |
| `imageryProvider` | `false` | No basemap; also removes a network dependency |
| `terrainProvider` | ellipsoid | Never queried |
| `skyBox`, `skyAtmosphere`, `sun`, `moon` | off | A tensor is not lit by the sun |
| `baseLayerPicker`, `geocoder`, `timeline`, `animation`, `navigationHelpButton` | off | GIS widgets |
| `scene.mode` | `SCENE3D` | |
| `requestRenderMode` | `true` | Render on change, not at 60 fps. A static model should not spin a laptop fan |
| `maximumScreenSpaceError` | 16, configurable | Governs refinement aggressiveness |
| `cacheBytes` / tile memory | from [`MEMORY_BUDGET.md`](MEMORY_BUDGET.md) | Browser memory is the binding constraint |

Model placement: a fixed local ENU frame at a chosen origin, with the model's
bounding box centred on it. Coordinates are grid coordinates transformed once at
the root; every tile's box comes from the shared layout
([`GRID_ARCHITECTURE.md`](GRID_ARCHITECTURE.md) §7), so the viewer and the
workspace agree about where block `(4,7)` is.

**No offline Cesium Ion token, no network imagery.** `docs/TESTING.md` and CI
already forbid network access in tests, and the viewer must run on a machine with
no internet.

---

## 3. Tileset lifecycle

```text
select model
  → GET /v1/visualizations/{modelId}/tileset.json
  → Cesium3DTileset.fromUrl
  → attach to scene, zoom to root bounds
  → camera moves → Cesium requests tiles by screen-space error
  → GET /v1/visualizations/{modelId}/tiles/{tileId}.glb
  → tile rendered
  ...
  → user selects a different model
  → tileset.destroy(), scene.primitives.remove, event listeners removed
```

**Disposal is not optional.** `docs/CURRENT_ARCHITECTURE.md` §8 records that
`mm` adds `window` listeners and never removes them, and that `disposeAndClear`
disposes geometries but not materials or textures — harmless in a single-page
demo, a leak the moment the thing is embedded and re-initialized. `AC-041`
requires that repeated selection and re-initialization not leak, and `QM-0056`
owns the teardown path: tileset destroyed, primitives removed, listeners removed,
`requestRender` handles released, and the tile cache cleared.

---

## 4. LOD behaviour

The policy is already written and tested; the viewer wires it to Cesium.

```text
LOD_DISTANCE_THRESHOLDS = [4096, 1024, 256, 64, 16]
geometricError(lod)     = 1024 / 2^lod
```

Both constants move to the shared contract in `QM-0004`, so the viewer stops
hand-mirroring `q_tileset::GeometricError`.

| Camera state | Loads | Never loads |
| --- | --- | --- |
| Far — model overview | LOD 0–1 tiles, model and subsystem bounds | Anything exact |
| Mid — layers visible | LOD 2 layer tiles, layer statistics | Anything exact |
| Near — one tensor fills the view | LOD 3 tensor tile, histogram, block layout | Anything exact |
| Close — blocks resolved | LOD 4 block tiles, quantized samples | **Still nothing exact** |
| Explicit selection | LOD 5 — the selected extent only | — |

**Zooming in never automatically retrieves a complete tensor.** The finest
automatically loaded level is 4, which carries quantized samples and statistics.
Exact values require a selection or a query — the tested distinction between
`hovering` and `selected` in `decideLoad`.

Prefetch: current tile → children → sibling metadata → stop. Never exact values.

---

## 5. Picking and address resolution

The path that makes `AC-004` true:

```text
click at (x, y)
  → scene.pick → Cesium3DTileFeature
  → featureId (tile-local u32)
  → tile's EXT_structural_metadata, or a daemon lookup in fallback profile B/C
  → { tile_id, block_extent, local_index }
  → local_index → (row, column) within the block
  → block_extent.row_start + row, .column_start + column
  → tensor_id → canonical address
  → "model.layers[10].self_attention.query_projection.weight[1031, 1802]"
```

Two properties make this exact rather than approximate:

1. **Position is derived, so the inverse is exact.** Nothing was stored and
   rounded on the way in ([`GRID_ARCHITECTURE.md`](GRID_ARCHITECTURE.md) §2.3).
2. **`TileId` is extent- and LOD-sensitive** (`TILE-003`), so a tile cannot be
   confused with a different blocking of the same tensor.

Hover shows the address and metadata. **Selection is what triggers an exact
read**, and only for the selected cell or extent.

Selection presentation must not rely on colour alone (task specification §18):
the plan uses outline plus scale bump plus frame emphasis, any one of which
survives a monochrome display.

---

## 6. Exactness display

`AC-010` is `Partial` today: the data model carries fidelity end to end and is
verified, but **no UI renders it**. This is the viewer's most important
non-geometric job.

| Badge | Colour-independent glyph | Shown when |
| --- | --- | --- |
| `METADATA` | ▢ | Shape, dtype, address only — nothing read |
| `AGGREGATE` | ▤ | A statistic over all of a region |
| `SAMPLED` | ▨ | A statistic over a subset |
| `QUANTIZED` | ▩ | Values present, lossily encoded |
| `EXACT` | ▣ | Values as stored in the checkpoint |

Rules:

* Every panel that shows a number shows a badge.
* **A sampled tile must never be displayed in a way that implies it holds all
  exact values** — the task specification §14's explicit prohibition. The
  viewport carries a persistent indicator of the *coarsest* fidelity currently on
  screen, so a user who zoomed out cannot mistake a summary for the data.
* Hovering a badge explains what it means and what would produce a finer one.
* The glyph is not decoration: it is the accessibility path, because a badge
  distinguished only by colour fails the same test §18 applies to selection.

---

## 7. Hierarchy navigation and search

| Feature | Behaviour |
| --- | --- |
| Tree | Model → subsystem → layer → module → tensor. Lazily expanded from `GET /v1/models/{id}/layers` and `/tensors` |
| Breadcrumbs | Reflect the current camera focus **and** the current selection when they differ |
| Search by canonical address | Exact match jumps and fits |
| Search by alias | `Q[10]` resolves; **ambiguity shows candidates**, never a silent jump |
| Search by raw name | Falls back to raw-name lookup (`CAT-004`) |
| Filters | By role, component, layer range, dtype, rank — the catalog already supports all five (`CAT-005`) |
| Fit selection | Camera flies to the selected object's bounds |
| Reset view | Returns to the model overview |

Navigating the tree moves the camera; moving the camera updates the breadcrumb.
Neither triggers an exact read.

---

## 8. Controls

Per the task specification §25:

**Model source** — open local model · open SafeTensors file · open sharded
directory · recent models · import metadata · generate visualization · resume
conversion · cancel conversion.

**Cesium** — fit model · fit selection · reset view · show hierarchy frames ·
show major grid · show minor grid · show labels · show tile bounds · LOD status ·
exactness status.

**Development only, hidden by default** — tile bounds, tile content URIs,
screen-space error overlay, request statistics, selected extension profile. Gated
behind a `?dev=1` flag or a build-time constant, off in the default build.

---

## 9. Error states

Every one of these has a designed state. Silence is the failure mode this
architecture is least willing to accept, because a viewer that shows nothing and
says nothing is indistinguishable from a viewer that is working on an empty
model.

| Condition | Presentation | Recovery |
| --- | --- | --- |
| Daemon unreachable | Banner: "Local service not running", with the expected URL and the command to start it | Retry button; poll with backoff |
| `501` from a route | **Not a failure.** Shows the declared gap and its requirement ID | None — it is a known state (`CESIUM-003`) |
| Conversion incomplete | Partial model with a progress indicator; unconverted subtrees greyed and labelled | Link to the job |
| Missing tile (404) | Placeholder box at the tile's bounds, marked missing | Retry once; then leave the marker |
| Corrupted GLB | Tile marked failed; siblings keep rendering | Log the tile ID; offer regeneration |
| Missing `.qtile` | Geometry renders; the inspector shows "values unavailable" instead of numbers | Offer to regenerate the tile |
| Incompatible tileset version | Refuse to load; name both versions | Regenerate |
| Invalid feature metadata | Picking falls back to a daemon lookup by tile ID | Degraded, still correct |
| Cache miss | Invisible; a status-bar counter only | — |
| Query cancelled | Result panel shows "cancelled", partial results labelled | Re-run |
| glTF extension unsupported | Falls back through profiles A → B → C; the dev panel names the active profile | Automatic (`QM-0057`) |

**No error state is a blank screen, and none is a console-only message.**
`AC-043` requires an empty console; an error the user cannot see is worse than
one they can.

---

## 10. Camera and view state

| Behaviour | Rule |
| --- | --- |
| Initial | Fit the model bounds with a fixed margin, isometric |
| Fit selection | Fly to bounds with a fixed margin; duration bounded so it never feels stuck |
| Reset | Return to initial |
| Presets | Isometric · front · top · volume — the four already in `cameraPresetPose` |
| Serialization | Position, orientation, selection, and LOD flags in the URL query string |
| Restore | On load and on `popstate` |
| Debounce | Camera writes to history debounced (`mm` used 250 ms; reused, and the constant is named this time) |

URL state reuses the `mm` port's parameter machinery
(`apps/web/matrix-workspace/src/util/params.ts`, `app/url.ts`, 3 tests) —
**without** the `config`-URL branch, which fetched an arbitrary URL synchronously
and applied the response as state (`docs/CURRENT_ARCHITECTURE.md` §3). That
branch is deprecated and not carried forward.

---

## 11. Requirements

| ID | Requirement | State | Task |
| --- | --- | --- | --- |
| `CESIUM-002` | LOD policy; exact values only on selection | ✓ Verified | verify only |
| `CESIUM-003` | Daemon client; 501 is a value | ✓ Verified | verify only |
| `CESIUM-004` | Geometric error decreases monotonically | ✓ Verified | verify only |
| `CESIUM-005` | A viewer that renders a tileset | Not started | `QM-0050`, `QM-0051`, `QM-0052` |
| `CESIUM-006` | Cesium initialization with GIS features disabled | New | `QM-0050` |
| `CESIUM-007` | Feature pick → canonical address (`AC-004`) | New | `QM-0053` |
| `CESIUM-008` | Exactness badges in the UI (`AC-010`) | New | `QM-0054` |
| `CESIUM-009` | Hierarchy, breadcrumbs, search by address and alias | New | `QM-0055` |
| `CESIUM-010` | glTF extension capability probe and fallback | New | `QM-0057` |
| `CESIUM-013` | Camera fit/reset/presets, URL state, full disposal | New | `QM-0056` |
