# CURRENT_ARCHITECTURE — how today's code is put together

Scope note: `docs/CURRENT_ARCHITECTURE.md` is the **symbol-level evidence record
for `mm/`** and remains authoritative for that directory. This document covers
the **whole repository as it stands** — the Rust workspace, the three web
applications, and how the `mm` port sits inside them — and records the points
where the code diverges from `ARCHITECTURE.md`.

---

## 1. Process and module topology

```text
┌─────────────────────────── one machine, local only ───────────────────────────┐
│                                                                               │
│  fixtures/…/*.safetensors        ← Artifact Plane, never rewritten            │
│         │ mmap + byte range                                                   │
│         ▼                                                                     │
│  q-source ──── q-safetensors ──── q-architecture ──── q-nsir                  │
│   ModelSource   header/index      plugin registry     canonical address       │
│   budgets       ingest/read       generic|llama       alias grammar           │
│         │                                                                     │
│         ▼                                                                     │
│  q-catalog  (SQLite, versioned migrations)          ← Metadata Plane          │
│   models · tensors · tensor_blocks · tensor_statistics · visual_tiles · jobs  │
│         │                                                                     │
│         ├──► q-weightql ──► q-expression   (parse → resolve → shape → cost)   │
│         │                                                                     │
│         ├──► q-tensor-runtime  (Lod, BlockExtent, TensorBlock, TileId)        │
│         │        │                                                            │
│         │        ├──► q-statistics ──► q-gpu (CpuBackend = reference)         │
│         │        │                       └── q-cuda ✗ refuses                 │
│         │        └──► q-tiles (.qtile v1 ✓)        ← Tensor Tile Plane        │
│         │                                                                     │
│         └──► q-tileset ✗   q-gltf ✗                ← Visualization Plane      │
│                                                                               │
│  q-cache (L1 in-process, L2 content-addressed on disk) — built, unwired       │
│                                                                               │
│  q-daemon (axum) ─── 8 routes live, 5 return 501 with a requirement ID        │
│  q-cli    (clap)  ─── inspect · layers · tensors · value · slice · query · stats│
│         │ HTTP                                                                │
└─────────┼─────────────────────────────────────────────────────────────────────┘
          ▼
  apps/web  (npm workspaces)
   ├── quatricmorph-workspace/   Three.js, ported from mm — the only thing that renders
   ├── model-viewer/       lod-policy.ts + tile-client.ts — no renderer
   └── query-interface/    weightql.ts + katex-preview.ts + app.ts — no chat

  ✓ built and tested   ✗ refuses with a requirement ID
```

**One process today.** `q-daemon` owns the catalog, the source, and the query
engine in-process; the web applications are static Vite builds that talk to it
over HTTP. There is no worker pool, no job executor, and no separate conversion
process — which is precisely what `JOB-002` and Phase 03 introduce.

---

## 2. The four planes, as implemented

`README.md` §"The four data planes" asserts that every module declares its plane
in a top-of-file doc comment, and spot-checks confirm it (`q-tiles`, `q-gltf`,
`q-tileset`, `q-tensor-runtime` all do).

| Plane | Crates | Implemented? |
| --- | --- | --- |
| **Artifact** | `q-source`, `q-safetensors` | Yes. Read-only, mmap + range, budgets enforced |
| **Metadata** | `q-architecture`, `q-nsir`, `q-catalog`, `q-tensor-runtime`, `q-expression`, `q-weightql` | Yes, except statistics/tile rows are never written |
| **Tensor Tile** | `q-tiles`, `q-statistics` | Format yes; **generation no** |
| **Visualization** | `q-tileset`, `q-gltf` | Types and guardrails only; **both builders refuse** |

The separation is real, not documentary. `q-gltf` enforces it with tests:
`a_glb_without_a_qtile_sidecar_is_refused` makes it impossible to produce a GLB
that is the sole carrier of tensor values.

---

## 3. Key abstractions that already exist

Reusing these is cheaper than re-deriving them, and every Phase 03–07 task in
this plan is written against them by name.

| Abstraction | Location | Shape |
| --- | --- | --- |
| `ModelSource` | `crates/q-source/src/lib.rs` | `manifest()` + `read_range(uri, offset, len)`, exactly as `ARCHITECTURE.md` §4.1 specifies |
| `TensorDescriptor` | `crates/q-source/src/descriptor.rs` | Matches `ARCHITECTURE.md` §4.1 field for field, plus `linear_index` sharing SafeTensors' row-major convention |
| `AccessScale` | `crates/q-source/src/lib.rs` | Access scale is a **type**, not a comment (`SRC-018`): metadata scale cannot read payload; visualization scale is never exact |
| `Lod` | `crates/q-tensor-runtime/src/lib.rs:35` | Closed 6-variant enum; `carries_exact_values()` true only at the finest level |
| `BlockExtent`, `TensorBlock`, `SourceByteRanges` | `crates/q-tensor-runtime` | Block planning derives **one byte run per row** without reading |
| `TileId` | `crates/q-tensor-runtime/src/lib.rs:261` | 16 bytes, stable, sensitive to extent and LOD |
| `QTileHeader` / `QTile` | `crates/q-tiles/src/lib.rs` | 72-byte header, magic `QTILE\0\0\0`, v1, little-endian, 256 MiB payload ceiling |
| `Backend` | `crates/q-gpu/src/lib.rs:73` | The compute seam. `CpuBackend` is the declared reference; `CudaBackend` implements the same trait and refuses |
| `CacheKey` | `crates/q-cache/src/lib.rs:47` | `ARCHITECTURE.md` §13.2 key, length-prefixed so field boundaries cannot collide |
| `GridRuler3D` | `apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts:289` | Ten parameters, snap invariant at `1e-6`, `assertVecSnapped` at layout boundaries |
| `TensorBlockSource` | `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts:53` | Interface with `HandEnteredSource` (works) and `DaemonBlockSource` (refuses) |
| `Expr` | `crates/q-expression/src/lib.rs` | Closed enum — the reason no `eval` exists anywhere in the product |

---

## 4. The matrix workspace, after the `mm` port

`apps/web/quatricmorph-workspace/src/` is 49 TypeScript modules outside
`__tests__/`, spread over ten directories plus five at the root
(`find apps/web/quatricmorph-workspace/src -name '*.ts' -not -path '*__tests__*' | wc -l`).
The eight directories in the table below carry the port's substance; `examples/`
and `ref/` hold `mm` demo entry points and `assets/` holds a font, no TypeScript.
The port's central achievement is that **pure math is separated from Three.js
scene state**, which is what makes real tensor blocks possible at all.

| Directory | Modules | Tested? |
| --- | --- | --- |
| `math/` | `matmul`, `blocking`, `animation-schedule`, `shape`, `validate`, `parse`, `presets` | 34 tests, no Three.js import |
| `layout/` | `grid-ruler`, `tensor-frame` | 13 tests |
| `viz/` | `mat`, `matmul`, `array2d`, `material`, `sizing`, `epilog`, `init`, `layout`, `expr`, `constants`, `defaults` | 13 tests |
| `interaction/` | `selection`, `animation` | 6 tests |
| `app/` | `create-app`, `scene`, `url`, `default-params`, `instructions` | — |
| `gui/` | `research-gui` (faithful), `mvp-gui` (reduced) | — |
| `tensor/` | `block-adapter` | 5 tests |
| `util/` | `params`, `objects`, `geometry`, `text` | 3 tests |

**How cells are drawn today.** `viz/mat.ts:51` allocates a `THREE.Points` cloud
via `emptyPoints`; `viz/material.ts:4` loads `/assets/ball.png` into a single
shared `ShaderMaterial` whose vertex shader sizes points by
`mag * pointSize / -mvPosition.z`. Value → size is `Mat.sizeFromData`
(`mat.ts:110`); value → colour is `Mat.colorFromData` (`mat.ts:144`), HSL with a
configurable zero hue, hue gap, and hue spread.

Three consequences for the product requirement that cells be **sphere blocks**
whose *size, colour, and opacity* encode the value:

1. Round sprites already read as spheres. The visual target is close.
2. **There is no opacity channel.** Nothing in `mat.ts` or `material.ts` drives
   alpha from data. This is a genuine gap and a shader change (`QM-0063`).
3. Point sprites are camera-facing quads, not geometry. They cannot be
   occlusion-sorted or lit, and their screen size is distance-derived rather than
   grid-derived — which is in tension with the grid invariant at close range.
   Whether to keep sprites or move to `InstancedMesh` spheres is
   `ADR-CANDIDATE-015`, with a measurement task rather than a guess.

---

## 5. Security posture, as built

| Control | Where | Evidence |
| --- | --- | --- |
| No arbitrary code execution (Rust) | `q-weightql` parser over a closed `Expr` enum | `arbitrary_code_execution_constructs_are_rejected` |
| No arbitrary code execution (browser) | `apps/web/query-interface/src/weightql.ts` | `rejects_arbitrary_code_execution_constructs` |
| `mm`'s `eval` path not carried forward | Absence in `apps/web/` | `SEC-004`, `ADR-006` |
| Path traversal refused | `q-source` local + daemon roots | `path_traversal_is_refused`, `a_traversal_attempt_never_escapes_a_root` |
| No SQL injection surface | `q-catalog` binds every caller value; only enum-derived `&'static str` is interpolated | `SEC-005` |
| Header allocation bomb refused | `absurd_header_length_is_refused_before_allocating` | `SRC-013` |
| Whole-tensor reads refused | `q-weightql` plan | `whole_tensor_reads_are_refused_with_an_explanation` |

**Gaps:** no daemon origin policy or CORS decision (`SEC-007`); KaTeX renders
user text with no stated sanitization contract (`SEC-006`); no request rate or
concurrency limit on the daemon.

---

## 6. Recorded divergences from `ARCHITECTURE.md`

These are **open questions**, not plan errors. Each has an ADR candidate and a
task. `ARCHITECTURE.md` is not edited by this plan.

### 6.1 Operand plane mapping — verified divergence

`ARCHITECTURE.md` §8.2 states:

```text
A: XY plane      B: YZ plane      C: XZ plane
```

`apps/web/quatricmorph-workspace/src/layout/grid-ruler.ts:9-10` states, and implements:

```text
World X → J (output cols), Y → I (output rows), Z → K (contraction)
A on I×K,  B on K×J,  C on I×J
```

Resolving the code's mapping through its own axis assignment: A spans I (Y) and
K (Z) → **A is the YZ plane**; B spans K (Z) and J (X) → **B is XZ**; C spans I
(Y) and J (X) → **C is XY**. That is the exact opposite assignment from §8.2 for
A and C.

The task specification §16 independently states `World X → J, Y → I, Z → K` with
`A → I×K, B → K×J, C → I×J` — agreeing with the code, not with §8.2.

Two independent sources agree against one. **Decided: keep the code's mapping and
correct `ARCHITECTURE.md` §8.2** —
[`ADR-009`](../docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md),
accepted 2026-08-04; correction scheduled as `QM-0090`. Changing the code instead would invalidate 13 passing
`grid-ruler` tests and the proven `mm` placement semantics for no gain.

### 6.2 Catalog technology

`ARCHITECTURE.md` §5 and §2.1 name DuckDB / Arrow / Parquet. The implementation
is SQLite (`rusqlite`, bundled). Already recorded in
`docs/decisions/ADR-003-catalog-sqlite.md` and tracked as `CAT-010` (Not
Started). **This plan keeps SQLite for the MVP** — the catalog's workload is
point lookups and small hierarchy queries, which is SQLite's shape, and
`CAT-006` already proves it scales to a 10¹²-parameter manifest in 35.7 MB.

### 6.3 Repository root

`ARCHITECTURE.md` §16 shows a `quatricmorph/` top directory. The workspace is at
the repository root, and `apps/desktop/` does not exist (correctly — Tauri is a
non-goal). Recorded in `docs/decisions/ADR-001-workspace-at-repository-root.md`.
No action.

### 6.4 Duplicated LOD authority

Not a divergence from `ARCHITECTURE.md` but an internal inconsistency:
`q_tensor_runtime::Lod`, `q_tileset::GeometricError::for_lod`, and
`apps/web/model-viewer/src/lod-policy.ts` each define part of the ladder, with
the TypeScript copy hand-mirrored. See [`REPOSITORY_ANALYSIS.md`](REPOSITORY_ANALYSIS.md)
§5. Closed by `QM-0004` + `QM-0005`.

### 6.5 `ARCHITECTURE.md` §12.1 renderer stack

§12.1 lists "React or Svelte" for the Cesium prototype. `apps/web/` uses plain
TypeScript with Vite and no framework. `ADR-CANDIDATE-010` decides whether the
viewer adopts a framework; **recommended default is no framework** for the MVP,
since the viewer's state is a handful of selections and a camera, and adding one
would be the largest new dependency in the repository after Cesium itself.

---

## 7. What the current architecture makes easy, and what it makes hard

**Easy**, because the seam already exists:

* Swapping CPU for CUDA — both implement `q_gpu::Backend`
* Adding an architecture family — `architectures/*/plugin.toml` + registry
* Adding a cache tier — `CacheTier` trait, `LayeredCache` composes
* Adding a WeightQL node — closed enum, one place to extend, shape check follows
* Persisting statistics — the table and the accumulator both exist, unconnected

**Hard**, and therefore where the plan spends its tasks:

* **Producing any visual artifact at all.** The pipeline from `TensorBlock` to
  `.qtile` to GLB to `tileset.json` has never run end to end. Four tasks.
* **Making the viewer real.** No Cesium dependency, no scene, no picking path.
  Eight tasks.
* **Keeping one spatial system across two languages and three packages.** Two
  Phase 00 tasks, then a standing conformance test.
* **Executing anything numeric beyond a slice read.** `WQL-006` needs a compute
  path, a cost gate, and a cancellation story.
* **Job execution.** Everything about jobs exists except something that runs one.
