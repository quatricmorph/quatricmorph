# MASTER_PLAN — Quatricmorph v1

> **v1 is one diagnostic, shipped end to end, in the hands of design partners.**
> It is not the platform. The platform is still the destination; this plan is the
> shortest honest route to finding out whether anyone will pay to walk it.

Rewritten against [`../Quatricmorph - Standalone Business, Market, and Technical
Strategy.md`](<../Quatricmorph - Standalone Business, Market, and Technical Strategy.md>)
(August 2026). The reconciliation with the previous plan — what moved, what it
costs, what is reversible — is in [`STRATEGY_ALIGNMENT.md`](STRATEGY_ALIGNMENT.md).

---

## 1. What this plan re-points, and what it keeps

**The spine survives. The head changes.**

The strategy document's first engineering instruction is *"build the out-of-core
streaming spine first."* That spine is largely built and tested here already:
SafeTensors ingestion, memory-mapped byte-range reads, named and enforced memory
budgets, a canonical address space, a catalog that indexes a 10¹²-parameter
manifest in 35.7 MB, an L1/L2 content-addressed cache, a job state machine, and
CPU reference statistics. That is 391 passing tests of exactly the infrastructure
the strategy calls the moat.

What changes is what sits on top of it. The previous plan's critical path ended
in a CesiumJS viewer and an animated matrix multiplication — Level 1–2 on the
strategy's value ladder (§4): attention and curiosity, near-zero willingness to
pay. The new critical path ends in a **quantization-error diagnosis that changes
an engineering decision with a dollar figure attached** — Level 3.

| | Previous plan | v1 |
| --- | --- | --- |
| Critical path ends at | Cesium renders a tileset; `A @ B` animates | A ranked fragile-layer report a compression engineer acts on |
| Proof of "out-of-core" | Synthetic 10¹² manifest (metadata only) | **Real ≥ 24 GB checkpoint streamed under a bounded resident-byte ceiling** |
| Primary artifact | GLB + `tileset.json` | Markdown report + JSON manifest |
| Success measured by | 46 acceptance criteria, all internal | Acceptance criteria **plus** four external PMF signals |
| Tests kept | 391 | 391 — none deleted |

Nothing built is thrown away. `.qtile`, GLB, the tileset builder, `GridRuler3D`,
the matmul engine and its animation schedule all stay in the tree, keep their
tests, and keep their tasks. They are `Deferred`, not `Superseded`.

## 2. Current repository summary

Verified by running the commands, not by reading claims. Evidence in
[`REPOSITORY_ANALYSIS.md`](REPOSITORY_ANALYSIS.md) and [`../STATUS.md`](../STATUS.md).

```bash
cargo test --workspace          # 290 passed; 0 failed
cd apps/web && npx vitest run    # 101 passed (12 files)
```

| Layer | State | v1 relevance |
| --- | --- | --- |
| Rust workspace | 17 crates, ~15 200 lines, at repository root (`ADR-001`) | Foundation |
| SafeTensors ingestion | Working. Single-file and sharded, byte-range reads, cancel/resume, stable IDs, 18 requirements `Verified` | **v1 spine** |
| NSIR / addressing | Working. Canonical addresses, alias grammar, generic + Llama resolvers, MoE expert addressing, ambiguity returns candidates | **v1 spine** |
| Catalog | Working. SQLite, versioned migrations, hierarchy queries. A 10¹²-parameter manifest indexes in **35.7 MB peak** | **v1 spine** |
| Statistics | CPU reference computed and tested. **Never persisted** | **v1 — must persist** |
| Cache | L1 and L2 work and survive reopen. **Nothing calls them** | **v1 — must wire** |
| Job state machine | Transitions tested; **no runner** | **v1 — must run** |
| WeightQL | Parses, resolves, shape-checks, costs, executes scalar/slice reads. Matmul plans but does not execute | v1 keeps what exists; execution deferred |
| `.qtile` v1 | Encode/decode round-trips byte-exact. **No pyramid is generated** | **Deferred** |
| GLB / `tileset.json` | Types and guardrails only; both builders refuse to emit placeholders | **Deferred** |
| CesiumJS viewer | LOD policy and daemon client tested. **Nothing renders** | **Deferred** |
| Matrix workspace | `mm` ported; pure math extracted and tested; `GridRuler3D` holds a grid invariant | **Deferred** |
| CUDA | Four `.cu` files, never compiled, never executed | **Deferred (post-v1)** |
| Daemon | 8 routes serve real data; 5 return 501 carrying a requirement ID | **v1 — add diagnostics routes** |

The honest one-line summary is unchanged and still governs: **the metadata and
addressing spine is real and tested; nothing renders, nothing computes on a GPU,
and no visual artifact has ever been produced.**

### The one thing the existing tests do *not* prove

`CAT-006` indexes a synthetic trillion-parameter *manifest* in 35.7 MB. It proves
**metadata scale**. It proves nothing about streaming real bytes: no test in this
repository has ever read more than a 1.2 MB fixture, and `models/distilbert-distilgpt2`
is 339 MB. The strategy's central technical claim — out-of-core diagnosis of a
checkpoint that does not fit in a 24 GB-class GPU — is currently **unproven on
real data**, and closing that gap is `QM-0100`, the first task in the plan.

## 3. What v1 is

One sentence, from the strategy document §11, and every task is judged against it:

> **Quatricmorph shows the quantization error you currently cannot see, so you can
> decide which layers to leave at higher precision.**

The v1 pipeline:

```text
Real open-weight SafeTensors checkpoint (≥ 24 GB on disk)
→ header inspection, architecture resolution, canonical addresses   [built]
→ bounded streaming block reader                                    [QM-0030]
→ simulate a candidate quantisation config per block (RTN int8/int4,
    per-tensor / per-channel / per-group, symmetric / asymmetric)   [QM-0120]
→ paired block reduction: ‖W−Ŵ‖_F, RMSE, max|Δ|, relative error,
    outlier attribution — base and simulated block, never resident
    together beyond one block                                       [QM-0121]
→ aggregate per channel → tensor → module → layer → expert → model  [QM-0123]
→ rank fragile tensors; compute the mixed-precision frontier
    (bytes saved vs. weight-space error, greedy over the ranking)   [QM-0125]
→ persist to the catalog; cache by content address                  [QM-0020, QM-0032]
→ Markdown report + JSON manifest, deterministic and Git-diffable   [QM-0140]
→ heat-map surface over layer × channel, fed by the manifest        [QM-0150]
→ CLI exit codes + daemon route so CI and coding agents can ask     [QM-0143]
```

### 3.1 The decision that makes this cheap

**v1 simulates the quantisation; it does not ingest a third-party quantised
artifact.**

The value proposition is a *pre-quantisation* decision — "which layers should I
leave at higher precision?" — so the engine only ever needs the base checkpoint
and a config. `Ŵ = dequant(quant(W, config))` is computed block by block and
discarded. That ships on the existing SafeTensors reader with **zero new
input-format work**.

Reading someone else's quantised checkpoint — GPTQ / AWQ / compressed-tensors
packed int4 with scales, zero-points and `g_idx`, and GGUF — is a different
problem (format archaeology) and answers a different question ("did the quant I
already ran go wrong?"). It is real, it is wanted, and it is **post-v1**:
[`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §3.1. Letting it into v1 is the single
most likely way this plan slips.

### 3.2 What v1 may never claim

The strategy asks for "an estimated accuracy-cost tradeoff." A weight-space error
metric is **not** an accuracy prediction, and Hessian-weighted sensitivity
(strategy §7.3) needs activations from a calibration set — which needs an
inference runtime, an explicit non-goal in every version of this architecture.

So v1 emits:

* exact, reproducible weight-space error metrics;
* a **ranking** of tensors and layers by those metrics;
* a bytes-vs-error frontier, where both axes are computed, not estimated.

and v1 refuses to emit a predicted accuracy delta. That refusal is a **seam**
(`EVAL-001`) returning `NotImplemented` with its requirement ID, exactly as this
repository already does for every unbuilt capability. Details and the exact
wording the report must use: [`DIAGNOSTIC_ARCHITECTURE.md`](DIAGNOSTIC_ARCHITECTURE.md) §8.

This is not caution for its own sake. Product axiom 5 and `AGENTS.md` rule 6
already forbid unevidenced semantic claims, and the tool's entire credibility
with a compression engineer rests on it saying "this is a weight-space proxy"
before they discover it themselves.

## 4. What "out-of-core" means, on the hardware that exists

The development machine is an **Apple M3 Pro, 36 GB unified memory, 51 GB free
disk**. There is no discrete VRAM to overflow, so "does not fit in 24 GB of VRAM"
is not directly testable here. Restate the property as something the code
enforces and a test can measure:

> **Peak resident bytes stays under a configured ceiling `C` while streaming a
> checkpoint `N ×` larger than `C`, with `N` ≥ 100.**

`crates/q-source/src/budget.rs` already implements named, enforced budgets; this
is that mechanism pointed at the streaming path. Concretely, v1's headline run:

| Quantity | v1 target | Why |
| --- | --- | --- |
| Checkpoint on disk | **≥ 24 GB** | The strategy's reference ceiling (RTX 3090-class) |
| Configured resident ceiling `C` | **≤ 2 GB** | Comfortably under a 24 GB card *and* under this machine's RAM |
| Ratio `N` | **≥ 100 ×** | Makes the claim structural, not incidental |
| Peak RSS, measured | **≤ 1.25 × C** | Measured with `/usr/bin/time -l`, not asserted |

**Disk is the binding constraint, and it is recorded as one.** 51 GB free means a
30–40 GB checkpoint is the largest v1 can hold locally. The strategy's 1.5 TB
frontier-MoE example is **not provable on this machine** and v1 will not claim it:
[`DEFINITION_OF_DONE.md`](DEFINITION_OF_DONE.md) carries an explicit waiver, and
the escape routes — external NVMe, or the NVIDIA Inception credits the strategy
recommends applying for in Days 0–30 — are named in
[`VALIDATION_PLAN.md`](VALIDATION_PLAN.md) §2.

## 5. Program boundaries

| # | Subsystem | Today | v1 delta |
| --- | --- | --- | --- |
| 1 | Ingestion and catalog | `q-source`, `q-safetensors`, `q-architecture`, `q-nsir`, `q-catalog` | Model-level metadata from `config.json`; statistics rows persisted; Qwen resolver (the family v1's target checkpoints belong to) |
| 2 | Block runtime | `q-tensor-runtime`, `q-statistics`, `q-gpu` | Bounded streaming reader; **paired** block reduction in the `Backend` trait; job runner; cache wired |
| 3 | **Diagnostic engine** (new) | — | `q-quant` (simulation) and `q-diagnostics` (engine, aggregation, ranking, frontier) |
| 4 | **Report and manifest** (new) | — | `q-report`: deterministic Markdown + versioned JSON manifest |
| 5 | Local service and CLI | `q-daemon`, `q-cli` | `quatricmorph diagnose` / `report`; diagnostics routes; CI exit codes |
| 6 | Diagnostic surface | `apps/web/*` | One new lightweight heat-map app fed by the manifest. **No Cesium, no Three.js scene graph** |
| 7 | Metal accelerator | `gpu/metal/` placeholder | Metal backend behind the existing `Backend` trait, differentially verified against CPU |

Subsystems 8–10 of the previous plan — Cesium viewer, matrix workspace, chat —
are retained in [`TARGET_ARCHITECTURE.md`](TARGET_ARCHITECTURE.md) and deferred.

### 5.1 The trait change that unblocks everything

`q_gpu::Backend` today is single-tensor:

```rust
fn block_statistics(&self, source, descriptor, extent, histogram_bins) -> Result<TensorStatistics>;
```

The wedge needs a **paired** reduction — base block against its simulated
counterpart — with per-output-channel partials. That extension is the first real
engineering task on the critical path (`QM-0121`), and it is the reason
`QM-0004` (a shared spatial contract for a viewer v1 is not building) is no
longer first. Signature and tolerances: [`DIAGNOSTIC_ARCHITECTURE.md`](DIAGNOSTIC_ARCHITECTURE.md) §4.

## 6. Phases

| Phase | Name | Tasks | Outcome |
| --- | --- | --- | --- |
| 10 | Out-of-core proof on a real checkpoint | 5 | A ≥ 24 GB checkpoint streams end to end under a ≤ 2 GB ceiling, measured |
| 11 | Quantisation-error diagnostic engine | 8 | Simulation, paired reduction, aggregation, ranking, mixed-precision frontier, Metal lane |
| 12 | Report, manifest, and CI/agent interface | 5 | A deterministic Markdown report and a versioned manifest; CLI exit codes; daemon routes |
| 13 | Diagnostic surface | 4 | A heat-map that makes concentration visible in one screenshot |
| 14 | Validation and v1 release | 6 | Design-partner runs, the decision-change case, docs and STATUS regenerated, root documents amended |

Phases 00–03, 08 and 09 contribute the tasks named in §7; the rest are
`Deferred`. Entry and exit conditions are in `phases/*/README.md`.

## 7. Critical path

```text
QM-0100  acquire and verify a real ≥24 GB checkpoint      ← start immediately, long lead time
QM-0101  bounded-residency proof, measured
  → QM-0030  bounded streaming block reader
  → QM-0120  quantisation simulation (RTN, group/channel, sym/asym)
  → QM-0121  paired block reduction in the Backend trait
  → QM-0122  streaming diagnostic pass over a whole tensor
  → QM-0123  aggregation: channel → tensor → layer → expert → model
  → QM-0125  ranking and the mixed-precision frontier
  → QM-0141  deterministic Markdown report
  → QM-0150  heat-map surface
  → QM-0161  design-partner run on a real checkpoint
  → QM-0162  documented decision-change case                ══ the actual goal
  → QM-0165  v1 release audit
```

Thirteen tasks. **Every one runs on an M3 Pro with no NVIDIA hardware**, and none
of them requires a renderer, a tileset, or a chat interface.

Two tasks are prerequisites without being serial links, and both are scheduled
early: `QM-0140` (manifest schema) gates `QM-0141`, `QM-0143` and `QM-0150`, and
`QM-0033` (job runner) gates cancellation and resume (`V1-06`). Neither sits
between two path tasks, so neither lengthens the path.

`QM-0100` is first because it has the longest lead time in the whole plan — a
multi-hour download against a disk with 51 GB free — and because every
performance claim downstream is unprovable without it.

## 8. Parallel lanes

| Lane | Tasks | Touches | Blocked by |
| --- | --- | --- | --- |
| **P — Proof** (critical) | `QM-0100`, `QM-0101`, `QM-0030` | `crates/q-source`, `q-tensor-runtime`, `fixtures/` | — |
| **Q — Quantisation engine** (critical) | `QM-0120`…`QM-0127` | `crates/q-quant`, `q-diagnostics`, `q-gpu` | `QM-0030` |
| **R — Report and interface** | `QM-0140`…`QM-0143` | `crates/q-report`, `q-cli`, `q-daemon` | `QM-0123` for real data; the schema can be written first |
| **S — Surface** | `QM-0150`…`QM-0153` | `apps/web/diagnostics` | `QM-0140` (manifest schema only — not real data) |
| **T — Catalog and persistence** | `QM-0010`, `QM-0011`, `QM-0012`, `QM-0020`, `QM-0031`, `QM-0032` | `crates/q-catalog` | Sequential among themselves (shared file) |
| **U — Metal accelerator** | `QM-0126`, `QM-0127` | `crates/q-gpu`, `gpu/metal/` | `QM-0121`. **Blocks nothing** — CPU is the reference and ships v1 |
| **V — Validation** (runs from day 1) | `QM-0160`…`QM-0167` | No code | **Nothing.** Partner conversations start before the tool works |

Lane V is the lane most likely to be skipped and the one the strategy is most
insistent about: *"Line up design partners before polishing the UI — the goal in
the first 30 days is conversations, not stars."*

## 9. Integration gates

| Gate | Task | Proves |
| --- | --- | --- |
| **G1 — Residency** | `QM-0101` | A real ≥ 24 GB checkpoint streams with measured peak RSS ≤ 1.25 × a ≤ 2 GB ceiling. Without this the product claim is fiction |
| **G2 — Numerical** | `QM-0122` | Simulated quantisation and every error metric match an independent Python/NumPy reference on golden tensors, bit-for-bit where the arithmetic is exact |
| **G3 — Artifact** | `QM-0141` | The same checkpoint and config produce a **byte-identical** report and manifest across runs and machines; a changed config produces a readable diff |
| **G4 — Legibility** | `QM-0151` | A reader who has never seen the tool identifies the three most fragile layers from one screenshot, unprompted |
| **G5 — Decision** | `QM-0162` | A named engineer changed a real quantisation decision because of the output. **This is the release gate that matters** |

G5 is not an engineering gate and cannot be closed by writing code. That is
deliberate: the strategy's kill criteria (§10) are precisely the absence of it.

## 10. Release criteria

v1 ships when [`DEFINITION_OF_DONE.md`](DEFINITION_OF_DONE.md) is satisfied. In
summary:

1. `cargo test --workspace` and `npx vitest run` pass, with counts **above** the
   290 + 101 baseline and no newly ignored tests.
2. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -D warnings`
   are clean.
3. The headline run completes from a clean checkout on a machine with no NVIDIA
   GPU, and its measured peak RSS is recorded in the report.
4. Every error metric is verified against an independent Python reference.
5. The report is deterministic and Git-diffable; a golden report is checked in.
6. `STATUS.md` is regenerated from a real run and contains no row whose status is
   more favourable than its evidence.
7. No document or UI string claims an accuracy prediction, a trillion-parameter
   local execution, or any capability the tests do not demonstrate.
8. At least one design partner has run it on a checkpoint the founder did not
   choose.

Criterion 8 is the one that distinguishes v1 from a demo.

## 11. Explicit non-goals for v1

Everything in [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §4 and §5, plus these
v1-specific deferrals, restated so no task can quietly adopt one:

CesiumJS viewer · `.qtile` pyramid generation · GLB tile content · `tileset.json`
· the matrix-multiplication workspace and its animation · chat · KaTeX rendering
of user expressions · WeightQL matmul **execution** · reading third-party
quantised checkpoints (GPTQ/AWQ/compressed-tensors/GGUF) · MoE routing diagnosis
from runtime activations · checkpoint-diff and merge-collision forensics · CUDA ·
any predicted accuracy delta.

Several of these are *next* — the strategy names MoE expert-health as the second
module and checkpoint-diff as the third. `PRODUCT_SCOPE.md` §3 keeps them ordered
and keeps their seams open.

## 12. What this plan does not do

It does not complete v1. It is a plan. The tasks in `tasks/` are the work — and
one of them, `QM-0162`, cannot be completed by working alone.
