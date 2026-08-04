# PRODUCT_SCOPE — what is in v1, what is a seam, what is deferred, what is never

This document exists to stop v1 expanding. Every capability Quatricmorph might
have falls into exactly one of five buckets. A task may only implement something
in bucket 1.

| Bucket | Meaning | A task may |
| --- | --- | --- |
| **1. v1 capability** | Shipped, tested, and exercised in the headline run | Implement it |
| **2. Architectural seam** | A named trait, schema field, enum variant, or route that a later capability plugs into. Returns `NotImplemented` with a requirement ID today | Create the seam; implement the refusal; **not** the capability |
| **3. Deferred — next modules** | Wanted, specified, sequenced. Blocked on a **product decision**, not an engineering dependency | Nothing |
| **4. Deferred — the platform** | The full visualization platform. Correct work, later release | Nothing |
| **5. Explicit non-goal** | Deliberately excluded, sometimes permanently | Nothing |

Buckets 3 and 4 are the change from the previous version of this document, which
had a single "future" bucket. The distinction matters: bucket 3 is what v1's
success unlocks; bucket 4 is what v1 was carved out of. Both are reversible by
editing a `## Status` line. See [`STRATEGY_ALIGNMENT.md`](STRATEGY_ALIGNMENT.md).

The repository's bucket-2 idiom is already consistent — every stub returns
`QError::NotImplemented` carrying its requirement ID, and
`apps/web/model-viewer/src/tile-client.ts` treats a `501` as *a declared gap, not
a retryable failure*. New seams follow that pattern.

---

## 1. v1 capabilities

### 1.1 Already built and verified — carried into v1 unchanged

These need no implementation task, only a verification task confirming they still
hold at release (`QM-0001`). Cited to [`../STATUS.md`](../STATUS.md).

| Capability | Requirements |
| --- | --- |
| Single-file and sharded SafeTensors ingestion, headers only | `SRC-001`…`SRC-004` |
| Memory-mapped byte-range reads; invalid offsets refused | `SRC-005`, `SRC-015` |
| Stable `model_id` / `tensor_id` across reopen | `SRC-006` |
| Ingestion allocates nothing proportional to checkpoint size | `SRC-007`, `AC-001` |
| Cancellable and resumable metadata import | `SRC-009`, `SRC-010`, `AC-003` |
| Corrupt headers, duplicate names, unknown dtypes refused, never guessed | `SRC-012`…`SRC-014` |
| Exact f32 / bf16 / f16 decoding including subnormals | `SRC-016` |
| Named, enforced memory budgets | `SRC-017` |
| Access scale is a type, not a comment | `SRC-018` |
| Canonical addresses; generic and Llama resolvers; `unknown` preserved | `NSIR-001`…`NSIR-004` |
| **MoE expert addressing** (`Expert[12,37].up`) | `NSIR-003` |
| Alias grammar; ambiguity returns candidates | `NSIR-005`, `NSIR-007` |
| Versioned, idempotent catalog migrations; future schema refused | `CAT-001`, `CAT-002` |
| Hierarchy browse; canonical lookup; filters; byte-range arithmetic | `CAT-003`…`CAT-005`, `CAT-007` |
| Trillion-parameter **manifest** indexed in bounded memory | `CAT-006` |
| CPU reference statistics, Welford-stable, streaming-equals-batch | `STAT-001`, `STAT-003`, `STAT-004` |
| Sampled results labelled approximate | `STAT-005` |
| **Comparison metrics: cosine similarity, relative L2** | `STAT-006` |
| L1/L2 cache with eviction, surviving reopen | `CACHE-001`…`CACHE-004` |
| Job state machine with illegal transitions rejected | `JOB-001`, `JOB-003` |
| Daemon: models, layers, tensors, exact value, block window | `API-001`…`API-008` |
| Path traversal refused; model-root boundary enforced | `SEC-001` |
| No arbitrary code execution, Rust and browser | `WQL-009`, `SEC-002`…`SEC-004` |
| WeightQL lexer, parser, resolution, shape check, cost, plan IDs | `WQL-001`…`WQL-005`, `WQL-010`…`WQL-012` |

`STAT-006` deserves note: relative L2 and cosine similarity, already implemented
and hand-verified, are two of the metrics the diagnostic engine needs. The wedge
is closer to the existing code than it looks.

### 1.2 To be built in v1

| Capability | Requirement | Task |
| --- | --- | --- |
| A real ≥ 24 GB open-weight checkpoint, acquired and header-verified | `SRC-020` | `QM-0100` |
| **Bounded-residency proof**: peak RSS ≤ 1.25 × a ≤ 2 GB ceiling while streaming it | `PERF-002` | `QM-0101` |
| Bounded streaming block reader with named budgets and backpressure | `TILE-009` | `QM-0030` |
| Quantisation simulation: RTN int8/int4, per-tensor / per-channel / per-group, symmetric and asymmetric | `QUANT-001` | `QM-0120` |
| **Paired block reduction** in `q_gpu::Backend`, with per-output-channel partials | `QUANT-002` | `QM-0121` |
| Streaming diagnostic pass over a whole tensor, verified against a Python reference | `QUANT-003` | `QM-0122` |
| Aggregation: channel → tensor → module → layer → expert → model | `QUANT-004` | `QM-0123` |
| Outlier attribution: share of squared error carried by the top-*p* magnitude weights | `QUANT-005` | `QM-0124` |
| Fragility ranking and the **mixed-precision frontier** (bytes vs. weight-space error) | `QUANT-006` | `QM-0125` |
| Metal backend, differentially verified against the CPU reference | `GPU-003` | `QM-0126`, `QM-0127` |
| Statistics and diagnostics persisted in the catalog | `STAT-002`, `DIAG-001` | `QM-0020` |
| Cache wired into the diagnostic path | `CACHE-008` | `QM-0032` |
| Job runner: checkpointing, atomic output, resume, cancellation | `JOB-002` | `QM-0033` |
| Model-level metadata (`hidden_size`, `layer_count`, `parameter_count`) from `config.json` | `CAT-011` | `QM-0012` |
| Qwen-family architecture resolver | `NSIR-006` (Qwen only) | `QM-0010` |
| **Versioned JSON manifest** of every diagnostic result | `REP-001` | `QM-0140` |
| **Deterministic, Git-diffable Markdown report** | `REP-002` | `QM-0141` |
| Golden report and a diff test proving determinism | `REP-003` | `QM-0142` |
| CLI exit codes for CI (`--fail-above`) and daemon diagnostics routes | `REP-004`, `API-012` | `QM-0143` |
| Heat-map surface over layer × channel, fed by the manifest | `SURF-001` | `QM-0150` |
| Degradation to aggregate above a rendering ceiling, stated in the UI | `SURF-002` | `QM-0153` |
| Design-partner run on a checkpoint the founder did not choose | `VAL-001` | `QM-0161` |
| A documented decision-change case | `VAL-002` | `QM-0162` |

---

## 2. Architectural seams

Built as a seam in v1. The seam is tested; the capability behind it refuses with
its requirement ID.

| Seam | Form in v1 | Requirement held open |
| --- | --- | --- |
| **Accuracy prediction from a calibration set** | `q_diagnostics::AccuracyEstimate` exists as a type; every constructor refuses. The report prints the weight-space caveat instead | `EVAL-001` |
| **Hessian-weighted sensitivity** (strategy §7.3) | Same seam. Needs activations, which need an inference runtime — a permanent non-goal at this layer | `EVAL-002` |
| **Third-party quantised checkpoints** (GPTQ / AWQ / compressed-tensors / GGUF) | `QuantisedSource` trait alongside `ModelSource`; `DType` already carries `I8`, `U8`, `F8E4M3`, `F8E5M2`. Packed sub-byte layouts refuse | `QUANT-010` |
| **Additional quantisation schemes** (NF4, MXFP4, AWQ-style scaling, GPTQ error feedback) | `QuantScheme` is an open enum behind a trait; v1 implements RTN only and names the others in the refusal | `QUANT-011` |
| **MoE expert-health from weights** (expert-pair cosine similarity, per-expert norm) | Aggregation already keys by expert (`NSIR-003`); the metric slots into the same reducer | `MOE-001` |
| **MoE routing from runtime activations** | No `TensorRole` activation variants; adding them is additive | `MOE-002` |
| **Checkpoint diff / merge collision** | The paired reduction in `QM-0121` is already *two sources against each other* — a base-vs-variant diff is the same kernel with a different second operand | `DIFF-001` |
| **MCP-style agent interface** | The daemon's diagnostics routes return the same manifest the CLI writes; an MCP server is a thin adapter over them | `API-013` |
| **Remote checkpoints over HTTP Range** | `ModelSource` trait; `crates/q-source/src/http.rs` computes ranges correctly, transport refuses | `SRC-008` |
| **CUDA acceleration** | `q_gpu::Backend`; `CudaBackend` enforces the ceiling and refuses execution | `CUDA-001` |
| **L0 GPU-resident / L3 browser / L4 remote cache** | `CacheTier` trait; tiers refuse rather than missing silently | `CACHE-005`…`CACHE-007` |
| **Kimi and DeepSeek resolvers** | `architectures/{kimi,deepseek}/plugin.toml` with `implemented = false`; the registry never lets them claim a model | `NSIR-006` |
| **3D spatial error map** | The manifest carries the coordinates a tiler would need. The tile pipeline is deferred, not deleted | `TILE-004` |

The `DIFF-001` seam is worth the emphasis: engine 3 in the strategy (checkpoint
diff, §7.5) and engine 1 (quantisation error, §7.3) are *the same computation*
over different second operands. Building `QM-0121` as a genuinely paired reduction
— rather than hard-coding "base vs. its own simulated quantisation" — makes the
third module nearly free later. That is one of the few places where generalising
early is cheaper than not.

---

## 3. Deferred — the next modules

Sequenced by the strategy §8. **No v1 task implements any of these, and no v1
acceptance criterion depends on one.**

### 3.1 Module 2 — verify an existing quantised checkpoint

Read a third-party quantised artifact and diff it against its base: packed int4
with scales, zero-points and `g_idx`; `compressed-tensors`; GGUF k-quants. Answers
"did the quantisation I already ran go wrong, and where?" — a different question
from v1's "which layers should I leave at higher precision?"

Deferred because it is **format archaeology on the critical path**: three packing
conventions, none of them fully specified, each with its own dequantisation edge
cases. It would double the v1 timeline for a question the pre-quantisation
diagnostic already partly answers.

### 3.2 Module 3 — MoE routing and expert health

The strategy's second wedge (§7.4, §8). Split into two, because they have
different costs:

* **Weight-space expert health** — per-expert norms, expert-pair cosine
  similarity, redundancy flags. Computable from weights alone, reuses `QM-0123`'s
  expert-keyed aggregation, and is a plausible v1.5.
* **Routing statistics** — per-expert load, routing entropy. Needs router
  probabilities over a token sample, which needs an inference runtime. Bucket 5
  at this layer; it belongs to a separate ingestion path if it is ever built.

Note the strategy's warning: MixtureKit already ships routing heatmaps. If this
module is taken up, it must lead with out-of-core scale and the serving-cost
estimate, not with "we visualise routing."

### 3.3 Module 4 — checkpoint diff and merge-collision forensics

`ΔW = Ŵ_merged − W_base`, collision score between two adapters' deltas. Reuses
`QM-0121` wholesale via the `DIFF-001` seam. Third in the strategy's sequence
because the segment is hardest to reach and the CLI ecosystem below it is the
most crowded.

---

## 4. Deferred — the platform

Fully specified in this directory, correct, and **not in v1**. Every one has
tasks with numbers that are preserved.

| Capability | Where it is specified | Tasks |
| --- | --- | --- |
| `.qtile` pyramid generation | [`TILING_ARCHITECTURE.md`](TILING_ARCHITECTURE.md) | `QM-0040`…`QM-0046` |
| Instanced GLB tile content, feature IDs | [`TILING_ARCHITECTURE.md`](TILING_ARCHITECTURE.md) | `QM-0042`, `QM-0043` |
| `tileset.json` generation and external validation | [`TILING_ARCHITECTURE.md`](TILING_ARCHITECTURE.md) | `QM-0044`, `QM-0046` |
| CesiumJS model viewer, picking, inspector, hierarchy search | [`CESIUM_VIEWER_ARCHITECTURE.md`](CESIUM_VIEWER_ARCHITECTURE.md) | `QM-0050`…`QM-0057` |
| Shared spatial contract and conformance tests | [`GRID_ARCHITECTURE.md`](GRID_ARCHITECTURE.md) | `QM-0004`, `QM-0005`, `QM-0060` |
| Grid matrix workspace, sphere-block cells, real-block matmul | [`MATRIX_WORKSPACE_ARCHITECTURE.md`](MATRIX_WORKSPACE_ARCHITECTURE.md) | `QM-0061`…`QM-0068` |
| WeightQL matmul execution, stacked slices, statistical `SELECT` | [`WEIGHTQL_ARCHITECTURE.md`](WEIGHTQL_ARCHITECTURE.md) | `QM-0070`…`QM-0073` |
| Chat that emits WeightQL plans; KaTeX sanitisation | [`QUERY_UI_ARCHITECTURE.md`](QUERY_UI_ARCHITECTURE.md) | `QM-0074`, `QM-0075` |
| CUDA build, kernels, and soak | [`CUDA_ARCHITECTURE.md`](CUDA_ARCHITECTURE.md) | `QM-0034`…`QM-0036`, `QM-0083` |

**Why the platform was deferred rather than descoped.** The strategy's value
ladder (§4) puts "browse layers, attention heads, experts in 3D" at Level 2:
repeated use among researchers, weak willingness to pay. It is not that the
platform is wrong — it is the eventual product — but that building it *before* a
Level-3 diagnostic exists means spending the whole window on the part nobody pays
for. The strategy's pivot criteria (§10) go further: if the spatial interface
turns out not to be what drives adoption, the correct response is a headless
diagnostic engine with a lightweight report UI, which is precisely what v1 is.

**v1 is therefore also the cheapest possible test of whether the platform is
worth building.**

---

## 5. Explicit non-goals

Not built, v1 or otherwise, at this layer:

Training visualization · automatic differentiation · gradient visualization · a
full inference runtime · token-conditioned hidden states · runtime attention
probabilities · complete Q/K/V activation capture · LoRA editing · model
morphing · distributed cluster execution · multi-user collaboration · user
accounts · a remote SaaS control plane · notebook integration · full Hugging Face
Hub browsing · arbitrary Python execution · full trillion-parameter numerical
execution on one GPU · a native Vulkan renderer · Tauri desktop packaging ·
multi-GPU scheduling · full-model spectral decomposition · automatic semantic
interpretation of visible weight patterns.

### 5.1 Structurally forbidden

Enforced by existing tests, not merely stated:

| Prohibition | Enforced by |
| --- | --- |
| One cube GLB per weight | `q_gltf::MAX_INSTANCES_PER_TILE`; `cube_per_weight_explosions_are_refused` |
| A GLB as the only carrier of tensor values | `a_glb_without_a_qtile_sidecar_is_refused` |
| Storing absolute positions for every scalar | Position derives from origin + logical index + layout rule |
| Sending an entire tensor to the browser | `assertBlockIsBounded`; `whole_tensor_reads_are_refused_with_an_explanation` |
| Chat freely executing terabyte expressions | Chat emits a plan; execution is a separate explicit act |
| Treating colour as semantic proof | `AGENTS.md` rule 6 |

### 5.2 Forbidden claims — new for v1, and load-bearing

v1's output is a number an engineer will act on. These prohibitions protect the
only asset the product has at this stage, which is being trusted.

| Never claim | Why | Say instead |
| --- | --- | --- |
| A predicted accuracy or eval delta | Requires an evaluation the tool does not run | "Weight-space error. Accuracy impact is not measured — run your eval on the recommended config" |
| That relative Frobenius error *is* sensitivity | It is a proxy, and a coarse one | "Ranked by relative weight-space error, a proxy for sensitivity" |
| That a trillion-parameter checkpoint was processed locally | 51 GB of free disk says otherwise | The measured size, and the measured peak RSS, both printed |
| That a metric was GPU-computed when CPU ran it | The backend is recorded per run | The backend name in the run-metadata block |
| That a sampled statistic is exact | `STAT-005` already types this | The fidelity label the data model carries |

`DIAGNOSTIC_ARCHITECTURE.md` §8 specifies the report's exact wording; `QM-0090`
audits every string against this table.

---

## 6. The sphere-per-scalar reconciliation

Retained from the previous version because it remains the correct answer for the
deferred workspace, and because deleting it would lose the reasoning.

The product requirement — *"each matrix should visualize as multiple sphere
blocks … each sphere block represents a single scalar value"* — appears to collide
with the §19 prohibition on one primitive per weight. It does not: **one sphere
per scalar within a bounded, explicitly selected block — yes. One GLB primitive
per parameter of the model — never.** A 256×256 block is 65 536 spheres,
comfortably inside both the `MAX_INSTANCES_PER_TILE` and `MAX_WORKSPACE_SPHERES`
ceilings; a 4096×4096 tensor is 16.7 million and is never rendered as spheres at
all. Full derivation in [`GRID_ARCHITECTURE.md`](GRID_ARCHITECTURE.md) §5–§6.

**v1 renders no spheres.** Its surface is a 2D heat-map (`QM-0150`), and the same
ceiling-and-degrade discipline applies to it: above the rendering ceiling, degrade
to an aggregate cell and say so in the UI (`SURF-002`), never silently truncate.
