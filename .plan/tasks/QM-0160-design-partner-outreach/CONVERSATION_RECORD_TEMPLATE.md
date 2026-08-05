# Conversation record — TEMPLATE

**This file is a blank form. It records no conversation, because no conversation
has been held.**

Copy it to `conversations/<YYYY-MM-DD>-<TARGET_ID>.md` and fill it in by hand,
the same day as the call. Every `<TO BE FILLED BY A HUMAN — no agent may complete
this>` marker below must be replaced with something a real person actually said
or did. **No agent may replace any of them.** An invented answer here is not a
placeholder — it is a fabricated interview with a fabricated engineer, and it
would contaminate `QM-0161`, `QM-0162`, `QM-0163`, and `QM-0164`, all of which
read this evidence downstream.

Leave a marker in place if you did not get an answer. "Not answered" is a
finding; a filled-in guess is not.

---

## Session

| Field | Value |
| --- | --- |
| Date | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| `target_id` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| `log_id` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Segment | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Channel / medium | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Duration | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Consent given for notes | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Consent given for recording | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| May be quoted, attributed | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| May be quoted, anonymised | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Value-proposition sentence used verbatim | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| Honesty block delivered before the questions | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |

If both quotation fields are unfilled or negative, nothing from this record may
appear in any external document, including `QM-0166`'s write-up.

---

## Q1 — Walk me through the last time a quantisation config surprised you. What did you see, and when did you see it?

**Their answer:**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Was there a specific incident, or a general impression?**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**How long between the config and noticing?**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**What did it cost — compute, wall clock, a shipped regression?**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

---

## Q2 — What do you look at today to decide which layers stay at higher precision?

**Their answer:**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Tools, scripts, or heuristics named:**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Is there a written-down rule, or is it per-model judgement?**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

---

## Q3 — If you could see where the error concentrates before running an eval, what would you do differently?

**Their answer:**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**A concrete different action, or "probably nothing"?** *(the honest second
answer is the more valuable one — see* `INTERVIEW_GUIDE.md` *§2)*

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Would it replace the eval, or come before it?**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

---

## Q4 — Would you run this on a checkpoint you cannot share with me?

**Verbatim answer — their words, not a summary.** Acceptance criterion 4 requires
this field specifically, and paraphrase does not satisfy it.

> `<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Conditions attached (network, telemetry, licence, approval, air-gap):**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Who else would have to approve it:**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

---

## The "agree to look" ask (Template F)

**Did they agree to look at a first diagnosis?**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**What they expect to see for it to be worth their time — their words:**

> `<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Which checkpoint they had in mind, if they said:**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

---

## Signals observed

Mapped to [`../../VALIDATION_PLAN.md`](../../VALIDATION_PLAN.md) §3. Mark
`observed`, `not observed`, or `too early`. **`too early` is the correct answer
for all four before an engine exists** — do not upgrade it because the
conversation went well.

| # | Signal | Requirement | Downstream task | State |
| --- | --- | --- | --- | --- |
| 1 | Would import their own private checkpoint | `V1-29` | `QM-0161` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| 2 | Repeated use across weeks | `V1-31` | `QM-0164` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| 3 | A documented decision change | `V1-30` | `QM-0162` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |
| 4 | Willingness to pay | `V1-32` | `QM-0163` | `<TO BE FILLED BY A HUMAN — no agent may complete this>` |

**Not signals**, and not to be recorded as any: "this is cool", a follow on
social media, a star, an offer to retweet, conference interest.

Pricing is `QM-0163`'s task and is **out of scope here**. If they volunteered a
number, record it verbatim below and do not negotiate, quote, or confirm one.

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

---

## What I got wrong

**A belief I held before this conversation that is now weaker:**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Something they said that does not fit the plan:**

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**Did anything they said touch a pivot criterion?** *(*`VALIDATION_PLAN.md`* §5 —
diagnosis lands but the spatial view does not; quantisation is cold and something
else is hot; or the inverse, the 3D view is the reason they would pay)*

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

A record where this whole section is empty is usually a record of a conversation
where you talked too much.

---

## Follow-up owed

`<TO BE FILLED BY A HUMAN — no agent may complete this>`

**`outreach-log.csv` updated:** `<TO BE FILLED BY A HUMAN — no agent may complete this>`
