# MEMORY_BUDGET — every buffer, as a formula

## 0. Principle

**Every budget is a formula over a named configuration variable.** Not a fixed
promise, because a fixed promise about memory is a promise about a machine, a
model, and a workload that the author did not have.

The existing code already works this way: `q_source::budget` defines named,
enforced budgets and `a_tight_metadata_budget_is_enforced` proves the enforcement
is real rather than advisory.

Symbols used throughout:

```text
W      dtype width in bytes           f32 = 4, f16/bf16 = 2, f64 = 8
Br,Bc  block rows, block columns      default 256 × 256
E      block elements = Br × Bc       default 65 536
N      concurrent blocks in flight    default 4
T      tensor rows × columns
P      model parameter count
```

---

## 1. The claim this document has to support

`CAT-006` is `Verified`: a manifest of **47 278 tensors describing 1.048×10¹²
parameters and 2.10 TB of payload** indexes and queries with **35.7 MB of peak
allocation** — a 56 040:1 ratio — while opening no artifact.

That is the whole architecture in one measurement. Every budget below exists to
keep it true as the pipeline grows.

---

## 2. Metadata scale

Reading headers, never payload. Enforced by type: `AccessScale::Metadata` cannot
read payload (`SRC-018`, `metadata_scale_never_reads_payload`).

```text
header_bytes(shard)   ≈ 8 + JSON header length          # typically 10–100 KB
manifest_memory       ≈ tensor_count × sizeof(TensorDescriptor)
                      ≈ tensor_count × ~200 B
catalog_query_memory  = O(result rows), never O(model)
```

| Variable | Default | Enforced by |
| --- | --- | --- |
| `MAX_HEADER_BYTES` | 64 MiB | `absurd_header_length_is_refused_before_allocating` |
| `MAX_METADATA_BYTES` | 256 MiB | `a_tight_metadata_budget_is_enforced` |

At 47 278 tensors: ≈ 9.5 MB of descriptors. Measured peak 35.7 MB including
SQLite and JSON parsing. **Independent of the 2.10 TB the manifest describes.**

---

## 3. Source read buffers

```text
block_read_bytes  = Br × Bc × W                     = 256 KiB   (256×256 f32)
byte_runs         = Br                              = 256 runs
run_bytes         = Bc × W                          = 1 KiB
row_stride        = tensor_columns × W
```

`TensorBlock::plan` derives one byte run per row **without reading**. For a
256-column window of a 4096-column f32 tensor: 256 runs of 1 KiB at a 16 KiB
stride — 256 KiB of I/O rather than the 4 MiB a naive row-span read would cost.

| Variable | Default | Note |
| --- | --- | --- |
| `MAX_SOURCE_BUFFER_BYTES` | 64 MiB | Ceiling across all in-flight reads |
| `MMAP_THRESHOLD_BYTES` | 1 MiB | Below this, `pread` beats mapping |

mmap does not count against RSS the way a heap allocation does; the kernel evicts
pages under pressure. This is why local reads are mapped and remote reads are not.

---

## 4. Decoded host blocks

```text
decoded_block_bytes = E × 4                         # always f32 after decode
host_staging_bytes  = N × decoded_block_bytes       = 4 × 256 KiB = 1 MiB
```

| Variable | Default | Formula |
| --- | --- | --- |
| `MAX_HOST_STAGING_BYTES` | 512 MiB | ≥ `N × E × 4` |
| `MAX_CONCURRENT_BLOCKS` | 4 | ≤ `MAX_HOST_STAGING_BYTES / (E × 4)` |
| `MAX_OUTPUT_QUEUE_DEPTH` | 64 | Backpressure onto the reader |

**Backpressure, not buffering.** When the output queue is full the reader blocks.
Growing the queue instead would convert a throughput problem into an
out-of-memory crash, which is strictly worse because it destroys the completed
work that a stalled pipeline preserves.

---

## 5. GPU buffers

Per [`CUDA_ARCHITECTURE.md`](CUDA_ARCHITECTURE.md) §3.

```text
gpu_input_bytes  = N × E × 4                        = 1 MiB   at defaults
gpu_output_bytes = N × (sizeof(Stats) + E × 2)      ≈ 512 KiB at defaults
gpu_total        = gpu_input_bytes + gpu_output_bytes + kernel_scratch

usable_vram      = min(free_vram × USABLE_VRAM_FRACTION,
                       MAX_GPU_INPUT_BYTES + MAX_GPU_OUTPUT_BYTES)
```

| Variable | Default | Note |
| --- | --- | --- |
| `RTX_3090_VRAM_BYTES` | 24 GiB | Constant, already defined |
| `USABLE_VRAM_FRACTION` | `0.80` | Already defined |
| `MAX_GPU_INPUT_BYTES` | 2 GiB | ≈ 10 % of the usable ceiling |
| `MAX_GPU_OUTPUT_BYTES` | 512 MiB | Outputs are ~2 orders smaller |
| `MAX_PINNED_BYTES` | 128 MiB | Pinned memory degrades the whole system, not just this process |

At defaults the GPU holds **1.5 MiB** of a model that may be terabytes. That
ratio is the design, not an accident of small defaults.

**Adaptive sizing:** on allocation failure, halve `Br` and `Bc`, retry, down to
64×64. Below that, fail naming the budget that could not be met.

---

## 6. Writer buffers

```text
qtile_payload_bytes = E × bytes_per_cell            # 4, 2, or 8
                    = 256 KiB | 128 KiB | 512 KiB   at 256×256
qtile_buffer        = header (72 B) + payload

glb_instance_bytes  = 12 (translation) + 12 (scale) + 4 (feature id) = 28 B
glb_tile_bytes      ≈ E × 28 + shared mesh + JSON chunk
                    ≈ 1.8 MB                        at 65 536 instances
```

| Variable | Default | Enforced by |
| --- | --- | --- |
| `MAX_QTILE_PAYLOAD_BYTES` | 256 MiB | Already a constant; refuses corrupt or hostile files |
| `MAX_INSTANCES_PER_TILE` | 262 144 | `GlbTileSpec::validate` |
| `MAX_GLB_BUFFER_BYTES` | 64 MiB | Refuse rather than assemble a huge tile in memory |
| `MAX_TILESET_NODES` | 1 000 000 | A tileset larger than this needs implicit tiling, an extension point |

A tile is assembled fully in memory before its atomic write, so
`MAX_GLB_BUFFER_BYTES` is a real ceiling on peak RSS during conversion, not a
guideline.

---

## 7. Cache

```text
l1_bytes ≤ DEFAULT_L1_ENTRIES × mean_entry_bytes,  and ≤ l1_max_bytes
l2_bytes ≤ DEFAULT_L2_MAX_BYTES                    = 8 GiB
```

| Variable | Default | Note |
| --- | --- | --- |
| `DEFAULT_L1_ENTRIES` | 512 | Already defined |
| `L1_MAX_BYTES` | 256 MiB | Eviction by count **and** by bytes — both already tested |
| `DEFAULT_L2_MAX_BYTES` | 8 GiB | Already defined; eviction tested |
| `L3_BROWSER_BYTES` | 512 MiB | Extension point |

L2 is content-addressed on disk, so its ceiling is disk, not RAM. Its cost to RSS
is one open file handle at a time.

---

## 8. Browser

The binding constraint of the whole system. A tab that exceeds its heap is killed
by the browser, and no amount of server-side discipline prevents it.

```text
cesium_tile_bytes    ≈ loaded_tiles × glb_tile_bytes
workspace_cell_bytes = cells × (position 12 + colour 12 + size 4)  = 28 B/cell
                     = 1.8 MB   at 65 536 cells
                     = 7.3 MB   at 262 144 cells (the ceiling)
query_result_bytes   = result_elements × 4
```

| Variable | Default | Enforced by |
| --- | --- | --- |
| `CESIUM_CACHE_BYTES` | 512 MiB | Cesium's own `cacheBytes` |
| `MAX_LOADED_TILES` | 256 | ≈ 460 MB at 1.8 MB/tile |
| `MAX_WORKSPACE_SPHERES` | 262 144 | `assertBlockIsBounded` |
| `MAX_JSON_RESULT_ELEMENTS` | 4 096 | Above this the API returns a `.qtile` |
| `MAX_BLOCK_REQUEST_BYTES` | 4 MiB | `refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser` |

**A whole tensor is never sent.** Not "usually not" — the refusal is a test, and
it fires before the network is touched.

---

## 9. Worked example: converting one 4096×4096 f32 tensor

```text
tensor          4096 × 4096 × 4 B          = 64 MiB on disk
blocks          16 × 16 = 256 blocks of 256×256

peak host RSS during conversion:
  source mmap        (pages, evictable)     ~0 MiB counted
  host staging       4 × 256 KiB            =  1.0 MiB
  decoded blocks     4 × 256 KiB            =  1.0 MiB
  statistics accum   4 × ~1 KiB             = ~0.0 MiB
  qtile writer       1 × 128 KiB            =  0.1 MiB
  glb writer         1 × 1.8 MB             =  1.8 MiB
  catalog + sqlite                          = ~8.0 MiB
  ─────────────────────────────────────────────────────
  total                                     ≈ 12 MiB

peak GPU (if CUDA):                          ≈ 1.5 MiB
output artifacts    256 qtile + 256 glb     ≈ 32 MB + 460 MB
```

**12 MiB of RSS to convert a 64 MiB tensor**, and the 12 MiB does not grow with
the tensor — only the block count does. That is the property that makes the same
pipeline valid for a 2.10 TB checkpoint.

---

## 10. Enforcement

| Mechanism | Where | State |
| --- | --- | --- |
| Named budget types, checked at allocation | `q_source::budget` | ✓ `SRC-017` |
| Access scale as a type | `q_source::AccessScale` | ✓ `SRC-018` |
| VRAM ceiling before launch | `q_cuda::check_workload` | ✓ `CUDA-006` |
| Instance ceiling before emit | `q_gltf::GlbTileSpec::validate` | ✓ `GLB-002` |
| Payload ceiling on decode | `q_tiles::MAX_QTILE_PAYLOAD_BYTES` | ✓ `TILE-006` |
| Block-request ceiling in the browser | `assertBlockIsBounded` | ✓ `GRID-005` |
| Whole-tensor read refusal | `q-weightql` plan | ✓ `WQL-011` |
| Backpressure in the conversion pipeline | job executor | `QM-0033` |
| Peak-RSS assertion in the conversion test | `QM-0031` | new |
| Browser heap soak | `QM-0082` | new |

Nine of eleven already exist. The plan adds enforcement to the two stages that do
not yet run.

---

## 11. Configuration

Every variable above is settable, in this precedence order:

```text
CLI flag  >  environment variable  >  config file  >  compiled default
```

Naming: `QM_MAX_HOST_STAGING_BYTES`, `QM_MAX_CONCURRENT_BLOCKS`, and so on. Each
is reported in the job record and in `GET /v1/jobs/{jobId}`, so a run's actual
budgets are recoverable after the fact — which is what makes a performance report
reproducible rather than anecdotal.
