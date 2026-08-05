# The first conversation

**It is not a demo.** There is nothing to demo — see
[`VALUE_PROPOSITION.md`](VALUE_PROPOSITION.md) §4 — and `TASK.md` §"Risks" names
"conversations drift into demos of nothing" as a live risk with the four
questions as its mitigation.

Twenty-five minutes. Four questions. You are there to be told something you did
not already believe.

---

## 1. The four questions, verbatim

From [`../../VALIDATION_PLAN.md`](../../VALIDATION_PLAN.md) §2. Ask them in this
order; the order is doing work.

> 1. **Walk me through the last time a quantisation config surprised you. What did
>    you see, and when did you see it?**
> 2. **What do you look at today to decide which layers stay at higher precision?**
> 3. **If you could see where the error concentrates before running an eval, what
>    would you do differently?**
> 4. **Would you run this on a checkpoint you cannot share with me?**

*Recorded variance:* `TASK.md` §"The first conversation" abbreviates question 1 to
"What did you see, and when?" `VALIDATION_PLAN.md` §2 is the source `TASK.md`
cites and carries the fuller form, so the fuller form is used here. Reconciling
the two documents is `QM-0167`'s job, not this task's.

## 2. Why each one is there

**Q1 — the last time it surprised you.** A memory, not an opinion. Opinions about
tooling are free; a specific incident carries the cost, the delay, and the
detection point. *"When did you see it"* is the half people forget to ask: if the
answer is "after a week of evals," the product's entire value is the gap between
the config and that week.

**Q2 — what you look at today.** Establishes the baseline the product has to beat.
The honest possible answer is "nothing, I quantise everything and check the eval,"
and that answer is more useful than any feature request.

**Q3 — what would you do differently.** The counterfactual. If the answer is "not
much, I'd still run the eval," the diagnosis is informational rather than
decision-changing, and `V1-30` — the documented decision change, the one
`DEFINITION_OF_DONE.md` says may not be waived — is in trouble. Better to hear it
in week one.

**Q4 — a checkpoint you cannot share with me.** The one that matters.
`VALIDATION_PLAN.md` §3 makes a partner importing their own private checkpoint
the first PMF signal (`V1-29`), and §2 says plainly that it is answered by whether
they say yes to this question, **not by anything on a screen.** It also encodes
the deployment constraint: yes means it must run on their machine, which the local
daemon and local-file architecture already assume.

Record the answer to Q4 **verbatim** — criterion 4. A hedge is data. "Depends on
what it phones home" is a different answer from "no," and paraphrasing loses it.

## 3. Protocol

**Before**

* Confirm with Template E. Get consent for notes or recording *at the start of
  the call*, out loud, and write down what they agreed to.
* Re-read [`VALUE_PROPOSITION.md`](VALUE_PROPOSITION.md) §5 — the five things not
  to say. Under time pressure, `CAT-006`'s trillion-parameter number is the one
  that will try to come out of your mouth. It is a metadata index.
* Open a copy of
  [`CONVERSATION_RECORD_TEMPLATE.md`](CONVERSATION_RECORD_TEMPLATE.md).

**During**

* Open with the value-proposition sentence verbatim, then the honesty block. Then
  stop talking.
* Ask the four questions. Follow-ups are fine; replacements are not.
* Aim for **20 minutes of them, 5 of you.** If you are past minute ten and still
  explaining, the conversation has already failed.
* Write their words down as their words. The record has a verbatim field for a
  reason.

**Not during**

* No demo. Not "just a quick look at the CLI." There is no diagnosis to show, and
  showing header parsing invites a judgement on a thing that is not the product.
* No pitching the roadmap. They cannot validate something that does not exist.
* No asking for their checkpoint, their config, or anything under NDA.
* No arguing with an answer you did not want.

**After**

* Fill the record the same day. Not tomorrow.
* Template F for the "agree to look" ask, and capture **what they expect to see**
  — the half that always gets lost.
* Update [`outreach-log.csv`](outreach-log.csv): `outcome`, `outcome_date`,
  `conversation_record`, `q4_answer_recorded`, `agreed_to_look`.

## 4. What counts as a conversation

Criterion 3 needs three, so the bar has to be written down before it is
convenient to lower it.

**Counts:** all four questions asked, of someone in a revenue-bearing segment
(§1 of [`TARGET_LIST_SCHEMA.md`](TARGET_LIST_SCHEMA.md)), with the answers
recorded and Q4 verbatim. Written or asynchronous is fine — a thoughtful reply
answering all four is a conversation.

**Does not count:** a chat with a friend or colleague; a conversation with the
`academic` segment (log it, exclude it from the count); a call that turned into a
demo; a conversation where Q4 was never asked; "sounds cool" with no answers.

## 5. The answer that should worry you most

Not "no." **"Sure, sounds useful"** with no incident behind Q1 and no
counterfactual behind Q3. That is politeness, it costs the speaker nothing, and
it is indistinguishable from validation until months of building have gone into
it. `VALIDATION_PLAN.md` §3's list of things that are explicitly *not* signals —
stars, views, "this is really cool," conference interest — is the same failure
arriving through a different door.

A flat, specific "no, weight-space error isn't what I'd act on" is worth more
than five warm conversations, and it is worth it in week one rather than month
six.
