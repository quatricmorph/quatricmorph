# QM-0092 — CUDA requirements, dtypes, and limitations

## Status

Blocked

Unblocks when `QM-0084` reaches `Complete`.

## Phase

Phase 09 — Documentation and release

## Objective

Document what Quatricmorph **cannot** do, plainly and where users will read it.

## Repository Evidence

* `gpu/cuda/README.md` — the model for this document: *"Treat every performance
  or numerical claim below as an intention, not a measurement."*
* `STATUS.md`: *"Trillion-scale means metadata. … It proves nothing about
  loading weights, because that is not possible and is not claimed anywhere."*
* `crates/q-source/src/dtype.rs` — `fp8_refuses_rather_than_approximates`.
* `architectures/{kimi,deepseek}/plugin.toml` — declared, `implemented = false`.
* `q_cuda::RTX_3090_VRAM_BYTES = 24 GiB`, `USABLE_VRAM_FRACTION = 0.80`.
* `QM-0084`'s measured numbers.

## Requirements Covered

`DOC-003`, `MVP-45`, `MVP-46`.

## Dependencies

`QM-0084`, `QM-0035` (or its waiver).

## Blocks

`QM-0094`.

## Parallelization

Parallel with `QM-0090`, `QM-0093`.

## Program Boundary

Documentation only.

## Scope

* `docs/LIMITATIONS.md`, linked from `README.md`.
* CUDA requirements: toolkit, compute capability, the feature flag, and what is
  and is not verified.
* Supported dtypes and what refuses.
* Measured conversion throughput with the hardware named.
* Every extension point that refuses, by requirement ID.
* **The RTX 3090 arithmetic, explicitly.**

## Out of Scope

`STATUS.md` (`QM-0091`) · marketing copy · future roadmap.

## Files Expected to Change

* `README.md` — a link
* `gpu/cuda/README.md` — reconcile with what `QM-0034`/`QM-0035` achieved

## Files Expected to Add

* `docs/LIMITATIONS.md`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

The section that matters most, stated with arithmetic rather than assertion:

```text
## What "trillion-scale" means here

It means metadata and addressing under bounded memory. It never means loading a
trillion parameters anywhere.

An RTX 3090 has 24 GB of VRAM. At fp32 that is roughly 6×10⁹ parameters if
nothing else were resident — about 0.6 % of a trillion-parameter model, before
any working buffer. At f16, about 1.2 %.

Quatricmorph does not load a model into VRAM. It streams one bounded block at a
time. A trillion-parameter checkpoint cannot be held, and cannot be fully
computed, on one RTX 3090 — or on any single GPU available today.

What is proven: crates/q-catalog/tests/trillion_scale_manifest.rs indexes and
queries a 10¹²-parameter manifest — 47 278 tensors describing 2.10 TB of
payload — using 35.7 MB of peak allocation, opening no artifact at all.
```

## Memory and Performance Constraints

Every number in the document must come from `QM-0084`'s reports or from an
existing verified test. **No estimates.**

## Implementation Plan

1. Write the trillion-scale section with the arithmetic above.
2. CUDA section: toolkit version, `sm_86`, the `cuda` feature, and — honestly —
   which kernels have run on hardware and which have not.
3. Dtype section: f32, bf16, f16 supported; fp8 refuses; unknown dtypes refuse.
4. Architecture support: generic, Llama, Qwen implemented; Kimi and DeepSeek
   declared and never claiming.
5. Throughput from `QM-0084`, with hardware named.
6. Extension points, each with its requirement ID.
7. Every requirement still `Stub` or `Not Started`, by ID.
8. Reconcile `gpu/cuda/README.md` with reality.

## Error Handling

* A number without a measurement → **it does not go in the document**.
* A capability whose status is ambiguous → stated as ambiguous, with the
  requirement ID.
* If no RTX 3090 ran the kernels, the CUDA section says so plainly — that is the
  whole point of the document.

## Acceptance Criteria

1. `docs/LIMITATIONS.md` exists and is linked from `README.md`.
2. The trillion-scale section states the arithmetic and the proven measurement.
3. **No document claims one RTX 3090 can hold or fully compute a 10¹²-parameter
   model** — verified by a text search across the repository.
4. CUDA requirements are stated, including what has never run on hardware.
5. Supported dtypes listed; refusals named.
6. Architecture support listed accurately.
7. Throughput numbers cite `QM-0084` and name the hardware.
8. Every extension point is listed with its requirement ID.
9. Every remaining `Stub` and `Not Started` is listed by ID.
10. `gpu/cuda/README.md` matches reality after `QM-0034`/`QM-0035`.

## Verification Plan

**Automated** — a text search for prohibited claims; a check that every cited
requirement ID exists in `STATUS.md`.
**Manual** — read the document as a sceptical user would.

## Suggested Commands

```bash
# Affirmative-claim search. The bare pattern also matches correct *negations*
# (".plan/MASTER_PLAN.md" and this file both discuss the claim in order to deny
# it), so the second grep strips negated lines. A surviving match is a real claim.
grep -rniE 'trillion.*(vram|gpu memory|fits|loads? into)' --include='*.md' . \
  | grep -viE 'not |never|cannot|no claim|does not|refuse|is not possible' \
  || echo "no affirmative claim found"

grep -oE '\b[A-Z]+-[0-9]{3}\b' docs/LIMITATIONS.md | sort -u          # cross-check vs STATUS.md
```

## Test Cases

| Input | Expected |
| --- | --- |
| Text search for a "1T fits in VRAM" claim | **No match** |
| Every requirement ID in the document | Exists in `STATUS.md` |
| Every number | Traceable to a measurement |
| CUDA section | States what has never run on hardware |
| Dtype section | fp8 listed as refusing |
| Kimi/DeepSeek | Listed as declared, not implemented |
| `gpu/cuda/README.md` | Matches post-`QM-0034` reality |

## Risks

| Risk | Mitigation |
| --- | --- |
| Limitations are softened for presentation | The text search is mechanical; numbers must be traceable |
| The document goes stale | `QM-0094` re-audits it at every release |
| An unmeasured number slips in | Every number cites its source report |

## Completion Evidence

* `docs/LIMITATIONS.md` in full.
* The prohibited-claim search output, empty.
* The requirement-ID cross-check.
* The updated `gpu/cuda/README.md`.
