# Quatricmorph — Master Document

Project-level context for the Quatricmorph trillion-scale tensor visualization MVP. **Implementation architecture source of truth:** [`ARCHITECTURE.md`](ARCHITECTURE.md). If this document conflicts with `ARCHITECTURE.md`, follow `ARCHITECTURE.md`.

Related documents: [`docs/PRODUCT_BRIEF.md`](docs/PRODUCT_BRIEF.md), [`docs/ROADMAP.md`](docs/ROADMAP.md), [`docs/requirements/VIZ_MVP.md`](docs/requirements/VIZ_MVP.md), [`docs/requirements/MVP_REQUIREMENTS.md`](docs/requirements/MVP_REQUIREMENTS.md), [`AGENTS.md`](AGENTS.md).

---

# 1. Product Vision

Quatricmorph is a spatial computational environment for inspecting, querying, visualizing, and processing neural-network tensors at model scale.

The long-term target is models containing up to approximately:

```text
1 trillion parameters
```

Quatricmorph must allow users to navigate:

```text
Model
→ Subsystem
→ Layer
→ Module
→ Tensor
→ Tensor block
→ Slice
→ Scalar
```

The first MVP must demonstrate the architecture required for trillion-parameter models without pretending that an entire trillion-parameter checkpoint can fit in the memory of one NVIDIA RTX 3090.

For this MVP, **trillion-scale support** means:

* A trillion-parameter sharded checkpoint can be indexed from its manifests and SafeTensors headers.
* Model metadata can be browsed without loading all weight bytes.
* Tensor data can be accessed through byte-range reads.
* Conversion can run incrementally, block by block.
* Conversion jobs can be cancelled, resumed, and cached.
* v1 conversion kernels process selected blocks on Metal (Apple GPU, first verified target for v1). CUDA kernels on an NVIDIA RTX 3090 are the planned **next step, deferred until after v1** (broader backends per `ARCHITECTURE.md` §12).
* Visual LOD artifacts can be generated without creating one visual object per parameter.
* The browser receives only the visual tiles, metadata, and selected exact tensor regions it needs.
* Full-model residency in system RAM, GPU VRAM, browser memory, or GLB files is never required.

The MVP must prove an end-to-end workflow using a manageable real SafeTensors fixture while validating that the metadata, addressing, job, cache, and tiling architecture does not impose a model-size ceiling.

---

# 2. Primary MVP Workflow

## 2.1 v1 — Out-of-core quantization-error diagnostic (current release)

The scope decision that makes this, and not the platform workflow in §2.2, the
first release is recorded in [`ARCHITECTURE.md`](ARCHITECTURE.md) §17.1 and
[`.plan/STRATEGY_ALIGNMENT.md`](.plan/STRATEGY_ALIGNMENT.md).

The first working workflow is:

```text
Real open-weight SafeTensors checkpoint
→ SafeTensors manifest and header inspection
→ Architecture resolution
→ Canonical tensor metadata
→ Block-addressable tensor catalog
→ Bounded streaming block reader under a configured resident-byte ceiling
→ CPU/Metal quantization simulation, block by block (v1; CUDA is a post-v1 accelerator lane)
→ Paired block reduction: base block against its simulated counterpart
→ Per-channel, per-tensor, per-layer weight-space error aggregation
→ Fragility ranking and a bytes-versus-error frontier
→ Deterministic Markdown report + versioned JSON manifest
→ One 2D diagnostic heat-map fed by that manifest
```

Executable subsystems (minimum):

```text
1. SafeTensors ingestion and metadata catalog
2. Bounded streaming block runtime with named memory budgets
3. Quantization simulation and the paired-reduction compute backend (CPU; Metal in v1)
4. Diagnostic engine: aggregation, outlier attribution, ranking, frontier
5. Report and manifest emitter, with CI/agent exit codes
6. Local query and tensor-block service
7. One lightweight diagnostic surface — no Cesium, no Three.js scene graph
```

**Concrete v1 transform-pipeline input:** `models/distilbert-distilgpt2/` — a
local, single-file SafeTensors checkpoint (GPT-2/distilgpt2 architecture, 6
layers, resolved via the generic resolver, not Qwen/Llama). It is not sharded, so
it exercises the ingestion → conversion → diagnostic → report path end to end but
does not exercise the sharded/trillion-manifest path — that remains covered by
the synthetic fixtures in `fixtures/`
(`crates/q-catalog/tests/trillion_scale_manifest.rs`). Larger MoE checkpoints are
out of v1 scope; see [`.plan/MASTER_PLAN.md`](.plan/MASTER_PLAN.md) §4.

v1 release gate: `V1-01` … `V1-32` in
[`.plan/DEFINITION_OF_DONE.md`](.plan/DEFINITION_OF_DONE.md). v1 reports
*weight-space* error, measured; it does not predict a downstream behavioural or
benchmark delta, and that seam refuses with its requirement ID rather than
estimating one ([`.plan/DIAGNOSTIC_ARCHITECTURE.md`](.plan/DIAGNOSTIC_ARCHITECTURE.md) §8).

## 2.2 Platform workflow (the release that follows v1)

Retained in full and still correct. Sequenced after v1
([`ARCHITECTURE.md`](ARCHITECTURE.md) §17.3, Phases 0–6); its acceptance criteria
are §20.2 below.

```text
Local or sharded SafeTensors checkpoint
→ SafeTensors manifest and header inspection
→ Architecture resolution
→ Canonical tensor metadata
→ Block-addressable tensor catalog
→ CPU/Metal-accelerated statistics and visual encoding (CUDA is a post-v1 accelerator lane)
→ Multiresolution tensor tiles
→ GLB tile content
→ tileset.json
→ CesiumJS model viewer
→ Select model, layer, tensor, or block
→ Query exact values or slices
→ Open selected tensors in the matrix workspace
→ Visualize matrix multiplication on a shared 3D grid ruler
→ Control the scene through chat, selectors, and mathematical expressions
```

Executable subsystems (minimum):

```text
1. SafeTensors ingestion and metadata catalog
2. CPU/Metal-accelerated tensor conversion pipeline; CUDA-accelerated lane (next step)
3. GLB, tensor-tile, and tileset compiler
4. Local query and tensor-block service
5. CesiumJS model viewer
6. Grid-aligned matrix multiplication viewer
7. Chat, selector, WeightQL, and KaTeX interface
```

Concrete platform MVP profile ([`ARCHITECTURE.md`](ARCHITECTURE.md) §18):

```text
Model: 0.5B–7B SafeTensors
Architecture: Qwen or Llama-like
Tensor: Q projection or MLP down projection
Viewer: CesiumJS
LOD: model → layer → tensor → block
Query: exact scalar and tensor slice
Math: one A @ B visualization
```

The Qwen/Llama-like profile remains the target family for later real checkpoints;
a GPT-2 resolver may be added if `models/` grows beyond the one fixture named in
§2.1.

---

# 3. Reality and Feasibility Constraints

An RTX 3090 normally has:

```text
24 GB VRAM
```

Therefore the architecture uses:

* Streaming
* Memory mapping
* SafeTensors byte ranges
* Bounded CPU buffers
* Bounded pinned-memory buffers
* Bounded GPU staging buffers
* Block-level GPU execution (Metal in v1; CUDA is the deferred next-step lane)
* Incremental output writing
* Content-addressed caching
* Resumable jobs
* Backpressure
* Explicit memory budgets

These are different capabilities:

```text
Metadata-scale support
Visualization-scale support
Selected-block exact access
Selected-block CUDA processing
Full-model offline conversion
Full-model numerical computation
```

The first MVP may demonstrate exact conversion and visualization on a smaller real model or selected tensors, but its architecture and tests must prove that a trillion-parameter manifest can be indexed and traversed without loading the checkpoint into memory.

**“Supports one trillion parameters” does not mean:**

* Loading all weights into RAM.
* Loading all weights into VRAM.
* Converting every scalar into a cube.
* Storing complete weights in GLB.
* Sending the complete model into CesiumJS.
* Running full-model matrix multiplication on the RTX 3090.
* Completing full conversion within an unrealistic fixed time.

---

# 4. Architectural Principles

## 4.1 Four Data Planes

### Artifact Plane

Contains immutable source artifacts:

```text
config.json
tokenizer.json
model.safetensors.index.json
model-00001-of-XXXXX.safetensors
model-00002-of-XXXXX.safetensors
...
```

Responsibilities:

* Preserve source identity.
* Record source revision and hash.
* Read SafeTensors headers.
* Resolve shard locations.
* Read exact byte ranges.
* Avoid rewriting original checkpoint data.

### Metadata Plane

Contains lightweight queryable entities:

```text
Model
Subsystem
Layer
Module
Tensor
TensorBlock
TensorStatistics
VisualTile
Expression
QueryPlan
QueryResult
VisualizationPreset
ConversionJob
```

Storage candidates for MVP: DuckDB, Arrow, Parquet (choose the smallest suitable combination).

### Tensor Tile Plane

Contains tensor-native multiresolution information:

```text
Global statistics
Layer statistics
Tensor statistics
Block summaries
Quantized samples
Exact selected blocks
Tensor addresses
Logical coordinates
```

Use a tensor-oriented sidecar format such as `*.qtile`. Do not treat GLB as the authoritative tensor store.

### Visualization Plane

Contains only render-oriented artifacts:

```text
tileset.json
GLB tile content
Bounding volumes
Geometric errors
Instance transforms
Feature IDs
Quantized visual classes
Tile-local metadata
Labels
Camera presets
```

Visual artifacts must reference stable metadata and tensor identifiers.

## 4.2 Product axioms

1. Checkpoint bytes are the source of truth; indexes are rebuildable.
2. No tensor transformation is successful until validated.
3. Compatibility must be proven, not inferred from matching shapes alone.
4. A model variant stays virtual until materialization is required.
5. Semantic claims require behavioral or causal evidence.
6. Out-of-core execution is first-class.
7. Results expose cost, approximation, confidence, and provenance.
8. Local execution is the default.
9. Visualization is generated from the same query/lineage substrate as automation.
10. Open formats and reproducible recipes beat proprietary containers.

---

# 5. Program Boundaries and Repository Layout

Target layout ([`ARCHITECTURE.md`](ARCHITECTURE.md) §16):

```text
quatricmorph/
├── crates/
│   ├── q-source/
│   ├── q-safetensors/
│   ├── q-architecture/
│   ├── q-nsir/
│   ├── q-catalog/
│   ├── q-tensor-runtime/
│   ├── q-statistics/
│   ├── q-weightql/
│   ├── q-expression/
│   ├── q-tiles/
│   ├── q-gltf/
│   ├── q-tileset/
│   ├── q-cache/
│   ├── q-gpu/
│   ├── q-daemon/
│   └── q-cli/
├── gpu/
│   ├── wgsl/
│   ├── cuda/
│   └── metal/
├── apps/
│   ├── desktop/
│   └── web/
├── python/
│   └── quatricmorph/
├── architectures/
│   ├── llama/
│   ├── qwen/
│   ├── kimi/
│   ├── deepseek/
│   └── generic/
├── schemas/
│   ├── nsir/
│   ├── qtile/
│   ├── weightql/
│   └── visualization/
├── fixtures/
└── docs/
```

Legacy / reference paths (not the product target):

| Path | Role |
| --- | --- |
| `mm/` | Historical matrix-viz reference — read-only; do not delete |
| `quatricmorph/` | Legacy Three.js experiment — do not expand as product path |

Boundary questions for extraction and new code:

* What can be retained from `mm`.
* What must be extracted from `mm`.
* What belongs in the Rust conversion pipeline.
* What belongs in the CUDA / Metal compute plugins.
* What belongs in the browser.
* What belongs in the local daemon.
* Which components need stable shared schemas.
* Which components can initially remain in one executable.

---

# 6. SafeTensors Ingestion

Rust-based SafeTensors ingestion supports:

* Local SafeTensors files.
* Sharded SafeTensors checkpoints.
* `model.safetensors.index.json`.
* SafeTensors header parsing.
* Tensor names, shapes, dtypes, byte offsets, shard locations.
* Source revisions and hashes.
* Partial byte-range reads.
* Memory-mapped local reads where practical.
* Resumable metadata import.
* Cancellation.
* Corruption and offset validation.

Conceptual source abstraction:

```rust
pub trait ModelSource {
    fn manifest(&self) -> Result<ModelManifest>;

    fn read_range(
        &self,
        uri: &str,
        offset: u64,
        length: u64,
    ) -> Result<ByteStream>;
}
```

Tensor descriptor:

```rust
pub struct TensorDescriptor {
    pub tensor_id: TensorId,
    pub raw_name: String,
    pub canonical_name: String,
    pub shape: Vec<u64>,
    pub dtype: DType,
    pub shard_uri: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub layer_index: Option<u32>,
    pub semantic_role: TensorRole,
}
```

Ingestion must not allocate a buffer proportional to total checkpoint size.

Import flow:

```text
Model URI
→ resolve Hugging Face revision (or local path)
→ download/read index JSON
→ inspect all SafeTensors headers
→ verify offsets and shapes
→ resolve architecture
→ generate canonical tensor IDs
→ persist metadata
→ optionally build coarse summaries
```

---

# 7. Neural Structure Intermediate Representation (NSIR)

NSIR normalizes architecture-specific tensor names into reusable identities.

Example source name:

```text
model.layers.10.self_attn.q_proj.weight
```

Example semantic representation:

```json
{
  "stack": "language",
  "layer": 10,
  "component": "attention",
  "operation": "query_projection",
  "parameter": "weight",
  "axes": [
    "output_channel",
    "input_channel"
  ]
}
```

Architecture resolvers (MVP families first; plugins extend per `ARCHITECTURE.md` §4.2):

```text
generic transformer
Llama-like
Qwen-like
```

Resolvers may return `unknown`. They must never infer semantic meaning solely from matching tensor shapes.

Stable canonical addresses:

```text
model.layers[10].self_attention.query_projection.weight
```

Canonical addresses are reusable across catalog records, URLs, WeightQL, chat responses, Cesium feature metadata, matrix viewer selections, logs, cache keys, and query plans.

---

# 8. Metadata Catalog

Local catalog technology candidates: DuckDB, Parquet, Arrow, SQLite — choose based on MVP needs rather than adopting all at once.

Minimum entities:

```text
models
tensors
tensor_blocks
tensor_statistics
visual_tiles
conversion_jobs
cache_entries
```

Catalog capabilities:

* Model hierarchy queries.
* Layer and module queries.
* Tensor lookup by canonical address or alias.
* Shape, dtype, and semantic-role filters.
* Byte-range resolution.
* Tile-to-tensor and tensor-to-tile resolution.
* Conversion status and cache lookup.
* Exact versus approximate result metadata.

Schema versioning and migration belong from the beginning. Table sketches live in `ARCHITECTURE.md` §5.

---

# 9. GPU-Accelerated Conversion (Metal in v1, CUDA next step)

**v1 uses Metal (and CPU) for the conversion stage.** CUDA on an NVIDIA RTX
3090 is the same `Backend` trait's next lane, deferred until after v1 ships —
see `.plan/CUDA_ARCHITECTURE.md` and
`.plan/decisions/ADR-CANDIDATE-003-metal-build.md`. The conversion-stage
compute plugin (Metal now, CUDA later) handles block-oriented workloads such as:

* FP16 and BF16 conversion.
* Quantization.
* Min/max reduction, mean, variance, L1/L2 norms.
* Positive, negative, and zero ratios.
* Histogram generation.
* Block sampling and value normalization.
* Visual classification.
* Morton-order encoding where beneficial.
* Optional block matrix multiplication and tensor comparison.

Not GPU-plugin responsibilities:

* SafeTensors header parsing.
* Catalog queries.
* File path handling.
* GLB container writing.
* `tileset.json` generation.
* Cesium tile traversal.
* Browser UI state.

Data flow:

```text
CPU reader
→ bounded host buffer
→ optional pinned memory
→ bounded GPU staging buffer
→ Metal kernel (v1) / CUDA kernel (next step)
→ compact output buffer
→ qtile and GLB writer
```

Configurable budgets:

```text
maximum host staging bytes
maximum pinned-memory bytes
maximum GPU staging bytes
maximum concurrent blocks
maximum output queue depth
```

The converter adapts block size under memory pressure. Device discovery, compute-capability validation, VRAM detection, CPU fallback, cancellation between blocks, and resume from completed block manifests are required. Entire tensors must not be required to fit in VRAM.

---

# 10. Tensor Block and LOD Model

Large matrices divide into configurable logical blocks (e.g. `256 × 256`, `512 × 512`) based on dtype, dimensions, GPU budget (Metal in v1), desired LOD, GLB size, Cesium traversal, and picking/query granularity.

LOD hierarchy ([`ARCHITECTURE.md`](ARCHITECTURE.md) §9):

```text
LOD 0 — Model
LOD 1 — Architecture or subsystem
LOD 2 — Layer
LOD 3 — Tensor
LOD 4 — Tensor block
LOD 5 — Sampled or exact scalar region
```

| LOD | Object | Data |
| --- | --- | --- |
| 0 | Model | parameter count, bytes, global distributions |
| 1 | Subsystem | layer ranges, aggregate norms |
| 2 | Layer | tensor count, mean norm, anomaly score |
| 3 | Tensor | shape, dtype, histogram, spectrum summary |
| 4 | Block | block statistics, quantized samples |
| 5 | Scalar region | exact or sampled weight values |

Loading rules:

```text
zoom out → only load summary tiles
zoom in → load tensor metadata
zoom deeper → load block summaries
select or inspect → range-read exact bytes from SafeTensors
```

Tile system defines parent-child relationships, stable tile/block IDs, geometric error, bounding volumes, content URIs, refinement policy, incremental manifests, cancellation, and resumability.

---

# 11. GLB and Tensor Sidecar Output

Conversion flow:

```text
*.safetensors
→ metadata catalog
→ tensor blocks
→ GPU summaries and visual encoding (Metal in v1; CUDA next step)
→ *.qtile
→ *.glb
→ tileset.json
```

## GLB responsibilities

May contain: shared unit geometry, instance transforms, quantized visual classes, feature IDs, tile-local metadata, tensor/block references, selection-compatible metadata, bounds-related data.

Must not contain: complete FP16/BF16 checkpoints, one mesh per parameter, duplicated cube geometry per scalar, complete exact tensor values for the model, the authoritative metadata catalog, or analysis results that belong in tensor-native storage.

Evaluate: `EXT_mesh_gpu_instancing`, `EXT_mesh_features`, `EXT_structural_metadata` — with capability tests and fallbacks.

## Tensor sidecar (`.qtile`)

Example header:

```rust
pub struct QTileHeader {
    pub version: u16,
    pub encoding: u16,
    pub lod: u8,
    pub dimensions: u8,
    pub count: u32,
    pub tensor_id: [u8; 16],
    pub origin: [u32; 3],
    pub extent: [u32; 3],
    pub min_value: f32,
    pub max_value: f32,
}
```

Payload may include Morton coordinates, quantized values, flags, local IDs, sample metadata, and optional exact-value references. Binary schema, endianness, versioning, alignment, compression, checksums, streaming reads, and forward compatibility are part of the format contract.

---

# 12. Spatial Model Layout and CesiumJS Viewer

The CesiumJS viewer represents the model hierarchy as a navigable spatial structure with deterministic placement for Model → Subsystem → Layer → Module → Tensor → Tensor block.

Layout parameters include model origin, layer/module spacing, tensor padding, block cell size, major/minor grid intervals, label margin, frame padding, and depth spacing. Positions derive from logical addresses and layout rules — not arbitrary scattered offsets.

Viewer responsibilities:

```text
Load tileset.json
Traverse model LOD
Render GLB tile content
Pick models, layers, tensors, and blocks
Resolve feature IDs
Display metadata
Navigate hierarchy
Open tensor selections
Coordinate with query service and matrix workspace
Persist view state
```

Default chrome: header, hierarchy/search panel, central Cesium viewport, inspector, center-bottom chat/query box, optional matrix workspace, status and exactness indicators.

Visible data must be labeled as metadata-only, aggregate, approximate, sampled, quantized, or exact. Zooming out must not trigger exact tensor reads; exact values load only after explicit selection or query.

Renderer strategy ([`ARCHITECTURE.md`](ARCHITECTURE.md) §12):

* **Renderer A** — CesiumJS prototype (tile traversal, LOD, picking, visualization MVP).
* **Renderer B** — Native tensor renderer (Tauri + wgpu / WGSL) for procedural cells, compute culling, matrix animation, and large interactive workloads.

Cesium is a tile-traversal and rendering layer, not a tensor compute engine.

---

# 13. Shared 3D Grid Ruler and Matrix Workspace

## GridRuler3D

Coordinate and alignment system shared by tensor planes, matrix/vector/scalar cells, frames, multiplication guides, axis labels, slice selections, result cells, intermediate expression nodes, and camera-fit bounds.

Parameters include `cellSize`, `minorGridSpacing`, `majorGridInterval`, `tensorPadding`, `labelMargin`, `framePadding`, `operandGap`, `axisMargin`, `depthSpacing`, and `origin`.

Invariant (with documented floating-point tolerance):

```text
position.x % cellSize ≈ 0
position.y % cellSize ≈ 0
position.z % cellSize ≈ 0
```

Positions derive from workspace origin + tensor anchor + logical index + block origin + cell size. Do not store independent absolute positions for every scalar.

## Matrix multiplication workspace

Supports selected real tensor blocks or manually entered expressions `A @ B = C`.

Required shape combinations:

```text
Matrix @ Matrix → Matrix
Matrix @ Column Vector → Column Vector
Row Vector @ Matrix → Row Vector
Row Vector @ Column Vector → Scalar
```

Axes (recommended): World X → J, World Y → I, World Z → K. Planes: A → I×K, B → K×J, C → I×J.

Modes:

* **Concept mode** — generated or sampled values to explain the operation.
* **Real tensor-block mode** — explicitly selected checkpoint blocks (e.g. `A[0:256, 0:256] @ B[0:256, 0:256]`).

Do not automatically multiply an entire large tensor merely to produce an animation. Define interactive dimension and block-size limits for GPU and browser.

`mm/` may supply reusable math and visualization behavior for extraction into this workspace; it is not the product surface.

## TensorGridFrame

Every matrix, vector, scalar, or selected tensor block has outer boundary, margins, shape label, canonical address, row/column guides, axis labels, deterministic anchor/orientation, grid-aligned cells, and camera-fit bounds. Markers stay within cell boundaries; numerical labels appear only at suitable zoom or for selections.

## Multiplication interaction

For result cell `C[i,j]`: highlight `A[i,:]`, highlight `B[:,j]`, highlight shared K, show products, update running sum, reveal `C[i,j]`. Controls: Play, Pause, Step, Previous, Reset Calculation, Reset View, Fit View.

Separate state domains: tensor data, expression, layout, selection, animation, camera, display, query, serialized state. Selection must not rely only on color.

---

# 14. WeightQL, Expressions, and Chat

## WeightQL

Standardized query language resolving models, layers, modules, tensors, slices, scalars, statistics, matrix expressions, and comparisons.

Canonical selectors:

```text
model.layers[10].self_attention.query_projection.weight
model.layers[10].self_attention.query_projection.weight[100,42]
```

Aliases:

```text
Q[10]
Q[10][100,42]
K[10][0:256,0:256]
MLP.down[24][:]
Expert[12,37].up[0:128,:]
```

Contextual selectors resolve to a canonical unambiguous tensor address before execution. Ambiguous aliases return candidates instead of silently choosing.

Parsing pipeline:

```text
Input text
→ Tokenization
→ Parser
→ AST
→ Alias resolution
→ Canonical tensor references
→ Shape checking
→ Cost estimation
→ Execution tier selection
→ Query plan
→ Explicit user execution
```

## Mathematical expressions

Constrained MVP subset: tensor reference, slice, transpose, matrix multiplication, basic statistics, comparison.

Planner steps: resolve references and byte ranges; validate shapes; determine exact/sampled/approximate; estimate I/O and host/GPU memory; select CPU or GPU (Metal in v1; CUDA next step); build visualization instructions; require explicit execution for expensive operations. Shape mismatch fails before any GPU kernel launch.

No arbitrary code execution (`eval`, unrestricted SQL/Python, shell interpolation, etc.).

## Chat and query UI

Center-bottom (or centrally anchored) interface supporting natural-language requests, WeightQL, selectors, mathematical expressions, KaTeX, history, suggestions, current-selection context, candidate resolution, cost preview, and cancellation.

Chat must not read SafeTensors bytes directly. Chat produces or invokes a validated WeightQL plan. Results clearly label exact, approximate, sampled, quantized, and statistical interpretations.

---

# 15. Local Daemon, Jobs, and Cache

## Local daemon

Connects browser apps to catalog, source files, cache, and GPU runtime (Metal in v1; CUDA next step). Responsibilities include open/import, exact values and slices, serving tileset/GLB/qtile, executing WeightQL plans, running conversion jobs, progress, cancel/resume, and cache inspection.

Illustrative API groups ([`ARCHITECTURE.md`](ARCHITECTURE.md) §14):

```http
GET /v1/models
GET /v1/models/{modelId}
GET /v1/models/{modelId}/layers
GET /v1/tensors/{tensorId}
GET /v1/tensors/{tensorId}/statistics
GET /v1/tensors/{tensorId}/value
GET /v1/tensors/{tensorId}/blocks
GET /v1/visualizations/{modelId}/tileset.json
GET /v1/visualizations/{modelId}/tiles/{tileId}.glb
GET /v1/visualizations/{modelId}/tiles/{tileId}.qtile
POST /v1/query
POST /v1/conversions
GET /v1/jobs/{jobId}
POST /v1/jobs/{jobId}/cancel
POST /v1/jobs/{jobId}/resume
```

Transport may be HTTP, WebSocket/SSE for progress, direct local invocation, and static-file serving for generated tiles. Safe local-file access boundaries are mandatory.

## Conversion jobs

Resumable jobs track source model, conversion version, configuration hash, phase, current tensor/block, completed/failed blocks, bytes read/written, GPU/CPU time, cache hits, errors, and timestamps.

States: Pending, Inspecting, Indexing, Converting, Writing, Validating, Paused, Cancelled, Failed, Complete.

Checkpoint after bounded work units. Crash recovery must not redo completed blocks. Outputs use temporary files and atomic rename; partially written GLB/qtile must never be published as valid.

## Cache levels

```text
L0 — GPU-resident active blocks
L1 — Process-memory decoded blocks
L2 — Local NVMe content-addressed artifacts
L3 — Browser Cache Storage or IndexedDB
L4 — Future remote object storage
```

MVP requires L0–L3. Cache keys include source model hash, tensor ID, logical slice, LOD, statistics algorithm/version, and visualization encoding (not purely visual palette choices when shaders can apply them).

---

# 16. MVP User Interface — Platform Release

> **This describes the platform release's surface (§2.2), not v1's.** v1 ships
> one lightweight 2D diagnostic heat-map fed by the report manifest — no Cesium
> and no Three.js scene graph (§2.1;
> [`.plan/MASTER_PLAN.md`](.plan/MASTER_PLAN.md) §5). The description below is
> retained unchanged for the release that follows.

## Header

Brand as Quatricmorph (tensor visualization). Do not present the application as `mm`. Retain original-project attribution in repository documentation and license files.

## Model source controls

Open local model / SafeTensors file / sharded checkpoint directory; recent models; import metadata; generate visualization; resume/cancel conversion.

## Navigation

Model hierarchy; layer/module/tensor selectors; canonical-address and alias search; breadcrumbs.

## Cesium controls

Fit model/selection; reset view; hierarchy frames; major/minor grid; labels; tile bounds; LOD status; exactness status. Development-only tile debugging hidden by default.

## Matrix workspace controls

Open selected tensor; select slice; assign A/B; transpose; validate shapes; visualize A @ B; play/pause/step/previous/reset; fit workspace.

## Query interface

Chat input; WeightQL; KaTeX preview; candidate selector; cost estimate; execute/cancel; history.

---

# 17. Explicitly Out of Scope (Platform Release)

Every item below is out of scope for v1 as well. v1 **additionally** defers the
CesiumJS model viewer, the matrix-multiplication workspace, and the chat/KaTeX
query interface to the platform release
([`.plan/PRODUCT_SCOPE.md`](.plan/PRODUCT_SCOPE.md)).

* Training visualization; automatic differentiation; gradient visualization.
* Full inference runtime; token-conditioned hidden states; runtime attention probabilities; complete Q/K/V activation capture.
* LoRA editing; model morphing.
* Distributed cluster execution; multi-user collaboration; user accounts; remote SaaS control plane.
* Notebook integration; full Hugging Face Hub browsing; arbitrary Python execution.
* Full trillion-parameter numerical execution on one RTX 3090.
* Native Metal/Vulkan renderer as MVP deliverable (extension points allowed; Phase 3–4 in roadmap).
* Tauri desktop packaging as MVP deliverable.
* Custom WebGPU renderer replacing CesiumJS as MVP deliverable.
* Multi-GPU scheduling; full-model spectral decomposition.
* Automatic semantic interpretation of visible weight patterns.
* One cube GLB per weight; absolute positions per scalar; sending entire tensors into the browser; treating Cesium as compute.

Extension points may remain for later phases; they are not first-MVP requirements.

---

# 18. Implementation Roadmap

Aligned with [`ARCHITECTURE.md`](ARCHITECTURE.md) §17 and [`docs/ROADMAP.md`](docs/ROADMAP.md).

**Current release — v1, the out-of-core quantization-error diagnostic** (§2.1;
`ARCHITECTURE.md` §17.2). Its tasks and sequence are
[`.plan/MASTER_PLAN.md`](.plan/MASTER_PLAN.md) phases 10–14; its gate is
[`.plan/DEFINITION_OF_DONE.md`](.plan/DEFINITION_OF_DONE.md).

**Platform release — Phases 0–6, following v1.** Retained unchanged and not
renumbered:

| Phase | Name | Goal |
| --- | --- | --- |
| 0 | Tensor Tiling Spike | One SafeTensors file → one large tensor → five LODs → tileset → Cesium → exact cell value |
| 1 | Dense Model Browser | Sharded SafeTensors, architecture resolver, hierarchy, statistics, Cesium LOD, exact lookup, local cache |
| 2 | Mathematical Query Engine | Aliases, slices, transpose, matmul, query plans, visual expression graph |
| 3 | Custom WebGPU Renderer | Procedural cells, storage buffers, compute culling; Cesium overview or replaced in workspace |
| 4 | Native GPU Desktop | Tauri, wgpu, Metal/Vulkan/DX12, CUDA plugin, memory scheduler |
| 5 | Runtime Neural Observability | Hidden states, Q/K/V, attention, MoE routing, prompt-conditioned visualization |
| 6 | Trillion-Scale Remote Execution | Object storage, distributed workers, streaming, shared workspaces, CDN summaries |

Active engineering track: **v1** (`.plan/MASTER_PLAN.md`). Phases 0–6 above are
deferred to the platform release; `TILE-*` in
[`docs/requirements/VIZ_MVP.md`](docs/requirements/VIZ_MVP.md) is that release's
Phase 0 checklist and is not the current coding target.

---

# 19. Test Strategy (Project Context)

Coverage areas:

* **SafeTensors** — headers, shards, offsets, dtypes, corruption, stable IDs, exact scalar/slice, no full-checkpoint allocation.
* **Trillion-scale metadata** — synthetic ~1T-parameter manifest, bounded memory during indexing, navigation without opening all payloads.
* **CUDA** — CPU reference comparisons, FP16/BF16/FP32, reductions, histograms, quantization, OOM adaptation, cancellation, RTX 3090 verification where available.
* **Tile generation** — qtile round trip, GLB/tileset validation, stable feature IDs, resume, atomic output, cache reuse.
* **Cesium viewer** — tileset open, LOD, picking, metadata, missing/corrupt tiles, camera fit, disposal, browser memory.
* **Matrix workspace** — valid and invalid shape combinations; negatives/zeros; grid alignment; stepping; disposal.
* **WeightQL** — canonical/alias/ambiguity, slice, transpose, matmul, cost estimation, cancellation, exactness metadata, resource limits.
* **End-to-end** — open fixture → import → convert → tileset → Cesium selection → exact value vs Python SafeTensors → matrix A @ B → query via chat.

Commands and package-level conventions: [`docs/TESTING.md`](docs/TESTING.md).

---

# 20. MVP Acceptance Criteria

## 20.1 v1 acceptance criteria

v1's release gate is `V1-01` … `V1-32` in
[`.plan/DEFINITION_OF_DONE.md`](.plan/DEFINITION_OF_DONE.md), which is
authoritative for the current release and also records the disposition of each
criterion in §20.2 below. Five of the 32 are external and cannot be closed by
writing code; one may not be waived.

## 20.2 Platform-release acceptance criteria

> **These are the acceptance criteria for the platform release (§2.2;
> `ARCHITECTURE.md` §17.3), not for v1.** They are retained **unchanged** — still
> correct, needed again when that release resumes — and their numbering is
> preserved because other documents cite it.

The platform MVP is complete only when:

1. The application is branded as Quatricmorph.
2. A local SafeTensors file can be opened.
3. A sharded SafeTensors checkpoint can be indexed.
4. Indexing does not load the complete checkpoint into RAM.
5. A synthetic trillion-parameter manifest can be indexed using bounded memory.
6. Model, layer, module, tensor, and block metadata can be browsed.
7. Tensor names map to stable canonical addresses.
8. Unknown semantic roles remain unknown rather than being guessed.
9. Selected tensor blocks can be read by byte range.
10. Metal processing runs on Apple GPU hardware in v1 (first verified target for v1). CUDA processing on an NVIDIA RTX 3090 is deferred as an explicit next step, not required for v1.
11. GPU processing (Metal in v1) uses bounded block buffers.
12. GPU results (Metal in v1) are validated against CPU references.
13. Conversion produces versioned qtile artifacts, valid GLB tile content, and valid `tileset.json`.
14. Generated work can be cancelled and resumed; completed block artifacts are reused from cache.
15. CesiumJS loads the generated tileset and performs camera-based LOD loading.
16. Zooming out does not load exact scalar data.
17. Selecting a visual feature resolves to the correct tensor or block.
18. Clicking or querying a scalar returns the correct exact value matching a Python SafeTensors reference.
19. The UI distinguishes aggregate, sampled, quantized, approximate, and exact information.
20. A selected tensor block opens in the matrix workspace on the shared 3D grid ruler.
21. Compatible matrix blocks can be multiplied; incompatible shapes are rejected before GPU execution (Metal in v1).
22. Multiplication can be animated deterministically with play/pause/step/previous/reset.
23. Users can query canonical addresses and aliases; ambiguous aliases return candidates.
24. Users can submit slice queries and constrained matrix expressions; KaTeX renders expressions.
25. Query cost is estimated before expensive execution; queries can be cancelled.
26. Chat uses WeightQL and cannot directly access arbitrary checkpoint bytes.
27. Repeated selection/reinitialization and GPU jobs (Metal in v1) do not obviously leak browser or device memory.
28. Original license and attribution are preserved.
29. Documentation accurately describes implemented capabilities and limitations.
30. The product does not claim that one RTX 3090 can hold or fully compute a one-trillion-parameter model.

Within the platform release, Phase 0 spike acceptance is the stricter near-term
gate (`ARCHITECTURE.md` §18; `docs/requirements/VIZ_MVP.md`). Do not mark Phase 1+
or morph/export complete based only on Phase 0 or legacy Three.js work.

---

# 21. Architecture Decisions (Open)

Decisions that need explicit ADRs as implementation proceeds:

```text
Rust workspace introduction
CUDA build strategy
SafeTensors library selection
Catalog technology
qtile v1 binary schema
GLB instancing strategy
3D Tiles 1.0 versus 1.1 features
Implicit versus explicit tiling
CesiumJS framework shell
Reuse versus extraction of existing mm code
Local daemon transport
WeightQL parser technology
Browser caching strategy
Canonical tensor ID generation
Model layout algorithm
```

Recommendations are not approved decisions until repository evidence makes alternatives nonviable or an ADR records the choice.

---

# 22. Target Architecture Summary

```text
SafeTensors
→ NSIR semantic model
→ Tensor-native block database
→ Multiresolution Tensor Tiles
→ WeightQL and mathematical expressions
→ CesiumJS overview
  + custom WebGPU tensor renderer
→ Metal / CUDA acceleration
→ runtime activations and model morphing
```

Quatricmorph should not become `SafeTensors → billions of cube GLBs`. It should become:

```text
SafeTensors
→ semantic tensor address space
→ queryable block hierarchy
→ procedural multiresolution visualization
→ exact on-demand computation
```

The tensor database and virtual computational objects form the core layer; visualization is one projection of the same data and query substrate.
