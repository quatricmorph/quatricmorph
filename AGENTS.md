# Quatricmorph — Autonomous Agent Guide

This repository hosts the Quatricmorph product line. Autonomous coding agents must follow this guide before changing code.

## Canonical sources

| Document | Purpose |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | **Root source of truth** — tensor visualization + platform implementation architecture |
| [docs/PRODUCT_BRIEF.md](docs/PRODUCT_BRIEF.md) | Condensed thesis, axioms, and wedge |
| [docs/PRODUCT_ARCHITECTURE_v1.md](docs/PRODUCT_ARCHITECTURE_v1.md) | Broader product vision (subordinate to root `ARCHITECTURE.md` on implementation) |
| [docs/requirements/PREREQUISITES.md](docs/requirements/PREREQUISITES.md) | **Gate checklist** before agent implementation work |
| [docs/requirements/VIZ_MVP.md](docs/requirements/VIZ_MVP.md) | Phase 0 Tensor Tiling Spike (`TILE-*`) |
| [docs/requirements/MVP_REQUIREMENTS.md](docs/requirements/MVP_REQUIREMENTS.md) | Platform P0/P1 (`PLAT-*`) |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Phases 0–6 |
| [docs/TESTING.md](docs/TESTING.md) | Test strategy and commands |
| [docs/agent/CHARTER.md](docs/agent/CHARTER.md) | How the autonomous agent must operate |

If any document conflicts with [ARCHITECTURE.md](ARCHITECTURE.md), follow `ARCHITECTURE.md` and fix or remove the conflicting text.

## Current codebase

| Path | Role |
| --- | --- |
| `ARCHITECTURE.md` | Immutable implementation architecture SoT |
| `docs/` | Product, requirements, agent charter (must align with root architecture) |
| Target layout | `crates/`, `apps/web`, `apps/desktop`, `architectures/`, `schemas/`, `fixtures/` per architecture §16 |
| `mm/` | Historical matrix-viz reference — read-only; do not delete; not product surface |
| `quatricmorph/` | Legacy Three.js experiment — not the architecture target; do not expand as product path |

## Non-negotiable rules

1. Read [docs/requirements/PREREQUISITES.md](docs/requirements/PREREQUISITES.md) and satisfy open gates before large features.
2. Prefer **Phase 0** (`docs/requirements/VIZ_MVP.md`) until Phase 1+ is explicitly started.
3. Keep changes small, tested, and reversible. Follow architecture §16 crate/app layout for new platform code.
4. Every visualization must remain backed by shared query/math substrate (WeightQL / exact range reads); distinguish exact vs sampled vs approximate.
5. Validation before “success”: structural checks and unit tests must pass for the packages you change.
6. Never claim semantic understanding of model weights without evidence.
7. Obey architecture §19 (no cube-per-weight GLBs; no full-tensor browser dumps; Cesium is not a compute engine).

## Default agent workflow

1. Identify which requirement ID you are implementing (e.g. `TILE-07`, `PLAT-P0-INGEST`).
2. Write or update a failing test when behavior is pure/deterministic.
3. Implement the minimal change.
4. Run the relevant test/build commands for touched packages.
5. Update the requirement checklist status only when acceptance criteria are met.
