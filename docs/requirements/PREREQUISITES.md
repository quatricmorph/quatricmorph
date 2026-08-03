# Prerequisites — Gate Before Autonomous Agent Implementation

Complete these gates before authorizing an autonomous agent to implement large Quatricmorph features.

Status legend: `[ ]` open · `[x]` done · `[~]` partial

## G0 — Repository & documentation

- [x] Product architecture document checked into `docs/PRODUCT_ARCHITECTURE_v1.md`
- [x] Condensed brief available (`docs/PRODUCT_BRIEF.md`)
- [x] Agent charter available (`docs/agent/CHARTER.md`)
- [x] `AGENTS.md` at repo root
- [x] Cursor rules under `.cursor/rules/`
- [x] Visualization MVP requirements extracted (`docs/requirements/VIZ_MVP.md`)
- [x] Platform MVP requirements extracted (`docs/requirements/MVP_REQUIREMENTS.md`)
- [x] Testing strategy documented (`docs/TESTING.md`)

## G1 — Local engineering environment

- [x] Node.js toolchain for `quatricmorph/` (`npm install`, Vite, TypeScript)
- [x] `npm run build` succeeds in `quatricmorph/`
- [x] Unit test runner configured (`vitest`) — see G2
- [ ] CI workflow runs `npm test` + `npm run build` on PR (add when remote is ready)
- [ ] Editor/agent can find rules via `.cursor/rules/` (verify in Cursor UI)

## G2 — Test baseline (required before autonomous coding)

- [x] Vitest installed and `npm test` script present
- [x] Seed tests for pure viz math (`Array2D`, `genExpr` / defaults)
- [ ] Seed tests for URL param compress/round-trip (`util/params`)
- [ ] Golden fixture policy documented for sample matrices
- [ ] Visual/smoke checklist for manual Three.js verification
- [x] `npm test` and `npm run build` both green on scaffold

## G3 — Scope lock for the next agent sprint

Pick **one** active track. Do not mix platform Rust work with viz MVP unless explicitly tasked.

### Track A — Visualization MVP (recommended next)

- [ ] Requirement IDs in `VIZ_MVP.md` prioritized for sprint
- [ ] MarginGrid3D / frame contracts agreed (from `prompts.md`)
- [ ] Out-of-scope list enforced (no attention/LoRA/model loading in UI)

### Track B — Platform Phase 1 Inspect (later)

- [ ] Rust workspace scaffold exists
- [ ] SafeTensors fixture library location agreed
- [ ] NSIR schema draft frozen for adapters
- [ ] Local daemon / CLI interface sketched

## G4 — Safety & product constraints

- [x] `mm/` kept as read-only reference (do not delete)
- [x] Axiom “validation before success” captured in agent rules
- [ ] No network model downloads in automated tests without explicit allowlist
- [ ] License/provenance note for any third-party weights used as fixtures

## Gate decision

Autonomous agents may start **Track A** implementation when G0–G2 are complete and G3 Track A checkboxes for the sprint are filled.

Autonomous agents must **not** start Track B until G3 Track B prerequisites are complete.
