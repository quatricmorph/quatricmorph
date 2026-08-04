# QM-0160 — Design-partner outreach and NVIDIA Inception

## Status

Ready

**Start on day 1, in parallel with the first engineering task.** Not after the
tool works. The strategy is explicit that this is the ordering solo founders get
wrong, and it is the cheapest task in the plan to start and the most expensive to
start late.

## Phase

Phase 14 — Validation and v1 release

## Objective

Have 3–5 design partners lined up, and an NVIDIA Inception application submitted,
**before** the engine produces its first real diagnosis.

## Repository Evidence

Not a code task. Its inputs are:

* `.plan/VALIDATION_PLAN.md` §2 — segments, the four questions, where to find
  people.
* `.plan/MASTER_PLAN.md` §3 — the value-proposition sentence, to be used verbatim.
* `.plan/DEFINITION_OF_DONE.md` §1 waiver — why Inception matters: 51 GB of free
  disk caps the local checkpoint at 30–40 GB.

## Requirements Covered

`VAL-000` (new). Enables `V1-29`…`V1-32`.

## Dependencies

None.

## Blocks

`QM-0161`, `QM-0162`, `QM-0163`, `QM-0164`.

## Parallelization

Lane V. Touches no code and conflicts with nothing.

## Program Boundary

No repository code. Artifacts live in `.plan/tasks/QM-0160-*/` — outreach log,
value-proposition text, Inception confirmation.

## Scope

* Lock the value-proposition sentence and use it verbatim in every message.
* Identify and contact candidate design partners in the beachhead segment.
* Run first conversations using `VALIDATION_PLAN.md` §2's four questions.
* Submit the NVIDIA Inception application.
* Read the Palace paper (arXiv:2509.26213) before the streaming design is final.
* Keep a dated log.

## Out of Scope

Demos of an unfinished tool · a landing page · a launch post · pricing
(`QM-0163`) · anything measured in stars or views.

## The value-proposition sentence

Verbatim, everywhere — outreach, README, report header, conversation openers:

> **Quatricmorph shows the quantisation error you currently cannot see, so you
> can decide which layers to leave at higher precision.**

One sentence, used consistently, is how a stranger repeats what the product does
to a colleague. Rewording per message loses that.

## The first conversation

Four questions, from `VALIDATION_PLAN.md` §2, asked before any demo:

1. Walk me through the last time a quantisation config surprised you. What did
   you see, and when?
2. What do you look at today to decide which layers stay at higher precision?
3. If you could see where the error concentrates before running an eval, what
   would you do differently?
4. Would you run this on a checkpoint you cannot share with me?

Question 4 is the one that matters. It is `V1-29` in advance, and the answer is
known long before any code is ready.

## Target segments

Beachhead first (`VALIDATION_PLAN.md` §2): model-compression and quantisation
engineers. Then infra/serving. Universities and open-source users are
distribution and credibility, **not** revenue, and time spent there does not
count toward this task.

Channels: quantisation- and vLLM-adjacent Discords, r/LocalLLaMA, ML-infra
circles, maintainers and heavy users of the compression toolchains, direct
outreach.

## Acceptance Criteria

1. The value-proposition sentence is fixed and recorded.
2. At least **10 contacts attempted**, logged with date and channel.
3. At least **3 conversations held**, with the four questions asked and answers
   recorded.
4. For each conversation: the answer to question 4, verbatim.
5. At least **3 partners have agreed to look** at a first diagnosis.
6. The Inception application is submitted; the confirmation is recorded.
7. The Palace paper is read and a short note records where this design agrees
   with it and where it deliberately departs.
8. Conversations that went nowhere are logged too — a log of successes only is
   not evidence.

## Verification Plan

**Manual.** The log is the artifact.

## Test Cases

Not applicable.

## Risks

| Risk | Mitigation |
| --- | --- |
| Deferred until the tool is "ready" | It is `Ready` on day 1 and blocks four later tasks; `EXECUTION_ORDER.md` §10 names it beside the first three actions |
| Conversations drift into demos of nothing | The four questions come before any demo, and there is nothing to demo yet |
| Only friendly contacts are approached | Acceptance criterion 8 requires logging the ones that went nowhere |
| Inception applied for when it is already blocking | Submitted in Days 0–30, before the disk ceiling matters |
| The pitch drifts message to message | Verbatim sentence, criterion 1 |

## Completion Evidence

* The outreach log: date, channel, contact, outcome — including failures.
* Three or more conversation records with all four answers.
* The Inception confirmation.
* The Palace reading note: agreements and deliberate departures.
* The list of partners who agreed to look, and what they expect to see.
