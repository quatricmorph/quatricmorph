# Phase 13 — Diagnostic surface

## Goal

One screenshot in which a compression engineer sees **where quantisation error
concentrates** and can name the fragile layers without being told.

```text
manifest.json
→ heat-map: layer (rows) × output channel or channel group (columns)
→ drill: layer → tensor → channel
→ above the rendering ceiling: aggregate cell, labelled as aggregated
```

## Deliberately small

This phase builds **no Cesium, no 3D tile traversal, no Three.js scene graph, no
GLB**. Those exist in `apps/web/model-viewer` and `apps/web/matrix-workspace`,
they keep their tests, and they are deferred with the platform release.

Two reasons, both from the strategy:

* The value ladder puts "browse a model in 3D" at Level 2 — repeated researcher
  use, weak willingness to pay. The ranking and the frontier are the Level-3
  content; the heat-map's job is to make them legible fast.
* The pivot criteria (§10) say that if the spatial interface turns out not to be
  what drives adoption, the correct response is a headless engine with a
  lightweight report UI. Building the lightweight UI **first** means the pivot
  costs nothing.

## Scheduled against the schema, not the engine

`QM-0150` starts as soon as `QM-0140` fixes the manifest schema, using synthetic
manifests. The surface is therefore reviewable before the engine produces
anything — which is the cheapest possible moment to discover that it is not what
partners want.

## Entry conditions

* `QM-0140` complete: the manifest schema exists and is versioned.
* Real manifests (`QM-0152`) require `QM-0123`.

## Tasks

| ID | Title | Kind | Lane | Requirements |
| --- | --- | --- | --- | --- |
| `QM-0150` | Heat-map surface over layer × channel | Implementation | S | `SURF-001`, `V1-24`, `V1-27` |
| `QM-0151` | Legibility review with real data | Verification | S | `V1-25` |
| `QM-0152` | Surface reads a real manifest end to end | Verification | S | `V1-24` |
| `QM-0153` | Rendering ceiling and labelled degradation | Implementation | S | `SURF-002`, `V1-26` |

## Exit conditions — Gate G4

1. The heat-map renders the `V1-01` checkpoint's manifest.
2. **Legibility:** at least three readers who have not seen the tool identify the
   three most fragile layers from one screenshot, unprompted. Failures are
   recorded too — an unrecorded failed attempt makes the gate meaningless.
3. Magnitude is legible in greyscale. Selection and ranking may never depend on
   colour alone.
4. Above the rendering ceiling the surface degrades to an aggregate cell and says
   so in the UI — never silent truncation. This is the same discipline
   `assertBlockIsBounded` already enforces in the workspace.
5. No unresolved runtime errors in the browser console.

**If G4 fails repeatedly, do not add features.** A surface that needs explaining
is evidence for [`../../VALIDATION_PLAN.md`](../../VALIDATION_PLAN.md) §5.1's
headless pivot, and the report already carries the same content in text.
