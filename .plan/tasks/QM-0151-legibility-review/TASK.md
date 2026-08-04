# QM-0151 — Legibility review

## Status

Blocked

Unblocks when `QM-0152` reaches `Complete`.

## Phase

Phase 13 — Diagnostic surface — **Gate G4**

## Objective

Establish, with people rather than assertions, that a reader who has never seen
Quatricmorph can identify the most fragile layers from one screenshot — or
establish that they cannot, which is equally valuable.

## Repository Evidence

* `QM-0150`, `QM-0152` — the surface, rendering a real manifest.
* `QM-0141` — the report, which carries the same content in text and is the
  fallback if the visual form fails.
* `.plan/VALIDATION_PLAN.md` §5.1 — the headless pivot this gate feeds.

## Requirements Covered

`V1-25`.

## Dependencies

`QM-0152`.

## Blocks

`QM-0165`. Informs `QM-0161`.

## Parallelization

Lane S. No code unless the review demands a change.

## Program Boundary

No code by default. Findings become tasks.

## Scope

* Show one screenshot of a real diagnosis to at least three people who have not
  seen the tool.
* Ask one question — *"what does this tell you about this model?"* — and record
  the answer verbatim before saying anything else.
* Record failures as prominently as successes.

## Out of Scope

Redesign · adding features in response to a single opinion · demoing the tool
live (that is `QM-0161`).

## Method

Deliberately narrow, because a loose version of this test always passes:

1. **One screenshot**, no narration, no legend explained in advance.
2. **One question**, asked before any explanation: *what does this tell you about
   this model?*
3. Record the answer verbatim, including hesitation and wrong guesses.
4. Only then ask the specific question: *which layers would you leave at higher
   precision?*
5. Record whether they named the top-3 fragile layers, and whether they used the
   colour scale, the redundant encoding, or the adjacent ranked list to do it.

Step 5's last clause matters: if every reader uses the ranked list and ignores the
map, that is a finding about the map, and it is `VALIDATION_PLAN.md` §5.1's
signal.

**Readers should be ML-literate but not compression specialists.** A specialist
knows what to look for and passes the test on prior knowledge; a non-technical
reader fails it for reasons the tool cannot fix.

## Acceptance Criteria

1. At least three readers, none of whom built the tool.
2. Verbatim answers recorded for every reader, including failures.
3. For each: did they name the fragile layers unprompted?
4. For each: which element did they use — map, redundant encoding, or ranked list?
5. A written conclusion, including "the map is not doing the work" if that is
   what the evidence says.
6. Any change made in response is a separate task, not folded into this one.

## Verification Plan

**Manual only.** No automated substitute exists, and inventing one would defeat
the purpose.

## Test Cases

Not applicable — this task's subject is people.

## Risks

| Risk | Mitigation |
| --- | --- |
| Leading the reader | One question, asked before any explanation, recorded verbatim |
| Only successes recorded | Acceptance criterion 2 requires failures |
| A single negative result triggers a redesign | Three readers minimum; changes become their own tasks |
| The gate is quietly skipped as "soft" | It is `V1-25` and a release criterion |

## Completion Evidence

* The screenshot shown.
* Three or more verbatim accounts.
* The tally: named unprompted, yes/no; element used.
* The written conclusion.
* Any tasks filed as a result.
