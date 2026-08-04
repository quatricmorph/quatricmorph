# QM-0166 — Technical write-up

## Status

Blocked

Unblocks when `QM-0102` and `QM-0162` reach `Complete`.

## Phase

Phase 14 — Validation and v1 release

## Objective

Write the short technical account of out-of-core quantisation-error diagnosis —
the artifact that carries the work whether or not the business validates.

## Repository Evidence

* `QM-0101`, `QM-0102` — the residency proof and the benchmarks, which are the
  paper's systems contribution.
* `QM-0122` — the reference-verified metrics.
* `QM-0162` — the case study.
* Strategy §5: out-of-core visualisation and diagnosis of very large models is
  publishable at IEEE VIS / EuroVis-adjacent venues and MLSys / NeurIPS-ICML
  workshops. §11 Days 60–90 asks for a draft.

## Requirements Covered

None. This is the outcome that survives either result.

## Dependencies

`QM-0102`, `QM-0162`, `QM-0151`.

## Blocks

Nothing.

## Parallelization

Lane V.

## Program Boundary

`docs/writeups/`.

## Scope

* A short technical write-up: problem, the out-of-core design, the measured
  residency and throughput results, the diagnostic method, the case study, and an
  honest limitations section.
* Positioning against the closest prior art, named specifically.
* A blog-length version and a workshop-length version from the same material.

## Out of Scope

A full conference submission · new experiments to strengthen the story · claims
beyond `PRODUCT_SCOPE.md` §5.2.

## Structure

| Section | Content |
| --- | --- |
| Problem | Quantisation decisions are made against tabular per-tensor metrics; error concentration is invisible |
| Prior art | Google AI Edge Quantization Debugger (five scalar metrics, int8 only); layer-sensitivity work shipping static plots; Palace (arXiv:2509.26213) for out-of-core GPU tensor visualisation; Netron for structure. Positioned by what each does **not** do, factually |
| Design | Bounded-residency streaming, the two-pass parameter derivation, the paired reduction, exact partial composition |
| Results | Measured peak RSS vs. checkpoint size at three sizes; throughput; the ratio `N` |
| Method | The metrics, the ranking, the frontier — with the greedy caveat stated |
| Case study | `QM-0162`, anonymised as required |
| Limitations | Weight-space only; no accuracy measured; greedy not optimal; attribution at bucket resolution; simulated rather than ingested quantisation; the largest checkpoint tested and why |

The limitations section is not a formality. It is the same discipline
`STATUS.md` already enforces, and it is what makes the systems results credible.

## Acceptance Criteria

1. Every number in the write-up traces to a task's completion evidence.
2. Prior art is cited specifically and characterised factually — no strawmen.
3. The limitations section covers all seven items above.
4. The largest checkpoint actually tested is stated, with the disk constraint that
   set it.
5. No claim exceeds `PRODUCT_SCOPE.md` §5.2.
6. Both lengths are drafted from the same material.
7. A reader could reproduce the residency result from the commands given.

## Verification Plan

**Manual** — trace every number to its evidence; one external reader.

## Risks

| Risk | Mitigation |
| --- | --- |
| Numbers drift from the evidence | Criterion 1 traces each one |
| Prior art strawmanned | Criterion 2; the strategy's own citations are the starting point |
| Limitations softened for a venue | Criterion 3 enumerates them |
| The write-up becomes a substitute for validation | It is blocked on `QM-0162`, not a replacement for it |

## Completion Evidence

* Both drafts.
* The number-to-evidence trace table.
* The external reader's comments.
