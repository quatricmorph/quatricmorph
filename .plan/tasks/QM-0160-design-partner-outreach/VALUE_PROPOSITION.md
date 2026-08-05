# The value-proposition sentence

Acceptance criterion 1 is *"the value-proposition sentence is fixed and
recorded."* This file records it. **Fixing it is a ratification act the founder
owes it**, because the repository currently carries two spellings of it, and an
agent may not pick between them on the founder's behalf.

---

## 1. The sentence

Transcribed byte-for-byte from [`../../MASTER_PLAN.md`](../../MASTER_PLAN.md) §3,
which is the source `TASK.md` §"Repository Evidence" names as the one to use
verbatim, and which in turn attributes it to the strategy document §11:

> **Quatricmorph shows the quantization error you currently cannot see, so you can
> decide which layers to leave at higher precision.**

This is the form used in every template in this directory.

## 2. The variance, recorded rather than fixed

`TASK.md` §"The value-proposition sentence" restates it with British spelling —
`quantisation` — matching the prevailing style of `.plan/` (75 occurrences of
`quantisation` against 28 of `quantization` across `.plan/`, `README.md`, and
`STATUS.md` at this commit).

Why the templates use the `MASTER_PLAN` form anyway: `TASK.md` names
`MASTER_PLAN.md` §3 as the source and says "to be used verbatim." A document that
cites its source loses to the source it cites. House style does not override
that.

Why this is still not "fixed": one sentence used inconsistently is exactly the
failure criterion 1 exists to prevent, and the repository is currently
inconsistent. Reconciling `.plan/` documents against each other is `QM-0167`'s
job and outside this task's `## Program Boundary`, so the discrepancy is reported
here and **not** edited in `MASTER_PLAN.md` or `TASK.md`.

### Ratification — the founder decides, once

**Canonical spelling for all external use:**
`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Date ratified:**
`<TO BE FILLED BY A HUMAN — no agent may complete this>`

If the ratified answer is `quantisation`, the change is one case-sensitive
replacement of `quantization` → `quantisation` across this directory, and a note
to `QM-0167` to align `MASTER_PLAN.md` §3. If it is `quantization`, nothing in
this directory changes.

Do not reword the sentence. Reword it per message and a stranger can no longer
repeat it to a colleague, which is the entire reason it is one sentence.

## 3. Where it is used verbatim

Outreach messages · conversation openers · the report header · `README.md`.
Same words, same order, every time.

---

## 4. The honesty block — mandatory, not optional

The sentence is **present tense about a capability that does not exist yet.**
`STATUS.md` at this commit has no quantisation-error engine: `QUANT-001`…`QUANT-006`
(`QM-0120`…`QM-0125`) are unbuilt, there is no report, no ranking, no
mixed-precision frontier, no heat-map, and no code has ever run on a GPU.

The resolution is structural, not editorial. The sentence stays verbatim, and
**every message that carries it carries this block immediately after it.** Not a
footnote, not an appendix — the body of the message, so that it cannot be sent
without it.

> **What exists today:** the streaming and addressing layer. Sharded SafeTensors
> indexed from headers alone; canonical addresses that survive a reopen; exact
> byte-range reads that match Python's `safetensors` value for value; CPU
> reference statistics. Around 290 Rust tests and a web suite cover it.
>
> **What does not exist today:** the quantisation-error engine itself. No
> ranking, no report, no heat-map. Nothing has run on a GPU — the CUDA sources
> have never been compiled. Statistics are computed but not yet persisted. The
> largest checkpoint anything here has actually read is a 1.2 MB fixture — not
> the ≥ 24 GB one the design targets, and not the 339 MB model sitting on disk
> next to it, which nothing has streamed yet either.
>
> **What it will never do:** predict an accuracy or eval delta. That needs an
> inference runtime and a calibration set, which are explicit non-goals. It
> measures **weight-space** error and ranks by it — a proxy for sensitivity, and
> a coarse one. You would still run your own eval on the config it recommends.

Every clause above is traceable to [`../../../STATUS.md`](../../../STATUS.md) and
to [`../../PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §5.2.

### Refresh discipline

The block describes a moving target. Before a send session, re-read `STATUS.md`
and update it — **downward-only claims**, meaning a capability may be added to
"what exists" only when `STATUS.md` marks it `Verified` or `Implemented`, never
because a task is in progress.

**Last checked against `STATUS.md`:**
`<TO BE FILLED BY A HUMAN — no agent may complete this>`

## 5. The four forbidden claims, in outreach form

From [`../../PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §5.2, restated as things
not to say in a message or a conversation.

| Never say | Say instead |
| --- | --- |
| "It predicts how much accuracy you will lose" | "It measures weight-space error. Accuracy impact is not measured — you would run your eval on the recommended config" |
| "It measures layer sensitivity" | "It ranks by relative weight-space error, a proxy for sensitivity" |
| "It handles trillion-parameter checkpoints" | The measured checkpoint size and the measured peak resident set, both printed. The trillion-parameter result is a **metadata** index — 47 278 tensors in 35.7 MB peak, opening no artifact — and it says nothing about streaming real bytes |
| "GPU-accelerated" (today) | The backend that actually ran, named per run. Today that is the CPU reference backend |
| "It found that this layer is fragile" (of a sampled figure) | The fidelity label the result carries: `exact`, `sampled`, or `approximate` |

The third row is the one most likely to be reached for under pressure in a live
conversation, because `CAT-006` is a genuinely impressive number. It is a
metadata result. Letting it stand in for streaming real bytes is wrong in
`DEFINITION_OF_DONE.md` §1's own words, and it would be wrong out loud too.
