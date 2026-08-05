# Conversation records

**This directory contains zero conversation records, because zero conversations
have been held.**

One file per conversation, named `<YYYY-MM-DD>-<TARGET_ID>.md`, copied from
[`../CONVERSATION_RECORD_TEMPLATE.md`](../CONVERSATION_RECORD_TEMPLATE.md) and
filled in by hand by the person who held the conversation.

No agent may create a file here. Acceptance criteria 3 and 4 are counted by
listing this directory and checking each record's question-4 field, so a
fabricated file here would directly falsify a release gate.

```bash
# Conversation records held (expected: 0 at the QM-0160 scaffolding commit)
ls -1 .plan/tasks/QM-0160-design-partner-outreach/conversations/*-T-*.md 2>/dev/null | wc -l
```
