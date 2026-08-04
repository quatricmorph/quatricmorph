# QM-0085 — Runtime error audit and security review

## Status

Blocked

Unblocks when `QM-0152` reaches `Complete`. Scope narrows to the v1 surface and CLI.

**v1 dependency rewiring.** This task's `## Dependencies` section names tasks that are now `Deferred`. For v1 it is unblocked by the tasks named above; the original edges return with the post-v1 platform release. See [`EXECUTION_ORDER.md`](../../EXECUTION_ORDER.md) §10.

## Phase

Phase 08 — Integration and performance

## Objective

An empty browser console across the full manual checklist, and a security review
of every boundary added since the baseline.

## Repository Evidence

* `MVP-43` — *"The browser console contains no unresolved runtime errors."*
* `apps/web/quatricmorph-workspace/src/util/__tests__/params.test.ts` currently prints
  `error loading params from json '{not-json' …` — a **deliberate** log in a
  passing test, so the audit must distinguish expected from unresolved output.
* `SEC-001`…`SEC-005` Verified; `SEC-006`…`SEC-008` added by `QM-0075` and
  `QM-0050`.
* `crates/q-daemon` — every 501 carries a requirement ID.

## Requirements Covered

`MVP-43`, `SEC-007`, `SEC-008`; audits `SEC-001`…`SEC-006`.

## Dependencies

`QM-0080`, `QM-0075`.

## Blocks

`QM-0094`.

## Parallelization

Parallel with `QM-0081`…`QM-0084`.

## Program Boundary

Audit across both web applications and the daemon. Fixes land in the owning
module.

## Scope

* Capture console output across the full manual checklist and the end-to-end run.
* Classify every message: **expected** (a deliberate log, allowlisted with a
  reason) or **unresolved** (a bug).
* Verify daemon error bodies leak no absolute paths or internal state.
* Re-verify every `SEC-*` requirement against the current code.
* Check dependency licenses and known advisories.

## Out of Scope

New security features · penetration testing · fixing non-security bugs found,
which become their own tasks.

## Files Expected to Change

Whatever the audit finds. Expected: none to a handful.

## Files Expected to Add

* `apps/web/e2e/console-audit.spec.ts`
* `scripts/console-allowlist.json`
* `docs/SECURITY_AUDIT.md`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
// console-allowlist.json
[ { "pattern": "error loading params from json",
    "reason": "deliberate: params.test.ts asserts malformed JSON does not throw",
    "source": "apps/web/quatricmorph-workspace/src/util/params.ts" } ]
```

**Every allowlisted message needs a stated reason and a source.** An allowlist
without reasons becomes a way to hide errors, which is the opposite of the point.

## Memory and Performance Constraints

The console audit runs as part of the end-to-end job; it adds no meaningful time.

## Implementation Plan

1. Instrument Playwright to capture `console` and `pageerror` across every
   scenario.
2. Classify against the allowlist; fail on anything unlisted.
3. Fix or allowlist each finding, with a reason.
4. Audit daemon error bodies for path and state leakage.
5. Re-verify each `SEC-*` requirement against current code, recording the
   evidence.
6. Run a dependency license and advisory check.
7. Write `docs/SECURITY_AUDIT.md`.

## Error Handling

* An unlisted console message → **fail**, naming the message and the scenario.
* A `pageerror` → always a failure; never allowlistable.
* An error body containing an absolute path → fail; fix at the source.
* A dependency advisory → recorded; a high-severity one blocks the release.

## Acceptance Criteria

1. Zero unlisted console messages across the full checklist and the end-to-end
   run.
2. Zero `pageerror` events.
3. Every allowlist entry has a reason and a source.
4. No daemon error body contains an absolute path or internal state.
5. Every `SEC-*` requirement re-verified with cited evidence.
6. CORS is never `*`; the daemon binds `127.0.0.1` by default.
7. A CSP is present in both apps with no `unsafe-eval`.
8. Dependency licenses recorded; advisories triaged.
9. `docs/SECURITY_AUDIT.md` written and dated.

## Verification Plan

**Automated** — `console-audit.spec.ts` in CI; a dependency advisory check.
**Manual** — walk the checklist with the console open; review error bodies by
hand.

## Suggested Commands

```bash
npx playwright test apps/web/e2e/console-audit.spec.ts    # introduced here
cargo deny check                                           # introduced here
npm audit --workspaces
curl -s localhost:PORT/v1/tensors/bogus/value | jq         # check for leakage
```

## Test Cases

| Input | Expected |
| --- | --- |
| Full checklist | Zero unlisted console messages |
| Any `pageerror` | Test fails |
| A new deliberate log without an allowlist entry | Fails until documented |
| `GET /v1/tensors/bogus/value` | Error with no absolute path |
| Traversal attempt | 403, no path in the body |
| `Origin: http://evil.example` | 403 |
| CSP headers | No `unsafe-eval` |
| `cargo deny` | No high-severity advisories |
| Every `SEC-*` requirement | Re-verified with evidence |

## Risks

| Risk | Mitigation |
| --- | --- |
| The allowlist becomes a dumping ground | Every entry needs a reason and a source; reviewed at release |
| Console errors appear only in rare paths | The audit covers the full checklist, not a happy path |
| A dependency advisory appears after the audit | Dated; re-run before release |

## Completion Evidence

* Console capture from the full checklist, classified.
* The allowlist with reasons.
* Daemon error-body samples.
* `cargo deny` and `npm audit` output.
* `docs/SECURITY_AUDIT.md`, dated.
