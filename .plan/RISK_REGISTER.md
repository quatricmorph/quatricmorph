# RISK_REGISTER

Ranked by exposure = likelihood × impact. Each risk names a **trigger** — the
observable event that means it has materialised — because a risk without a
trigger is a worry, not a managed risk.

**Scope note.** `R1`…`R*` below are the technical risks of the full platform.
Several of them — `R1` (Cesium), `R2` (spatial contract drift) — belong to
deferred work and are **not live during v1**; they return with the platform
release. The risks that decide v1 are `M1`…`M6` immediately below, and they are
listed first because for a solo founder in a closing window they dominate.

---

# Market and scope risks — live during v1

## M1 — v1 ships and nobody changes a decision · **Medium / Critical**

**Concern.** The strategy's single largest assumption is that engineers will
change a real decision, and pay, because of what Quatricmorph reveals. Everything
else in this plan is infrastructure for testing it. If the diagnosis is
interesting but not decision-changing, the product is at Level 2 on the value
ladder and there is no business yet.

**Trigger.** `QM-0162` completes with no case, after genuine attempts — or two of
the four PMF signals in [`VALIDATION_PLAN.md`](VALIDATION_PLAN.md) §3 are missing
by month 6.

**Mitigation.** `V1-30` may not be waived (`DEFINITION_OF_DONE.md` §9). Partner
conversations start on day 1 (`QM-0160`), not after the tool works, so the
assumption is probed months before the release gate reaches it.

**Residual.** High, and irreducible by engineering. That is what makes it the
top risk rather than a footnote — and `VALIDATION_PLAN.md` §4 records the kill
response in advance so it is a decision, not a mood.

---

## M2 — scope creep back toward the platform · **High / High**

**Concern.** The strategy names this as the main risk for a solo founder here:
*"Do not build the full 18-capability platform before one diagnostic has a
documented decision-change case."* Forty-four `Deferred` tasks sit in this
directory, fully specified and tempting, and the tile pipeline in particular is
genuinely close to working.

**Trigger.** Any commit touching `crates/q-tiles`, `q-gltf`, `q-tileset`,
`apps/web/model-viewer`, or `apps/web/matrix-workspace` in a v1 task; or a
`Deferred` task moved to `Ready` without a `VALIDATION_PLAN.md` §5 decision.

**Mitigation.** `Deferred` is a distinct status an agent may not start
(`.plan/README.md`); `PRODUCT_SCOPE.md` §4 lists the boundary; `EXECUTION_ORDER.md`
§1 repeats it. Each deferred task names the release that takes it up, so the
question "when does this come back?" already has an answer.

**Residual.** Moderate. The mechanism is good; the temptation is constant.

---

## M3 — the window closes · **Medium / High**

**Concern.** The strategy reads the unopposed window as **12–18 months, not
indefinite**. Three parties circle adjacent territory: MixtureKit (MoE routing
visualisation, shipped Dec 2025), Palace (out-of-core GPU tensor visualisation,
Sep 2025 — retargetable at checkpoints), and Goodfire ($150M Series B, Feb 2026,
moving from "understand" toward "design and intervene").

**Trigger.** Any of: a Palace-like framework retargeted at ML checkpoints; a
Goodfire release operating on raw tensors rather than learned features; a W&B or
CoreWeave feature touching static checkpoint weights; MixtureKit extending to
arbitrary large open-weight checkpoints.

**Mitigation.** Quarterly competitive watch (`VALIDATION_PLAN.md` §7), one
paragraph appended here per quarter. v1 is deliberately the shortest path to a
Level-3 result rather than the broadest platform.

**Residual.** Moderate. Speed is the only real mitigation, and this plan is the
attempt at it.

---

## M4 — an overclaim destroys credibility with the first serious user · **Low / Critical**

**Concern.** The audience is engineers who will check. A predicted accuracy delta
the tool cannot support, a "trillion-parameter" claim the disk cannot back, a GPU
attribution when the CPU ran it — any one of these, caught once, ends the
relationship and is repeated to colleagues.

**Trigger.** A forbidden claim from `PRODUCT_SCOPE.md` §5.2 appearing in a
document, a report template, or a UI string.

**Mitigation.** The forbidden-claims table is a specification, not advice;
`V1-22` audits report strings; `QM-0090` audits every document; `QM-0165`'s
release audit greps the repository. The `EVAL-001`/`EVAL-002` seams refuse the
accuracy claim in code rather than relying on discipline.

**Residual.** Low, and the mechanisms are cheap. Kept at Critical impact because
it is unrecoverable.

---

## M5 — the checkpoint will not fit · **Medium / Medium**

**Concern.** The development machine has 51 GB of free disk and 36 GB of unified
memory. v1's headline checkpoint is capped at roughly 30–40 GB, and the
strategy's 1.5 TB frontier-MoE example is not provable locally.

**Trigger.** `QM-0100` cannot place a ≥ 24 GB checkpoint on disk, or `QM-0101`
cannot hold its residency ceiling on one.

**Mitigation.** `DEFINITION_OF_DONE.md` §1 carries an explicit waiver stating what
v1 claims and what it does not. Escape routes: external NVMe, or the NVIDIA
Inception credits `QM-0160` applies for in Days 0–30 — free, no equity, applied
for before it becomes blocking.

**Residual.** Low on the honesty axis (the waiver is written), moderate on the
marketing axis: "24 GB" is a less arresting number than "1.5 TB", and the
temptation to imply the larger one is exactly `M4`.

---

## M6 — the surface, not the diagnosis, is what people react to · **Medium / Low**

**Concern.** The inverse of the usual worry: partners may praise the heat-map and
ignore the ranking, which would mean v1 validated a Level-1 artifact.

**Trigger.** `QM-0151`'s legibility review shows readers using the ranked list and
ignoring the map — or the reverse, with nobody acting on either.

**Mitigation.** `QM-0151` records *which element* each reader used. The report
carries the same content in text, so a headless pivot (`VALIDATION_PLAN.md` §5.1)
costs nothing.

**Residual.** Low. Either outcome is information, and both are cheap to act on.

---

# Technical risks

Below this line, the register is unchanged. Note that `R1` and `R2` concern
deferred work and are **dormant during v1**.

---

## R1 — CesiumJS cannot render this at all · **High / High** *(dormant during v1)*

**Concern.** Cesium is a geospatial engine. It assumes an ellipsoid, a globe, and
GIS coordinates. `ARCHITECTURE.md` §12.1 is candid that it "still carries many GIS
and geospatial rendering assumptions." A model laid out in a local grid at
metre-ish scale may hit precision, culling, or frustum problems that no amount of
configuration fixes. **Nothing has ever been rendered**, so this is untested.

**Trigger.** `QM-0050`'s viewer spike fails to render a hand-authored 3-tile
tileset, or renders it with visible precision artifacts.

**Mitigation.** `QM-0050` is a **spike, scheduled early and deliberately small**:
a hand-written tileset with three tiles, before any generator work depends on it.
Placement uses a local ENU frame at a fixed origin with the globe disabled.
`ADR-CANDIDATE-009` records the decision and its fallback: Three.js with a custom
LOD traversal, reusing the tile format unchanged, at the cost of writing
traversal and culling ourselves.

**Residual.** Moderate. The fallback is real but expensive.

---

## R2 — the spatial contract is introduced and then bypassed · **Medium / High** *(dormant during v1)*

**Concern.** `QM-0004` creates one contract; nothing stops a later task from
declaring a local constant "just for now". The current
`geometricErrorForLod = 1024 / 2 ** lod` with the comment *"mirrors
`q_tileset::GeometricError`"* is exactly how this happens — no one intended
drift; someone needed a number.

**Trigger.** A numeric literal for a grid parameter, a distance threshold, or a
geometric error appears outside `apps/web/core` or the schema.

**Mitigation.** `QM-0005`'s conformance suites make drift a red test rather than
a silent divergence. Additionally, a lint rule and a review checklist item: any
new spatial constant must come from the contract or add a field to it.

**Residual.** Low, once G1 is in place. This risk is highest **before** G1, which
is why G1 is the first gate.

---

## R3 — no RTX 3090 is ever available · **High / Low**

**Concern.** Five requirements stay `Hardware-Unverified` indefinitely.

**Trigger.** Phase 08 completes with no CUDA execution recorded.

**Mitigation.** **This is already handled by design.** CUDA is off the critical
path ([`MASTER_PLAN.md`](MASTER_PLAN.md) §5); the CPU backend is the numerical
reference; `ADR-008` waives the gate; the end-to-end demonstration runs with zero
CUDA. If no GPU appears, the MVP ships with `CUDA-*` at `Hardware-Unverified`,
`STATUS.md` says so, and the documentation claims nothing else.

**Residual.** Negligible for the MVP. The impact is on future performance work,
not on delivery.

---

## R4 — glTF extension support is worse than assumed · **Medium / Medium**

**Concern.** `EXT_mesh_gpu_instancing`, `EXT_mesh_features`, and
`EXT_structural_metadata` may be partially supported, silently ignored, or
correct in the loader but wrong in the renderer. `ARCHITECTURE.md` §10.2 warns
explicitly against assuming support.

**Trigger.** The `QM-0057` capability probe reports a profile below A.

**Mitigation.** The probe runs at viewer start and selects among three emission
profiles, A → B → C ([`TILING_ARCHITECTURE.md`](TILING_ARCHITECTURE.md) §6.3).
Profile C uses no extension beyond core glTF 2.0 and therefore cannot fail for
extension reasons. The active profile is visible in the dev panel.

**Residual.** Low. The floor is real and reachable.

---

## R5 — 262 144 spheres will not render interactively · **Medium / Medium**

**Concern.** The sphere budget is derived from the GLB instance ceiling, not from
a measurement. On an integrated GPU it may not hold 30 fps, especially with
value-driven opacity requiring blending.

**Trigger.** `QM-0063`/`QM-0064` measure below 30 fps at 65 536 cells on the
reference machine.

**Mitigation.** Measure at both 65 536 and 262 144 before choosing the renderer
(`ADR-CANDIDATE-015`). Point sprites are the default precisely because they are
one vertex per cell. `GRID-010`'s degradation path is designed in: above budget,
show an aggregate representation and **say so in the badge**.

**Residual.** Low. Degradation is honest and already specified.

---

## R6 — two hand-written parsers drift · **Medium / Medium**

**Concern.** `ADR-005` chose hand-written parsers in Rust and TypeScript for one
grammar. They will drift. When they do, the KaTeX preview shows one grouping and
the daemon executes another — the worst kind of divergence, because it is
invisible until a result is wrong.

**Trigger.** A query parses in one language and not the other, or produces a
different AST.

**Mitigation.** `QM-0074` adds a shared conformance corpus that both suites run.
`ADR-CANDIDATE-012` records the alternative — compile the Rust parser to WASM —
and why it is not the MVP default: a build step and a payload for 640 lines of
parser.

**Residual.** Low with the corpus; the corpus is what converts an invisible bug
into a red test.

---

## R7 — conversion is too slow to be usable · **Medium / Medium**

**Concern.** The CPU backend converts one block at a time. A 7 B-parameter model
is ~14 GB at f16; at the `QM-0031` budget of 5 s per 4096×4096 tensor, a
full-model conversion is hours.

**Trigger.** `QM-0084` measures full-model conversion beyond a working session.

**Mitigation.** Conversion is scoped — the API accepts
`model | subsystem | layer | tensor | block`, and the MVP demonstration converts a
*selected hierarchy*, not a whole model. Jobs are resumable, so a long conversion
survives interruption. CPU parallelism across blocks is available and bounded by
`MAX_CONCURRENT_BLOCKS`. CUDA is the long-term answer.

**Residual.** Low for the MVP, real for production use. Documented as a
limitation in `QM-0092` rather than hidden.

---

## R8 — browser memory growth across sessions · **Medium / Medium**

**Concern.** `AC-041` requires no obvious leaks. `mm` adds `window` listeners and
never removes them, and its `disposeAndClear` disposes geometries but **not
materials or textures** (`docs/CURRENT_ARCHITECTURE.md` §8, defects 4 and 6). The
port inherited the structure; Cesium adds tilesets, which are large.

**Trigger.** `QM-0082`'s soak shows heap growth beyond 10 % over 100 iterations.

**Mitigation.** `QM-0056` and `QM-0067` own explicit teardown: tileset destroyed,
primitives removed, listeners removed, materials and textures disposed.
`renderer.info.memory` is already surfaced in the GUI, so the number is visible
during development rather than only in a soak.

**Residual.** Low. The defect is known, located, and assigned.

---

## R9 — the fixture is too small to prove anything visual · **High / Low**

**Concern.** The largest tensor in `fixtures/tiny-llama-2shard` is `[128, 48]`.
It cannot be decomposed into 256×256 blocks, cannot produce a five-level pyramid,
and cannot demonstrate that zooming out avoids exact reads.

**Trigger.** Already true. This risk has materialised.

**Mitigation.** `QM-0003`, scheduled in Phase 00 and a hard prerequisite for
Phases 04, 05, and 08. The fixture is **generated, not committed** — a 4096×4096
f32 tensor is 64 MiB — and CI's existing reproducibility job establishes the
pattern.

**Residual.** Negligible once `QM-0003` lands.

---

## R10 — the plan's citations go stale · **High / Low**

**Concern.** This plan cites line numbers, symbol names, and test names across
62 tasks. Code moves.

**Trigger.** A task's `Repository Evidence` names a path or symbol that no longer
exists.

**Mitigation.** [`README.md`](README.md) §"How plans are updated" makes a stale
citation a **bug in the plan** whose fix takes precedence over the task that
found it. Tasks cite symbols and test names in preference to line numbers, since
those survive edits. `QM-0002` re-validates citations at the start.

**Residual.** Low. Annoying, not dangerous.

---

## R11 — `STATUS.md` and the plan disagree · **Medium / Medium**

**Concern.** Two documents claim to describe what is built. They will diverge.

**Trigger.** A requirement's status differs between `STATUS.md` and
[`REQUIREMENT_TRACEABILITY.md`](REQUIREMENT_TRACEABILITY.md).

**Mitigation.** Precedence is explicit: **`STATUS.md` wins**, because it is
generated from a real run. The traceability document carries a `Current status`
column copied from it and marked as such. `QM-0091` regenerates `STATUS.md` at
release, and a task is not `Complete` until it has updated the relevant rows.

**Residual.** Low.

---

## R12 — scope expands through "while we're here" · **Medium / Medium**

**Concern.** Every subsystem here has an obvious next step: a WebGPU renderer,
runtime activations, DuckDB, a desktop app. `ARCHITECTURE.md` §17 describes all
of them, which makes them feel in-scope.

**Trigger.** A task's `Scope` section contains something
[`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) puts in bucket 3 or 4.

**Mitigation.** `PRODUCT_SCOPE.md`'s four buckets, and the rule that a task may
only implement bucket 1. Extension points are named so that "we'll need this
later" has an answer that costs a trait rather than a phase.

**Residual.** Low, with review discipline.

---

## R13 — a plausible wrong result ships · **Low / Very high**

**Concern.** The failure mode this architecture cares most about: a statistic
computed on padded zeros, a resolver guessing a role from shape, a `DaemonBlockSource`
returning zeros on error, a quantized tile displayed as exact. Each is
individually plausible and none is obviously wrong on screen.

**Trigger.** Any test whose expected value is produced by the code under test.
Any code path that returns a default value on error.

**Mitigation.** The repository already has the antibodies, and they are cultural
as much as technical: `unknown_dtype_is_rejected_not_guessed`;
`generic_resolver_returns_unknown_for_names_it_was_not_taught`;
`the_daemon_source_refuses_rather_than_returning_plausible_zeros`;
`the_builder_refuses_rather_than_emitting_a_placeholder_glb`;
`ambiguous_alias_returns_candidates_not_a_silent_pick`. Blocks are clamped, never
padded. Fidelity is carried end to end and rendered as a badge.
[`TEST_STRATEGY.md`](TEST_STRATEGY.md) §7 rule 1 requires independently computed
expectations.

**Residual.** Low, and worth continued vigilance — this is the risk that would
most damage the product's credibility, because a user cannot detect it.

---

## Summary

| ID | Risk | L | I | Owner phase | Residual |
| --- | --- | --- | --- | --- | --- |
| R1 | Cesium cannot render this | H | H | 05 (spike in 00) | Moderate |
| R2 | Spatial contract bypassed | M | H | 00 | Low |
| R3 | No RTX 3090 | H | L | 03 | Negligible |
| R4 | glTF extension support | M | M | 04–05 | Low |
| R5 | Sphere count performance | M | M | 06 | Low |
| R6 | Parser drift | M | M | 07 | Low |
| R7 | Conversion speed | M | M | 03 | Low (MVP) |
| R8 | Browser memory growth | M | M | 05–06 | Low |
| R9 | Fixture too small | H | L | 00 | Negligible |
| R10 | Stale citations | H | L | all | Low |
| R11 | `STATUS.md` divergence | M | M | 09 | Low |
| R12 | Scope expansion | M | M | all | Low |
| R13 | Plausible wrong result | L | VH | all | Low |

**R1 is the one to watch.** It is the only risk whose fallback costs a phase
rather than a task, which is why `QM-0050` is scheduled as an early spike rather
than as the first task of Phase 05.
