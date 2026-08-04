# TARGET_ARCHITECTURE — the program boundaries the MVP ends with

## 1. Principle: the layout does not change, the seams get filled

`ARCHITECTURE.md` §16 specifies the crate and application layout, and the
repository already implements it (`ADR-001` records the one departure: the
workspace sits at the repository root, not under `quatricmorph/`). **No task in
this plan creates, renames, moves, or deletes a crate.** The target layout is
the current layout with four things added:

1. A **shared spatial contract** that Rust and both web applications consume.
2. A **conversion executor** — the thing that turns catalog rows into artifacts.
3. A **shared web core package** so the viewer and the workspace use one grid.
4. Artifact **writers** where `q-tileset` and `q-gltf` currently refuse.

## 2. Target layout

`+` marks what this plan adds. Everything unmarked exists today.

```text
quatricmorph/                        (repository root)
├── Cargo.toml                       workspace, 18 members
├── crates/
│   ├── q-source/                    Artifact plane: ModelSource, budgets, ids, dtype
│   ├── q-safetensors/               Artifact plane: header, index, ingest, read
│   ├── q-architecture/              Metadata: plugin registry
│   ├── q-nsir/                      Metadata: canonical address, alias, resolver
│   ├── q-catalog/                   Metadata: SQLite, migrations, jobs
│   ├── q-tensor-runtime/            Metadata: Lod, BlockExtent, TensorBlock, TileId
│   │   + src/stream.rs              +  bounded block streaming reader
│   ├── q-statistics/                Tensor Tile: CPU reference statistics
│   ├── q-weightql/                  Metadata: lexer, parser, plan
│   ├── q-expression/                Metadata: closed AST
│   ├── q-tiles/                     Tensor Tile: .qtile v1
│   │   + src/pyramid.rs             +  LOD pyramid generation
│   ├── q-gltf/                      Visualization: GlbTileSpec, guardrails
│   │   + src/instanced.rs           +  the builder that actually emits
│   ├── q-tileset/                   Visualization: TilesetNode, geometric error
│   │   + src/builder.rs             +  the builder that actually emits
│   ├── q-cache/                     L1 + L2
│   ├── q-gpu/                       Backend trait, CpuBackend reference
│   ├── q-cuda/                      CudaBackend
│   │   + build.rs                   +  nvcc, feature-gated
│   │   + src/ffi.rs                 +  kernel bindings
│   ├── q-daemon/                    axum HTTP service
│   │   + src/jobs.rs                +  the conversion executor
│   └── q-cli/                       clap CLI
├── gpu/
│   ├── cuda/                        reduce · histogram · matmul · quantize
│   ├── metal/                       placeholder (extension point)
│   └── wgsl/                        placeholder (extension point)
├── apps/web/                        npm workspaces
│   + ├── core/                      +  SHARED: grid, LOD, addresses, exactness
│   ├── model-viewer/                CesiumJS browser
│   ├── matrix-workspace/            Three.js, from mm
│   └── query-interface/             WeightQL + KaTeX
├── architectures/                   generic · llama · (+qwen) · kimi · deepseek
├── schemas/                         nsir · qtile · weightql · visualization(+spatial)
├── fixtures/                        tiny-llama-{single,2shard} + (+generated large)
├── tests/                           cross-crate integration
├── docs/                            requirements, ADRs, evidence
└── .plan/                           this plan
```

### Why `apps/web/core/` is a new package and not a fourth copy

The product requirement is that one 3D grid system be *shared across all
visualizations and mathematical operations*. Today the grid lives in
`matrix-workspace` and the LOD policy lives in `model-viewer`, each unaware of
the other, and both hand-mirror Rust constants
([`CURRENT_ARCHITECTURE.md`](CURRENT_ARCHITECTURE.md) §6.4).

`apps/web/core/` is a workspace package holding exactly the things both
applications must agree on, generated from or asserted against
`schemas/visualization/schema.json`:

| Module | Contents | Replaces |
| --- | --- | --- |
| `spatial/grid.ts` | The ten grid parameters, snap, `assertSnapped`, cell-centre derivation | `matrix-workspace/src/layout/grid-ruler.ts` (re-exported for compatibility) |
| `spatial/axes.ts` | Axis binding: tensor axes → world axes; rank ≤ 3 implemented, rank > 3 refuses | new (`GRID-007`) |
| `lod/ladder.ts` | The 6-level ladder, distance thresholds, geometric-error rule | `model-viewer/src/lod-policy.ts` constants |
| `address/canonical.ts` | Canonical address parse/format, alias forms | duplicated ad-hoc parsing |
| `fidelity/exactness.ts` | `metadata \| aggregate \| sampled \| quantized \| exact` and its badge contract | `block-adapter.ts`'s local `Fidelity` type |

`matrix-workspace` and `model-viewer` depend on it. Nothing else changes.

## 3. The seven MVP subsystems

Each is independently runnable and independently testable, which is what the task
specification §5 means by "clearly isolated executable subsystems".

### 3.1 Ingestion and metadata catalog

**Owns:** `q-source`, `q-safetensors`, `q-architecture`, `q-nsir`, `q-catalog`.
**Runs as:** a library, driven by `q-cli inspect` or by daemon bootstrap.
**Contract out:** `ModelManifest`, `TensorDescriptor`, canonical addresses, and
the SQLite catalog file.
**Invariant:** never allocates proportionally to checkpoint size (`SRC-007`),
never reads payload at metadata scale (`SRC-018`).
**MVP delta:** Qwen resolver; model-level metadata from `config.json`.

### 3.2 Block runtime and conversion

**Owns:** `q-tensor-runtime`, `q-statistics`, `q-gpu`, `q-cuda`, `gpu/`.
**Runs as:** a library plus the executor in `q-daemon/src/jobs.rs`.
**Contract out:** `TensorBlock` → `BlockData` → `TensorStatistics` + quantized
visual records, under named budgets.
**Invariant:** every buffer is bounded and named; no stage holds a whole tensor;
CPU is the numerical reference and any other backend is diffed against it.
**MVP delta:** streaming reader, conversion pass, job executor, cache wiring,
Metal build and differential verification (v1); CUDA build and differential
verification (next step, post-v1).

### 3.3 Tile, GLB, and tileset compiler

**Owns:** `q-tiles`, `q-gltf`, `q-tileset`.
**Runs as:** a library invoked by the job executor; `q-cli convert` for humans.
**Contract out:** `*.qtile`, `*.glb`, `tileset.json` on disk, plus `visual_tiles`
rows.
**Invariant:** GLB never carries values a `.qtile` does not also carry
(`GLB-003`); instance count is capped (`MAX_INSTANCES_PER_TILE`); every write is
to a temporary file with an atomic rename.
**MVP delta:** all three builders.

### 3.4 Local query and tensor-block service

**Owns:** `q-daemon`, `q-cli`.
**Runs as:** `cargo run -p q-daemon -- --model-root <dir>`.
**Contract out:** the HTTP API in [`API_CONTRACTS.md`](API_CONTRACTS.md).
**Invariant:** file access confined to configured roots (`SEC-001`); every 501
carries a requirement ID; a shape mismatch is a 400 *before* any read.
**MVP delta:** serve tiles and statistics; conversion jobs; progress; cancel and
resume; origin policy.

### 3.5 CesiumJS model viewer

**Owns:** `apps/web/model-viewer`, depends on `apps/web/core`.
**Runs as:** `npm run dev --workspace model-viewer`.
**Contract in:** `tileset.json`, GLB, `.qtile`, and the metadata API.
**Invariant:** camera movement alone never triggers an exact read (`AC-006`);
Cesium performs no tensor arithmetic; every displayed number carries a fidelity
badge.
**MVP delta:** the entire renderer.

### 3.6 Grid-aligned matrix workspace

**Owns:** `apps/web/matrix-workspace`, depends on `apps/web/core`.
**Runs as:** `npm run dev --workspace matrix-workspace`.
**Contract in:** a bounded tensor block from the daemon, or hand-entered values.
**Invariant:** every position is grid-snapped within `1e-6` and derived, never
stored; no request may pull a whole tensor into the browser (`GRID-005`).
**MVP delta:** shared grid core, ruled-grid rendering, sphere-block cells with a
value→opacity channel, live block adapter, real-block matmul.

### 3.7 Chat, selector, WeightQL, and KaTeX interface

**Owns:** `apps/web/query-interface`, depends on `apps/web/core`.
**Runs as:** embedded in the viewer shell, centre-bottom.
**Invariant:** chat never reads bytes; it emits a WeightQL plan, shows its cost,
and requires an explicit act to execute (`ARCHITECTURE.md` §15, §19).
**MVP delta:** chat→plan, candidate resolution UI, cost preview, cancellation,
KaTeX sanitization.

## 4. Cross-subsystem contracts

The only things any two subsystems may assume about each other.

| Contract | Defined in | Consumed by |
| --- | --- | --- |
| Canonical tensor address | `q-nsir`, `schemas/nsir/schema.json` | catalog, WeightQL, daemon, both web apps, cache keys, URLs, logs |
| `TileId` (16 bytes, hex) | `q_tensor_runtime::TileId` | tiles, GLB, tileset, catalog, viewer picking |
| `.qtile` v1 binary | `crates/q-tiles`, `schemas/qtile/schema.json` | writer, daemon, viewer, workspace |
| Tileset node shape | `schemas/visualization/schema.json` | `q-tileset`, catalog, viewer |
| **Spatial contract** (grid + LOD + geometric error) | `schemas/visualization/schema.json` **(extended by `QM-0004`)** | `q-tileset`, `q-tensor-runtime`, `apps/web/core`, both web apps |
| Fidelity vocabulary | `q_source::AccessScale` + `apps/web/core/fidelity` | every API response, every UI badge |
| Cache key | `q_cache::CacheKey` | statistics, tiles, query results |
| WeightQL plan | `schemas/weightql/schema.json` | daemon, chat, query interface |
| HTTP API | [`API_CONTRACTS.md`](API_CONTRACTS.md) | both web apps, `q-cli` |

A change to any of these is a **breaking change** and needs a version bump plus a
conformance-test update. `SCHEMA_PLAN.md` §5 gives the procedure.

## 5. Data flow at MVP completion

```text
1. IMPORT        q-cli inspect | daemon bootstrap
   safetensors headers → TensorDescriptor → NSIR → catalog rows
   reads ~20 KB of a 1.2 MB checkpoint; nothing proportional to size

2. CONVERT       POST /v1/conversions → job executor
   for each tensor, for each block (bounded, checkpointed, cancellable):
     stream block  → q_gpu::Backend (CPU today, Metal in v1, CUDA next step)
     → TensorStatistics + quantized visual cells
     → .qtile (atomic write)  → cache
   then per LOD level: → .glb (instanced) → visual_tiles rows
   finally: → tileset.json (atomic write)

3. BROWSE        model-viewer
   GET tileset.json → Cesium traverses by camera → GET tile.glb
   camera movement loads tiles ONLY; never exact values

4. SELECT        click a feature
   featureId → TileId → tensor_id + BlockExtent → canonical address
   inspector shows metadata + statistics, badged by fidelity

5. QUERY         query-interface / chat
   text → WeightQL AST → resolve → shape check → cost estimate → plan
   user confirms → execute → exact values by byte range

6. COMPUTE       matrix workspace
   selected block → bounded request → grid-aligned sphere cells
   assign A and B → validate shapes → A @ B → animate C[i,j] accumulation
```

Steps 1, 5's read path, and 6's math are built. Steps 2, 3, 4 are the MVP's
substance.

## 6. What runs where

| Concern | Where | Never |
| --- | --- | --- |
| SafeTensors header parsing | Rust, host | GPU, browser |
| Byte-range reads | Rust, host, mmap | Browser |
| Catalog queries | Rust, SQLite | GPU, browser |
| Block statistics, quantization, histograms | `q_gpu::Backend` — CPU now, Metal in v1, CUDA next step | Browser |
| Morton encoding | Same backend | Browser |
| Block matmul | Same backend; small blocks may also run in the browser for animation | — |
| `.qtile` / GLB / `tileset.json` writing | Rust, host | GPU, browser |
| Tile traversal, culling, LOD selection | CesiumJS, browser | Rust |
| Cell placement and colour mapping | Browser (shader or CPU) | Baked into the GLB |
| Expression parsing | Both — Rust is authoritative, browser mirrors for preview | — |
| Plan execution | Rust, daemon | Browser |

## 7. Extension points, named

Each is a real seam in the MVP that refuses with a requirement ID. Implementing
any of them is out of scope ([`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §2) —
**except the Metal implementation of `q_gpu::Backend`**, which is v1 work
(`ADR-CANDIDATE-003`, `Decided`); CUDA remains the deferred, out-of-v1-scope
implementation of that same seam.

| Seam | Where | Opens |
| --- | --- | --- |
| `q_gpu::Backend` | `crates/q-gpu/src/lib.rs:73` | Metal implemented in v1; CUDA (next step, post-v1), wgpu, distributed remain seams |
| `ModelSource` | `crates/q-source/src/lib.rs` | HTTP Range, object storage, Hub |
| `CacheTier` | `crates/q-cache/src/lib.rs:98` | L0 GPU, L3 browser, L4 CDN |
| Architecture plugin registry | `crates/q-architecture` | Any model family |
| `Expr` enum | `crates/q-expression` | New operators, reductions, comparisons |
| `spatial/axes.ts` axis binding | `apps/web/core` | Rank > 3 tensors |
| Tileset node fields | `q_tileset::TilesetNode` | Implicit tiling, subtree availability |
| Visual encoding as viewer-side mapping | not baked into GLB | `CustomShader`, WebGPU renderer |
| Job block manifest | `q_catalog::job` | Distributed workers |
