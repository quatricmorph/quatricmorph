# ADR candidates

## What these are

A **candidate** is a decision that has been identified, researched, and given a
recommended default — but **not made**. It is not authoritative. A task that
depends on an undecided candidate sits at `Blocked` until the candidate is
promoted.

Promotion means: a real ADR is written to `docs/decisions/ADR-0NN-<slug>.md` in
the repository proper, and the candidate here is marked `Promoted → ADR-0NN`.
`docs/decisions/` already holds ten accepted ADRs and is the permanent home;
`.plan/decisions/` is a staging area that ends when the MVP does.

**A recommendation is not an approval.** Per the task specification §34, a
candidate is only marked decided when repository evidence makes the alternatives
nonviable — which is true for four of the twenty below (`001`, `004`, `006`,
`020`), each of which merely records something already shipped.

## How a deadline is derived

A deadline is **not authored**. It is:

```text
deadline = the earliest task, in EXECUTION_ORDER wave order,
           listed in this candidate's own "Tasks affected" section
```

The distinction that matters is between the task that *implements* a decision and
the task that first *commits* to one. They are usually different, and it is the
committing task that sets the deadline — because after it, code or a frozen
schema exists that assumes an answer.

A task **commits** when its `Scope`, `Data Contracts`, or `Files Expected to Add`
contains a concrete value that only one option produces. A task that merely
*cites* a candidate as rationale does not commit.

This rule is mechanically checkable, and it was added after six candidates
(`009`, `013`, `014`, `016`, `018`, `019`) were found naming the implementing
task. Four of those contradicted themselves on their own face — the deadline was
later than the earliest entry in their own `Tasks affected` list. The sharpest
case: `QM-0004` (Wave 0) froze `axis_binding.world_axes` and
`max_implemented_rank` into a cross-language golden vector at gate G1 while
candidates `014` and `016` still read `Open` with deadlines in Waves 2 and 3.

When a candidate answers two coupled questions with different commit points, it
carries **two deadlines** (see `009`).

## Format

Every candidate carries: Context · Repository evidence · Decision required ·
Options · Advantages · Disadvantages · Risks · Recommended default · Tasks
affected · Decision deadline.

## Status vocabulary

| Status | Meaning |
| --- | --- |
| `Open` | Researched, recommended, **not decided** |
| `Decided` | Repository evidence makes the alternatives nonviable; recording only |
| `Promoted → ADR-0NN` | A real ADR now exists in `docs/decisions/` |
| `Superseded` | Replaced by another candidate, named |

## Index

| # | Decision | Status | Deadline | Recommended default |
| --- | --- | --- | --- | --- |
| [001](ADR-CANDIDATE-001-rust-workspace.md) | Rust workspace introduction | `Decided` | — | Root workspace; already shipped as `ADR-001` |
| [002](ADR-CANDIDATE-002-cuda-build.md) | CUDA build strategy | `Open` | Before `QM-0034` | `build.rs` + `nvcc`, feature-gated **off** by default |
| [003](ADR-CANDIDATE-003-metal-build.md) | Metal compute for Apple GPUs | `Open` | Post-MVP | Extension point only; same `Backend` trait |
| [004](ADR-CANDIDATE-004-safetensors-library.md) | SafeTensors library selection | `Decided` | — | Own parser; already shipped and verified |
| [005](ADR-CANDIDATE-005-catalog-technology.md) | Catalog technology | `Open` | Before Phase 08 | SQLite; revisit only on measured need |
| [006](ADR-CANDIDATE-006-qtile-v1.md) | `.qtile` v1 binary schema | `Decided` | — | v1 frozen; already shipped as `ADR-004` |
| [007](ADR-CANDIDATE-007-web-core-package.md) | Shared web core package | `Open` | Before `QM-0060` | New `apps/web/core` package |
| [008](ADR-CANDIDATE-008-implicit-tiling.md) | Implicit versus explicit tiling | `Open` | Before `QM-0044` | Explicit; keep the implicit seam |
| [009](ADR-CANDIDATE-009-3d-tiles-version.md) | 3D Tiles 1.0 versus 1.1; non-geospatial use | `Open` | Placement before `QM-0050`; version before `QM-0044` | 1.1, glTF tile content, local ENU frame |
| [010](ADR-CANDIDATE-010-viewer-shell.md) | CesiumJS framework shell | `Open` | Before `QM-0050` | No framework; plain TypeScript + Vite |
| [011](ADR-CANDIDATE-011-daemon-transport.md) | Local daemon transport | `Open` | Before `QM-0033` | HTTP + Server-Sent Events |
| [012](ADR-CANDIDATE-012-weightql-parser.md) | WeightQL parser technology | `Open` | Before `QM-0074` | Keep two hand-written parsers + a shared corpus |
| [013](ADR-CANDIDATE-013-browser-test-strategy.md) | Browser test strategy | `Open` | Before `QM-0050` | Playwright for render and pick; vitest for logic |
| [014](ADR-CANDIDATE-014-model-layout-planes.md) | Model layout algorithm and plane mapping | `Promoted → ADR-009` | Passed (was `QM-0002`) | Kept the code's mapping; `ARCHITECTURE.md` §8.2 corrected by `QM-0090` |
| [015](ADR-CANDIDATE-015-cell-primitive.md) | Sphere-block rendering primitive | `Open` | Before `QM-0063` | Point sprites + opacity; measure `InstancedMesh` |
| [016](ADR-CANDIDATE-016-nd-axis-binding.md) | N-D axis binding | `Promoted → ADR-010` | Passed (was `QM-0004`) | Rank ≤ 3 implemented; rank > 3 refuses |
| [017](ADR-CANDIDATE-017-glb-instancing.md) | GLB instancing strategy | `Open` | Before `QM-0042` | `EXT_mesh_gpu_instancing` with a 3-profile fallback |
| [018](ADR-CANDIDATE-018-tensor-id.md) | Canonical tensor ID generation | `Open` | Before `QM-0020` | Keep BLAKE3 over `(model_id, raw_name)` |
| [019](ADR-CANDIDATE-019-browser-cache.md) | Browser caching strategy | `Open` | Before `QM-0032` | HTTP caching for the MVP; Cache Storage is L3 |
| [020](ADR-CANDIDATE-020-mm-reuse.md) | `mm` reuse versus extraction | `Decided` | — | Already executed; recorded for completeness |

## Deadlines

A deadline is expressed as "before task `QM-XXXX`" rather than a date, because
this plan has no calendar. The deadline is the point at which the decision stops
being cheap: after it, code exists that assumes an answer.

Which task that is follows from the rule in §"How a deadline is derived" above —
it is the earliest *committing* task in the candidate's own `Tasks affected`
list, not the task that implements the decision. Changing a `Tasks affected`
list means re-checking the deadline.
