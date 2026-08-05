# QM-0160 — outreach scaffolding

**Nothing in this directory is a record of anything that happened.**

Every file here is a template or an empty structure. No design partner has been
contacted. No conversation has been held. No name, company, handle, reply, quote,
meeting, decision, price, or usage figure appears in any file in this directory,
and none may be added by an automated agent.

`QM-0160`'s acceptance criteria require real messages sent to real people outside
this repository. An agent cannot satisfy them, and an agent that produces
something *looking* like a satisfied criterion has destroyed the only thing the
log is for. The task's `## Status` is `Blocked` for that reason.

## The two marker kinds, and the difference between them

| Marker | Means | Who fills it |
| --- | --- | --- |
| `<PLACEHOLDER_IN_CAPITALS>` | A slot in reusable message copy — swap it per recipient when you send | A human, at send time |
| `<TO BE FILLED BY A HUMAN — no agent may complete this>` | A field in a **record**: something observed, decided, or received | A human, **only** — never an agent, never "as an example" |

The second marker is a single exact string. It is greppable on purpose:

```bash
grep -rc "TO BE FILLED BY A HUMAN" .plan/tasks/QM-0160-design-partner-outreach/
```

A file whose markers have all been replaced by real content is a real record. A
file with markers remaining is unfinished. There is no third state, and nothing
should ever be filled with a plausible-looking example.

## Files

| File | What it is | State |
| --- | --- | --- |
| [`VALUE_PROPOSITION.md`](VALUE_PROPOSITION.md) | The sentence, its two variants, the ratification the founder owes it, and the honesty block every message must carry | Sentence transcribed; ratification unfilled |
| [`MESSAGE_TEMPLATES.md`](MESSAGE_TEMPLATES.md) | The actual outreach copy — cold direct, in-context reply, follow-up, decline acknowledgement, scheduling | Complete, with `<PLACEHOLDER>` slots |
| [`TARGET_LIST_SCHEMA.md`](TARGET_LIST_SCHEMA.md) | Who qualifies, who does not, and the column definitions | Criteria complete; **no people** |
| [`target-list.csv`](target-list.csv) | The target list | Header row only, **zero data rows** |
| [`OUTREACH_LOG_SCHEMA.md`](OUTREACH_LOG_SCHEMA.md) | Column definitions and the outcome enum | Complete |
| [`outreach-log.csv`](outreach-log.csv) | The dated log | Header row only, **zero data rows** |
| [`INTERVIEW_GUIDE.md`](INTERVIEW_GUIDE.md) | The four questions verbatim, the protocol around them, and what not to do | Complete |
| [`CONVERSATION_RECORD_TEMPLATE.md`](CONVERSATION_RECORD_TEMPLATE.md) | One copy per conversation held | Template, every field unfilled |
| [`conversations/`](conversations/) | Where the filled-in records go | **Zero records** |
| [`INCEPTION_APPLICATION_PREP.md`](INCEPTION_APPLICATION_PREP.md) | The repository-grounded facts an NVIDIA Inception application needs, and where the confirmation goes | Facts complete; submission and confirmation unfilled |
| [`PALACE_READING_NOTE.md`](PALACE_READING_NOTE.md) | Where this design agrees with the Palace paper and where it departs | Structure only — **the paper has not been read** |

## The rule that governs the copy

Every claim in every template is traceable to [`../../../STATUS.md`](../../../STATUS.md)
or to a task specification in `.plan/`. Where the templates describe what
Quatricmorph does, they describe what it does **today**, which is much less than
what it is being built to do. `PRODUCT_SCOPE.md` §5.2's forbidden-claims table is
the standard, and every result is labelled `exact`, `sampled`, or `approximate`.

This is not modesty. The product's entire asset at this stage is being trusted
with a number an engineer will act on. A single oversold outreach message spends
that asset before the engine exists.
