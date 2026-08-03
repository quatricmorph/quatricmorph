# Quatricmorph — Autonomous Agent Guide

This repository hosts the Quatricmorph product line. Autonomous coding agents must follow this guide before changing code.

## Canonical sources

| Document | Purpose |
| --- | --- |
| [docs/PRODUCT_ARCHITECTURE_v1.md](docs/PRODUCT_ARCHITECTURE_v1.md) | Full product + technical architecture (source of truth for platform vision) |
| [docs/PRODUCT_BRIEF.md](docs/PRODUCT_BRIEF.md) | Condensed thesis, axioms, and wedge |
| [docs/requirements/PREREQUISITES.md](docs/requirements/PREREQUISITES.md) | **Gate checklist** before agent implementation work |
| [docs/requirements/MVP_REQUIREMENTS.md](docs/requirements/MVP_REQUIREMENTS.md) | P0/P1 acceptance criteria |
| [docs/requirements/VIZ_MVP.md](docs/requirements/VIZ_MVP.md) | Browser visualization MVP (from `prompts.md`) |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Phase plan (Inspect → Query → Morph → Verify) |
| [docs/TESTING.md](docs/TESTING.md) | Test strategy and commands |
| [docs/agent/CHARTER.md](docs/agent/CHARTER.md) | How the autonomous agent must operate |
| [prompts.md](prompts.md) | Detailed visualization MVP engineering brief |

## Current codebase

| Path | Role |
| --- | --- |
| `quatricmorph/` | Vite + TypeScript Three.js visualizer (migrated from `mm/`) |
| `mm/` | **Read-only reference** — do not delete; do not treat as product surface |
| `docs/` | Product and agent documentation |

## Non-negotiable rules

1. Read [docs/requirements/PREREQUISITES.md](docs/requirements/PREREQUISITES.md) and satisfy open gates before large features.
2. Prefer the **visualization MVP** (`docs/requirements/VIZ_MVP.md`) until Phase 1 platform work is explicitly started.
3. Keep changes small, tested, and reversible. Do not invent backend/Rust/CLI systems unless the current task names them.
4. Every visualization must remain backed by explicit math + reproducible params (URL / fixtures).
5. Validation before “success”: structural checks and unit tests must pass (`npm test`, `npm run build` in `quatricmorph/`).
6. Never claim semantic understanding of model weights without evidence (architecture axiom).

## Default agent workflow

1. Identify which requirement ID you are implementing (e.g. `VIZ-01`, `PLAT-P0-INGEST`).
2. Write or update a failing test when behavior is pure/deterministic.
3. Implement the minimal change.
4. Run `npm test` and `npm run build` in `quatricmorph/`.
5. Update the requirement checklist status only when acceptance criteria are met.
