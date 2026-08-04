# TEST_STRATEGY

## 0. Baseline

**391 tests pass**, verified by running both suites at commit `5ca434d`:

```bash
cargo test --workspace          # 290 passed; 0 failed      exit 0
cd apps/web && npx vitest run   # 101 passed (12 files)     exit 0
```

`docs/TESTING.md` records the existing strategy and this plan extends it rather
than replacing it. Three properties of the current suite are worth preserving
deliberately, because each is easy to lose:

1. **No test touches the network.** Fixtures are checked in, and CI regenerates
   them and diffs to prove they are reproducible from
   `fixtures/generate_fixtures.py`.
2. **Expected values are computed by hand or by an independent implementation**,
   not by the code under test. `hand_computed_moments_on_a_small_fixture`,
   `hand_computed_histogram_binning`, `hand_computed_cosine_similarity_and_relative_l2`;
   and `golden.json` comes from Python `safetensors==0.8.0`.
3. **Test names are assertions.** `the_builder_refuses_rather_than_emitting_a_fake_tileset`
   tells you the requirement without opening the file.

---

## 1. Test levels

| Level | Where | Runs in |
| --- | --- | --- |
| Unit | `#[cfg(test)]` in each crate; `__tests__/` in each web package | Every commit |
| Crate integration | `crates/*/tests/` | Every commit |
| Cross-crate integration | `tests/tests/` | Every commit |
| Contract conformance | Rust **and** vitest over one golden vector | Every commit |
| Artifact validation | External validators over generated output | CI job |
| End-to-end | Full pipeline over a fixture | CI job |
| Soak | Memory and handle growth over many iterations | Nightly / pre-release |
| Hardware | Metal differential and leak tests (v1); CUDA differential and leak tests (next step, post-v1) | Apple GPU (v1) / **RTX 3090 only** (post-v1) |
| Manual | Visual and interaction checklist | Pre-release |

---

## 2. By subsystem

### 2.1 SafeTensors — 18 requirements, 17 verified

Covered: single file; sharded; offsets; dtypes including f32/bf16/f16 subnormals;
corruption; stable IDs across reopen; exact scalar and slice lookup; cancellation;
resume; **no full-checkpoint allocation** (`ingestion_reads_only_headers_not_payload`,
`the_whole_slice_reads_a_negligible_fraction_of_the_checkpoint`).

Gap: `SRC-008` HTTP transport — range arithmetic verified, transport is an
extension point.

**Add** (`QM-0003`): the same suite against a fixture with a 4096×4096 tensor, to
exercise block decomposition at a realistic scale.

### 2.2 Trillion-scale metadata — verified

`crates/q-catalog/tests/trillion_scale_manifest.rs`: 47 278 tensors,
1.048×10¹² parameters, 2.10 TB described, **35.7 MB peak allocation**, no artifact
opened.

**Add** (`QM-0013`): promote the synthetic manifest generator to a reusable
fixture tool; assert peak allocation stays under a named budget rather than a
literal, so the assertion survives an intentional budget change.

### 2.3a Metal — v1's hardware-gated GPU lane

v1 runs its differential and device-memory GPU tests against **Metal on Apple
GPU hardware**, not CUDA. The same shapes as §2.3 below apply — min/max, mean,
variance, norms, ratios, histogram, quantization/Morton, block matmul, OOM
block-halving and retry, cancellation, and a device-memory soak — verified
against `CpuBackend` at analogous tolerances (`ADR-CANDIDATE-003`,
`.plan/CUDA_ARCHITECTURE.md` §12). This is the test suite that actually runs
in v1, on the development/target machine (Apple silicon).

### 2.3 CUDA — hardware-gated (next step, post-v1)

Not exercised in v1. Kept as the test plan for the deferred CUDA next step,
once RTX 3090 access is available.

| Test | Hardware |
| --- | --- |
| VRAM ceiling arithmetic; every operation refuses | none — passing today |
| `nvcc` compiles all four `.cu` files | toolkit only |
| Min/max, mean, variance, norms, ratios vs `CpuBackend`, f32/f16/bf16 | **RTX 3090** |
| Histogram bin counts — **exact** vs CPU | **RTX 3090** |
| Quantization and Morton — **exact** vs CPU | **RTX 3090** |
| Block matmul — rel `1e-5` vs CPU | **RTX 3090** |
| Block dimensions 64, 128, 256, 512 | **RTX 3090** |
| Out-of-memory halves the block and retries | **RTX 3090** |
| Cancellation between blocks frees device memory | **RTX 3090** |
| 10 000-job device-memory soak | **RTX 3090** |

Tolerances in [`CUDA_ARCHITECTURE.md`](CUDA_ARCHITECTURE.md) §6. **Until an RTX
3090 runs these, the requirements stay `Hardware-Unverified` and the tasks stay
`Implemented`.** None of this blocks v1, which ships on the Metal suite above.

### 2.4 Tile generation

Covered: `.qtile` round trip byte-for-byte; exact f32 preservation; 8 corruption
classes; little-endian on any host; quantized tiles declare themselves lossy;
geometric error halves; a non-refining child is rejected; GLB refuses
cube-per-weight and refuses to be the only carrier of values.

**Add:** pyramid generation over a real tensor (`QM-0041`); GLB validated by the
Khronos `gltf-validator` (`QM-0046`); `tileset.json` validated against the
published 3D Tiles schema (`QM-0046`); stable feature IDs across regeneration
(`QM-0043`); bounds containment parent ⊇ children (`QM-0040`); resume skips
completed blocks (`QM-0045`); atomic output — a killed job leaves no file under a
final name (`QM-0045`); cache reuse skips recompute (`QM-0032`).

### 2.5 Cesium viewer

Covered: LOD policy — camera movement alone never reads exact values; exact reads
only on explicit selection; 501 treated as a declared gap.

**Add:** tileset opens and renders (`QM-0051`); refinement by camera distance
(`QM-0052`); pick → correct canonical address (`QM-0053`); missing tile shows a
marker and does not break siblings (`QM-0051`); corrupted tile fails alone;
camera fit and reset; selection survives a reload via URL state (`QM-0056`);
**browser memory does not grow across 100 model switches** (`QM-0082`); full
disposal (`QM-0056`).

Cesium in vitest needs a WebGL context. `ADR-CANDIDATE-013`: unit-test the
policy and address logic headlessly (as today), and cover render and pick with a
**Playwright** job that runs a real browser. A mocked WebGL context would test the
mock.

### 2.6 Matrix workspace — 74 tests today

Required shape matrix, all covered by `math/__tests__/matmul.test.ts`:

```text
2×3 @ 3×2 → 2×2      3×3 @ 3×1 → 3×1      1×3 @ 3×2 → 1×2
1×3 @ 3×1 → 1×1      1×1 @ 1×1 → 1×1      2×3 @ 2×2 → validation error
```

Also covered: negative values, zeros, decimals; grid alignment; the snap
invariant and its documented tolerance; deterministic stepping; URL round trip.

**Add:** selected real blocks from the daemon (`QM-0066`); hover metadata
completeness (`QM-0068`); vectors and scalars framed on the same grid
(`QM-0065`); sphere size/colour/opacity mapping including `v = 0` keeping its
cell (`QM-0063`); the sphere budget degrading rather than truncating (`QM-0064`);
reset and re-initialization; **disposal disposes materials and textures, not only
geometries** (`QM-0082`) — a recorded `mm` defect.

### 2.7 WeightQL — 12 requirements, 10 verified

Covered: canonical address; alias; ambiguity returns candidates; slice;
transpose; matmul planning; shape mismatch rejected before execution; cost
estimate; whole-tensor refusal; invalid syntax; no arbitrary execution;
deterministic plan IDs.

**Add:** matmul execution vs CPU reference (`QM-0070`); stacked slices
(`QM-0071`); `GROUP BY layer_index` (`QM-0072`); cancellation mid-execution
(`QM-0073`); a resource-limit rejection carrying its threshold (`QM-0073`);
**grammar conformance between the Rust and TypeScript parsers over one shared
corpus** (`QM-0074`).

That last one matters because two hand-written parsers for one grammar
(`ADR-005`) will drift, and drift means the preview shows something different
from what executes.

### 2.8 Contract conformance — new

`QM-0005`, gate **G1**. One golden vector at
`schemas/visualization/golden-spatial.json`, asserted by both languages:
geometric error per LOD; LOD per distance; cell centres; and the load-decision
table that mechanises `AC-006`. If either language changes a rule, one suite goes
red.

---

## 3. End-to-end demonstration

`QM-0080`, gate **G4**. The task specification §32 sequence, as one CI job:

```text
1. open the SafeTensors fixture
2. import metadata                        → catalog rows, bounded memory asserted
3. convert a selected tensor hierarchy    → job runs, checkpoints, completes
4. generate .qtile, GLB, tileset.json     → all three validate externally
5. open in CesiumJS                       → tileset loads and renders (Playwright)
6. select a tensor block                  → resolves to the correct address
7. retrieve one exact value               → 4 bytes read
8. verify against Python safetensors      → equals golden.json
9. assign blocks to the matrix workspace  → grid-aligned, bounded
10. visualize A @ B                       → result equals the CPU reference
11. query the selection through chat      → produces a plan with a cost
```

Steps 1, 2, 7, 8 already pass in `tests/tests/end_to_end_scalar_slice.rs` — 4
tests over 6 golden scalars, 2 golden slices, and 2 bf16 tensors.

**This runs on a machine with no NVIDIA GPU.** If it needed CUDA it would not
run in CI, and an end-to-end test that does not run is not a test.

---

## 4. Soak tests

`QM-0082`, `QM-0083`. Nightly, not per-commit.

| Soak | Iterations | Assertion |
| --- | --- | --- |
| Browser: model switch | 100 | JS heap returns to within 10 % of baseline |
| Browser: workspace re-init | 100 | `renderer.info.memory` geometries and textures return to baseline |
| Daemon: query loop | 10 000 | RSS stable; no file-descriptor growth |
| CUDA: block jobs | 10 000 | `cudaMemGetInfo` free returns to its start value — **RTX 3090** |
| Conversion: kill and resume | 50 | No orphaned `.tmp` files; no duplicate work; output identical to an uninterrupted run |

The last one is the strongest statement the pipeline can make: an interrupted
conversion and an uninterrupted one produce byte-identical artifacts.

---

## 5. Manual checklist

Some things cannot be asserted. `QM-0094` runs this before release, with
screenshots as evidence.

* Zoom model → layer → tensor → block; refinement is smooth and monotone
* Zooming out visibly loads nothing exact (dev panel request log)
* Click a cell; the address matches what `q-cli value` returns for that index
* Fidelity badges are legible and correct at every level
* A sampled tile never *looks* like exact data
* Play / pause / step / previous / reset behave; previous is exactly forward's inverse
* Grid lines are aligned, labelled, and toggle independently
* Spheres do not overlap at any zoom
* A zero cell is visibly present, not absent
* Selection is legible with colour removed (greyscale screenshot)
* Ambiguous alias shows candidates and does not choose
* Cost preview appears before an expensive query
* Cancel stops work within one block
* **The browser console is empty** (`AC-043`)

---

## 6. CI

Current jobs — `rust` (fmt, clippy, build, test), `fixtures` (regenerate and
`git diff --exit-code`), `web` (vitest + a `matrix-workspace` build uploaded as an
artifact). Deliberately **no CUDA job**, with a comment explaining that a job
which "passed" without the hardware would be worse than none.

**Add:**

| Job | Runs | Task |
| --- | --- | --- |
| `contract` | Both conformance suites over the golden vector | `QM-0005` |
| `artifacts` | Generate from the fixture; validate with `gltf-validator` and `3d-tiles-validator` | `QM-0046` |
| `e2e` | The §32 sequence, headless browser | `QM-0080` |
| `soak` | Nightly, not per-commit | `QM-0082` |
| `cuda` | **Only when a self-hosted RTX 3090 runner exists** | `QM-0035` |

---

## 7. Rules that hold across the plan

1. **Expected values are computed independently.** By hand, or by Python
   `safetensors`, or by the CPU reference — never by the code under test.
2. **A refusal is tested as carefully as a success.** Every stub has a test that
   it refuses *with its requirement ID*. This is why `STATUS.md` can be trusted.
3. **A test name is an assertion.** If it needs a comment to explain what it
   checks, it is misnamed.
4. **No network in the default suite.** Fixtures are checked in; CI proves they
   are reproducible.
5. **Hardware-gated tests are labelled, not skipped silently.** A skipped test
   that reports success is how `Hardware-Unverified` becomes a false `Verified`.
6. **A performance number does not exist until it is measured** on the hardware it
   claims.
