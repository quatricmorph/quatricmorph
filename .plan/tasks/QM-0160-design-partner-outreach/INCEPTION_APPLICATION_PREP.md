# NVIDIA Inception — application preparation

**No application has been submitted. No confirmation has been received. No
credits have been granted.** Acceptance criterion 6 is unmet and cannot be met by
an agent: submitting an application to an external organisation on behalf of a
real company is a human act with legal weight.

What this file is: the repository-grounded facts an application needs, assembled
so the human is not reconstructing them from `STATUS.md` at 1 a.m., plus the
fields where the outcome gets recorded.

> **Not verified against the live application.** This file does **not** reproduce
> NVIDIA Inception's form, questions, eligibility rules, or terms. Nothing here
> was checked against NVIDIA. Read the actual application when you open it; where
> it asks for something not covered below, the answer is not in this repository
> and you will have to write it.

---

## 1. Why this is on the critical path at all

[`../../DEFINITION_OF_DONE.md`](../../DEFINITION_OF_DONE.md) §1, the checkpoint-size
waiver:

* The development machine has **51 GB of free disk**, which caps v1's headline
  checkpoint at roughly **30–40 GB**.
* The strategy document's 1.5 TB frontier-MoE example is therefore **not provable
  on this hardware**, and v1 claims nothing about it.
* What closes the gap: external NVMe, **or NVIDIA Inception credits**. Neither is
  a v1 blocker.

And [`../../VALIDATION_PLAN.md`](../../VALIDATION_PLAN.md) §2 on timing: apply in
Days 0–30, because *"applying late is the only way it can become blocking."* The
application is cheap now and expensive at the moment the disk ceiling actually
bites.

There is a second, sharper reason. `STATUS.md` §"CUDA / GPU" records that **no
code in that section has ever run on a GPU** — `gpu/cuda/*.cu` has never been
compiled, and `CUDA-002`…`CUDA-005` carry no tests because none can be written
here. The target is an RTX 3090-class 24 GB device that was not available in the
environment the code was written in. GPU access converts five
`Hardware-Unverified` rows into either `Verified` or a bug report. That is a
concrete, checkable use of credits rather than a general wish for compute.

*Programme characterisation.* `VALIDATION_PLAN.md` §2 describes Inception as
"free, no equity, GPU cloud credits." That is **this repository's** understanding,
recorded in August 2026, not a verified statement of current programme terms.
Confirm the terms yourself before relying on any of them.

## 2. Facts about the project, ready to quote

All traceable to [`../../../STATUS.md`](../../../STATUS.md) at this commit. Do not
inflate any of them; §3 lists the ones that would be easy to.

**What it is.** Quatricmorph shows the quantization error you currently cannot
see, so you can decide which layers to leave at higher precision. An out-of-core
diagnostic over open-weight SafeTensors checkpoints that ranks tensors and layers
by weight-space quantisation error, so an engineer can choose a mixed-precision
config before running an eval.

**What is built, tested, and passing.**

| | |
| --- | --- |
| Rust workspace | 17 crates, ~15 200 lines |
| Test count | 290 Rust tests passing, plus a web suite; `cargo fmt` and `clippy -D warnings` clean |
| Ingestion | Single-file and sharded SafeTensors, header-only; cancellable and resumable; corrupt headers, duplicate names and unknown dtypes refused rather than guessed |
| Addressing | Canonical address space stable across reopen; generic and Llama resolvers; MoE expert addressing; ambiguous aliases return candidates, never a silent pick |
| Exactness | f32 / bf16 / f16 decoded exactly including subnormals; scalar and slice reads verified against Python's `safetensors` on golden values |
| Memory discipline | Named, enforced budgets; access scale is a type, not a comment; whole-tensor reads refused with an explanation |
| Metadata scale | A synthetic 10¹²-parameter **manifest** — 47 278 tensors describing 2.10 TB of payload — indexed and queried at **35.7 MB peak allocation**, opening no artifact |
| Query layer | WeightQL parses, resolves, shape-checks before execution, estimates I/O, and executes scalar and slice reads; no arbitrary code execution, enforced by a closed enum and tested |

**What is not built.** The quantisation-error engine (`QUANT-001`…`QUANT-006`),
the report and manifest, the heat-map, GPU execution of any kind, persisted
statistics, and the cache wiring. Nothing renders. `STATUS.md` §"What a reader
should not be surprised by" is the list, and it is short and blunt on purpose.

**What the hardware is for.** Streaming a ≥ 24 GB checkpoint under a measured
resident-byte ceiling (`V1-03`: peak RSS ≤ 1.25 × a ceiling `C ≤ 2 GB`), and
compiling and differentially verifying the CUDA kernels against the CPU reference
backend for the first time.

**Market context**, from [`../../STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) §2:
quantisation-error tooling in production is still tabular — five scalar metrics
per tensor — and the window is judged at 12–18 months.

## 3. Claims that must not appear in the application

The same discipline as the outreach copy, and it matters more here because an
application is a written record given to a third party.
[`../../PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §5.2 is the standard.

* **Not** "handles trillion-parameter checkpoints." `CAT-006` is a metadata index.
  Say "indexes a trillion-parameter manifest in 35.7 MB of peak allocation" and
  let the number do its own work.
* **Not** "GPU-accelerated." No kernel has been compiled. Say the CUDA sources
  exist, are unverified, and that hardware access is what would verify them —
  which is a better reason to grant credits than a claim that they are already
  working.
* **Not** a predicted accuracy or eval delta. Weight-space error, and a stated
  proxy relationship to sensitivity.
* **Not** a partner count, a user count, a pilot, or revenue. All four are zero,
  and criteria 2–5 of this very task are unmet.
* **Not** a benchmark number that was never measured. The only measured
  performance figure in the repository is `CAT-006`'s 35.7 MB peak.

## 4. Facts only a human holds

Not in this repository, and not inferable from it. An agent that filled any of
these in would be inventing a company.

| Field | Value |
| --- | --- |
| Legal entity name | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Incorporation status and jurisdiction | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Founder name and role | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Contact email | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Website or public repository URL | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Funding status | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Headcount | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Founding date | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |

## 5. Submission and confirmation — criterion 6

Criterion 6 is *"the Inception application is submitted; the confirmation is
recorded."* Both halves are below and both are empty.

| Field | Value |
| --- | --- |
| Date submitted | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Application or reference number | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Confirmation received (date, form) | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Where the confirmation artifact is filed | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Outcome | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Credits or resources granted, if any | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Terms accepted — anything that constrains the project | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |

Record a rejection here too, with the same care. `VALIDATION_PLAN.md` §2 says the
credits are an escape route from the disk ceiling, not a dependency — a rejection
changes the route (external NVMe), not the plan, and `DEFINITION_OF_DONE.md` §1
already says neither is a v1 blocker.
