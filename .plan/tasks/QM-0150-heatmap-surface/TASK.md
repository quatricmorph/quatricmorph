# QM-0150 — Heat-map surface

## Status

Complete

Claimed by `impl-agent-15` on branch `task/qm-0150-heatmap-surface`, base
`e82fe98`. `QM-0140` is `Complete`, so the schema this consumes exists. Still
built against synthetic manifests, deliberately early.

## Phase

Phase 13 — Diagnostic surface

## Objective

One screen on which a compression engineer sees where quantisation error
concentrates across a checkpoint, and can drill from layer to tensor to channel.

## Repository Evidence

* `apps/web/model-viewer/src/lod-policy.ts` — `never_reads_exact_values_from_camera_movement_alone`
  (`CESIUM-002`): the discipline of not fetching detail without an explicit act.
* `apps/web/model-viewer/src/tile-client.ts` — `treats_a_501_as_a_declared_gap_not_a_failure_to_retry`
  (`CESIUM-003`): how this codebase handles declared gaps.
* `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts` —
  `refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser` (`GRID-005`).
* `apps/web/vitest.config.ts`, 115 passing web tests across 13 files — the
  harness this joins.

**Path note:** the directory is `apps/web/quatricmorph-workspace`;
`apps/web/matrix-workspace` no longer exists. `QM-0006` completed the rename that
commit `103297d` had begun in the references but never applied to the directory,
and `QM-0002` corrected the `.plan/` prose that still carried the old name. The
drift this note used to warn about is closed.

## Requirements Covered

`SURF-001`, `V1-24`, `V1-27`.

## Dependencies

`QM-0140`.

## Blocks

`QM-0151`, `QM-0152`, `QM-0153`.

## Parallelization

Lane S. New app directory; conflicts with nothing.

## Program Boundary

`apps/web/diagnostics` (new). **No Cesium, no Three.js scene graph, no GLB, no
tile traversal.**

## Scope

* A grid heat-map: layers as rows, output channels or channel groups as columns,
  cell colour and a redundant channel encoding relative error.
* Drill-down: model → layer → tensor → channel.
* The ranked fragile-layer list and the frontier table beside the map.
* Loads the **summary** projection by default; per-tensor detail on demand.

## Out of Scope

3D anything · WebGL shader work beyond what a 2D canvas needs · editing or
applying a recommendation · running a diagnosis from the browser (the daemon
does that) · authentication.

## Why this is deliberately small

The strategy's value ladder puts 3D model browsing at Level 2 — repeated
researcher use, weak willingness to pay. The Level-3 content is the ranking and
the frontier; this surface exists to make them legible in one glance. And the
pivot criteria in [`VALIDATION_PLAN.md`](../../VALIDATION_PLAN.md) §5.1 say that
if the spatial view is not what drives adoption, the response is a headless
engine with a lightweight report UI — which is cheapest if the lightweight UI was
what got built.

## Files Expected to Add

* `apps/web/diagnostics/` — `index.html`, `src/app.ts`, `src/heatmap.ts`,
  `src/manifest-client.ts`, `src/__tests__/`
* `apps/web/package.json` — workspace entry

## Data Contracts

Consumes `schemas/diagnostics/manifest.v1.json`. TypeScript types are generated
from or validated against the schema — hand-written types that drift from the
Rust producer are exactly the failure `QM-0140` exists to prevent.

```ts
type Cell = {
  layerIndex: number;
  channelStart: number; channelEnd: number;   // a column may aggregate channels
  relativeError: number;
  aggregated: boolean;                        // true when the cell covers >1 channel
};
```

`aggregated` is surfaced in the UI, not merely tracked — `QM-0153`'s requirement.

## Memory and Performance Constraints

```text
cells rendered ≤ MAX_HEATMAP_CELLS   (default 250 000)
```

Above it, columns aggregate and the cells say so. A 100-layer model with 8 192
channels per layer is 819 200 cells; it must aggregate, never truncate, and never
attempt to render every channel.

The full per-tensor manifest is never fetched wholesale. The summary projection
plus on-demand detail is the browser-side analogue of the block ceiling the
workspace already enforces.

## Implementation Plan

1. Scaffold the app in the existing web workspace, joining the vitest harness.
2. Manifest client: fetch summary, validate against the schema, refuse an unknown
   `manifest_version` naming both versions.
3. Heat-map on a 2D canvas: rows from `layers`, columns from per-channel data,
   aggregating columns above the cell ceiling.
4. Colour scale plus a **redundant** encoding — cell fill fraction or a glyph —
   so magnitude survives greyscale and colour-vision differences. Selection is
   never conveyed by colour alone (the same rule the workspace already follows).
5. Ranked list and frontier table beside the map, from the same manifest.
6. Drill-down fetching per-tensor detail only on an explicit click.
7. Tests against synthetic manifests: cell counts, aggregation thresholds,
   version refusal, no-fetch-without-click.

## Error Handling

| Case | Behaviour |
| --- | --- |
| Unknown `manifest_version` | Refuse with both versions shown; do not partially render |
| Missing optional section (no experts) | The view omits it; no empty panel |
| A `refusals[]` entry | Shown in the UI, with its requirement ID — the same 501-is-a-declared-gap discipline |
| Manifest larger than the cell ceiling | Aggregate and label; never truncate silently |
| Fetch failure | Named error with a retry, distinguished from a declared gap |

## Acceptance Criteria

1. A synthetic manifest renders as a heat-map with correct row and column counts.
2. Above `MAX_HEATMAP_CELLS`, columns aggregate and the cells are labelled
   aggregated.
3. Magnitude is legible in greyscale — a screenshot proves it.
4. Ranked list and frontier table render from the same manifest and agree with it.
5. Per-tensor detail is fetched **only** on an explicit click, asserted by a
   request-log test.
6. An unknown manifest version is refused, not partially rendered.
7. `refusals[]` entries are visible with their requirement IDs.
8. The whole per-tensor manifest is never fetched for the initial view.
9. Web tests join the existing suite and it stays green.

## Verification Plan

**Automated** — vitest against synthetic manifests, including the request-log
assertion.
**Manual** — screenshots, colour and greyscale.

## Suggested Commands

```bash
cd apps/web && npx vitest run
cd apps/web/diagnostics && npx vite build && npx vite preview
```

## Test Cases

| Input | Expected |
| --- | --- |
| 12 layers × 512 channels | 6 144 cells, no aggregation |
| 100 layers × 8 192 channels | Aggregated columns, labelled |
| Greyscale render | Ranking still readable |
| Manifest version 2 | Refused, both versions shown |
| Manifest with refusals | Shown with IDs |
| Initial load | Summary only; per-tensor array not requested |
| Click a layer | One detail fetch, for that layer only |
| Empty ranking | Explanatory state, not a blank panel |

## Risks

| Risk | Mitigation |
| --- | --- |
| The surface grows toward the deferred 3D viewer | Program boundary forbids Cesium/Three.js; review catches it |
| Colour alone conveys the finding | Redundant encoding is an acceptance criterion, with a greyscale screenshot |
| The full manifest is pulled into the browser | Summary-by-default and the request-log test |
| TypeScript types drift from the Rust producer | Generated from or validated against the shared schema |

## Completion Evidence

* Screenshots: colour and greyscale, small and aggregated cases.
* Web test output with counts.
* The request log showing summary-only initial load.
* The version-refusal state.

## Orchestration

**Awaiting Independent Review — fix cycle 1 complete.**

| Field | Value |
| --- | --- |
| Lane | S |
| Branch | `task/qm-0150-heatmap-surface` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0150` |
| Base | `e82fe98` |
| Head | Three commits on top of the base: `c2dc194` `feat(web): add the diagnostics heatmap surface`, `6db4dd8` `docs(plan): record independent review verdict`, and this one, subject `fix(web): clear the canvas on refusal and make the labelling test unfoolable [QM-0150]`. Its SHA is not written here: a commit cannot carry its own hash, and this table is inside it. Read it with `git rev-parse task/qm-0150-heatmap-surface`. Same reasoning as `scripts/baseline.json`'s `_qm_0101_measurement_note`. |
| Agent | `impl-agent-17` (fix cycle **1 of at most 3**); implementation was `impl-agent-15`, review `review-agent-16` |
| Review answered | `review-agent-16`, **CHANGES_REQUESTED** on `c2dc194` — B1 stale canvas on refusal, B2 defeatable labelling test, B3 under-scanning vocabulary checker, B4 legend naming an undrawn mark; plus the degenerate zero-span legend, and the keyword-guard limit recorded under `## Claim limits` |
| Evidence | `.plan/evidence/QM-0150.md` — see `## Fix cycle 1`. The reviewer's `## Independent review` section is unmodified. |
| Merge path | L |
| Tests added | +221 web tests over +8 files, all in `apps/web/diagnostics` (+27 over +1 file in this cycle) |
| Floor before | rust 677 / 51 binaries · web 115 / 13 files |
| Floor after | rust 677 / 51 binaries (unchanged — no Rust touched) · web 336 / 21 files |
| Rust note | `cargo test --workspace` measures 677 / 51 on this branch, which does not contain `QM-0011` or `QM-0121`; `main` is at 744 / 54. No Rust file is in this branch's diff. The controller reconciles the Rust floor at merge. |

Two things the reviewer should look at first.

1. **`apps/web/quatricmorph-workspace/src/util/__tests__/workspace-paths.test.ts`
   was edited.** `QM-0006`'s guard pins the workspace list and the include-glob
   count by hand, so any package added to `apps/web` turns those two assertions
   red. `EXPECTED_WORKSPACES` gained `'diagnostics'` and
   `EXPECTED_INCLUDE_GLOB_COUNT` went 4 → 5; both assertions are still exact and
   the file's test count is unchanged. It is the only edit to a pre-existing web
   package.
2. **Manifest v1 publishes no per-channel error**, so this surface draws one
   cell per layer (summary) or per tensor (full), each spanning the output
   channels the tensor's `shape` declares and each labelled `aggregated`. The
   column planner is written and tested at per-channel resolution for the day a
   manifest carries it; nothing here invents a channel value.
   `.plan/evidence/QM-0150.md` §Research records the reasoning in full.

**G4 is not satisfied by this task.** No reader other than the implementing
agent has looked at the surface. `QM-0151` is the legibility review.
