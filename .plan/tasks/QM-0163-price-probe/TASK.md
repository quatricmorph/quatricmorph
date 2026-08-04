# QM-0163 — Price probe

## Status

Blocked

Unblocks when `QM-0162` reaches `Complete`.

## Phase

Phase 14 — Validation and v1 release

## Objective

Find out whether anyone will pay — not how much, and not to maximise revenue.
A willingness-to-pay signal is PMF signal 4; the number attached to it is noise
at this stage.

## Repository Evidence

* `.plan/VALIDATION_PLAN.md` §3, signal 4.
* Strategy §11, Days 60–90: *"Run a price probe — a paid pilot or a
  design-partner license — to test willingness to pay, not to maximize revenue."*

## Requirements Covered

`VAL-003`, `V1-32`.

## Dependencies

`QM-0162`.

## Blocks

`QM-0165`.

## Parallelization

Lane V.

## Program Boundary

No repository code. No billing infrastructure.

## Scope

* Ask partners who have had a decision-change or repeated-use experience whether
  they would pay, and in what form.
* Record the answer, the form, and the objection.
* Where a yes is real, take it: a paid pilot or a design-partner licence.

## Out of Scope

Building billing · a pricing page · a free tier · a licence server · negotiating
enterprise terms.

## Method

Ask the concrete version, not the hypothetical one. "Would you pay for this?"
gets a polite yes; the useful questions are:

1. Who in your organisation would this budget come from?
2. Is this a tool budget, an infra budget, or a research budget?
3. If I put a number on it, what would make it an easy yes — and what would make
   it an easy no?
4. Would you run a paid pilot for one quarter?

Record the **objection** as carefully as the answer. "We would need it to read our
quantised checkpoints" is a roadmap input (`QUANT-010`, deferred module 2). "We
would need someone else to have used it first" is a distribution problem. "We do
not have a budget for tools" is a segment problem, and it means the beachhead was
chosen wrong.

## Acceptance Criteria

1. At least three willingness-to-pay conversations, with partners who have
   actually used the tool.
2. For each: the budget owner, the budget type, and the objection, recorded.
3. At least one of: a signed pilot, a card entered, or an explicit "we would
   budget for this" — or a written statement that none was obtained.
4. Objections are categorised: roadmap, distribution, or segment.
5. No billing infrastructure was built to run this probe.

## Verification Plan

**Manual.**

## Risks

| Risk | Mitigation |
| --- | --- |
| Politeness read as willingness | The four concrete questions; the budget-owner question filters hardest |
| The probe becomes a pricing exercise | Out of scope; the objective says so |
| Billing gets built for three conversations | Acceptance criterion 5 |
| A "no" is not recorded | Criterion 3 requires the negative statement explicitly |
| Objections are dismissed rather than categorised | Criterion 4 |

## Completion Evidence

* Three or more conversation records with budget owner, type, and objection.
* The outcome: pilot, card, budget statement, or an explicit none.
* The objection categorisation and what it implies for the next module.
