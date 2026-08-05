# Palace reading note — STRUCTURE ONLY

**The paper has not been read.** Acceptance criterion 7 is unmet.

What was done: the arXiv identifier was resolved to confirm it is not a dead
reference. What was **not** done: reading the paper, or forming any judgement
about where this design agrees with it or departs from it. Every judgement field
below is empty, and an agent may not fill one. A fabricated "deliberate
departure" would be worse than no note at all — it would look like a design
decision had been reasoned through when it had not.

---

## Identifier verification

| | |
| --- | --- |
| Cited as | `arXiv:2509.26213` (`TASK.md` §Scope; `VALIDATION_PLAN.md` §6) |
| Title | *Palace: A Library for Interactive GPU-Accelerated Large Tensor Processing and Visualization* |
| Authors | Drees, Risse |
| Submitted | 30 September 2025 |
| Retrieved | `https://arxiv.org/abs/2509.26213`, 2026-08-04 |
| Result | **The identifier resolves and the subject matches.** The abstract describes interactive out-of-core tensor processing and visualisation with an asynchronous concurrent architecture |

Abstract page only. The paper itself was not read, and nothing below was derived
from it.

*Recorded variance:* `VALIDATION_PLAN.md` §6's Days 0–30 table assigns the Palace
reading to `QM-0101`; `TASK.md` §Scope and criterion 7 assign it to `QM-0160`.
Both say it must happen before the streaming design is final. Which task owns it
is `QM-0167`'s reconciliation, not this task's — the note is filed here because
criterion 7 lives here.

## Why it is worth the hour

Two independent reasons, both already written down in this repository:

* **`MASTER_PLAN.md` §6, Days 0–30:** *"the closest published prior art to this
  streaming layer, and the plan's chunked pull-based architecture should be a
  deliberate agreement or a deliberate departure, not a coincidence."*
* **`VALIDATION_PLAN.md` §7, competitive watch:** *"Has anyone retargeted an
  out-of-core tensor framework at ML checkpoints? Would remove the systems moat
  directly."* Palace is a general out-of-core GPU tensor framework
  (`STRATEGY_ALIGNMENT.md` §2). If retargeting it at SafeTensors checkpoints is a
  weekend, the systems moat is thinner than the plan assumes, and that is worth
  knowing before `QM-0030` and `QM-0101` are built rather than after.

Read it **before the streaming design is final** — that is `QM-0030` (bounded
streaming block reader) and `QM-0101` (the residency proof). Reading it after is
the same cost with none of the benefit.

## What to compare — the decisions the note has to cover

The left column is grounded in this repository; the rest is for the reader. Mark
each `agrees`, `departs`, or `not addressed by the paper`, and where it departs,
say **why** — a departure with no reason is a coincidence with better wording.

| This design | Where it is specified | Agrees / departs / not addressed | Reason, if it departs | Consequence if the paper is right and this is wrong |
| --- | --- | --- | --- | --- |
| Chunked, **pull-based** streaming rather than a push pipeline | `QM-0030`, `TILE-009` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Bounded residency as a **named, enforced budget** — peak RSS ≤ 1.25 × `C`, `C ≤ 2 GB` | `SRC-017`, `V1-03`, `AC-001` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Byte-range reads over memory-mapped local files; whole-tensor reads **refused** | `SRC-005`, `WQL-011` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| A canonical address space independent of file layout | `NSIR-004` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Metadata catalog separated from payload; manifest indexed without opening artifacts | `CAT-006` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Exactness as a **type**: `exact` / `sampled` / `approximate` carried end to end | `SRC-018`, `STAT-005` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Six-level LOD ladder; only the finest level carries exact values | `TILE-001` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Layered cache L0–L4, content-addressed, tiers refuse rather than miss silently | `CACHE-001`…`CACHE-007` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| CPU reference backend is ground truth; every other backend is differentially verified against it | `GPU-001`, `GPU-002` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Synchronous refusal (`NotImplemented` + requirement ID) over asynchronous approximation | `PRODUCT_SCOPE.md` §2 | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Domain-specific to ML checkpoints (SafeTensors, architecture resolvers) rather than general tensors | `NSIR-*`, `SRC-*` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |

## The two questions the note must answer explicitly

Criterion 7 asks for agreements and deliberate departures. These two are the ones
with a consequence attached, and each needs a paragraph rather than a table cell.

**1. Is there a technique in the paper that this design should adopt outright?**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**2. Could Palace be retargeted at SafeTensors checkpoints, and how long would it
take?** This is `VALIDATION_PLAN.md` §7's moat question. An honest answer of
"weeks" belongs in `RISK_REGISTER.md`, not in this file's margin.

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

---

| Field | Value |
| --- | --- |
| Date read | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Read before `QM-0030` / `QM-0101` were finalised | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Follow-on tasks raised | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| `RISK_REGISTER.md` entry needed | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
