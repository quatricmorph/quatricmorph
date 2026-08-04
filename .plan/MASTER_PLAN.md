# MASTER_PLAN — Quatricmorph first working MVP

## 1. Current repository summary

Verified by running the commands, not by reading claims. Evidence in
[`REPOSITORY_ANALYSIS.md`](REPOSITORY_ANALYSIS.md).

```bash
cargo test --workspace          # 290 passed; 0 failed
cd apps/web && npx vitest run    # 101 passed (12 files)
```

**391 tests pass.** The repository is not a matrix-multiplication demo awaiting a
rebrand. It is a partially built tensor platform:

| Layer | State |
| --- | --- |
| Rust workspace | 17 crates, ~15 200 lines, at repository root (`ADR-001`) |
| SafeTensors ingestion | Working. Single-file and sharded, byte-range reads, cancel/resume, stable IDs, 18 requirements `Verified` |
| NSIR / addressing | Working. Canonical addresses, alias grammar, generic + Llama resolvers, ambiguity returns candidates |
| Catalog | Working. SQLite, versioned migrations, hierarchy queries. A 10¹²-parameter manifest indexes in **35.7 MB peak** |
| WeightQL | Parses, resolves, shape-checks, costs, and executes scalar/slice reads. Matmul **plans but does not execute** |
| Statistics | CPU reference computed and tested. **Never persisted** — the API returns 501 |
| `.qtile` v1 | Encode/decode round-trips byte-exact, rejects 8 corruption classes. **No pyramid is ever generated** |
| GLB / `tileset.json` | Types and guardrails only. Both builders **refuse rather than emit a placeholder** |
| CesiumJS viewer | LOD policy and daemon client are tested. **Nothing renders** |
| Matrix workspace | `mm` ported to TypeScript. Pure math extracted and tested. `GridRuler3D` exists and holds a grid invariant |
| Cache | L1 and L2 work and survive reopen. **Nothing calls them** |
| CUDA | Four `.cu` files, **never compiled, never executed**. `q-cuda` returns `NotImplemented` for every operation |
| Daemon | 8 routes serve real data; 5 return 501 carrying a requirement ID |

The honest one-line summary: **the metadata and addressing spine is real and
tested; nothing renders, nothing computes on a GPU, and no visual artifact has
ever been produced.**

## 2. Target MVP

The MVP is the end-to-end path in the task specification §2, executed on a real
checkpoint fixture. **The v1 transform-pipeline input is
`models/distilbert-distilgpt2/`** — a local, single-file, un-sharded SafeTensors
checkpoint (GPT-2/distilgpt2, generic resolver). It is gitignored, not committed;
the sharded/trillion-manifest path continues to be proven by the synthetic
fixtures under `fixtures/` and `crates/q-catalog/tests/trillion_scale_manifest.rs`,
not by `models/`:

```text
Local or sharded SafeTensors checkpoint
→ header inspection and architecture resolution     [built]
→ canonical tensor metadata in the catalog          [built]
→ block-addressable tensor catalog                  [built]
→ statistics and visual encoding over blocks        [CPU built, not wired; CUDA unbuilt]
→ multiresolution .qtile tiles                      [format built, generation missing]
→ GLB tile content                                  [missing]
→ tileset.json                                      [missing]
→ CesiumJS model viewer                             [missing]
→ select model / layer / tensor / block             [missing]
→ query exact values or slices                      [built]
→ open selected tensors in the matrix workspace     [adapter is a stub]
→ visualize A @ B on the shared 3D grid ruler       [layout built, execution missing]
→ control the scene through chat, selectors, KaTeX  [parser and preview built, chat missing]
```

The work is therefore concentrated in the **middle and the far end** of the
pipeline. Ingestion is not the problem. Rendering, artifact generation, and
execution are.

### What "trillion-scale" means in this MVP

It means exactly four things, and this plan never widens them:

1. A trillion-parameter **sharded manifest can be indexed** from `config.json`,
   `model.safetensors.index.json`, and SafeTensors headers, in bounded memory.
   *Already proven*: `crates/q-catalog/tests/trillion_scale_manifest.rs` indexes
   47 278 tensors describing 1.048×10¹² parameters and 2.10 TB of payload using
   35.7 MB of peak allocation — a 56 040:1 ratio — while opening no artifact.
2. Model metadata is **browsable without loading weight bytes**.
3. Tensor data is reached only by **byte-range read of a selected region**.
4. Conversion, statistics, and tiling run **block by block**, cancellable,
   resumable, and cached.

It does **not** mean, and this plan will not claim, that a trillion-parameter
checkpoint can be loaded into RAM, into an RTX 3090's 24 GB of VRAM, into a
browser, or into a GLB. See [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §5.

### v1 GPU lane: Metal, not CUDA

**v1 uses Metal as its only GPU compute lane for the conversion stage.** The
development and target hardware for v1 is Apple silicon with no NVIDIA GPU
present, so the conversion stage (block statistics, quantization, visual
encoding) runs on CPU with Metal as the accelerated path, both behind the same
`q_gpu::Backend` trait. CUDA is **not** part of v1; it is an explicit next
step — see "CUDA: deferred to next step" below.

### CUDA: deferred to next step (post-v1)

An RTX 3090 has **24 GB of VRAM**. At fp32, that is roughly 6×10⁹ parameters if
nothing else were resident — about 0.6 % of a trillion-parameter model, before
any working buffers, and in practice far less. When the CUDA lane is taken up
post-v1, the architecture treats the GPU as a **block processor with a hard
ceiling**, never as a place where a model lives:

* `q_cuda::RTX_3090_VRAM_BYTES` and `USABLE_VRAM_FRACTION = 0.80` already exist
  and are enforced by `the_vram_ceiling_is_enforced_without_a_device`.
* Every kernel operates on one host-streamed block. `gpu/cuda/README.md` states
  this and this plan keeps it for when the lane is implemented.
* Every budget in [`MEMORY_BUDGET.md`](MEMORY_BUDGET.md) is a formula over a
  configuration variable, not a fixed promise.
* **v1 completes with zero CUDA.** CUDA is a post-v1 accelerator lane, not on
  the critical path, and not scheduled for v1 — see §5.

## 3. Program boundaries

Seven executable or clearly isolated subsystems, mapped to what exists today.
Full detail in [`TARGET_ARCHITECTURE.md`](TARGET_ARCHITECTURE.md).

| # | Subsystem | Today | MVP delta |
| --- | --- | --- | --- |
| 1 | SafeTensors ingestion and metadata catalog | `q-source`, `q-safetensors`, `q-architecture`, `q-nsir`, `q-catalog` | Qwen resolver; statistics and tile rows persisted |
| 2 | Block runtime and accelerated conversion | `q-tensor-runtime`, `q-statistics`, `q-gpu`, `gpu/metal/` (v1); `q-cuda`, `gpu/cuda/` (next step) | Streaming block reader; CPU conversion pass; job runner; Metal build and differential verification (v1); CUDA build and differential verification (post-v1) |
| 3 | Tile, GLB, and tileset compiler | `q-tiles`, `q-gltf`, `q-tileset` | Pyramid generation; instanced GLB; `tileset.json`; atomic resumable output |
| 4 | Local query and tensor-block service | `q-daemon`, `q-cli` | Serve tiles and statistics; conversion jobs; cancellation; origin policy |
| 5 | CesiumJS model viewer | `apps/web/model-viewer` (shell + tested policy) | An actual viewer: tileset load, LOD, picking, inspector, exactness badges, search |
| 6 | Grid-aligned matrix workspace | `apps/web/quatricmorph-workspace` | Shared grid core; N-D axis binding; ruled-grid rendering; sphere-block cells; live block adapter; real-block matmul |
| 7 | Chat, selector, WeightQL, KaTeX interface | `apps/web/query-interface` | Chat → plan; candidate resolution; cost preview; cancellation; sanitization |

## 4. Phase summary

Ten phases, 62 tasks. Entry and exit conditions in `phases/*/README.md`.

| Phase | Name | Tasks | Outcome |
| --- | --- | --- | --- |
| 00 | Repository baseline and shared contracts | 5 | Baseline confirmed; divergences registered; LOD-capable fixture; **one shared spatial contract** consumed by Rust and both web apps |
| 01 | SafeTensors ingestion completion | 4 | Qwen resolver; model-level metadata from `config.json`; manifest generator promoted to a tool |
| 02 | Catalog and NSIR completion | 4 | Statistics, blocks, and tiles persist; candidate resolution surfaced |
| 03 | Block runtime and compute | 8 | Bounded streaming block reader; CPU conversion pass; job runner; cache wired; CUDA build and differential verification |
| 04 | Tensor tiles, GLB, and tileset | 7 | `.qtile` pyramid, instanced GLB, `tileset.json`, atomically written and resumable, externally validated |
| 05 | Cesium model viewer | 8 | A tileset renders; a click resolves to a canonical address; exactness is visible |
| 06 | Grid matrix workspace | 9 | One grid ruler for everything; sphere-block cells; real blocks multiplied and animated |
| 07 | WeightQL and chat | 6 | Matmul executes; statistics queries; chat produces plans with cost preview and cancellation |
| 08 | Integration and performance | 6 | The §32 end-to-end demonstration; soaks; failure injection; scaling |
| 09 | Documentation and release | 5 | Docs match reality; `STATUS.md` regenerated; acceptance audit against 46 criteria |

## 5. Critical path

The longest chain of `Complete`-blocking dependencies. **It contains no CUDA
task.**

```text
QM-0001 baseline verification
  → QM-0004 shared spatial contract (schemas/visualization)
  → QM-0003 LOD-capable fixture + goldens
  → QM-0030 bounded streaming block reader
  → QM-0031 CPU statistics conversion pass
  → QM-0033 job runner (checkpoint, atomic, resume)
  → QM-0040 LOD ladder and block layout planner
  → QM-0041 .qtile pyramid generation
  → QM-0042 instanced GLB tile builder
  → QM-0044 tileset.json generation
  → QM-0051 viewer loads a tileset from the daemon
  → QM-0053 feature pick → canonical tensor address
  → QM-0066 live tensor-block adapter
  → QM-0067 real-block matmul in the workspace
  → QM-0080 end-to-end demonstration
  → QM-0094 MVP acceptance audit
```

Sixteen tasks. Every one of them runs on the development machine this repository
was verified on (darwin, Apple silicon) with no NVIDIA hardware.

**Why CUDA is off the critical path.** `q_gpu::CpuBackend` is `GPU-002 Verified`
and `q_gpu::block_statistics_default` already computes the statistics the tile
pyramid needs. Routing Phase 04 through the CPU backend makes the pipeline
buildable and demonstrable today; a Metal backend then accelerates it in v1
behind the same `q_gpu::Backend` trait without changing a single downstream
artifact, and a CUDA backend replaces/joins it as the **next step, after v1**.
`docs/decisions/ADR-008-track-b-prerequisite-waiver.md` already waives the RTX
3090 gate for exactly this reason. If CUDA were on the critical path, the MVP
would be unbuildable in the environment that must build it.

## 6. Parallel workstreams

Five lanes that can proceed concurrently once Phase 00 completes. Shared-file
risks are enumerated in [`DEPENDENCY_GRAPH.md`](DEPENDENCY_GRAPH.md).

| Lane | Tasks | Touches | Blocked by |
| --- | --- | --- | --- |
| **A — Artifact pipeline** (critical) | `QM-0030`…`QM-0046` | `crates/q-tensor-runtime`, `q-statistics`, `q-tiles`, `q-gltf`, `q-tileset`, `q-catalog` | `QM-0004` |
| **B — Viewer** | `QM-0050`…`QM-0057` | `apps/web/model-viewer` | `QM-0004`; `QM-0044` for anything that must render real data |
| **C — Workspace** | `QM-0060`…`QM-0068` | `apps/web/quatricmorph-workspace` | `QM-0004`; `QM-0066` needs a running daemon |
| **D — Query and chat** | `QM-0070`…`QM-0075` | `crates/q-weightql`, `q-expression`, `q-gpu`, `apps/web/query-interface` | `QM-0020` for statistics queries |
| **E — Metal accelerator (v1)** | `QM-0037`, new Metal build/kernel tasks | `crates/q-gpu`, `gpu/metal/` | Blocks nothing on the critical path; targets Apple silicon (dev/target hardware) |
| **F — CUDA accelerator (next step, post-v1)** | `QM-0034`…`QM-0036`, `QM-0083` | `crates/q-cuda`, `gpu/cuda/` | **Requires RTX 3090. Deferred to post-v1.** Blocks nothing on the critical path |

Lanes B and C both consume the shared spatial contract from `QM-0004` but write
to different packages, so they do not conflict after it lands. Lane A owns the
Rust artifact crates exclusively.

## 7. Integration checkpoints

Five gates. Each is a task whose acceptance criteria are integration-level, and
no downstream phase starts until its gate is `Verified`.

| Gate | Task | Proves |
| --- | --- | --- |
| **G1 — Contract** | `QM-0005` | Rust and both web apps derive the grid parameters, LOD ladder, and geometric-error rule from one schema; a cross-language test fails if they drift |
| **G2 — Artifact** | `QM-0046` | Generated `.qtile`, GLB, and `tileset.json` validate against external validators, not just our own round-trip |
| **G3 — Render** | `QM-0053` | A camera-driven tileset load renders, and a click resolves to the correct canonical tensor address |
| **G4 — Exactness** | `QM-0080` | A value clicked in the viewer equals the value Python's `safetensors` reads at the same index |
| **G5 — Release** | `QM-0094` | All 46 MVP acceptance criteria have cited evidence, or a written waiver |

## 8. Release criteria

The MVP ships when every criterion in [`DEFINITION_OF_DONE.md`](DEFINITION_OF_DONE.md)
is `Verified` or carries a written, signed-off waiver naming the reason and the
task that would close it. In summary form, the MVP requires:

1. `cargo test --workspace` and `npx vitest run` pass with no failures and no
   newly ignored tests.
2. The §32 end-to-end demonstration runs from a clean checkout on a machine with
   no NVIDIA GPU, and its output artifacts validate externally.
3. One exact scalar retrieved through the viewer matches the Python
   `safetensors` reference for the same index.
4. `STATUS.md` is regenerated from a real run and contains no row whose status is
   more favourable than its evidence.
5. `ARCHITECTURE.md`'s recorded divergences are resolved — either the code
   changed or the document did, by task, with an ADR.
6. The documentation contains no claim that one RTX 3090 can hold or fully
   compute a one-trillion-parameter model.
7. `mm`'s MIT license and Meta Platforms attribution are intact at
   `mm/LICENSE`, `apps/web/quatricmorph-workspace/LICENSE`, and
   `apps/web/quatricmorph-workspace/NOTICE.md`.

## 9. Explicit non-goals

Restated here so no task can quietly adopt one. The full list with rationale is
in [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §4.

Training or gradient visualization · autodiff · a full inference runtime ·
token-conditioned hidden states · runtime attention probabilities · LoRA editing
· model morphing · distributed execution · multi-user collaboration · accounts ·
a remote control plane · notebook integration · Hugging Face Hub browsing ·
arbitrary Python or JavaScript execution · a native Metal or Vulkan renderer ·
Tauri packaging · a WebGPU renderer replacing CesiumJS · multi-GPU scheduling ·
full-model spectral decomposition · automatic semantic interpretation of weight
patterns · **full trillion-parameter numerical execution on one RTX 3090**.

Extension points for several of these are designed in and named in
[`TARGET_ARCHITECTURE.md`](TARGET_ARCHITECTURE.md) §7. Designing an extension
point is in scope. Implementing it is not.

## 10. What this plan does not do

It does not complete the Quatricmorph MVP. It is a plan. The 62 tasks in
`tasks/` are the work; this directory is the description of the work.
