# Phase 14 — Validation and v1 release

## Goal

```text
a working diagnostic
→ in the hands of engineers who did not build it
→ running on checkpoints the founder did not choose
→ changing at least one real engineering decision
→ documented, priced, and released without overclaiming
```

## The phase that cannot be completed alone

Every other phase in this plan describes work an engineer finishes by writing
code. This one does not, and that is the point. The strategy's single largest
assumption is:

> That engineers working on quantization, fine-tuning, and MoE routing will change
> a real decision, and pay, because of what Quatricmorph reveals. *(§12)*

`QM-0162` — a documented decision-change case — is the only acceptance criterion
in [`../../DEFINITION_OF_DONE.md`](../../DEFINITION_OF_DONE.md) that **may not be
waived**. Waiving it means shipping a demo and calling it a product.

## This phase starts on day 1

`QM-0160` runs in parallel with the first engineering task, not after the last
one. The strategy is explicit that partner conversations precede polish, and this
is the ordering solo founders most reliably get wrong: two months of building,
then a search for someone who cares.

## Entry conditions

* `QM-0160`: **none.** It starts immediately.
* `QM-0161` onward: a working end-to-end run (G3 passed) and a report worth
  showing someone.

## Tasks

| ID | Title | Kind | Lane | Requirements |
| --- | --- | --- | --- | --- |
| `QM-0160` | Design-partner outreach and NVIDIA Inception application | Validation | V | `VAL-000` |
| `QM-0161` | Design-partner run on a checkpoint the founder did not choose | Validation | V | `VAL-001`, `V1-29` |
| `QM-0162` | Documented decision-change case | Validation | V | `VAL-002`, `V1-30` |
| `QM-0163` | Price probe | Validation | V | `VAL-003`, `V1-32` |
| `QM-0164` | Repeated-use log | Validation | V | `VAL-004`, `V1-31` |
| `QM-0165` | v1 release audit | Verification | V | all `V1-*` |
| `QM-0166` | Technical write-up | Documentation | V | — |
| `QM-0167` | Root-document amendment | Documentation | V | `V1-H5` |

`QM-0090`, `QM-0091`, `QM-0092`, `QM-0093` (Phase 09) supply the documentation
work this phase audits.

## Exit conditions — Gate G5 and release

1. `V1-30` is satisfied **without a waiver**.
2. Every other `V1-*` criterion is satisfied or carries a written waiver naming
   the reason, the requirement ID, and the task that would close it.
3. `STATUS.md` is regenerated from a real run and contains no row whose status is
   more favourable than its evidence.
4. Root documents no longer contradict `.plan/` about what v1 is (`QM-0167`).
5. No document or UI string claims a capability the tests do not demonstrate.

## If the signals do not appear

[`../../VALIDATION_PLAN.md`](../../VALIDATION_PLAN.md) §4 holds the kill criteria
and §5 the pivot criteria, written down in advance so that the response is a
decision rather than a mood.

The response that is explicitly **wrong**: building module 2 because module 1 did
not land, or resuming the deferred platform because "the report needed a better
UI." Both convert a clear negative result into an ambiguous one and spend the
remaining window doing it.
