Moonshot has now announced Kimi K3 as a **2.8 trillion parameter** model. If you stored all 2.8T parameters in FP16, the raw weights alone would be about **5.6 TB**; even 4-bit would still be around **1.4 TB**. The browser clearly isn't where the entire model should live.

But a `[3072, 1792]` tensor is a different story:

```
3072 × 1792 = 5,505,024
```

That's only ~5.5 million scalars. In FP32 that's ~22 MB, FP16 ~11 MB, uint8 visualization ~5.5 MB. A 3072×1792 size is also smaller than WebGPU's common minimum 2D texture limit of 8192×8192.

So **a `[3072,1792]` matrix can absolutely be rendered directly on the GPU**. What you shouldn't do is create 5.5 million `Mesh` objects or spheres.

---

# 1. Best strategy: Tensor → Texture → 1 Quad

If the goal is to view the weight matrix as a heatmap:

```
5,505,024 weights
       ↓
3072 × 1792 GPU texture
       ↓
Fragment shader
       ↓
1 PlaneGeometry
       ↓
~2 triangles
```

This is the most efficient approach.

Three.js has `DataTexture`, which lets you create a texture directly from a `TypedArray`.

Example idea:

```ts
const texture = new THREE.DataTexture(
    values,
    1792,
    3072,
    THREE.RedFormat,
    THREE.FloatType
);

texture.needsUpdate = true;

const material = new THREE.ShaderMaterial({
    uniforms: {
        tensor: { value: texture },
        minValue: { value: min },
        maxValue: { value: max },
    },

    vertexShader: `...`,

    fragmentShader: `
        uniform sampler2D tensor;

        void main() {
            float w = texture2D(tensor, vUv).r;

            // weight → colormap
            ...
        }
    `
});
```

The GPU doesn't need to create geometry corresponding to 5.5M weights. Each weight becomes a **texel**, and the fragment shader decides which pixel needs to be rasterized.

This should be Quatricmorph's default renderer.

---

# 2. Don't render a sphere for every weight

Suppose you use:

```
5.5M weights
=
5.5M spheres
```

Even though `THREE.InstancedMesh` drastically reduces draw calls, it **doesn't eliminate the vertex/triangle cost of each sphere**. Three.js also describes `InstancedMesh` as mainly a way to reduce draw calls for objects sharing the same geometry/material.

For example, an extremely low-poly sphere with just 20 triangles:

```
5.5M × 20
≈ 110 million triangles
```

That's before even counting shading, depth, transforms, and overdraw.

So:

```
❌ Mesh per scalar
❌ Sphere per scalar

⚠️ InstancedMesh per scalar

✅ Texture heatmap
✅ GPU Points
✅ procedural quads
✅ LOD aggregation
```

---

# 3. If you want 3D visualization

Quatricmorph could have a mode where:

```
x = column
y = row
z = weight value
```

but in that case I'd use **GPU points**, not spheres.

Example:

```
3072 × 1792

        • • • •
      • • • •
    • • • •
  • • • •

height = weight
color  = weight / delta
```

You don't even need to upload XYZ.

The shader can derive position from `vertex_index`:

```
index
  ↓
row = index / width
col = index % width

x = col
y = row
z = tensor[index]
```

So the GPU only needs:

```
tensor values
```

not:

```
position[]
normal[]
matrix[]
color[]
```

WebGPU is especially well-suited to this approach because Three.js now supports storage buffers and compute via `WebGPURenderer`, `StorageBufferAttribute`, TSL, and storage textures.

---

# 4. But the real solution for Quatricmorph is Semantic LOD

Don't treat tensor visualization like an ordinary 3D scene.

You need:

> **Tensor-specific Level of Detail.**

For example, a matrix:

```
3072 × 1792
```

creates a pyramid:

```
LOD 0    3072 × 1792    5.50M
LOD 1    1536 × 896     1.38M
LOD 2     768 × 448     344K
LOD 3     384 × 224      86K
LOD 4     192 × 112      21K
LOD 5      96 × 56        5K
LOD 6      48 × 28        1K
```

When the tensor is occupying:

```
500 × 300 pixels
```

then rendering:

```
3072 × 1792
```

is pointless.

You only need about:

```
500 × 300
```

samples.

---

# 5. But ordinary mipmaps aren't enough

This is where Quatricmorph can differ from an image viewer.

If averaging:

```
[0.001
 0.003
 9.830
 0.001]
```

into one pixel, you could lose the outlier `9.830`.

So each tensor block should keep multiple statistics:

```
TensorBlock
├── mean
├── min
├── max
├── maxAbs
├── RMS
├── variance
├── L1
├── L2
├── zeroRatio
├── positiveRatio
├── negativeRatio
├── histogram
└── outlierCount
```

Then Quatricmorph has visualization modes such as:

```
Mean
Magnitude
Max magnitude
Variance
Sparsity
Distribution
Gradient
Delta
Cosine similarity
Sign disagreement
```

This is a **semantic mipmap** rather than an image mipmap.

---

# 6. LOD matters even more for morph/diff

For example:

```
A = original weight
B = fine-tuned weight

Δ = B - A
```

Quatricmorph should have its own pyramid:

```
          Tensor A
             │
Tensor B ────┼──── GPU
             │
             ▼
        delta = B-A
             │
        ┌────┴─────┐
        │          │
       mean       maxAbs
        │          │
        └────┬─────┘
             ▼
       visualization
```

When zoomed out:

```
color = maxAbs(Δ block)
```

When zoomed in:

```
color = exact Δ[i,j]
```

That way, a change that's tiny in area but huge in magnitude still shows up.

This is an extremely important feature for model inspection.

---

# 7. WebGPU lets Quatricmorph push diffing onto the GPU

A good architecture would be:

```
Tensor A ──► GPU Storage Buffer ─┐
                                ├─► Compute Shader
Tensor B ──► GPU Storage Buffer ─┘
                                        │
                       ┌────────────────┼───────────────┐
                       ▼                ▼               ▼
                     B-A            abs(B-A)        statistics
                       │                │               │
                       └────────────────┼───────────────┘
                                        ▼
                               Storage Texture
                                        │
                                        ▼
                                  Three.js render
```

Three.js currently supports compute-oriented storage buffers and storage textures in the WebGPU backend.

This avoids the workflow:

```
GPU → CPU → JS → GPU
```

for every operation.

---

# 8. Morph parameters don't even need to create a new tensor

For example:

```
W_α = (1-α)W_A + αW_B
```

Don't create:

```js
const C = new Float32Array(5_500_000);
```

every time the slider changes.

Just upload:

```
A texture
B texture
α uniform
```

Shader:

```glsl
float a = texture(A, uv).r;
float b = texture(B, uv).r;

float w = mix(a, b, alpha);
```

When the user drags:

```
Morph
0 ─────────●───────── 1
A                      B
```

The CPU only changes one scalar:

```ts
uniforms.alpha.value = value;
```

The GPU generates the visualization itself.

You can have:

```
Original
Morph result
Delta
Magnitude
Sign change
```

nearly in real time.

---

# 9. A 2.8T model needs hierarchical virtualization

For Kimi K3, the UI should treat the model as a tree:

```
Kimi K3
│
├── embedding
│
├── layer 0
│   ├── attention
│   │   ├── q_proj
│   │   ├── k_proj
│   │   ├── v_proj
│   │   └── o_proj
│   │
│   ├── experts
│   │   ├── expert 0
│   │   ├── expert 1
│   │   └── ...
│   │
│   └── ...
│
├── layer 1
│
├── ...
│
└── lm_head
```

At the model level:

```
1 layer ≈ 1 visual object
```

not:

```
1 parameter ≈ 1 visual object
```

---

# 10. Quatricmorph's semantic zoom

I'd design around 5 levels.

### Level 0 — Model

```
Kimi K3
┌──────────────────────────────┐
│ Embedding                    │
│ Layer 0                      │
│ Layer 1                      │
│ Layer 2                      │
│ ...                          │
│ Output                       │
└──────────────────────────────┘
```

Each layer shows:

```
parameter count
dtype
mean
std
norm
sparsity
delta
```

---

### Level 1 — Layer

```
Layer 73

┌───────────┐ ┌───────────┐
│ Attention │ │ Experts   │
└───────────┘ └───────────┘

┌────┐ ┌────┐ ┌────┐ ┌────┐
│ Q  │ │ K  │ │ V  │ │ O  │
└────┘ └────┘ └────┘ └────┘
```

---

### Level 2 — Tensor

```
q_proj.weight

shape
[3072,1792]
```

Render mip:

```
384 × 224
```

---

### Level 3 — Tile

Zoom:

```
[1536:1792, 512:768]
```

load:

```
256 × 256
```

---

### Level 4 — Scalar

Only once the user zooms in close enough:

```
row 1873
column 934

A       = -0.01831
B       = -0.02382
Δ       = -0.00551
relative = 30.1%
```

---

# 11. Use virtual tensor tiles

For very large tensors, split into:

```
256 × 256
```

or:

```
512 × 512
```

tiles.

For example:

```
Tensor

┌────┬────┬────┬────┐
│ T0 │ T1 │ T2 │ T3 │
├────┼────┼────┼────┤
│ T4 │ T5 │ T6 │ T7 │
├────┼────┼────┼────┤
│ ...                  │
└──────────────────────┘
```

The viewport only requests:

```
visible tiles
+
1 tile margin
```

Just like Google Maps.

---

# 12. GPU tile cache

The browser might hold:

```
GPU cache: 256–1024 MB
CPU cache: 0.5–2 GB
```

depending on the hardware.

For example:

```
                  ┌─────────────┐
Viewport ────────►│ Tile Manager│
                  └──────┬──────┘
                         │
             ┌───────────┼───────────┐
             ▼           ▼           ▼
          GPU cache   CPU cache    Network
             │           │           │
             └───────────┴───────────┘
```

LRU:

```
visible       → pin
near viewport → high priority
recent        → cache
old           → evict
```

WebGPU implementations may limit memory to keep the browser/system responsive, so Quatricmorph shouldn't assume all VRAM is available.

---

# 13. Don't send BF16/FP32 if it's only for visualization

A very significant optimization:

Model:

```
BF16 / FP16
```

but visualization doesn't necessarily need that precision.

For example, each tile stores:

```
raw        FP16
visual     uint8
stats      FP32
```

Heatmap:

```
value
 ↓
normalization
 ↓
0..255
 ↓
Uint8
```

Matrix `[3072,1792]`:

```
FP32     ~22 MB
FP16     ~11 MB
Uint8   ~5.5 MB
```

If the mip currently shown is 384×224:

```
384 × 224
≈ 86 KB
```

You cut roughly:

```
22 MB
→
86 KB
```

for that visualization frame.

---

# 14. Exact weights are only loaded when needed

Visualization can use quantized data.

When the user hovers:

```
weight [1234,872]
```

the client requests the exact:

```
FP16/BF16 value
```

Or loads a small block:

```
16 × 16
```

around the cursor.

That way:

```
visual fidelity ≠ numerical fidelity
```

The UI stays accurate for inspection without needing to keep the entire tensor at full precision on the GPU.

---

# 15. Safetensors is well suited as the source index

The Safetensors header contains:

```
tensor name
dtype
shape
data_offsets
```

and Hugging Face can parse the metadata using small HTTP Range requests.

So the loader can:

```
model.safetensors.index.json
            │
            ▼
      TensorRegistry
            │
    ┌───────┼────────┐
    ▼       ▼        ▼
 tensor A tensor B tensor C
```

But I would **not recommend using the Safetensors layout directly as the visualization tile store**.

The reason is that a rectangular region:

```
256 rows × 256 cols
```

in a row-major tensor isn't necessarily a contiguous file range.

You'd need a lot of range reads.

---

# 16. Quatricmorph should have its own format/cache for visualization

For example:

```
model.qm/
│
├── manifest.json
│
├── tensors/
│   │
│   ├── q_proj/
│   │   ├── metadata.json
│   │   │
│   │   ├── lod0/
│   │   │   ├── 0_0.tile
│   │   │   ├── 0_1.tile
│   │   │   └── ...
│   │   │
│   │   ├── lod1/
│   │   ├── lod2/
│   │   └── stats/
│   │
│   └── ...
```

Each tile:

```
Header
├── shape
├── dtype
├── min
├── max
├── mean
├── std
├── maxAbs
└── histogram

Payload
└── quantized tensor
```

---

# 17. Lob is extremely well suited to this problem

If you use the content-addressed block architecture you're building for Lob, I wouldn't even create a separate Quatricmorph format.

You could have:

```
Safetensors
      │
      ▼
    Lob
      │
      ├── tensor manifest
      │
      ├── 256 × 256 blocks
      │
      ├── mip blocks
      │
      └── statistics blocks
              │
              ▼
        Quatricmorph
```

A tensor:

```
q_proj.weight
[3072,1792]

manifest
│
├── block 00 → sha256:ab87...
├── block 01 → sha256:91ae...
├── block 02 → sha256:817c...
└── ...
```

If two checkpoints share identical blocks, they're only stored once.

This also makes Quatricmorph diffing natural:

```
Model A             Model B

block AA ────────── block AA
block AB ────────── block AB
block AC        ┌── block XY
block AD        └── block XZ
```

Only the changed blocks get fetched.

---

# 18. WebGPU should be the primary renderer

Three.js now has `WebGPURenderer`; this renderer prioritizes WebGPU and can fall back to WebGL2.

I'd split it as:

```
                  Quatricmorph Renderer
                          │
               ┌──────────┴──────────┐
               ▼                     ▼
         WebGPU backend         WebGL2 fallback
               │                     │
          full features          basic viewer
```

### WebGPU

For:

```
compute diff
compute aggregation
histogram
morph
LOD generation
GPU filtering
GPU picking
indirect draw
```

Three.js even exposes indirect geometry draw data generated by a compute shader in the WebGPU path.

### WebGL2

For:

```
heatmap
texture diff
simple morph
basic point cloud
```

---

# 19. GPU-driven LOD

A more advanced architecture:

```
                         camera
                           │
                           ▼
                    Compute shader
                           │
             calculate visible tiles
                           │
               ┌───────────┴───────────┐
               ▼                       ▼
            visible                 hidden
               │
               ▼
         indirect draw
```

The CPU doesn't have to loop:

```js
for (const tile of millionsOfTiles)
```

every frame.

The GPU decides which tile gets drawn.

---

# 20. Keep only the visible working set

For example, if the user is viewing:

```
Kimi K3
 ↓
layer 72
 ↓
expert 31
 ↓
down_proj.weight
 ↓
viewport
```

the GPU might only hold:

```
20–100 tiles
```

instead of:

```
2.8 trillion parameters
```

This is exactly **out-of-core tensor visualization**.

---

# 21. The architecture I recommend

```
             Kimi K3 / Qwen / Llama
                     Safetensors
                          │
                          ▼
               ┌───────────────────┐
               │ Tensor Indexer    │
               │                   │
               │ shape             │
               │ dtype             │
               │ offsets           │
               └─────────┬─────────┘
                         │
                         ▼
               ┌───────────────────┐
               │ Tensor Tile Store │
               │                   │
               │ 256×256 blocks    │
               │ statistics        │
               │ mip pyramid       │
               └─────────┬─────────┘
                         │
                HTTP / local IPC
                         │
                         ▼
              ┌─────────────────────┐
              │ Quatricmorph Client │
              └──────────┬──────────┘
                         │
                   Tile Scheduler
                         │
              ┌──────────┴──────────┐
              ▼                     ▼
        Web Worker               GPU cache
              │                     │
              └──────────┬──────────┘
                         ▼
                    WebGPU
                         │
           ┌─────────────┼────────────┐
           ▼             ▼            ▼
         compute       texture       points
           │             │            │
           └─────────────┼────────────┘
                         ▼
                     Three.js
```

---

# 22. An optimization especially well suited to Quatricmorph: Delta-first rendering

If comparing two models:

```
A
B
```

you might naturally think you need to render:

```
A
B
A-B
```

But what the UI usually cares about most is:

```
Where did the model change?
```

So the global model view only needs to store:

```
per-block delta statistics
```

For example:

```
Layer 0      Δ 0.0003
Layer 1      Δ 0.0007
Layer 2      Δ 0.3910   ← highlight
...
Layer 71     Δ 0.0012
```

User clicks layer 2:

```
Tensor q_proj      Δ 0.91
Tensor k_proj      Δ 0.02
Tensor v_proj      Δ 0.04
```

Clicks q_proj:

```
tiles
```

And only then are the actual values finally loaded.

It behaves more like a debugger than a 3D viewer.

---

# 23. Importance sampling instead of random sampling

When you want a 3D particle visualization:

Don't:

```
random(weight)
```

Instead sample according to:

```
P(i) ∝ |w_i|
```

or, for diffing:

```
P(i) ∝ |Δw_i|
```

or a hybrid:

```
score_i =
α |Δw_i|
+ β |w_i|
+ γ variance(block_i)
```

This way, 100k points can convey more information than 5M uniformly sampled points.

---

# 24. I'd set a hard budget

For example, for Quatricmorph desktop/web:

```
Maximum raster heatmap:
    ~8M visible texels

Maximum point visualization:
    250K–1M points

Maximum detailed cubes/spheres:
    10K–100K

GPU tensor cache:
    configurable

CPU visualization cache:
    configurable

Full model:
    unlimited/out-of-core
```

There's no reason to render 2.8T scalars all at once.

---

# 25. Automatic renderer selection

You could implement:

```ts
if (screenPixels < 250_000)
    renderAggregatedTexture();

else if (elements < 8_000_000)
    renderTensorTexture();

else
    renderTiledTensor();

if (mode === "3d") {
    if (elements > 500_000)
        renderImportanceSampledPoints();
    else
        renderPoints();
}
```

In other words, Quatricmorph chooses its own representation automatically.

---

# 26. This is the final architecture I recommend

I'd split the renderer into just **4 primitives**, no more:

```
Quatricmorph Tensor Renderer

1. TensorHeatmap
   └── texture / virtual texture

2. TensorSurface
   └── procedural grid

3. TensorPoints
   └── GPU point cloud

4. TensorAggregate
   └── block statistics
```

Then:

```
                 zoom out
                    ↑
                    │
             TensorAggregate
                    │
              TensorHeatmap
                    │
              TensorSurface
                    │
              TensorPoints
                    │
                    ↓
                  zoom in
```

---

## What I think matters most

If Quatricmorph really wants to support models the size of **Kimi K3's 2.8T**, I would **not** design it as a Three.js application that simply reads Safetensors.

I would design it as:

> **An out-of-core tensor database + GPU tensor visualization engine.**

Pipeline:

```
Safetensors
      ↓
tensor/block index
      ↓
content-addressed tiles
      ↓
statistical mip pyramid
      ↓
viewport-driven streaming
      ↓
WebGPU compute
      ↓
Three.js
```

And specifically for a `[3072,1792]` matrix, the default mode only needs:

> **one `DataTexture` + one quad + a shader**, not 5.5 million objects.

Quatricmorph's real potential strength then isn't just "viewing weights" — it's **Google Maps / Unreal Nanite–style virtualization built specifically for neural-network parameter space**: zooming from 2.8T parameters down to a single scalar without ever having to load the entire model into the browser.