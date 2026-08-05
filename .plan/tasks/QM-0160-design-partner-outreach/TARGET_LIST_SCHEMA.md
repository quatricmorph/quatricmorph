# Target-list schema

The criteria for who is worth approaching, and the structure that holds them.

**[`target-list.csv`](target-list.csv) has a header row and zero data rows, and
that is correct.** No candidate has been identified. No agent may add one — not a
name, not a handle, not a company, not a repository, not "an illustrative
example." A single invented row makes every real row that follows it
unverifiable.

---

## 1. Segments, and which ones count

From [`../../VALIDATION_PLAN.md`](../../VALIDATION_PLAN.md) §2, unchanged.

| Segment code | Segment | Their problem | Priority |
| --- | --- | --- | --- |
| `compression` | Model-compression / quantisation engineers | Which layers are fragile under quantisation? | **Beachhead — start here** |
| `infra` | AI infrastructure / serving teams | Route imbalance, wasted capacity | Second |
| `finetune` | Fine-tuning / LoRA / merge shops | Did this merge collide? | Third |
| `lab` | Foundation-model labs | Deep checkpoint understanding | Hard, long cycles |
| `academic` | Universities, open-source users | Teaching, papers | **Does not count** |

`academic` is in the enum so that time spent there is visible rather than
invisible. `TASK.md` §"Target segments" is explicit: universities and open-source
users are distribution and credibility, **not revenue**, and time spent there
does not count toward this task's criteria 2, 3, or 5. Log those contacts, then
exclude them from the counts.

Target: **3–5 design partners lined up before the engine works.** Not 30 leads.

## 2. Qualification — evidence of fit

A row may only be added when there is a **public, citable artifact** showing the
person makes or influences the decision the product is about. The `source_url`
column exists to hold that citation, and a row without one is not qualified.

Qualifying evidence, strongest first:

1. They have shipped or maintained a quantised checkpoint, or a quantisation
   pipeline, and said something about *which layers they kept at higher
   precision*.
2. They maintain, or are a heavy contributor to, a compression toolchain.
3. They have publicly debugged a quantisation regression — a config that
   surprised them. This is question 1 already answered in public, and it is the
   single best signal on this list.
4. They have published on layer sensitivity, mixed-precision assignment, or
   outlier-driven quantisation error.
5. They run inference at a scale where a precision decision has a serving-cost
   figure attached.

## 3. Disqualifiers

Do not add a row when any of these hold. Each one costs a send and returns
nothing.

* No public evidence they touch a precision decision. Interest in LLMs is not
  evidence.
* The only contact route is a generic company address with no named engineer
  behind it.
* The channel's rules forbid the approach. Check before, not after.
* Reaching them requires them to share a checkpoint. Question 4 asks whether they
  *would* run it on a checkpoint they cannot share; the ask itself must never
  require one.
* They are a personal friend or an existing colleague who would say yes to be
  kind. `TASK.md` §"Risks" names "only friendly contacts are approached" as a
  risk, and criterion 8 exists because of it.
* They are a competitor named in `VALIDATION_PLAN.md` §7's competitive watch.
  That is a different conversation with different rules.

## 4. Columns

| Column | Type | Meaning |
| --- | --- | --- |
| `target_id` | `T-001`, `T-002`, … | Stable key. The outreach log references this, never a name |
| `added_date` | `YYYY-MM-DD` | When the row was created |
| `segment` | enum §1 | `compression` \| `infra` \| `finetune` \| `lab` \| `academic` |
| `evidence_of_fit` | free text, one clause | Which of §2's five, concretely. "Works on ML" is not an answer |
| `source_url` | URL | The citable artifact behind `evidence_of_fit`. **Required** |
| `channel` | enum | `discord` \| `reddit` \| `email` \| `github` \| `forum` \| `referral` \| `in-person` \| `other` |
| `channel_rules_checked` | `yes` \| `no` | Whether that channel's self-promotion policy was read |
| `contact_handle` | free text | Handle or address as it exists in the channel |
| `contact_name` | free text | Name if publicly attached to the handle; blank otherwise |
| `priority` | `1` \| `2` \| `3` | 1 = beachhead with §2 evidence 1–3; 2 = beachhead, weaker evidence; 3 = other segment |
| `approach_notes` | free text | The one clause that goes into Template A's `<SPECIFIC_THING_THEY_PUBLISHED_OR_SHIPPED>` |
| `status` | enum | `candidate` \| `queued` \| `contacted` \| `retired` |
| `retired_reason` | free text | Why, if `retired`. Blank otherwise |

`contact_name` is a schema field, not an instruction to find names. Leave it blank
when the channel is pseudonymous — a handle is enough to send a message, and
attaching a real name to a pseudonymous handle is not this task's business.

## 5. Handling

* This file holds identifying information about real people. It stays local. Do
  not commit it populated to a public remote, and do not paste rows into an issue,
  a chat, or a model prompt.
* Delete a row on request. There is no counter-argument to that.
* [`outreach-log.csv`](outreach-log.csv) deliberately carries **no names** — only
  `target_id`. That is what makes the log shareable when the list is not.

## 6. Working order

Priority 1 first, and only widen when it is genuinely exhausted. Criterion 2 asks
for 10 contacts attempted; criterion 3 for 3 conversations held. A list of 40
priority-3 rows is a way of avoiding the 10 that matter.

**Rows in [`target-list.csv`](target-list.csv) at this commit: 0.**
