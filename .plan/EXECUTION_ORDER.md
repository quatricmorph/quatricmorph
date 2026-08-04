# EXECUTION_ORDER

## 1. How to use this document

An agent picks the earliest `Ready` task on the critical path. If none is
`Ready`, it takes any `Ready` task from a parallel lane whose
`Parallelization` section does not name a file another in-progress task is
editing ([`DEPENDENCY_GRAPH.md`](DEPENDENCY_GRAPH.md) §4).

**Read [`../STATUS.md`](../STATUS.md) first.** It, not this plan, is the record
of what is built.

## 2. Wave order

Tasks within a wave may run concurrently. A wave starts when its predecessor
completes, except where a gate says otherwise.

### Wave 0 — baseline and contracts (5 tasks)

```text
parallel:  QM-0001  baseline verification
           QM-0002  divergence register
           QM-0003  LOD-capable fixture
then:      QM-0004  spatial contract        (alone — shared schema)
then:      QM-0005  conformance tests       ══ GATE G1
```

**Nothing in Phases 04–06 may start before G1.** Each of those phases would
otherwise add a fourth copy of the spatial rules.

### Wave 1 — ingestion, catalog, and the viewer spike (9 tasks)

```text
Lane A:  QM-0012 → QM-0020 → QM-0021 → QM-0022      (sequential — shared file)
Lane A:  QM-0030                                     (needs QM-0003)
Lane B:  QM-0050  viewer spike                       ← EARLY, de-risks R1
Lane E:  QM-0034  cuda build                         [CUDA toolkit]
free:    QM-0010 → QM-0011,  QM-0013,  QM-0023
```

`QM-0050` is scheduled here, far ahead of the rest of Phase 05, because it is the
only task whose failure costs a phase rather than a task.

### Wave 2 — compute and the web core (7 tasks)

```text
Lane A:  QM-0031 → QM-0032 → QM-0033
Lane A:  QM-0037                                     (needs QM-0034)
Lane C:  QM-0060  web core                           (alone — both lanes consume it)
Lane E:  QM-0035 → QM-0036                           [RTX 3090]
```

### Wave 3 — artifacts and workspace foundations (11 tasks)

```text
Lane A:  QM-0040 → QM-0041 → QM-0042 → QM-0043 → QM-0044 → QM-0045 → QM-0046  ══ G2
Lane C:  QM-0061,  QM-0062,  QM-0063 → QM-0064
Lane C:  QM-0065                                     (needs QM-0061, QM-0062)
```

Lane A is a strict chain; Lane C runs beside it.

### Wave 4 — viewer and query (13 tasks)

```text
Lane B:  QM-0051 → QM-0052 → QM-0053                 ══ G3
Lane B:  QM-0054,  QM-0055,  QM-0056,  QM-0057       (parallel after QM-0051)
Lane C:  QM-0066 → QM-0067 → QM-0068
Lane D:  QM-0070 → QM-0071 → QM-0072 → QM-0073       (sequential — shared file)
Lane D:  QM-0074,  QM-0075                            (parallel after QM-0073)
```

### Wave 5 — integration (6 tasks)

```text
           QM-0080  end-to-end demonstration         ══ G4  (alone)
parallel:  QM-0081  failure injection
           QM-0082  browser soak
           QM-0084  benchmarks
           QM-0085  error and security audit
Lane E:    QM-0083  cuda soak                        [RTX 3090]
```

### Wave 6 — release (5 tasks)

```text
parallel:  QM-0090  documentation      (ADR gate satisfied: ADR-009)
           QM-0092  limitations
           QM-0093  licensing
then:      QM-0091  regenerate STATUS.md
then:      QM-0094  acceptance audit                 ══ G5
```

## 3. Critical path

```text
QM-0001 → QM-0004 → QM-0005 → QM-0030 → QM-0031 → QM-0033 → QM-0040 → QM-0041
        → QM-0042 → QM-0044 → QM-0051 → QM-0053 → QM-0066 → QM-0067 → QM-0080
        → QM-0094
```

**Sixteen tasks. Not one requires an NVIDIA GPU.**

`QM-0003` is a co-prerequisite of `QM-0030` and starts in Wave 0.

### Why CUDA is not on it

`q_gpu::CpuBackend` is `GPU-002 Verified` and `block_statistics_default` already
computes what the tile pyramid needs. Routing Phase 04 through the CPU backend
makes the pipeline buildable on the machine that must build it;
`docs/decisions/ADR-008-track-b-prerequisite-waiver.md` already waives the RTX
3090 gate. CUDA replaces a backend behind `q_gpu::Backend` and changes no
downstream artifact.

Had CUDA been on the critical path, the MVP would be unbuildable in its own
development environment.

## 4. Lanes

| Lane | Tasks | Owns | Unblocked by |
| --- | --- | --- | --- |
| **A — artifacts** (critical) | `QM-0030`…`QM-0033`, `QM-0037`, `QM-0040`…`QM-0046`, plus catalog work | `crates/q-tensor-runtime`, `q-statistics`, `q-tiles`, `q-gltf`, `q-tileset`, `q-catalog` | G1 |
| **B — viewer** | `QM-0050`…`QM-0057` | `apps/web/model-viewer` | G1; G2 for real data |
| **C — workspace** | `QM-0060`…`QM-0068` | `apps/web/core`, `apps/web/quatricmorph-workspace` | G1 |
| **D — query** | `QM-0070`…`QM-0075` | `crates/q-weightql`, `q-expression`, `apps/web/query-interface` | `QM-0031`, `QM-0020` |
| **E — CUDA** | `QM-0034`…`QM-0036`, `QM-0083` | `crates/q-cuda`, `gpu/cuda` | G1. **Blocks nothing** |

## 5. Runnable without an RTX 3090

**59 of 62 tasks.**

Only `QM-0035`, `QM-0036`, and `QM-0083` require the device. `QM-0034` needs a
CUDA toolkit to compile, not a device, and its acceptance criteria include that
`cargo build --workspace` keeps working **without** the toolkit.

Consequently 43 of the 46 acceptance criteria are achievable with no NVIDIA
hardware; `MVP-10`, `MVP-12`, and `MVP-42` are the exceptions and have a defined
waiver path.

## 6. Requiring an RTX 3090

| Task | What it proves |
| --- | --- |
| `QM-0035` | Reduction and histogram kernels match the CPU reference on hardware |
| `QM-0036` | Quantization, Morton, and matmul match; OOM adaptation works |
| `QM-0083` | 10 000 block jobs leak no device memory |

Without hardware these stay `Blocked`, `CUDA-001`…`CUDA-005`, `CUDA-008`, and
`CUDA-009` stay `Hardware-Unverified` in `STATUS.md`, and the documentation
claims nothing more. That is the honest outcome, not a failure.

## 7. Where concurrency is forbidden

Not every parallel-looking task may actually run in parallel.

| Sequence | Reason |
| --- | --- |
| `QM-0012` → `QM-0020` → `QM-0021` → `QM-0022` → `QM-0072` | All edit `crates/q-catalog/src/lib.rs` |
| `QM-0070` → `QM-0071` → `QM-0072` → `QM-0073` | All edit `crates/q-weightql/src/plan.rs` |
| `QM-0040` → `QM-0041` → `QM-0042` → `QM-0043` → `QM-0044` | Each consumes the previous stage's output |
| `QM-0063` → `QM-0064` → `QM-0068` | All edit `viz/mat.ts` |
| `QM-0004` alone | Shared schema; every lane consumes it |
| `QM-0060` alone | Both web lanes consume it |
| `QM-0080` alone | It is the integration gate |

## 8. If a gate fails

| Gate | Failure | Response |
| --- | --- | --- |
| **G1** | Rust and TypeScript constants disagree | The transcription in `QM-0004` is wrong. Fix it before any consumer exists — this is the cheapest moment in the whole plan |
| **G2** | External validators reject an artifact | File a task against the emitting builder. **Do not proceed to Phase 05** — the viewer would be debugging our bug through Cesium's error messages |
| **G3** | A pick resolves to the wrong address | Off-by-one in index composition or feature-ID ordering. Blocks `QM-0080` |
| **G3** | Cesium cannot render acceptably | `ADR-CANDIDATE-009`'s fallback: Three.js with custom traversal. **The tile format does not change**; only `apps/web/model-viewer` is rewritten |
| **G4** | The three-way value comparison disagrees | Something between the byte range and the pixel is wrong. Highest-severity possible finding; halt and bisect |
| **G5** | A criterion has neither evidence nor a waiver | The release is blocked. File the task |

## 9. Estimated shape

Not a schedule — this plan has no calendar. A shape, for sequencing intuition:

| Wave | Tasks | Longest chain | Parallelism |
| --- | --- | --- | --- |
| 0 | 5 | 3 (`0001`→`0004`→`0005`) | 3 |
| 1 | 9 | 4 (`0012`→…→`0022`) | 4 lanes |
| 2 | 7 | 3 (`0031`→`0032`→`0033`) | 3 lanes |
| 3 | 11 | 7 (`0040`→…→`0046`) | 2 lanes |
| 4 | 13 | 4 (`0070`→…→`0073`) | 4 lanes |
| 5 | 6 | 2 (`0080`→ rest) | 5 after `0080` |
| 6 | 5 | 3 (`0090`→`0091`→`0094`) | 3 |

Wave 3 is the longest single chain and the schedule's centre of gravity. Lane C
and Lane B both have real work available beside it, so the wave is not
serialized in practice.

## 10. The first three actions

For an agent starting now, with nothing in progress:

1. **`QM-0001`** — run both suites, record the baseline. Nothing should be built
   on an unverified baseline, and this takes minutes.
2. **`QM-0003`** — start the fixture generator in parallel; it gates seven later
   tasks and needs no decisions.
3. **`QM-0002`** — validate the plan's own citations before anyone relies on
   them.

Then `QM-0004` alone, then `QM-0005`, and the lanes open.
