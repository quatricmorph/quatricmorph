# QM-0162 — Documented decision-change case

## Status

Blocked

Unblocks when `QM-0161` reaches `Complete`.

## Phase

Phase 14 — Validation and v1 release — **Gate G5**

## Objective

Document one case in which Quatricmorph's output **changed a real engineering
decision**.

This is the only acceptance criterion in the plan that may not be waived. It is
the whole point of v1.

## Repository Evidence

* `.plan/VALIDATION_PLAN.md` §3 — PMF signal 3, and §4's kill criteria, which are
  defined by its absence.
* `.plan/MASTER_PLAN.md` §9 — G5, stated as a gate that cannot be closed by
  writing code.
* The strategy document §10: *"a documented case where the tool changed a real
  engineering decision (a fragile quant config was dropped, routing was
  rebalanced, a bad merge was aborted before shipping)."*

## Requirements Covered

`VAL-002`, `V1-30`.

## Dependencies

`QM-0161`.

## Blocks

`QM-0165` — the release.

## Parallelization

Lane V. Alone: it needs a person, not a lane.

## Program Boundary

No repository code.

## What counts

A decision that **would have gone the other way** without the tool, with a
before, an output, and an after.

Qualifying examples:

* A quantisation config was dropped or changed because the diagnosis showed error
  concentrated in layers the engineer had assumed were safe.
* Specific layers were kept at higher precision, with the byte cost accepted, on
  the strength of the frontier.
* A planned uniform-precision rollout was replaced by a mixed-precision one.
* A config was **kept** after the diagnosis showed the fragile layers were ones
  they had already protected — a confirmed decision is a changed decision only if
  the confirmation was in doubt, and the record must say which.

Not qualifying:

* "This is really useful." — an opinion, not a decision.
* "I'll definitely use this." — a future intention.
* The founder deciding something about the founder's own test model.
* A decision the engineer had already made before running the tool.

## Method

1. From `QM-0161`'s answer to *what would you do differently?*, follow up.
2. Ask what they actually did, and when.
3. Record the decision **before** (what they were going to do), the **output**
   (which layers, which numbers), and the decision **after**.
4. Get it in their words where possible — a quote is worth more than a summary,
   for the write-up and for the founder's own honesty.
5. Record whether they would have found it another way. If they say yes, that is
   part of the record — the case is weaker and the plan should know it.

## Acceptance Criteria

1. A named engineer (named to the founder; publishable anonymously) made a
   decision.
2. The before, the output, and the after are all recorded.
3. The decision is specific — a config, a layer set, a precision — not a
   sentiment.
4. Their own words are recorded where they were willing.
5. The counterfactual is recorded: would they have found this another way?
6. The date is recorded.
7. If no case exists after genuine attempts, **that is written down as the
   result** and `VALIDATION_PLAN.md` §4's kill criteria are consulted rather than
   quietly deferred.

Criterion 7 is what makes this an experiment rather than a formality.

## Verification Plan

**Manual.** The record is the artifact. There is no automated version of this and
building one would be a way of avoiding it.

## Risks

| Risk | Mitigation |
| --- | --- |
| Enthusiasm recorded as a decision | Criterion 3: a config, a layer set, a precision |
| The founder's own decision counts as the case | Explicitly excluded |
| The gate is quietly waived to ship | `DEFINITION_OF_DONE.md` §9 states it may not be waived |
| A weak case is inflated | The counterfactual question is an acceptance criterion |
| Absence is treated as "not yet" indefinitely | Criterion 7 routes to the kill criteria, which have a month-6 horizon |

## Completion Evidence

* The dated case: who, what they were going to do, what the tool showed, what
  they did.
* Their own words.
* The counterfactual answer.
* Or: a written statement that no case was found, what was attempted, and what
  `VALIDATION_PLAN.md` §4 says to do about it.
