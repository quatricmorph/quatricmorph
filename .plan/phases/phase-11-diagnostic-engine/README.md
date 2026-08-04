# Phase 11 — Quantisation-error diagnostic engine

## Goal

```text
Base checkpoint block  ──┐
                         ├─→ paired reduction → per-channel + per-block partials
Simulated Ŵ block     ──┘
   → aggregate: channel → tensor → module → layer → expert → model
   → rank by relative weight-space error
   → mixed-precision frontier: bytes added vs. error removed
```

This is the only phase in the v1 plan containing genuinely novel code. Everything
before it is plumbing that mostly exists; everything after it is presentation and
proof.

## Design

[`../../DIAGNOSTIC_ARCHITECTURE.md`](../../DIAGNOSTIC_ARCHITECTURE.md) is the
specification. The two decisions that shape the phase:

**Simulate, do not ingest.** v1 computes `Ŵ = dequant(quant(W, config))` itself
and never reads a third-party quantised artifact. The value proposition is a
*pre*-quantisation decision, so the base checkpoint plus a config is sufficient —
and that means zero new input-format work. GPTQ/AWQ/`compressed-tensors`/GGUF
ingestion is `QUANT-010`, a seam, and a later module.

**Everything is a partial.** Blocks contribute sums of squares, not finished
metrics. RMSE and relative error are derived once, at the top. Computing a metric
per block and averaging is the most likely correctness bug in this engine, and
`V1-12` exists to catch it.

## The trait change

`q_gpu::Backend` is single-tensor today. `QM-0121` adds a **paired** reduction
taking a base block and a counterpart block. Built generically — counterpart, not
"its own quantisation" — the same kernel serves checkpoint-diff forensics later
(`DIFF-001`) at near-zero marginal cost. This is one of the few places in the plan
where generalising early is cheaper than not.

## Entry conditions

* **G1 passed** — bounded residency proven on a real checkpoint.
* `QM-0030` complete: blocks stream through bounded, named buffers.
* A Python/NumPy reference environment available (the `AC-005` pattern, already
  used against `safetensors==0.8.0`).

## Tasks

| ID | Title | Kind | Lane | Requirements |
| --- | --- | --- | --- | --- |
| `QM-0120` | Quantisation simulation: RTN int8/int4, per-tensor/channel/group, sym/asym | Implementation | Q | `QUANT-001`, `V1-08`, `V1-15` |
| `QM-0121` | Paired block reduction in `q_gpu::Backend` | Implementation | Q | `QUANT-002`, `V1-11` |
| `QM-0122` | Streaming diagnostic pass over a whole tensor, Python-verified | Implementation | Q | `QUANT-003`, `V1-09`, `V1-10`, `V1-13` |
| `QM-0123` | Aggregation: channel → tensor → module → layer → expert → model | Implementation | Q | `QUANT-004`, `V1-12` |
| `QM-0124` | Outlier attribution | Implementation | Q | `QUANT-005` |
| `QM-0125` | Fragility ranking and the mixed-precision frontier | Implementation | Q | `QUANT-006`, `V1-20` |
| `QM-0126` | Metal backend build integration | Implementation | U | `GPU-003` |
| `QM-0127` | Metal differential verification against CPU | Verification | U | `V1-14` |

Lane U blocks nothing. `CpuBackend` is the numerical reference and ships v1; Metal
makes the headline run faster and changes no output.

## Exit conditions — Gate G2

1. Every metric matches an independent Python/NumPy reference on golden tensors.
2. Streaming aggregation equals whole-tensor computation.
3. Two runs of the same config produce byte-identical partials.
4. Per-channel vectors are correctly oriented — proven on an asymmetric fixture
   where transposing the axis would change the answer.
5. Degenerate cases (all-zero group, non-finite weights, subnormal scale) behave
   as specified in `DIAGNOSTIC_ARCHITECTURE.md` §3.1 — refusing, never guessing.

**A wrong number is worse than a missing one.** If G2 fails, halt and bisect
against the goldens; do not proceed to the report.

## What this phase may never produce

A predicted accuracy delta, a Hessian-weighted sensitivity score, or a claim that
an expert is "dead". All three need activations from a calibration set, which
needs an inference runtime — a standing non-goal. The seams `EVAL-001` and
`EVAL-002` refuse with their requirement IDs, and
`DIAGNOSTIC_ARCHITECTURE.md` §8 fixes the wording the report uses instead.
