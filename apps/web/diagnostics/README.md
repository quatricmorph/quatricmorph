# `diagnostics` — the quantisation-error heat-map surface

`SURF-001`, `V1-24`, `V1-27` · `QM-0150`

One screen showing where quantisation error concentrates across a checkpoint,
fed by a single document: `schemas/diagnostics/manifest.v1.json`.

## Program boundary

**No Cesium, no Three.js scene graph, no GLB, no tile traversal.** A 2-D canvas
and an SVG, and nothing else. `src/__tests__/boundary.test.ts` scans these
sources and fails if any of those appear. The deferred platform renderer is a
different program; if the spatial view turns out to be what drives adoption,
`.plan/STRATEGY_ALIGNMENT.md` §7 resumes that lane — it is not resumed by this
package growing into it.

## What the surface will and will not say

`ARCHITECTURE.md` §19: *do not assume a colour pattern corresponds to a semantic
concept.* This surface says **where** one measured number is large. It does not
say what that means.

* Colour, fill height and glyph all encode `sqrt(sum_sq_delta / sum_sq_base)`,
  and the legend says so in the image.
* The legend also says the scale is relative to the visible map, not an absolute
  threshold — otherwise the darkest cell reads as "bad" in absolute terms.
* `app.ts` carries the wordings `.plan/DIAGNOSTIC_ARCHITECTURE.md` §8 requires
  (the accuracy caveat, "a proxy for sensitivity", and the frontier's
  not-proven-optimal claim reproduced from the manifest verbatim), plus a
  forbidden-vocabulary check over every string the surface displays.

## Exact, sampled, approximate

Every displayed value is labelled. The word from `manifest.fidelity` appears in
the header, beside every row of the map, in the legend, and on every ranking,
frontier and expert row. `aggregated` is tracked **separately**: `sampled` is
the engine's coarseness and `aggregated` is the renderer's, and conflating them
would tell a reader the data is coarse when the display is.

## Refusal rather than fabrication

A manifest this build cannot read renders a refusal state with no cells at all.
An unknown `manifest_version` names both versions. A body above
`MAX_MANIFEST_PAYLOAD_BYTES` is refused before it is parsed. A layer whose
`sum_sq_base` is zero is drawn as *no measurement*, never as an error of zero.

## The cell ceiling

`MAX_HEATMAP_CELLS` is 250 000. Above it columns merge — **by maximum, never by
mean**, so one catastrophic channel is not averaged away — and merged cells
carry a dashed border that is legible without hovering and in greyscale. Nothing
is ever truncated: a test asserts every channel index is covered by exactly one
cell at every aggregation factor.

## What the manifest does not carry

Manifest v1 publishes partials per layer, per expert and per tensor. It
publishes **no per-channel partials**. So the map draws one cell per layer from
a summary projection and one cell per tensor from a full one, each spanning the
output channels the tensor's `shape` declares, each labelled `aggregated`. The
column planner in `heatmap.ts` accepts per-channel bands and is tested at that
resolution, but this surface will not invent channel values a manifest does not
publish.

## Commands

```bash
cd apps/web && npx vitest run                       # the suite this package joins
cd apps/web && npm run build --workspace diagnostics
cd apps/web/diagnostics && npx vite dev             # ?run=<runId>&palette=greyscale
```

## Artifacts

`artifacts/*.svg` are committed renderings — colour, greyscale, aggregated,
empty, sampled and refused. `src/__tests__/artifacts.test.ts` asserts the
committed bytes still match what the renderer produces, so they cannot go stale.
Regenerate after a deliberate rendering change:

```bash
cd apps/web && QM_WRITE_ARTIFACTS=1 npx vitest run diagnostics/src/__tests__/artifacts.test.ts
```

They are SVG rather than browser screenshots: no browser runs in this
environment. They are produced by the same draw plan the 2-D canvas painter
consumes, and a test asserts the two choose the same colour for the same cell.
**No browser has rendered this surface.**

## Not built here

Daemon wiring, run selection and a browser test against a live daemon are
`QM-0152`. The marker vocabulary for degraded rendering is `QM-0153`. The
legibility review with a real reader — gate G4 — is `QM-0151`, and building this
does not satisfy it.
