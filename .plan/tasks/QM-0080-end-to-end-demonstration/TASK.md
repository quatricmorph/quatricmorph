# QM-0080 — End-to-end demonstration

## Status

Blocked

Unblocks when Phases 03–07 reaches `Complete`. **Integration gate G4.**

## Phase

Phase 08 — Integration and performance

## Objective

Run the full eleven-step workflow as one CI job, on a machine with **no NVIDIA
GPU**.

## Repository Evidence

* `tests/tests/end_to_end_scalar_slice.rs` (353 lines) — 4 tests over 6 golden
  scalars, 2 golden slices, and 2 bf16 tensors, against
  `fixtures/tiny-llama-2shard/golden.json` (`safetensors==0.8.0`). **Steps 1, 2,
  7, and 8 already pass.**
* `AC-005` Verified — the exact scalar matches the Python reference.
* Every other step is delivered by a Phase 03–07 task.
* `.github/workflows/build.yaml` has no end-to-end job.

## Requirements Covered

`MVP-22`, `MVP-23`, `AC-004`, `AC-006`, `AC-007`, `AC-010`; integrates all lanes.

## Dependencies

`QM-0046`, `QM-0053`, `QM-0055`, `QM-0057`, `QM-0067`, `QM-0074`.

## Blocks

`QM-0091`, `QM-0094`.

## Parallelization

Runs alone. **This is gate G4.**

## Program Boundary

`tests/`, CI, Playwright.

## Scope

The task specification §32 sequence:

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

## Out of Scope

Performance tuning (`QM-0084`) · soaks (`QM-0082`, `QM-0083`) · failure injection
(`QM-0081`) · anything requiring CUDA.

## Files Expected to Change

* `.github/workflows/build.yaml`

## Files Expected to Add

* `tests/tests/end_to_end_pipeline.rs` — steps 1–4, 7, 8
* `apps/web/e2e/full-workflow.spec.ts` — steps 5, 6, 9, 10, 11
* `scripts/e2e.sh` — orchestrates both halves

## Files Expected to Remove or Deprecate

None. `end_to_end_scalar_slice.rs` stays; this task **extends** rather than
replaces it.

## Data Contracts

The decisive assertion, and the reason the whole architecture exists:

```text
value clicked in the viewer
  == value returned by GET /v1/tensors/{id}/value
  == value in fixtures/…/golden.json      (produced by Python safetensors)
```

Three independent paths to one number. If they disagree, something between the
byte range and the pixel is wrong.

## Memory and Performance Constraints

* The whole run in under 15 minutes in CI.
* Conversion peak RSS asserted **< 32 MB**.
* Browser heap asserted < 600 MB.
* **Zero CUDA.** An end-to-end test that needs hardware CI lacks is not a test.

## Implementation Plan

1. Rust half: generate the fixture, import, convert a tensor hierarchy, generate
   artifacts, validate, read the exact value, compare to `golden.json`.
2. Start the daemon against the generated output.
3. Playwright half: open the viewer, load the tileset, navigate, pick a cell,
   compare its address and value against the API, open the workspace, assign
   blocks, multiply, compare against the CPU reference, submit a chat query.
4. `scripts/e2e.sh` orchestrating both with clean setup and teardown.
5. CI job with artifacts: screenshots, logs, the request log.

## Error Handling

* Any step failing → the job fails **naming the step number**, so a failure is
  immediately locatable.
* The daemon failing to start → fail with its log.
* A flaky render → **retried at most once**, with the retry recorded. More than
  one retry hides a real problem.
* A timeout → fail with the last completed step.

## Acceptance Criteria

1. All eleven steps pass on a machine with no NVIDIA GPU.
2. The value from the viewer equals the API value equals `golden.json`.
3. Navigation issues **zero** exact-value requests (step 5 → 6).
4. Artifacts pass external validation (`QM-0046`).
5. `A @ B` on real blocks matches the CPU reference to `1e-5`.
6. Chat produces a plan with a cost, and executes only on the explicit act.
7. Conversion peak RSS < 32 MB.
8. Browser heap < 600 MB.
9. The run completes in under 15 minutes.
10. Screenshots are archived for steps 5, 6, 9, and 10.

## Verification Plan

**Automated** — the CI job.
**Manual** — review the archived screenshots and the request log.

## Suggested Commands

```bash
./scripts/e2e.sh                                        # introduced here
cargo test --test end_to_end_pipeline                    # introduced here
npx playwright test apps/web/e2e/full-workflow.spec.ts   # introduced here
cargo test --test end_to_end_scalar_slice                # verified today
```

## Test Cases

| Step | Assertion |
| --- | --- |
| 1–2 | Import completes; peak memory bounded |
| 3 | Job reaches `Complete`; block count matches the plan |
| 4 | `gltf-validator` and `3d-tiles-validator` pass |
| 5 | Tileset renders; screenshot |
| 6 | Picked address matches the API |
| 7 | 4 bytes read |
| 8 | Value equals `golden.json` |
| 9 | Block renders grid-aligned; screenshot |
| 10 | `A @ B` matches the CPU reference |
| 11 | Plan with cost; execution requires the explicit act |
| Throughout | Zero exact requests during navigation |

## Risks

| Risk | Mitigation |
| --- | --- |
| The test is flaky and gets disabled | Small, assertive steps; each maps to an acceptance criterion, so disabling one visibly disables a criterion |
| It is slow and gets skipped | 15-minute budget as an acceptance criterion |
| It passes while something is subtly wrong | Three independent paths to the same value |

## Completion Evidence

* Full CI job log with per-step timings.
* Screenshots from steps 5, 6, 9, 10.
* The three-way value comparison.
* Peak RSS and browser heap measurements.
* The request log proving zero exact reads during navigation.
