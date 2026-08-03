# Transform `mm` into the First Quatricmorph MVP

## Role

Act as a senior graphics engineer, mathematical-visualization engineer, and product-focused frontend architect.

Transform the existing `./mm` source code into the first working MVP of **Quatricmorph**.

Repository: ./mm
Do not rebuild the application blindly. First inspect the entire repository, understand the existing rendering and matrix-multiplication architecture, and then refactor the smallest amount of code necessary to produce a coherent Quatricmorph MVP.

---

# Product Vision

Quatricmorph is a browser-based spatial visualization environment for understanding tensor and matrix operations.

The first MVP must focus exclusively on:

```text
A @ B = C
```

It must visualize simple matrix multiplication in a structured 3D coordinate system.

Every matrix, vector, scalar, value, label, and multiplication guide must align to a shared **3D margin grid**, similar to how handwriting is constrained and aligned by ruled or graph-paper margins.

The grid is not merely a decorative background. It is the spatial layout system for the entire visualization.

---

# First-MVP Scope

Support only one multiplication expression at a time:

```text
A @ B = C
```

Support these shape combinations:

```text
Matrix @ Matrix → Matrix
Matrix @ Column Vector → Column Vector
Row Vector @ Matrix → Row Vector
Row Vector @ Column Vector → Scalar
```

Represent the tensor types consistently:

* Matrix: `m × n`
* Column vector: `n × 1`
* Row vector: `1 × n`
* Scalar: `1 × 1`

Infer the result type from its shape. Do not create unrelated rendering rules for vectors or scalars. They must use the same grid-cell, frame, alignment, value, and interaction systems as matrices.

---

# Explicitly Out of Scope

Do not implement the following in this MVP:

* Attention-head visualization
* Q, K, V, or softmax pipelines
* LoRA visualization
* Nested matrix expressions
* Transformer blocks
* MLP visualizations
* Tensor broadcasting
* Batched multiplication
* Sparse tensor support
* PyTorch model loading
* Notebook integration
* Remote dataset loading
* Model-weight import
* Training visualization
* Automatic differentiation
* Gradient visualization
* Collaborative editing
* User accounts
* Backend services
* Desktop or mobile applications

Existing advanced functionality may remain internally when removing it would destabilize the application, but it must not appear in the Quatricmorph MVP interface.

---

# Phase 1: Repository Analysis

Before modifying code:

1. Read the complete repository.
2. Identify the purpose and dependencies of:

   * `index.html`
   * `viz.js`
   * `gui.js`
   * `util.js`
   * `assets/`
   * `examples/`
   * `lib/`
3. Trace the complete lifecycle:

   * Parameter creation
   * Matrix initialization
   * Matrix multiplication
   * Three.js object creation
   * Layout calculation
   * Camera initialization
   * Animation
   * Hover and selection
   * GUI updates
   * URL serialization
   * Cleanup and disposal
4. Locate the existing implementations of:

   * `Array2D`
   * `Mat`
   * `MatMul`
   * Matrix placement
   * Row and flow guides
   * Value-to-size mapping
   * Value-to-color mapping
   * Text labels
   * Orbit controls
   * Shareable URL state
5. Write a concise implementation plan before editing.
6. Preserve working behavior unless it conflicts with the new MVP specification.

Do not assume that the current architecture is clean. Separate reusable behavior from experimental or research-oriented behavior where necessary.

---

# Core Mathematical Model

Use standard matrix multiplication:

```text
A ∈ R^(m×k)
B ∈ R^(k×n)
C = A @ B
C ∈ R^(m×n)
```

Each output value is:

```text
C[i,j] = Σ A[i,k] × B[k,j]
```

Validate dimensions before rendering:

```text
A.columns === B.rows
```

When dimensions are invalid:

* Do not attempt multiplication.
* Do not leave partially initialized Three.js objects.
* Show a clear validation message.
* Explain the expected relationship between the dimensions.
* Preserve the last valid scene until valid input is provided, or render an intentional empty state.

Use deterministic floating-point calculations for the same input values.

---

# 3D Margin Grid

## Concept

Build a reusable spatial component named conceptually:

```text
MarginGrid3D
```

The grid must behave like graph paper extended into three dimensions.

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

All visual objects must derive their positions from this coordinate system.

Do not scatter objects using independent offsets or arbitrary magic numbers.

---

## Coordinate System

Use mathematically meaningful global axes:

```text
I = output-row dimension
J = output-column dimension
K = contraction dimension
```

A recommended world mapping is:

```text
World X → J
World Y → I
World Z → K
```

Place the tensor planes consistently:

```text
A uses the I × K plane
B uses the K × J plane
C uses the I × J plane
```

The three tensor planes should form a visually understandable multiplication volume or spatial corner.

The shared dimensions must align exactly:

* The `I` dimension of `A` aligns with the `I` dimension of `C`.
* The `K` dimension of `A` aligns with the `K` dimension of `B`.
* The `J` dimension of `B` aligns with the `J` dimension of `C`.

Use the existing `mm` spatial model where it already represents these relationships correctly. Refactor its placement calculations so they are explicitly derived from the shared margin-grid coordinate system.

---

## Tensor Margin Frame

Create a reusable component named conceptually:

```text
TensorMarginFrame
```

Every matrix, vector, and scalar must have:

* An outer boundary frame
* A consistent inner margin
* A title margin
* Row and column guide lines
* One grid cell per tensor value
* Shape labels
* Axis labels where relevant
* A deterministic anchor point
* A deterministic orientation
* A bounding box that can be used for camera fitting

The title should appear in the title margin:

```text
A
B
C
```

The shape should be visible near the title:

```text
A [2 × 3]
B [3 × 2]
C [2 × 2]
```

A column vector must still occupy a framed `n × 1` grid.

A row vector must still occupy a framed `1 × n` grid.

A scalar must occupy a framed `1 × 1` grid and must never float outside the shared layout.

---

## Alignment Rules

All objects must snap to grid-derived coordinates.

Required invariants:

```text
position.x % cellSize === 0
position.y % cellSize === 0
position.z % cellSize === 0
```

Allow small floating-point tolerance in tests.

The following must remain aligned after any input change:

* Tensor cells
* Tensor frames
* Titles
* Shape labels
* Axis labels
* Multiplication guides
* Highlight paths
* Result cells
* Camera-fit bounds

Do not calculate label positions directly from viewport pixels. Labels belong to their tensor frames and must move with them.

---

# Value Visualization

Reuse the existing value-to-color and value-to-size logic where practical.

For each tensor value:

* Position it at the center of its grid cell.
* Preserve a visible empty cell even when the value is zero.
* Use color to distinguish positive, negative, and zero values.
* Use marker size, height, or depth to communicate magnitude.
* Keep numerical text readable when labels are enabled.
* Prevent markers from crossing their cell boundaries under normal settings.

For the first MVP, reusing the existing point-sprite renderer is acceptable.

Do not spend the MVP rewriting the complete rendering system into meshes unless the existing point renderer prevents correct cell alignment.

If practical without destabilizing the application, use instanced cube or voxel markers as an optional Quatricmorph visual style. The margin-grid architecture is more important than the primitive shape.

---

# Basic Multiplication Interaction

Provide a minimal but educational multiplication animation.

For an output cell `C[i,j]`:

1. Highlight row `i` of `A`.
2. Highlight column `j` of `B`.
3. Highlight their shared `K` positions.
4. Show each pair:

```text
A[i,k] × B[k,j]
```

5. Show the running sum.
6. Write or reveal the final value in `C[i,j]`.
7. Advance to the next output cell.

Provide these controls:

```text
Play
Pause
Step
Previous Step
Reset Calculation
Reset View
Fit View
```

The first MVP only needs one clear algorithm:

```text
output-cell dot product
```

Hide the existing advanced algorithm selector unless it is required internally.

Animation state must remain separate from matrix data and scene-layout state.

---

# Hover and Selection

Hovering over a value must show:

```text
Tensor: A
Index: [i, k]
Value: 1.25
Shape: [m, k]
```

Use the correct indices for `B` and `C`.

Clicking a result cell should select its multiplication path and highlight:

```text
A[i, :]
B[:, j]
C[i, j]
```

The selected state must remain visible until:

* Another cell is selected
* The calculation advances
* The user clears the selection
* The matrices are regenerated

Do not rely only on color. Also use brightness, scale, outline, guide thickness, or opacity.

---

# MVP User Interface

Replace the research-oriented `mm` interface with a simple Quatricmorph interface.

## Header

Display:

```text
Quatricmorph
Spatial Matrix Multiplication
```

Do not present the product as `mm`.

Retain the original license and attribution requirements in the repository and documentation.

---

## Input Panel

Provide controls for:

```text
A rows
A columns
B rows
B columns
A values
B values
```

Automatically synchronize:

```text
B rows = A columns
```

Allow the user to unlock dimensions only when useful, while still validating the operation.

Provide simple presets:

```text
Random
Identity
Sequential
Zeros
Ones
Small Example
```

Use compact editable grids or structured text input for values.

An acceptable textual format is:

```text
1, 2, 3
4, 5, 6
```

Reject malformed rows clearly.

---

## Visualization Controls

Expose only controls needed for the MVP:

```text
Show values
Show tensor frames
Show minor grid
Show major grid
Show axis labels
Show multiplication guides
Animation speed
Marker scale
Grid cell size
Camera preset
```

Camera presets:

```text
Isometric
Front
Top
Multiplication Volume
```

Keep `OrbitControls`.

Do not expose internal diagnostic parameters in the default UI.

---

## Shareable State

Preserve the existing shareable-link capability.

The URL should serialize only necessary product state:

```text
A shape
B shape
A values
B values
Grid settings
Display settings
Camera preset or camera position
Selected output cell
```

Do not serialize transient animation timers or disposable Three.js objects.

Provide a visible:

```text
Copy Share Link
```

button.

Large state may use the repository’s existing compression mechanism.

Opening a shared URL must reproduce the same matrices and spatial layout.

---

# Architecture

Preserve existing working modules where possible, but separate the new concepts clearly.

A preferred conceptual architecture is:

```text
src/
  math/
    tensor.js
    matmul.js
    validation.js

  scene/
    scene-context.js
    margin-grid-3d.js
    tensor-margin-frame.js
    tensor-renderer.js
    matmul-layout.js
    multiplication-guides.js
    camera-controller.js

  interaction/
    hover-controller.js
    selection-controller.js
    animation-controller.js

  state/
    app-state.js
    share-state.js

  ui/
    controls.js
    matrix-input.js
    validation-message.js

  main.js
```

This structure is guidance, not permission to perform an unnecessary complete rewrite.

When practical:

* Extract reusable logic from `viz.js`.
* Keep pure matrix operations independent from Three.js.
* Keep layout calculations independent from GUI code.
* Keep URL serialization independent from rendering.
* Keep animation state independent from matrix data.
* Keep disposal logic next to the Three.js resources it owns.

Avoid circular dependencies.

---

# Tooling

Do not introduce React, Svelte, Vue, Electron, Tauri, or a backend.

The MVP should remain a lightweight browser application.

You may introduce Vite as a minimal development and build shell when it materially improves:

* Local development
* Module organization
* Testing
* Production builds
* Deployment

Do not introduce Vite merely to rewrite working code.

Continue using:

```text
Three.js
OrbitControls
```

Continue using `lil-gui` only if it can support the simplified interface cleanly. A small native HTML control panel is also acceptable.

---

# Design Direction

The visual style should communicate:

```text
Mathematical
Spatial
Precise
Minimal
Technical
Calm
```

Use a dark neutral workspace by default.

Recommended visual hierarchy:

* Subtle minor grid lines
* Stronger major grid lines
* Clear tensor boundary frames
* Distinct input and output tensors
* High-contrast active multiplication guides
* Readable text labels
* Minimal visual noise

The 3D margin grid should look intentional and architectural, not like an infinite game-world grid.

Avoid:

* Excessive glow
* Random neon colors
* Heavy gradients
* Unnecessary shadows
* Floating controls inside the 3D scene
* Decorative particles
* Dense diagnostic panels

---

# Default Example

The application should start with:

```text
A = [
  [1, 2, 3],
  [4, 5, 6]
]

B = [
  [7, 8],
  [9, 10],
  [11, 12]
]
```

The expected result is:

```text
C = [
  [58, 64],
  [139, 154]
]
```

The initial camera must show all three framed tensor planes and their shared `I`, `J`, and `K` alignment.

---

# Required Test Cases

## Mathematical Tests

Verify:

```text
2×3 @ 3×2 → 2×2
3×3 @ 3×1 → 3×1
1×3 @ 3×2 → 1×2
1×3 @ 3×1 → 1×1
1×1 @ 1×1 → 1×1
```

Verify invalid input:

```text
2×3 @ 2×2 → validation error
```

Test negative values, zero values, decimals, and values larger than one.

---

## Layout Tests

Verify that:

* Every tensor value maps to exactly one grid cell.
* Vectors use the same coordinate system as matrices.
* Scalars use a single framed cell.
* Shared `I`, `J`, and `K` dimensions align.
* Tensor frames contain all their values.
* Labels stay attached after camera movement.
* Grid alignment remains stable after changing dimensions.
* Camera fitting includes frames, titles, and axis labels.
* Resizing does not break layout.
* Reinitialization does not leave duplicate scene objects.

---

## Interaction Tests

Verify:

* Hover metadata is correct.
* Selecting `C[i,j]` highlights the correct row and column.
* Step advances deterministically.
* Reset restores the initial multiplication state.
* Reset View restores the intended camera.
* Share links reconstruct the same scene.
* Invalid URL state falls back safely.
* Repeated reinitialization does not continuously increase renderer memory.

---

# Performance Target

The MVP should remain interactive for matrices up to at least:

```text
32 × 32
```

Target:

* Smooth orbiting on a normal desktop browser
* No obvious memory leak after repeated matrix changes
* No complete page reload when matrix values change
* No unnecessary recreation of static grid geometry
* Reuse materials and geometries where possible
* Dispose replaced geometries, materials, textures, and labels correctly

Do not optimize prematurely for massive tensors.

---

# Documentation

Update `README.md` with:

1. Quatricmorph MVP description
2. Current scope
3. Screenshot or preview
4. Local development instructions
5. Production build instructions
6. Matrix input format
7. Supported operations
8. Shareable-link behavior
9. Current limitations
10. Attribution to the original `mm` project
11. Original and resulting license information

Also create:

```text
docs/ARCHITECTURE.md
```

Document:

* Source architecture
* Mathematical data flow
* Scene graph
* Margin-grid coordinate system
* Tensor-plane placement
* Animation state machine
* URL-state format
* Resource ownership and disposal
* Extension points for future Quatricmorph versions

Add a diagram showing:

```text
User Input
    ↓
Validation
    ↓
Tensor Data
    ↓
Matrix Multiplication
    ↓
Margin-Grid Layout
    ↓
Three.js Scene Objects
    ↓
Interaction and Animation
    ↓
Renderer
```

---

# Implementation Constraints

* Do not change correct matrix multiplication behavior unnecessarily.
* Do not remove the original license.
* Do not claim that Quatricmorph is an official PyTorch product.
* Do not leave the UI partially branded as `mm`.
* Do not leave attention or LoRA controls visible.
* Do not use hard-coded positions for individual matrix shapes.
* Do not duplicate math logic inside rendering classes.
* Do not use `eval` for new matrix input functionality.
* Do not block the browser with synchronous remote data requests.
* Do not introduce a backend.
* Do not add speculative future features.
* Do not mark work complete while tests or runtime errors remain.

---

# Acceptance Criteria

The implementation is complete only when:

1. The application is branded as Quatricmorph.
2. A user can enter two compatible matrices.
3. The result is computed correctly.
4. Matrix, vector, and scalar shapes render correctly.
5. All tensors align within the shared 3D margin grid.
6. The `I`, `J`, and `K` dimensions are spatially consistent.
7. A user can orbit, reset, and fit the camera.
8. A user can hover values and inspect indices.
9. A user can select an output cell and see its row-column multiplication path.
10. Play, pause, step, and reset work deterministically.
11. Shareable links restore the same visualization.
12. Invalid dimensions produce a clear error.
13. Advanced attention, LoRA, and nested-expression features are absent from the MVP interface.
14. The browser console contains no unresolved runtime errors.
15. Repeated input changes do not create obvious memory leaks.
16. Mathematical, layout, and interaction tests pass.
17. The README and architecture documentation describe the implemented system accurately.

---

# Final Output

After implementation, provide:

```text
1. Summary of the original architecture
2. Summary of the implemented Quatricmorph architecture
3. Files added
4. Files modified
5. Files removed or deprecated
6. Important architectural decisions
7. Mathematical cases tested
8. UI and interaction cases tested
9. Performance observations
10. Known MVP limitations
11. Commands used to run and verify the application
12. Recommended next step for MVP 2
```

Do not report features that were not actually implemented or verified.

The final result must be a focused, functional first Quatricmorph MVP—not a roadmap, mockup, or partial rebranding of `mm`.
