# Prerequisites — Gate Before Autonomous Agent Implementation

Complete these gates before authorizing an autonomous agent to implement large Quatricmorph features.

**Source of truth:** [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)

Status legend: `[ ]` open · `[x]` done · `[~]` partial

## G0 — Repository & documentation

- [x] Canonical implementation architecture at repo root `ARCHITECTURE.md`
- [x] Docs index and product brief defer to root architecture
- [x] Agent charter available (`docs/agent/CHARTER.md`)
- [x] `AGENTS.md` at repo root
- [x] Cursor rules under `.cursor/rules/`
- [x] Phase 0 requirements extracted (`docs/requirements/VIZ_MVP.md` / `TILE-*`)
- [x] Platform requirements aligned (`docs/requirements/MVP_REQUIREMENTS.md`)
- [x] Testing strategy documented (`docs/TESTING.md`)
- [x] Conflicting Three.js-as-product architecture docs removed or redirected

## G1 — Local engineering environment

- [ ] Rust toolchain for `crates/` (when Phase 0 Rust work starts)
- [ ] TypeScript app toolchain for CesiumJS viewer (`apps/web/` or agreed path)
- [ ] `cargo test` / package scripts defined for active crates
- [ ] Viewer build succeeds for Cesium spike
- [ ] CI runs tests + build on PR (when remote is ready)

## G2 — Test baseline (required before autonomous coding)

- [ ] Fixture policy for small SafeTensors samples (allowlisted; no large downloads in CI)
- [ ] Seed tests: header parse, byte-range read, scalar equality vs Python reference
- [ ] Seed tests: canonical address round-trip / LOD tile metadata
- [ ] Manual Cesium smoke checklist (zoom LOD, click cell, exact value)
- [ ] No network model downloads in default unit tests

## G3 — Scope lock for the next agent sprint

Pick **one** active track. Do not mix Phase 4 native desktop with Phase 0 tiling unless explicitly tasked.

### Track A — Phase 0 Tensor Tiling Spike (recommended next)

- [x] Requirement IDs in `VIZ_MVP.md` (`TILE-*`) defined from architecture §18
- [ ] Single-file SafeTensors fixture chosen (0.5B–7B class or cropped tensor fixture)
- [ ] CesiumJS + tileset.json spike path agreed
- [ ] Out-of-scope list enforced (no cube-per-weight; no full-model; no Cesium compute)

### Track B — Phase 1 Dense Model Browser (later)

- [ ] Rust workspace scaffold exists (`q-safetensors`, `q-architecture`, `q-nsir`, `q-catalog`, …)
- [ ] Architecture plugin layout under `architectures/`
- [ ] NSIR / catalog schema draft frozen
- [ ] Local daemon / CLI interface sketched (`q-daemon`, `q-cli`)

## G4 — Safety & product constraints

- [x] `mm/` kept as historical read-only reference (do not delete; not product surface)
- [x] Legacy `quatricmorph/` Three.js tree not treated as architecture target
- [x] Axiom “validation before success” + “no semantic claims without evidence” in agent rules
- [ ] No network model downloads in automated tests without explicit allowlist
- [ ] License/provenance note for any third-party weights used as fixtures

## Gate decision

Autonomous agents may start **Track A (Phase 0)** when G0 is complete and G2–G3 Track A items for the sprint are filled (or explicitly waived by the user for a spike).

Autonomous agents must **not** start Track B until G3 Track B prerequisites are complete.
