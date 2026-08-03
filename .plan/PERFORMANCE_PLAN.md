# PERFORMANCE_PLAN

## 0. The rule

**No performance number appears in any Quatricmorph document until it has been
measured on the hardware it claims.**

The repository already holds itself to this. `gpu/cuda/README.md` says of its own
kernels: *"Treat every performance or numerical claim below as an intention, not
a measurement."* `STATUS.md` marks them `Hardware-Unverified` with the test column
reading *"none — never compiled or executed"*.

This document therefore contains **budgets and measurement plans**, and exactly
one measured number.

---

## 1. The one measured number

| Measurement | Value | Source |
| --- | --- | --- |
| Trillion-parameter manifest indexing | **35.7 MB peak allocation** for 47 278 tensors describing 1.048×10¹² parameters and 2.10 TB of payload — a 56 040:1 ratio, opening no artifact | `crates/q-catalog/tests/trillion_scale_manifest.rs`, `CAT-006`, `Verified` |

Everything else below is a budget to be met, not a result to be quoted.

---

## 2. Budgets

Each has an owning task that measures it and records the result in that task's
`Completion Evidence`. Budgets are **provisional until first measurement**; a
budget that turns out to be wrong is corrected in the plan, not quietly missed.

### 2.1 Ingestion

| Metric | Budget | Task |
| --- | --- | --- |
| Header read per shard | ≤ 100 KB | ✓ already asserted |
| Metadata import, 111-tensor fixture | < 100 ms | `QM-0001` |
| Metadata import, 47 278-tensor synthetic manifest | < 30 s, peak < 64 MB | `QM-0013` |
| Peak RSS during import | O(tensor count), never O(bytes) | ✓ `SRC-007` |

### 2.2 Exact reads

| Metric | Budget | Task |
| --- | --- | --- |
| Scalar read | **exactly `dtype_width` bytes** | ✓ `scalar_read_touches_only_dtype_width_bytes` |
| 256×256 f32 slice | exactly 256 KiB, in ≤ 256 runs | ✓ `TILE-002` |
| Latency, warm page cache | < 5 ms | `QM-0030` |
| Latency, cold | < 50 ms on NVMe | `QM-0030` |

### 2.3 Conversion (CPU backend)

| Metric | Budget | Task |
| --- | --- | --- |
| Statistics, one 256×256 f32 block | < 2 ms | `QM-0031` |
| Full pass, 4096×4096 f32 (256 blocks) | < 5 s single-threaded | `QM-0031` |
| Peak RSS during that pass | **< 32 MB** — see [`MEMORY_BUDGET.md`](MEMORY_BUDGET.md) §9, which predicts ≈ 12 MB | `QM-0031` |
| `.qtile` write, quantized 256×256 | < 1 ms | `QM-0041` |
| GLB write, 65 536 instances | < 50 ms | `QM-0042` |
| `tileset.json`, 1 000 nodes | < 100 ms | `QM-0044` |

The peak-RSS budget is the load-bearing one. It must not grow with tensor size —
only the block count may grow. `QM-0031` asserts it as a test, not a benchmark,
because a regression here breaks the architecture's premise rather than merely
making it slow.

### 2.4 CUDA — unmeasured

| Metric | Budget | Task | Hardware |
| --- | --- | --- | --- |
| Reduction on 256×256 f32 | speedup ≥ 1× vs CPU | `QM-0035` | RTX 3090 |
| Reduction on 4096×4096 f32 | speedup ≥ 10× vs CPU | `QM-0035` | RTX 3090 |
| Host→device, 1 MiB pinned | ≥ 8 GB/s | `QM-0035` | RTX 3090 |
| Block matmul 512³ | ≥ 1 TFLOP/s f32 | `QM-0036` | RTX 3090 |
| Peak device memory, defaults | ≤ 2.5 GiB | `QM-0036` | RTX 3090 |

The 1× floor on small blocks is deliberate and expected to be tight: a 256×256
block is 256 KiB, and PCIe round-trip latency may exceed the CPU's compute time
entirely. **If the measurement shows CUDA is slower on small blocks, the finding
is recorded and the backend selector uses a size threshold** — it is not a
failure, it is the reason the threshold exists.

### 2.5 Viewer

| Metric | Budget | Task |
| --- | --- | --- |
| Time to first render, 1 000-tile tileset | < 2 s | `QM-0051` |
| Frame time while navigating | < 16 ms at 256 loaded tiles | `QM-0052` |
| Tile request latency, local daemon | < 20 ms | `QM-0051` |
| Pick → resolved address | < 50 ms | `QM-0053` |
| JS heap, 256 loaded tiles | < 600 MB | `QM-0082` |
| Heap after 100 model switches | within 10 % of baseline | `QM-0082` |

`requestRenderMode: true` means frame time matters only while something is
changing. A static model should cost approximately nothing, and a fan that spins
on a still scene is a bug.

### 2.6 Matrix workspace

| Metric | Budget | Task |
| --- | --- | --- |
| Render 65 536 spheres | < 100 ms initial, < 16 ms per frame | `QM-0063` |
| Render 262 144 spheres (ceiling) | < 400 ms initial, < 33 ms per frame | `QM-0064` |
| Block fetch + decode, 256×256 | < 200 ms | `QM-0066` |
| In-browser matmul, 256³ | < 200 ms | `QM-0067` |
| In-browser matmul, 512³ | < 1.5 s → above this, delegate to the daemon | `QM-0067` |
| Animation step | < 16 ms | `QM-0067` |
| Heap after 100 re-inits | within 10 % of baseline | `QM-0082` |

The 512³ threshold is where the delegation rule comes from: it is the point at
which a single-threaded JS matmul stops feeling interactive.

### 2.7 Query

| Metric | Budget | Task |
| --- | --- | --- |
| Parse + plan | < 10 ms | `QM-0073` |
| Alias resolution against the catalog | < 20 ms | `QM-0073` |
| Scalar query, end to end | < 30 ms | ✓ measurable today |
| Cancellation latency | < one block | `QM-0073` |

---

## 3. Benchmark harness

`QM-0084`. Reproducible, not anecdotal.

* **Rust**: `criterion` benches in the crates that own the work, gated behind a
  `bench` feature so the default `cargo test` stays fast.
* **Browser**: `performance.mark`/`measure` around render, fetch, decode, and
  matmul, collected by the Playwright job.
* **Reporting**: JSON per run, checked into `benchmarks/` with the commit SHA,
  machine description, and every configuration variable from
  [`MEMORY_BUDGET.md`](MEMORY_BUDGET.md) §11 — because a number without its
  budgets is not reproducible.

Every benchmark records: hardware, OS, commit, dtype, block dimensions, backend,
iterations, mean, median, p95, and peak memory. A single mean is not a
measurement.

---

## 4. Scaling

What must stay flat as the model grows — the properties that make trillion-scale
metadata support real rather than rhetorical.

| Quantity | Must scale as | Verified |
| --- | --- | --- |
| Import peak memory | O(tensor count) | ✓ `CAT-006` |
| Catalog query time | O(result rows), indexed | ✓ `CAT-003`, `CAT-005` |
| Conversion peak memory | O(block size × concurrency) — **independent of tensor size** | `QM-0031` |
| Conversion time | O(bytes converted) | `QM-0031` |
| Tileset node count | O(tiles), and tiles are O(blocks) | `QM-0044` |
| Viewer memory | O(loaded tiles), bounded by `MAX_LOADED_TILES` | `QM-0082` |
| Browser query result | O(requested elements), capped | ✓ `WQL-011`, `GRID-005` |

**Nothing may scale as O(model bytes) in memory.** That is the single scaling
invariant the architecture rests on, and every row above is a way of restating
it.

`QM-0084` measures import time and peak memory at 10³, 10⁴, and 10⁵ tensors and
asserts the curve is linear in tensor count with a bounded constant.

---

## 5. Known risks

| Risk | Concern | Mitigation |
| --- | --- | --- |
| CesiumJS bundle size | ~3 MB gzipped; largest dependency in the repository | Measure in `QM-0050`; tree-shake; lazy-load the viewer route. `ADR-CANDIDATE-010` |
| 262 144 spheres in Three.js | May not hold 30 fps on integrated GPUs | Measure at both 65 536 and 262 144; degrade per `GRID-010` |
| SQLite write throughput during conversion | Row-per-block inserts could dominate | Batch in transactions; measure in `QM-0031` |
| Small-block CUDA transfer overhead | May exceed the compute it accelerates | Measure; use a size threshold in backend selection |
| Browser `.qtile` decode on the main thread | Would stall animation | Decode in a Web Worker — designed in from the start |
| Cesium `requestRenderMode` with animation | Continuous render defeats the optimization | Request render on animation frames only while playing |
