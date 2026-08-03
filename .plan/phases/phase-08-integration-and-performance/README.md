# Phase 08 — Integration and performance

## Goal

```text
SafeTensors → conversion → Cesium selection → exact query → matrix visualization
   as ONE demonstration, on a machine with no NVIDIA GPU
```

## Entry conditions

* **G1**, **G2**, and **G3** passed.
* Phases 03, 04, 05, 06, 07 complete.
* Playwright configured (`ADR-CANDIDATE-013`).

## Tasks

| ID | Title | Kind | Requirements |
| --- | --- | --- | --- |
| `QM-0080` | End-to-end demonstration | Verification | `MVP-22`, `MVP-23`, `AC-004`…`AC-010` |
| `QM-0081` | Cache reuse, resume, and failure injection | Verification | `CACHE-008`, `JOB-002`, `TILE-011`, `MVP-16`, `MVP-17` |
| `QM-0082` | Browser memory and disposal soak | Verification | `PERF-002`, `CESIUM-013`, `MVP-41` |
| `QM-0083` | 🔧 CUDA device-memory soak | Verification | `CUDA-009`, `MVP-42` |
| `QM-0084` | Scaling benchmarks and the benchmark harness | Verification | `PERF-004`, `CAT-006` |
| `QM-0085` | Runtime error audit and security review | Verification | `MVP-43`, `SEC-007`, `SEC-008` |

🔧 = requires an RTX 3090.

## The demonstration — **integration gate G4**

The task specification §32 sequence, run as one CI job:

```text
 1. open the SafeTensors fixture
 2. import metadata                        bounded memory asserted
 3. convert a selected tensor hierarchy    job runs, checkpoints, completes
 4. generate .qtile, GLB, tileset.json     all three validate externally
 5. open in CesiumJS                       tileset loads and renders
 6. select a tensor block                  resolves to the correct address
 7. retrieve one exact value               4 bytes read
 8. verify against Python safetensors      equals golden.json
 9. assign blocks to the matrix workspace  grid-aligned, bounded
10. visualize A @ B                        matches the CPU reference
11. query the selection through chat       produces a plan with a cost
```

Steps 1, 2, 7, and 8 already pass in `tests/tests/end_to_end_scalar_slice.rs` —
4 tests over 6 golden scalars, 2 golden slices, and 2 bf16 tensors.

**The whole sequence runs with zero CUDA.** An end-to-end test that needs
hardware CI does not have is not a test.

## Failure injection — `QM-0081`

The pipeline is built to be interrupted; this proves it.

| Injected failure | Expected behaviour |
| --- | --- |
| Kill mid-conversion | Resume produces **byte-identical** output; no orphaned `.tmp` files |
| Daemon stopped during viewing | Viewer shows a banner with the start command; retries with backoff |
| Delete a `.qtile` referenced by a tile | Geometry renders; the inspector says "values unavailable" |
| Corrupt a GLB | That tile fails alone; siblings keep rendering |
| Disk full during a write | Job fails cleanly; **nothing is published under a final name** |
| Cancel mid-conversion | Stops within one block; the completed manifest survives |
| Second conversion of the same tensor | Cache hits; compute skipped |
| Missing shard | Reported by name |

## Exit conditions

1. The eleven-step demonstration passes from a clean checkout, no NVIDIA GPU.
2. Every failure-injection case behaves as tabulated.
3. Browser heap returns within 10 % of baseline over 100 model switches and 100
   workspace re-initializations.
4. Conversion peak RSS is **independent of tensor size** — measured at 1024²,
   2048², and 4096², with a flat curve.
5. Import time and peak memory are linear in tensor count at 10³, 10⁴, and 10⁵.
6. The browser console is empty across the full manual checklist.
7. Benchmarks are recorded in `benchmarks/` with commit SHA, hardware, and every
   configuration variable — reproducible, not anecdotal.
8. On an RTX 3090: 10 000 block jobs return `cudaMemGetInfo` free to its start
   value — **or** `MVP-42` takes a written waiver.

## Parallelization

`QM-0080` first — it is the gate. `QM-0081`…`QM-0085` are independent of each
other and run in parallel after it. `QM-0083` runs only where hardware exists and
blocks nothing.

## Risks

| Risk | Mitigation |
| --- | --- |
| The end-to-end test is flaky and gets disabled | Keep it small and assertive; every step maps to an acceptance criterion, so disabling one is visibly disabling a criterion |
| A performance budget is missed | The budget is corrected in the plan with the measurement recorded — **not** quietly dropped |
| R3 — no RTX 3090 | `QM-0083` waived; every other exit condition is hardware-free |
