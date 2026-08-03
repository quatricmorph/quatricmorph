# Generate the Quatricmorph Technical Requirements Document

Act as a principal software architect, technical product manager, computer-graphics engineer, and mathematical-visualization engineer.

Your task is to analyze the existing Quatricmorph project and produce a complete implementation-grade Technical Requirements Document.

Quatricmorph is being developed by transforming the existing `bhosmer/mm` matrix-multiplication visualizer into a standalone product.

Source repository:

```text
https://github.com/bhosmer/mm
```

The document must describe the technical requirements for the **first Quatricmorph MVP**, not the complete long-term product vision.

---

# Primary Objective

Produce the following file:

```text
docs/TECHNICAL_REQUIREMENTS.md
```

The document must define exactly what engineers and autonomous coding agents need to build, test, verify, and maintain the first Quatricmorph MVP.

It must convert the current product vision into:

* Functional requirements
* Non-functional requirements
* Mathematical requirements
* Rendering requirements
* Spatial-layout requirements
* Interaction requirements
* State-management requirements
* Performance requirements
* Testing requirements
* Security requirements
* Compatibility requirements
* Documentation requirements
* Acceptance criteria

Do not implement the product in this task.

Do not write a generic requirements template. Analyze the current source code and produce requirements grounded in the actual repository.

---

# Product Context

Quatricmorph is a browser-based 3D mathematical-visualization application.

The first MVP focuses exclusively on visualizing:

```text
A @ B = C
```

It must support:

```text
Matrix @ Matrix → Matrix
Matrix @ Column Vector → Column Vector
Row Vector @ Matrix → Row Vector
Row Vector @ Column Vector → Scalar
```

The defining feature of the MVP is shared **3D grid ruled lines**.

Every matrix, vector, scalar, tensor frame, value marker, label, multiplication guide, and result must align to this shared spatial grid.

The design concept is similar to handwriting aligned using ruled margins or graph paper, but extended into three-dimensional mathematical space.

---

# Required Analysis Before Writing

Before producing the document:

1. Read the entire repository.
2. Inspect all source files, configuration files, examples, assets, dependencies, build scripts, and documentation.
3. Identify the current application architecture.
4. Identify the current rendering stack.
5. Identify the current mathematical model.
6. Identify how matrix data is represented.
7. Identify how matrix multiplication is calculated.
8. Identify how Three.js scene objects are created and disposed.
9. Identify how matrices are positioned in 3D.
10. Identify how labels, hover interactions, guides, animations, and camera controls work.
11. Identify how URL state and shareable links work.
12. Identify experimental, unused, legacy, and advanced functionality.
13. Identify technical debt that could affect the MVP transformation.
14. Identify the existing license and attribution requirements.
15. Identify assumptions that cannot be verified directly from the code.

Do not invent repository behavior.

When information is uncertain, explicitly mark it as:

```text
Open Question
```

or:

```text
Assumption Requiring Verification
```

---

# Document Audience

Write the requirements for:

* Software engineers
* Computer-graphics engineers
* Frontend engineers
* QA engineers
* Technical reviewers
* Autonomous coding agents
* Future maintainers

The document must be precise enough that different engineers can independently implement compatible solutions.

Avoid vague requirements such as:

```text
The application should be fast.
```

Use measurable requirements such as:

```text
The application shall maintain interactive camera navigation for matrices up to 32 × 32 on a representative desktop browser.
```

---

# Required Document Structure

Use the following structure.

---

## 1. Document Control

Include:

* Document title
* Product name
* MVP version
* Document status
* Intended audience
* Source repository
* Last updated date
* Requirement ownership
* Related documents
* Change-history table

Use placeholders where ownership or dates cannot be determined.

---

## 2. Purpose

Explain:

* Why this document exists
* What decisions it governs
* What implementation phase it covers
* How engineers and coding agents should use it
* What is explicitly outside its authority

---

## 3. Product Summary

Describe:

* Quatricmorph
* The first MVP
* The source project being transformed
* The value of spatial matrix visualization
* The role of the 3D grid ruled lines
* The primary user workflow
* The intended learning and inspection use cases

Keep this section technical and concise.

---

## 4. Goals

Define measurable MVP goals.

At minimum include:

* Correct matrix multiplication
* Spatially consistent visualization
* Matrix, vector, and scalar support
* Shared 3D grid-ruled-lines alignment
* Interactive calculation exploration
* Deterministic animation
* Shareable application state
* Lightweight browser deployment
* Maintainable architecture
* Verifiable behavior

---

## 5. Non-Goals

Explicitly exclude:

* Attention visualization
* Transformer visualization
* LoRA visualization
* Batched tensors
* Broadcasting
* Automatic differentiation
* Gradient visualization
* Training visualization
* PyTorch runtime integration
* Notebook runtime integration
* Model loading
* Remote tensor storage
* User accounts
* Collaboration
* Backend services
* Desktop application
* Mobile application
* Plugin architecture
* Large-scale tensor optimization
* Production analytics
* AI-generated explanations

Existing internal code may remain temporarily when removing it would create unnecessary implementation risk, but excluded functionality must not appear in the MVP user interface.

---

## 6. Stakeholders and User Types

Define relevant users, such as:

* Student
* Educator
* Machine-learning engineer
* Researcher
* Technical reviewer
* Maintainer

For each user type, describe:

* Primary goal
* Expected interaction
* Required technical capability
* MVP limitations relevant to that user

Do not invent enterprise personas for the first MVP.

---

## 7. User Workflows

Document the required workflows.

At minimum include:

### 7.1 Open the Application

The user opens the browser application and sees a valid default multiplication example.

### 7.2 Enter Matrix Values

The user changes matrix dimensions and values.

### 7.3 Validate Dimensions

The system checks whether:

```text
A.columns === B.rows
```

### 7.4 Compute the Result

The application computes:

```text
C = A @ B
```

### 7.5 Inspect the 3D Layout

The user orbits, zooms, pans, resets, and fits the view.

### 7.6 Inspect a Tensor Element

The user hovers over an element and sees its tensor name, index, value, and shape.

### 7.7 Inspect a Result Cell

The user selects `C[i,j]` and sees the corresponding row, column, element pairs, and accumulation path.

### 7.8 Animate Multiplication

The user plays, pauses, steps, reverses, or resets the multiplication animation.

### 7.9 Share a Visualization

The user generates a URL that restores the relevant application state.

### 7.10 Recover from Invalid Input

The system shows a clear validation state without corrupting the current Three.js scene.

For each workflow specify:

* Preconditions
* User actions
* System response
* Validation behavior
* Failure behavior
* Final state

---

## 8. Mathematical Requirements

Define the mathematical model precisely.

Include:

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

Define requirements for:

* Matrix shape validation
* Row vectors
* Column vectors
* Scalars
* Decimal values
* Negative values
* Zero values
* Large values within JavaScript numeric limits
* Floating-point behavior
* Deterministic output
* Input normalization
* Invalid numeric input
* Empty input
* Non-rectangular rows
* `NaN`
* Positive infinity
* Negative infinity

Specify whether unsupported values must be rejected, normalized, or displayed as validation errors.

Require that mathematical operations remain independent from Three.js rendering code.

---

## 9. Tensor Data Model

Define the canonical data model for:

* Matrix
* Row vector
* Column vector
* Scalar
* Tensor identifier
* Tensor shape
* Tensor values
* Element address
* Display metadata
* Selection metadata

Define whether the project should retain, refactor, or replace the existing `Array2D` abstraction.

Require that:

* Data shape is explicit.
* Data length matches the shape.
* Tensor values are stored in a deterministic ordering.
* Rendering code cannot mutate canonical mathematical data unexpectedly.
* Result data is derived from input data.
* Input tensors and result tensors are distinguishable.

Include proposed TypeScript or JavaScript interface examples where useful, but do not require a full TypeScript migration unless justified by repository analysis.

---

## 10. 3D Coordinate-System Requirements

Define the global mathematical coordinate system.

Use:

```text
I = output-row dimension
J = output-column dimension
K = contraction dimension
```

Recommended world mapping:

```text
World X → J
World Y → I
World Z → K
```

Define the tensor planes:

```text
A uses the I × K plane
B uses the K × J plane
C uses the I × J plane
```

Require exact alignment of shared dimensions:

* `A.I` with `C.I`
* `A.K` with `B.K`
* `B.J` with `C.J`

Specify:

* World origin
* Axis direction
* Handedness
* Cell units
* Tensor anchors
* Tensor orientation
* Frame offsets
* Label offsets
* Positive directions
* Grid snapping tolerance
* Camera up direction

Do not allow different modules to define conflicting coordinate conventions.

---

## 11. 3D Grid-Ruled-Lines Requirements

Define a reusable spatial-layout abstraction named conceptually:

```text
GridRuledLines3D
```

It must control:

```text
cellSize
minorGridSpacing
majorGridInterval
tensorPadding
framePadding
titleMargin
labelMargin
operandGap
depthSpacing
origin
```

Define requirements for:

* Minor grid lines
* Major grid lines
* Tensor boundaries
* Margin areas
* Label regions
* Cell centers
* Tensor-plane alignment
* Grid visibility
* Grid scaling
* Grid updates
* Grid reuse
* Grid geometry disposal
* Camera-fit bounds

Every visible mathematical object must derive its world position from the grid-ruled-lines system.

Prohibit arbitrary per-shape placement constants outside the layout subsystem.

Specify a floating-point alignment tolerance for tests.

---

## 12. Tensor Frame Requirements

Define a reusable component named conceptually:

```text
TensorMarginFrame
```

Each matrix, vector, and scalar frame must include:

* Tensor boundary
* Inner cell grid
* Title area
* Tensor name
* Tensor shape
* Row guides
* Column guides
* Axis association
* World-space bounding box
* Selection support
* Visibility control

Require that:

* Matrix frames support `m × n`.
* Column vectors render as `n × 1`.
* Row vectors render as `1 × n`.
* Scalars render as `1 × 1`.
* Labels move with frames.
* Frames participate in camera fitting.
* Frames update correctly when tensor shapes change.
* Old frame resources are disposed.

---

## 13. Value-Rendering Requirements

Define requirements for tensor value markers.

Each value must:

* Occupy exactly one grid cell.
* Be positioned at the cell center.
* Preserve a visible cell when its value is zero.
* Expose its index and value for interaction.
* Support positive, negative, and zero visual states.
* Support magnitude visualization.
* Respect cell boundaries.
* Remain readable under supported camera presets.

Define acceptable rendering approaches:

* Existing point sprites
* Instanced geometry
* Cubes or voxels
* Lightweight custom shaders

The MVP must prefer reuse of the existing renderer unless it prevents correct layout or interaction.

Define requirements for:

* Geometry reuse
* Material reuse
* Shader behavior
* Transparency
* Depth testing
* Marker scaling
* Label scaling
* Value normalization
* Color mapping
* Clamping
* Extreme values
* Empty or zero states

---

## 14. Multiplication-Guide Requirements

Define how the selected dot product is visualized.

For `C[i,j]`, the system must highlight:

```text
A[i, :]
B[:, j]
C[i, j]
```

It must communicate the relationship:

```text
A[i,k] × B[k,j]
```

for each contraction index `k`.

Define requirements for:

* Row highlights
* Column highlights
* Contraction-axis alignment
* Pairwise guides
* Running-sum display
* Final result display
* Highlight ordering
* Selected state
* Hover state
* Active animation state
* Completed state
* Inactive state

Do not rely only on hue differences.

Use at least one additional signal such as:

* Opacity
* Scale
* Outline
* Brightness
* Line thickness
* Motion

---

## 15. Animation Requirements

Define the multiplication animation state machine.

Suggested states:

```text
idle
selected
playing
paused
stepping
completed
resetting
```

Define required commands:

```text
Play
Pause
Step Forward
Step Backward
Reset Calculation
Select Output Cell
Clear Selection
```

Specify:

* Transition rules
* Deterministic step order
* Current output index
* Current contraction index
* Running sum
* Completed output cells
* Animation speed
* Cancellation behavior
* Input-change behavior
* Scene-rebuild behavior
* URL-restoration behavior

The same input and initial state must produce the same animation sequence.

Animation state must not mutate the canonical input matrices.

---

## 16. Interaction Requirements

Define requirements for:

### Hover

Display:

```text
Tensor
Index
Value
Shape
```

### Selection

Selecting a result cell activates its dot-product path.

### Camera

Support:

```text
Orbit
Zoom
Pan
Reset View
Fit View
```

### Camera Presets

Support:

```text
Isometric
Front
Top
Multiplication Volume
```

### Keyboard Interaction

Determine whether keyboard shortcuts are required for the MVP.

Possible shortcuts:

```text
Space → Play or Pause
Right Arrow → Step Forward
Left Arrow → Step Backward
R → Reset Calculation
F → Fit View
Escape → Clear Selection
```

Only include shortcuts that can be implemented consistently across supported browsers.

Specify pointer behavior, hover priority, selection priority, and behavior when UI panels overlap the canvas.

---

## 17. User-Interface Requirements

Define the required application areas.

### Header

Display:

```text
Quatricmorph
Spatial Matrix Multiplication
```

### Input Panel

Support:

* `A` row count
* `A` column count
* `B` row count
* `B` column count
* `A` values
* `B` values
* Presets
* Validation messages

### Visualization Panel

Contain the Three.js canvas.

### Calculation Controls

Support animation and selection controls.

### Display Controls

Support:

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

### Share Control

Support:

```text
Copy Share Link
```

Define responsive behavior for reasonable desktop viewport sizes.

The first MVP may prioritize desktop browsers.

Do not require full mobile usability.

---

## 18. Input and Validation Requirements

Define the accepted matrix-entry format.

Example:

```text
1, 2, 3
4, 5, 6
```

Specify:

* Row separators
* Column separators
* Whitespace handling
* Decimal syntax
* Negative values
* Empty cells
* Trailing separators
* Unequal row lengths
* Invalid tokens
* Minimum shape
* Maximum supported interactive shape
* Synchronization between dimensions and values
* Whether dimensions are inferred or explicitly controlled

Define clear validation messages.

Examples:

```text
Matrix A has inconsistent row lengths.
```

```text
Matrix A has 3 columns, but Matrix B has 2 rows.
```

```text
The value "abc" is not a valid number.
```

The renderer must not receive structurally invalid tensor data.

---

## 19. Default Example Requirements

The application must start with:

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

Expected result:

```text
C = [
  [58, 64],
  [139, 154]
]
```

The initial camera must display:

* `A`
* `B`
* `C`
* Shared `I`, `J`, and `K` dimensions
* Tensor frames
* Relevant labels
* The complete multiplication volume

---

## 20. Application-State Requirements

Define state categories:

```text
Mathematical State
Layout State
Display State
Interaction State
Animation State
Camera State
Shareable State
Transient Runtime State
```

Specify which state is canonical and which is derived.

Require:

* One authoritative application-state model
* Predictable state transitions
* No hidden state inside unrelated GUI controls
* No circular updates
* Safe restoration from URL state
* Safe fallback from invalid state
* Separation of serializable and non-serializable state

Three.js objects must never be serialized.

---

## 21. Shareable-Link Requirements

Preserve or refactor the existing URL-state mechanism.

The shared state should include:

```text
A shape
B shape
A values
B values
Grid settings
Display settings
Camera preset or camera transform
Selected output cell
```

Transient timers and renderer objects must not be included.

Define requirements for:

* Encoding
* Compression
* Versioning
* Backward compatibility
* Maximum practical URL size
* Invalid payload handling
* Missing field handling
* Default-value omission
* Security validation
* Copy-to-clipboard behavior
* Loading-state behavior

Include a state-schema version, such as:

```text
v=1
```

---

## 22. Scene-Lifecycle Requirements

Define the Three.js scene lifecycle.

Cover:

* Scene creation
* Renderer creation
* Camera creation
* OrbitControls creation
* Grid creation
* Tensor creation
* Guide creation
* Label creation
* Raycasting
* Animation loop
* Resize handling
* Scene rebuild
* Partial update
* Resource disposal
* Application shutdown

Require explicit ownership of:

* Geometries
* Materials
* Textures
* Render targets
* DOM labels
* Event listeners
* Timers
* Animation-frame handles

Repeated matrix changes must not leave duplicate objects or listeners.

---

## 23. Architecture Requirements

Describe the intended component boundaries.

A recommended conceptual structure is:

```text
src/
  math/
    tensor.js
    matmul.js
    validation.js

  layout/
    coordinate-system.js
    grid-ruled-lines-3d.js
    tensor-layout.js
    matmul-layout.js

  scene/
    scene-context.js
    tensor-renderer.js
    tensor-frame.js
    multiplication-guides.js
    camera-controller.js
    resource-manager.js

  interaction/
    hover-controller.js
    selection-controller.js
    animation-controller.js

  state/
    app-state.js
    share-state.js

  ui/
    matrix-input.js
    controls.js
    validation-view.js

  main.js
```

This is a conceptual target, not a mandate for unnecessary restructuring.

Define dependency rules:

* `math` must not depend on Three.js.
* `layout` may use mathematical data but must not depend on GUI components.
* `scene` may depend on `math` and `layout`.
* `interaction` may depend on scene identifiers and application state.
* `ui` may update application state but must not perform matrix multiplication directly.
* `share-state` must not serialize runtime scene objects.
* Resource ownership must be explicit.

---

## 24. Technology Requirements

Identify the actual technologies currently used by the repository.

Evaluate whether the MVP should retain or introduce:

* JavaScript modules
* TypeScript
* Three.js
* OrbitControls
* `lil-gui`
* Native HTML controls
* Vite
* A unit-test framework
* A browser-test framework
* ESLint
* Formatting tools

Do not recommend React, Vue, Svelte, Electron, Tauri, or a backend for the first MVP unless the repository analysis reveals a critical reason.

For each recommended technology change, document:

* Problem being solved
* Expected benefit
* Migration cost
* Risk
* Decision
* Whether it is required for MVP

---

## 25. Browser and Platform Requirements

Define supported environments.

At minimum consider:

* Latest stable Chrome
* Latest stable Edge
* Latest stable Firefox
* Latest stable Safari

Specify:

* WebGL requirements
* ES-module support
* Clipboard API fallback
* High-DPI rendering
* Window resizing
* Minimum viewport
* Hardware acceleration
* Touch behavior
* Unsupported browser behavior

The requirements must distinguish:

```text
Required Support
Best-Effort Support
Not Supported
```

---

## 26. Performance Requirements

Define measurable targets.

At minimum include:

* Interactive orbiting for matrices up to `32 × 32`
* No complete page reload when values change
* No unnecessary rebuild of static grid geometry
* Reuse of geometry and materials where practical
* Stable frame rendering for the default example
* Bounded memory growth after repeated matrix replacement
* Responsive input handling
* Controlled device-pixel ratio
* Reasonable initial bundle size
* Reasonable initial-load time

Define a representative performance-test environment using placeholders when exact hardware is not specified.

Separate:

* Functional maximum
* Recommended interactive maximum
* Stress-test maximum

Do not claim support for arbitrary tensor sizes.

---

## 27. Reliability Requirements

Require:

* Invalid data cannot crash the render loop.
* Failed parsing cannot corrupt the last valid scene.
* Scene updates must be recoverable.
* URL-state failures must fall back safely.
* Animation cancellation must leave a valid state.
* Window resize must not produce invalid camera projection.
* Missing assets must produce visible errors or safe fallbacks.
* Reinitialization must be idempotent.
* Repeated reset operations must produce the same result.

---

## 28. Security Requirements

Although the MVP is client-side, define requirements for:

* No use of `eval` for user matrix input
* Safe URL-state parsing
* Bounded input size
* Safe DOM insertion
* No execution of imported text
* No arbitrary remote script loading
* No synchronous arbitrary remote-data requests
* Dependency review
* Content Security Policy compatibility
* Clipboard API permission handling
* Prevention of prototype-pollution-style state merging
* Avoidance of secrets in the source repository

Review existing source code for unsafe patterns, including expression evaluation or arbitrary URL loading.

Document whether existing unsafe functionality must be removed, disabled, or isolated for the MVP.

---

## 29. Accessibility Requirements

Define realistic MVP accessibility requirements.

Include:

* Keyboard-operable controls
* Visible focus states
* Text labels for buttons
* Sufficient UI contrast
* Validation messages available as text
* Non-color-only selection indicators
* Screen-reader labels for form controls
* Reduced-motion behavior where practical
* Clear numerical representation outside the canvas where required

Do not claim that the Three.js visualization itself will be fully screen-reader accessible.

Document the minimum equivalent textual information that must remain available.

---

## 30. Observability and Diagnostics

Define client-side diagnostic requirements.

Include:

* Development-mode warnings
* Clear parsing errors
* Clear WebGL initialization errors
* Optional scene-resource counters
* Optional renderer statistics
* No verbose production console logging
* No unresolved runtime errors
* Actionable error boundaries or safe error views

Do not introduce a telemetry backend for the MVP.

---

## 31. Testing Requirements

Define the complete test strategy.

### Unit Tests

Cover:

* Matrix parsing
* Shape validation
* Matrix multiplication
* Vector handling
* Scalar handling
* State serialization
* State restoration
* Coordinate conversion
* Grid snapping
* Tensor bounds
* Animation-state transitions

### Mathematical Cases

Test:

```text
2×3 @ 3×2 → 2×2
3×3 @ 3×1 → 3×1
1×3 @ 3×2 → 1×2
1×3 @ 3×1 → 1×1
1×1 @ 1×1 → 1×1
```

Invalid case:

```text
2×3 @ 2×2 → validation error
```

Also test:

* Negative values
* Decimal values
* Zero matrices
* Identity matrices
* Large finite values
* Malformed input
* Non-rectangular input
* Invalid URL state

### Layout Tests

Verify:

* One value per cell
* Shared dimension alignment
* Correct tensor-plane orientation
* Scalar framing
* Vector framing
* Frame bounds
* Label attachment
* Camera-fit bounds
* Stable layout after dimension changes

### Interaction Tests

Verify:

* Hover metadata
* Result-cell selection
* Correct row-column highlighting
* Play
* Pause
* Step forward
* Step backward
* Reset
* Camera reset
* Fit view
* Share-link restoration

### Lifecycle Tests

Verify:

* Repeated scene rebuild
* Resource disposal
* Listener cleanup
* Animation cancellation
* Resize handling
* No duplicate scene nodes
* No unbounded memory growth

### Visual Regression Tests

Define stable screenshots for:

* Default scene
* Matrix-vector scene
* Vector-matrix scene
* Vector-vector scalar scene
* Selected result cell
* Active multiplication step
* Invalid input state
* Major and minor grid visibility

---

## 32. Verification Commands

After inspecting the repository, define the actual commands required for:

```text
Install dependencies
Start development server
Run unit tests
Run integration tests
Run browser tests
Run lint
Run format checks
Build production output
Preview production output
```

Do not invent commands that do not match the final recommended project configuration.

Where the repository currently lacks the required tooling, describe the required tooling addition and proposed commands.

---

## 33. Requirement Traceability

Assign every technical requirement a stable identifier.

Use prefixes such as:

```text
QM-FR-001    Functional Requirement
QM-MATH-001  Mathematical Requirement
QM-GRID-001  Grid-Ruled-Lines Requirement
QM-REN-001   Rendering Requirement
QM-INT-001   Interaction Requirement
QM-ANI-001   Animation Requirement
QM-STATE-001 State Requirement
QM-PERF-001  Performance Requirement
QM-SEC-001   Security Requirement
QM-TEST-001  Testing Requirement
QM-DOC-001   Documentation Requirement
```

Every requirement must include:

* ID
* Title
* Requirement statement
* Rationale
* Priority
* Verification method
* Dependencies
* Related acceptance criteria

Use normative language:

```text
shall
must
must not
```

Avoid using `should` unless the requirement is intentionally optional.

---

## 34. Priority Model

Classify requirements as:

```text
P0 — Required for MVP correctness
P1 — Required for MVP usability
P2 — Valuable but deferrable
P3 — Future consideration
```

Requirements marked `P3` must not silently expand the MVP scope.

---

## 35. Risks and Technical Debt

Identify project-specific risks.

At minimum evaluate:

* Large monolithic visualization files
* Mixed mathematical and rendering logic
* Hidden mutable state
* Global variables
* Experimental code paths
* Use of synchronous network requests
* Use of `eval`
* Resource leaks
* Shader portability
* Label rendering
* Browser compatibility
* URL size
* Performance at larger dimensions
* Hard-coded offsets
* Implicit coordinate conventions
* Advanced `mm` features interfering with MVP simplification
* License and attribution compliance

For each risk include:

* Description
* Probability
* Impact
* Detection method
* Mitigation
* Owner placeholder
* MVP blocking status

---

## 36. Open Questions

List unresolved decisions that materially affect implementation.

Possible examples:

* Point sprites versus instanced cubes
* JavaScript versus TypeScript
* `lil-gui` versus native controls
* URL query string versus hash fragment
* DOM labels versus canvas text
* Maximum editable matrix size
* Whether dimensions are inferred from values
* Whether step-backward stores snapshots or recomputes state
* Whether camera transforms are fully serialized
* Whether existing expression-based initializers remain internally

Do not treat unresolved questions as settled requirements.

Provide a recommended default for each open question.

---

## 37. Acceptance Criteria

Define implementation-complete criteria.

At minimum require that:

1. The application is branded as Quatricmorph.
2. The default matrix multiplication is mathematically correct.
3. Compatible matrices can be entered and visualized.
4. Invalid dimensions produce a clear error.
5. Matrix, vector, and scalar outputs render correctly.
6. Every tensor aligns with shared 3D grid ruled lines.
7. `I`, `J`, and `K` dimensions align consistently.
8. Tensor frames and labels remain attached.
9. Hover metadata is correct.
10. Selecting `C[i,j]` highlights the correct input row and column.
11. Multiplication animation is deterministic.
12. Play, pause, step, reverse, and reset behave correctly.
13. Camera controls and presets work.
14. Fit View includes all relevant tensor geometry and labels.
15. Share links restore equivalent application state.
16. Invalid URL state falls back safely.
17. Attention and LoRA features are absent from the MVP UI.
18. Browser console contains no unresolved runtime errors.
19. Repeated scene changes do not create obvious resource leaks.
20. Required automated tests pass.
21. Documentation matches the implemented architecture.
22. Original project attribution and licensing are preserved.

---

## 38. Definition of Done

The Technical Requirements Document is complete only when:

* It reflects the actual repository.
* It distinguishes current behavior from required behavior.
* It uses stable requirement IDs.
* Requirements are measurable.
* Requirements are testable.
* Priorities are assigned.
* Verification methods are defined.
* Risks are documented.
* Open questions are explicit.
* Non-goals prevent uncontrolled scope growth.
* Autonomous coding agents can decompose the document into implementation tasks.
* Engineers can review implementation against the document without relying on undocumented assumptions.

---

# Writing Rules

Use clear, direct technical English.

Avoid:

* Marketing language
* Generic software advice
* Unsupported architectural claims
* Unmeasurable adjectives
* Premature future-product requirements
* Repeating the same requirement in multiple sections
* Mixing implementation status with desired behavior
* Claiming that unverified repository behavior exists

Use tables when they improve traceability.

Use Mermaid diagrams for:

* System component architecture
* Mathematical data flow
* State transitions
* Scene lifecycle
* Input-to-render pipeline

Required high-level flow:

```text
User Input
    ↓
Parsing
    ↓
Validation
    ↓
Canonical Tensor State
    ↓
Matrix Multiplication
    ↓
Grid-Ruled-Lines Layout
    ↓
Three.js Scene Construction
    ↓
Interaction and Animation
    ↓
Rendering
```

---

# Final Response

After generating `docs/TECHNICAL_REQUIREMENTS.md`, provide:

```text
1. Repository areas inspected
2. Current architectural findings
3. Technical requirements file created
4. Total requirements by category
5. P0 requirement count
6. P1 requirement count
7. Main technical risks
8. Main open questions
9. Recommended architecture direction
10. Recommended implementation phases
11. Recommended verification commands
12. Any repository information that could not be verified
```

Do not implement product features during this task.

Do not modify unrelated source files.

Do not report requirements as verified implementation behavior unless they already exist and were confirmed in the source code.
