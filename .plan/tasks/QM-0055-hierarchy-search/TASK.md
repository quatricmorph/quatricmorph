# QM-0055 — Hierarchy, breadcrumbs, and search

## Status

Blocked

Unblocks when `QM-0051` reaches `Complete`.

## Phase

Phase 05 — Cesium model viewer

## Objective

Navigate `Model → Subsystem → Layer → Module → Tensor → Block` by tree, by
breadcrumb, and by search — with ambiguous aliases showing candidates.

## Repository Evidence

* `q-catalog` — `hierarchy_browse_returns_one_summary_per_layer` (`CAT-003`),
  `canonical_address_lookup_and_raw_name_fallback` (`CAT-004`),
  `shape_dtype_and_resolution_filters_work` and
  `role_and_layer_filters_drive_alias_resolution` (`CAT-005`). All Verified.
* Daemon: `GET /v1/models/{id}/layers`, `/tensors` (`API-001`).
* `an_ambiguous_alias_is_a_409_carrying_its_candidates` (`API-007`).
* `QM-0021` provides `tile_for_address` for fly-to.
* `tensor_anchor` is a pure function of the canonical address
  ([`GRID_ARCHITECTURE.md`](../../GRID_ARCHITECTURE.md) §7), so fly-to needs no
  round trip.

## Requirements Covered

`CESIUM-009`, `MVP-06`, `MVP-34`.

## Dependencies

`QM-0051`, `QM-0021`.

## Blocks

`QM-0080`.

## Parallelization

Lane B, parallel with `QM-0054`, `QM-0056`, `QM-0057`.

## Program Boundary

`apps/web/model-viewer`.

## Scope

* Lazily expanded hierarchy tree from the catalog routes.
* Breadcrumbs reflecting **both** camera focus and selection when they differ.
* Search by canonical address, alias, and raw name.
* Ambiguous alias → candidate list; the user chooses.
* Filters: role, component, layer range, dtype, rank.
* Fly-to on selection, using `tile_for_address`.

## Out of Scope

Chat (`QM-0074`) · candidate UI in the query box (`QM-0075`) · editing.

## Files Expected to Change

* `apps/web/model-viewer/src/shell/layout.ts`

## Files Expected to Add

* `apps/web/model-viewer/src/hierarchy/{tree,search,breadcrumbs,filters}.ts`
* `apps/web/model-viewer/src/__tests__/hierarchy.test.ts`
* `apps/web/model-viewer/e2e/search.spec.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

Search input is classified by parse attempt, in order: canonical address → alias
→ raw name. Only text no parser accepts is reported as not found.

A `409` renders the candidate list from `QM-0023`'s contract. **The list is never
truncated and never pre-selected.**

## Memory and Performance Constraints

* The tree is **lazily expanded**: a 47 278-tensor model must not build 47 278
  DOM nodes. Expanding a layer fetches only that layer's tensors.
* Search must be a catalog query, not a client-side scan.
* Fly-to computes the anchor client-side from the address; no round trip.

## Implementation Plan

1. Tree component fetching children on expand.
2. Search box with the three-stage classification.
3. Candidate list rendering on 409.
4. Breadcrumbs from selection and camera state.
5. Filter controls mapped to the catalog's five filter kinds.
6. Fly-to: address → anchor → bounds → camera.
7. Tests, including a large-model tree that expands lazily.

## Error Handling

* Search finding nothing → "not found", with the closest matches by prefix if
  any.
* Ambiguous → candidates; **never a silent pick**.
* A layer with no tensors → shown as empty, not hidden; an absent node reads as
  "does not exist".
* A catalog error → visible message, tree remains usable.

## Acceptance Criteria

1. The tree expands model → layer → module → tensor.
2. A 47 278-tensor model builds **no more than one layer's** nodes at a time.
3. Search by canonical address selects and flies to the tensor.
4. Search by `Q[10]` resolves.
5. Search by `Att[10]` shows ≥ 4 candidates and does not choose.
6. Search by raw name resolves via the fallback.
7. All five filter kinds work.
8. Breadcrumbs distinguish camera focus from selection.
9. Fly-to needs no additional round trip.

## Verification Plan

**Automated** — vitest for classification, filters, and lazy expansion;
Playwright for search → fly-to.
**Manual** — browse the large fixture; try each search form.

## Suggested Commands

```bash
cargo test -p q-catalog                                      # verified today
npx playwright test apps/web/model-viewer/e2e/search.spec.ts   # introduced here
```

## Test Cases

| Input | Expected |
| --- | --- |
| Expand a layer | Only that layer's tensors fetched |
| 47 278-tensor model | Node count bounded per expansion |
| `model.layers[10]…q_proj.weight` | Selects and flies |
| `Q[10]` | Resolves |
| `Att[10]` | ≥ 4 candidates; no auto-pick |
| `model.layers.10.self_attn.q_proj.weight` | Raw-name fallback resolves |
| Filter role = attention query | Only those tensors |
| Filter layer 8–12 | Only those layers |
| Camera on layer 3, selection on layer 10 | Breadcrumbs show both |
| Nonsense text | "Not found", with prefix suggestions |

## Risks

| Risk | Mitigation |
| --- | --- |
| Eager tree building freezes on a large model | Lazy expansion is an acceptance criterion, tested at 47 278 |
| Client-side search does not scale | Search is a catalog query |
| Candidates truncated for layout | No truncation; the list scrolls |

## Completion Evidence

* Node-count measurement on the 47 278-tensor model.
* Screenshots of each search form, including the candidate list.
* Filter test output.
* Breadcrumb screenshot with focus and selection differing.
