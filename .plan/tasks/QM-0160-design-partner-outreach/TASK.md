# QM-0160 — Design-partner outreach and NVIDIA Inception

## Status

Blocked

Requires a human to send real messages to real people. No agent can satisfy this.
Scaffolding is committed; the acceptance criteria remain unmet.

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
* `.plan/DEFINITION_OF_DONE.md` §1 waiver — why Inception matters: 21 GB of free
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

## Scaffolding

Committed in this directory. Templates and empty structures only — **no record of
anything that happened**, because nothing has happened. See
[`README.md`](README.md) for the index and the marker convention.

| Artifact | Covers | State |
| --- | --- | --- |
| [`VALUE_PROPOSITION.md`](VALUE_PROPOSITION.md) | Criterion 1 | Sentence transcribed from `MASTER_PLAN.md` §3; ratification of the `quantization`/`quantisation` variance unfilled |
| [`MESSAGE_TEMPLATES.md`](MESSAGE_TEMPLATES.md) | Scope: "use it verbatim in every message" | Six templates, complete, `<PLACEHOLDER>` slots per recipient |
| [`TARGET_LIST_SCHEMA.md`](TARGET_LIST_SCHEMA.md) + [`target-list.csv`](target-list.csv) | Scope: "identify … candidate design partners" | Criteria and columns complete; **0 rows** |
| [`OUTREACH_LOG_SCHEMA.md`](OUTREACH_LOG_SCHEMA.md) + [`outreach-log.csv`](outreach-log.csv) | Criteria 2, 8; "keep a dated log" | 12-value outcome enum, 9 of them failures; **0 rows** |
| [`INTERVIEW_GUIDE.md`](INTERVIEW_GUIDE.md) | Criterion 3 | The four questions verbatim, plus protocol |
| [`CONVERSATION_RECORD_TEMPLATE.md`](CONVERSATION_RECORD_TEMPLATE.md) + [`conversations/`](conversations/) | Criteria 3, 4, 5 | Blank form; **0 records** |
| [`INCEPTION_APPLICATION_PREP.md`](INCEPTION_APPLICATION_PREP.md) | Criterion 6 | Repository-grounded facts assembled; submission and confirmation unfilled |
| [`PALACE_READING_NOTE.md`](PALACE_READING_NOTE.md) | Criterion 7 | Comparison structure and identifier verification; **the paper has not been read** |

**All eight acceptance criteria are unmet.** Contacts attempted: 0. Conversations
held: 0. Partners agreed to look: 0. Inception applications submitted: 0. Papers
read: 0. Criterion-by-criterion detail in
[`../../evidence/QM-0160.md`](../../evidence/QM-0160.md).

## Orchestration

| Field | Value |
| --- | --- |
| Controller state | **Blocked — human dependency** |
| Lane | V |
| Wave | 0 |
| Branch | `task/qm-0160-design-partner-outreach` |
| Worktree | `/Users/thanh/Quatricmorph/.qm-worktrees/qm-0160` |
| Base commit | `848621b` |
| Head commit | The single scaffolding commit on this branch — `git rev-parse task/qm-0160-design-partner-outreach`. A commit cannot contain its own hash, so the value is reported by `impl-agent-5` and belongs in `## Merge` of the evidence record rather than being invented here |
| Implementation agent | `impl-agent-5` |
| Evidence record | [`../../evidence/QM-0160.md`](../../evidence/QM-0160.md) |
| Merge path | L |
| Tests added | none — human-dependent scaffolding exempt class |
| Human blocker | **Requires a human to send real messages to real people. No agent can satisfy this. Scaffolding is committed; the acceptance criteria remain unmet.** |

