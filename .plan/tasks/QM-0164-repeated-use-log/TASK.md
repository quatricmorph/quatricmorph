# QM-0164 — Repeated-use log

## Status

Blocked

Unblocks when `QM-0161` reaches `Complete`.

## Phase

Phase 14 — Validation and v1 release

## Objective

Establish whether anyone comes back. A single session is a demo; the same person
returning across weeks is the difference between interest and use.

## Repository Evidence

* `.plan/VALIDATION_PLAN.md` §3, signal 2: *"Repeated use — the same user returns
  for multiple sessions across weeks, not a single demo session."*

## Requirements Covered

`VAL-004`, `V1-31`.

## Dependencies

`QM-0161`.

## Blocks

`QM-0165`.

## Parallelization

Lane V. Runs over weeks in the background; it is a log, not a sprint.

## Program Boundary

No repository code. **No telemetry.**

## Scope

* A dated log of each partner's sessions: when, which model, which config, what
  they did with the output.
* Explicit note of who did not come back.
* Distinguish self-initiated returns from prompted ones.

## Out of Scope

Usage telemetry · analytics in the tool · a dashboard · nudging partners to
generate the signal.

## No telemetry — a deliberate choice

Adding usage reporting to a local tool that reads private checkpoints would be a
trust cost far larger than the measurement is worth, and this product's entire
pitch is that the checkpoint never leaves the machine. The log is therefore
manual, and it is smaller and more honest for it.

## The distinction that matters

A prompted return — *"could you try it on X for me?"* — is **not** signal 2. A
self-initiated return, where the partner ran it again without being asked, is.
The log records which, and the release audit reads the distinction rather than the
total.

## Acceptance Criteria

1. A dated session log per partner, spanning at least three weeks.
2. Each entry records: date, model, config, what they did with the output, and
   whether the session was self-initiated or prompted.
3. Partners who ran it once and did not return are listed explicitly.
4. A summary distinguishing self-initiated from prompted returns.
5. No telemetry was added to the tool.

## Verification Plan

**Manual.**

## Risks

| Risk | Mitigation |
| --- | --- |
| Prompted returns counted as organic | Criterion 2 records which, criterion 4 separates them |
| Non-returners omitted | Criterion 3 lists them by name |
| The window is too short to mean anything | At least three weeks |
| Telemetry added "just to measure" | Criterion 5; the trust cost is stated above |

## Completion Evidence

* The dated log.
* The list of non-returners.
* The self-initiated vs. prompted summary.
