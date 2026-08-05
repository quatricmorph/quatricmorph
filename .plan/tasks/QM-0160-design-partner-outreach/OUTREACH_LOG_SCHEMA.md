# Outreach-log schema

**[`outreach-log.csv`](outreach-log.csv) has a header row and zero data rows.**

Zero rows is the honest state: nothing has been sent. An agent may never add a
row. A fabricated row is not a placeholder, it is a false record of contact with
a real person, and it would make every genuine row after it worthless.

The log **is the artifact** — `TASK.md` §"Verification Plan" says so, and there
is no automated test that can stand in for it.

---

## 1. What criterion 8 requires of the schema

> *Conversations that went nowhere are logged too — a log of successes only is not
> evidence.*

That is a constraint on the columns, not on discipline. An `outcome` enum that
can only express progress makes criterion 8 unsatisfiable no matter how honest
the person filling it in is. So the enum below has more failure values than
success values, and `no-reply` is a first-class outcome rather than an empty
cell.

The other structural choice: **one row per attempt, appended, never edited except
to close it.** A row whose `outcome` was `no-reply` and later became
`conversation-held` is edited in place and its `outcome_date` updated; a row is
never deleted to make the log look better.

## 2. Columns

| Column | Type | Meaning |
| --- | --- | --- |
| `log_id` | `L-001`, … | Stable key, assigned in send order |
| `date_sent` | `YYYY-MM-DD` | The date the message actually went out. Criterion 2 requires it |
| `target_id` | `T-nnn` | Reference into [`target-list.csv`](target-list.csv). **No names in this file** |
| `segment` | enum | Copied from the target row, so the log stands alone for counting |
| `channel` | enum | `discord` \| `reddit` \| `email` \| `github` \| `forum` \| `referral` \| `in-person` \| `other`. Criterion 2 requires it |
| `template_used` | `A` \| `B` \| `C` \| `D` \| `E` \| `F` \| `custom` | Which [`MESSAGE_TEMPLATES.md`](MESSAGE_TEMPLATES.md) template. `custom` needs a reason in `notes` |
| `follow_up_date` | `YYYY-MM-DD` \| blank | The single permitted follow-up. Blank if none was sent |
| `outcome` | enum §3 | Current state of this attempt |
| `outcome_date` | `YYYY-MM-DD` | When `outcome` last changed |
| `conversation_record` | filename \| blank | e.g. `conversations/2026-08-nn-T-001.md`. Required when `outcome` is `conversation-held` |
| `q4_answer_recorded` | `yes` \| `no` \| `n/a` | Whether question 4's answer is recorded **verbatim** in the conversation record. Criterion 4 |
| `agreed_to_look` | `yes` \| `no` \| `not-asked` | Criterion 5 |
| `counts_toward_criteria` | `yes` \| `no` | `no` for `academic` segment and for anything not a genuine external contact |
| `notes` | free text | One line. Not a transcript — that belongs in the conversation record |

## 3. The `outcome` enum

| Value | Meaning | Counts toward |
| --- | --- | --- |
| `sent` | Out the door, nothing back yet, follow-up window still open | 2 |
| `bounced` | Never arrived — bad address, closed DMs, deleted account | **not** 2; it is not an attempt that reached anyone |
| `no-reply` | Follow-up window closed in silence | 2 |
| `declined` | Replied, not interested | 2 |
| `wrong-fit` | Replied; they do not make this decision. A qualification error worth learning from | 2 |
| `deferred` | Interested, not now. Includes a stated re-contact date in `notes` | 2 |
| `channel-blocked` | Removed by a moderator, or the channel's rules forbade it | **not** 2; log it anyway, it is a real cost |
| `conversation-scheduled` | Agreed to talk, has not happened | 2 |
| `conversation-held` | The four questions were asked. Requires `conversation_record` | 2 **and** 3 |
| `no-show` | Scheduled, did not happen | 2 |
| `agreed-to-look` | Held, **and** they agreed to look at a first diagnosis | 2, 3, **and** 5 |
| `withdrawn` | I stopped pursuing it. Reason in `notes` | 2 |

Nine of the twelve values describe something that did not work. That ratio is
deliberate: the realistic shape of ten cold contacts is mostly silence, and a log
that cannot represent silence is a log that will be quietly not kept.

`bounced` and `channel-blocked` are excluded from criterion 2's count of
"contacts attempted" because nobody was reached. They stay in the file because
they are the cheapest lesson available about channel choice.

## 4. Counting the criteria off the log

Once rows exist, these are the counts. Written now so the definitions are fixed
before there is any incentive to bend them.

| Criterion | Count | Threshold |
| --- | --- | --- |
| 2 — contacts attempted | Rows where `counts_toward_criteria = yes` and `outcome` not in {`bounced`, `channel-blocked`} | ≥ 10 |
| 3 — conversations held | Rows where `outcome` in {`conversation-held`, `agreed-to-look`} and `counts_toward_criteria = yes` | ≥ 3 |
| 4 — question 4 verbatim | Of those, rows where `q4_answer_recorded = yes` | all of them |
| 5 — partners who agreed to look | Rows where `agreed_to_look = yes` | ≥ 3 |
| 8 — failures logged | Rows with a failure `outcome` | > 0, and honestly the majority |

One person, one `target_id`, however many rows. Criterion 2 counts *attempts*;
criteria 3 and 5 count *people*, so deduplicate by `target_id` for those two.

## 5. Filling it in

Log at send time, not at the end of the week. `TASK.md` §"Completion Evidence"
asks for "date, channel, contact, outcome — including failures", and a date
reconstructed from memory a fortnight later is not a date.

**Rows in [`outreach-log.csv`](outreach-log.csv) at this commit: 0.
Contacts attempted: 0. Conversations held: 0. Partners agreed to look: 0.**
