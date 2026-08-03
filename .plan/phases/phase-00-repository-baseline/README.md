# Phase 00 — Repository baseline and shared contracts

## Goal

```text
Confirm the 391-test baseline
→ register plan↔repository divergences
→ generate an LOD-capable fixture
→ establish ONE shared spatial contract consumed by Rust and both web apps
```

## Why this phase is not what the task specification describes

The task specification §29 defines Phase 00 as *"Understand current `mm`
architecture → establish build baseline → identify reusable matrix visualization
behavior → protect license and attribution."*

**All four are already done**, and inventing work to fill the phase would waste
effort and obscure what actually blocks the MVP:

| Specified goal | Already satisfied by |
| --- | --- |
| Understand `mm` | `docs/CURRENT_ARCHITECTURE.md` — 305 lines, per-symbol reuse decisions across ~78 symbols, plus 6 defects and a security finding |
| Build baseline | Verified: `cargo test --workspace` → 290 passed; `npx vitest run` → 101 passed |
| Reusable behaviour identified | The port exists: `apps/web/matrix-workspace/`, 40 modules, 74 tests |
| License and attribution protected | `mm/LICENSE` unmodified; `apps/web/matrix-workspace/{LICENSE,NOTICE.md}`; `AGENTS.md` marks `mm/` read-only |

So Phase 00 is retargeted to what genuinely blocks everything downstream: **the
three-spatial-authority problem** ([`REPOSITORY_ANALYSIS.md`](../../REPOSITORY_ANALYSIS.md) §5)
and **a fixture too small to exercise the visual pipeline** (§6).

## Entry conditions

* A clean checkout at `main`.
* Rust toolchain ≥ 1.78; Node 22; Python 3.12 with `numpy` and `safetensors` for
  fixture regeneration.

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0001` | Baseline verification and evidence capture | Verification | 102 `Verified` rows; `MVP-02`…`MVP-04`, `MVP-07`, `MVP-29`, `MVP-32`, `MVP-33`, `MVP-35` |
| `QM-0002` | Plan↔repository reconciliation and divergence register | Verification | `DOC-005` |
| `QM-0003` | LOD-capable generated fixture with golden values | Implementation | `SRC-019` |
| `QM-0004` | Shared spatial contract in `schemas/visualization` | Implementation | `GRID-006`, `SCHEMA-001` |
| `QM-0005` | Cross-language spatial conformance tests | Verification | `GRID-011`, `SCHEMA-002` |

## Exit conditions — **integration gate G1**

1. Both suites pass at or above 290 + 101, with the counts recorded in
   `QM-0001`'s `Completion Evidence`.
2. Every divergence between this plan, `ARCHITECTURE.md`, `STATUS.md`, and the
   code is registered in `QM-0002` with an ADR candidate attached.
3. A fixture exists containing at least one 4096×4096 tensor, generated (not
   committed), with golden values from Python `safetensors`.
4. `schemas/visualization/schema.json` contains `spatial_contract`, and its
   values are **exactly** today's values — nothing has changed behaviourally.
5. A conformance test in Rust and a conformance test in vitest both assert
   against `golden-spatial.json`, and both pass.
6. `apps/web/model-viewer/src/lod-policy.ts` no longer declares a geometric-error
   constant of its own.

**No Phase 04, 05, or 06 task may start before G1.** They would each add a fourth
copy of the spatial rules.

## Parallelization

`QM-0001` and `QM-0002` are independent and may run first, together. `QM-0003` is
independent of both. `QM-0004` → `QM-0005` is a strict sequence.

## Risks addressed

| Risk | How |
| --- | --- |
| R2 — spatial contract bypassed | The contract lands **before** any consumer is written |
| R9 — fixture too small | `QM-0003` |
| R10 — stale citations | `QM-0002` re-validates every citation in this plan |

## Deliberately not in this phase

Reading `mm` again · re-deriving reuse decisions · any behavioural change ·
touching `mm/`.
