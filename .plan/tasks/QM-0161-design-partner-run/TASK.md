# QM-0161 — Design-partner run on a checkpoint the founder did not choose

## Status

Blocked

Unblocks when `QM-0143`, `QM-0141`, and `QM-0152` reach `Complete`, and
`QM-0160` has partners lined up.

## Phase

Phase 14 — Validation and v1 release

## Objective

Get a real engineer to run Quatricmorph on a checkpoint **they** chose — ideally
one the founder cannot see — and record what happened.

## Repository Evidence

* `QM-0143` — the CLI a partner actually runs.
* `QM-0141` — the report they read and can share.
* `QM-0152` — the surface, if they open it.
* `.plan/VALIDATION_PLAN.md` §3 — this is PMF signal 1, the highest-priority one.

## Requirements Covered

`VAL-001`, `V1-29`.

## Dependencies

`QM-0160`, `QM-0143`, `QM-0141`, `QM-0152`.

## Blocks

`QM-0162`, `QM-0163`, `QM-0164`.

## Parallelization

Lane V.

## Program Boundary

No repository code. Findings become tasks.

## Scope

* At least one partner runs the tool on a checkpoint of their choosing.
* Record: which model, which config, what the output said, what surprised them,
  what they did next.
* Record every friction point in the setup, verbatim.
* A private checkpoint is the strongest form of this signal.

## Out of Scope

Building features requested during the session · a live-demo substitute where the
founder drives · pricing (`QM-0163`).

## Why "a checkpoint the founder did not choose" matters

A diagnosis of a model the founder selected proves the tool runs. A diagnosis of
a model the partner selected proves the tool is **useful to someone else** — and
a partner willing to point it at a checkpoint they cannot share has decided it is
worth the trust. That is the difference between signal 1 and a demo.

Falling back to a public checkpoint the partner chose is still valid evidence and
must be recorded as the weaker form it is.

## Method

1. Give them the CLI and the README. **Do not drive.**
2. Watch, or ask for a transcript. Note every point where they get stuck — a
   confusing flag, an unclear default, an error message that does not say what to
   do.
3. Let them read the report unaided before explaining anything.
4. Ask afterwards: *did this show you anything you did not already know?*
5. Ask: *what would you do differently because of it?* — that answer is the seed
   of `QM-0162`.
6. Record friction verbatim. "It was fine" is not a record.

## Acceptance Criteria

1. At least one partner ran the tool themselves, on a checkpoint they chose.
2. Whether the checkpoint was private is recorded.
3. Model, config, and the resulting ranking and frontier are recorded.
4. Their answer to *did this show you anything you did not already know?* is
   recorded verbatim.
5. Every friction point is logged, with the exact wording of any confusing
   message.
6. Their answer to *what would you do differently?* is recorded verbatim.
7. Nothing was changed in the tool mid-session to make it work — if something had
   to be, that is the finding and it is written down.

## Verification Plan

**Manual.** The record is the artifact.

## Risks

| Risk | Mitigation |
| --- | --- |
| The founder drives and the session proves nothing | Criterion 1: they ran it |
| Friction is smoothed over in the retelling | Criterion 5 requires verbatim wording |
| A single enthusiastic partner is treated as validation | This is signal 1 of four; `QM-0164` tests whether they come back |
| Feature requests become the roadmap | Requests are logged; changes are separate tasks judged against `PRODUCT_SCOPE.md` |
| Setup friction blocks the run entirely | That is itself the finding — record it and file the fix |

## Completion Evidence

* Who, when, which model, private or public.
* The config they chose and why.
* The report they got (redacted if private).
* Verbatim answers to both questions.
* The full friction log.
* Any tasks filed as a result.
