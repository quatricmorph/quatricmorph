# Quatricmorph Trillion-Scale MVP — Full Implementation Planning Prompt

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
* CUDA kernels can process selected blocks on an NVIDIA RTX 3090.
* Visual LOD artifacts can be generated without creating one visual object per parameter.
* The browser receives only the visual tiles, metadata, and selected exact tensor regions it needs.
* Full-model residency in system RAM, GPU VRAM, browser memory, or GLB files is never required.

The MVP must prove an end-to-end workflow using a manageable real SafeTensors fixture while validating that the metadata, addressing, job, cache, and tiling architecture does not impose a model-size ceiling.

---

# 2. Primary MVP Workflow

The first working workflow must be:

```text
Local or sharded SafeTensors checkpoint
→ SafeTensors manifest and header inspection
→ Architecture resolution
→ Canonical tensor metadata
→ Block-addressable tensor catalog
→ CUDA-accelerated statistics and visual encoding
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

The MVP must be split into multiple programs or clearly isolated executable subsystems.

At minimum, plan for:

```text
1. SafeTensors ingestion and metadata catalog
2. CUDA-accelerated tensor conversion pipeline
3. GLB, tensor-tile, and tileset compiler
4. Local query and tensor-block service
5. CesiumJS model viewers
6. Grid-aligned matrix multiplication viewer
7. Chat, selector, WeightQL, and KaTeX interface
```

---

# 3. Reality and Feasibility Constraints

The plan must not make false claims about processing a complete one-trillion-parameter model entirely on one RTX 3090.

An RTX 3090 normally has:

```text
24 GB VRAM
```

Therefore, the architecture must use:

* Streaming
* Memory mapping
* SafeTensors byte ranges
* Bounded CPU buffers
* Bounded pinned-memory buffers
* Bounded GPU staging buffers
* Block-level CUDA execution
* Incremental output writing
* Content-addressed caching
* Resumable jobs
* Backpressure
* Explicit memory budgets

The plan must distinguish:

```text
Metadata-scale support
Visualization-scale support
Selected-block exact access
Selected-block CUDA processing
Full-model offline conversion
Full-model numerical computation
```

These are different capabilities.

The first MVP may demonstrate exact conversion and visualization on a smaller real model or selected tensors, but its architecture and tests must prove that a trillion-parameter manifest can be indexed and traversed without loading the checkpoint into memory.

Do not define “supports one trillion parameters” as:

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

The implementation plan must preserve four distinct data planes.

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

Possible storage:

```text
DuckDB
Arrow
Parquet
```

The plan must choose the smallest suitable MVP combination after repository inspection.

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

Use a tensor-oriented sidecar format such as:

```text
*.qtile
```

Do not treat GLB as the authoritative tensor store.

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

---

# 5. Required Program Boundaries

The plan must define clear executable and library boundaries.

Use the repository evidence to determine the final layout, but evaluate an architecture similar to:

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
│   ├── q-cuda/
│   ├── q-daemon/
│   └── q-cli/
├── gpu/
│   ├── cuda/
│   └── shaders/
├── apps/
│   └── web/
│       ├── model-viewer/
│       ├── quatricmorph-workspace/
│       └── query-interface/
├── architectures/
│   ├── generic/
│   ├── llama/
│   └── qwen/
├── schemas/
│   ├── nsir/
│   ├── qtile/
│   ├── weightql/
│   └── visualization/
├── fixtures/
├── docs/
└── .plan/
```

Do not force this layout blindly.

The planning process must first determine:

* What can be retained from `mm`.
* What must be extracted from `mm`.
* What belongs in the Rust conversion pipeline.
* What belongs in the CUDA runtime.
* What belongs in the browser.
* What belongs in the local daemon.
* Which components need stable shared schemas.
* Which components can initially remain in one executable.

---

# 6. Mandatory Repository Analysis

Before generating implementation tasks:

1. Read the complete repository.
2. Inspect all source files, build files, assets, examples, tests, documentation, and licenses.
3. Identify the current entry points.
4. Run or inspect the current development and build commands.
5. Trace the current matrix visualization lifecycle.
6. Identify reusable components.
7. Identify obsolete research-oriented behavior.
8. Identify architecture assumptions that conflict with trillion-scale tensor visualization.

At minimum, inspect:

```text
index.html
viz.js
gui.js
util.js
assets/
examples/
lib/
package.json
README.md
LICENSE*
```

Locate and document the existing implementations of:

```text
Array2D
Mat
MatMul
matrix initialization
matrix multiplication
Three.js scene creation
matrix placement
value-to-color mapping
value-to-size mapping
row guides
flow guides
text labels
camera initialization
camera fitting
OrbitControls
hover
selection
animation
GUI state
URL serialization
state compression
resource disposal
```

Trace the complete current lifecycle:

```text
Application startup
→ Parameter creation
→ Matrix creation
→ Matrix multiplication
→ Layout calculation
→ Three.js resource creation
→ Camera setup
→ Animation
→ Picking
→ GUI updates
→ URL serialization
→ Reinitialization
→ Disposal
```

For every reusable area, record:

```text
Current file
Current symbol
Current responsibility
Dependencies
Problems
Reuse strategy
Extraction strategy
Planned destination
```

Separate verified repository facts from assumptions and recommendations.

---

# 7. SafeTensors Ingestion

Plan a Rust-based SafeTensors ingestion subsystem.

It must support:

* Local SafeTensors files.
* Sharded SafeTensors checkpoints.
* `model.safetensors.index.json`.
* SafeTensors header parsing.
* Tensor names.
* Shapes.
* Dtypes.
* Byte offsets.
* Shard locations.
* Source revisions and hashes.
* Partial byte-range reads.
* Memory-mapped local reads where practical.
* Resumable metadata import.
* Cancellation.
* Corruption and offset validation.

Define an abstraction conceptually similar to:

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

Plan a tensor descriptor containing at least:

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

The ingestion process must not allocate a buffer proportional to total checkpoint size.

Plan explicit tests for:

* One SafeTensors file.
* Multiple shards.
* Large synthetic shard manifests.
* Missing shards.
* Invalid offsets.
* Unsupported dtype.
* Duplicate names.
* Corrupted headers.
* Cancellation.
* Resume after interruption.
* Stable tensor IDs after reopening.

---

# 8. Neural Structure Intermediate Representation

Plan a canonical semantic representation named conceptually:

```text
NSIR
```

NSIR must normalize architecture-specific tensor names into reusable identities.

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

Plan architecture resolvers for:

```text
generic transformer
Llama-like
Qwen-like
```

Additional model families are out of scope for the first MVP unless already required by repository fixtures.

Resolvers must be allowed to return:

```text
unknown
```

They must never infer semantic meaning solely from matching tensor shapes.

Plan stable canonical addresses such as:

```text
model.layers[10].self_attention.query_projection.weight
```

Canonical addresses must be reusable across:

* Catalog records
* URLs
* WeightQL
* Chat responses
* Cesium feature metadata
* Matrix viewer selections
* Logs
* Cache keys
* Query plans

---

# 9. Metadata Catalog

Plan a local metadata catalog.

Evaluate:

```text
DuckDB
Parquet
Arrow
SQLite
```

Choose based on actual MVP requirements rather than introducing all technologies simultaneously.

At minimum, plan entities for:

```text
models
tensors
tensor_blocks
tensor_statistics
visual_tiles
conversion_jobs
cache_entries
```

The catalog must support:

* Model hierarchy queries.
* Layer and module queries.
* Tensor lookup by canonical address.
* Tensor lookup by alias.
* Shape and dtype filters.
* Semantic-role filters.
* Byte-range resolution.
* Tile-to-tensor resolution.
* Tensor-to-tile resolution.
* Conversion status.
* Cache lookup.
* Exact versus approximate result metadata.

Plan schema versioning and migration from the beginning.

---

# 10. CUDA-Accelerated Conversion on RTX 3090

Plan a CUDA subsystem for NVIDIA GPUs, with the RTX 3090 as the first verified target.

CUDA must be used for block-oriented workloads such as:

* FP16 and BF16 conversion.
* Quantization.
* Min/max reduction.
* Mean and variance.
* L1 and L2 norms.
* Positive, negative, and zero ratios.
* Histogram generation.
* Block sampling.
* Value normalization.
* Visual classification.
* Morton-order encoding where beneficial.
* Optional block matrix multiplication.
* Optional tensor comparison.

Do not use CUDA for responsibilities better handled by:

* SafeTensors header parsing.
* Catalog queries.
* File path handling.
* GLB container writing.
* `tileset.json` generation.
* Cesium tile traversal.
* Browser UI state.

The plan must define:

```text
CPU reader
→ bounded host buffer
→ optional pinned memory
→ bounded GPU staging buffer
→ CUDA kernel
→ compact output buffer
→ qtile and GLB writer
```

Define configurable budgets:

```text
maximum host staging bytes
maximum pinned-memory bytes
maximum GPU staging bytes
maximum concurrent blocks
maximum output queue depth
```

The converter must adapt block size when memory pressure or allocation failure occurs.

Plan:

* CUDA device discovery.
* Compute-capability validation.
* VRAM detection.
* Driver/runtime compatibility checks.
* CPU fallback for supported operations.
* Clear failure when CUDA is required but unavailable.
* Deterministic kernel results within documented numerical tolerance.
* Kernel benchmarks.
* Error propagation.
* Cancellation between blocks.
* Resume from completed block manifests.

Do not require that the entire tensor fit into VRAM.

---

# 11. Tensor Block and LOD Model

Plan a block-addressable tensor representation.

A large matrix must be divided into logical blocks such as:

```text
256 × 256
512 × 512
```

Block size must be configurable and selected based on:

* Dtype.
* Tensor dimensions.
* CUDA memory budget.
* Desired LOD.
* GLB size.
* Cesium traversal behavior.
* Picking granularity.
* Query granularity.

Plan the following LOD hierarchy:

```text
LOD 0 — Model
LOD 1 — Architecture or subsystem
LOD 2 — Layer
LOD 3 — Tensor
LOD 4 — Tensor block
LOD 5 — Sampled or exact scalar region
```

Example data per LOD:

```text
LOD 0:
parameter count
total bytes
global value distribution
subsystem bounds

LOD 1:
layer ranges
aggregate norms
module counts

LOD 2:
tensor counts
layer statistics
anomaly summaries

LOD 3:
tensor shape
dtype
histogram
norms
block layout

LOD 4:
block statistics
quantized samples
exact-value availability

LOD 5:
selected exact values
selected small slices
```

The plan must define:

* Parent-child relationships.
* Stable tile IDs.
* Stable block IDs.
* Geometric error calculation.
* Bounding volumes.
* Content URIs.
* Refinement policy.
* Tile availability.
* Optional implicit tiling.
* Cancellation and resumability.
* Incremental manifest updates.

---

# 12. GLB and Tensor Sidecar Output

The required conversion flow is:

```text
*.safetensors
→ metadata catalog
→ tensor blocks
→ CUDA summaries and visual encoding
→ *.qtile
→ *.glb
→ tileset.json
```

The plan must preserve the distinction between visual and tensor data.

## GLB responsibilities

GLB may contain:

* Shared unit geometry.
* Instance transforms.
* Quantized visual classes.
* Feature IDs.
* Tile-local metadata.
* Tensor or block references.
* Selection-compatible metadata.
* Bounds-related data.

GLB must not contain:

* The complete FP16 or BF16 checkpoint.
* One independent mesh for every parameter.
* Duplicated cube geometry per scalar.
* Complete exact tensor values for the model.
* The authoritative metadata catalog.
* Reproducible analysis results that belong in tensor-native storage.

Evaluate:

```text
EXT_mesh_gpu_instancing
EXT_mesh_features
EXT_structural_metadata
```

Do not assume CesiumJS supports every relevant glTF extension perfectly. Plan capability tests and fallback paths.

## Tensor sidecar responsibilities

Plan a versioned format such as:

```text
tile_12_4_7.qtile
```

The header may contain:

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

The payload may contain:

```text
Morton coordinates
quantized values
flags
local IDs
sample metadata
optional exact-value references
```

The planning process must define:

* Binary schema.
* Endianness.
* Versioning.
* Alignment.
* Compression.
* Checksums.
* Corruption handling.
* Streaming reads.
* Browser-decoding strategy.
* Rust-decoding strategy.
* Forward compatibility.

---

# 13. Spatial Model Layout in CesiumJS

The CesiumJS viewer must represent the model hierarchy as a navigable spatial structure.

Plan deterministic spatial placement for:

```text
Model
Subsystem
Layer
Module
Tensor
Tensor block
```

The layout must not use arbitrary scattered offsets.

Define a spatial layout system using:

```text
model origin
layer spacing
module spacing
tensor padding
block cell size
major grid interval
minor grid interval
label margin
frame padding
depth spacing
```

All model objects must derive their positions from logical addresses and layout rules.

Examples:

```text
Layer index → primary model axis
Module role → secondary grouping axis
Tensor index → local tensor grid
Tensor block coordinates → local block coordinates
Scalar coordinates → procedural cell coordinates
```

The viewer must support:

* Model-level overview.
* Layer navigation.
* Tensor selection.
* Tensor-block selection.
* Cesium camera-based LOD.
* Progressive loading.
* Picking.
* Highlighting.
* Breadcrumb navigation.
* Search by canonical address.
* Search by alias.
* Selected-object metadata.
* Fit selected object.
* Reset model view.
* Tile debugging behind a development flag.

Zooming out must not trigger exact tensor reads.

Zooming in must not automatically retrieve complete tensors.

Exact values are loaded only after explicit selection or query.

---

# 14. CesiumJS Viewer Program

Plan a dedicated CesiumJS application or workspace.

Responsibilities:

```text
Load tileset.json
Traverse model LOD
Render GLB tile content
Pick models, layers, tensors, and blocks
Resolve feature IDs
Display metadata
Navigate hierarchy
Open tensor selections
Coordinate with query service
Coordinate with matrix workspace
Persist view state
```

The default layout should include:

```text
Header
Hierarchy or search panel
Central Cesium viewport
Inspector panel
Center-bottom chat and query box
Optional matrix workspace
Status and exactness indicators
```

The viewer must display whether visible data is:

```text
Metadata only
Aggregate
Approximate
Sampled
Quantized
Exact
```

Do not visually imply that a sampled tile contains all exact values.

Plan loading and error states for:

* Missing tiles.
* Corrupted GLB.
* Missing qtile.
* Incompatible tileset version.
* Local daemon unavailable.
* Query cancellation.
* CUDA conversion incomplete.
* Partially generated models.
* Cache miss.
* Invalid feature metadata.

---

# 15. Shared 3D Grid Ruler

Plan a reusable spatial system named conceptually:

```text
GridRuler3D
```

This grid is not decorative.

It is the coordinate and alignment system shared by:

* Tensor planes.
* Matrix cells.
* Vector cells.
* Scalar cells.
* Tensor frames.
* Matrix multiplication guides.
* Axis labels.
* Slice selections.
* Result cells.
* Intermediate expression nodes.
* Camera-fit bounds.

It must define:

```text
cellSize
minorGridSpacing
majorGridInterval
tensorPadding
labelMargin
framePadding
operandGap
axisMargin
depthSpacing
origin
```

Required grid invariant:

```text
position.x % cellSize ≈ 0
position.y % cellSize ≈ 0
position.z % cellSize ≈ 0
```

Use a documented floating-point tolerance.

Every tensor block opened from the Cesium viewer must map into this workspace through its logical tensor coordinates.

Do not store independent absolute positions for every scalar.

Position must be derived from:

```text
workspace origin
+ tensor anchor
+ logical tensor index
+ block origin
+ grid cell size
```

---

# 16. Matrix Multiplication Workspace

The existing `mm` implementation may be refactored into a dedicated matrix workspace.

The first MVP should support a selected real tensor block or manually entered matrix expression:

```text
A @ B = C
```

Required shape combinations:

```text
Matrix @ Matrix → Matrix
Matrix @ Column Vector → Column Vector
Row Vector @ Matrix → Row Vector
Row Vector @ Column Vector → Scalar
```

Use:

```text
A ∈ R^(m×k)
B ∈ R^(k×n)
C = A @ B
C ∈ R^(m×n)
```

And:

```text
C[i,j] = Σ A[i,k] × B[k,j]
```

Recommended axes:

```text
World X → J
World Y → I
World Z → K
```

Tensor planes:

```text
A → I × K
B → K × J
C → I × J
```

Shared dimensions must align:

* `A.I` with `C.I`.
* `A.K` with `B.K`.
* `B.J` with `C.J`.

The workspace must support:

```text
Concept mode
Real tensor-block mode
```

### Concept mode

Uses generated or sampled values to explain the operation.

### Real tensor-block mode

Uses explicitly selected tensor blocks from the checkpoint.

The MVP must not automatically multiply an entire large tensor merely to produce an animation.

It should operate on a selected region such as:

```text
A[0:256, 0:256] @ B[0:256, 0:256]
```

The plan must define maximum interactive dimensions and block-size limits for the RTX 3090 and browser.

---

# 17. Tensor Frames and Cell Rendering

Plan a reusable component named conceptually:

```text
TensorGridFrame
```

Every matrix, vector, scalar, or selected tensor block must have:

* Outer boundary.
* Inner margin.
* Title margin.
* Shape label.
* Canonical tensor address.
* Row and column guides.
* Axis labels.
* Deterministic anchor.
* Deterministic orientation.
* Grid-aligned cells.
* Camera-fit bounds.

Examples:

```text
Q[10] [256 × 256]
Kᵀ[10] [256 × 256]
Result [256 × 256]
```

The renderer may reuse existing Three.js point or sprite behavior for the MVP if it satisfies alignment and picking requirements.

Optional instanced voxels may be planned only when they provide measurable value.

For each value:

* Position at the center of its logical grid cell.
* Preserve the cell for zero values.
* Distinguish negative, zero, and positive values.
* Represent magnitude through scale, height, opacity, or another documented channel.
* Prevent markers from crossing normal cell boundaries.
* Show numerical labels only at suitable zoom levels or for selected regions.
* Avoid creating a DOM label for every weight.

---

# 18. Matrix Multiplication Interaction

For selected result cell:

```text
C[i,j]
```

Plan this deterministic sequence:

```text
Highlight row A[i, :]
→ Highlight column B[:, j]
→ Highlight shared K positions
→ Show A[i,k] × B[k,j]
→ Update running sum
→ Reveal C[i,j]
→ Advance
```

Required controls:

```text
Play
Pause
Step
Previous Step
Reset Calculation
Reset View
Fit View
```

Plan separate state domains:

```text
Tensor data
Expression data
Layout state
Selection state
Animation state
Camera state
Display state
Query state
Serialized state
```

Hover information must include:

```text
Tensor canonical address
Alias
Logical index
Block index
Value
Shape
Dtype
Exactness
Source shard
```

Selection must not rely only on color.

Use one or more of:

* Scale.
* Outline.
* Brightness.
* Guide thickness.
* Opacity.
* Frame emphasis.
* Animated path.

---

# 19. WeightQL and Selector Syntax

Plan a standardized query language named:

```text
WeightQL
```

WeightQL must resolve:

* Models.
* Layers.
* Modules.
* Tensors.
* Tensor slices.
* Scalar values.
* Statistical queries.
* Matrix expressions.
* Comparisons.

Canonical selector examples:

```text
model.layers[10].self_attention.query_projection.weight
model.layers[10].self_attention.query_projection.weight[100,42]
```

Convenience aliases:

```text
Q[10]
Q[10][100,42]
K[10][0:256,0:256]
MLP.down[24][:]
Expert[12,37].up[0:128,:]
```

The interface may also accept contextual selectors such as:

```text
layer[0][10].attention[1].Q[0]
```

However, contextual selectors must resolve into a canonical unambiguous tensor address before execution.

When an alias is ambiguous, return candidates instead of silently choosing.

Example ambiguity:

```text
Att[10]
```

Possible matches:

```text
Q
K
V
O
attention-related metadata
```

Plan parsing stages:

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

---

# 20. Mathematical Expressions

WeightQL must support a constrained MVP expression subset:

```text
tensor reference
slice
transpose
matrix multiplication
basic statistics
comparison
```

Example:

```text
A = tensor("Q[10][0:256,0:256]")
B = transpose(tensor("K[10][0:256,0:256]"))

show A @ B
```

The parser must build an AST.

Example:

```text
MatMul
├── TensorRef(A)
└── Transpose
    └── TensorRef(B)
```

The planner must:

1. Resolve all tensor references.
2. Resolve byte ranges.
3. Validate shapes.
4. Determine whether data is exact, sampled, or approximate.
5. Estimate bytes read.
6. Estimate host memory.
7. Estimate GPU memory.
8. Select CPU or CUDA.
9. Build visualization instructions.
10. Require explicit execution for expensive operations.

Shape mismatch must fail before any CUDA kernel is launched.

The MVP must not allow arbitrary code execution.

Do not use:

```text
eval
Function constructor
shell interpolation
unrestricted SQL
unrestricted Python execution
```

---

# 21. Center Chat and Mathematical Query Box

Plan a center-bottom or centrally anchored chat and query interface.

The input must support:

* Natural-language requests.
* WeightQL.
* Tensor selectors.
* Mathematical expressions.
* KaTeX-rendered formulas.
* Query history.
* Suggested selectors.
* Current-selection context.
* Candidate resolution.
* Cost preview.
* Cancellation.

Examples:

```text
Show Q[10].
```

```text
Show layer[10].attention.Q.
```

```text
Open model.layers[10].self_attention.query_projection.weight.
```

```text
Show Q[10][100, :].
```

```text
Compare Q[10][100, :] with Q[20][100, :].
```

```text
Visualize Q[10][0:128, :] @ K[10][:, 0:128].
```

```text
Show the L2 norm of every query projection.
```

Chat must not read SafeTensors bytes directly.

Chat must produce or invoke a validated WeightQL plan.

The result UI must clearly label:

```text
Exact result
Approximate result
Sampled result
Quantized visualization
Statistical interpretation
```

KaTeX should display expressions such as:

```text
QK^\top
```

```text
C_{ij} = \sum_k A_{ik}B_{kj}
```

```text
\lVert W \rVert_2
```

The plan must define safe rendering and sanitization for user-provided mathematical text.

---

# 22. Local Daemon and API

Plan a local service that connects the browser applications to the catalog, source files, cache, and CUDA runtime.

Possible responsibilities:

```text
Open model
Import metadata
Inspect jobs
Read exact values
Read tensor slices
Serve tileset.json
Serve GLB
Serve qtile
Execute WeightQL plans
Run CUDA jobs
Report progress
Cancel work
Resume work
Inspect cache
```

Plan API groups such as:

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

Do not treat these exact routes as mandatory before inspecting the repository and selecting the MVP transport.

Evaluate:

* HTTP.
* WebSocket or Server-Sent Events for progress.
* Direct local invocation.
* Static-file serving for generated tiles.

The plan must define safe local-file access boundaries.

---

# 23. Conversion Job System

Full-model conversion may run for a long time.

Plan a resumable job system containing:

```text
job ID
source model ID
conversion version
configuration hash
current phase
current tensor
current block
completed blocks
failed blocks
bytes read
bytes written
GPU time
CPU time
cache hits
errors
started time
updated time
```

Required states:

```text
Pending
Inspecting
Indexing
Converting
Writing
Validating
Paused
Cancelled
Failed
Complete
```

Plan checkpointing after bounded units of work.

A process crash must not require restarting completed tensor blocks.

Generated outputs must use temporary files and atomic rename where practical.

A partially written GLB or qtile must never be published as valid.

---

# 24. Cache Architecture

Plan local cache levels:

```text
L0 — GPU-resident active blocks
L1 — Process-memory decoded blocks
L2 — Local NVMe content-addressed artifacts
L3 — Browser Cache Storage or IndexedDB
L4 — Future remote object storage
```

Only L0–L3 are required for the local MVP.

Cache keys should include:

```text
source model hash
tensor ID
logical slice
LOD
statistics algorithm
algorithm version
quantization encoding
visualization encoding
```

Do not include purely visual palette choices when the browser shader can apply them dynamically.

Plan:

* Size limits.
* Eviction.
* Corruption detection.
* Cache versioning.
* Cache reuse after reopening.
* Cache inspection.
* Cache clearing.
* Concurrent access.
* Partial-entry cleanup.

---

# 25. MVP User Interface

Plan a unified Quatricmorph interface.

## Header

Display:

```text
Quatricmorph
Trillion-Scale Tensor Visualization
```

Do not present the application as `mm`.

Retain required original-project attribution in repository documentation and license files.

## Model source controls

Provide:

```text
Open local model
Open SafeTensors file
Open sharded checkpoint directory
Recent models
Import metadata
Generate visualization
Resume conversion
Cancel conversion
```

## Navigation

Provide:

```text
Model hierarchy
Layer selector
Module selector
Tensor selector
Canonical-address search
Alias search
Breadcrumbs
```

## Cesium controls

Provide:

```text
Fit model
Fit selection
Reset view
Show hierarchy frames
Show major grid
Show minor grid
Show labels
Show tile bounds
LOD status
Exactness status
```

Development-only tile debugging must remain hidden by default.

## Matrix workspace controls

Provide:

```text
Open selected tensor
Select slice
Assign to A
Assign to B
Transpose
Validate shapes
Visualize A @ B
Play
Pause
Step
Previous
Reset calculation
Fit workspace
```

## Query interface

Provide:

```text
Chat input
WeightQL input
KaTeX preview
Candidate selector
Cost estimate
Execute
Cancel
History
```

---

# 26. Explicitly Out of Scope

Do not plan the following as first-MVP requirements:

* Training visualization.
* Automatic differentiation.
* Gradient visualization.
* Full inference runtime.
* Token-conditioned hidden states.
* Runtime attention probabilities.
* Complete Q/K/V activation capture.
* LoRA editing.
* Model morphing.
* Distributed cluster execution.
* Multi-user collaboration.
* User accounts.
* Remote SaaS control plane.
* Notebook integration.
* Full Hugging Face Hub browsing.
* Arbitrary Python execution.
* Full trillion-parameter numerical execution on one RTX 3090.
* Native Metal renderer.
* Native Vulkan renderer.
* Tauri desktop packaging.
* Custom WebGPU renderer replacing CesiumJS.
* Multi-GPU scheduling.
* Full-model spectral decomposition.
* Automatic semantic interpretation of visible weight patterns.

The architecture may preserve extension points for these capabilities, but tasks must not implement them during the MVP.

---

# 27. Required Planning Directory

Create:

```text
.plan/
├── README.md
├── MASTER_PLAN.md
├── PRODUCT_SCOPE.md
├── REPOSITORY_ANALYSIS.md
├── CURRENT_ARCHITECTURE.md
├── TARGET_ARCHITECTURE.md
├── DATA_ARCHITECTURE.md
├── CUDA_ARCHITECTURE.md
├── TILING_ARCHITECTURE.md
├── CESIUM_VIEWER_ARCHITECTURE.md
├── MATRIX_WORKSPACE_ARCHITECTURE.md
├── WEIGHTQL_ARCHITECTURE.md
├── QUERY_UI_ARCHITECTURE.md
├── API_CONTRACTS.md
├── SCHEMA_PLAN.md
├── REQUIREMENT_TRACEABILITY.md
├── DEPENDENCY_GRAPH.md
├── EXECUTION_ORDER.md
├── TEST_STRATEGY.md
├── PERFORMANCE_PLAN.md
├── MEMORY_BUDGET.md
├── MIGRATION_STRATEGY.md
├── RISK_REGISTER.md
├── SECURITY_MODEL.md
├── DEFINITION_OF_DONE.md
├── decisions/
│   ├── README.md
│   └── ADR-CANDIDATE-*.md
├── phases/
│   ├── phase-00-repository-baseline/
│   ├── phase-01-safetensors-ingestion/
│   ├── phase-02-catalog-and-nsir/
│   ├── phase-03-cuda-block-runtime/
│   ├── phase-04-tensor-tiles-and-glb/
│   ├── phase-05-cesium-model-viewer/
│   ├── phase-06-grid-quatricmorph-workspace/
│   ├── phase-07-weightql-and-chat/
│   ├── phase-08-integration-and-performance/
│   └── phase-09-documentation-and-release/
└── tasks/
    ├── QM-0001-*/
    │   └── TASK.md
    ├── QM-0002-*/
    │   └── TASK.md
    └── ...
```

You may refine phase boundaries based on repository evidence.

Do not create empty documents.

---

# 28. Required Root Documents

## `.plan/README.md`

Document:

* Purpose of `.plan`.
* Authoritative documents.
* Task numbering.
* Status vocabulary.
* Dependency conventions.
* How an autonomous agent selects the next task.
* How verification evidence is recorded.
* How plans are updated when repository facts change.

Statuses:

```text
Undefined
Ready
In Progress
Blocked
Implemented
Verified
Complete
Superseded
```

A task is not `Complete` until it is implemented and verified.

## `.plan/MASTER_PLAN.md`

Include:

* Current repository summary.
* Target MVP.
* Program boundaries.
* Phase summary.
* Critical path.
* Parallel workstreams.
* Integration checkpoints.
* Release criteria.
* Explicit non-goals.
* Trillion-scale definition.
* RTX 3090 constraints.

## `.plan/PRODUCT_SCOPE.md`

Clearly distinguish:

```text
MVP capability
Architectural extension point
Future capability
Explicit non-goal
```

Prevent the planning system from silently expanding the MVP.

## `.plan/DATA_ARCHITECTURE.md`

Document:

* Four data planes.
* SafeTensors source model.
* NSIR.
* Catalog.
* Tensor blocks.
* qtile.
* GLB.
* tileset.
* IDs.
* Cache keys.
* Exactness metadata.
* Versioning.

## `.plan/CUDA_ARCHITECTURE.md`

Document:

* Supported GPU.
* Kernel responsibilities.
* CPU/GPU data flow.
* Memory budgets.
* Block scheduling.
* Fallback behavior.
* Cancellation.
* Determinism.
* Testing.
* Benchmarking.
* Error handling.

## `.plan/TILING_ARCHITECTURE.md`

Document:

* LOD hierarchy.
* Tile hierarchy.
* Block dimensions.
* Geometric errors.
* Bounding volumes.
* GLB structure.
* qtile structure.
* Metadata references.
* Cesium compatibility.
* Incremental generation.
* Validation.

## `.plan/CESIUM_VIEWER_ARCHITECTURE.md`

Document:

* Viewer components.
* Tileset lifecycle.
* LOD.
* Picking.
* Metadata resolution.
* Hierarchy navigation.
* Selection state.
* Query integration.
* Exactness indicators.
* Error states.
* Camera behavior.
* Resource disposal.

## `.plan/MATRIX_WORKSPACE_ARCHITECTURE.md`

Document:

* Existing `mm` components to reuse.
* GridRuler3D.
* TensorGridFrame.
* I/J/K coordinate mapping.
* Matrix rendering.
* Real tensor-block loading.
* Selection.
* Multiplication animation.
* Camera fitting.
* State separation.
* Cleanup.

## `.plan/WEIGHTQL_ARCHITECTURE.md`

Document:

* Grammar.
* AST.
* Canonical addresses.
* Aliases.
* Ambiguity handling.
* Shape system.
* Cost planning.
* Execution tiers.
* Exactness.
* Security boundaries.
* Query result schema.

## `.plan/QUERY_UI_ARCHITECTURE.md`

Document:

* Chat responsibilities.
* Query responsibilities.
* KaTeX rendering.
* Candidate selection.
* Cost confirmation.
* Query progress.
* Cancellation.
* Current selection context.
* Result rendering.
* Exact versus approximate labels.

## `.plan/MEMORY_BUDGET.md`

Define budgets for:

```text
Source read buffers
CPU decoded blocks
Pinned host memory
GPU input buffers
GPU output buffers
GLB writer buffers
qtile writer buffers
Cesium tile memory
Browser query results
Matrix workspace
```

Include formulas and configuration variables rather than unsupported fixed promises.

## `.plan/SECURITY_MODEL.md`

Cover:

* Local file access.
* Path traversal.
* Query parsing.
* Resource limits.
* Denial-of-service protection.
* Malformed SafeTensors.
* Malformed GLB.
* Malformed qtile.
* Browser content sanitization.
* KaTeX sanitization.
* Local daemon origin policy.
* No arbitrary code execution.

---

# 29. Recommended Phases

## Phase 00 — Repository Baseline

Goal:

```text
Understand current mm architecture
→ establish build baseline
→ identify reusable matrix visualization behavior
→ protect license and attribution
```

Plan tasks for:

* Repository inventory.
* Build verification.
* Runtime verification.
* Current architecture diagrams.
* Matrix behavior characterization.
* Resource-disposal characterization.
* URL-state characterization.
* Reuse and deprecation map.

## Phase 01 — SafeTensors Ingestion

Goal:

```text
Open one SafeTensors file
→ parse metadata
→ read one exact tensor slice
```

Plan tasks for:

* Rust workspace or integration strategy.
* SafeTensors parser.
* Local source abstraction.
* Sharded index parser.
* Byte-range reader.
* Stable model and tensor IDs.
* Validation.
* Cancellation.
* Resume metadata.

## Phase 02 — Catalog and NSIR

Goal:

```text
Raw tensor names
→ canonical model hierarchy
→ queryable local catalog
```

Plan tasks for:

* Catalog schema.
* Migrations.
* Generic resolver.
* Qwen or Llama resolver.
* Canonical addresses.
* Alias index.
* Hierarchy query.
* Unknown-role behavior.
* Synthetic trillion-scale manifest test.

## Phase 03 — CUDA Block Runtime

Goal:

```text
Selected tensor block
→ bounded CUDA processing
→ statistics and quantized visual records
```

Plan tasks for:

* CUDA build.
* Device detection.
* Memory scheduler.
* Dtype decoding.
* Reduction kernels.
* Quantization kernels.
* Histogram kernels.
* CPU reference.
* Determinism tests.
* RTX 3090 verification.
* Cancellation.
* CPU fallback.

## Phase 04 — Tensor Tiles, GLB, and Tileset

Goal:

```text
Tensor metadata and block summaries
→ qtile
→ GLB
→ tileset.json
```

Plan tasks for:

* LOD rules.
* Block layout.
* Bounding volumes.
* Geometric error.
* qtile schema.
* qtile encoder and decoder.
* Shared cube geometry.
* GPU instancing.
* Feature IDs.
* GLB validation.
* Tileset generation.
* Atomic output.
* Resume manifests.

## Phase 05 — Cesium Model Viewer

Goal:

```text
Open tileset.json
→ navigate model to tensor block
→ pick a block
→ resolve its canonical tensor address
```

Plan tasks for:

* Viewer shell.
* Cesium initialization.
* Local tileset loading.
* Hierarchy navigation.
* Feature picking.
* Inspector.
* Exactness badges.
* Search.
* Camera fitting.
* Tile error handling.
* Resource cleanup.
* URL state.

## Phase 06 — Grid Matrix Workspace

Goal:

```text
Selected tensor blocks
→ assign A and B
→ validate shapes
→ visualize A @ B
```

Plan tasks for:

* Extract pure math from `mm`.
* Separate animation state.
* GridRuler3D.
* TensorGridFrame.
* Matrix placement.
* Vector and scalar handling.
* Tensor-block adapter.
* Row and column selection.
* Multiplication guides.
* Running sum.
* Play, pause, step, previous, reset.
* Camera fitting.
* Disposal.

## Phase 07 — WeightQL and Chat

Goal:

```text
Selector or natural-language request
→ validated query plan
→ viewer or matrix action
```

Plan tasks for:

* Selector grammar.
* Canonical resolver.
* Alias resolver.
* Candidate response.
* Slice syntax.
* Matrix-expression AST.
* Shape checking.
* Cost estimator.
* Query-plan schema.
* Query executor.
* Chat integration.
* KaTeX preview.
* Current-selection context.
* Cancellation.
* Exactness labels.

## Phase 08 — Integration and Performance

Goal:

```text
SafeTensors
→ CUDA conversion
→ Cesium selection
→ exact query
→ matrix visualization
```

Plan tasks for:

* End-to-end fixture.
* Cache reuse.
* Resume.
* Failure injection.
* Browser memory.
* CUDA memory.
* Repeated scene changes.
* Large manifest scaling.
* GLB and tileset validation.
* Exact scalar comparison.
* Query cancellation.
* Runtime error audit.

## Phase 09 — Documentation and Release

Goal:

```text
Reproducible local MVP
```

Plan tasks for:

* README.
* Architecture documentation.
* CUDA requirements.
* Supported dtypes.
* Conversion commands.
* Viewer commands.
* Query examples.
* Matrix examples.
* Current limitations.
* Attribution.
* License.
* Demo assets.
* Acceptance audit.

---

# 30. Task Format

Create one folder per task:

```text
.plan/tasks/QM-XXXX-short-name/
```

Each folder must contain:

```text
TASK.md
```

Use:

```markdown
# QM-XXXX — Task title

## Status

Ready

## Phase

Phase identifier and name.

## Objective

One precise outcome.

## Repository Evidence

List actual files, symbols, dependencies, and observed behavior that justify
the task.

## Requirements Covered

List stable requirement IDs.

## Dependencies

List prerequisite task IDs.

## Blocks

List dependent task IDs.

## Parallelization

Explain whether the task can run concurrently and identify shared-file risks.

## Program Boundary

Identify the executable, crate, application, or shared schema affected.

## Scope

List included work.

## Out of Scope

List excluded work.

## Files Expected to Change

List existing paths.

## Files Expected to Add

List planned paths.

## Files Expected to Remove or Deprecate

List only when justified.

## Data Contracts

Describe schemas, IDs, files, API messages, or binary formats involved.

## Memory and Performance Constraints

Describe bounded-memory requirements and expected complexity.

## Implementation Plan

Provide ordered repository-specific steps.

## Error Handling

Describe failures, cancellation, partial output, recovery, and safe fallback.

## Acceptance Criteria

Use objective and testable statements.

## Verification Plan

List automated and manual verification.

## Suggested Commands

Separate verified current commands from commands introduced by planned tasks.

## Test Cases

Provide concrete inputs and expected results.

## Risks

List task-specific risks and mitigations.

## Completion Evidence

Define logs, screenshots, generated files, benchmark output, test output, or
other evidence required before completion.
```

---

# 31. Task Sizing Rules

Each task should normally correspond to one focused branch and one pull request.

Split tasks when they combine independent responsibilities such as:

```text
SafeTensors parsing + Cesium UI
CUDA kernels + GLB generation
WeightQL parsing + chat design
Matrix rendering + tileset generation
Catalog migration + GPU execution
```

Prefer tasks such as:

```text
Parse SafeTensors shard index
Implement range-readable tensor source
Define canonical tensor ID
Add generic transformer resolver
Add CUDA min/max reduction kernel
Add CPU reference statistics
Define qtile v1 schema
Encode one tensor block as qtile
Generate one instanced GLB tile
Generate tensor-level tileset hierarchy
Resolve Cesium feature ID to tensor block
Extract MatMul math from viz.js
Implement GridRuler3D
Implement tensor-block-to-matrix adapter
Parse Q[10][0:256,0:256]
Render KaTeX expression preview
Estimate query I/O and GPU memory
```

Do not create giant tasks titled:

```text
Implement backend
Implement CUDA
Build viewer
Add chat
```

---

# 32. Required Test Strategy

Plan automated and manual testing for:

## SafeTensors

* Header parsing.
* Shards.
* Offsets.
* Dtypes.
* Corruption.
* Stable IDs.
* Exact scalar lookup.
* Exact slice lookup.
* No full-checkpoint allocation.

## Trillion-scale metadata

* Synthetic manifest representing approximately one trillion parameters.
* Bounded memory during indexing.
* Layer and tensor navigation.
* Stable catalog queries.
* No requirement to open all tensor payloads.

## CUDA

* CPU reference comparisons.
* FP16.
* BF16 where supported.
* FP32.
* Min/max.
* Mean.
* Variance.
* Norms.
* Histograms.
* Quantization.
* Multiple block dimensions.
* Out-of-memory adaptation.
* Cancellation.
* RTX 3090 execution.

## Tile generation

* qtile round trip.
* GLB validation.
* tileset schema validation.
* Stable feature IDs.
* Bounds.
* Geometric errors.
* Resume.
* Atomic output.
* Cache reuse.

## Cesium viewer

* Tileset opening.
* LOD behavior.
* Picking.
* Metadata lookup.
* Missing tile.
* Corrupted tile.
* Camera fit.
* Selection persistence.
* Browser memory.
* Disposal.

## Matrix workspace

Verify:

```text
2×3 @ 3×2 → 2×2
3×3 @ 3×1 → 3×1
1×3 @ 3×2 → 1×2
1×3 @ 3×1 → 1×1
1×1 @ 1×1 → 1×1
```

Invalid:

```text
2×3 @ 2×2 → validation error
```

Also test:

* Negative values.
* Zeros.
* Decimals.
* Selected real blocks.
* Grid alignment.
* Vectors.
* Scalars.
* Hover metadata.
* Selection.
* Deterministic stepping.
* Reset.
* Camera fit.
* Reinitialization.
* Resource disposal.

## WeightQL

* Canonical address.
* Alias.
* Ambiguity.
* Slice.
* Transpose.
* Matrix multiplication.
* Shape mismatch.
* Cost estimation.
* Query cancellation.
* Exactness metadata.
* Invalid syntax.
* Resource-limit rejection.

## End-to-end

Required demonstration:

```text
Open SafeTensors fixture
→ import metadata
→ convert selected tensor hierarchy
→ generate qtile, GLB, and tileset.json
→ open in CesiumJS
→ select a tensor block
→ retrieve one exact value
→ verify against Python SafeTensors
→ assign blocks to matrix workspace
→ visualize A @ B
→ query the selection through chat
```

---

# 33. MVP Acceptance Criteria

The implementation plan must map every criterion to implementation and verification tasks.

The MVP is complete only when:

1. The application is branded as Quatricmorph.
2. A local SafeTensors file can be opened.
3. A sharded SafeTensors checkpoint can be indexed.
4. Indexing does not load the complete checkpoint into RAM.
5. A synthetic trillion-parameter manifest can be indexed using bounded memory.
6. Model, layer, module, tensor, and block metadata can be browsed.
7. Tensor names are mapped to stable canonical addresses.
8. Unknown semantic roles remain unknown rather than being guessed.
9. Selected tensor blocks can be read by byte range.
10. CUDA processing runs on an NVIDIA RTX 3090.
11. CUDA processing uses bounded block buffers.
12. CUDA results are validated against CPU references.
13. Conversion produces versioned qtile artifacts.
14. Conversion produces valid GLB tile content.
15. Conversion produces a valid `tileset.json`.
16. Generated work can be cancelled and resumed.
17. Completed block artifacts are reused from cache.
18. CesiumJS loads the generated tileset.
19. CesiumJS performs camera-based LOD loading.
20. Zooming out does not load exact scalar data.
21. Selecting a visual feature resolves to the correct tensor or block.
22. Clicking or querying a scalar returns the correct exact value.
23. The exact value matches a Python SafeTensors reference.
24. The UI distinguishes aggregate, sampled, quantized, approximate, and exact information.
25. A selected tensor block can be opened in the matrix workspace.
26. Tensors align to the shared 3D grid ruler.
27. Matrix, row vector, column vector, and scalar layouts use one coordinate system.
28. Compatible matrix blocks can be multiplied.
29. Incompatible shapes are rejected before CUDA execution.
30. The multiplication path can be animated deterministically.
31. Play, pause, step, previous, and reset work.
32. A user can query a canonical tensor address.
33. A user can query aliases such as `Q[10]`.
34. Ambiguous aliases return candidate tensors.
35. A user can submit a slice query.
36. A user can submit a constrained matrix expression.
37. Mathematical expressions render with KaTeX.
38. Query cost is estimated before expensive execution.
39. Queries can be cancelled.
40. Chat uses WeightQL and cannot directly access arbitrary checkpoint bytes.
41. Repeated selection and reinitialization do not create obvious browser memory leaks.
42. Repeated CUDA block jobs do not continuously leak device memory.
43. The browser console contains no unresolved runtime errors.
44. The original license and attribution are preserved.
45. Documentation accurately describes implemented capabilities and limitations.
46. The product does not claim that one RTX 3090 can hold or fully compute a one-trillion-parameter model.

---

# 34. Required Architecture Decisions

Create ADR candidates for decisions including:

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

Each ADR candidate must include:

```text
Context
Repository evidence
Decision required
Options
Advantages
Disadvantages
Risks
Recommended default
Tasks affected
Decision deadline
```

Do not mark recommendations as approved decisions unless repository evidence makes alternatives nonviable.

---

# 35. Requirement Traceability

Create stable requirement IDs, for example:

```text
SRC-001
NSIR-001
CAT-001
CUDA-001
TILE-001
GLB-001
CESIUM-001
GRID-001
MATMUL-001
WQL-001
CHAT-001
CACHE-001
PERF-001
SEC-001
DOC-001
AC-001
```

Every requirement must map to:

```text
Implementation task IDs
Verification task IDs
Documentation task IDs where applicable
```

No acceptance criterion may remain unmapped.

---

# 36. Dependency and Execution Planning

Create:

```text
DEPENDENCY_GRAPH.md
EXECUTION_ORDER.md
```

Identify:

* Critical path.
* Parallelizable Rust crates.
* Parallelizable frontend work.
* Shared schema blockers.
* CUDA toolchain blockers.
* Large merge-conflict files.
* Integration gates.
* Fixture dependencies.
* Hardware-dependent verification.
* Tasks that can run without an RTX 3090.
* Tasks that require an RTX 3090.

A likely critical path is:

```text
Repository baseline
→ source abstraction
→ SafeTensors metadata
→ canonical IDs
→ catalog
→ block reader
→ CUDA statistics
→ qtile schema
→ GLB tile
→ tileset
→ Cesium selection
→ exact block query
→ matrix adapter
→ WeightQL integration
→ end-to-end verification
```

Do not force concurrency when tasks modify the same core files or shared schema.

---

# 37. Plan Quality Audit

Before finishing, audit `.plan/`.

Verify:

1. Every document is repository-grounded.
2. Every task references actual repository evidence.
3. Every acceptance criterion maps to tasks.
4. Every task has dependencies.
5. Every task has objective verification.
6. RTX 3090 memory limitations are explicit.
7. Trillion-scale support is defined accurately.
8. GLB is not treated as the tensor database.
9. qtile or an equivalent tensor-native sidecar is planned.
10. No plan creates one cube per parameter.
11. No plan sends complete tensors to the browser unnecessarily.
12. SafeTensors access is lazy and block-oriented.
13. CUDA jobs are bounded, cancellable, and resumable.
14. Model hierarchy and tensor IDs are stable.
15. Cesium is used for LOD traversal and visualization, not tensor compute.
16. Matrix visualization uses the shared 3D grid ruler.
17. Chat invokes WeightQL rather than reading files directly.
18. Ambiguous selectors return candidates.
19. Exact, approximate, sampled, and quantized data are distinguished.
20. Testing is distributed across phases rather than deferred to the end.
21. Hardware-specific tests are identified.
22. CPU-reference tests are included.
23. Security and resource limits are included.
24. Original licensing is preserved.
25. No files outside `.plan/` were modified.

Correct all gaps before completing the task.

---

# 38. Final Response

After generating `.plan/`, provide:

```text
1. Summary of the existing repository architecture
2. Summary of the planned Quatricmorph architecture
3. Planning documents created
4. Number of phases
5. Number of implementation tasks
6. Number of verification tasks
7. Critical path
8. Parallelizable workstreams
9. Tasks requiring an RTX 3090
10. Tasks executable without CUDA hardware
11. CUDA and memory risks
12. GLB and Cesium compatibility risks
13. WeightQL and query risks
14. ADR candidates
15. Confirmed current repository commands
16. Proposed future commands
17. Remaining repository uncertainties
18. Confirmation that no files outside `.plan/` were modified
```

Do not implement production code.

Do not report planned behavior as implemented.

Do not claim that generating `.plan/` completes the Quatricmorph MVP.

Do not claim that an RTX 3090 can load an entire one-trillion-parameter checkpoint into VRAM.

The final `.plan/` must be detailed enough for an autonomous multi-agent implementation system to execute the Quatricmorph MVP task by task.
