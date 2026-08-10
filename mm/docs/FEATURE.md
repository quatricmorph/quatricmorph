Quatricmorph should treat tensor interaction as a **first-class interaction system**, not merely a WebGL viewer. The closest mental model is:

**Blender viewport + CAD inspector + tensor debugger + scientific visualization + immutable model editor.**

The interaction hierarchy should be:

`Model → Layer → Module → Tensor → Block/Tile → Slice/Row/Column → Scalar`

while preserving lazy/out-of-core loading rather than materializing entire tensors.

## 1. Complete Tensor Interaction Feature Map

| System                           | Feature                      | Expected behavior                                                         |     |   |
| -------------------------------- | ---------------------------- | ------------------------------------------------------------------------- | --- | - |
| **Viewport Navigation**          | Orbit                        | Blender-style MMB orbit around pivot                                      |     |   |
|                                  | Pan                          | Shift+MMB / two-finger pan                                                |     |   |
|                                  | Dolly / Zoom                 | Wheel/pinch, logarithmic speed for huge scenes                            |     |   |
|                                  | Fly navigation               | WASD navigation through very large models                                 |     |   |
|                                  | Zoom to cursor               | Cursor location becomes zoom target                                       |     |   |
|                                  | Frame Selected               | `F` focuses currently selected tensor/block                               |     |   |
|                                  | Frame All                    | Fit visible model into viewport                                           |     |   |
|                                  | Reset View                   | Restore canonical camera                                                  |     |   |
|                                  | Camera presets               | Front / Back / Left / Right / Top / Bottom                                |     |   |
|                                  | Perspective / Orthographic   | Toggle projection                                                         |     |   |
|                                  | View cube / navigation gizmo | Blender/CAD-style orientation control                                     |     |   |
|                                  | Orbit pivot control          | World / selection / cursor                                                |     |   |
|                                  | Semantic zoom                | Zoom changes Model → Layer → Tensor → Block → Scalar representation       |     |   |
|                                  | Smooth transition            | Animate between hierarchy levels                                          |     |   |
|                                  | Minimap                      | Overview of huge model/tensor space                                       |     |   |
|                                  | Breadcrumb navigation        | `Model / layer.32 / q_proj / weight / block[4,8]`                         |     |   |
| **Hover**                        | Object hover                 | Highlight object below pointer                                            |     |   |
|                                  | Hover tooltip                | Name, shape, dtype, location, value/statistics                            |     |   |
|                                  | Scalar hover                 | `tensor[i,j] = value`                                                     |     |   |
|                                  | Block hover                  | Block bounds and block statistics                                         |     |   |
|                                  | Semantic hover               | Tensor role: Q/K/V/O, MLP, embedding, expert, etc.                        |     |   |
|                                  | Relationship preview         | Hover tensor highlights connected tensors                                 |     |   |
| **Picking**                      | GPU picking                  | Fast object detection independent of triangle count                       |     |   |
|                                  | ID-buffer picking            | Every visible logical entity gets pick ID                                 |     |   |
|                                  | LOD-aware picking            | Clicking aggregate object selects aggregate; zoom enables finer selection |     |   |
|                                  | Precise scalar picking       | Map pixel → tile → logical tensor coordinate                              |     |   |
|                                  | Depth-aware picking          | Correct selection in overlapping tensor views                             |     |   |
| **Selection**                    | Click Select                 | Single object                                                             |     |   |
|                                  | Shift Select                 | Add/remove selection                                                      |     |   |
|                                  | Active object                | One active entity among selected entities                                 |     |   |
|                                  | Box Select                   | Blender `B`                                                               |     |   |
|                                  | Lasso Select                 | Irregular region                                                          |     |   |
|                                  | Circle Select                | Brush-style selection                                                     |     |   |
|                                  | Select All                   | Current interaction scope                                                 |     |   |
|                                  | Invert Selection             | Select complement                                                         |     |   |
|                                  | Select Similar               | Same module/type/dtype/shape/statistical property                         |     |   |
|                                  | Range Select                 | Rows/columns/index intervals                                              |     |   |
|                                  | Slice Select                 | `tensor[128:256, :]`                                                      |     |   |
|                                  | Axis Select                  | Select entire row/column/channel/head                                     |     |   |
|                                  | Block Select                 | Select tensor tile                                                        |     |   |
|                                  | Semantic Select              | All attention Q weights, all experts, etc.                                |     |   |
|                                  | Query Select                 | `select where abs(value) > x`                                             |     |   |
|                                  | Selection Set                | Save named selection                                                      |     |   |
|                                  | Lock Selection               | Prevent accidental change                                                 |     |   |
| **Selection Modes**              | Model mode                   | Whole checkpoint                                                          |     |   |
|                                  | Layer mode                   | Transformer layer                                                         |     |   |
|                                  | Module mode                  | attention/MLP/etc.                                                        |     |   |
|                                  | Tensor mode                  | complete tensor                                                           |     |   |
|                                  | Block mode                   | tile/chunk                                                                |     |   |
|                                  | Slice mode                   | arbitrary N-D subset                                                      |     |   |
|                                  | Scalar mode                  | individual value                                                          |     |   |
| **Highlight**                    | Hover highlight              | Temporary outline/glow                                                    |     |   |
|                                  | Selected highlight           | Persistent selection indication                                           |     |   |
|                                  | Active highlight             | Stronger indication than normal selected                                  |     |   |
|                                  | Parent highlight             | Show containing tensor/module                                             |     |   |
|                                  | Child highlight              | Optional selected descendants                                             |     |   |
|                                  | Relationship highlight       | Connected tensors                                                         |     |   |
|                                  | Diff highlight               | Changed regions                                                           |     |   |
|                                  | Threshold highlight          | Values satisfying predicate                                               |     |   |
|                                  | NaN/Inf highlight            | Invalid numerical regions                                                 |     |   |
|                                  | Outlier highlight            | Statistical anomalies                                                     |     |   |
|                                  | Gradient heat highlight      | magnitude/sign/distribution                                               |     |   |
|                                  | Search-result highlight      | WeightQL result overlay                                                   |     |   |
| **Visibility**                   | Hide Selected                | Blender `H`                                                               |     |   |
|                                  | Hide Unselected              | isolate region                                                            |     |   |
|                                  | Unhide All                   | restore                                                                   |     |   |
|                                  | Local View                   | Blender `/` isolate selected entity                                       |     |   |
|                                  | X-Ray                        | Select through foreground objects                                         |     |   |
|                                  | Ghost objects                | Context visible with reduced prominence                                   |     |   |
|                                  | Layer visibility             | toggle layer/module/tensor                                                |     |   |
|                                  | Filter visibility            | dtype/name/type/query                                                     |     |   |
|                                  | LOD visibility               | control aggregation level                                                 |     |   |
| **Tensor Inspector**             | Identity                     | canonical tensor path                                                     |     |   |
|                                  | Shape                        | dimensions                                                                |     |   |
|                                  | Dtype                        | BF16/FP16/FP32/INT8/etc.                                                  |     |   |
|                                  | Device/storage               | file/shard/backend/location                                               |     |   |
|                                  | Size                         | elements + bytes                                                          |     |   |
|                                  | Min/max                      | exact or sampled                                                          |     |   |
|                                  | Mean/std                     | statistics                                                                |     |   |
|                                  | Norms                        | L1/L2/Frobenius                                                           |     |   |
|                                  | Sparsity                     | zero/near-zero ratio                                                      |     |   |
|                                  | Quantiles                    | value distribution                                                        |     |   |
|                                  | Histogram                    | visual distribution                                                       |     |   |
|                                  | NaN/Inf                      | numerical validation                                                      |     |   |
|                                  | Tensor provenance            | source checkpoint                                                         |     |   |
|                                  | Architecture role            | semantic meaning                                                          |     |   |
|                                  | Offset                       | SafeTensors byte range                                                    |     |   |
|                                  | Exactness                    | exact / sampled / approximate                                             |     |   |
| **Coordinate Inspector**         | Tensor cursor                | Current logical `[i,j,...]`                                               |     |   |
|                                  | Axis labels                  | dimension meaning                                                         |     |   |
|                                  | Global ↔ local index         | tile coordinate conversion                                                |     |   |
|                                  | World ↔ tensor coordinate    | viewport position mapping                                                 |     |   |
|                                  | Selected bounds              | index range                                                               |     |   |
|                                  | Selection count              | number of elements                                                        |     |   |
| **Gizmo System**                 | Tensor gizmo                 | interaction handles                                                       |     |   |
|                                  | Axis handles                 | X/Y/Z corresponding to tensor axes                                        |     |   |
|                                  | Range handles                | drag slice boundaries                                                     |     |   |
|                                  | Plane handles                | select matrix planes                                                      |     |   |
|                                  | Slice plane                  | move through N-D tensors                                                  |     |   |
|                                  | Threshold handle             | interactive clipping                                                      |     |   |
|                                  | LOD handle                   | expand/collapse resolution                                                |     |   |
| **Direct Editing**               | Scalar edit                  | type exact value                                                          |     |   |
|                                  | Row/column edit              | operate over selected slice                                               |     |   |
|                                  | Zero                         | set selection to zero                                                     |     |   |
|                                  | Fill                         | constant value                                                            |     |   |
|                                  | Scale                        | `W *= α`                                                                  |     |   |
|                                  | Add                          | `W += Δ`                                                                  |     |   |
|                                  | Clamp                        | min/max                                                                   |     |   |
|                                  | Normalize                    | selected region                                                           |     |   |
|                                  | Mask                         | boolean selection                                                         |     |   |
|                                  | Copy/Paste                   | between compatible regions                                                |     |   |
|                                  | Replace                      | selection from another checkpoint                                         |     |   |
|                                  | Interpolate                  | `W = (1-t)A + tB`                                                         |     |   |
|                                  | Noise                        | controlled perturbation                                                   |     |   |
|                                  | Quantize preview             | FP16→INT8 etc.                                                            |     |   |
| **Transform Architecture**       | Non-destructive operations   | Original SafeTensors immutable                                            |     |   |
|                                  | Delta overlay                | edits represented separately                                              |     |   |
|                                  | Operation stack              | Blender modifier-like workflow                                            |     |   |
|                                  | Enable/disable operation     | interactive experimentation                                               |     |   |
|                                  | Reorder operation            | operation graph                                                           |     |   |
|                                  | Parameter editing            | change transform after creation                                           |     |   |
|                                  | Apply/bake                   | create derived checkpoint                                                 |     |   |
|                                  | Provenance                   | every operation recorded                                                  |     |   |
| **Undo / History**               | Undo                         | operation-level undo                                                      |     |   |
|                                  | Redo                         | redo operation                                                            |     |   |
|                                  | Timeline                     | inspect edit sequence                                                     |     |   |
|                                  | Named snapshots              | checkpoints inside session                                                |     |   |
|                                  | Compare before/after         | interactive A/B                                                           |     |   |
|                                  | Restore selection            | undo selection changes separately                                         |     |   |
| **Tensor Cursor**                | 3D Tensor Cursor             | Blender 3D cursor analogue                                                |     |   |
|                                  | Place cursor                 | click location                                                            |     |   |
|                                  | Snap cursor                  | scalar/block/tensor center                                                |     |   |
|                                  | Cursor coordinates           | logical tensor indices                                                    |     |   |
|                                  | Operations around cursor     | pivot transformations                                                     |     |   |
| **Snapping**                     | Scalar snap                  | nearest scalar coordinate                                                 |     |   |
|                                  | Block snap                   | tile boundaries                                                           |     |   |
|                                  | Row/column snap              | matrix axes                                                               |     |   |
|                                  | Tensor edge snap             | bounds                                                                    |     |   |
|                                  | Semantic snap                | head/expert/channel boundary                                              |     |   |
|                                  | Grid snap                    | visualization grid                                                        |     |   |
| **Outliner**                     | Model tree                   | checkpoint hierarchy                                                      |     |   |
|                                  | Expand/collapse              | lazy tree                                                                 |     |   |
|                                  | Select from tree             | synchronized with viewport                                                |     |   |
|                                  | Viewport → tree sync         | reveal selected node                                                      |     |   |
|                                  | Visibility toggle            | eye icon                                                                  |     |   |
|                                  | Lock toggle                  | prevent modification                                                      |     |   |
|                                  | Search                       | fuzzy tensor lookup                                                       |     |   |
|                                  | Filter                       | module/tensor/dtype                                                       |     |   |
|                                  | Multi-select                 | batch operations                                                          |     |   |
| **Properties Panel**             | Object properties            | selected logical entity                                                   |     |   |
|                                  | Visual properties            | representation settings                                                   |     |   |
|                                  | Statistics                   | tensor analytics                                                          |     |   |
|                                  | Transform stack              | modifications                                                             |     |   |
|                                  | Compare                      | reference tensor                                                          |     |   |
|                                  | Provenance                   | checkpoint/history                                                        |     |   |
|                                  | Validation                   | problems/warnings                                                         |     |   |
| **Context Menu**                 | Inspect                      | open inspector                                                            |     |   |
|                                  | Focus                        | frame selection                                                           |     |   |
|                                  | Hide/isolate                 | visibility                                                                |     |   |
|                                  | Compare with...              | tensor diff                                                               |     |   |
|                                  | Query                        | create WeightQL query                                                     |     |   |
|                                  | Transform                    | add transformation                                                        |     |   |
|                                  | Export selection             | slice/tensor                                                              |     |   |
|                                  | Copy tensor path             | clipboard                                                                 |     |   |
| **Search / Command**             | Global Search                | find tensor/module                                                        |     |   |
|                                  | Command Palette              | VS Code-style                                                             |     |   |
|                                  | WeightQL                     | programmatic selection                                                    |     |   |
|                                  | Search by shape              | e.g. `[4096,4096]`                                                        |     |   |
|                                  | Search by role               | Q/K/V/expert                                                              |     |   |
|                                  | Search by statistics         | outliers/sparsity/etc.                                                    |     |   |
|                                  | Search → selection           | query results selectable                                                  |     |   |
| **Tensor Visualization**         | Heatmap                      | matrix values                                                             |     |   |
|                                  | Point/grid view              | spatial values                                                            |     |   |
|                                  | Volume                       | rank-3+                                                                   |     |   |
|                                  | Histogram                    | distribution                                                              |     |   |
|                                  | Density                      | huge tensors                                                              |     |   |
|                                  | Signed magnitude             | positive/negative                                                         |     |   |
|                                  | Difference view              | A-B                                                                       |     |   |
|                                  | Ratio view                   | A/B                                                                       |     |   |
|                                  | Sparsity view                | zeros                                                                     |     |   |
|                                  | Quantization view            | quantization bins/errors                                                  |     |   |
|                                  | Statistical LOD              | aggregate blocks                                                          |     |   |
| **Clipping / Slicing**           | Axis slicing                 | choose index along dimension                                              |     |   |
|                                  | Range clipping               | min/max indices                                                           |     |   |
|                                  | Arbitrary slice              | tensor expression                                                         |     |   |
|                                  | Exploded view                | separate slices                                                           |     |   |
|                                  | Cross-section                | inspect interior                                                          |     |   |
|                                  | Slice animation              | move plane across dimension                                               |     |   |
| **Compare Mode**                 | Side-by-side                 | A versus B                                                                |     |   |
|                                  | Overlay                      | same coordinate space                                                     |     |   |
|                                  | Difference heatmap           | `A-B`                                                                     |     |   |
|                                  | Absolute difference          | `                                                                         | A-B | ` |
|                                  | Relative difference          | normalized difference                                                     |     |   |
|                                  | Cosine similarity            | selected regions                                                          |     |   |
|                                  | Alignment                    | compatible tensors                                                        |     |   |
|                                  | Linked camera                | synchronized navigation                                                   |     |   |
|                                  | Linked selection             | selection mirrored                                                        |     |   |
| **Matrix Operation Interaction** | Operand selection            | select A/B                                                                |     |   |
|                                  | Output selection             | inspect C                                                                 |     |   |
|                                  | Dependency highlighting      | selecting result reveals contributors                                     |     |   |
|                                  | Matmul tracing               | `C[i,j] → A[i,:] × B[:,j]`                                                |     |   |
|                                  | Step execution               | individual multiply/accumulate                                            |     |   |
|                                  | Play/Pause                   | operation animation                                                       |     |   |
|                                  | Previous/Next                | deterministic step navigation                                             |     |   |
|                                  | Running sum                  | accumulator display                                                       |     |   |
| **Annotation**                   | Pin                          | bookmark tensor location                                                  |     |   |
|                                  | Note                         | attach explanation                                                        |     |   |
|                                  | Region annotation            | comment on selection                                                      |     |   |
|                                  | Issue marker                 | numerical/model problem                                                   |     |   |
|                                  | Tags                         | semantic labels                                                           |     |   |
|                                  | Share location               | URL containing model/view/selection                                       |     |   |
| **Keyboard Interaction**         | Shortcut map                 | Blender-like controls                                                     |     |   |
|                                  | Modal tools                  | select/slice/measure/etc.                                                 |     |   |
|                                  | Repeat last action           | efficient repeated operations                                             |     |   |
|                                  | Numeric input                | exact transform values                                                    |     |   |
| **AI Interaction**               | Ask about selection          | selected tensor becomes context                                           |     |   |
|                                  | Explain tensor               | semantic explanation                                                      |     |   |
|                                  | Transform selection          | natural-language → operation plan                                         |     |   |
|                                  | Compare selection            | explain differences                                                       |     |   |
|                                  | Diagnose                     | find anomalies                                                            |     |   |
|                                  | Generate WeightQL            | from natural language                                                     |     |   |
|                                  | Highlight AI result          | results appear in viewport                                                |     |   |
| **Validation/Safety**            | Shape validation             | reject invalid transform                                                  |     |   |
|                                  | dtype validation             | conversion constraints                                                    |     |   |
|                                  | semantic validation          | module compatibility                                                      |     |   |
|                                  | NaN/Inf detection            | before export                                                             |     |   |
|                                  | VRAM/RAM estimate            | before execution                                                          |     |   |
|                                  | I/O estimate                 | checkpoint operation cost                                                 |     |   |
|                                  | Preview                      | visualize prospective edit                                                |     |   |
|                                  | Dry Run                      | validate operation graph                                                  |     |   |
|                                  | Execute                      | materialize result                                                        |     |   |
|                                  | Export                       | write new checkpoint                                                      |     |   |

This gives Quatricmorph something much closer to an **interaction language for tensors** than a normal visualization application.

---

# 2. The most important abstraction: Selection

I would make **Selection** one of the deepest primitives in Quatricmorph.

Instead of representing selection as simply:

```ts
selectedObjectId
```

use something closer to:

```ts
type TensorSelection =
  | ModelSelection
  | LayerSelection
  | ModuleSelection
  | TensorSelection
  | BlockSelection
  | SliceSelection
  | ScalarSelection
  | QuerySelection
  | SemanticSelection;
```

A selection should contain approximately:

```ts
interface Selection {
  artifactId: string;

  tensorPath?: string;

  level:
    | "model"
    | "layer"
    | "module"
    | "tensor"
    | "block"
    | "slice"
    | "scalar";

  ranges?: TensorRange[];

  logicalIndices?: bigint[];

  query?: WeightQLExpression;

  exactness: "exact" | "sampled" | "approximate";

  source: "viewport" | "outliner" | "query" | "ai" | "script";

  statistics?: SelectionStatistics;
}
```

Then **everything consumes a Selection**:

```text
                         ┌─ Inspector
                         ├─ Statistics
                         ├─ Histogram
                         ├─ Diff
Viewport ─┐              ├─ Morph
Outliner ─┼─ Selection ──┼─ Transform
WeightQL ─┤              ├─ Export
AI ───────┤              ├─ Validation
Script ───┘              └─ AI context
```

This architecture will become extremely valuable later.

---

# 3. Blender concepts that Quatricmorph should copy directly

There are several Blender interaction concepts worth preserving almost literally.

### Object Mode → Tensor Mode

Blender:

```text
Object Mode
Edit Mode
Sculpt Mode
Weight Paint
```

Quatricmorph could eventually have:

```text
Model Mode
Tensor Mode
Inspect Mode
Compare Mode
Morph Mode
Trace Mode
```

But **selection level should remain independent from workspace mode**.

For example:

```text
Morph Mode
    selected:
        layer.28.self_attn.q_proj.weight
        block[8:12, 4:8]
```

---

# 4. Blender's Active Object distinction is especially useful

Suppose the user selects:

```text
W1
W2
W3
```

Quatricmorph should distinguish:

```text
Selected:
W1
W2

Active:
W3
```

The active tensor can be:

* transform target
* comparison reference
* morph destination
* inspector subject
* alignment reference

This will make multi-tensor operations much more predictable.

---

# 5. Highlighting should communicate semantics, not merely selection

Consider:

```text
C = A @ B
```

When hovering:

```text
C[4, 7]
```

Quatricmorph should highlight:

```text
A[4, :]
       ×
B[:, 7]
       ↓
C[4, 7]
```

Clicking makes this relation persistent.

Likewise, selecting:

```text
layers.18.self_attn.q_proj.weight
```

could subtly show:

```text
input
  ↓
Q projection
  ↓
Q
  ↓
attention
```

This is something Blender does not have, and it could become one of Quatricmorph's strongest differentiators.

For the initial matrix-multiplication experience, this should exactly preserve your existing `A[I,K] @ B[K,J] → C[I,J]` spatial convention and deterministic Play/Pause/Step tracing.

---

# 6. Semantic Zoom is probably more important than normal camera zoom

A trillion-parameter model cannot be represented as billions of individually rendered objects.

Zoom should therefore change **representation**:

```text
Far
│
├── Model
│
├── Transformer blocks
│
├── Layer
│
├── Module
│
├── Tensor
│
├── Tensor tiles
│
├── Matrix regions
│
├── Rows / columns
│
└── Scalars
    Near
```

For example, the same tensor could progressively become:

```text
[Tensor bounding box]
        ↓
[256 statistical tiles]
        ↓
[4096 tiles]
        ↓
[heatmap]
        ↓
[individual values]
```

This fits Quatricmorph's existing multiresolution tile architecture: the renderer should compute positions procedurally from logical indices/tile origins, rather than creating one scene object per weight.

---

# 7. Selection and rendering must be decoupled

This is particularly important.

You might only render:

```text
~100K visual primitives
```

while the tensor contains:

```text
5,505,024 values
```

or a model contains:

```text
1T parameters
```

Therefore:

```text
Logical Tensor World
         │
         ▼
Spatial Layout
         │
         ▼
LOD / Tile Resolver
         │
         ▼
Visualization
         │
mouse
         ▼
Picking
         │
         ▼
Logical Selection
```

The selection should refer to:

```text
tensor path
+
logical coordinates
+
range
```

—not to a Three.js mesh UUID.

This is one of the architectural decisions I would lock in very early.

---

# 8. Tensor Cursor

I would also adapt Blender's **3D Cursor** into a **Tensor Cursor**.

Example:

```text
Tensor Cursor

Tensor:
model.layers.41.mlp.down_proj.weight

Coordinate:
[1772, 892]

Value:
-0.0184326

Block:
[6, 3]

World:
(12.42, 4.81, -7.25)
```

The cursor can act as:

```text
navigation target
inspection point
measurement origin
transform pivot
slice origin
comparison coordinate
AI context anchor
```

For rank-N tensors, this becomes even more useful.

---

# 9. Outliner + Viewport + Inspector must be one synchronized system

Selecting from any surface should update every other surface:

```text
                    ┌──────────────┐
                    │   Outliner   │
                    └──────┬───────┘
                           │
                           ▼
┌──────────────┐     Selection      ┌──────────────┐
│    WeightQL  │◄────────┼────────►│   Inspector  │
└──────────────┘          │          └──────────────┘
                          │
                          ▼
                   ┌──────────────┐
                   │   Viewport   │
                   └──────────────┘
                          │
                          ▼
                   ┌──────────────┐
                   │      AI      │
                   └──────────────┘
```

If I click a scalar in the viewport, the Outliner should automatically reveal its tensor.

If I click:

```text
model.layers.27.self_attn.v_proj.weight
```

in the Outliner, the viewport should optionally frame it.

If AI says:

> The largest divergence occurs in layers 17–24 of `o_proj`.

those regions should become actual selections/highlights.

---

# 10. Editing should not behave like Blender mesh editing internally

The **interaction UX** can resemble Blender, but Quatricmorph should not mutate checkpoint memory directly.

Your existing immutable checkpoint architecture suggests:

```text
Original SafeTensors
        │
        ▼
Selection
        │
        ▼
Operation
        │
        ▼
Delta / Virtual Tensor
        │
        ▼
Preview
        │
        ▼
Validation
        │
        ▼
Materialize
        │
        ▼
New SafeTensors
```

For example:

```text
Select q_proj block
        ↓
Scale × 0.92
        ↓
Transform node created
        ↓
Preview visualization
        ↓
Validate
        ↓
Export derived model
```

not:

```text
pointer → mutate raw checkpoint bytes
```

This preserves the existing `propose → preview/dry-run → validate → execute/export` philosophy.

---

# 11. Recommended Viewport Toolbar

I would keep the visible primary tools surprisingly small:

```text
┌────────────────────────────────────────────────────────┐
│ Select │ Cursor │ Slice │ Inspect │ Measure │ Transform│
└────────────────────────────────────────────────────────┘
```

Then selection gets submodes:

```text
Select
 ├ Tensor
 ├ Block
 ├ Slice
 ├ Row
 ├ Column
 ├ Scalar
 ├ Box
 ├ Lasso
 └ Query
```

This prevents Quatricmorph from becoming visually overwhelming.

---

# 12. Tensor-specific equivalent of Blender selection

A useful mapping is:

| Blender        | Quatricmorph               |
| -------------- | -------------------------- |
| Scene          | Model                      |
| Collection     | Layer group                |
| Object         | Tensor                     |
| Mesh           | Tensor data                |
| Vertex group   | Tensor region              |
| Vertex         | Scalar                     |
| Face selection | Block selection            |
| Edge loop      | Row/column/channel         |
| Edit Mode      | Tensor Edit                |
| Modifier       | Tensor Transform           |
| 3D Cursor      | Tensor Cursor              |
| Outliner       | Model/Tensor Outliner      |
| Properties     | Tensor Inspector           |
| Local View     | Isolate Tensor             |
| X-Ray          | Select through tensors     |
| Weight Paint   | Tensor heatmap             |
| Geometry Nodes | Morph/Transform Graph      |
| Shader Editor  | Computation Graph          |
| Timeline       | Operation/runtime trace    |
| Dope Sheet     | Parameter/history timeline |

This gives Quatricmorph a coherent interaction vocabulary instead of inventing every UX convention from scratch.

---

# 13. Implementation architecture

I would split the implementation into these major subsystems:

```text
@quatricmorph/interaction
    SelectionManager
    ActiveEntityManager
    HoverManager
    PickingManager
    TensorCursor
    InteractionModeManager
    ShortcutManager

@quatricmorph/navigation
    CameraController
    SemanticZoomController
    FocusController
    NavigationHistory

@quatricmorph/spatial
    TensorLayout
    TensorCoordinateSystem
    SpatialIndex
    TensorAddressResolver

@quatricmorph/render
    TileRenderer
    HighlightRenderer
    SelectionOverlayRenderer
    DiffRenderer
    HeatmapRenderer
    LODManager

@quatricmorph/query
    WeightQL
    SelectionQuery
    StatisticsQuery
    TileQuery

@quatricmorph/inspect
    TensorInspector
    ScalarInspector
    StatisticsInspector
    ProvenanceInspector

@quatricmorph/edit
    TransformStack
    DeltaStore
    TensorOperation
    PreviewEngine
    UndoManager

@quatricmorph/compare
    AlignmentEngine
    DiffEngine
    LinkedSelection
    LinkedCamera

@quatricmorph/annotation
    Pins
    Notes
    Regions

@quatricmorph/ai
    SelectionContext
    ExplainSelection
    QueryGeneration
    TransformPlanner
```

The renderer remains merely one consumer of these systems.

---

# 14. What I would implement first

The dependency order matters considerably:

```text
P0 — Interaction foundation

TensorAddress
    ↓
TensorCoordinateSystem
    ↓
LOD/Tile mapping
    ↓
GPU Picking
    ↓
Hover
    ↓
SelectionManager
    ↓
HighlightRenderer
    ↓
Camera focus
```

Then:

```text
P1 — Blender-quality interaction

Box/Lasso selection
Active selection
Multi-selection
Hide/Isolate
Tensor Cursor
Outliner synchronization
Inspector
Semantic Zoom
Selection history
```

Then:

```text
P2 — Tensor-native interaction

Slice selection
Row/column selection
Query selection
Statistical selection
Relationship highlighting
Matmul tracing
Diff selection
Semantic selection
```

Then:

```text
P3 — Editing

Transform stack
Delta representation
Preview
Undo/Redo
Value editing
Scale/Add/Clamp/Zero
Copy/Replace
Morph
Validation
```

Then:

```text
P4 — Model engineering

Checkpoint compare
Morph graph
Virtual models
Runtime activation inspection
Intervention
Lineage
AI-assisted operations
```

---

# 15. The core interaction loop

Ultimately almost every action in Quatricmorph should follow:

```text
NAVIGATE
   ↓
HOVER
   ↓
SELECT
   ↓
HIGHLIGHT
   ↓
INSPECT
   ↓
QUERY
   ↓
TRANSFORM
   ↓
PREVIEW
   ↓
VALIDATE
   ↓
MATERIALIZE
```

This is the tensor equivalent of Blender's:

```text
Navigate → Select → Edit → Inspect → Render
```

And I would define **six absolutely foundational primitives** underneath nearly every future Quatricmorph feature:

**`TensorAddress` → `TensorSelection` → `TensorCursor` → `TensorView` → `TensorOperation` → `TensorDelta`.**

If these six abstractions are designed correctly, features such as diff, morphing, WeightQL, AI selection, tensor editing, model comparison, GPU visualization, and eventually activation/runtime debugging can all share the same interaction substrate rather than becoming separate UI systems.
