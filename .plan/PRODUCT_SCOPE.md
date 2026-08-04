# PRODUCT_SCOPE — what is in the MVP, what is a hook, what is not

This document exists to stop the MVP expanding. Every capability Quatricmorph
might have falls into exactly one of four buckets. A task may only implement
something in bucket 1. A task may *design for* bucket 2. A task may not touch
buckets 3 or 4.

| Bucket | Meaning | A task may |
| --- | --- | --- |
| **1. MVP capability** | Shipped, tested, demonstrated in the §32 end-to-end run | Implement it |
| **2. Architectural extension point** | A named seam — trait, schema field, enum variant, route — that a later capability plugs into. Returns `NotImplemented` with a requirement ID today | Create the seam; implement the refusal; **not** implement the capability |
| **3. Future capability** | Wanted, planned in `ARCHITECTURE.md` §17 phases 3–6, not now | Nothing |
| **4. Explicit non-goal** | Deliberately excluded, sometimes permanently | Nothing |

The repository already uses bucket 2's idiom consistently: every stub returns
`QError::NotImplemented` carrying its own requirement ID, and
`apps/web/model-viewer/src/tile-client.ts` treats a `501` as *a declared gap, not
a retryable failure*. New extension points must follow that pattern rather than
inventing a placeholder convention.

---

## 1. MVP capabilities

### 1.1 Already built and verified — carried into the MVP unchanged

These need no implementation task. They need a **verification task** confirming
they still hold at release. Cited to `STATUS.md`.

| Capability | Requirements |
| --- | --- |
| Single-file and sharded SafeTensors ingestion, headers only | `SRC-001`…`SRC-004` |
| Memory-mapped byte-range reads; invalid offsets refused | `SRC-005`, `SRC-015` |
| Stable `model_id` / `tensor_id` across reopen | `SRC-006` |
| Ingestion allocates nothing proportional to checkpoint size | `SRC-007`, `AC-001` |
| Cancellable and resumable metadata import | `SRC-009`, `SRC-010`, `AC-003` |
| Corrupt headers, duplicate names, unknown dtypes refused, never guessed | `SRC-012`…`SRC-014` |
| Exact f32 / bf16 / f16 decoding including subnormals | `SRC-016` |
| Canonical addresses; generic and Llama resolvers; `unknown` preserved | `NSIR-001`…`NSIR-004` |
| Alias grammar `Q[10][100,42]`, `MLP.down[24][:]`, `Expert[12,37].up[0:128,:]` | `NSIR-005` |
| Ambiguous alias returns candidates, never a silent pick | `NSIR-007` |
| Versioned, idempotent catalog migrations; future schema refused | `CAT-001`, `CAT-002` |
| Trillion-parameter manifest indexed in bounded memory | `CAT-006` |
| WeightQL lexer, parser, resolution, shape check, cost estimate, plan IDs | `WQL-001`…`WQL-005`, `WQL-010`…`WQL-012` |
| No arbitrary code execution, Rust and browser | `WQL-009`, `SEC-002`, `SEC-003`, `SEC-004` |
| CPU reference statistics, Welford-stable, streaming-equals-batch | `STAT-001`, `STAT-003`…`STAT-006` |
| `.qtile` v1 round trip, corruption rejection, little-endian, lossy self-declaration | `TILE-005`…`TILE-008` |
| Cube-per-weight and GLB-without-sidecar refused | `GLB-002`, `GLB-003` |
| L1/L2 cache with eviction, surviving reopen | `CACHE-001`…`CACHE-004` |
| Daemon: models, layers, tensors, exact value, block window, query | `API-001`…`API-008` |
| Path traversal refused; model-root boundary enforced | `SEC-001` |
| `GridRuler3D` grid invariant within 1e-6 | `GRID-001`, `GRID-002` |
| Pure matmul, blocking, and animation schedule separated from Three.js | `MATMUL-001`…`MATMUL-004` |
| Job state machine with illegal transitions rejected | `JOB-001`, `JOB-003` |

### 1.2 To be built in the MVP

| Capability | Requirement | Phase |
| --- | --- | --- |
| One shared spatial contract: grid parameters, LOD ladder, geometric-error rule, consumed by Rust **and** both web apps | `GRID-006` | 00 |
| LOD-capable fixture large enough to exercise LOD 0–5 and 256×256 blocking | `SRC-019` | 00 |
| Qwen-family architecture resolver | `NSIR-006` (Qwen only) | 01 |
| Model-level metadata (`hidden_size`, `layer_count`, `parameter_count`) from `config.json` | `CAT-011` | 01 |
| Tensor statistics persisted and served | `STAT-002` | 02 |
| `visual_tiles` rows written; tile↔tensor resolution both ways | `CAT-012` | 02 |
| Bounded streaming block reader with named budgets | `TILE-009` | 03 |
| CPU conversion pass: block statistics over a whole tensor | `STAT-008` | 03 |
| Job runner: checkpointing, atomic output, resume, cancellation | `JOB-002` | 03 |
| Cache wired into the block, statistics, and query paths | `CACHE-008` | 03 |
| Metal build integration and differential verification vs CPU (v1's GPU lane; CUDA equivalent `CUDA-007`/`CUDA-008` deferred to the next step, post-v1 — see §2) | `GPU-003` | 03 |
| `.qtile` pyramid generation | `TILE-004` | 04 |
| Instanced GLB tile content with feature IDs | `GLB-001`, `GLB-004` | 04 |
| `tileset.json` generation | `CESIUM-001` | 04 |
| Atomic, resumable artifact writing | `TILE-011` | 04 |
| External artifact validation (glTF validator, 3D Tiles schema) | `TILE-012` | 04 |
| A CesiumJS viewer that renders a generated tileset | `CESIUM-005` | 05 |
| Feature pick → canonical tensor or block address | `CESIUM-007`, `AC-004` | 05 |
| Exactness badges: metadata / aggregate / sampled / quantized / exact | `CESIUM-008`, `AC-010` | 05 |
| Hierarchy navigation, breadcrumbs, search by address and alias | `CESIUM-009` | 05 |
| glTF extension capability probe with fallback | `CESIUM-010` | 05 |
| Shared grid core package used by viewer and workspace | `GRID-006` | 06 |
| Ruled-grid rendering: major, minor, axis labels, origin | `GRID-008` | 06 |
| **Sphere-block cells**: one sphere per scalar within a bounded block, size and colour and opacity from value | `GRID-009` | 06 |
| Sphere budget with documented degradation to aggregate | `GRID-010` | 06 |
| Live tensor-block adapter against the daemon | `GRID-004` | 06 |
| Real-block `A @ B` with deterministic animation and controls | `MATMUL-006` | 06 |
| Matmul execution backend | `WQL-006` | 07 |
| Stacked slice composition | `WQL-008` | 07 |
| Statistical `SELECT … GROUP BY layer_index` | `WQL-007` | 07 |
| Cost preview, explicit execution, query cancellation | `WQL-013`, `API-011` | 07 |
| Chat that emits WeightQL plans and never reads bytes | `CHAT-001` | 07 |
| KaTeX sanitization of user-supplied mathematical text | `SEC-006` | 07 |
| Daemon origin policy | `SEC-007` | 07 |

---

## 2. Architectural extension points

Built as a seam in the MVP. The seam is tested; the capability behind it is not
implemented and refuses with its requirement ID.

| Extension point | Seam in the MVP | Requirement held open |
| --- | --- | --- |
| **Higher-dimensional tensors (rank > 3)** | Axis-binding table maps tensor axes → world axes with a declared facet rule. Rank ≤ 3 implemented; rank > 3 refuses | `GRID-007` |
| **Remote checkpoints over HTTP Range** | `ModelSource` trait; `crates/q-source/src/http.rs` computes ranges correctly, transport refuses | `SRC-008` |
| **Kimi and DeepSeek resolvers** | `architectures/{kimi,deepseek}/plugin.toml` declared with `implemented = false`; registry never lets them claim a model | `NSIR-006` |
| **CUDA acceleration (next step, post-v1)** | `q_gpu::Backend` trait; `CudaBackend` implements the ceiling check and refuses execution until RTX 3090 hardware verifies it. Deferred until after v1 (`ADR-CANDIDATE-002`) | `CUDA-001` |
| **wgpu compute (v1 implements Metal directly; wgpu stays the extension point)** | Same `Backend` trait. `gpu/wgsl/compute.wgsl` remains a placeholder; `gpu/metal/compute.metal` is v1 work, not a placeholder (`ADR-CANDIDATE-003`, `Decided`) | `GPU-003` |
| **L0 GPU-resident cache** | `CacheTier` trait; `LayeredCache` composes tiers | `CACHE-005` |
| **L3 browser and L4 remote cache** | `L3BrowserCache` / `L4RemoteCache` refuse rather than missing silently | `CACHE-006`, `CACHE-007` |
| **Implicit 3D Tiles subdivision** | Tileset builder emits explicit tiles; the node type carries the fields implicit tiling would need | `CESIUM-011` |
| **Cesium `CustomShader` procedural cells** | Visual encoding is a documented value→channel mapping, applied in the viewer, not baked into the GLB | `CESIUM-012` |
| **Runtime activations** | `TensorRole` is a closed enum with no activation variants; adding them is additive | Phase 5 of `ARCHITECTURE.md` §17 |
| **Distributed block workers** | Job records carry a block manifest; nothing assumes a single process owns it | Phase 6 |

---

## 3. Future capabilities

Wanted, architected for in `ARCHITECTURE.md` §17, and **not planned here**. No
task in `.plan/` implements any of these; no acceptance criterion depends on one.

* Custom WebGPU tensor renderer replacing GLB tiles (`ARCHITECTURE.md` Phase 3)
* Native GPU desktop: Tauri, wgpu, Metal/Vulkan/DX12 backends, GPU memory
  scheduler, multi-GPU jobs (Phase 4)
* Runtime neural observability: hidden states, Q/K/V activations, attention
  probabilities, residual stream, MoE routing (Phase 5)
* Trillion-scale remote execution: object storage, distributed block workers,
  Arrow transfer, server-side tile generation, shared workspaces, CDN publishing
  (Phase 6)
* Morph and Verify product verbs (`docs/ROADMAP.md`)
* DuckDB / Arrow / Parquet catalog backend (`CAT-010`; departure recorded in
  `docs/decisions/ADR-003-catalog-sqlite.md`)

---

## 4. Explicit non-goals

From the task specification §26, plus this repository's own §19 prohibitions.
These are not "later". A task proposing one is out of scope by definition.

**Not built, MVP or otherwise:** training visualization · automatic
differentiation · gradient visualization · a full inference runtime ·
token-conditioned hidden states · runtime attention probabilities · complete
Q/K/V activation capture · LoRA editing · model morphing · distributed cluster
execution · multi-user collaboration · user accounts · a remote SaaS control
plane · notebook integration · full Hugging Face Hub browsing · arbitrary Python
execution · full trillion-parameter numerical execution on one RTX 3090 · a
native Metal renderer · a native Vulkan renderer · Tauri desktop packaging · a
custom WebGPU renderer replacing CesiumJS · multi-GPU scheduling · full-model
spectral decomposition · automatic semantic interpretation of visible weight
patterns.

**Structurally forbidden** (`ARCHITECTURE.md` §19 — these are enforced by
existing tests, not merely stated):

| Prohibition | Enforced by |
| --- | --- |
| One cube GLB per weight | `q_gltf::MAX_INSTANCES_PER_TILE = 262_144`; `cube_per_weight_explosions_are_refused` |
| A GLB as the only carrier of tensor values | `a_glb_without_a_qtile_sidecar_is_refused` |
| Storing absolute positions for every scalar | Position derives from `tile origin + logical index + layout rule`; `GridRuler3D` computes, never stores |
| Sending an entire tensor to the browser | `assertBlockIsBounded`; `refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser`; `whole_tensor_reads_are_refused_with_an_explanation` |
| Cesium as a compute engine | Viewer calls the daemon; no tensor arithmetic in `apps/web/model-viewer` |
| Chat freely executing terabyte expressions | Chat emits a plan; the plan carries a cost estimate; execution is a separate explicit act |
| Treating colour as semantic proof | `AGENTS.md` rule 6; documentation and UI copy audited in `QM-0090` |

---

## 5. The sphere-per-scalar reconciliation

The product requirement — *"each matrix should visualize as multiple sphere
blocks … each sphere block represents a single scalar value"* — appears to
collide with the §19 prohibition on one primitive per weight. It does not, and
the distinction is numeric rather than rhetorical.

| | Tiling pipeline (Visualization Plane artifact) | Matrix workspace (interactive view) |
| --- | --- | --- |
| What a sphere is | Never emitted per scalar. A tile carries **instance transforms** over one shared unit mesh, and above `MAX_INSTANCES_PER_TILE = 262_144` the tile refines instead of growing | A **rendered primitive** inside a block the user explicitly selected |
| Ceiling | 262 144 instances per tile, enforced by `GlbTileSpec::validate` | `MAX_WORKSPACE_SPHERES`, defined in `GRID_ARCHITECTURE.md` §6, enforced by `assertBlockIsBounded` |
| Above the ceiling | Refine to children; never emit a bigger tile | Degrade to an aggregate cell representation and **say so in the exactness badge**; never silently truncate |
| Persisted? | No. Position is derived from Morton coordinate + tile origin + cell spacing | No. Derived from `workspace origin + tensor anchor + logical index + block origin + cellSize` |

So: **one sphere per scalar within a bounded, explicitly selected block — yes.
One GLB primitive per parameter of the model — never.** A 256×256 block is
65 536 spheres, comfortably inside both ceilings. A 4096×4096 tensor is 16.7
million and is never rendered as spheres at all; it is rendered as tiles, and
descending into it yields blocks.

The value→channel encoding is specified in [`GRID_ARCHITECTURE.md`](GRID_ARCHITECTURE.md)
§5. Note that `opacity` is a **new** channel: `apps/web/matrix-workspace/src/viz/mat.ts`
has `sizeFromData` and `colorFromData` but nothing drives alpha, and
`src/viz/material.ts` is a `ShaderMaterial` over `public/assets/ball.png`. Adding
opacity is a shader change with its own task (`QM-0063`), and it is deliberately
a **redundant** channel — task specification §18 forbids selection conveyed by
colour alone, so magnitude must remain legible through scale even where opacity
is disabled or the display is monochrome.
