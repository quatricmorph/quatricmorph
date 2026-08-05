# STRATEGY_ALIGNMENT — why v1 is one diagnostic

Read this before anything else in `.plan/`. It records a scope decision that
contradicts several documents still in the repository, so that nobody has to
reconstruct the reasoning from the diff.

---

## 1. What changed

[`../Quatricmorph - Standalone Business, Market, and Technical Strategy.md`](<../Quatricmorph - Standalone Business, Market, and Technical Strategy.md>)
(August 2026) is the newest document in the repository and self-describes as
replacing the earlier framing. Its central instruction:

> Build Quatricmorph as a single sharp wedge — out-of-core, GPU-native
> quantization-error visualization on a real trillion-parameter-class open-weight
> checkpoint — and prove that it changes an engineer's decision before expanding
> to MoE-routing or checkpoint-diff diagnostics. *(§12)*

The previous plan built something else: a CesiumJS model viewer, an animated
matrix-multiplication workspace, and a chat/KaTeX query interface — 62 tasks
across ten phases, ending in a rendered tileset and an animated `A @ B`.

Both are Quatricmorph. Only one of them is a first release.

## 2. The argument, in the strategy's own terms

The strategy's **value ladder** (§4) is the filter every roadmap decision passes
through:

| Level | What it is | What it produces |
| --- | --- | --- |
| 1 — Visual demonstration | "Look at a trillion parameters in 3D" | Attention, stars, ~zero revenue. Dead end alone |
| 2 — Model understanding | Browsing layers, heads, experts; checkpoint diffs | Repeated researcher usage, weak willingness to pay |
| 3 — Engineering diagnosis | "This layer's quantisation error will cost you accuracy"; "this expert is dead"; "this merge collided" | The only level that produces paid subscriptions, because it changes a decision with money attached |

The previous plan's critical path terminated at Level 1–2. Its final acceptance
criteria were "Cesium loads the generated tileset" and "multiplication can be
animated deterministically" — both true, both impressive, neither a purchase
trigger.

Three further facts from the strategy sharpen the urgency rather than change the
direction:

* **The window is 12–18 months, not indefinite** (§1). MixtureKit shipped MoE
  routing visualisation in December 2025; Palace published a general out-of-core
  GPU tensor visualisation framework in September 2025; Goodfire raised $150M in
  February 2026 and is moving from "understand" toward "design and intervene."
  Nobody occupies the exact seat. Three parties are circling it.
* **Quantisation-error tooling in production is still tabular** (§2). Google's AI
  Edge Quantization Debugger reports five scalar metrics per tensor; academic
  layer-sensitivity work still ships R plots. This is the strongest and least
  contested wedge.
* **Scope discipline, not technical difficulty, is the main risk for a solo
  founder** (§9). Stated as a "do not" in the strategy's own words.

## 3. What that means concretely

| | Previous plan | v1 |
| --- | --- | --- |
| Critical path | 16 tasks → Cesium renders a tileset; `A @ B` animates | 15 tasks → a ranked fragile-layer report a compression engineer acts on |
| First task | `QM-0001` baseline, then `QM-0004` shared spatial contract | `QM-0100` acquire a real ≥ 24 GB checkpoint (longest lead time in the plan) |
| Out-of-core proof | Synthetic 10¹² manifest, 35.7 MB peak — **metadata only** | Real ≥ 24 GB checkpoint, measured peak RSS under a ≤ 2 GB ceiling |
| Primary artifact | GLB + `tileset.json` | Markdown report + JSON manifest, deterministic and Git-diffable |
| Release gate | 46 internal acceptance criteria | 32 criteria, of which 5 are external and one (`V1-30`) may not be waived |
| Surface | CesiumJS 3D tile traversal | One 2D heat-map fed by the manifest |

## 4. What was *not* changed

* **No code is deleted.** All 391 tests stay green and stay required.
* **No task is renumbered or removed.** Deferred tasks keep their IDs, their
  specifications, and their dependency edges.
* **No architecture document is discarded.** `TILING_ARCHITECTURE.md`,
  `CESIUM_VIEWER_ARCHITECTURE.md`, `MATRIX_WORKSPACE_ARCHITECTURE.md`,
  `GRID_ARCHITECTURE.md`, `WEIGHTQL_ARCHITECTURE.md`, `QUERY_UI_ARCHITECTURE.md`
  and `CUDA_ARCHITECTURE.md` remain correct and remain the specification for the
  release that follows v1.
* **The four data planes, the canonical address space, the exactness type
  discipline, and the no-unevidenced-claims rule are untouched.** They are what
  makes the wedge buildable in weeks rather than months.

Deferral is one line in a `## Status` field. That is deliberate: if
[`VALIDATION_PLAN.md`](VALIDATION_PLAN.md) §5's pivot criteria fire the other way
— the diagnosis is valued *and* the spatial view is what drives adoption — the
platform lane resumes exactly where it stopped.

## 5. What the deferral costs

Stated plainly, because a scope decision with no downside is usually a scope
decision that has not been thought through.

| Cost | Assessment |
| --- | --- |
| The "wow" demo slips | Real. The 3D checkpoint fly-through is the thing that gets shared. The strategy's answer: that is distribution, not validation (§9), and a heat-map that finds a real problem gets shared by the person it helped |
| Three web apps sit idle | `model-viewer` and `quatricmorph-workspace` keep their tests and their builds; they do not rot in one release cycle |
| The shared spatial contract (`QM-0004`) is not built | It has three consumers, none of which v1 builds. Building it now means writing a schema against three imagined callers |
| Momentum on a nearly-complete tile pipeline | `.qtile` round-trips and the builders refuse placeholders — genuinely close. But "close to an artifact nobody has asked to buy" is the trap the value ladder describes |
| Sunk design work | Roughly 30 task specifications move to a later release. None is wasted; all are dated and cited |

The asymmetry that decides it: **if the wedge succeeds, the platform gets built
with revenue and design partners behind it. If the platform is built first and
the wedge never validates, the platform was built for nobody.**

## 6. Documents that now disagree with this plan

`.plan/README.md`'s precedence table records the conflict; `QM-0167` resolves it.
Until then, this table is the reconciliation.

| Document | Section | Says | Disposition |
| --- | --- | --- | --- |
| `../ARCHITECTURE.md` | §17 phase roadmap | Phase 0 tensor tiling spike is the active track | **Superseded for v1.** Becomes the platform release's roadmap |
| `../ARCHITECTURE.md` | §18 acceptance criteria | 30 criteria, viewer- and matmul-centric | **Superseded for v1** by `DEFINITION_OF_DONE.md` `V1-*`; retained for the platform release |
| `../MASTER_DOCUMENT.md` | §2 primary MVP workflow | Ends in Cesium + matrix workspace + chat | **Superseded for v1.** §1's four-data-plane model and §3's feasibility constraints remain fully in force |
| `../MASTER_DOCUMENT.md` | §20 acceptance criteria | The same 30 criteria | Same disposition |
| `../docs/ROADMAP.md` | Phase 0 | "Tensor Tiling Spike (now)" | Amend "now" → the platform release |
| `../docs/PRODUCT_BRIEF.md` | Immediate engineering wedge | Phase 0 tiling spike | Amend to the diagnostic wedge |
| `../docs/requirements/VIZ_MVP.md` | `TILE-*` | Phase 0 acceptance | Deferred wholesale |
| `../README.md` | "What works today" | Accurate | No change needed — it already describes only what exists |
| `../STATUS.md` | All | Accurate | No change beyond regeneration (`QM-0091`) |

`ARCHITECTURE.md` §1–§16 and §19 — the data planes, ingestion, NSIR, catalog,
block/LOD model, memory discipline, and the structural prohibitions — are **not**
superseded. v1 is built on them.

## 7. What would reverse this decision

Recorded now, while the reasoning is fresh, so that reversing it later is a
decision rather than a drift:

1. A design partner says, unprompted, that the 3D spatial view — not the ranking
   — is the reason they would pay. That is `VALIDATION_PLAN.md` §5's inverse
   pivot, and it moves the platform lane back onto the critical path.
2. The quantisation wedge fails all four PMF signals by month 6 **and** a
   different Level-3 diagnostic shows heat. Follow the heat (strategy §10); the
   platform still does not come first.
3. A funded party ships the wedge before v1 does. Then the differentiator is gone,
   and the honest response is the strategy's §10 endgame — research output and
   technical brand — not a broader platform.

None of these is "the tile pipeline was nearly finished."
