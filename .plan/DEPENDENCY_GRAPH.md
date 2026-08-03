# DEPENDENCY_GRAPH

62 tasks. Edges derived from each `TASK.md`'s `Dependencies` and `Blocks`
sections; a disagreement between the two directions is a plan bug, caught by
`QM-0002`.

## 1. The graph

```text
PHASE 00 ─ contracts and baseline ───────────────────────────────────────────
  QM-0001 baseline verification ─┬─► QM-0002 divergence register
                                  └─► QM-0004 spatial contract ─► QM-0005 ◄══ G1
  QM-0003 LOD-capable fixture ────────────────────────────────────┐
                                                                   │
PHASE 01/02 ─ ingestion and catalog ───────────────────────────────┼─────────
  QM-0005 ─┬─► QM-0010 qwen ─► QM-0011 conformance                │
           ├─► QM-0012 config metadata ─► QM-0020 statistics      │
           ├─► QM-0013 manifest tool                              │
           └─► QM-0023 candidates                                 │
                          QM-0020 ─► QM-0021 tiles ─► QM-0022 blocks
                                                                   │
PHASE 03 ─ block runtime (Lane A) ─────────────────────────────────┘
  QM-0003 ─► QM-0030 stream ─► QM-0031 stats pass ─► QM-0032 cache ─► QM-0033 jobs
                                    ▲                                     │
                             QM-0020┘                              QM-0022┘
  QM-0034 backend selection ◄── QM-0037

PHASE 03 ─ CUDA (Lane E, RTX 3090) ─────────────────────────────────────────
  QM-0005 ─► QM-0034 cuda build ─► QM-0035 reduce/hist ─► QM-0036 quant/matmul
                     └────────────► QM-0037 selection            └─► QM-0083

PHASE 04 ─ artifacts (Lane A) ──────────────────────────────────────────────
  QM-0031 ─► QM-0040 planner ─► QM-0041 qtile ─► QM-0042 glb ─► QM-0043 features
  QM-0021 ─┘                        ▲               └─► QM-0044 tileset
  QM-0033 ─────────────────────────┘                        │
                          QM-0044 ─► QM-0045 atomic ─► QM-0046 validation ◄══ G2

PHASE 05 ─ viewer (Lane B) ─────────────────────────────────────────────────
  QM-0005 ─► QM-0050 SPIKE (early, parallel with Lane A)
  QM-0050 + QM-0046 ─► QM-0051 load ─► QM-0052 lod ─► QM-0053 pick ◄══ G3
  QM-0051 ─┬─► QM-0054 badges   (needs QM-0020)
           ├─► QM-0055 hierarchy (needs QM-0021)
           ├─► QM-0056 camera/url/disposal
           └─► QM-0057 capability probe (needs QM-0043)

PHASE 06 ─ workspace (Lane C) ──────────────────────────────────────────────
  QM-0005 ─► QM-0060 web core ─┬─► QM-0061 axis binding
                                ├─► QM-0062 ruled grid ─┐
                                ├─► QM-0063 spheres ─► QM-0064 budget
                                └─► QM-0052 (Lane B also consumes it)
                  QM-0061 + QM-0062 ─► QM-0065 frames
                  QM-0064 + QM-0032 ─► QM-0066 adapter ─► QM-0067 matmul ─► QM-0068 hover
                                                              ▲
                                                    QM-0065 ──┘

PHASE 07 ─ query (Lane D) ──────────────────────────────────────────────────
  QM-0031 + QM-0037 + QM-0032 ─► QM-0070 ─► QM-0071 ─► QM-0072 ─► QM-0073
                                                (needs QM-0020, QM-0013)
  QM-0073 ─┬─► QM-0074 chat (needs QM-0023)
           └─► QM-0075 candidates/katex/security

PHASE 08 ─ integration ─────────────────────────────────────────────────────
  QM-0046 + QM-0053 + QM-0055 + QM-0057 + QM-0067 + QM-0074 ─► QM-0080 ◄══ G4
  QM-0080 ─┬─► QM-0081 failure injection
           ├─► QM-0082 browser soak (needs QM-0056, QM-0067)
           ├─► QM-0084 benchmarks (needs QM-0013)
           └─► QM-0085 error/security audit (needs QM-0075)
  QM-0036 ─► QM-0083 cuda soak  [RTX 3090]

PHASE 09 ─ release ─────────────────────────────────────────────────────────
  QM-0080 ─┬─► QM-0090 docs (blocked on ADR-CANDIDATE-014)
           ├─► QM-0092 limitations (needs QM-0084, QM-0035|waiver)
           └─► QM-0093 licensing
  QM-0090 ─► QM-0091 status ─► QM-0094 acceptance audit ◄══ G5
```

## 2. Critical path — 16 tasks, no CUDA

```text
QM-0001 → QM-0004 → QM-0005 → QM-0030 → QM-0031 → QM-0033 → QM-0040 → QM-0041
        → QM-0042 → QM-0044 → QM-0051 → QM-0053 → QM-0066 → QM-0067 → QM-0080
        → QM-0094
```

`QM-0003` is a co-prerequisite of `QM-0030` and runs in parallel from the start.

## 3. Integration gates

| Gate | Task | Blocks until passed |
| --- | --- | --- |
| **G1** Contract | `QM-0005` | All of Phases 04, 05, 06 |
| **G2** Artifact | `QM-0046` | Every Phase 05 task except `QM-0050` |
| **G3** Render | `QM-0053` | `QM-0080` |
| **G4** Exactness | `QM-0080` | All of Phase 09 |
| **G5** Release | `QM-0094` | Release |

## 4. Shared-file risk

Tasks touching the same file must not run concurrently. These are the
merge-conflict hotspots.

| File | Tasks | Rule |
| --- | --- | --- |
| `crates/q-catalog/src/lib.rs` (987 lines) | `QM-0012`, `QM-0020`, `QM-0021`, `QM-0022`, `QM-0072` | **Strictly sequential** in that order |
| `crates/q-weightql/src/plan.rs` (673) | `QM-0070`, `QM-0071`, `QM-0072`, `QM-0073` | **Strictly sequential** in that order |
| `crates/q-daemon/src/lib.rs` (941) | `QM-0012`, `QM-0020`, `QM-0032`, `QM-0033`, `QM-0041`, `QM-0042`, `QM-0044`, `QM-0073`, `QM-0075` | Route additions are additive; coordinate on the router table. `QM-0033` adds a module — merge it before the later route tasks |
| `apps/web/model-viewer/src/index.ts` | `QM-0050`…`QM-0057` | `QM-0050` establishes the shell first; the rest touch their own modules |
| `apps/web/matrix-workspace/src/viz/mat.ts` | `QM-0063`, `QM-0064`, `QM-0068` | Sequential: `QM-0063` → `QM-0064` → `QM-0068` |
| `schemas/visualization/schema.json` | `QM-0004`, `QM-0021` | `QM-0004` first, alone |
| `crates/q-gltf/src/instanced.rs` | `QM-0042`, `QM-0043` | Sequential |
| `.github/workflows/build.yaml` | `QM-0001`, `QM-0003`, `QM-0005`, `QM-0046`, `QM-0080`, `QM-0082`, `QM-0084` | Each adds a **separate job**; conflicts are trivial to resolve |

## 5. Fixture dependencies

`QM-0003` gates more than its phase suggests.

| Needs the large fixture | Why |
| --- | --- |
| `QM-0030` | 256×256 block decomposition needs a tensor larger than `[128,48]` |
| `QM-0031` | Peak-RSS flatness across 1024², 2048², 4096² |
| `QM-0035`, `QM-0036` | Realistic block sizes for differential testing |
| `QM-0040`…`QM-0046` | A five-level pyramid needs a tensor with five levels in it |
| `QM-0051`…`QM-0053` | LOD refinement needs something to refine |
| `QM-0080` | The end-to-end demonstration |
| `QM-0084` | Scaling curves |

`QM-0013`'s synthetic manifest gates `QM-0072` and `QM-0084`.

## 6. ADR blockers

A task depending on an undecided ADR sits at `Blocked`.

| ADR candidate | Blocks | Deadline |
| --- | --- | --- |
| `002` CUDA build | `QM-0034` | Before `QM-0034` |
| `005` Catalog technology | `QM-0072` (reopens only above 1 s) | Before Phase 08 |
| `007` Web core package | `QM-0060` | Before `QM-0060` |
| `008` Implicit tiling | `QM-0044` | Before `QM-0044` |
| `009` 3D Tiles + placement | `QM-0044`, `QM-0050` | Before `QM-0044` |
| `010` Viewer shell | `QM-0050` | Before `QM-0050` |
| `011` Daemon transport | `QM-0033` | Before `QM-0033` |
| `012` Parser technology | `QM-0074` | Before `QM-0074` |
| `013` Browser tests | `QM-0051` | Before `QM-0051` |
| `014` Plane mapping | `QM-0060`, **`QM-0090`** | Before `QM-0060`; `QM-0090` cannot edit `ARCHITECTURE.md` without it |
| `015` Cell primitive | `QM-0063` | The default before; the choice is made *by* `QM-0063` |
| `016` Axis binding | `QM-0061` | Before `QM-0061` |
| `017` GLB instancing | `QM-0042` | Before `QM-0042` |
| `018` Tensor IDs | `QM-0021` | Before `QM-0021` |
| `019` Browser cache | `QM-0051` | Before `QM-0051` |

## 7. Hardware gating

| Requires RTX 3090 | Blocks on the critical path |
| --- | --- |
| `QM-0035`, `QM-0036`, `QM-0083` | **Nothing** |

`QM-0034` needs a CUDA **toolkit** to compile, not a device. Without hardware,
`QM-0035`, `QM-0036`, and `QM-0083` stay `Blocked`, their requirements stay
`Hardware-Unverified`, and `MVP-10`, `MVP-12`, and `MVP-42` take written
waivers. **Nothing else changes.**

## 8. Cross-lane coupling

Three places where lanes genuinely touch:

1. **`QM-0060` serves both B and C.** `QM-0052` (viewer) and every Phase 06 task
   import from `apps/web/core`. It must land before either lane proceeds far.
2. **`QM-0066` needs a running daemon** with converted output, so Lane C's tail
   depends on Lane A reaching `QM-0041`.
3. **`QM-0080` needs all four lanes.** It is the only task with six
   dependencies.

Everything else is lane-local after G1.

## 9. Task kinds

| Kind | Count | Tasks |
| --- | --- | --- |
| **Verification** | **17** | `QM-0001`, `QM-0002`, `QM-0005`, `QM-0011`, `QM-0013`, `QM-0023`, `QM-0035`, `QM-0036`, `QM-0046`, `QM-0080`, `QM-0081`, `QM-0082`, `QM-0083`, `QM-0084`, `QM-0085`, `QM-0093`, `QM-0094` |
| **Documentation** | **3** | `QM-0090`, `QM-0091`, `QM-0092` |
| **Implementation** | **42** | All others |
| | **62** | |

Verification tasks add no production code and can be scheduled
opportunistically, except where they are gates (`QM-0005`, `QM-0046`,
`QM-0080`, `QM-0094`).

Three verification tasks require an RTX 3090: `QM-0035`, `QM-0036`, `QM-0083`.
The other 14 run anywhere.
