# Quatricmorph — Tensor Visualization Implementation Architecture

## 1. Implementation Goals

Quatricmorph will open an open-weight model from Hugging Face or local disk and turn it into a computational space that can:

* be browsed from model down to layer, module, tensor, block, and scalar;
* visualize extremely large tensors using a level-of-detail mechanism;
* query a specific tensor or weight;
* evaluate mathematical expressions such as `(A @ B) @ C`;
* visually simulate the matrix multiplication process;
* perform statistical analysis without loading the entire checkpoint into RAM;
* use CPU, WebGPU, Metal, CUDA, or a distributed system depending on the workload;
* cache results so sessions can be reopened and shared on the web.

This architecture directly extends Quatricmorph's current definition: an immutable checkpoint source, tensors normalized into NSIR objects, large data read lazily, and every interface sharing the same query layer.

---

# 2. Architectural Principles

## 2.1 Distinguishing Four Data Types

Quatricmorph should not simply split data into `Schema` and `Data`. It should be split into four planes:

```text
1. Artifact Plane
   Original SafeTensors, tokenizer, config, shard indexes

2. Metadata Plane
   Model, layer, tensor, shape, dtype, byte range, semantic role

3. Tensor Tile Plane
   Statistical summaries, block values, sampled values, visual tiles

4. Visualization Plane
   tileset.json, GLB, GPU buffers, labels, camera state
```

### Artifact Plane

Contains the precise weight data:

```text
model-00001-of-00064.safetensors
model-00002-of-00064.safetensors
...
config.json
tokenizer.json
model.safetensors.index.json
```

SafeTensors provides tensor name, dtype, shape, and byte offsets in the header. This means the Rust parser can read metadata or a tensor slice without loading the whole checkpoint. Metadata can also be retrieved via HTTP Range requests.

### Metadata Plane

Contains small, quickly queryable objects:

```text
Model
Layer
Module
Tensor
TensorBlock
Tile
Expression
QueryResult
VisualizationPreset
```

This is where DuckDB, Arrow, and Parquet are used.

### Tensor Tile Plane

Contains multi-resolution representations:

```text
global statistics
layer summaries
tensor summaries
block summaries
sampled values
exact block values
```

This data should use a dedicated binary format, e.g. `.qtile`, rather than GLB.

### Visualization Plane

Contains only the data needed to render:

* bounding volumes;
* tile hierarchy;
* instance positions;
* quantized colors;
* labels;
* tensor IDs;
* selected statistics.

---

# 3. Overall Architecture

```text
Hugging Face / Local SafeTensors
                │
                ▼
┌───────────────────────────────────┐
│ SafeTensors Ingestion Engine      │
│ header · shards · range reader    │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Architecture Resolver             │
│ names → layers → tensor roles     │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ NSIR Compiler                     │
│ canonical model and tensor schema │
└───────────┬─────────────┬─────────┘
            │             │
            ▼             ▼
┌───────────────────┐  ┌────────────────────┐
│ Metadata Catalog  │  │ Tensor Block Engine│
│ DuckDB/Parquet    │  │ mmap/range/GPU     │
└─────────┬─────────┘  └──────────┬─────────┘
          │                       │
          └───────────┬───────────┘
                      ▼
┌───────────────────────────────────┐
│ Tensor Tile Compiler              │
│ summaries · sampling · pyramids   │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Visualization Artifact Compiler   │
│ tileset.json · GLB · qtile        │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ CesiumJS Viewer / Native Renderer │
│ LOD · query · matrix animation    │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Chat and Mathematical Query Layer │
│ WeightQL · expression planner     │
└───────────────────────────────────┘
```

---

# 4. SafeTensors Ingestion

## 4.1 Import Process

```text
Model URI
→ resolve Hugging Face revision
→ download/read index JSON
→ inspect all SafeTensors headers
→ verify offsets and shapes
→ resolve architecture
→ generate canonical tensor IDs
→ persist metadata
→ optionally build coarse summaries
```

Quatricmorph does not need to download the entire model during import.

Example Rust interface:

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

## 4.2 Architecture Plugins

Each model family has a resolver:

```text
architectures/
├── generic-transformer/
├── llama/
├── qwen/
├── kimi/
├── deepseek/
├── mistral/
└── gemma/
```

The resolver converts:

```text
model.layers.10.self_attn.q_proj.weight
```

into:

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

With MoE:

```json
{
  "layer": 10,
  "component": "moe",
  "expert": 37,
  "operation": "down_projection"
}
```

The resolver must be allowed to return `unknown`. It must never guess a semantic role just because two tensors share the same shape.

---

# 5. Schema and Database

## 5.1 Model Table

```sql
models(
    model_id,
    source_uri,
    source_revision,
    source_hash,
    architecture,
    parameter_count,
    layer_count,
    hidden_size,
    imported_at
)
```

## 5.2 Tensor Table

```sql
tensors(
    tensor_id,
    model_id,
    raw_name,
    canonical_name,
    layer_index,
    component,
    role,
    shape,
    dtype,
    shard_uri,
    byte_start,
    byte_length,
    parameter_count
)
```

## 5.3 Block Table

```sql
tensor_blocks(
    block_id,
    tensor_id,
    lod,
    row_start,
    row_end,
    column_start,
    column_end,
    source_byte_ranges,
    statistics_id,
    content_hash
)
```

## 5.4 Statistics Table

```sql
tensor_statistics(
    statistics_id,
    subject_id,
    count,
    min_value,
    max_value,
    mean,
    variance,
    l1_norm,
    l2_norm,
    zero_ratio,
    positive_ratio,
    negative_ratio,
    histogram,
    approximate,
    algorithm_version
)
```

## 5.5 Visualization Tile Table

```sql
visual_tiles(
    tile_id,
    parent_tile_id,
    model_id,
    tensor_id,
    lod,
    bounds,
    geometric_error,
    qtile_uri,
    glb_uri,
    child_count
)
```

---

# 6. Tensor Addressing

Quatricmorph needs two kinds of addresses.

## 6.1 Canonical Address

```text
model.layers[10].self_attention.query_projection.weight[100,42]
```

A canonical address is unique and reusable across queries, APIs, reports, and annotations.

## 6.2 Contextual Alias

A user might enter:

```text
Att[10][100]
```

But the system must resolve this to an unambiguous object:

```json
{
  "input": "Att[10][100]",
  "resolved_tensor":
    "model.layers.10.self_attn.q_proj.weight",
  "resolved_slice": {
    "row": 100,
    "columns": "all"
  },
  "confidence": 1.0
}
```

If `Att` could refer to Q, K, V, O, or attention probabilities, the query must return a list of candidates rather than silently picking one tensor.

The syntax should support:

```text
Q[10][100, 42]
K[10][0:256, 0:256]
MLP.down[24][:]
Expert[12, 37].up[0:128, :]
```

---

# 7. WeightQL and Mathematical Expressions

The query layer should be standardized under the name **WeightQL**. Morphing continues to use its own Morph IR.

## 7.1 Scalar Query

```sql
SELECT value
FROM tensor(
  "model.layers.10.self_attn.q_proj.weight"
)
AT [100, 42];
```

## 7.2 Slice Query

```sql
SELECT slice
FROM tensor("Q[10]")
ROWS 0:256
COLUMNS 0:256;
```

## 7.3 Statistical Query

```sql
SELECT
    layer_index,
    mean(weight),
    stddev(weight),
    l2_norm(weight)
FROM model("kimi-k3").tensors
WHERE role = "attention_query_projection"
GROUP BY layer_index;
```

## 7.4 Matrix Expressions

```text
A = tensor("Q[10]")
B = tensor("K[10]").transpose()
C = tensor("V[10]")

show (A @ B) @ C
```

The parser converts the expression into an AST:

```text
MatMul
├── MatMul
│   ├── TensorRef(A)
│   └── TensorRef(B)
└── TensorRef(C)
```

The planner then:

1. resolves tensor references;
2. checks shapes;
3. inserts explicitly declared transposes or casts;
4. determines the computation tier;
5. chooses exact, sampled, or block-level execution;
6. builds the visualization graph;
7. executes when the user requests it.

Type-checking example:

```text
A: [128, 4096]
B: [4096, 128]
A @ B: [128, 128]

C: [128, 4096]
(A @ B) @ C: [128, 4096]
```

An incompatible expression must fail before GPU execution.

---

# 8. Matrix Multiplication Visualization

Quatricmorph should not, by default, multiply an entire matrix just to produce an animation.

Instead, the system selects a region or block:

```text
A[i, k] × B[k, j] → C[i, j]
```

## 8.1 Modes

### Concept Mode

Uses simulated or sampled values to explain the multiplication.

### Tensor Block Mode

Executes on a real block:

```text
A[0:256, 0:256] @ B[0:256, 0:256]
```

### Runtime Mode

Uses real activations from a prompt:

```text
hidden_state[token=42]
    @
q_proj.weight
    →
query_vector
```

### Full Compute Mode

Only runs when the user explicitly requests it and has seen the estimated cost.

## 8.2 3D Representation

Each operand is a plane:

```text
A: XY plane
B: YZ plane
C: XZ plane
```

The shared axis represents the reduced dimension:

```text
k = 0 ... K-1
```

Animation:

```text
highlight A[i,k]
→ highlight B[k,j]
→ multiply
→ accumulate C[i,j]
```

For `(A @ B) @ C`, the viewer shows two multiplication nodes and a virtual intermediate tensor. The intermediate tensor does not need to be fully materialized yet; it can exist only as an expression graph.

---

# 9. LOD System for Tensors

## 9.1 Proposed Levels

```text
LOD 0 — Model
A single block representing the whole model.

LOD 1 — Architecture
Embedding, transformer stack, output head, router.

LOD 2 — Layer
One block per layer.

LOD 3 — Tensor
Q, K, V, O, MLP, norm, expert tensors.

LOD 4 — Tensor block
E.g. a 256 × 256 block.

LOD 5 — Sampled or exact values
A single scalar or a small region.
```

## 9.2 Data at Each Level

| LOD | Object        | Data                                          |
| --- | ------------- | ---------------------------------------------- |
| 0   | Model         | parameter count, bytes, global distributions   |
| 1   | Subsystem     | layer ranges, aggregate norms                  |
| 2   | Layer         | tensor count, mean norm, anomaly score         |
| 3   | Tensor        | shape, dtype, histogram, spectrum summary      |
| 4   | Block         | block statistics, quantized samples            |
| 5   | Scalar region | exact or sampled weight values                 |

## 9.3 Loading Rules

```text
zoom out
→ only load summary tiles

zoom in
→ load tensor metadata

zoom deeper
→ load block summaries

select or inspect
→ range-read exact bytes from SafeTensors
```

Cesium 3D Tiles already supports hierarchy, geometric error, and view-based tile loading. 3D Tiles 1.1 allows glTF to be used directly as tile content, along with structured metadata and implicit tiling. CesiumJS also only loads tile content as needed based on the camera.

---

# 10. The Role of GLB and tileset.json

## 10.1 GLB Is Not a Tensor Database

GLB should only contain:

* shared geometry;
* instance transforms;
* quantized visual attributes;
* feature IDs;
* tile-local metadata.

It should not contain:

* full FP16/BF16 weights;
* many copies of the same cube mesh;
* reproducible analysis results;
* exact tensor data for the whole model.

## 10.2 A Sample GLB Tile

```text
tile_12_4_7.glb
├── unit cube mesh
├── instance transforms
├── instance color/value class
├── tensor-local IDs
└── metadata references
```

Khronos has the `EXT_mesh_gpu_instancing` extension for rendering many copies of the same mesh with fewer draw calls. However, Quatricmorph must check the renderer's actual support level and have its own fallback.

## 10.3 Tensor Sidecar

```text
tile_12_4_7.qtile
```

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

The payload can contain:

```text
Morton coordinates
quantized value
flags
optional local tensor index
```

---

# 11. Procedural Rendering

The idea of procedural materials is correct, but procedural generation should live in the shader and GPU buffers, not as a fixed exported "Blender material."

## 11.1 Minimal GPU Record

```rust
#[repr(C)]
pub struct VisualCell {
    pub morton_coordinate: u32,
    pub quantized_value: i16,
    pub flags: u16,
    pub local_id: u32,
}
```

No need to store position three times.

The shader computes position:

```text
position =
    tile_origin
    + decode_morton(morton_coordinate)
      * cell_spacing
```

Color:

```text
negative → negative palette
zero     → neutral palette
positive → positive palette
```

Height or scale:

```text
scale = log(1 + abs(weight)) × normalization
```

## 11.2 Data Sent to the GPU

Only send:

```text
tile origin
tile extent
quantized values
selection mask
filter parameters
normalization parameters
```

Camera culling, cell placement, and color mapping are handled on the GPU.

## 11.3 Cesium CustomShader

CesiumJS allows attaching a `CustomShader` to a tileset's model content, but the documentation currently marks this feature as experimental. It can therefore be used for prototyping but should not become a long-term core dependency.

---

# 12. Two Different Renderers

## 12.1 Renderer A — CesiumJS Prototype

Responsibilities:

* tile traversal;
* camera;
* selection;
* LOD;
* picking;
* tileset debugging;
* visualization MVP.

Stack:

```text
React or Svelte
CesiumJS
3D Tiles 1.1
GLB
CustomShader
Web Worker
```

Cesium is well suited for validating:

```text
model hierarchy
→ tensor tiling
→ progressive loading
→ camera-based inspection
```

But Cesium still carries many GIS and geospatial rendering assumptions.

## 12.2 Renderer B — Quatricmorph Native Tensor Renderer

Responsibilities:

* procedural tensor cells;
* storage buffers;
* compute culling;
* indirect drawing;
* large scatter plots;
* tensor block animation;
* matrix multiplication;
* runtime activation visualization.

Proposed stack:

```text
Tauri
Rust
wgpu
WGSL
Metal / Vulkan / DirectX 12
```

`wgpu` runs natively on Metal, Vulkan, and DirectX 12, and can also run in the browser via WebGPU. This is a suitable abstraction for building a renderer that shares most of its shaders and resource model between desktop and web.

## 12.3 CUDA and Metal Compute Plugins

Not every workload should be forced through the renderer.

```text
Rendering:
wgpu / WebGPU / Metal / Vulkan

Large tensor compute:
CUDA
Metal Performance Shaders
CPU SIMD/BLAS

Experimental runtime:
PyTorch
Candle
custom kernels
```

The CUDA plugin handles:

* full matrix multiplication;
* quantization;
* spectral analysis;
* large checkpoint comparison.

wgpu handles:

* visualization;
* interactive reductions;
* filtering;
* culling;
* lightweight compute.

---

# 13. Caching Architecture

## 13.1 Cache Levels

```text
L0 — GPU resident cache
Visible tiles and selected tensors

L1 — Process memory
Decoded qtiles and hot metadata

L2 — Local NVMe
Content-addressed tile and analysis cache

L3 — Browser
Cache Storage / IndexedDB

L4 — Remote object storage and CDN
Published tiles and shared summaries
```

## 13.2 Cache Key

```text
hash(
    source_model_hash,
    tensor_id,
    logical_slice,
    lod,
    summary_algorithm,
    algorithm_version,
    visualization_encoding
)
```

The color palette does not necessarily need to be part of the cache key if color is computed in the shader.

## 13.3 Prefetching

When the camera moves closer to a tensor:

```text
load current tile
→ prefetch children
→ prefetch sibling metadata
→ do not fetch exact values yet
```

Exact SafeTensors ranges are only read when:

* the user selects a region;
* a query requires exact values;
* an analysis pass requests the block;
* multiplication uses that block.

---

# 14. Local and Web API

## 14.1 Metadata API

```http
GET /v1/models
GET /v1/models/{modelId}
GET /v1/models/{modelId}/layers
GET /v1/tensors/{tensorId}
GET /v1/tensors/{tensorId}/statistics
```

## 14.2 Tensor Block API

```http
GET /v1/tensors/{tensorId}/blocks
    ?rows=0:256
    &columns=0:256
    &format=qtile
    &precision=int8
```

## 14.3 Exact Value API

```http
GET /v1/tensors/{tensorId}/value?index=100,42
```

## 14.4 Visualization API

```http
GET /v1/visualizations/{modelId}/tileset.json
GET /v1/visualizations/{modelId}/tiles/{tileId}.glb
GET /v1/visualizations/{modelId}/tiles/{tileId}.qtile
```

## 14.5 Query API

```http
POST /v1/query
```

```json
{
  "model": "kimi-k3",
  "expression": "(Q[10] @ transpose(K[10])) @ V[10]",
  "mode": "block",
  "slice": {
    "rows": [0, 128],
    "columns": [0, 128]
  }
}
```

Initial response:

```json
{
  "plan_id": "plan:b3:...",
  "status": "planned",
  "estimated_read_bytes": 100663296,
  "estimated_gpu_bytes": 67108864,
  "execution_backend": "webgpu",
  "approximation": "none",
  "visualization_uri": "/v1/plans/.../graph"
}
```

---

# 15. Chat Assistant

Chat must not read weight bytes directly. It calls the WeightQL planner instead.

Example user request:

```text
Show Att[10][100].
```

The assistant builds a plan:

```text
1. Resolve Att in layer 10.
2. Detect four candidates: Q, K, V, O.
3. Use current UI selection: Q projection.
4. Resolve row 100.
5. Request the tensor block containing row 100.
6. Display values and visual highlight.
```

Example:

```text
Compare Q[10][100] with Q[20][100].
```

Resolved query:

```sql
COMPARE
    tensor("Q[10]")[100, :]
WITH
    tensor("Q[20]")[100, :]
BY cosine_similarity, relative_l2;
```

The chat response must distinguish between:

```text
Exact result
Approximate result
Sampled visualization
Statistical interpretation
```

---

# 16. Repository Structure

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

---

# 17. Implementation Roadmap

## Phase 0 — Tensor Tiling Spike

Goal:

```text
Open one SafeTensors file
→ select one 4096 × 4096 tensor
→ create five LOD levels
→ generate tileset.json
→ visualize in CesiumJS
→ click a cell and retrieve the exact value
```

Full-model support is out of scope for this phase.

## Phase 1 — Dense Model Browser

Supports:

* sharded SafeTensors;
* architecture resolver;
* model/layer/tensor hierarchy;
* tensor statistics;
* Cesium LOD;
* exact weight lookup;
* local cache.

Output:

```text
Open a Qwen/Llama-like model
→ zoom to model
→ layer
→ tensor
→ block
→ scalar
```

## Phase 2 — Mathematical Query Engine

Supports:

* tensor aliases;
* slices;
* transpose;
* reshape;
* addition;
* multiplication;
* reduction;
* query plans;
* visual expression graph.

Goal:

```text
Visualize (A @ B) @ C
```

on real tensor blocks.

## Phase 3 — Custom WebGPU Renderer

Replaces detailed GLBs with:

* GPU storage buffers;
* procedural cells;
* compute culling;
* indirect drawing;
* data-driven shaders.

Cesium is used only for overview, or is fully replaced within the tensor workspace.

## Phase 4 — Native GPU Desktop

Implements:

* Tauri;
* wgpu;
* Metal backend;
* Vulkan/DX12 backend;
* CUDA compute plugin;
* GPU memory scheduler;
* multi-GPU jobs.

## Phase 5 — Runtime Neural Observability

Adds:

* hidden states;
* Q/K/V activations;
* attention probabilities;
* residual stream;
* MoE routing;
* token-conditioned visualization;
* matrix multiplication from real prompts.

## Phase 6 — Trillion-Scale Remote Execution

Adds:

* object storage;
* distributed block workers;
* Arrow transfer;
* server-side tile generation;
* query result streaming;
* shared workspaces;
* CDN-published visualization summaries.

---

# 18. Concrete MVP

The first MVP should not start with the full Kimi K3.

It should choose:

```text
Model: 0.5B–7B SafeTensors
Architecture: Qwen or Llama-like
Tensor: Q projection or MLP down projection
Viewer: CesiumJS
LOD: model → layer → tensor → block
Query: exact scalar and tensor slice
Math: one A @ B visualization
```

## Acceptance Criteria

1. Do not load the entire checkpoint into RAM.
2. Successfully parse sharded SafeTensors.
3. Metadata import can be cancelled and resumed.
4. Clicking a visual cell returns the correct tensor address.
5. The exact scalar must match the Python SafeTensors reference.
6. Zooming out does not load exact values.
7. Zooming in only reads the necessary byte ranges.
8. The cache is reused after reopening.
9. An expression with a shape mismatch is rejected before execution.
10. The UI clearly indicates exact, sampled, or approximate results.

---

# 19. What Not to Do

## Do not create one cube GLB per weight

This is a data explosion, not a visualization optimization.

## Do not store absolute positions for every scalar

Position must be computed from:

```text
tile origin + logical index + layout rule
```

## Do not send the entire tensor into the browser

The browser should only receive visual tiles and selected exact slices.

## Do not use Cesium as a tensor compute engine

Cesium is a tile-traversal and rendering layer.

## Do not let chat freely execute terabyte-scale expressions

Chat must produce a plan and show the estimated I/O first.

## Do not assume a color pattern corresponds to a semantic concept

Raw weight visualization only represents numerical structure; it does not prove cognitive function.

---

# 20. Target Architecture

```text
SafeTensors
    ↓
NSIR semantic model
    ↓
Tensor-native block database
    ↓
Multiresolution Tensor Tiles
    ↓
WeightQL and mathematical expressions
    ↓
CesiumJS overview
    +
custom WebGPU tensor renderer
    ↓
Metal / CUDA acceleration
    ↓
runtime activations and model morphing
```

Quatricmorph should not become:

```text
SafeTensors → billions of cube GLBs
```

But instead should become:

```text
SafeTensors
→ semantic tensor address space
→ queryable block hierarchy
→ procedural multiresolution visualization
→ exact on-demand computation
```

This is also consistent with the current product architecture: the tensor database and virtual computational objects form the core layer; visualization is just one projection of the same data and query substrate.
