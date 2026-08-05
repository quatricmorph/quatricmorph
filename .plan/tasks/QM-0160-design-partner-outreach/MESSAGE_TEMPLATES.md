# Outreach message templates

Reusable copy. `<PLACEHOLDERS_IN_CAPITALS>` are swapped per recipient at send
time by a human. Nothing here is addressed to anyone; no recipient exists.

**Every template carries the value-proposition sentence verbatim and the honesty
block from [`VALUE_PROPOSITION.md`](VALUE_PROPOSITION.md) §4.** Deleting the
honesty block to make a message shorter is the one edit that is not allowed.

### A note on spelling, until ratification

Two repository documents are quoted verbatim below and they spell the word
differently: the value-proposition sentence comes from `MASTER_PLAN.md` §3
(`quantization`) and the four questions come from `VALIDATION_PLAN.md` §2
(`quantisation`). Surrounding prose follows the value-proposition sentence for
now. Once [`VALUE_PROPOSITION.md`](VALUE_PROPOSITION.md) §2's ratification field
is filled, one case-sensitive replacement across this file settles it.

---

## Template A — cold direct message

The primary one. Email, DM, or forum PM. Do not lengthen it.

> **Subject:** four questions about quantization configs — no demo, nothing to sell
>
> Hi `<RECIPIENT_NAME>`,
>
> I'm building a tool, and I'd like 25 minutes of your time *before* it works
> rather than after.
>
> One sentence, so you can decide in ten seconds whether this is even your
> problem:
>
> > Quatricmorph shows the quantization error you currently cannot see, so you
> > can decide which layers to leave at higher precision.
>
> Where it actually is, plainly. **Built and tested:** sharded SafeTensors
> indexed from headers alone, canonical addresses that survive a reopen, exact
> byte-range reads that agree with Python's `safetensors` value for value, CPU
> reference statistics. **Not built:** the quantization-error engine itself — no
> ranking, no report, no heat-map, and no code has ever run on a GPU. The largest
> checkpoint it has actually read so far is a 1.2 MB fixture — not the ≥ 24 GB
> one the design targets, and not the 339 MB model sitting on disk next to it,
> which nothing has streamed yet either. **What it will never do:** predict an
> accuracy or eval
> delta. That needs an inference runtime, which is a deliberate non-goal. It
> measures weight-space error and ranks by it — a proxy for sensitivity, and a
> coarse one.
>
> So there is nothing to demo, and I'm not going to pretend otherwise.
>
> I'm asking you specifically because `<SPECIFIC_THING_THEY_PUBLISHED_OR_SHIPPED —
> one factual clause, something you actually read>`, which puts you closer to this
> decision than almost anyone else I could ask.
>
> Four questions, 25 minutes, `<DATE_OPTIONS>`:
>
> 1. Walk me through the last time a quantisation config surprised you. What did
>    you see, and when did you see it?
> 2. What do you look at today to decide which layers stay at higher precision?
> 3. If you could see where the error concentrates before running an eval, what
>    would you do differently?
> 4. Would you run this on a checkpoint you cannot share with me?
>
> If the answer is no, that is genuinely useful too, and I'll stop there.
>
> `<YOUR_NAME>`
> `<YOUR_CONTACT>`

**Why it is shaped this way.** The sentence goes near the top so an unqualified
reader can leave in ten seconds — that is a feature. The honesty block goes
*before* the ask, not after it, so the ask is made against an accurate picture.
The questions are printed in full so the recipient can answer by reply if a call
is more than they want to give; a written answer to question 4 counts.

---

## Template B — reply in context

For a Discord, forum, or issue thread where the person has *just* described a
quantization surprise. Shorter, because the context is already established, and
because a wall of text in a live channel reads as a pitch.

> `<ONE_LINE_RESPONDING_TO_WHAT_THEY_ACTUALLY_SAID — substantive, not flattery>`
>
> I'm building something aimed exactly at that: Quatricmorph shows the
> quantization error you currently cannot see, so you can decide which layers to
> leave at higher precision. Being straight — the error engine isn't built yet.
> What works today is the streaming and addressing layer: header-only indexing of
> sharded SafeTensors and exact byte-range reads. No ranking, no report, nothing
> on a GPU, nothing to demo. And it will never predict an accuracy delta — it
> measures weight-space error, which is a proxy.
>
> Would you be up for 25 minutes so I can ask four questions about how you make
> that call today? Happy to keep it here in writing if that's easier.
>
> `<YOUR_CONTACT>`

**Channel discipline.** Reply in the thread where the problem was raised. Do not
cross-post the same message to several channels on the same day; do not DM
someone who has not replied in the thread. `VALIDATION_PLAN.md` §2 names the
channels — quantisation- and vLLM-adjacent Discords, r/LocalLLaMA, ML-infra
circles, maintainers and heavy users of the compression toolchains — and each has
its own self-promotion rule. Read it before the first post, not after the ban.

---

## Template C — follow-up

**One.** Not two. Sent `<FOLLOW_UP_DAYS>` days after Template A, then the row is
closed as `no-reply` and left alone.

> Hi `<RECIPIENT_NAME>` — following up once on the note below, then I'll leave
> you alone.
>
> Still no demo; still four questions. If quantization configs aren't your
> problem these days, just say so and I'll close the loop.
>
> `<YOUR_NAME>`

A second follow-up converts a neutral non-answer into a negative impression, and
the log has no column that makes it worth it.

---

## Template D — acknowledging a no

Send it. Acceptance criterion 8 requires the conversations that went nowhere to
be logged, and a graceful close is what makes someone answer a different question
in six months.

> Understood, and thanks for the straight answer — that's useful in itself.
>
> If you ever want the thing that doesn't exist yet, it'll be at
> `<YOUR_CONTACT>`. Good luck with `<THING_THEY_ARE_WORKING_ON>`.

Log the outcome as `declined`, `wrong-fit`, or `deferred` — the distinction
matters later, and it is not recoverable from memory.

---

## Template E — confirming a scheduled conversation

> `<DATE_AND_TIME_WITH_TIMEZONE>`, `<CALL_LINK_OR_CHANNEL>`, 25 minutes.
>
> To set expectations: I'm not going to show you anything. It's four questions
> about how you decide what stays at higher precision today. Nothing is recorded
> unless you say yes at the start, and I won't ask you to share a checkpoint, a
> config, or anything under NDA.
>
> If it turns out I'm wasting your time, say so at minute five and we'll stop.

The "nothing is recorded unless you say yes" line is load-bearing: the
[`CONVERSATION_RECORD_TEMPLATE.md`](CONVERSATION_RECORD_TEMPLATE.md) has a
consent field, and question 4 is about trust with a private checkpoint. Asking
for consent badly poisons the question you most need answered.

---

## Template F — after the conversation, the "agree to look" ask

This is the one that closes acceptance criterion 5, and it is a *separate* ask
made *after* the four questions, never bundled into the first message.

> Thanks — `<ONE_SPECIFIC_THING_THEY_SAID_THAT_CHANGED_YOUR_MIND>` was the part I
> didn't expect.
>
> One ask: when there's a first real diagnosis — a ranked list of which tensors
> carry the most weight-space error under a given config, with the numbers
> labelled `exact` or `sampled` — would you look at it and tell me whether it's
> wrong?
>
> If yes: what would it have to show for the ten minutes to be worth it? I'd
> rather build to your answer than guess.

Record their answer to the second question verbatim. "The list of partners who
agreed to look, **and what they expect to see**" is what the task's Completion
Evidence asks for, and the second half is the part that is always lost.

---

## What never goes in a message

* A screenshot of something that does not compute a diagnosis.
* A number from `CAT-006` presented as streaming performance. It is a metadata
  index — 47 278 tensors, 35.7 MB peak, no artifact opened.
* "GPU-accelerated." No kernel in this repository has ever been compiled or run.
* A predicted accuracy delta, an eval delta, or a quality guarantee.
* A request for their checkpoint, their config, or anything covered by an NDA.
  Question 4 asks whether they *would* run it on a checkpoint they cannot share.
  Asking them to actually share one destroys the answer.
* A deadline, a scarcity claim, or a fake cohort ("I'm only taking five
  partners"). There is no cohort.
