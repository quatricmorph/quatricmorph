# Quatricmorph - Product Definition and Technical Architecture
## Product Definition and Technical Architecture

**Document status:** Product-definition draft v1.0  
**Audience:** Founders, research engineers, ML systems engineers, model-platform teams, design partners, and investors  
**Product category:** Open-weight model infrastructure  
**Primary wedge:** Local-first checkpoint inspection, tensor querying, reproducible model comparison, and controlled model morphing

### Authority

**Implementation architecture source of truth:** [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

This document is the broader product vision (Inspect → Query → Morph → Verify). Where it conflicts with root `ARCHITECTURE.md` on ingestion, four data planes, LOD/tiles, WeightQL, CesiumJS vs custom WebGPU renderers, APIs, repository layout, roadmap phases 0–6, MVP acceptance, or “what not to do,” **follow `ARCHITECTURE.md`** and treat this file as subordinate product narrative.

Immediate engineering work is Phase 0 (Tensor Tiling Spike), not the full morph/export wedge below.

---

## 1. Executive Summary

Quatricmorph is a local-first platform for loading, indexing, querying, analyzing, comparing, transforming, validating, versioning, and exporting open-weight neural-network models.

The product treats model artifacts not as opaque checkpoint files but as structured computational objects:

- tensors with semantic roles;
- architecture graphs;
- tokenizer and vocabulary assets;
- quantization metadata;
- adapters and parameter deltas;
- runtime activations and routing events;
- transformation expression graphs;
- evaluation records;
- provenance and license constraints.

Its four core verbs are:

```text
Inspect → Query → Morph → Verify
```

The central product thesis is:

```text
Open-weight model
→ normalized neural representation
→ tensor-native indexing and query
→ statistical and behavioral analysis
→ controlled mathematical transformation
→ automated validation
→ reproducible model artifact
```

Quatricmorph should not compete as another model viewer. Visualization is an interface, not the product. The durable product is the combination of:

1. a cross-architecture semantic ontology;
2. an out-of-core tensor analytical engine;
3. a declarative query language;
4. immutable virtual-model expression graphs;
5. a compiler for transformations;
6. alignment and compatibility analysis;
7. evaluation integrated with lineage and export;
8. governance for model artifacts.

The recommended **implementation** starting point is root [`ARCHITECTURE.md`](../ARCHITECTURE.md) Phase 0–1:

```text
Open SafeTensors (single file → then sharded)
→ NSIR + metadata catalog without full RAM residency
→ multiresolution Tensor Tiles (.qtile) + tileset.json
→ CesiumJS LOD browse: model → layer → tensor → block → scalar
→ WeightQL / exact on-demand reads
```

The longer product wedge (compare → morph → verify → export) remains valid product vision but is **not** the Phase 0 coding target and must obey architecture §19 (no cube-per-weight GLBs; visualization is a projection of the same query substrate).

---

## 2. One-Sentence Product Definition

> **Quatricmorph is a tensor-native analytical database, model debugger, and controlled transformation runtime for open-weight neural networks.**

### Public-facing description

> **Inspect, query, compare, morph, and verify open-weight models without loading entire checkpoints or relying on opaque one-off scripts.**

### Recommended category language

- **For researchers:** semantic observability and transformation runtime for open-weight AI.
- **For independent developers:** a model debugger and visual tensor workbench.
- **For model-platform teams:** a reproducible model transformation and validation system.
- **For enterprise buyers:** governed model-artifact analysis, lineage, and approval infrastructure.
- **For investors:** developer infrastructure for the open-weight model lifecycle.
- **For open-source contributors:** an extensible tensor query engine, model ontology, and morph compiler.

### Evaluation of alternative descriptions

| Description                                                                                    | Strength                                    | Weakness                                                           | Best audience                     |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------ | --------------------------------- |
| DuckDB for neural-network tensors                                                              | Memorable; communicates local analytics     | Understates transformation, runtime, and model semantics           | Data and infrastructure engineers |
| Git and Blender for open-weight models                                                         | Communicates versioning plus visual editing | Too metaphorical; Git diffs and 3D editing are imperfect analogies | General developers and media      |
| A compiler and debugger for model weights                                                      | Technically accurate and differentiated     | Does not directly express database querying                        | Researchers and systems engineers |
| A semantic observability and transformation runtime for open-weight AI                         | Most complete and precise                   | Longer and less immediately accessible                             | Enterprise and technical strategy |
| Tensor-native analytical database, neural observability system, and model-morphing environment | Complete category definition                | Too long for public messaging                                      | Internal product definition       |

**Recommendation:** use “model debugger and transformation runtime” publicly, while retaining “tensor-native analytical database” as the core technical architecture.

---

## 3. Product Thesis

### 3.1 Core thesis

Open-weight models are becoming software supply-chain artifacts, yet their internal representation remains file-centric. Users download shards, inspect configuration files, write bespoke PyTorch code, run separate benchmark tools, and store complete copies of every intermediate model.

Quatricmorph replaces this workflow with a normalized computational model:

```text
checkpoint files
≠ the model

model =
architecture graph
+ tensor address space
+ tokenizer contract
+ semantic component ontology
+ optional runtime traces
+ transformation lineage
+ evaluation evidence
```

The platform should make a model addressable at multiple levels:

```text
artifact
→ subsystem
→ layer
→ component
→ tensor
→ block
→ slice
→ scalar region
```

### 3.2 Product axioms

1. **Checkpoint bytes are the source of truth; indexes are rebuildable.**
2. **No tensor transformation is considered successful until validated.**
3. **Compatibility must be proven, not inferred from matching tensor shapes alone.**
4. **A model variant should remain virtual until materialization is required.**
5. **Claims about semantics require behavioral or causal evidence.**
6. **Out-of-core execution is a first-class requirement, not an optimization.**
7. **Every result must expose cost, approximation level, confidence, and provenance.**
8. **Local execution is the default; remote compute is optional.**
9. **Visualization must be generated from the same query and lineage substrate as automation.**
10. **Open formats and reproducible recipes are more strategic than proprietary checkpoint containers.**

---

## 4. Problem Statement

### 4.1 Current workflow fragmentation

Open-weight model work is spread across:

- model repositories and registries;
- SafeTensors, GGUF, PyTorch, ONNX, and distributed checkpoint formats;
- architecture-specific Python code;
- Jupyter notebooks;
- model-merging YAML files;
- quantization pipelines;
- interpretability frameworks;
- benchmark harnesses;
- experiment trackers;
- object storage;
- shell scripts;
- manually maintained spreadsheets and Markdown reports.

The fragmentation creates both engineering cost and scientific risk.

### 4.2 Core problems

#### Model checkpoints are opaque

A checkpoint may contain hundreds or thousands of tensors across many shards. Names are architecture-specific, semantic roles are implicit, and users cannot reliably ask cross-family questions without writing adapters.

#### Large models are not interactively explorable

At hundreds of billions or trillions of parameters, full scans are expensive, full decompositions are infeasible, and browser visualizations cannot transfer raw matrices. Interactive analysis requires summaries, sketches, tiling, sampling, range reads, and progressive refinement.

#### Every analysis becomes custom code

A question such as “which MLP down-projection changed most after fine-tuning?” should not require a bespoke script. Today it usually does.

#### Model transformations are under-specified

Merge recipes often describe coefficients but omit exact source hashes, tokenizer decisions, software versions, alignment assumptions, numerical precision, seed, and validation.

#### Arbitrary averaging silently fails

Equal tensor shapes do not imply functional alignment. Independently trained models may use different neuron permutations, attention-head orderings, representational bases, vocabularies, or scaling conventions.

#### Evaluation is disconnected from transformation

Users often materialize a checkpoint, upload or deploy it, and evaluate later. The transformation system cannot prevent obviously damaged artifacts or explain regressions.

#### Intermediate checkpoints waste storage

A series of merges, adapter applications, and quantizations may create several complete model copies even when each variant is expressible as a small delta or expression graph.

#### MoE internals are especially difficult

Expert indices are local identifiers, not semantic labels. Routing behavior is input-dependent. Weight similarity alone is insufficient to determine expert equivalence, and changing experts without recalibrating routers can invalidate the system.

#### Provenance and licensing are incomplete

Model artifacts may combine sources with incompatible licenses or unclear lineage. The final model rarely contains a machine-readable bill of materials for weights, data-derived adapters, tokenizers, and transformations.

---

## 5. Target Users

### 5.1 Primary users

#### Open-weight model researchers

**Need:** understand model structure, compare checkpoints, explore weight-space geometry, and run reproducible morph experiments.

**Primary value:** replace one-off scripts with queryable, inspectable workflows.

#### Model-training and post-training engineers

**Need:** detect unstable layers, inspect checkpoint drift, compare fine-tuning runs, validate adapters, and diagnose regressions.

**Primary value:** turn checkpoint comparison into an engineering feedback loop.

#### Model-merging researchers

**Need:** align, combine, optimize, explain, and reproduce merge recipes.

**Primary value:** compiler-like plans, compatibility gates, virtual models, and integrated evaluation.

#### Quantization engineers

**Need:** localize quantization error, identify sensitive layers and channels, compare schemes, and connect tensor damage to behavior.

**Primary value:** multiresolution error maps plus calibration-aware validation.

#### Mechanistic interpretability researchers

**Need:** inspect activations, attention, residual streams, features, experts, and causal interventions.

**Primary value:** unified static and runtime object model, with claims tied to evidence.

#### MoE researchers

**Need:** study routing, expert utilization, redundancy, specialization, alignment, pruning, transplantation, and router recalibration.

**Primary value:** expert signatures, alignment workbench, and validation gates.

#### AI infrastructure engineers

**Need:** safe loading, storage efficiency, artifact lineage, deterministic exports, distributed execution, and deployment validation.

**Primary value:** a model lifecycle control plane rather than another notebook.

### 5.2 Secondary users

- Independent developers who want visual inspection without custom infrastructure.
- AI safety and audit teams that require provenance and regression analysis.
- Enterprise model-platform teams that require access controls, approvals, and private deployment.
- Educators who need interactive explanations of architecture, tensors, attention, and routing.

### 5.3 Explicit non-targets for the initial product

- Users seeking one-click training of arbitrary foundation models.
- Consumers seeking a chat application.
- Teams that only need model hosting or inference serving.
- Users expecting reliable semantic interpretation from raw weights alone.
- Users expecting arbitrary architectures to merge without data or retraining.

---

## 6. Jobs to Be Done

### 6.1 Inspect

- When I receive a model, tell me what it contains and whether it is structurally valid.
- Normalize architecture-specific names so I can inspect models consistently.
- Show which tensors are unusual, damaged, low-rank, sparse, or numerically unstable.
- Explain which information came from metadata, static analysis, inference, or causal experiments.

### 6.2 Query

- Let me ask tensor-level questions declaratively.
- Avoid loading full tensors when indexes or sketches can answer the query.
- Let me join models by semantic role rather than brittle string matching.
- Return machine-readable results suitable for notebooks, CI, and AI agents.

### 6.3 Morph

- Let me define a mathematical model variant without immediately writing a new checkpoint.
- Detect invalid compatibility assumptions before expensive execution.
- Support layer-, component-, head-, channel-, adapter-, and expert-specific transformations.
- Produce deterministic, inspectable plans.

### 6.4 Verify

- Automatically test structural, numerical, runtime, behavioral, safety, and licensing constraints.
- Explain failures and identify the tensors or operations most likely responsible.
- Make export conditional on policy gates.
- Reproduce the same artifact on another machine.

### 6.5 Govern

- Track source artifacts, transformation lineage, evaluations, approvals, and final hashes.
- Search organization-wide model inventory by architecture, capability evidence, license, or transformation history.
- Generate a model software bill of materials.

---

## 7. Product Principles

### 7.1 Evidence before interpretation

Quatricmorph must separate:

- directly observed facts;
- statistical inferences;
- behaviorally correlated findings;
- causally validated findings;
- speculative research hypotheses.

A UI label such as “math expert” must never be assigned solely from a small set of activating tokens.

### 7.2 Query before visualization

Every visualization should be backed by a query plan and reproducible result identifier. The same data should be accessible through CLI, SDK, and API.

### 7.3 Lazy by default

The system should avoid:

- loading an entire checkpoint;
- materializing complete intermediate models;
- computing exact SVD when a sketch is sufficient;
- capturing all activations when selective hooks can answer the question.

### 7.4 Compatibility as a typed contract

Each operation declares preconditions:

```text
same architecture
same tensor schema
same tokenizer contract
same hidden width
compatible normalization
known parameter basis
validated alignment
```

Plans that violate required preconditions fail before execution.

### 7.5 Validation is part of compilation

Morph compilation should output both an execution plan and a validation plan. Export without validation is allowed only through an explicit unsafe override.

### 7.6 Local-first, cloud-capable

Core inspection and basic morphing must work on a workstation. Remote compute should be a replaceable backend, not a requirement.

### 7.7 Open core around formats and reproducibility

The ontology, query specification, manifest schemas, parser SDK, and basic local execution should be open to encourage adoption and trust.

---

## 8. North-Star Workflows

### 8.1 Model import

```text
Select local directory or remote URI
→ read safe metadata
→ detect shards and architecture
→ create content hashes
→ build semantic tensor catalog
→ compute low-cost indexes
→ display import report
```

### 8.2 Checkpoint comparison

```text
Select model A and model B
→ resolve semantic alignment
→ identify compatibility class
→ query metadata and cached summaries
→ schedule required block scans
→ generate difference maps
→ rank significant changes
→ save comparison snapshot
```

### 8.3 Virtual morph

```text
Select source models
→ choose transformation
→ resolve tensor scope
→ run compatibility and alignment checks
→ build immutable expression graph
→ estimate cost and risk
→ preview affected regions
→ validate selected sample blocks
→ save virtual-model URI
```

### 8.4 Evaluation and export

```text
Select virtual model
→ optimize expression graph
→ materialize tensors lazily
→ run structural and numerical gates
→ optionally run inference evaluation
→ produce scorecard
→ export full artifact or delta
→ sign manifest and store lineage
```

### 8.5 MoE investigation

```text
Import MoE model
→ identify router, shared experts, routed experts
→ capture routing traces on calibration data
→ build expert signatures
→ inspect utilization and similarity
→ propose candidate alignments or pruning
→ run interventions and evaluation
```

---

## 9. Functional Requirements

### 9.1 Ingestion

The system must support:

- SafeTensors and sharded SafeTensors in MVP;
- Hugging Face-compatible configuration and tokenizer metadata;
- local files, HTTP range-readable files, and S3-compatible object storage;
- memory mapping and partial reads;
- content hashing;
- safe, data-only parsing;
- plugin-based architecture adapters.

Later formats may include GGUF, ONNX, PyTorch state dictionaries through isolated conversion, JAX checkpoints, and distributed checkpoint formats.

### 9.2 Semantic architecture resolution

The resolver must map architecture-specific names into a normalized ontology.

Example:

```text
model.layers.12.self_attn.q_proj.weight
transformer.h.12.attn.query.weight
language_model.encoder.layers.12.attention.wq
```

becomes:

```yaml
stack: language
layer: 12
component: attention
subcomponent: query_projection
parameter: weight
axes:
  - output_channel
  - input_channel
```

Resolution output must include:

- canonical role;
- original name;
- architecture family;
- layer and module path;
- axis semantics;
- tensor ties and aliases;
- confidence;
- parser version;
- unresolved warnings.

### 9.3 Static tensor analysis

Required operations include:

- shape, dtype, offset, storage size;
- min, max, mean, variance;
- L1, L2, and infinity norms;
- sparsity at configurable thresholds;
- quantiles and histograms;
- sign distribution;
- outlier metrics;
- channel- and block-level statistics;
- approximate rank and spectral sketches;
- random projections;
- checksums and fingerprints.

### 9.4 Cross-model comparison

The system must support:

- tensor existence and shape diffs;
- semantic-role joins;
- absolute and relative L2 distance;
- cosine similarity;
- normalized update magnitude;
- block-level difference maps;
- spectral distance;
- quantization reconstruction error;
- tokenizer and vocabulary diffs;
- layer and component rankings;
- alignment confidence.

### 9.5 Query system

WeightQL must support:

- metadata scans;
- semantic filters;
- tensor slices;
- aggregate functions;
- model joins;
- virtual-model references;
- trace tables;
- lineage tables;
- evaluation tables;
- result materialization;
- explain plans;
- explicit approximation controls.

### 9.6 Morph compiler

The compiler must support:

- linear interpolation;
- layer-wise coefficients;
- task-vector arithmetic;
- SLERP for compatible flattened regions;
- model soups;
- TIES-style merge;
- DARE-style delta sparsification;
- LoRA composition;
- sparse deltas;
- selected layer or component passthrough;
- quantization and casting;
- deterministic execution;
- dry-run preview;
- full or delta export.

### 9.7 Validation

The validation engine must include:

- tensor inventory checks;
- dtype and shape validation;
- tokenizer contract checks;
- finite-value checks;
- tied-weight checks;
- numerical drift metrics;
- sampled forward pass;
- configurable perplexity suite;
- task evaluation integration;
- latency and memory profile;
- lineage and license checks;
- policy-based pass/fail gates.

### 9.8 Visualization

The UI must include:

- architecture map;
- tensor metadata explorer;
- multiresolution heatmap;
- cross-model difference viewer;
- distribution and singular-value plots;
- virtual-model graph;
- transformation preview;
- validation scorecard;
- lineage timeline.

Runtime activation, attention, and MoE views are later-phase requirements.

---

## 10. Non-Functional Requirements

### 10.1 Performance targets

Initial engineering targets:

- Import metadata for a 70B sharded SafeTensors model in under 30 seconds on local NVMe, excluding optional indexes.
- Open a previously indexed model in under 2 seconds.
- Answer metadata-only queries in under 200 milliseconds for catalogs up to one million tensor and derived-index records.
- Render an initial tensor heatmap in under 500 milliseconds when tiles are cached.
- Keep steady-state desktop memory below 2 GB for metadata browsing.
- Support checkpoint sizes larger than RAM through range reads and streaming.
- Resume interrupted indexing and export jobs.

These are targets, not guaranteed product claims, and should be benchmarked per storage and hardware class.

### 10.2 Correctness

- Indexes must be versioned and invalidated by source hash changes.
- Exact and approximate results must be labeled.
- Deterministic operations must record seed and numerical backend.
- Floating-point non-associativity must be documented for distributed execution.
- Exported tensor byte ranges must be checksum verified.

### 10.3 Reliability

- Operations must be restartable and idempotent.
- Source artifacts are immutable.
- Partial job outputs are isolated from committed artifacts.
- The catalog must recover from process interruption.
- Long scans must expose progress at tensor-block granularity.

### 10.4 Security

- Never execute arbitrary checkpoint code.
- Plugins run with explicit capabilities.
- Remote credentials remain in OS keychain or environment providers.
- Artifact access is auditable in team deployments.
- Sensitive models can remain entirely local or air-gapped.

### 10.5 Portability

- Core daemon: Linux, macOS, Windows.
- CPU backends: x86-64 and ARM64.
- GPU backends: Metal first (v1's only GPU compute lane), CUDA second (next
  step, deferred to post-v1); other accelerators through plugins.
- UI: desktop via Tauri and browser for remote workspaces.
- Query results: Arrow-compatible records plus JSON and CSV export.

### 10.6 Extensibility

- Versioned plugin APIs for architectures, formats, analysis functions, transformations, evaluations, and visualizations.
- Forward-compatible manifests.
- Stable canonical identifiers independent of UI labels.
- No architecture-specific logic embedded in the query parser.

---

## 11. Canonical Data Model

Quatricmorph should use two related intermediate representations.

### 11.1 Neural Semantic IR

The **Neural Semantic Intermediate Representation**, abbreviated **NSIR**, represents imported models and runtime neural objects.

Core object types:

```text
ModelArtifact
ModelComponent
TensorDescriptor
TensorView
TokenizerContract
QuantizationDescriptor
AdapterArtifact
RuntimeTrace
ActivationSeries
RoutingEvent
EvaluationRun
EvidenceRecord
```

Example tensor descriptor:

```json
{
  "tensor_id": "sha256:...",
  "model_id": "sha256:...",
  "source_name": "model.layers.18.self_attn.q_proj.weight",
  "semantic_role": "language.block.attention.query.weight",
  "layer_index": 18,
  "shape": [4096, 4096],
  "dtype": "F16",
  "axes": [
    {"index": 0, "role": "output_channel"},
    {"index": 1, "role": "input_channel"}
  ],
  "storage": {
    "uri": "file:///models/model-00003-of-00008.safetensors",
    "byte_start": 1828182,
    "byte_end": 35395414
  },
  "quantization": null,
  "parser": {
    "plugin": "llama-family",
    "version": "1.2.0",
    "confidence": 0.99
  }
}
```

### 11.2 Morph IR

The **Morph Intermediate Representation**, abbreviated **MIR**, represents immutable model transformations.

Core node types:

```text
Source
Select
Align
Cast
Interpolate
Slerp
Delta
Scale
Add
Mask
Sparsify
Permute
Project
Quantize
Dequantize
Tie
Export
Validate
```

Each MIR node declares:

- input types;
- output type;
- shape constraints;
- tokenizer constraints;
- execution backend options;
- determinism properties;
- required evidence;
- estimated I/O and compute;
- validation obligations.

### 11.3 Evidence record

Every interpretation can attach:

```json
{
  "claim": "Layer 22 is unusually sensitive to 4-bit quantization",
  "evidence_type": "behavioral_correlation",
  "confidence": 0.82,
  "inputs": [
    "quantization-diff:sha256:...",
    "evaluation-run:sha256:..."
  ],
  "limitations": [
    "Calibration set contains mostly English text",
    "No causal intervention was performed"
  ]
}
```

---

## 12. System Architecture

### 12.1 High-level architecture

```text
Desktop UI / Web UI / CLI / SDK / Notebook
                    │
                    ▼
         Quatricmorph API and Session Layer
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
   Control Plane             Data Plane
        │                       │
        │              Tensor Scan and Compute
        │                       │
        ├─ Catalog              ├─ Local mmap/range reader
        ├─ NSIR registry        ├─ CPU kernels
        ├─ WeightQL planner     ├─ Metal kernels (v1) / CUDA kernels (next step)
        ├─ MIR compiler         ├─ PyTorch/Candle adapters
        ├─ Lineage              ├─ Runtime trace adapters
        ├─ Policy engine        └─ Distributed workers
        └─ Job scheduler
                    │
                    ▼
    Local files / object storage / model registries
```

### 12.2 Control plane

The control plane manages:

- model registration;
- semantic ontology;
- query planning;
- job scheduling;
- virtual-model graphs;
- lineage;
- evaluation metadata;
- access policy;
- audit;
- cache catalogs.

Recommended initial storage:

- SQLite for embedded desktop metadata;
- DuckDB for analytical metadata queries and local result caching;
- content-addressed files for manifests and derived index blobs.

Team deployment can move to PostgreSQL for transactional metadata while retaining DuckDB or a distributed analytical service for local query fragments.

### 12.3 Data plane

The data plane executes tensor work:

- range reads;
- mmap views;
- dtype conversion;
- block statistics;
- vectorized reductions;
- sketch construction;
- pairwise comparison;
- morph operations;
- materialization;
- optional inference.

Execution is organized around **tensor blocks**, not scalar rows.

### 12.4 Query planning

The planner classifies every operation:

```text
Tier 0: catalog-only
Tier 1: derived-index lookup
Tier 2: selected tile or tensor slice
Tier 3: full single-tensor scan
Tier 4: aligned multi-tensor scan
Tier 5: GPU tensor execution
Tier 6: model materialization
Tier 7: inference or activation capture
Tier 8: distributed evaluation
```

The UI must show the tier, estimated bytes read, memory, compute backend, and expected cache effects before expensive execution.

### 12.5 Recommended implementation languages

#### Rust

Use Rust for:

- SafeTensors and range-reading core;
- memory mapping;
- catalog daemon;
- query planner and execution coordination;
- block statistics;
- content hashing;
- Tensor Tile generation;
- MIR validation;
- deterministic streaming export;
- desktop backend.

#### Python

Use Python for:

- integration with PyTorch, Transformers, vLLM, SGLang, and evaluation libraries;
- experimental transformations;
- activation hooks;
- interpretability workflows;
- research plugins;
- calibration and benchmark orchestration.

Python should not own critical artifact integrity or long-lived metadata state.

#### TypeScript and React

Use TypeScript for:

- desktop and web UI;
- query editor;
- architecture navigation;
- visual state management;
- collaboration surfaces.

#### WebGPU

Use WebGPU for:

- heatmap compositing;
- tile decoding;
- difference overlays;
- interactive reductions over already-loaded visual tiles;
- large scatter and embedding views.

WebGPU should not be the authoritative backend for model export.

#### Metal (v1) / CUDA (next step, post-v1)

v1 uses Metal for these kernels. CUDA is the same role's deferred next step,
targeted at an NVIDIA RTX 3090 once v1 ships. Use Metal (v1) or CUDA
(post-v1) framework kernels for:

- high-throughput block transforms;
- full tensor comparisons;
- quantization;
- activation-heavy analysis;
- representation metrics;
- evaluation inference.

#### Candle

Candle is useful as a Rust-native inference and tensor backend for selected architectures, but the product should not depend on Candle alone. PyTorch remains the compatibility backend; vLLM and SGLang are serving/evaluation adapters.

---

## 13. Tensor Database Design

### 13.1 Why a relational table of scalar parameters is wrong

A trillion parameters stored as individual rows would produce unacceptable metadata overhead, random access patterns, and query-planning complexity. Most useful queries operate on tensor blocks, channels, rows, columns, projections, distributions, or sketches.

Quatricmorph should store:

- one catalog record per tensor;
- optional records per semantic axis or channel group;
- one record per derived block or tile;
- compact sketches;
- lazy references to raw bytes.

### 13.2 Storage layers

```text
Model Catalog
├── artifact metadata
├── architecture and ontology mappings
├── tokenizer contracts
├── tensor descriptors
├── shard locations
├── lineage
├── recipes
├── evaluations
└── policy and license records

Derived Tensor Index
├── global statistics
├── block statistics
├── histograms and quantiles
├── norm summaries
├── sparsity summaries
├── outlier maps
├── spectral sketches
├── random projections
├── similarity fingerprints
└── optional activation signatures

Raw Artifact Storage
├── SafeTensors shards
├── tokenizer files
├── adapters
├── sparse deltas
├── quantized artifacts
├── tile/index blobs
└── immutable manifests
```

### 13.3 Block model

Each tensor is divided into logical blocks independently of physical file shards.

A block descriptor includes:

```text
tensor_id
block_coordinates
logical_slice
byte ranges
dtype
compression
summary version
checksum
```

Default block shapes should be role-aware:

- matrix projections: 256×256 or 512×512;
- embeddings: token-row groups;
- vectors: contiguous ranges;
- experts: expert-major grouping;
- quantized tensors: align with quantization group size.

### 13.4 Derived indexes

#### Exact low-cost indexes

- shape;
- dtype;
- byte size;
- tensor hash;
- min/max;
- L1/L2 norm;
- non-finite counts;
- exact zero count.

#### Approximate indexes

- t-digest or KLL-style quantiles;
- fixed or adaptive histograms;
- count-min sketches for discrete routing events;
- randomized SVD or Lanczos sketches;
- random projections;
- SimHash-like fingerprints;
- sampled channel statistics.

Every approximate index records error bounds or sampling configuration where meaningful.

### 13.5 Remote storage

Remote object access should use:

- HTTP range requests;
- S3-compatible ranged GET;
- read coalescing;
- per-shard header caching;
- local block cache;
- checksum validation;
- concurrency limits.

SafeTensors is particularly suitable for initial support because tensor metadata exposes byte ranges and the format is data-only. For derived arrays and traces, Quatricmorph may adopt a chunked N-dimensional layout inspired by Zarr or use an embedded array engine. The choice should be benchmark-driven rather than treated as a branding dependency.

### 13.6 Distributed execution

Distributed execution is required only after local product-market fit.

A distributed query decomposes into:

```text
catalog plan
→ block scan tasks
→ local partial aggregates
→ reduce stage
→ optional result tiles
```

Large result transport should use Arrow-compatible batches or Arrow Flight. Raw tensors should remain near storage whenever possible.

### 13.7 Trillion-parameter difficulty

At one trillion FP16 parameters, raw weights are roughly two terabytes before replicas, indexes, or optimizer states. A single complete scan is therefore an I/O job, not an interactive operation. Quatricmorph must:

- distinguish metadata queries from scans;
- reuse derived summaries;
- push filters to semantic catalogs;
- select only required shards and ranges;
- use approximate sketches;
- schedule scans asynchronously within the local job system;
- stream partial results;
- avoid centralizing data in the UI;
- make cost visible before execution.

---

## 14. WeightQL Design

### 14.1 Purpose

WeightQL is a declarative language for querying model structure, tensors, runtime traces, lineage, and evaluations.

It is not SQL with a scalar row per parameter. Tensor values remain typed array objects.

### 14.2 Core namespaces

```text
catalog.models
catalog.tensors
catalog.tokenizers
catalog.artifacts
model('id').tensors
model('id').components
model('id').tiles
trace('id').activations
trace('id').attention
trace('id').routing
lineage('id').nodes
evaluation('id').metrics
virtual_model('id').nodes
```

### 14.3 Tensor type

A WeightQL tensor value includes:

```text
TensorRef<dtype, shape, axes, semantic_role>
```

Operators may consume a TensorRef without materializing it. The planner selects an index, scan, or backend.

### 14.4 Example: metadata and statistics

```sql
SELECT
    layer_index AS layer,
    source_name,
    shape,
    dtype,
    stats.mean,
    stats.stddev,
    stats.l2_norm,
    stats.sparsity_1e_6
FROM model('model-a').tensors
WHERE semantic_role LIKE 'language.block.attention.query.%'
ORDER BY layer;
```

### 14.5 Example: explicit computation

```sql
SELECT
    layer_index,
    mean(weight) AS mean,
    stddev(weight) AS std,
    l2_norm(weight) AS norm,
    sparsity(weight, threshold => 1e-6) AS sparsity
FROM model('model-a').tensors
WHERE semantic_role = 'language.block.attention.query.weight'
USING APPROXIMATION 'block-index'
WITH ERROR <= 0.01;
```

### 14.6 Example: aligned comparison

```sql
WITH paired AS (
    SELECT *
    FROM ALIGN(
        model('base-model'),
        model('fine-tuned-model'),
        BY => 'semantic_role',
        REQUIRE => ['shape', 'tokenizer_contract']
    )
)
SELECT
    a.layer_index,
    cosine_similarity(a.weight, b.weight) AS similarity,
    relative_l2(a.weight, b.weight) AS relative_change,
    spectral_distance(a.weight, b.weight) AS spectral_change
FROM paired
WHERE a.semantic_role LIKE 'language.block.mlp.%'
ORDER BY relative_change DESC;
```

### 14.7 Example: MoE routing

```sql
SELECT
    layer_index,
    expert_index,
    activation_frequency(),
    routing_entropy(),
    routing_overlap(dataset_partition => 'code'),
    output_signature_similarity()
FROM trace(
    model => 'moe-model',
    dataset => 'calibration-prompts',
    capture => ['routing', 'expert_output']
)
GROUP BY layer_index, expert_index;
```

### 14.8 Example: virtual morph

```sql
CREATE VIRTUAL MODEL 'experiment-42' AS
MORPH model('base')
WITH model('math-tuned')
USING TASK_VECTOR(alpha => 0.35)
ON layers(16, 31)
EXCEPT roles('embedding.%', 'normalization.%');
```

### 14.9 Explain plan

```sql
EXPLAIN ANALYZE
SELECT spectral_distance(a.weight, b.weight)
FROM ...
```

Example output:

```text
Catalog filter: 64 tensors
Existing spectral sketches: 52 tensors
New randomized sketches required: 12 tensors
Estimated read: 18.4 GB
Backend: CPU AVX2
Peak memory: 1.2 GB
Approximation: randomized SVD, rank=64, seed=42
```

### 14.10 Natural-language layer

Natural-language requests compile into WeightQL or MIR and must display the resolved plan.

Example:

> “Find the layers most damaged by quantization.”

Resolved plan:

```text
1. Align FP16 and quantized checkpoints by semantic role.
2. Dequantize selected blocks.
3. Compute relative L2, cosine, SQNR, and spectral-sketch drift.
4. Rank layers by weighted damage score.
5. Do not run inference.
```

No natural-language request may modify or export a model without explicit approval of the resolved plan.

### 14.11 Extensibility

Functions are versioned:

```text
spectral_distance@1
cka@1
routing_entropy@1
quantization_error@2
```

Plugin functions declare:

- input types;
- required data;
- exact or approximate semantics;
- backend support;
- deterministic behavior;
- cache key;
- cost estimator.

---

## 15. Tensor Tiles

### 15.1 Definition

Tensor Tiles are a multiresolution representation of large tensors for visualization and approximate query.

For a matrix:

```text
Level 0: one global summary
Level 1: 32×32 summary blocks
Level 2: 256×256 summary blocks
Level 3: 2048×2048 summary blocks
Level 4: raw, sampled, or quantized values
```

The levels describe logical resolution, not necessarily fixed dimensions for every tensor.

### 15.2 Tile payload

A tile may contain:

- count;
- mean;
- variance;
- min and max;
- L1 and L2 norms;
- sparsity;
- quantiles;
- sign ratio;
- outlier count;
- quantization error;
- difference metrics;
- optional spectral summary;
- checksum;
- source and index versions.

### 15.3 Pyramid generation

Tiles should be generated bottom-up or directly from streamed blocks:

```text
raw tensor blocks
→ fine summaries
→ hierarchical reduction
→ compressed tile blobs
```

For remote models, tile generation can be incremental. Initial import creates Level 0 and coarse Level 1 summaries; deeper levels are generated on demand.

### 15.4 Visualization use

#### Heatmaps

The viewport requests only tiles intersecting the current region and zoom level.

#### Difference maps

Tiles are derived from aligned source blocks and store signed or absolute difference statistics.

#### Quantization maps

Each tile stores reconstruction error, clipping count, and optional activation-weighted sensitivity.

#### Outlier inspection

A tile can include compact top-k locations or a secondary sparse index.

#### Cross-model comparison

Two synchronized viewports use the same semantic axes and alignment mapping.

### 15.5 Web architecture

```text
UI viewport
→ tile request
→ catalog resolves model/tensor/level/coordinates
→ cache lookup
→ local or remote tile fetch
→ WebGPU decode and compositing
```

The browser never receives the complete tensor unless explicitly requested and allowed.

### 15.6 Collaborative inspection

A shared annotation references:

```text
model hash
tensor id
tile level
logical coordinates
query result id
color scale
annotation text
```

This makes annotations stable and reproducible instead of screenshot-based.

### 15.7 Limitations

Tensor Tiles can reveal numerical patterns but cannot by themselves establish semantic meaning. Their summaries may hide rare scalar events; users must be able to refine to higher resolution or exact scans.

---

## 16. Virtual Models and Morph Compiler

### 16.1 Virtual Model definition

A Virtual Model is an immutable, content-addressed MIR expression graph that resolves to a model artifact but is not necessarily materialized.

Example:

```text
base
+ 0.70 × code_delta
+ 0.35 × math_delta
- 0.15 × verbosity_delta
```

Example component recipe:

```text
layers 0–15: lerp(A, B, 0.30)
layers 16–31: lerp(A, B, 0.70)
attention: A
MLP: B
normalization: A
embedding and LM head: A
```

### 16.2 URI

```text
qmodel://workspace/experiment-42@sha256:...
```

### 16.3 Graph properties

- immutable;
- typed;
- content-addressed;
- reproducible;
- backend-independent at the logical level;
- optimizable;
- partially materializable;
- queryable.

### 16.4 Compiler passes

#### Validation pass

Checks shapes, dtypes, semantic roles, tokenizer contracts, and operation-specific preconditions.

#### Canonicalization pass

Normalizes equivalent expressions and stable ordering.

#### Algebraic simplification

Examples:

```text
lerp(A, A, α) → A
add(delta(A, B), A) → B
scale(scale(X, a), b) → scale(X, a·b)
```

#### Sparse-delta fusion

Combines compatible masks and deltas without dense expansion.

#### Cast and quantization fusion

Where numerically acceptable:

```text
interpolate FP16 sources
→ quantize output
```

can be fused into block-wise execution without an intermediate full FP16 artifact.

#### Common-subexpression elimination

Shared source blocks are read once across multiple dependent operations.

#### Backend partitioning

Metadata checks run on CPU; heavy transforms can be assigned to GPU; export streams results to storage.

### 16.5 Partial materialization

A query for one tensor in a virtual model materializes only the required dependency subgraph.

An inference runtime may materialize layer blocks just in time, but initial releases should prefer cached full materialization for serving reliability.

### 16.6 Strategic value

Virtual Models reduce:

- storage duplication;
- experiment setup time;
- export churn;
- hidden procedural state;
- ambiguity in complex merges.

They also make model transformations searchable, reviewable, and suitable for CI.

### 16.7 Risks

- Long expression chains can become expensive or numerically unstable.
- Lazy remote dependencies can disappear.
- Different backends may produce small numerical differences.
- Inference from deeply virtual models can have unpredictable latency.
- Content addressing requires canonical graph serialization.

Mitigations include graph flattening, source pinning, backend recording, cache policies, and mandatory materialization for production deployment.

---

## 17. Morphing Methods

### 17.1 Compatibility classes

Quatricmorph must classify source models before exposing operations.

| Class | Description                                                      | Default policy                                              |
| ----- | ---------------------------------------------------------------- | ----------------------------------------------------------- |
| C0    | Exact same artifact                                              | All algebraic identities allowed                            |
| C1    | Same base checkpoint with compatible fine-tunes or adapters      | Task-vector and delta methods allowed with validation       |
| C2    | Same architecture and tokenizer, different training trajectories | Averaging only after alignment and interpolation tests      |
| C3    | Same architecture, different tokenizer or vocabulary             | Restricted; vocabulary alignment required                   |
| C4    | Different layer count or expert count, compatible hidden width   | Structural stitching or selection; research-heavy           |
| C5    | Different hidden dimensions or component topology                | Learned projections required; research-heavy                |
| C6    | Completely different architecture families                       | No direct weight merge; distillation or adapter bridge only |

### 17.2 Linear interpolation

\[
\theta(t) = (1-t)\theta_A + t\theta_B
\]

**Established use:** checkpoints connected within a compatible low-loss region, especially fine-tunes from the same initialization.

**Valid preconditions:**

- same tensor schema;
- same parameter basis;
- compatible tokenizer;
- compatible normalization and architecture;
- no unresolved permutation or rotation mismatch;
- validation across interpolation points.

**Failure modes:**

- loss barrier between independently trained models;
- destructive cancellation;
- normalization drift;
- embedding mismatch;
- model-specific scaling differences.

Quatricmorph should sample an interpolation curve before export where evaluation cost permits.

### 17.3 Weighted averaging and model soups

\[
\theta^* = \sum_i \alpha_i \theta_i,\quad \sum_i \alpha_i=1
\]

Most credible when models share a pretrained initialization and related fine-tuning regime. A greedy or evaluation-guided selection is safer than assuming all candidates belong in the average.

### 17.4 SLERP

SLERP interpolates directions on a hypersphere and may preserve norm geometry better than linear interpolation in selected settings. It is still not a compatibility solution. The system must define whether SLERP is applied per tensor, per layer, or over a flattened selected region.

### 17.5 Task-vector arithmetic

\[
\Delta_i = \theta_i-\theta_0
\]

\[
\theta^*=\theta_0+\sum_i\alpha_i\Delta_i
\]

Useful when fine-tuned models share the same base and task updates are sufficiently composable. Risks include sign conflict, overlapping subspaces, scale mismatch, and capability interference.

### 17.6 TIES-style merge

TIES-style processing trims small deltas, resolves sign disagreement, and merges aligned changes. It is an established method, but it does not prove that the resulting behavior is desirable.

### 17.7 DARE-style merge

DARE-style processing randomly drops and rescales delta parameters before another merge method. It should record seed, drop rate, scope, and rescaling exactly. Stochastic sparsification makes reproducibility metadata mandatory.

### 17.8 LoRA composition

LoRA updates are low-rank:

\[
\Delta W = \frac{\alpha}{r}BA
\]

Composition can be performed as dense deltas, concatenated factors, or approximated recompression. Different adapters can conflict even when they target the same layers. Quatricmorph should expose update magnitude, subspace overlap, and recompression error.

### 17.9 Layer- and component-wise merge

Layer-dependent interpolation:

\[
\theta_l^*=(1-\alpha_l)\theta_l^A+\alpha_l\theta_l^B
\]

Component selection can preserve embeddings, normalization, attention, MLP, router, or LM head from chosen sources. The compiler must enforce tied-weight and residual-width constraints.

### 17.10 Pruning and replacement

Supported mature operations:

- head masking or structural pruning with architecture update;
- expert removal with router update;
- layer removal for compatible architectures;
- channel masks;
- selected tensor replacement from an ancestor.

All require runtime validation. Structural pruning often requires additional fine-tuning and should not be marketed as lossless.

### 17.11 Quantization

Quantization transformations include:

- dtype cast;
- symmetric or asymmetric group-wise quantization;
- dequantization;
- requantization;
- mixed-precision exceptions;
- outlier-channel preservation.

Static reconstruction error is necessary but insufficient. Activation-aware calibration and downstream evaluation are required for reliable quality conclusions.

### 17.12 Weight repair

Mature repair operations:

- restore tensor from a known-good ancestor;
- replace non-finite values using source lineage;
- undo a known adapter or delta;
- re-export corrupted shards.

Research-heavy repair:

- infer damaged weights without a reliable source;
- optimize selected tensors against behavior while preserving unrelated capabilities;
- repair a representation subspace using learned constraints.

### 17.13 Model stitching

Learned projections can connect representations between different widths or architectures. This is a training problem, not a direct merge. Quatricmorph may orchestrate it later but should label it experimental.

### 17.14 Dense-to-MoE conversion

Dense-to-MoE conversion is not averaging. It requires decisions about:

- expert initialization;
- duplication versus specialization;
- router architecture;
- load balancing;
- training objective;
- capacity factors;
- shared expert behavior;
- post-conversion training.

A plausible workflow may duplicate a dense MLP into experts and then specialize with training, but the useful result comes from optimization and routing calibration, not mathematical duplication alone.

---

## 18. Model Alignment Strategy

### 18.1 Alignment pipeline

```text
artifact compatibility
→ tensor-name alignment
→ semantic-role alignment
→ axis and shape alignment
→ tokenizer and vocabulary alignment
→ layer alignment
→ neuron/head/expert alignment
→ representation validation
```

Each stage produces a mapping, confidence score, evidence, and unresolved issues.

### 18.2 Tensor-name alignment

Use architecture plugins and explicit alias tables. Never rely only on suffix matching.

### 18.3 Semantic-component alignment

Map source components into NSIR roles and compare:

- component type;
- layer position;
- axis semantics;
- residual connections;
- tied parameters;
- quantization layout.

### 18.4 Neuron permutation alignment

Hidden neurons can be permuted if corresponding incoming and outgoing dimensions are permuted consistently. Independently trained networks may implement similar functions in different permutations.

Quatricmorph should support:

- weight matching;
- activation matching on calibration data;
- optimal assignment using Hungarian matching;
- iterative layer-wise matching;
- confidence and residual mismatch reporting.

Permutation alignment does not address all representational symmetries; rotations and distributed features can remain.

### 18.5 Attention-head alignment

Head indices are not universal identities. Alignment signals may include:

- Q/K/V/O weight similarity;
- attention-pattern similarity;
- output subspace similarity;
- activation signatures;
- ablation effects;
- token-pattern overlap.

Head alignment is more reliable within closely related fine-tunes than independently trained architectures.

### 18.6 Vocabulary alignment

Tokenizer compatibility matters because embedding row `i` and LM-head row `i` represent tokenizer-specific IDs.

Cases:

1. identical tokenizer files and vocabulary ordering;
2. overlapping vocabulary with different IDs;
3. vocabulary expansion;
4. different segmentation algorithms;
5. modality-specific special tokens.

The system should provide:

- exact tokenizer hash comparison;
- token-string mapping;
- collision and ambiguity report;
- special-token contract checks;
- initialization policy for unmatched tokens;
- evaluation of retokenization effects.

Naively averaging embedding matrices from different vocabularies is invalid.

### 18.7 Layer alignment

For different layer counts, candidate mappings may use:

- normalized depth;
- representation similarity;
- dynamic programming;
- learned layer correspondence;
- architecture-specific constraints.

Direct layer interpolation across mismatched depth remains research-heavy.

### 18.8 Representation alignment

Calibration inference can compute:

- CKA;
- canonical correlation variants;
- principal angles;
- output cosine similarity;
- token-conditioned activation signatures.

Representation alignment requires datasets and inference. It should be treated as stronger evidence than weight similarity but still task-distribution dependent.

### 18.9 Alignment decision policy

The compiler should require minimum confidence by operation:

```text
metadata comparison: no alignment confidence required
same-base task arithmetic: exact source-base lineage required
independent model interpolation: high neuron/head alignment confidence
expert transplantation: high expert and router compatibility confidence
cross-width stitching: learned projection and behavioral validation
```

---

## 19. MoE Strategy

### 19.1 Product position

MoE support should become a major differentiator after the dense-model core is stable. It is valuable because standard viewers and merge tools often expose experts as repeated tensors without a behavioral model of routing.

### 19.2 MoE object model

```text
MoEBlock
├── Router
├── RoutedExpert[]
├── SharedExpert[]
├── TopKPolicy
├── CapacityPolicy
├── LoadBalancingMetadata
└── RoutingTrace[]
```

### 19.3 Expert signature

For each expert, capture:

- weight statistics;
- spectral sketch;
- input activation centroid;
- output representation sketch;
- token routing distribution;
- sequence and domain routing distribution;
- utilization frequency;
- overflow and capacity events;
- router margin;
- benchmark intervention effects.

### 19.4 Expert Atlas

The atlas presents experts in a projected space based on selectable signatures:

- weight-space similarity;
- activation-space similarity;
- output-space similarity;
- routing overlap;
- causal contribution.

The UI must not imply that a 2D projection is a complete semantic map.

### 19.5 Expert alignment

Candidate cost function:

\[
C_{ij} =
w_1 d_{\text{weight}}
+w_2 d_{\text{spectral}}
+w_3 d_{\text{activation}}
+w_4 d_{\text{output}}
+w_5 d_{\text{routing}}
\]

Assignment methods:

- Hungarian matching for one-to-one alignment;
- optimal transport for soft many-to-many correspondence;
- clustering for redundant expert groups;
- learned projection for output-space mismatch.

Expert index numbers do not imply alignment because ordering is arbitrary and experts are jointly shaped by their router and training trajectory.

### 19.6 Expert pruning

Workflow:

```text
identify low-utilization or redundant candidates
→ simulate router remapping
→ run expert ablation
→ measure load and quality changes
→ optionally fine-tune router
→ export only after validation
```

Low utilization alone is insufficient; rare experts may be critical for specific domains.

### 19.7 Expert transplantation

Required checks:

- same hidden and intermediate dimensions;
- same activation function and normalization;
- compatible expert parameter layout;
- output-scale compatibility;
- router feature compatibility;
- calibration data;
- post-transplant router recalibration.

This is research-heavy and should not be in MVP.

### 19.8 Router recalibration

Possible approaches:

- temperature and bias calibration;
- load-balancing optimization;
- supervised routing from source traces;
- short post-training with frozen experts;
- distillation of routing decisions.

### 19.9 Scientific caution

Expert specialization can be real, weak, distributed, or dataset-dependent. Quatricmorph should report “routing association” before “semantic specialization” unless causal tests support the stronger claim.

---

## 20. Visualization System

### 20.1 Architecture Map

The architecture map represents the semantic graph:

```text
Tokenizer
→ Embedding
→ Repeated blocks
   ├── Attention
   ├── MLP or MoE
   ├── Normalization
   └── Residual paths
→ Final normalization
→ LM head
```

Zoom levels:

```text
model
→ stack
→ block
→ component
→ tensor
→ tile
→ slice
```

### 20.2 Tensor Explorer

Required views:

- 2D heatmap;
- block summary;
- row and column profiles;
- distribution;
- singular-value plot;
- difference overlay;
- quantization error;
- outlier channels;
- raw sample inspector;
- axis semantic labels.

A 3D view is optional and should only be used when it clarifies genuine structure, not as decorative visualization.

### 20.3 Matrix multiplication view

For selected projections, show:

```text
input activation × weight → output activation
```

The view should connect tensor dimensions to semantic axes, selected tokens, and runtime values when traces exist.

### 20.4 Attention Explorer

Later-phase capabilities:

- token-to-token attention;
- head comparison;
- Q/K geometry;
- attention entropy;
- top activating sequences;
- ablation and patching results;
- head alignment across checkpoints.

Attention weights are not explanations by themselves. Causal views must be clearly distinguished.

### 20.5 Activation Explorer

Later-phase capabilities:

- residual trajectories;
- hidden-state norms;
- MLP activation patterns;
- token-to-layer traces;
- prompt comparison;
- activation patching;
- representation similarity;
- sparse feature integration.

### 20.6 MoE Expert Atlas

Displays:

- expert utilization;
- routing distributions;
- load imbalance;
- similarity clusters;
- dead or redundant candidates;
- alignment mappings;
- expert merge or transplant previews.

### 20.7 Morph Timeline

```text
base
→ fine-tune
→ adapter
→ task vector
→ merge
→ quantization
→ evaluation
→ export
```

Each node links to source hashes, recipe, metrics, and approvals.

### 20.8 Transformation Preview

Before execution, show:

- affected tensors;
- unchanged tensors;
- coefficients by layer and component;
- expected bytes read and written;
- peak memory;
- alignment confidence;
- tokenizer warnings;
- validation plan;
- estimated checkpoint size;
- irreversible operations;
- research-heavy operations.

### 20.9 Why visualization alone is insufficient

Visualization is easy to copy and difficult to trust when disconnected from computation. A useful visualization must be:

- generated from reproducible queries;
- backed by exact model hashes;
- scalable through tiles and summaries;
- linked to validation;
- accessible through APIs;
- interpretable with approximation metadata.

The product moat is therefore not the heatmap; it is the semantic and computational substrate that makes the heatmap meaningful.

---

## 21. Validation and Evaluation

### 21.1 Validation levels

#### Level 0: plan validation

- source existence;
- operation schema;
- compatibility class;
- alignment requirements;
- estimated resources;
- license policy.

#### Level 1: artifact integrity

- tensor inventory;
- shapes and dtypes;
- shard offsets;
- checksums;
- finite values;
- tied parameters;
- tokenizer files.

#### Level 2: numerical integrity

- source distance;
- update-to-weight ratio;
- cosine similarity;
- norm changes;
- spectral sketch drift;
- quantization error;
- normalization statistics.

#### Level 3: runtime integrity

- load succeeds;
- sampled forward pass;
- finite logits;
- KV-cache behavior;
- deterministic generation where configured;
- memory within limit.

#### Level 4: behavioral evaluation

- perplexity;
- task suites;
- user regression prompts;
- generation quality;
- safety tests;
- domain-specific metrics.

#### Level 5: mechanistic or causal validation

- ablation;
- activation patching;
- feature steering;
- expert suppression;
- counterfactual prompts;
- source-checkpoint comparison.

### 21.2 Evaluation manifest

```yaml
evaluation:
  model: qmodel://workspace/experiment-42
  runtime:
    engine: vllm
    version: ...
    dtype: bfloat16
    seed: 42
  datasets:
    - id: calibration-english-v1
      hash: sha256:...
    - id: internal-code-regression-v3
      hash: sha256:...
  metrics:
    - perplexity
    - exact_match
    - pass_at_1
    - latency_p50
    - memory_peak
  decoding:
    temperature: 0
    max_tokens: 512
```

### 21.3 Integrated gates

Example policy:

```text
Block export if:
- any tensor contains NaN or Inf;
- tokenizer contract is unresolved;
- perplexity regresses more than 5%;
- required license metadata is missing;
- alignment confidence is below operation threshold.
```

### 21.4 Evaluation reproducibility

Evaluation records must include:

- model graph hash;
- materialized artifact hash;
- runtime engine and version;
- hardware;
- dataset hashes;
- prompt templates;
- tokenizer hash;
- decoding parameters;
- seeds;
- metric implementation versions;
- logs and failures.

### 21.5 Static versus runtime analysis

#### Checkpoint-only

- architecture inventory;
- tensor statistics;
- diffs;
- task-vector construction;
- approximate spectra;
- static quantization error;
- storage and lineage.

#### Requires GPU or high-throughput tensor backend

- large-scale quantization;
- full SVD-like analyses;
- many cross-model similarities;
- materialization of large models;
- activation-derived alignment.

#### Requires inference

- perplexity;
- activation drift;
- routing behavior;
- attention patterns;
- representation similarity;
- benchmark capability;
- causal intervention.

---

## 22. Model Version Control

### 22.1 Concept

Quatricmorph version control is content-addressed artifact lineage, not textual version control.

A model version can be represented as:

```text
base checkpoint
+ sparse delta
+ LoRA
+ task vector
+ merge expression
+ tokenizer patch
+ quantization recipe
+ metadata patch
```

### 22.2 Core operations

```bash
quatricmorph init
quatricmorph add-model ./base
quatricmorph branch math-experiment
quatricmorph apply recipe.yaml
quatricmorph diff base math-experiment
quatricmorph validate math-experiment
quatricmorph tag math-experiment v0.3
quatricmorph export v0.3
```

### 22.3 Diff semantics

A model diff can include:

- artifact and metadata changes;
- tensor existence changes;
- sparse or dense parameter delta;
- block statistics;
- semantic component summaries;
- tokenizer changes;
- virtual graph changes;
- evaluation changes.

It should not default to storing every numeric delta if a transformation recipe already provides a smaller reproducible representation.

### 22.4 Branching and merging

A branch points to a virtual-model graph. Merging branches means composing model expressions and resolving conflicts such as:

- both branches change the same tensor region;
- incompatible tokenizers;
- conflicting quantization;
- incompatible structural edits;
- contradictory license constraints.

### 22.5 Content-addressed storage

Hash boundaries:

- raw artifact;
- shard;
- tensor;
- tile/index blob;
- recipe;
- MIR graph;
- evaluation run;
- final export;
- model SBOM.

### 22.6 Signed artifacts

A signed export should include:

```text
artifact hash
recipe hash
source hashes
evaluation policy hash
evaluation result hashes
builder identity
timestamp
software environment
signature
```

### 22.7 Strategic analogy to Git

The Git analogy is useful for:

- immutable history;
- branching;
- content addressing;
- reproducible builds;
- tags;
- review.

It breaks down because:

- tensors are huge;
- numerical diffs can be dense;
- merge conflicts are semantic and behavioral;
- equivalent networks may use different parameter symmetries;
- small parameter changes can have large behavioral effects.

Quatricmorph should use Git-like workflow concepts without pretending tensor merges are textual three-way merges.

---

## 23. Security, Licensing, and Provenance

### 23.1 Safe loading

- Prefer SafeTensors and other data-only formats.
- Never execute arbitrary Python during import.
- Isolate converters for unsafe legacy formats.
- Enforce file-size and decompression limits.
- Validate offsets and shape multiplication for overflow.

### 23.2 Source immutability

Imported artifacts are read-only. Transformations always produce new virtual or materialized artifacts.

### 23.3 Plugin security

Plugins declare permissions:

```text
read model bytes
write derived index
use network
execute GPU
launch subprocess
access secret
```

Enterprise deployments can disable unsigned plugins.

### 23.4 License graph

Each artifact carries:

- source license identifier;
- license text hash;
- use restrictions;
- attribution requirements;
- redistribution policy;
- commercial-use status;
- derivative-work obligations;
- unresolved warnings.

A transformation combines license constraints through a policy engine. The result is a warning and review aid, not automatic legal advice.

### 23.5 Provenance

Model SBOM fields:

- model sources;
- tokenizer sources;
- adapter sources;
- transformation recipes;
- quantization;
- evaluation datasets;
- software versions;
- builder;
- signatures;
- final hashes.

### 23.6 Enterprise controls

- role-based access;
- project and artifact permissions;
- approval workflows;
- immutable audit logs;
- private deployment;
- air-gapped operation;
- retention policies;
- artifact signing;
- organization policy packs;
- external sharing controls.

---

## 24. Product Surfaces

### 24.1 Desktop application

Recommended stack:

- Tauri;
- React and TypeScript;
- Rust daemon;
- WebGPU rendering;
- embedded SQLite and DuckDB;
- direct local file access.

Primary desktop workflows:

- open model;
- inspect catalog;
- compare;
- write WeightQL;
- create morph recipe;
- preview;
- validate;
- export.

### 24.2 Web application

The web application connects to a remote Quatricmorph daemon and shared registry. It should not require uploading raw weights to the browser.

### 24.3 CLI

```bash
quatricmorph inspect ./model
quatricmorph index ./model --profile standard
quatricmorph query ./model --file analysis.wql
quatricmorph compare ./base ./finetuned
quatricmorph morph recipe.yaml --dry-run
quatricmorph evaluate qmodel://experiment-42
quatricmorph export qmodel://experiment-42 --format safetensors
```

### 24.4 SDKs

Priority:

1. Python SDK;
2. Rust library;
3. HTTP and Arrow-compatible API;
4. TypeScript client;
5. gRPC or Arrow Flight for distributed transfer.

### 24.5 Notebook integration

A notebook cell can return:

- Arrow table;
- tensor summary object;
- tile-backed interactive visualization;
- virtual-model reference;
- evaluation record.

The notebook should reference daemon-managed artifacts rather than duplicate model state in the kernel.

### 24.6 Team server

- shared model registry;
- experiments;
- annotations;
- recipes;
- evaluations;
- approvals;
- audit;
- remote workers;
- private object storage.

---

## 25. CPU, GPU, and Inference Execution Matrix

| Operation                         |                CPU viable |              GPU useful | Inference required |
| --------------------------------- | ------------------------: | ----------------------: | -----------------: |
| Parse metadata and shards         |                       Yes |                      No |                 No |
| Architecture resolution           |                       Yes |                      No |                 No |
| Hashing and integrity checks      |                       Yes |                  Rarely |                 No |
| Global statistics                 |                       Yes |               For speed |                 No |
| Block histograms and quantiles    |                       Yes |               For speed |                 No |
| Approximate spectral sketches     |      Yes for small/medium |           Yes for large |                 No |
| Cross-model L2 and cosine         |                       Yes |           Yes for large |                 No |
| Linear interpolation export       |            Yes, I/O-bound |      Yes for throughput |                 No |
| Task-vector merge                 |                       Yes |      Yes for throughput |                 No |
| TIES/DARE                         |                       Yes |    Yes for large models |                 No |
| LoRA composition                  |                       Yes | Yes for dense expansion |                 No |
| Quantization                      |                 Sometimes |                 Usually |  Calibration often |
| Tokenizer alignment               |                       Yes |                      No |           Optional |
| Neuron alignment by weights       |                       Yes |                  Useful |                 No |
| Neuron alignment by activations   |                   Limited |                     Yes |                Yes |
| Attention and activation analysis |     No practical at scale |                     Yes |                Yes |
| MoE routing analysis              |                   Limited |                     Yes |                Yes |
| Perplexity and benchmarks         |     No practical at scale |                     Yes |                Yes |
| Runtime latency and memory        | No for target GPU runtime |                     Yes |                Yes |
| Causal tracing and patching       |     No practical at scale |                     Yes |                Yes |

---

## 26. Major Feature Scorecards

### 26.1 Semantic ontology and model catalog

- **User problem:** architecture-specific tensor names and layouts prevent reusable analysis.
- **Product behavior:** normalize models into NSIR with confidence and unresolved warnings.
- **Implementation:** versioned architecture plugins, alias rules, graph reconstruction, axis semantics.
- **Required data:** configs, tensor headers, tokenizer metadata, optional source architecture code descriptions.
- **Computational cost:** low; metadata-bound.
- **UI:** architecture tree, semantic filters, unresolved mapping inspector.
- **Failure modes:** incorrect role mapping, tied-weight omission, unsupported custom architecture.
- **Validation:** fixture checkpoints, round-trip parser tests, known architecture invariants.
- **MVP priority:** P0.
- **Strategic value:** very high; foundational cross-architecture asset.

### 26.2 Out-of-core tensor engine

- **User problem:** models exceed RAM and GPU memory.
- **Product behavior:** scan only required blocks, reuse indexes, stream results.
- **Implementation:** mmap, range reads, block scheduler, vectorized kernels, cache.
- **Required data:** tensor offsets, dtypes, shapes, storage URIs.
- **Computational cost:** I/O-dominated for scans.
- **UI:** explain plan, bytes-read estimate, progress, cancellation.
- **Failure modes:** fragmented remote reads, cache thrashing, dtype conversion overhead.
- **Validation:** checksum-based block tests, fault injection, benchmark suite.
- **MVP priority:** P0.
- **Strategic value:** very high; difficult engineering moat.

### 26.3 WeightQL

- **User problem:** every analysis requires custom code.
- **Product behavior:** declarative cross-model tensor queries with cost visibility.
- **Implementation:** parser, typed logical plan, optimizer, tensor function registry.
- **Required data:** catalog, indexes, tensor engine.
- **Computational cost:** depends on plan tier.
- **UI:** query editor, schema explorer, explain plan, result table and visualization.
- **Failure modes:** ambiguous functions, accidental full scans, unstable plugin semantics.
- **Validation:** golden query corpus, type tests, cost-plan tests.
- **MVP priority:** P1; basic subset in first public release.
- **Strategic value:** very high if ecosystem adoption occurs.

### 26.4 Tensor Tiles

- **User problem:** raw tensors are too large for interactive visualization.
- **Product behavior:** progressively load summaries by zoom and viewport.
- **Implementation:** multiresolution block pyramid, compressed tile cache, WebGPU renderer.
- **Required data:** raw blocks or derived indexes.
- **Computational cost:** initial generation can require full scan; later access is cheap.
- **UI:** heatmaps, diff overlays, outlier drill-down.
- **Failure modes:** misleading aggregation, expensive tile explosion, incompatible axis layouts.
- **Validation:** exact aggregation checks, visual regression, tile checksum tests.
- **MVP priority:** P0 coarse levels; advanced tiles P1.
- **Strategic value:** medium-high when integrated with query and collaboration.

### 26.5 Virtual Models and MIR

- **User problem:** intermediate checkpoints consume storage and obscure procedures.
- **Product behavior:** represent variants as immutable lazy expression graphs.
- **Implementation:** typed DAG, canonical serialization, optimizer, partial materializer.
- **Required data:** source hashes, recipes, compatibility mappings.
- **Computational cost:** low for graph creation; deferred to query or export.
- **UI:** graph canvas, affected-tensor preview, materialization status.
- **Failure modes:** deep graphs, unavailable sources, backend numerical variance.
- **Validation:** canonical hash tests, algebraic equivalence tests, reproducible builds.
- **MVP priority:** P0 simplified.
- **Strategic value:** extremely high.

### 26.6 Morph compiler

- **User problem:** merge recipes are opaque and weakly validated.
- **Product behavior:** compile transformations into deterministic block operations and validation gates.
- **Implementation:** MIR passes, backend partitioning, streaming exporter.
- **Required data:** virtual graph, alignment, tensor descriptors.
- **Computational cost:** model-size dependent.
- **UI:** recipe editor, dry run, resource estimate, warnings.
- **Failure modes:** unsupported operation combination, numerical drift, disk exhaustion.
- **Validation:** source/target invariants, sampled execution, final hash verification.
- **MVP priority:** P0 for interpolation and task vectors.
- **Strategic value:** very high.

### 26.7 Evaluation integration

- **User problem:** transformed models are exported before regressions are known.
- **Product behavior:** automatically run policy-defined checks and retain evidence.
- **Implementation:** evaluation manifest, runtime adapters, job orchestration, metric registry.
- **Required data:** model, tokenizer, datasets, runtime environment.
- **Computational cost:** potentially dominant.
- **UI:** scorecard, regression attribution, gate status.
- **Failure modes:** benchmark contamination, non-reproducible runtime, metric drift.
- **Validation:** pinned datasets, deterministic settings, independent reruns.
- **MVP priority:** P0 structural; P1 lightweight perplexity; broader suites later.
- **Strategic value:** high and commercially valuable.

### 26.8 Model alignment engine

- **User problem:** same shape does not imply same representation basis.
- **Product behavior:** generate explicit mappings and confidence before merge.
- **Implementation:** ontology, assignment algorithms, optional activation signatures.
- **Required data:** weights; calibration traces for stronger alignment.
- **Computational cost:** medium to very high.
- **UI:** alignment matrix, confidence, unmatched components.
- **Failure modes:** false confidence, dataset-dependent mapping, local optima.
- **Validation:** interpolation tests, representation metrics, downstream evaluation.
- **MVP priority:** P2 beyond exact semantic alignment.
- **Strategic value:** extremely high and technically difficult.

### 26.9 MoE Expert Atlas

- **User problem:** experts and routers are opaque and not semantically aligned across models.
- **Product behavior:** query routing, utilization, signatures, and candidate correspondences.
- **Implementation:** trace capture, sketches, embeddings, assignment, intervention runner.
- **Required data:** model weights, calibration prompts, runtime traces.
- **Computational cost:** high; inference-heavy.
- **UI:** expert map, routing flows, alignment workbench.
- **Failure modes:** semantic overclaim, calibration bias, routing instability.
- **Validation:** ablation, alternative datasets, causal interventions.
- **MVP priority:** P3.
- **Strategic value:** high research and product differentiation.

### 26.10 Enterprise governance

- **User problem:** private model variants lack approvals, audit, and license lineage.
- **Product behavior:** shared registry, policies, signed artifacts, SBOM, access control.
- **Implementation:** team control plane, RBAC, audit log, policy engine, signing.
- **Required data:** organization identity, artifact lineage, policies.
- **Computational cost:** low relative to tensor compute.
- **UI:** registry, approval queue, compliance report.
- **Failure modes:** policy misconfiguration, incomplete license metadata, key compromise.
- **Validation:** security review, audit tests, signature verification.
- **MVP priority:** enterprise phase.
- **Strategic value:** high revenue potential.

---

## 27. MVP Scope

> **Supersession:** For the concrete Phase 0 spike and acceptance criteria, use [`ARCHITECTURE.md`](../ARCHITECTURE.md) §17–§18 and [`requirements/VIZ_MVP.md`](requirements/VIZ_MVP.md). The morph/export-oriented MVP below is longer-horizon product scope, not the active engineering gate.

### 27.1 MVP objective

Prove that Quatricmorph can replace custom scripts for a common, valuable workflow:

```text
Open two related SafeTensors checkpoints
→ understand their structure
→ inspect and query numerical differences
→ create a controlled merge
→ validate it
→ export a reproducible artifact
```

### 27.2 Included

#### Ingestion

- local SafeTensors;
- sharded checkpoints;
- Hugging Face config and tokenizer metadata;
- Llama-, Mistral-, Qwen-, and Gemma-like dense decoder adapters;
- safe metadata parsing;
- mmap and streaming reads.

#### Catalog and analysis

- NSIR tensor catalog;
- architecture map;
- global and block statistics;
- approximate spectral summaries for selected tensors;
- index cache;
- tensor fingerprints.

#### Query

- metadata SELECT;
- semantic filters;
- aggregate statistics;
- aligned two-model comparison;
- explain plan;
- CLI and Python result access.

#### Visualization

- architecture tree;
- tensor table;
- Level 0–2 Tensor Tiles;
- heatmap;
- distribution view;
- cross-checkpoint difference view;
- layer ranking.

#### Morph

- linear interpolation;
- task-vector arithmetic;
- layer-specific coefficients;
- component include/exclude;
- simple LoRA application;
- Virtual Model DAG;
- dry-run and resource estimate;
- streaming SafeTensors export.

#### Verify

- file and tensor integrity;
- tokenizer identity check;
- non-finite check;
- numerical diff scorecard;
- sampled forward pass through a Python runtime adapter;
- optional small perplexity dataset;
- final manifest and hashes.

#### Interfaces

- Tauri desktop application;
- CLI;
- Python SDK;
- local daemon.

### 27.3 Explicitly excluded

- arbitrary architecture conversion;
- different-tokenizer merging;
- neuron permutation alignment;
- attention and activation capture;
- causal tracing;
- distributed execution;
- full enterprise collaboration;
- automated semantic labeling;
- expert transplantation;
- dense-to-MoE conversion;
- learned model stitching;
- evolutionary search over merge recipes;
- general training platform;
- hosted public model marketplace.

### 27.4 MVP team

A credible small team:

- one Rust systems engineer;
- one ML systems/research engineer;
- one frontend/scientific-visualization engineer;
- founder/product architect;
- part-time design and evaluation support.

### 27.5 MVP acceptance criteria

- Import at least four supported architecture families from representative public checkpoints.
- Compare two 7B–70B related checkpoints without requiring full RAM residency.
- Produce deterministic statistics and diff results across repeated runs.
- Create a Virtual Model and export a byte-valid SafeTensors checkpoint.
- Detect deliberately injected NaN, shape, tokenizer, and missing-tensor failures.
- Reproduce the exported hash from the same manifest on another machine with the same deterministic backend configuration.
- Complete the core workflow without requiring a notebook.

---

## 28. Development Roadmap

> **Supersession:** Active delivery phases are **0–6** in [`ARCHITECTURE.md`](../ARCHITECTURE.md) §17 (tiling spike → dense browser → WeightQL → WebGPU → native GPU → runtime observability → distributed). The Inspect → Query → Morph → Verify headings below are product-verb groupings and must not override those phases.

### Phase 1 — Inspect

**User value:** open a model safely and understand what it contains.

**Technical scope:**

- SafeTensors ingestion;
- shard resolver;
- NSIR ontology;
- architecture plugins;
- metadata catalog;
- global statistics;
- architecture and tensor browser.

**Dependencies:** Rust core, fixture library, local daemon, desktop shell.

**Risks:** architecture variance, custom remote-code models, incorrect ontology mapping.

**Success metrics:**

- import success rate on supported families;
- time to first architecture view;
- percentage of tensors semantically resolved;
- zero unsafe code execution.

**Deferred:** query language, runtime traces, model transformation.

---

### Phase 2 — Query

**User value:** answer reusable tensor questions without custom scripts.

**Technical scope:**

- WeightQL subset;
- typed planner;
- derived indexes;
- block engine;
- Tensor Tiles;
- query editor;
- Python and Rust APIs;
- remote range reads.

**Dependencies:** stable NSIR, statistics kernels, cache.

**Risks:** accidental full scans, index storage growth, language complexity.

**Success metrics:**

- percentage of common queries served from indexes;
- median metadata query latency;
- bytes avoided compared with full scans;
- repeat query cache hit rate.

**Deferred:** natural-language transformations, distributed execution.

---

### Phase 3 — Morph

**User value:** create reproducible model variants safely.

**Technical scope:**

- MIR and Virtual Models;
- interpolation;
- SLERP;
- task arithmetic;
- TIES;
- DARE;
- LoRA composition;
- layer slicing and passthrough;
- deterministic export.

**Dependencies:** query engine, semantic compatibility, content-addressed storage.

**Risks:** user overconfidence in merges, disk exhaustion, numerical inconsistency.

**Success metrics:**

- successful deterministic exports;
- storage saved by virtual variants;
- time from recipe to preview;
- percentage of invalid plans blocked before execution.

**Deferred:** independent-model alignment and structural stitching.

---

### Phase 4 — Verify

**User value:** know whether a transformation is structurally and behaviorally acceptable.

**Technical scope:**

- evaluation manifests;
- sampled runtime validation;
- perplexity;
- task suites;
- activation drift;
- representation similarity;
- performance profiling;
- regression gates;
- experiment records.

**Dependencies:** runtime adapters, dataset registry, job scheduler.

**Risks:** expensive evaluations, inconsistent benchmark environments, weak attribution.

**Success metrics:**

- evaluation completion rate;
- reproducibility across reruns;
- regressions caught before export;
- median evaluation setup time.

**Deferred:** automated benchmark optimization.

---

### Phase 5 — Neural Runtime Observability

**User value:** connect static weight changes to runtime behavior.

**Technical scope:**

- selective activation capture;
- attention traces;
- residual-stream analysis;
- prompt comparison;
- activation patching integration;
- sparse feature adapters.

**Dependencies:** PyTorch and inference adapters, trace storage, privacy controls.

**Risks:** enormous trace volume, framework fragmentation, interpretability overclaim.

**Success metrics:**

- trace capture overhead;
- percentage of traces queryable without full reload;
- reproducible intervention experiments;
- user-created evidence records.

**Deferred:** universal causal interpretation.

---

### Phase 6 — MoE Morph Lab

**User value:** understand and modify expert-based models.

**Technical scope:**

- router and expert ontology;
- routing traces;
- Expert Atlas;
- expert signatures;
- alignment;
- pruning simulation;
- transplantation experiments;
- router recalibration.

**Dependencies:** runtime observability, calibration datasets, assignment engine.

**Risks:** dataset-dependent conclusions, expensive evaluation, unstable router changes.

**Success metrics:**

- supported MoE families;
- expert utilization query latency;
- alignment validation quality;
- percentage of bad modifications rejected.

**Deferred:** one-click dense-to-MoE conversion.

---

### Phase 7 — Team and Enterprise

**User value:** govern model artifacts across an organization.

**Technical scope:**

- shared registry;
- RBAC;
- audit;
- approvals;
- artifact signing;
- policy packs;
- private deployment;
- distributed workers;
- model SBOM;
- organization search.

**Dependencies:** mature artifact model, identity integration, security review.

**Risks:** long enterprise sales, deployment complexity, compliance expectations.

**Success metrics:**

- governed artifacts;
- active projects;
- approval-cycle time;
- reproducible builds;
- enterprise retention and expansion.

**Deferred:** public marketplace unless strategically justified.

---

## 29. Business Model

### 29.1 Open-source core

Recommended open components:

- SafeTensors reader and model parser SDK;
- NSIR schema and ontology specification;
- WeightQL language specification;
- basic local catalog;
- basic statistics;
- basic comparison;
- Virtual Model and MIR specification;
- simple interpolation and task arithmetic;
- CLI;
- plugin SDK.

**Benefits:**

- research trust;
- architecture contributions;
- ecosystem adoption;
- reproducible paper artifacts;
- lower integration friction.

**Risks:**

- competitors can reuse core components;
- support burden;
- unclear boundary if too much is free.

The defense should be execution quality, advanced indexes, workflow integration, collaboration, governance, and accumulated compatibility knowledge—not closed file formats.

### 29.2 Professional desktop

Paid capabilities:

- accelerated indexing;
- advanced Tensor Tiles;
- GPU execution;
- richer comparison and quantization analysis;
- experiment workspaces;
- automated validation;
- advanced export;
- local multi-model search;
- premium architecture plugins.

**Benefits:** direct revenue from researchers and independent developers.

**Risks:** niche market size and high expectations for perpetual local software.

Recommended pricing may combine an annual license with optional compute credits rather than subscription-only lock-in.

### 29.3 Team and enterprise

Paid capabilities:

- shared private registry;
- access control;
- audit;
- approvals;
- signing;
- policy engine;
- model SBOM;
- distributed workers;
- air-gapped deployment;
- support and SLA.

**Benefits:** strongest willingness to pay and durable workflow embedding.

**Risks:** enterprise features can distract from core technical differentiation.

### 29.4 Hosted compute

Services:

- remote indexing;
- large scans;
- quantization;
- materialization;
- evaluation;
- merge-parameter search.

**Benefits:** monetizes expensive workflows and reduces customer setup.

**Risks:** high GPU cost, data confidentiality, queue management, competition with general compute providers.

Hosted compute should be optional and built after the local artifact model is mature.

### 29.5 Recommended sequence

```text
open-source core
→ professional local product
→ paid evaluation/compute jobs
→ team registry
→ enterprise governance
```

---

## 30. Competitive Positioning

### 30.1 Model viewers

Tools such as graph viewers are strong at architecture inspection but generally do not provide tensor-native analytical queries, virtual transformation graphs, integrated evaluation, or model-version lineage.

**Quatricmorph differentiation:** structure plus numerical analysis plus transformation plus validation.

### 30.2 Model-merging toolkits

Existing merge toolkits already support many practical merge algorithms and lazy tensor loading.

**Quatricmorph must not compete merely by reimplementing merge YAML.**

Differentiation:

- semantic ontology;
- queryable tensor database;
- compatibility and alignment evidence;
- visual transformation preview;
- virtual-model lifecycle;
- evaluation gates;
- artifact governance.

### 30.3 Interpretability libraries

Interpretability frameworks provide activation hooks and causal experiments, often inside Python notebooks.

Differentiation:

- static artifact catalog;
- cross-model lineage;
- large-scale tensor indexing;
- query language;
- persistent evidence records;
- transformation integration.

Quatricmorph should integrate rather than replace mature research libraries.

### 30.4 Experiment tracking platforms

Experiment trackers capture metrics, runs, and artifacts but generally treat model weights as opaque files.

Differentiation:

- tensor-aware diffs;
- semantic component ontology;
- virtual-model graphs;
- operation-aware validation.

### 30.5 Model registries and hubs

Registries distribute and version files but typically lack deep tensor queries and controlled transformation compilation.

Differentiation:

- inside-the-artifact analysis;
- mathematical lineage;
- reproducible derived artifacts;
- license-aware transformation graph.

### 30.6 Array databases

Array engines provide chunking, distributed access, and multidimensional queries but lack neural architecture semantics, transformation methods, and evaluation.

Quatricmorph should borrow proven array-system ideas rather than build an undifferentiated general-purpose database.

---

## 31. Product Moat

### 31.1 Easy to copy

- basic architecture tree;
- tensor metadata table;
- simple heatmap;
- linear interpolation;
- merge recipe editor;
- standard benchmark integration;
- basic model diff.

These are necessary but not defensible alone.

### 31.2 Moderately difficult

- high-quality Tensor Tiles;
- robust local out-of-core execution;
- multi-format safe ingestion;
- deterministic streaming export;
- well-designed query editor;
- integrated desktop experience.

### 31.3 Difficult and defensible

#### Cross-architecture semantic ontology

A reliable ontology requires years of architecture fixtures, edge cases, and maintenance. Its value grows with ecosystem coverage.

#### Query optimizer for tensor workloads

Efficiently choosing among metadata, indexes, range reads, CPU scans, GPU kernels, and inference is a deep systems problem.

#### Virtual-model compiler

Typed graph validation, lazy materialization, operation fusion, deterministic builds, and partial query execution are hard to implement correctly.

#### Alignment engine

Neuron, head, vocabulary, layer, representation, expert, and router alignment combine research and production engineering.

#### Model fingerprints and evidence corpus

With permission, accumulated anonymous statistics about architectures, quantization sensitivity, transformation failures, and validation patterns can improve diagnostics. Private customer weights must never be appropriated into a shared corpus.

#### Reproducible lineage and governance

Deep integration of transformations, evaluations, policy, and signing creates organizational lock-in based on workflow value rather than data captivity.

### 31.4 Strongest moat combination

```text
ontology
+ query optimizer
+ virtual-model compiler
+ validation lineage
+ alignment research
```

Visualization amplifies these assets but is not the moat.

---

## 32. Risks and Mitigations

### 32.1 Market too research-oriented

**Risk:** individual researchers have limited budgets.

**Mitigation:** use researchers for adoption, but design paid value around model-platform engineering, quantization, validation, and governance.

### 32.2 Existing merge tools are sufficient

**Risk:** users only need command-line merging.

**Mitigation:** focus on debugging, query, lineage, preview, and validation rather than algorithm count.

### 32.3 Architecture churn

**Risk:** new custom architectures appear faster than the core team can support them.

**Mitigation:** plugin SDK, ontology conformance tests, community adapters, paid certified plugins.

### 32.4 Scientific overclaim

**Risk:** visual patterns are marketed as semantic understanding.

**Mitigation:** evidence types, confidence, required causal validation, explicit limitations.

### 32.5 Trillion-parameter cost

**Risk:** indexes and scans become too expensive.

**Mitigation:** tiered index profiles, sampling, sketches, user-visible cost, remote workers, on-demand refinement.

### 32.6 Morph quality is unreliable

**Risk:** mathematically valid operations produce bad models.

**Mitigation:** compatibility classes, blocked operations, interpolation probes, validation gates, conservative defaults.

### 32.7 Evaluation cost dominates

**Risk:** users avoid integrated evaluation due to GPU expense.

**Mitigation:** layered validation, small calibration suites, early static gates, reusable evaluation caches, configurable policies.

### 32.8 Tokenizer and license complexity

**Risk:** users misinterpret automated compatibility.

**Mitigation:** hard tokenizer checks; license output labeled as policy assistance, not legal advice.

### 32.9 Desktop complexity

**Risk:** Tauri, Rust, Python, GPU backends, and WebGPU create a large support matrix.

**Mitigation:** isolate daemon protocol, ship a CPU-first core, make Python runtime optional, add accelerators incrementally.

### 32.10 Open-source monetization

**Risk:** cloud vendors commercialize the core.

**Mitigation:** open specifications and local core; monetize certified performance, advanced analysis, evaluation orchestration, collaboration, governance, and support.

### 32.11 Security of proprietary weights

**Risk:** remote processing creates confidentiality concerns.

**Mitigation:** local-first architecture, private workers, customer-controlled object storage, air-gapped deployment, no default telemetry on model contents.

---

## 33. Success Metrics

### 33.1 User-value metrics

- Median time from selecting a model to first useful architecture view.
- Median time to compare two checkpoints.
- Percentage of analyses completed without custom Python.
- Percentage of queries answered without full tensor scans.
- Percentage of invalid transformation plans blocked before materialization.
- Percentage of transformed models with reproducible manifests and evaluations.
- Time from detected regression to responsible layer or operation shortlist.
- Storage saved through Virtual Models, deltas, and deduplication.

### 33.2 Systems metrics

- Supported model size on defined workstation classes.
- Import throughput.
- Index generation throughput.
- query latency by plan tier;
- tile generation and cache hit rate;
- bytes read versus theoretical full scan;
- deterministic export success rate;
- materialization throughput;
- failure recovery rate.

### 33.3 Scientific and workflow metrics

- Number of architecture mappings with high-confidence semantic resolution.
- Reproducibility rate of morph experiments across machines.
- Correlation between static risk indicators and measured regressions.
- Alignment quality measured by interpolation barriers and downstream evaluation.
- Percentage of semantic claims with behavioral or causal evidence.

### 33.4 Commercial metrics

- Weekly active technical users running non-trivial analyses.
- Number of retained model projects per organization.
- Professional conversion among users handling models above a chosen size.
- Team workspaces with repeated validation and export workflows.
- Enterprise governed artifacts.
- Expansion from desktop to team registry and compute.

Avoid emphasizing downloads, screenshots, model count, or raw query count without evidence of completed workflows.

---

## 34. Final Recommendation

### 34.1 Product definition

Quatricmorph should be built as:

> **A local-first tensor-native analytical database, model debugger, and controlled transformation runtime for open-weight neural networks.**

The product should normalize model artifacts into a semantic intermediate representation, execute queries over tensor blocks and derived indexes, represent transformations as immutable Virtual Models, and require integrated validation before reproducible export.

### 34.2 Strategic wedge

Do not begin with general mechanistic interpretability, trillion-parameter distributed infrastructure, or MoE transplantation.

Begin with:

```text
SafeTensors
+ dense decoder models
+ semantic tensor catalog
+ out-of-core statistics
+ checkpoint comparison
+ Tensor Tiles
+ WeightQL subset
+ Virtual Models
+ interpolation and task vectors
+ deterministic validation and export
```

### 34.3 Product promise

Quatricmorph should promise:

- visibility into model structure and numerical change;
- reproducible model transformations;
- reduced storage through lazy variants;
- safer experimentation through compatibility and validation gates.

It should not promise:

- automatic understanding of learned concepts;
- safe merging of arbitrary models;
- preservation of capabilities after mathematical transformation;
- semantic expert labeling without experiments;
- lossless structural conversion.

### 34.4 Long-term direction

The long-term platform can become the control plane for open-weight model artifacts:

```text
model registry
+ tensor database
+ debugger
+ transformation compiler
+ evaluation system
+ governance layer
```

The strongest defensible path is not “better visualization.” It is a growing semantic and computational infrastructure that makes model weights inspectable, queryable, transformable, verifiable, and governable.

---

# Appendix A — Example Morph Recipe

```yaml
version: 1

sources:
  base:
    uri: hf://organization/base-model
    expected_hash: sha256:...
  math:
    uri: hf://organization/math-tuned
    expected_hash: sha256:...

compatibility:
  require:
    - same_architecture
    - same_tokenizer_contract
    - common_base_lineage

virtual_model:
  name: math-balanced-v1

operations:
  - id: math_delta
    type: task_vector
    base: base
    tuned: math

  - id: apply_math
    type: add
    input: base
    delta: math_delta
    scale:
      default: 0.20
      by_layer:
        "0:15": 0.10
        "16:31": 0.35
    select:
      exclude_roles:
        - embedding.weight
        - lm_head.weight
        - normalization.*

validation:
  gates:
    - artifact_integrity
    - finite_values
    - tokenizer_identity
    - sampled_forward
    - perplexity:
        maximum_relative_regression: 0.03

export:
  format: safetensors
  dtype: bfloat16
  deterministic: true
```

---

# Appendix B — Example WeightQL Queries

## B.1 Largest fine-tuning updates

```sql
SELECT
    a.layer_index,
    a.semantic_role,
    relative_l2(a.weight, b.weight) AS update_ratio
FROM ALIGN(model('base'), model('tuned'), BY => 'semantic_role') AS (a, b)
WHERE a.shape = b.shape
ORDER BY update_ratio DESC
LIMIT 50;
```

## B.2 Quantization damage by layer

```sql
SELECT
    layer_index,
    avg(relative_l2(fp.weight, dequant(q.weight))) AS relative_error,
    avg(cosine_similarity(fp.weight, dequant(q.weight))) AS cosine,
    avg(sqnr(fp.weight, dequant(q.weight))) AS sqnr
FROM ALIGN(model('fp16'), model('int4'), BY => 'semantic_role') AS (fp, q)
GROUP BY layer_index
ORDER BY relative_error DESC;
```

## B.3 Query a Virtual Model without full export

```sql
SELECT
    stats.mean,
    stats.stddev,
    stats.l2_norm
FROM virtual_model('math-balanced-v1').tensors
WHERE semantic_role = 'language.block.mlp.down.weight'
  AND layer_index = 24;
```

## B.4 Explain an expensive comparison

```sql
EXPLAIN
SELECT spectral_distance(a.weight, b.weight, rank => 64)
FROM ALIGN(model('a'), model('b'), BY => 'semantic_role') AS (a, b)
WHERE a.component = 'mlp';
```

---

# Appendix C — Suggested Repository Structure

```text
quatricmorph/
├── crates/
│   ├── qcatalog/
│   ├── qformats/
│   ├── qnsir/
│   ├── qweightql/
│   ├── qplanner/
│   ├── qexecutor/
│   ├── qtiles/
│   ├── qmir/
│   ├── qmorph/
│   ├── qexport/
│   └── qdaemon/
├── python/
│   ├── quatricmorph/
│   ├── runtime_adapters/
│   ├── evaluation_plugins/
│   └── research_plugins/
├── apps/
│   ├── desktop/
│   ├── web/
│   └── cli/
├── plugins/
│   ├── architectures/
│   ├── formats/
│   ├── analyses/
│   ├── transforms/
│   └── evaluations/
├── schemas/
│   ├── nsir/
│   ├── mir/
│   ├── manifest/
│   └── model-sbom/
├── fixtures/
│   ├── tiny-models/
│   ├── malformed-artifacts/
│   └── golden-queries/
└── docs/
    ├── architecture/
    ├── weightql/
    ├── plugin-sdk/
    └── security/
```

---

# Appendix D — Established and Research-Heavy Capabilities

## Established or production-feasible

- SafeTensors metadata and range-based access;
- semantic parsing for known architectures;
- block statistics and multiresolution summaries;
- checkpoint diffing;
- interpolation among closely related models;
- task-vector construction from a shared base;
- TIES- and DARE-style merge implementations;
- LoRA application and composition;
- deterministic streaming export;
- structural validation;
- evaluation harness integration;
- content-addressed lineage.

## Research-heavy

- robust alignment of independently trained large transformers;
- universal attention-head semantic alignment;
- reliable expert semantic labeling;
- expert transplantation across independently trained MoEs;
- automatic router repair;
- dense-to-MoE conversion without meaningful retraining;
- cross-width model stitching;
- semantic feature editing with guaranteed isolation;
- automatic weight repair without a known-good source;
- capability-preserving arbitrary architecture morphing.

---

# Appendix E — Research Basis

The product direction is consistent with established work on:

- model soups and weight averaging;
- task-vector arithmetic;
- TIES-style interference resolution;
- DARE-style delta sparsification;
- permutation-aware model alignment;
- representation similarity methods;
- SafeTensors and memory-mapped tensor access;
- chunked multidimensional array systems;
- high-performance Arrow-based data transport;
- reproducible language-model evaluation;
- model-merging toolkits and mechanistic-interpretability libraries.

These methods provide evidence that selected weight-space operations can be useful under constrained conditions. They do not establish that arbitrary tensor operations preserve model capability. Quatricmorph therefore treats compatibility analysis and evaluation as mandatory product layers rather than optional post-processing.
